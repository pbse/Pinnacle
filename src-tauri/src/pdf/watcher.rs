use notify::{RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::channel;
use tauri::{AppHandle, Emitter};

#[tauri::command]
pub fn start_folder_watcher(handle: AppHandle, path: String) -> Result<(), String> {
    let path_clone = path.clone();

    std::thread::spawn(move || {
        let (tx, rx) = channel();
        let mut watcher = notify::recommended_watcher(tx).unwrap();

        watcher
            .watch(Path::new(&path_clone), RecursiveMode::Recursive)
            .unwrap();

        for res in rx {
            match res {
                Ok(event) => {
                    if event.kind.is_create() {
                        for path in event.paths {
                            if path.extension().map_or(false, |ext| ext == "pdf") {
                                // Emit event to frontend
                                handle.emit("pdf-created", path.to_str().unwrap()).unwrap();
                            }
                        }
                    }
                }
                Err(e) => println!("watch error: {:?}", e),
            }
        }
    });

    Ok(())
}
