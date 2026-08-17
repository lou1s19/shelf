use std::{path::PathBuf, sync::mpsc::channel};

use tauri::{Manager, Window, Wry};
use tracing::instrument;

#[tauri::command]
#[specta::specta]
#[instrument(skip(window))]
pub async fn start_file_drag(window: Window<Wry>, path: PathBuf) -> Result<(), String> {
    if !path.is_file() {
        return Err(format!("File not found: {}", path.display()));
    }

    let (tx, rx) = channel();
    let app = window.app_handle().clone();

    app.run_on_main_thread(move || {
        #[cfg(target_os = "linux")]
        let drag_window = window.gtk_window().map_err(|e| e.to_string());
        #[cfg(not(target_os = "linux"))]
        let drag_window = Ok::<_, String>(window.clone());

        let result = drag_window.and_then(|drag_window| {
            drag::start_drag(
                &drag_window,
                drag::DragItem::Files(vec![path.clone()]),
                drag::Image::File(path),
                |_, _| {},
                drag::Options::default(),
            )
            .map_err(|e| e.to_string())
        });

        let _ = tx.send(result);
    })
    .map_err(|e| e.to_string())?;

    rx.recv().map_err(|e| e.to_string())?
}
