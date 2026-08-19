use crate::{
    RecordingStarted, RecordingStopped, RequestOpenSettings, recording,
    recording_settings::{RecordingSettingsStore, RecordingTargetMode},
    windows::ShowCapWindow,
};
use cap_recording::{RecordingMode, feeds::camera::DeviceOrModelID};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::menu::{CheckMenuItem, IconMenuItem, MenuId, PredefinedMenuItem, Submenu};
use tauri::{
    AppHandle,
    image::Image,
    menu::{Menu, MenuItem},
    tray::{TrayIcon, TrayIconBuilder},
};
use tauri::{Listener, Manager};
use tauri_specta::Event;

const CAMERA_ITEM_PREFIX: &str = "camera_item_";
const MIC_ITEM_PREFIX: &str = "mic_item_";

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
    StartStopRecording,
    RecordDisplay,
    RecordWindow,
    RecordArea,
    RecordCameraOnly,
    ScreenshotDisplay,
    ScreenshotWindow,
    ScreenshotArea,
    OpenTeleprompter,
    ViewAllRecordings,
    ViewAllScreenshots,
    OpenSettings,
    Quit,
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
            TrayItem::StartStopRecording => "start_stop_recording",
            TrayItem::RecordDisplay => "record_display",
            TrayItem::RecordWindow => "record_window",
            TrayItem::RecordArea => "record_area",
            TrayItem::RecordCameraOnly => "record_camera_only",
            TrayItem::ScreenshotDisplay => "screenshot_display",
            TrayItem::ScreenshotWindow => "screenshot_window",
            TrayItem::ScreenshotArea => "screenshot_area",
            TrayItem::OpenTeleprompter => "open_teleprompter",
            TrayItem::ViewAllRecordings => "view_all_recordings",
            TrayItem::ViewAllScreenshots => "view_all_screenshots",
            TrayItem::OpenSettings => "open_settings",
            TrayItem::Quit => "quit",
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

        if let Some(id) = id_str.strip_prefix(CAMERA_ITEM_PREFIX) {
            return Ok(TrayItem::SelectCamera(id.to_string()));
        }

        if let Some(name) = id_str.strip_prefix(MIC_ITEM_PREFIX) {
            return Ok(TrayItem::SelectMicrophone(name.to_string()));
        }

        match id_str {
            "start_stop_recording" => Ok(TrayItem::StartStopRecording),
            "record_display" => Ok(TrayItem::RecordDisplay),
            "record_window" => Ok(TrayItem::RecordWindow),
            "record_area" => Ok(TrayItem::RecordArea),
            "record_camera_only" => Ok(TrayItem::RecordCameraOnly),
            "screenshot_display" => Ok(TrayItem::ScreenshotDisplay),
            "screenshot_window" => Ok(TrayItem::ScreenshotWindow),
            "screenshot_area" => Ok(TrayItem::ScreenshotArea),
            "open_teleprompter" => Ok(TrayItem::OpenTeleprompter),
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
    devices: Arc<Mutex<TrayDeviceCache>>,
    is_recording: Arc<AtomicBool>,
}

