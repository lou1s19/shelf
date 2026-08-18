//! Shelf has no update feed.
//!
//! Upstream pulled releases from Cap's CrabNebula CDN. Leaving that in place
//! would have replaced Shelf with Cap on the next release, so the whole check
//! is gone. The commands stay because the settings screen calls them; they
//! report "no update available" and reach no network.
//!
//! To switch updates back on, point [`check`] at your own release feed and
//! restore the background loop.

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::AppHandle;
use tokio::sync::Mutex;

#[derive(Serialize, Deserialize, Type, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub enum UpdateChannel {
    #[default]
    Stable,
    Nightly,
}

#[derive(Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResult {
    pub version: String,
    pub notes: Option<String>,
    pub channel: UpdateChannel,
}

#[derive(Serialize, Type, tauri_specta::Event, Clone, Debug)]
pub struct UpdateDownloadProgress {
    pub downloaded: u32,
    pub total: Option<u32>,
}

#[derive(Serialize, Type, tauri_specta::Event, Clone, Debug)]
pub struct UpdateReady {
    pub version: String,
    pub installed: bool,
}

#[derive(Default)]
pub struct UpdatesState {
    _pending: Mutex<Option<String>>,
}

#[tauri::command(async)]
#[specta::specta]
pub async fn updates_check(_app: AppHandle) -> Result<Option<UpdateCheckResult>, String> {
    Ok(None)
}

#[tauri::command(async)]
#[specta::specta]
pub async fn updates_download_and_install(_app: AppHandle) -> Result<(), String> {
    Err("Shelf has no update feed. Build the new version from source.".to_string())
}

#[tauri::command(async)]
#[specta::specta]
pub fn updates_channel_changed(_app: AppHandle) -> Result<(), String> {
    Ok(())
}

pub fn spawn_background_loop(_app: AppHandle) {}
