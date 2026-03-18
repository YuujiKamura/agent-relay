use crate::error::Result;
use crate::session;
use std::collections::HashMap;
use std::path::PathBuf;

pub fn run() -> Result<()> {
    let dir = session_dir();
    if !dir.exists() {
        eprintln!("[clean] Session directory does not exist: {}", dir.display());
        return Ok(());
    }

    let Ok(entries) = std::fs::read_dir(&dir) else {
        eprintln!("[clean] Cannot read directory: {}", dir.display());
        return Ok(());
    };

    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("session") {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let mut map = HashMap::new();
        for line in content.lines() {
            if let Some((key, value)) = line.split_once('=') {
                map.insert(key.trim().to_string(), value.trim().to_string());
            }
        }
        let Some(pid) = map.get("pid").and_then(|p| p.parse::<u32>().ok()) else {
            continue;
        };
        if !session::is_process_alive(pid) {
            match std::fs::remove_file(&path) {
                Ok(_) => {
                    eprintln!("[clean] Removed {} (pid {} dead)", path.display(), pid);
                    removed += 1;
                }
                Err(e) => {
                    eprintln!("[clean] Failed to remove {}: {}", path.display(), e);
                }
            }
        }
    }

    eprintln!("[clean] Removed {} dead session file(s).", removed);
    Ok(())
}

fn session_dir() -> PathBuf {
    let local_app = std::env::var("LOCALAPPDATA").unwrap_or_default();
    PathBuf::from(local_app).join("WindowsTerminal/control-plane/winui3/sessions")
}
