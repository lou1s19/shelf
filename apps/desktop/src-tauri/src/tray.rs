use crate::{
    NewScreenshotAdded, NewStudioRecordingAdded, RecordingStarted, RecordingStopped,
    RequestOpenSettings, recording,
    recording_settings::{RecordingSettingsStore, RecordingTargetMode},
    windows::ShowCapWindow,
};
use cap_recording::{RecordingMode, feeds::camera::DeviceOrModelID};

use cap_project::{RecordingMeta, RecordingMetaInner};
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tauri::menu::{CheckMenuItem, IconMenuItem, MenuId, PredefinedMenuItem, Submenu};
use tauri::{
    AppHandle,
    image::Image,
    menu::{Menu, MenuItem},
    tray::{TrayIcon, TrayIconBuilder},
};
use tauri::{Listener, Manager};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use tauri_specta::Event;

const PREVIOUS_ITEM_PREFIX: &str = "previous_item_";
const CAMERA_ITEM_PREFIX: &str = "camera_item_";
const MIC_ITEM_PREFIX: &str = "mic_item_";
const MAX_PREVIOUS_ITEMS: usize = 6;
const MAX_TITLE_LENGTH: usize = 30;
const THUMBNAIL_SIZE: u32 = 32;

#[cfg(target_os = "linux")]
#[derive(Clone, Copy)]
enum LinuxTrayIcon {
    Default,
    Instant,
    Screenshot,
    Studio,
    Stop,
}

#[cfg(target_os = "linux")]
impl LinuxTrayIcon {
    fn name(self) -> &'static str {
        match self {
            Self::Default => "so.cap.desktop-tray-default-symbolic",
            Self::Instant => "so.cap.desktop-tray-instant-symbolic",
            Self::Screenshot => "so.cap.desktop-tray-screenshot-symbolic",
            Self::Studio => "so.cap.desktop-tray-studio-symbolic",
            Self::Stop => "so.cap.desktop-tray-stop-symbolic",
        }
    }

    fn svg(self) -> &'static str {
        match self {
            Self::Default => {
                include_str!("../icons/linux/so.cap.desktop-tray-default-symbolic.svg")
            }
            Self::Instant => {
                include_str!("../icons/linux/so.cap.desktop-tray-instant-symbolic.svg")
            }
            Self::Screenshot => {
                include_str!("../icons/linux/so.cap.desktop-tray-screenshot-symbolic.svg")
            }
            Self::Studio => include_str!("../icons/linux/so.cap.desktop-tray-studio-symbolic.svg"),
            Self::Stop => include_str!("../icons/linux/so.cap.desktop-tray-stop-symbolic.svg"),
        }
    }
}

#[derive(Debug)]
pub enum TrayItem {
    OpenCap,
    StartStopRecording,
    RecordDisplay,
    RecordWindow,
    RecordArea,
    TakeScreenshot,
    ImportVideo,
    ViewAllRecordings,
    ViewAllScreenshots,
    OpenSettings,
    Quit,
    PreviousItem(String),
    ModeStudio,
    ModeInstant,
    ModeScreenshot,
    RequestPermissions,
    /// Empty payload means "no camera".
    SelectCamera(String),
    /// Empty payload means "no microphone".
    SelectMicrophone(String),
    ToggleSystemAudio,
}

impl From<TrayItem> for MenuId {
    fn from(value: TrayItem) -> Self {
        match value {
            TrayItem::OpenCap => "open_cap",
            TrayItem::StartStopRecording => "start_stop_recording",
            TrayItem::RecordDisplay => "record_display",
            TrayItem::RecordWindow => "record_window",
            TrayItem::RecordArea => "record_area",
            TrayItem::TakeScreenshot => "take_screenshot",
            TrayItem::ImportVideo => "import_video",
            TrayItem::ViewAllRecordings => "view_all_recordings",
            TrayItem::ViewAllScreenshots => "view_all_screenshots",
            TrayItem::OpenSettings => "open_settings",
            TrayItem::Quit => "quit",
            TrayItem::PreviousItem(id) => {
                return format!("{PREVIOUS_ITEM_PREFIX}{id}").into();
            }
            TrayItem::ModeStudio => "mode_studio",
            TrayItem::ModeInstant => "mode_instant",
            TrayItem::ModeScreenshot => "mode_screenshot",
            TrayItem::RequestPermissions => "request_permissions",
            TrayItem::SelectCamera(id) => {
                return format!("{CAMERA_ITEM_PREFIX}{id}").into();
            }
            TrayItem::SelectMicrophone(name) => {
                return format!("{MIC_ITEM_PREFIX}{name}").into();
            }
            TrayItem::ToggleSystemAudio => "toggle_system_audio",
        }
        .into()
    }
}

impl TryFrom<MenuId> for TrayItem {
    type Error = String;

    fn try_from(value: MenuId) -> Result<Self, Self::Error> {
        let id_str = value.0.as_str();

        if let Some(path) = id_str.strip_prefix(PREVIOUS_ITEM_PREFIX) {
            return Ok(TrayItem::PreviousItem(path.to_string()));
        }

        if let Some(id) = id_str.strip_prefix(CAMERA_ITEM_PREFIX) {
            return Ok(TrayItem::SelectCamera(id.to_string()));
        }

        if let Some(name) = id_str.strip_prefix(MIC_ITEM_PREFIX) {
            return Ok(TrayItem::SelectMicrophone(name.to_string()));
        }

        match id_str {
            "open_cap" => Ok(TrayItem::OpenCap),
            "start_stop_recording" => Ok(TrayItem::StartStopRecording),
            "record_display" => Ok(TrayItem::RecordDisplay),
            "record_window" => Ok(TrayItem::RecordWindow),
            "record_area" => Ok(TrayItem::RecordArea),
            "take_screenshot" => Ok(TrayItem::TakeScreenshot),
            "import_video" => Ok(TrayItem::ImportVideo),
            "view_all_recordings" => Ok(TrayItem::ViewAllRecordings),
            "view_all_screenshots" => Ok(TrayItem::ViewAllScreenshots),
            "open_settings" => Ok(TrayItem::OpenSettings),
            "quit" => Ok(TrayItem::Quit),
            "mode_studio" => Ok(TrayItem::ModeStudio),
            "mode_instant" => Ok(TrayItem::ModeInstant),
            "mode_screenshot" => Ok(TrayItem::ModeScreenshot),
            "request_permissions" => Ok(TrayItem::RequestPermissions),
            "toggle_system_audio" => Ok(TrayItem::ToggleSystemAudio),
            value => Err(format!("Invalid tray item id {value}")),
        }
    }
}

