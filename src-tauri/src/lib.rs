// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use tauri::{menu::{MenuBuilder, MenuItemBuilder}, tray::TrayIconBuilder, AppHandle, Manager};

    let context = tauri::generate_context!();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet])
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
