import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { check } from "@tauri-apps/plugin-updater";
import {
  enable as enableAutostart,
  disable as disableAutostart,
  isEnabled as isAutostartEnabled,
} from "@tauri-apps/plugin-autostart";
import "./App.css";

function App() {
  const [path, setPath] = useState("");
  const [status, setStatus] = useState("");
  const [autostart, setAutostart] = useState<boolean | null>(null);

  useEffect(() => {
    async function init() {
      try {
        const update = await check();
        if (update?.available) {
          setStatus("Update available. Downloading…");
          await update.downloadAndInstall();
          setStatus("Update installed. Restart the app to finish.");
        }
      } catch (_) {
        // ignore updater errors silently
      }
      const current = await invoke<string | null>("get_watched_path");
      if (current && current.length > 0) {
        setPath(current);
        setStatus("Watching folder");
      } else {
        const def = await invoke<string>("get_default_watched_path");
        setPath(def);
        try {
          await invoke("set_watched_path", { path: def });
          setStatus("Watching default folder");
        } catch (e) {
          setStatus("Default folder not found. Please pick a folder.");
        }
      }
      try {
        const on = await isAutostartEnabled();
        setAutostart(on);
        try {
          const initialized = localStorage.getItem("autostartInitialized");
          if (!initialized && !on) {
            await enableAutostart();
            setAutostart(true);
          }
          if (!initialized) {
            localStorage.setItem("autostartInitialized", "1");
          }
        } catch (_) {
          // ignore
        }
      } catch (_) {
        setAutostart(false);
      }
    }
    init();
  }, []);

  async function chooseFolder() {
    const dir = await open({ directory: true, multiple: false });
    if (dir && typeof dir === "string") {
      try {
        await invoke("set_watched_path", { path: dir });
        setPath(dir);
        setStatus("Watching folder");
      } catch (e) {
        setStatus("Failed to watch folder");
      }
    }
  }

  async function toggleAutostart() {
    if (autostart === null) return;
    try {
      if (autostart) {
        await disableAutostart();
        setAutostart(false);
      } else {
        await enableAutostart();
        setAutostart(true);
      }
    } catch (_) {
      // ignore
    }
  }

  return (
    <main className="container">
      <h1>Melee TV Uploader</h1>
      <div className="row" style={{ gap: 12, alignItems: "center" }}>
        <button onClick={chooseFolder}>Choose folder…</button>
        <span style={{ fontSize: 12, opacity: 0.8 }}>{path}</span>
      </div>
      <div
        className="row"
        style={{ gap: 12, alignItems: "center", marginTop: 12 }}
      >
        <label style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <input
            type="checkbox"
            checked={!!autostart}
            onChange={toggleAutostart}
            disabled={autostart === null}
          />
          Start at login
        </label>
      </div>
      <p style={{ marginTop: 8 }}>{status}</p>
      <p style={{ marginTop: 16, fontSize: 12, opacity: 0.7 }}>
        New .slp files in this folder will be uploaded automatically.
      </p>
    </main>
  );
}

export default App;
