use cap_recording::{
    RecordingMode,
    feeds::{
        camera::{CameraDeviceSettings, DeviceOrModelID},
        microphone::MicrophoneDeviceSettings,
    },
    sources::screen_capture::ScreenCaptureTarget,
};
use std::collections::HashMap;
use tauri::{AppHandle, Wry};
use tauri_plugin_store::StoreExt;

use crate::tray;

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Debug, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum RecordingTargetMode {
    Display,
    Window,
    Area,
    Camera,
}

#[derive(serde::Serialize, serde::Deserialize, specta::Type, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct RecordingSettingsStore {
    pub target: Option<ScreenCaptureTarget>,
    pub mic_name: Option<String>,
    pub camera_id: Option<DeviceOrModelID>,
    pub mode: Option<RecordingMode>,
    pub system_audio: bool,
    pub camera_device_settings: HashMap<String, CameraDeviceSettings>,
    pub microphone_device_settings: HashMap<String, MicrophoneDeviceSettings>,
}

impl RecordingSettingsStore {
    const KEY: &'static str = "recording_settings";

    pub fn get(app: &AppHandle<Wry>) -> Result<Option<Self>, String> {
        match app.store("store").map(|s| s.get(Self::KEY)) {
            Ok(Some(store)) => match serde_json::from_value(store) {
                Ok(settings) => Ok(Some(settings)),
                Err(e) => Err(format!("Failed to deserialize general settings store: {e}")),
            },
            _ => Ok(None),
        }
    }

    /// Read/modify/write of the whole settings blob. Every setter goes through
    /// here so a tray-side change can never drop the fields it did not touch.
    fn update(app: &AppHandle<Wry>, apply: impl FnOnce(&mut Self)) -> Result<(), String> {
        let store = app.store("store").map_err(|e| e.to_string())?;

        let mut settings = Self::get(app)?.unwrap_or_default();
        apply(&mut settings);

        store.set(Self::KEY, serde_json::json!(settings));
        store.save().map_err(|e| e.to_string())
    }

    pub fn set_mode(app: &AppHandle<Wry>, mode: RecordingMode) -> Result<(), String> {
        Self::update(app, |settings| settings.mode = Some(mode))
    }

    pub fn set_mic_name(app: &AppHandle<Wry>, name: Option<String>) -> Result<(), String> {
        Self::update(app, |settings| settings.mic_name = name)
    }

    pub fn set_camera_id(app: &AppHandle<Wry>, id: Option<DeviceOrModelID>) -> Result<(), String> {
        Self::update(app, |settings| settings.camera_id = id)
    }

    pub fn set_system_audio(app: &AppHandle<Wry>, enabled: bool) -> Result<(), String> {
        Self::update(app, |settings| settings.system_audio = enabled)
    }

    pub fn set_target(
        app: &AppHandle<Wry>,
        target: Option<ScreenCaptureTarget>,
    ) -> Result<(), String> {
        Self::update(app, |settings| settings.target = target)
    }

    pub fn camera_settings_for(
        app: &AppHandle<Wry>,
        id: &DeviceOrModelID,
    ) -> Option<CameraDeviceSettings> {
        Self::get(app).ok().flatten().and_then(|settings| {
            settings
                .camera_device_settings
                .get(&camera_key(id))
                .copied()
        })
    }

    pub fn microphone_settings_for(
        app: &AppHandle<Wry>,
        label: &str,
    ) -> Option<MicrophoneDeviceSettings> {
        Self::get(app)
            .ok()
            .flatten()
            .and_then(|settings| settings.microphone_device_settings.get(label).copied())
    }
}

pub fn camera_key(id: &DeviceOrModelID) -> String {
    match id {
        DeviceOrModelID::DeviceID(device_id) => format!("device:{device_id}"),
        DeviceOrModelID::ModelID(model_id) => format!("model:{model_id}"),
    }
}

#[tauri::command]
#[specta::specta]
pub fn set_recording_mode(app: AppHandle, mode: RecordingMode) -> Result<(), String> {
    RecordingSettingsStore::set_mode(&app, mode)?;
    tray::update_tray_icon_for_mode(&app, mode);
    Ok(())
}