#[derive(Debug, Clone)]
enum PreviousItemType {
    StudioRecording,
    InstantRecording {
        #[allow(dead_code)]
        link: Option<String>,
    },
    Screenshot,
}

#[derive(Clone)]
struct CachedPreviousItem {
    path: PathBuf,
    pretty_name: String,
    thumbnail: Option<Vec<u8>>,
    thumbnail_width: u32,
    thumbnail_height: u32,
    item_type: PreviousItemType,
    created_at: std::time::SystemTime,
}

#[derive(Default)]
struct PreviousItemsCache {
    items: Vec<CachedPreviousItem>,
}

fn screenshots_path(app: &AppHandle) -> PathBuf {
    let path = app.path().app_data_dir().unwrap().join("screenshots");
    std::fs::create_dir_all(&path).unwrap_or_default();
    path
}

fn truncate_title(title: &str) -> String {
    if title.chars().count() <= MAX_TITLE_LENGTH {
        title.to_string()
    } else {
        let truncate_at = MAX_TITLE_LENGTH - 1;
        let byte_index = title
            .char_indices()
            .nth(truncate_at)
            .map(|(i, _)| i)
            .unwrap_or(title.len());
        format!("{}…", &title[..byte_index])
    }
}

fn load_thumbnail_data(path: &PathBuf) -> Option<(Vec<u8>, u32, u32)> {
    use image::imageops::FilterType;
    use image::{GenericImageView, RgbaImage};

    let image_data = std::fs::read(path).ok()?;
    let img = image::load_from_memory(&image_data).ok()?;

    let (orig_w, orig_h) = img.dimensions();
    let size = THUMBNAIL_SIZE;

    let scale = (size as f32 / orig_w as f32).max(size as f32 / orig_h as f32);
    let scaled_w = (orig_w as f32 * scale).round() as u32;
    let scaled_h = (orig_h as f32 * scale).round() as u32;

    let scaled = img.resize_exact(scaled_w, scaled_h, FilterType::Triangle);

    let x_offset = (scaled_w.saturating_sub(size)) / 2;
    let y_offset = (scaled_h.saturating_sub(size)) / 2;

    let mut result = RgbaImage::new(size, size);
    for y in 0..size {
        for x in 0..size {
            let src_x = x + x_offset;
            let src_y = y + y_offset;
            if src_x < scaled_w && src_y < scaled_h {
                result.put_pixel(x, y, scaled.get_pixel(src_x, src_y));
            }
        }
    }

    Some((result.into_raw(), size, size))
}

fn load_single_item(
    path: &PathBuf,
    screenshots_dir: &PathBuf,
    load_thumbnail: bool,
) -> Option<CachedPreviousItem> {
    if !path.is_dir() {
        return None;
    }

    let meta = RecordingMeta::load_for_project(path).ok()?;
    let created_at = path
        .metadata()
        .and_then(|m| m.created())
        .unwrap_or_else(|_| std::time::SystemTime::now());

    let is_screenshot = path.extension().and_then(|s| s.to_str()) == Some("cap")
        && path.parent().map(|p| p == screenshots_dir).unwrap_or(false);

    let (thumbnail_path, item_type) = if is_screenshot {
        let png_path = std::fs::read_dir(path).ok().and_then(|dir| {
            dir.flatten()
                .find(|e| e.path().extension().and_then(|s| s.to_str()) == Some("png"))
                .map(|e| e.path())
        });
        (png_path, PreviousItemType::Screenshot)
    } else {
        let thumb = path.join("screenshots/display.jpg");
        let thumb_path = if thumb.exists() { Some(thumb) } else { None };
        let item_type = match &meta.inner {
            RecordingMetaInner::Studio(_) => PreviousItemType::StudioRecording,
            RecordingMetaInner::Instant(_) => PreviousItemType::InstantRecording {
                link: meta.sharing.as_ref().map(|s| s.link.clone()),
            },
        };
        (thumb_path, item_type)
    };

    let (thumbnail, thumbnail_width, thumbnail_height) = if load_thumbnail {
        thumbnail_path
            .as_ref()
            .and_then(load_thumbnail_data)
            .map(|(data, w, h)| (Some(data), w, h))
            .unwrap_or((None, 0, 0))
    } else {
        (None, 0, 0)
    };

    Some(CachedPreviousItem {
        path: path.clone(),
        pretty_name: meta.pretty_name,
        thumbnail,
        thumbnail_width,
        thumbnail_height,
        item_type,
        created_at,
    })
}

