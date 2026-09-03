//! Shelf's update feed.
//!
//! Upstream pulled releases from Cap's CDN; that endpoint is gone, and pointing
//! at it would replace Shelf with a different app. What runs now is Shelf's own
//! feed, configured in `tauri.prod.conf.json`. The signature is checked against
//! the public key in the same file, so a hijacked domain still cannot install
//! anything.
//!
//! With no endpoint configured (the default dev build) every command here
//! reports "nothing to update" and reaches no network.

use serde::{Deserialize, Serialize};
use specta::Type;
use std::time::Duration;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

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
    /// Guards against two windows installing the same update at once.
    installing: Mutex<()>,
}

fn has_feed(app: &AppHandle) -> bool {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|updater| updater.get("endpoints"))
        .and_then(|endpoints| endpoints.as_array())
        .is_some_and(|endpoints| !endpoints.is_empty())
}

async fn find_update(app: &AppHandle) -> Result<Option<tauri_plugin_updater::Update>, String> {
    if !has_feed(app) {
        debug!("updates: no feed configured");
        return Ok(None);
    }

    let updater = app
        .updater_builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| match e {
            tauri_plugin_updater::Error::EmptyEndpoints => {
                "Shelf has no update feed in this build.".to_string()
            }
            e => e.to_string(),
        })?;

    updater.check().await.map_err(|e| e.to_string())
}

#[tauri::command(async)]
#[specta::specta]
pub async fn updates_check(app: AppHandle) -> Result<Option<UpdateCheckResult>, String> {
    Ok(find_update(&app).await?.map(|update| UpdateCheckResult {
        version: update.version.clone(),
        notes: update.body.clone(),
        channel: UpdateChannel::Stable,
    }))
}

#[tauri::command(async)]
#[specta::specta]
pub async fn updates_download_and_install(app: AppHandle) -> Result<(), String> {
    use tauri::Manager;
    use tauri_specta::Event;

    let state = app.state::<UpdatesState>();
    let _installing = state
        .installing
        .try_lock()
        .map_err(|_| "An update is already being installed.".to_string())?;

    let update = find_update(&app)
        .await?
        .ok_or_else(|| "There is no newer version to install.".to_string())?;

    let version = update.version.clone();
    let mut downloaded = 0u32;

    update
        .download_and_install(
            |chunk, total| {
                downloaded = downloaded.saturating_add(chunk as u32);
                let _ = UpdateDownloadProgress {
                    downloaded,
                    total: total.map(|total| total.min(u32::MAX as u64) as u32),
                }
                .emit(&app);
            },
            || {
                debug!("updates: download finished, installing");
            },
        )
        .await
        .map_err(|e| e.to_string())?;

    info!("updates: {version} installed, waiting for a restart");
    let _ = UpdateReady {
        version,
        installed: true,
    }
    .emit(&app);

    Ok(())
}

#[tauri::command(async)]
#[specta::specta]
pub fn updates_channel_changed(_app: AppHandle) -> Result<(), String> {
    // Shelf publishes one feed. The command stays because the settings screen
    // calls it; a second channel would be a second endpoint here.
    Ok(())
}

pub fn spawn_background_loop(app: AppHandle) {
    if !has_feed(&app) {
        return;
    }

    tauri::async_runtime::spawn(async move {
        loop {
            match find_update(&app).await {
                // Nothing is opened here on purpose. A new version is offered
                // in Settings; forcing an install is the update floor's job,
                // and that one explains itself to the user.
                Ok(Some(update)) => info!("updates: {} is available", update.version),
                Ok(None) => debug!("updates: already up to date"),
                Err(e) => warn!("updates: check failed: {e}"),
            }
            tokio::time::sleep(CHECK_INTERVAL).await;
        }
    });
}