pub(crate) fn refresh_tray_menu_for_app(app: &AppHandle) {
    refresh_tray_menu(app);
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

/// Both groups are always in the menu now (see CHANGELOG), tucked into
/// submenus so the top level stays short. Icons are reused from the
/// existing mode icons rather than inventing new artwork.
fn create_screenshot_submenu(app: &AppHandle) -> tauri::Result<Submenu<tauri::Wry>> {
    let icon = Image::from_bytes(include_bytes!("../icons/tray-default-icon-screenshot.png"))?;
    let submenu = Submenu::with_id_and_icon(
        app,
        "screenshot_menu",
        "Screenshot",
        true,
        Some(icon.clone()),
    )?;

    for (tray_item, label) in [
        (TrayItem::ScreenshotDisplay, "Display"),
        (TrayItem::ScreenshotWindow, "Window"),
        (TrayItem::ScreenshotArea, "Area"),
    ] {
        submenu.append(&IconMenuItem::with_id(
            app,
            tray_item,
            label,
            true,
            Some(icon.clone()),
            None::<&str>,
        )?)?;
    }

    Ok(submenu)
}

fn create_record_screen_submenu(app: &AppHandle) -> tauri::Result<Submenu<tauri::Wry>> {
    let icon = Image::from_bytes(include_bytes!("../icons/tray-default-icon.png"))?;
    let submenu = Submenu::with_id_and_icon(
        app,
        "record_screen_menu",
        "Record Screen",
        true,
        Some(icon.clone()),
    )?;

    for (tray_item, label) in [
        (TrayItem::RecordDisplay, "Display"),
        (TrayItem::RecordWindow, "Window"),
        (TrayItem::RecordArea, "Area"),
        (TrayItem::RecordCameraOnly, "Camera Only"),
    ] {
        submenu.append(&IconMenuItem::with_id(
            app,
            tray_item,
            label,
            true,
            Some(icon.clone()),
            None::<&str>,
        )?)?;
    }

    Ok(submenu)
}

fn build_tray_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
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

    menu.append(&create_screenshot_submenu(app)?)?;
    menu.append(&create_record_screen_submenu(app)?)?;

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
        TrayItem::OpenTeleprompter,
        "Teleprompter",
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

fn refresh_tray_menu(app: &AppHandle) {
    let app_clone = app.clone();

    let _ = app.run_on_main_thread(move || {
        let Some(tray) = app_clone.tray_by_id("tray") else {
            return;
        };

        if let Ok(menu) = build_tray_menu(&app_clone) {
            let _ = tray.set_menu(Some(menu));
        }
    });
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

fn handle_mode_selection(app: &AppHandle, mode: RecordingMode) {
    if let Err(e) = RecordingSettingsStore::set_mode(app, mode) {
        tracing::error!("Failed to set recording mode: {e}");
        return;
    }

    update_tray_icon_for_mode(app, mode);
    refresh_tray_menu(app);
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

enum PickerKind {
    Recording,
    Screenshot,
}

/// The menu offers both halves at once, so a screenshot entry has to put the app
/// into screenshot mode and a record entry has to take it out again. Instant is
/// kept when it was already selected; only screenshot mode falls back to Studio.
fn switch_to_recording_mode(app: &AppHandle) {
    if get_current_mode(app) != RecordingMode::Screenshot {
        return;
    }
    handle_mode_selection(app, RecordingMode::Studio);
}

fn open_picker_in(app: &AppHandle, target_mode: RecordingTargetMode, kind: PickerKind) {
    match kind {
        PickerKind::Screenshot => {
            if get_current_mode(app) != RecordingMode::Screenshot {
                handle_mode_selection(app, RecordingMode::Screenshot);
            }
        }
        PickerKind::Recording => switch_to_recording_mode(app),
    }

    let app = app.clone();
    tokio::spawn(async move {
        crate::open_target_picker(&app, target_mode).await;
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

fn handle_camera_selection(app: &AppHandle, device_id: &str) {
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

    refresh_tray_menu(app);

    let app = app.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::set_camera_input(app.clone(), app.state(), id, None).await {
            tracing::error!("Failed to apply camera selection from tray: {e}");
        }
    });
}

fn handle_microphone_selection(app: &AppHandle, name: &str) {
    let label = (!name.is_empty()).then(|| name.to_string());

    if let Err(e) = RecordingSettingsStore::set_mic_name(app, label.clone()) {
        tracing::error!("Failed to persist microphone selection: {e}");
        return;
    }

    refresh_tray_menu(app);

    let app = app.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::set_mic_input(app.state(), label).await {
            tracing::error!("Failed to apply microphone selection from tray: {e}");
        }
    });
}

fn handle_system_audio_toggle(app: &AppHandle) {
    let enabled = RecordingSettingsStore::get(app)
        .ok()
        .flatten()
        .map(|settings| settings.system_audio)
        .unwrap_or_default();

    if let Err(e) = RecordingSettingsStore::set_system_audio(app, !enabled) {
        tracing::error!("Failed to persist system audio setting: {e}");
        return;
    }

    refresh_tray_menu(app);
}

pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let devices = Arc::new(Mutex::new(TrayDeviceCache::default()));
    let is_recording = Arc::new(AtomicBool::new(false));

    app.manage(TrayMenuCache {
        devices: devices.clone(),
        is_recording: is_recording.clone(),
    });

    let menu = build_tray_menu(app)?;
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
            let is_recording = is_recording.clone();
            move |app: &AppHandle, event| match TrayItem::try_from(event.id) {
                Ok(TrayItem::StartStopRecording) => {
                    handle_start_stop(app, &is_recording);
                }
                Ok(TrayItem::OpenTeleprompter) => {
                    let app = app.clone();
                    tokio::spawn(async move {
                        if let Err(e) = ShowCapWindow::Teleprompter.show(&app).await {
                            tracing::error!("Failed to open teleprompter from tray: {e}");
                        }
                    });
                }
                Ok(TrayItem::RecordCameraOnly) => {
                    switch_to_recording_mode(app);

                    // Camera-only has no target to pick, so it goes straight to
                    // recording with the saved camera.
                    let app = app.clone();
                    tokio::spawn(async move {
                        if let Err(e) = RecordingSettingsStore::set_target(
                            &app,
                            Some(cap_recording::screen_capture::ScreenCaptureTarget::CameraOnly),
                        ) {
                            tracing::error!("Failed to select camera-only target: {e}");
                            return;
                        }

                        let mode = get_current_mode(&app);
                        let _ = crate::RequestStartRecording { mode }.emit(&app);
                    });
                }
                Ok(TrayItem::RecordDisplay) => {
                    open_picker_in(app, RecordingTargetMode::Display, PickerKind::Recording);
                }
                Ok(TrayItem::RecordWindow) => {
                    open_picker_in(app, RecordingTargetMode::Window, PickerKind::Recording);
                }
                Ok(TrayItem::RecordArea) => {
                    open_picker_in(app, RecordingTargetMode::Area, PickerKind::Recording);
                }
                Ok(TrayItem::ScreenshotDisplay) => {
                    open_picker_in(app, RecordingTargetMode::Display, PickerKind::Screenshot);
                }
                Ok(TrayItem::ScreenshotWindow) => {
                    open_picker_in(app, RecordingTargetMode::Window, PickerKind::Screenshot);
                }
                Ok(TrayItem::ScreenshotArea) => {
                    open_picker_in(app, RecordingTargetMode::Area, PickerKind::Screenshot);
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
                Ok(TrayItem::ModeStudio) => {
                    handle_mode_selection(app, RecordingMode::Studio);
                }
                Ok(TrayItem::ModeInstant) => {
                    handle_mode_selection(app, RecordingMode::Instant);
                }
                Ok(TrayItem::ModeScreenshot) => {
                    handle_mode_selection(app, RecordingMode::Screenshot);
                }
                Ok(TrayItem::SelectCamera(device_id)) => {
                    handle_camera_selection(app, &device_id);
                }
                Ok(TrayItem::SelectMicrophone(name)) => {
                    handle_microphone_selection(app, &name);
                }
                Ok(TrayItem::ToggleSystemAudio) => {
                    handle_system_audio_toggle(app);
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

    RecordingStarted::listen_any(&app, {
        let app = app.clone();
        let is_recording = is_recording.clone();
        move |_| {
            is_recording.store(true, Ordering::Relaxed);
            refresh_tray_menu(&app);

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
        move |_| {
            is_recording.store(false, Ordering::Relaxed);
            refresh_tray_menu(&app_handle);

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

    // `DevicesUpdated` carries no Deserialize impl, so the typed listener is out
    // of reach; the raw listener uses the same event name and payload shape.
    app.listen_any(<crate::DevicesUpdated as tauri_specta::Event>::NAME, {
        let app_handle = app.clone();
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

            refresh_tray_menu(&app_handle);
        }
    });

    Ok(())
}