fn load_all_previous_items(app: &AppHandle, load_thumbnails: bool) -> Vec<CachedPreviousItem> {
    let mut items = Vec::new();
    let screenshots_dir = screenshots_path(app);

    for recordings_dir in crate::recordings_locations::known_recordings_dirs(app) {
        let Ok(entries) = std::fs::read_dir(&recordings_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if let Some(item) = load_single_item(&entry.path(), &screenshots_dir, load_thumbnails) {
                items.push(item);
            }
        }
    }

    if screenshots_dir.exists()
        && let Ok(entries) = std::fs::read_dir(&screenshots_dir)
    {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("cap")
                && let Some(item) = load_single_item(&path, &screenshots_dir, load_thumbnails)
            {
                items.push(item);
            }
        }
    }

    items.sort_by_key(|b| std::cmp::Reverse(b.created_at));
    items.truncate(MAX_PREVIOUS_ITEMS);
    items
}

fn create_previous_submenu(
    app: &AppHandle,
    cache: &PreviousItemsCache,
) -> tauri::Result<Submenu<tauri::Wry>> {
    if cache.items.is_empty() {
        let submenu = Submenu::with_id(app, "previous", "Previous", false)?;
        submenu.append(&MenuItem::with_id(
            app,
            "previous_empty",
            "No recent items",
            false,
            None::<&str>,
        )?)?;
        return Ok(submenu);
    }

    let submenu = Submenu::with_id(app, "previous", "Previous", true)?;

    for item in &cache.items {
        let id = TrayItem::PreviousItem(item.path.to_string_lossy().to_string());
        let title = truncate_title(&item.pretty_name);

        let type_indicator = match &item.item_type {
            PreviousItemType::StudioRecording => "🎬 ",
            PreviousItemType::InstantRecording { .. } => "⚡ ",
            PreviousItemType::Screenshot => "📷 ",
        };
        let display_title = format!("{type_indicator}{title}");

        let icon = item.thumbnail.as_ref().map(|data| {
            Image::new_owned(data.clone(), item.thumbnail_width, item.thumbnail_height)
        });

        let menu_item = IconMenuItem::with_id(app, id, display_title, true, icon, None::<&str>)?;
        submenu.append(&menu_item)?;
    }

    Ok(submenu)
}

fn get_current_mode(app: &AppHandle) -> RecordingMode {
    RecordingSettingsStore::get(app)
        .ok()
        .flatten()
        .and_then(|s| s.mode)
        .unwrap_or_default()
}

fn should_use_minimal_onboarding_tray_menu(app: &AppHandle) -> bool {
    if !app.webview_windows().contains_key("onboarding") {
        return false;
    }
    !crate::permissions::do_permissions_check(false).necessary_granted()
}

/// Camera and microphone lists mirrored from the app-wide `devices-updated`
/// event. The tray menu is built ahead of time, not when it opens, so it must
/// never enumerate devices itself: that work already runs on a poll elsewhere
/// and blocking the main thread with it would stall the menu.
#[derive(Default, Clone)]
struct TrayDeviceCache {
    cameras: Vec<cap_camera::CameraInfo>,
    microphones: Vec<String>,
}

#[derive(serde::Deserialize)]
struct DevicesUpdatedPayload {
    cameras: Vec<cap_camera::CameraInfo>,
    microphones: Vec<String>,
}

pub(crate) struct TrayMenuCache {
    cache: Arc<Mutex<PreviousItemsCache>>,
    devices: Arc<Mutex<TrayDeviceCache>>,
    is_recording: Arc<AtomicBool>,
}

pub(crate) fn refresh_tray_menu_for_app(app: &AppHandle) {
    let Some(state) = app.try_state::<TrayMenuCache>() else {
        return;
    };
    refresh_tray_menu(app, &state.cache);
}

fn camera_matches(info: &cap_camera::CameraInfo, selected: &DeviceOrModelID) -> bool {
    match selected {
        DeviceOrModelID::DeviceID(device_id) => info.device_id() == device_id,
        DeviceOrModelID::ModelID(model_id) => info.model_id() == Some(model_id),
    }
}

fn create_camera_submenu(
    app: &AppHandle,
    devices: &TrayDeviceCache,
    selected: Option<&DeviceOrModelID>,
) -> tauri::Result<Submenu<tauri::Wry>> {
    let submenu = Submenu::with_id(app, "camera_menu", "Camera", true)?;

    submenu.append(&CheckMenuItem::with_id(
        app,
        TrayItem::SelectCamera(String::new()),
        "No Camera",
        true,
        selected.is_none(),
        None::<&str>,
    )?)?;

    if devices.cameras.is_empty() {
        submenu.append(&MenuItem::with_id(
            app,
            "camera_menu_empty",
            "No cameras found",
            false,
            None::<&str>,
        )?)?;
        return Ok(submenu);
    }

    for camera in &devices.cameras {
        let checked = selected.is_some_and(|id| camera_matches(camera, id));
        submenu.append(&CheckMenuItem::with_id(
            app,
            TrayItem::SelectCamera(camera.device_id().to_string()),
            camera.display_name(),
            true,
            checked,
            None::<&str>,
        )?)?;
    }

    Ok(submenu)
}

fn create_microphone_submenu(
    app: &AppHandle,
    devices: &TrayDeviceCache,
    selected: Option<&str>,
) -> tauri::Result<Submenu<tauri::Wry>> {
    let submenu = Submenu::with_id(app, "microphone_menu", "Microphone", true)?;

    submenu.append(&CheckMenuItem::with_id(
        app,
        TrayItem::SelectMicrophone(String::new()),
        "No Microphone",
        true,
        selected.is_none(),
        None::<&str>,
    )?)?;

    if devices.microphones.is_empty() {
        submenu.append(&MenuItem::with_id(
            app,
            "microphone_menu_empty",
            "No microphones found",
            false,
            None::<&str>,
        )?)?;
        return Ok(submenu);
    }

    for name in &devices.microphones {
        submenu.append(&CheckMenuItem::with_id(
            app,
            TrayItem::SelectMicrophone(name.clone()),
            name,
            true,
            selected == Some(name.as_str()),
            None::<&str>,
        )?)?;
    }

    Ok(submenu)
}

