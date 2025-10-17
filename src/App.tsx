import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { check } from "@tauri-apps/plugin-updater";
import "./App.css";

function App() {
  const [path, setPath] = useState("");
  const [status, setStatus] = useState("");

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

  return (
    <main className="container">
      <h1>Melee TV Uploader</h1>
      <div className="row" style={{ gap: 12, alignItems: "center" }}>
        <button onClick={chooseFolder}>Choose folder…</button>
        <span style={{ fontSize: 12, opacity: 0.8 }}>{path}</span>
      </div>
      <p style={{ marginTop: 8 }}>{status}</p>
      <p style={{ marginTop: 16, fontSize: 12, opacity: 0.7 }}>
        New .slp files in this folder will be uploaded automatically.
      </p>
    </main>
  );
}

export default App;
