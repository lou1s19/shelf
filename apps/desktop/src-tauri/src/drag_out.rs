use std::path::{Path, PathBuf};

use tauri::{Manager, Window, Wry};
use tracing::instrument;

/// Only files Cap itself produced may be dragged out. Without this a compromised
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

#[tauri::command]
#[specta::specta]
#[instrument(skip(window))]
pub async fn start_file_drag(window: Window<Wry>, path: PathBuf) -> Result<(), String> {
    if !path.is_file() {
        return Err(format!("File not found: {}", path.display()));
    }

    let app = window.app_handle().clone();

    if !is_allowed(&app, &path) {
        return Err(format!(
            "Refusing to drag a file outside Cap's own folders: {}",
            path.display()
        ));
    }

    let (tx, rx) = tokio::sync::oneshot::channel();

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

    rx.await.map_err(|e| e.to_string())?
}