fn build_tray_menu(app: &AppHandle, cache: &PreviousItemsCache) -> tauri::Result<Menu<tauri::Wry>> {
    if should_use_minimal_onboarding_tray_menu(app) {
        return Menu::with_items(
            app,
            &[
                &MenuItem::with_id(
                    app,
                    TrayItem::RequestPermissions,
                    "Request Permissions",
                    true,
                    None::<&str>,
                )?,
                &PredefinedMenuItem::separator(app)?,
                &MenuItem::with_id(
                    app,
                    "version",
                    format!("Shelf v{}", env!("CARGO_PKG_VERSION")),
                    false,
                    None::<&str>,
                )?,
                &MenuItem::with_id(app, TrayItem::Quit, "Quit Shelf", true, None::<&str>)?,
            ],
        );
    }

    let settings = RecordingSettingsStore::get(app)
        .ok()
        .flatten()
        .unwrap_or_default();
    let current_mode = settings.mode.unwrap_or_default();
    let is_screenshot_mode = current_mode == RecordingMode::Screenshot;

    let (devices, is_recording) = match app.try_state::<TrayMenuCache>() {
        Some(state) => (
            state.devices.lock().unwrap().clone(),
            state.is_recording.load(Ordering::Relaxed),
        ),
        None => (TrayDeviceCache::default(), false),
    };

    let previous_submenu = create_previous_submenu(app, cache)?;
    let camera_submenu = create_camera_submenu(app, &devices, settings.camera_id.as_ref())?;
    let microphone_submenu =
        create_microphone_submenu(app, &devices, settings.mic_name.as_deref())?;

    let menu = Menu::new(app)?;

    // 1. The one action the menu exists for.
    let primary_label = if is_recording {
        "Stop Recording"
    } else if is_screenshot_mode {
        "Take Screenshot"
    } else {
        "Start Recording"
    };
    menu.append(&MenuItem::with_id(
        app,
        TrayItem::StartStopRecording,
        primary_label,
        true,
        None::<&str>,
    )?)?;

    // 2. Quick actions that pick a target first.
    menu.append(&PredefinedMenuItem::separator(app)?)?;

    if is_screenshot_mode {
        menu.append(&MenuItem::with_id(
            app,
            TrayItem::RecordDisplay,
            "Screenshot Display",
            true,
            None::<&str>,
        )?)?;
        menu.append(&MenuItem::with_id(
            app,
            TrayItem::RecordWindow,
            "Screenshot Window",
            true,
            None::<&str>,
        )?)?;
        menu.append(&MenuItem::with_id(
            app,
            TrayItem::RecordArea,
            "Screenshot Area",
            true,
            None::<&str>,
        )?)?;
    } else {
        menu.append(&MenuItem::with_id(
            app,
            TrayItem::RecordDisplay,
            "Record Display",
            true,
            None::<&str>,
        )?)?;
        menu.append(&MenuItem::with_id(
            app,
            TrayItem::RecordWindow,
            "Record Window",
            true,
            None::<&str>,
        )?)?;
        menu.append(&MenuItem::with_id(
            app,
            TrayItem::RecordArea,
            "Record Area",
            true,
            None::<&str>,
        )?)?;
        menu.append(&MenuItem::with_id(
            app,
            TrayItem::TakeScreenshot,
            "Take a Screenshot",
            true,
            None::<&str>,
        )?)?;
    }

    // 3. Mode, visible at a glance instead of hidden in a submenu.
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        "mode_header",
        "Mode",
        false,
        None::<&str>,
    )?)?;

    for (tray_item, mode, label) in [
        (TrayItem::ModeStudio, RecordingMode::Studio, "Studio"),
        (TrayItem::ModeInstant, RecordingMode::Instant, "Instant"),
        (
            TrayItem::ModeScreenshot,
            RecordingMode::Screenshot,
            "Screenshot",
        ),
    ] {
        menu.append(&CheckMenuItem::with_id(
            app,
            tray_item,
            label,
            true,
            current_mode == mode,
            None::<&str>,
        )?)?;
    }

    // 4. Devices.
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&camera_submenu)?;
    menu.append(&microphone_submenu)?;
    menu.append(&CheckMenuItem::with_id(
        app,
        TrayItem::ToggleSystemAudio,
        "System Audio",
        crate::platform::is_system_audio_capture_supported(),
        settings.system_audio,
        None::<&str>,
    )?)?;

    // 5. Everything already captured.
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&previous_submenu)?;
    menu.append(&MenuItem::with_id(
        app,
        TrayItem::ViewAllRecordings,
        "View all recordings",
        true,
        None::<&str>,
    )?)?;
    menu.append(&MenuItem::with_id(
        app,
        TrayItem::ViewAllScreenshots,
        "View all screenshots",
        true,
        None::<&str>,
    )?)?;

    // 6. Rarely needed now that the menu carries the controls.
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        TrayItem::ImportVideo,
        "Import Media...",
        true,
        None::<&str>,
    )?)?;
    menu.append(&MenuItem::with_id(
        app,
        TrayItem::OpenCap,
        "Open Main Window",
        true,
        None::<&str>,
    )?)?;
    menu.append(&MenuItem::with_id(
        app,
        TrayItem::OpenSettings,
        "Settings",
        true,
        None::<&str>,
    )?)?;

    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        "version",
        format!("Shelf v{}", env!("CARGO_PKG_VERSION")),
        false,
        None::<&str>,
    )?)?;
    menu.append(&MenuItem::with_id(
        app,
        TrayItem::Quit,
        "Quit Shelf",
        true,
        None::<&str>,
    )?)?;

    Ok(menu)
}

