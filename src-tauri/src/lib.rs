// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use std::{collections::HashSet, env, fs, path::{Path, PathBuf}, sync::{Arc, Mutex}, thread, time::Duration};

use notify::{Config as NotifyConfig, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

fn resolve_server_url() -> String {
    if let Ok(val) = env::var("MELEE_TV_UPLOAD_URL") {
        if !val.is_empty() {
            return val;
        }
    }
    if cfg!(debug_assertions) {
        "http://localhost:3000".to_string()
    } else {
        "https://meleetv.boilerroom.tech".to_string()
    }
}

#[derive(Default)]
struct WatchState {
    watched_path: Option<PathBuf>,
    watcher: Option<RecommendedWatcher>,
    baseline_paths: HashSet<PathBuf>,
}

type SharedWatchState = Arc<Mutex<WatchState>>;

fn default_slippi_dir() -> PathBuf {
    let mut base = dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"));
    base.push("Documents");
    base.push("slippi");
    base
}

fn is_slp_file(path: &Path) -> bool {
    path.extension().map(|e| e.eq_ignore_ascii_case("slp")).unwrap_or(false)
}

fn collect_existing_slp_files(root: &Path) -> HashSet<PathBuf> {
    let mut set = HashSet::new();
    fn walk(dir: &Path, out: &mut HashSet<PathBuf>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, out);
                } else if path.is_file() && is_slp_file(&path) {
                    out.insert(path);
                }
            }
        }
    }
    walk(root, &mut set);
    set
}

fn wait_until_stable(path: &Path) -> std::io::Result<()> {
    // Wait briefly until file size stops changing to avoid partial reads
    let mut last_len = 0u64;
    for _ in 0..10 {
        let len = std::fs::metadata(path)?.len();
        if len > 0 && len == last_len {
            return Ok(());
        }
        last_len = len;
        thread::sleep(Duration::from_millis(150));
    }
    Ok(())
}

fn upload_file(file_path: &Path) {
    // Blocking upload in a dedicated thread; small and simple
    let path_str = file_path.to_string_lossy().to_string();
    thread::spawn(move || {
        let client = match reqwest::blocking::Client::builder().build() {
            Ok(c) => c,
            Err(_) => return,
        };
        // Send multipart where field name is "file" to match backend's req.file()
        let form = match reqwest::blocking::multipart::Form::new().file("file", &path_str) {
            Ok(f) => f,
            Err(_) => return,
        };
        let base = resolve_server_url();
        let url = format!("{}/upload", base);
        let _ = client.post(url).multipart(form).send();
    });
}

fn start_watcher(app: &tauri::AppHandle, state: &SharedWatchState, dir: PathBuf) -> Result<(), String> {
    // Drop existing watcher if any
    {
        let mut s = state.lock().unwrap();
        s.watcher.take();
        s.watched_path = Some(dir.clone());
        s.baseline_paths = collect_existing_slp_files(&dir);
    }

    let state_ref = state.clone();

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                // We care about Create events (new files) and also Rename to target
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Any => {
                        for path in event.paths {
                            if path.is_file() && is_slp_file(&path) {
                                // Ensure file belongs to current watched directory
                                let (watched_ok, already_seen) = {
                                    let mut s = state_ref.lock().unwrap();
                                    let in_dir = if let Some(ref base) = s.watched_path { path.starts_with(base) } else { true };
                                    let seen = s.baseline_paths.contains(&path);
                                    if !seen { s.baseline_paths.insert(path.clone()); }
                                    (in_dir, seen)
                                };
                                if !watched_ok { continue; }
                                if already_seen { continue; }
                                let p = path.clone();
                                tauri::async_runtime::spawn_blocking(move || {
                                    let _ = wait_until_stable(&p);
                                    upload_file(&p);
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
        },
        NotifyConfig::default(),
    ).map_err(|e| e.to_string())?;

    watcher
        .watch(&dir, RecursiveMode::Recursive)
        .map_err(|e| e.to_string())?;

    // Keep watcher alive in state
    let mut s = state.lock().unwrap();
    s.watcher = Some(watcher);
    Ok(())
}

#[tauri::command]
fn get_default_watched_path() -> String {
    default_slippi_dir().to_string_lossy().to_string()
}

#[tauri::command]
fn get_watched_path(state: tauri::State<SharedWatchState>) -> Option<String> {
    state
        .lock()
        .unwrap()
        .watched_path
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
fn set_watched_path(path: String, app: tauri::AppHandle, state: tauri::State<SharedWatchState>) -> Result<(), String> {
    let dir = PathBuf::from(path);
    if !dir.exists() {
        return Err("Directory does not exist".into());
    }
    start_watcher(&app, &state, dir)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use tauri::{menu::{MenuBuilder, MenuItemBuilder}, tray::TrayIconBuilder, AppHandle, Manager};

    let context = tauri::generate_context!();

    let watch_state: SharedWatchState = Arc::new(Mutex::new(WatchState::default()));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![get_default_watched_path, get_watched_path, set_watched_path])
        .manage(watch_state)
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .setup(|app| {
            // Build a minimal tray with a single item to toggle the main window
            let app_handle: AppHandle = app.handle().clone();

            // On macOS, set accessory policy to hide Dock icon for tray-first apps
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Create a simple tray menu
            let toggle_id = "toggle-window";
            let quit_id = "quit";
            let menu = MenuBuilder::new(app)
                .item(&MenuItemBuilder::with_id(toggle_id, "Show / Hide").build(app)?)
                .separator()
                .item(&MenuItemBuilder::with_id(quit_id, "Quit").build(app)?)
                .build()?;

            let _tray = TrayIconBuilder::new()
                // use app icon by default; platform picks the best variant
                .on_tray_icon_event(move |_tray, event| {
                    // Toggle window on left click
                    if let tauri::tray::TrayIconEvent::Click { .. } = event {
                        if let Some(window) = app_handle.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .menu(&menu)
                .build(app)?;
            // Initialize default watcher if none set yet
            let default_dir = default_slippi_dir();
            if let Err(err) = start_watcher(&app.handle(), &app.state::<SharedWatchState>(), default_dir) {
                let _ = err; // ignore; user can set later
            }

            Ok(())
        })
        .on_menu_event(|app, event| {
            match event.id().as_ref() {
                "quit" => {
                    app.exit(0);
                }
                "toggle-window" => {
                    if let Some(window) = app.get_webview_window("main") {
                        if window.is_visible().unwrap_or(false) {
                            let _ = window.hide();
                        } else {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                }
                _ => {}
            }
        })
        .run(context)
        .expect("error while running tauri application");
}
