use std::path::{Path, PathBuf};

use tauri::{Manager, Window, Wry};
use tracing::instrument;

/// Only files Shelf itself produced may be dragged out. Without this a compromised
/// webview could hand any readable file on the machine to another app.
fn is_allowed(app: &tauri::AppHandle<Wry>, path: &Path) -> bool {
    let Ok(path) = path.canonicalize() else {
        return false;
    };

    [
        crate::general_settings::GeneralSettingsStore::recordings_dir(app),
        app.path()
            .app_data_dir()
            .map(|dir| dir.join("screenshots"))
            .unwrap_or_default(),
    ]
    .iter()
    .filter_map(|dir| dir.canonicalize().ok())
    .any(|dir| path.starts_with(dir))
}

/// Returns `true` once the file was actually dropped somewhere, `false` when the drag was
/// cancelled. Callers use that to decide whether the dragged item may disappear from their list.
#[tauri::command]
#[specta::specta]
#[instrument(skip(window))]
pub async fn start_file_drag(window: Window<Wry>, path: PathBuf) -> Result<bool, String> {
    if !path.is_file() {
        return Err(format!("File not found: {}", path.display()));
    }

    let app = window.app_handle().clone();

    if !is_allowed(&app, &path) {
        return Err(format!(
            "Refusing to drag a file outside Shelf's own folders: {}",
            path.display()
        ));
    }

    // Checked as the real file, dragged as a link that carries the project's own name: dropping
    // three screenshots on the desktop must not produce three files called `original.png`.
    let path = crate::recording::shareable_file_path(&path);

    let (tx, rx) = tokio::sync::oneshot::channel();
    // The drop result arrives later, from the drag session's callback. It is a `Fn`, so the
    // sender lives behind a mutex and is taken by whichever call comes first.
    let (drop_tx, drop_rx) = tokio::sync::oneshot::channel();
    let drop_tx = std::sync::Arc::new(std::sync::Mutex::new(Some(drop_tx)));

    app.run_on_main_thread(move || {
        #[cfg(target_os = "linux")]
        let drag_window = window.gtk_window().map_err(|e| e.to_string());
        #[cfg(not(target_os = "linux"))]
        let drag_window = Ok::<_, String>(window.clone());

        let drop_tx_for_callback = drop_tx.clone();
        let result = drag_window.and_then(|drag_window| {
            drag::start_drag(
                &drag_window,
                drag::DragItem::Files(vec![path.clone()]),
                drag::Image::File(path),
                move |result, _| {
                    let mut slot = drop_tx_for_callback
                        .lock()
                        .unwrap_or_else(|e| e.into_inner());
                    if let Some(sender) = slot.take() {
                        let _ = sender.send(matches!(result, drag::DragResult::Dropped));
                    }
                },
                drag::Options::default(),
            )
            .map_err(|e| e.to_string())
        });

        // A drag that never started can never report a drop.
        if result.is_err() {
            let mut slot = drop_tx.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(sender) = slot.take() {
                let _ = sender.send(false);
            }
        }

        let _ = tx.send(result);
    })
    .map_err(|e| e.to_string())?;

    rx.await.map_err(|e| e.to_string())??;

    // A dropped sender means the drag session went away without reporting; treat that as cancelled.
    Ok(drop_rx.await.unwrap_or(false))
}