fn add_new_item_to_cache(cache: &Arc<Mutex<PreviousItemsCache>>, app: &AppHandle, path: PathBuf) {
    let screenshots_dir = screenshots_path(app);

    let Some(new_item) = load_single_item(&path, &screenshots_dir, true) else {
        return;
    };

    let mut cache_guard = cache.lock().unwrap();

    cache_guard.items.retain(|item| item.path != path);

    cache_guard.items.insert(0, new_item);

    cache_guard.items.truncate(MAX_PREVIOUS_ITEMS);
}

fn refresh_tray_menu(app: &AppHandle, cache: &Arc<Mutex<PreviousItemsCache>>) {
    let app_clone = app.clone();
    let cache_clone = cache.clone();

    let _ = app.run_on_main_thread(move || {
        let Some(tray) = app_clone.tray_by_id("tray") else {
            return;
        };

        let cache_guard = cache_clone.lock().unwrap();
        if let Ok(menu) = build_tray_menu(&app_clone, &cache_guard) {
            let _ = tray.set_menu(Some(menu));
        }
    });
}

fn handle_previous_item_click(app: &AppHandle, path_str: &str) {
    let path = PathBuf::from(path_str);

    let screenshots_dir = screenshots_path(app);
    let is_screenshot = path.extension().and_then(|s| s.to_str()) == Some("cap")
        && path.parent().map(|p| p == screenshots_dir).unwrap_or(false);

    if is_screenshot {
        let app = app.clone();
        let screenshot_path = path;
        tokio::spawn(async move {
            let _ = ShowCapWindow::ScreenshotEditor {
                path: screenshot_path,
            }
            .show(&app)
            .await;
        });
        return;
    }

    let meta = match RecordingMeta::load_for_project(&path) {
        Ok(m) => m,
        Err(e) => {
            tracing::error!("Failed to load recording meta for previous item: {e}");
            return;
        }
    };

    match &meta.inner {
        RecordingMetaInner::Studio(_) => {
            let app = app.clone();
            let project_path = path.clone();
            tokio::spawn(async move {
                let _ = ShowCapWindow::Editor { project_path }.show(&app).await;
            });
        }
        RecordingMetaInner::Instant(_) => {
            if let Some(sharing) = &meta.sharing {
                let _ = app.opener().open_url(&sharing.link, None::<String>);
            } else {
                let mp4_path = path.join("content/output.mp4");
                if mp4_path.exists() {
                    let _ = app
                        .opener()
                        .open_path(mp4_path.to_str().unwrap_or_default(), None::<String>);
                }
            }
        }
    }
}

pub fn get_tray_icon() -> &'static [u8] {
    include_bytes!("../icons/tray-default-icon.png")
}

pub fn get_mode_icon(mode: RecordingMode) -> &'static [u8] {
    if cfg!(target_os = "windows") {
        return get_tray_icon();
    }
    match mode {
        RecordingMode::Studio => include_bytes!("../icons/tray-default-icon-studio.png"),
        RecordingMode::Instant => include_bytes!("../icons/tray-default-icon-instant.png"),
        RecordingMode::Screenshot => include_bytes!("../icons/tray-default-icon-screenshot.png"),
    }
}

#[cfg(target_os = "linux")]
fn linux_tray_icon_for_mode(mode: RecordingMode) -> LinuxTrayIcon {
    match mode {
        RecordingMode::Studio => LinuxTrayIcon::Studio,
        RecordingMode::Instant => LinuxTrayIcon::Instant,
        RecordingMode::Screenshot => LinuxTrayIcon::Screenshot,
    }
}

#[cfg(target_os = "linux")]
fn write_linux_tray_symbolic_icon(icon: LinuxTrayIcon) -> std::io::Result<PathBuf> {
    let dir = dirs::runtime_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("cap-tray-icons");
    std::fs::create_dir_all(&dir)?;

    let path = dir.join(format!("{}.svg", icon.name()));
    let svg = icon.svg().as_bytes();
    if std::fs::read(&path).ok().as_deref() != Some(svg) {
        std::fs::write(path, svg)?;
    }

    Ok(dir)
}

#[cfg(target_os = "linux")]
fn set_linux_tray_icon(tray: &TrayIcon<tauri::Wry>, icon: LinuxTrayIcon) -> tauri::Result<()> {
    let icon_dir = write_linux_tray_symbolic_icon(icon).map_err(tauri::Error::Io)?;
    let icon_name = icon.name().to_string();

    tray.with_inner_tray_icon(move |inner| unsafe {
        let indicator = inner.app_indicator() as *mut libappindicator::AppIndicator;
        if let Some(indicator) = indicator.as_mut() {
            indicator.set_icon_theme_path(&icon_dir.to_string_lossy());
            indicator.set_icon_full(&icon_name, "Shelf tray icon");
        }
    })?;

    Ok(())
}

#[cfg(target_os = "linux")]
fn set_tray_icon_for_mode(tray: &TrayIcon<tauri::Wry>, mode: RecordingMode) -> tauri::Result<()> {
    set_linux_tray_icon(tray, linux_tray_icon_for_mode(mode))
}

#[cfg(not(target_os = "linux"))]
fn set_tray_icon_for_mode(tray: &TrayIcon<tauri::Wry>, mode: RecordingMode) -> tauri::Result<()> {
    let icon = Image::from_bytes(get_mode_icon(mode))?;
    tray.set_icon(Some(icon))?;
    tray.set_icon_as_template(true)?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn set_tray_stop_icon(tray: &TrayIcon<tauri::Wry>) -> tauri::Result<()> {
    set_linux_tray_icon(tray, LinuxTrayIcon::Stop)
}

#[cfg(not(target_os = "linux"))]
fn set_tray_stop_icon(tray: &TrayIcon<tauri::Wry>) -> tauri::Result<()> {
    let icon = Image::from_bytes(include_bytes!("../icons/tray-stop-icon.png"))?;
    tray.set_icon(Some(icon))?;
    tray.set_icon_as_template(true)?;
    Ok(())
}

pub fn update_tray_icon_for_mode(app: &AppHandle, mode: RecordingMode) {
    if cfg!(target_os = "windows") {
        return;
    }

    let Some(tray) = app.tray_by_id("tray") else {
        return;
    };

    if let Err(error) = set_tray_icon_for_mode(&tray, mode) {
        tracing::warn!("Failed to update tray icon: {error}");
    }
}

fn handle_mode_selection(
    app: &AppHandle,
    mode: RecordingMode,
    cache: &Arc<Mutex<PreviousItemsCache>>,
) {
    if let Err(e) = RecordingSettingsStore::set_mode(app, mode) {
        tracing::error!("Failed to set recording mode: {e}");
        return;
    }

    update_tray_icon_for_mode(app, mode);
    refresh_tray_menu(app, cache);
}

fn take_screenshot_of_cursor_display(app: &AppHandle) {
    let app = app.clone();
    tokio::spawn(async move {
        use cap_recording::screen_capture::ScreenCaptureTarget;
        use scap_targets::Display;

        let display = Display::get_containing_cursor().unwrap_or_else(Display::primary);
        let target = ScreenCaptureTarget::Display { id: display.id() };

        match recording::take_screenshot(app.clone(), target.clone()).await {
            Ok(path) => {
                if crate::automation::should_open_screenshot_editor(&app, &target) {
                    let _ = ShowCapWindow::ScreenshotEditor { path }.show(&app).await;
                }
            }
            Err(e) => {
                tracing::error!("Failed to take screenshot: {e}");
            }
        }
    });
}

/// The menu's primary item. Deliberately goes through the same
/// `RequestStartRecording` event the hotkeys and the UI use, so the saved
/// target, camera, microphone and system audio are applied in one place.
fn handle_start_stop(app: &AppHandle, is_recording: &Arc<AtomicBool>) {
    if is_recording.load(Ordering::Relaxed) {
        let app = app.clone();
        tokio::spawn(async move {
            if let Err(e) = recording::stop_recording(app.clone(), app.state()).await {
                tracing::error!("Failed to stop recording from tray: {e}");
            }
        });
        return;
    }

    let mode = get_current_mode(app);
    if mode == RecordingMode::Screenshot {
        take_screenshot_of_cursor_display(app);
        return;
    }

    let _ = crate::RequestStartRecording { mode }.emit(app);
}

fn handle_camera_selection(
    app: &AppHandle,
    device_id: &str,
    cache: &Arc<Mutex<PreviousItemsCache>>,
) {
    let id = if device_id.is_empty() {
        None
    } else {
        let devices = app.try_state::<TrayMenuCache>().map(|state| {
            let devices = state.devices.lock().unwrap();
            devices.cameras.clone()
        });

        let Some(info) = devices
            .unwrap_or_default()
            .into_iter()
            .find(|camera| camera.device_id() == device_id)
        else {
            tracing::warn!("Tray selected a camera that is no longer listed: {device_id}");
            return;
        };

        Some(DeviceOrModelID::from_info(&info))
    };

    if let Err(e) = RecordingSettingsStore::set_camera_id(app, id.clone()) {
        tracing::error!("Failed to persist camera selection: {e}");
        return;
    }

    refresh_tray_menu(app, cache);

    let app = app.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::set_camera_input(app.clone(), app.state(), id, None).await {
            tracing::error!("Failed to apply camera selection from tray: {e}");
        }
    });
}

fn handle_microphone_selection(
    app: &AppHandle,
    name: &str,
    cache: &Arc<Mutex<PreviousItemsCache>>,
) {
    let label = (!name.is_empty()).then(|| name.to_string());

    if let Err(e) = RecordingSettingsStore::set_mic_name(app, label.clone()) {
        tracing::error!("Failed to persist microphone selection: {e}");
        return;
    }

    refresh_tray_menu(app, cache);

    let app = app.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::set_mic_input(app.state(), label).await {
            tracing::error!("Failed to apply microphone selection from tray: {e}");
        }
    });
}

fn handle_system_audio_toggle(app: &AppHandle, cache: &Arc<Mutex<PreviousItemsCache>>) {
    let enabled = RecordingSettingsStore::get(app)
        .ok()
        .flatten()
        .map(|settings| settings.system_audio)
        .unwrap_or_default();

    if let Err(e) = RecordingSettingsStore::set_system_audio(app, !enabled) {
        tracing::error!("Failed to persist system audio setting: {e}");
        return;
    }

    refresh_tray_menu(app, cache);
}

pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let items = load_all_previous_items(app, false);
    let cache = Arc::new(Mutex::new(PreviousItemsCache { items }));
    let devices = Arc::new(Mutex::new(TrayDeviceCache::default()));
    let is_recording = Arc::new(AtomicBool::new(false));

    app.manage(TrayMenuCache {
        cache: cache.clone(),
        devices: devices.clone(),
        is_recording: is_recording.clone(),
    });

    let menu = {
        let cache_guard = cache.lock().unwrap();
        build_tray_menu(app, &cache_guard)?
    };
    let app = app.clone();

    let current_mode = get_current_mode(&app);
    let initial_icon = Image::from_bytes(get_mode_icon(current_mode))?;

    let _ = TrayIconBuilder::with_id("tray")
        .icon(initial_icon)
        .icon_as_template(cfg!(target_os = "macos"))
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event({
            let app_handle = app.clone();
            let cache = cache.clone();
            let is_recording = is_recording.clone();
            move |app: &AppHandle, event| match TrayItem::try_from(event.id) {
                Ok(TrayItem::StartStopRecording) => {
                    handle_start_stop(app, &is_recording);
                }
                Ok(TrayItem::OpenCap) => {
                    let app = app.clone();
                    tokio::spawn(async move {
                        let _ = ShowCapWindow::Main {
                            init_target_mode: None,
                        }
                        .show(&app)
                        .await;
                    });
                }
                Ok(TrayItem::RecordDisplay) => {
                    let app = app.clone();
                    tokio::spawn(async move {
                        crate::open_target_picker(&app, RecordingTargetMode::Display).await;
                    });
                }
                Ok(TrayItem::RecordWindow) => {
                    let app = app.clone();
                    tokio::spawn(async move {
                        crate::open_target_picker(&app, RecordingTargetMode::Window).await;
                    });
                }
                Ok(TrayItem::RecordArea) => {
                    let app = app.clone();
                    tokio::spawn(async move {
                        crate::open_target_picker(&app, RecordingTargetMode::Area).await;
                    });
                }
                Ok(TrayItem::TakeScreenshot) => {
                    take_screenshot_of_cursor_display(app);
                }
                Ok(TrayItem::ImportVideo) => {
                    let app = app.clone();
                    tokio::spawn(async move {
                        let file_path = app
                            .dialog()
                            .file()
                            .add_filter(
                                "Media Files",
                                &[
                                    "mp4", "mov", "avi", "mkv", "webm", "wmv", "m4v", "flv", "png",
                                    "jpg", "jpeg", "webp", "gif", "bmp", "tif", "tiff",
                                ],
                            )
                            .add_filter(
                                "Video Files",
                                &["mp4", "mov", "avi", "mkv", "webm", "wmv", "m4v", "flv"],
                            )
                            .add_filter(
                                "Image Files",
                                &["png", "jpg", "jpeg", "webp", "gif", "bmp", "tif", "tiff"],
                            )
                            .blocking_pick_file();

                        if let Some(file_path) = file_path {
                            let path = match file_path.into_path() {
                                Ok(p) => p,
                                Err(e) => {
                                    tracing::error!("Invalid file path: {e}");
                                    return;
                                }
                            };

                            if crate::import::is_supported_video_import_path(&path) {
                                match crate::import::start_video_import(app.clone(), path).await {
                                    Ok(project_path) => {
                                        let _ =
                                            ShowCapWindow::Editor { project_path }.show(&app).await;
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to import video: {e}");
                                        app.dialog()
                                            .message(format!("Failed to import video: {e}"))
                                            .title("Import Error")
                                            .kind(tauri_plugin_dialog::MessageDialogKind::Error)
                                            .blocking_show();
                                    }
                                }
                            } else if crate::import::is_supported_image_import_path(&path) {
                                match crate::import::start_image_import(app.clone(), path).await {
                                    Ok(path) => {
                                        let _ = ShowCapWindow::ScreenshotEditor { path }
                                            .show(&app)
                                            .await;
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to import image: {e}");
                                        app.dialog()
                                            .message(format!("Failed to import image: {e}"))
                                            .title("Import Error")
                                            .kind(tauri_plugin_dialog::MessageDialogKind::Error)
                                            .blocking_show();
                                    }
                                }
                            } else {
                                tracing::error!("Unsupported media import path: {:?}", path);
                                app.dialog()
                                    .message("Unsupported media file.")
                                    .title("Import Error")
                                    .kind(tauri_plugin_dialog::MessageDialogKind::Error)
                                    .blocking_show();
                            }
                        }
                    });
                }
                Ok(TrayItem::ViewAllRecordings) => {
                    let _ = RequestOpenSettings {
                        page: "recordings".to_string(),
                    }
                    .emit(&app_handle);
                }
                Ok(TrayItem::ViewAllScreenshots) => {
                    let _ = RequestOpenSettings {
                        page: "screenshots".to_string(),
                    }
                    .emit(&app_handle);
                }
                Ok(TrayItem::OpenSettings) => {
                    let app = app.clone();
                    tokio::spawn(
                        async move { ShowCapWindow::Settings { page: None }.show(&app).await },
                    );
                }
                Ok(TrayItem::Quit) => {
                    let app = app.clone();
                    tokio::spawn(async move {
                        crate::request_app_exit(app).await;
                    });
                }
                Ok(TrayItem::PreviousItem(path)) => {
                    handle_previous_item_click(app, &path);
                }
                Ok(TrayItem::ModeStudio) => {
                    handle_mode_selection(app, RecordingMode::Studio, &cache);
                }
                Ok(TrayItem::ModeInstant) => {
                    handle_mode_selection(app, RecordingMode::Instant, &cache);
                }
                Ok(TrayItem::ModeScreenshot) => {
                    handle_mode_selection(app, RecordingMode::Screenshot, &cache);
                }
                Ok(TrayItem::SelectCamera(device_id)) => {
                    handle_camera_selection(app, &device_id, &cache);
                }
                Ok(TrayItem::SelectMicrophone(name)) => {
                    handle_microphone_selection(app, &name, &cache);
                }
                Ok(TrayItem::ToggleSystemAudio) => {
                    handle_system_audio_toggle(app, &cache);
                }
                Ok(TrayItem::RequestPermissions) => {
                    let app = app.clone();
                    tokio::spawn(async move {
                        let _ = ShowCapWindow::Onboarding.show(&app).await;
                    });
                }
                _ => {}
            }
        })
        .on_tray_icon_event(move |tray, event| {
            // A click always opens the menu, including while recording. Stopping
            // lives in the menu now, and stopping on click would make the menu
            // unusable during a recording.
            if let tauri::tray::TrayIconEvent::Click { .. } = event {
                let _ = tray.set_visible(true);
            }
        })
        .build(&app);

    #[cfg(target_os = "linux")]
    if let Some(tray) = app.tray_by_id("tray")
        && let Err(error) = set_tray_icon_for_mode(&tray, current_mode)
    {
        tracing::warn!("Failed to initialize Linux tray icon: {error}");
    }

    {
        let app_clone = app.clone();
        let cache_clone = cache.clone();
        std::thread::spawn(move || {
            let screenshots_dir = screenshots_path(&app_clone);
            let items_needing_thumbnails: Vec<PathBuf> = {
                let cache_guard = cache_clone.lock().unwrap();
                cache_guard
                    .items
                    .iter()
                    .filter(|item| item.thumbnail.is_none())
                    .map(|item| item.path.clone())
                    .collect()
            };

            if items_needing_thumbnails.is_empty() {
                return;
            }

            for path in items_needing_thumbnails {
                if let Some(updated_item) = load_single_item(&path, &screenshots_dir, true) {
                    let mut cache_guard = cache_clone.lock().unwrap();
                    if let Some(existing) = cache_guard.items.iter_mut().find(|i| i.path == path) {
                        existing.thumbnail = updated_item.thumbnail;
                        existing.thumbnail_width = updated_item.thumbnail_width;
                        existing.thumbnail_height = updated_item.thumbnail_height;
                    }
                }
            }

            let app_for_refresh = app_clone.clone();
            let cache_for_refresh = cache_clone.clone();
            let _ = app_clone.run_on_main_thread(move || {
                if let Some(tray) = app_for_refresh.tray_by_id("tray") {
                    let cache_guard = cache_for_refresh.lock().unwrap();
                    if let Ok(menu) = build_tray_menu(&app_for_refresh, &cache_guard) {
                        let _ = tray.set_menu(Some(menu));
                    }
                }
            });
        });
    }

    RecordingStarted::listen_any(&app, {
        let app = app.clone();
        let is_recording = is_recording.clone();
        let cache = cache.clone();
        move |_| {
            is_recording.store(true, Ordering::Relaxed);
            refresh_tray_menu(&app, &cache);

            if cfg!(target_os = "windows") {
                return;
            }

            let Some(tray) = app.tray_by_id("tray") else {
                return;
            };

            if let Err(error) = set_tray_stop_icon(&tray) {
                tracing::warn!("Failed to update tray icon for recording start: {error}");
            }
        }
    });

    RecordingStopped::listen_any(&app, {
        let app_handle = app.clone();
        let is_recording = is_recording.clone();
        let cache = cache.clone();
        move |_| {
            is_recording.store(false, Ordering::Relaxed);
            refresh_tray_menu(&app_handle, &cache);

            if cfg!(target_os = "windows") {
                return;
            }

            let Some(tray) = app_handle.tray_by_id("tray") else {
                return;
            };

            let current_mode = get_current_mode(&app_handle);
            if let Err(error) = set_tray_icon_for_mode(&tray, current_mode) {
                tracing::warn!("Failed to update tray icon for recording stop: {error}");
            }
        }
    });

    NewStudioRecordingAdded::listen_any(&app, {
        let app_handle = app.clone();
        let cache_clone = cache.clone();
        move |event| {
            add_new_item_to_cache(&cache_clone, &app_handle, event.payload.path.clone());
            refresh_tray_menu(&app_handle, &cache_clone);
        }
    });

    NewScreenshotAdded::listen_any(&app, {
        let app_handle = app.clone();
        let cache_clone = cache.clone();
        move |event| {
            let path = if event.payload.path.extension().and_then(|s| s.to_str()) == Some("png") {
                event.payload.path.parent().map(|p| p.to_path_buf())
            } else {
                Some(event.payload.path.clone())
            };

            if let Some(path) = path {
                add_new_item_to_cache(&cache_clone, &app_handle, path);
                refresh_tray_menu(&app_handle, &cache_clone);
            }
        }
    });

    // `DevicesUpdated` carries no Deserialize impl, so the typed listener is out
    // of reach; the raw listener uses the same event name and payload shape.
    app.listen_any(<crate::DevicesUpdated as tauri_specta::Event>::NAME, {
        let app_handle = app.clone();
        let cache = cache.clone();
        let devices = devices.clone();
        move |event| {
            let payload = match serde_json::from_str::<DevicesUpdatedPayload>(event.payload()) {
                Ok(payload) => payload,
                Err(e) => {
                    tracing::warn!("Failed to read devices-updated payload for tray: {e}");
                    return;
                }
            };

            {
                let mut guard = devices.lock().unwrap();
                let unchanged = guard.microphones == payload.microphones
                    && guard.cameras.len() == payload.cameras.len()
                    && guard
                        .cameras
                        .iter()
                        .zip(payload.cameras.iter())
                        .all(|(a, b)| a.device_id() == b.device_id());
                if unchanged {
                    return;
                }
                guard.cameras = payload.cameras;
                guard.microphones = payload.microphones;
            }

            refresh_tray_menu(&app_handle, &cache);
        }
    });

    crate::recordings_locations::RecordingsMigrationProgress::listen_any(&app, {
        let app_handle = app.clone();
        let cache_clone = cache.clone();
        move |event| {
            // Rebuild once when a storage-folder migration finishes: moved
            // projects changed paths, so cached entries are stale.
            if event.payload.done == event.payload.total {
                let items = load_all_previous_items(&app_handle, false);
                cache_clone.lock().unwrap().items = items;
                refresh_tray_menu(&app_handle, &cache_clone);
            }
        }
    });

    Ok(())
}
