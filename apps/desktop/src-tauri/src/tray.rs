use crate::{
    RecordingStarted, RecordingStopped, RequestOpenSettings,
    hotkeys::{Hotkey, HotkeyAction, HotkeysStore},
    recording,
    recording_settings::{RecordingSettingsStore, RecordingTargetMode},
    tray_icons::{self, Tint, symbol},
    windows::ShowCapWindow,
};
use cap_recording::{RecordingMode, sources::screen_capture::ScreenCaptureTarget};

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Manager;
use tauri::menu::{CheckMenuItem, IconMenuItem, IsMenuItem, MenuId, PredefinedMenuItem, Submenu};
use tauri::{
    AppHandle,
    image::Image,
    menu::{Menu, MenuItem},
    tray::{TrayIcon, TrayIconBuilder},
};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tauri_specta::Event;

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
            Self::Default => "de.shelf.desktop-tray-default-symbolic",
            Self::Instant => "de.shelf.desktop-tray-instant-symbolic",
            Self::Screenshot => "de.shelf.desktop-tray-screenshot-symbolic",
            Self::Studio => "de.shelf.desktop-tray-studio-symbolic",
            Self::Stop => "de.shelf.desktop-tray-stop-symbolic",
        }
    }

    fn svg(self) -> &'static str {
        match self {
            Self::Default => {
                include_str!("../icons/linux/de.shelf.desktop-tray-default-symbolic.svg")
            }
            Self::Instant => {
                include_str!("../icons/linux/de.shelf.desktop-tray-instant-symbolic.svg")
            }
            Self::Screenshot => {
                include_str!("../icons/linux/de.shelf.desktop-tray-screenshot-symbolic.svg")
            }
            Self::Studio => {
                include_str!("../icons/linux/de.shelf.desktop-tray-studio-symbolic.svg")
            }
            Self::Stop => include_str!("../icons/linux/de.shelf.desktop-tray-stop-symbolic.svg"),
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
        }
        .into()
    }
}

impl TryFrom<MenuId> for TrayItem {
    type Error = String;

    fn try_from(value: MenuId) -> Result<Self, Self::Error> {
        let id_str = value.0.as_str();

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

pub(crate) struct TrayMenuCache {
    is_recording: Arc<AtomicBool>,
}

pub(crate) fn refresh_tray_menu_for_app(app: &AppHandle) {
    refresh_tray_menu(app);
}

/// A menu row with an SF Symbol in front of it. Falls back to a plain row when
/// the symbol is unavailable (older macOS, or another platform entirely).
fn row(
    app: &AppHandle,
    id: impl Into<MenuId>,
    label: &str,
    enabled: bool,
    accelerator: Option<String>,
    symbol: &'static str,
    tint: Tint,
) -> tauri::Result<Box<dyn IsMenuItem<tauri::Wry>>> {
    let id: MenuId = id.into();
    let icon = tray_icons::menu_icon(symbol, tint);

    // A key the menu layer cannot spell must not take the whole menu down with
    // it: the row is built again without its shortcut instead.
    let build = |accelerator: Option<String>| -> tauri::Result<Box<dyn IsMenuItem<tauri::Wry>>> {
        Ok(match icon.clone() {
            Some(icon) => Box::new(IconMenuItem::with_id(
                app,
                id.clone(),
                label,
                enabled,
                Some(icon),
                accelerator,
            )?),
            None => Box::new(MenuItem::with_id(
                app,
                id.clone(),
                label,
                enabled,
                accelerator,
            )?),
        })
    };

    match build(accelerator.clone()) {
        Ok(item) => Ok(item),
        Err(error) if accelerator.is_some() => {
            tracing::warn!("Tray row {label:?} dropped its shortcut: {error}");
            build(None)
        }
        Err(error) => Err(error),
    }
}

fn submenu_with_symbol(
    app: &AppHandle,
    id: &str,
    label: &str,
    symbol: &'static str,
    tint: Tint,
) -> tauri::Result<Submenu<tauri::Wry>> {
    match tray_icons::menu_icon(symbol, tint) {
        Some(icon) => Submenu::with_id_and_icon(app, id, label, true, Some(icon)),
        None => Submenu::with_id(app, id, label, true),
    }
}

/// The shortcut a row should advertise, taken from what is actually
/// registered. An unset action shows no shortcut rather than a wrong one.
fn accelerator_for(
    hotkeys: &HashMap<HotkeyAction, Hotkey>,
    action: HotkeyAction,
) -> Option<String> {
    hotkeys.get(&action).map(Hotkey::accelerator)
}

/// Only shortcuts the system actually holds. A stored shortcut whose
/// registration was refused (taken by another app, say) would otherwise be
/// advertised here and do nothing when pressed.
fn stored_hotkeys(app: &AppHandle) -> HashMap<HotkeyAction, Hotkey> {
    let global_shortcut = app.global_shortcut();

    HotkeysStore::get(app)
        .ok()
        .flatten()
        .map(|store| {
            store
                .entries()
                .into_iter()
                .filter(|(_, hotkey)| global_shortcut.is_registered(hotkey.shortcut()))
                .collect()
        })
        .unwrap_or_default()
}

fn target_label(target: Option<&ScreenCaptureTarget>) -> &'static str {
    match target {
        Some(ScreenCaptureTarget::Display { .. }) => "Display",
        Some(ScreenCaptureTarget::Window { .. }) => "Window",
        Some(ScreenCaptureTarget::Area { .. }) => "Area",
        Some(ScreenCaptureTarget::CameraOnly) => "Camera",
        None => "Display",
    }
}

fn mode_label(mode: RecordingMode) -> &'static str {
    match mode {
        RecordingMode::Studio => "Studio",
        RecordingMode::Instant => "Instant",
        RecordingMode::Screenshot => "Screenshot",
    }
}

fn mode_symbol(mode: RecordingMode) -> &'static str {
    match mode {
        RecordingMode::Studio => symbol::MODE_STUDIO,
        RecordingMode::Instant => symbol::MODE_INSTANT,
        // Deliberately not `CAPTURE`: the Screenshot submenu sits right above
        // this row and two identical glyphs read as a duplicate entry.
        RecordingMode::Screenshot => symbol::MODE_SCREENSHOT,
    }
}

/// The line at the top. It is the only place that says what the app is doing
/// right now, so it names the state and what a click would capture.
fn status_row(
    app: &AppHandle,
    is_recording: bool,
    mode: RecordingMode,
    target: Option<&ScreenCaptureTarget>,
) -> tauri::Result<Box<dyn IsMenuItem<tauri::Wry>>> {
    // The permission check talks to the system, so it is skipped while
    // recording: the answer cannot matter then, capture is already running.
    let permissions_missing =
        !is_recording && !crate::permissions::do_permissions_check(false).necessary_granted();

    let (label, symbol, tint) = if is_recording {
        ("Recording".to_string(), symbol::RECORDING, Tint::Red)
    } else if permissions_missing {
        (
            "Permissions needed".to_string(),
            symbol::WARNING,
            Tint::Amber,
        )
    } else if mode == RecordingMode::Screenshot {
        (
            format!("Ready · Screenshot, {}", target_label(target)),
            symbol::READY,
            Tint::Green,
        )
    } else {
        (
            format!("Ready · {}, {}", mode_label(mode), target_label(target)),
            symbol::READY,
            Tint::Green,
        )
    };

    row(app, "status", &label, false, None, symbol, tint)
}

fn create_screenshot_submenu(
    app: &AppHandle,
    hotkeys: &HashMap<HotkeyAction, Hotkey>,
) -> tauri::Result<Submenu<tauri::Wry>> {
    let submenu = submenu_with_symbol(
        app,
        "screenshot_menu",
        "Screenshot",
        symbol::CAPTURE,
        Tint::Label,
    )?;

    for (tray_item, label, glyph, action) in [
        (
            TrayItem::ScreenshotDisplay,
            "Display",
            symbol::DISPLAY,
            HotkeyAction::ScreenshotDisplay,
        ),
        (
            TrayItem::ScreenshotWindow,
            "Window",
            symbol::WINDOW,
            HotkeyAction::ScreenshotWindow,
        ),
        (
            TrayItem::ScreenshotArea,
            "Area",
            symbol::AREA,
            HotkeyAction::ScreenshotArea,
        ),
    ] {
        submenu.append(
            row(
                app,
                tray_item,
                label,
                true,
                accelerator_for(hotkeys, action),
                glyph,
                Tint::Label,
            )?
            .as_ref(),
        )?;
    }

    Ok(submenu)
}

fn create_record_screen_submenu(
    app: &AppHandle,
    hotkeys: &HashMap<HotkeyAction, Hotkey>,
) -> tauri::Result<Submenu<tauri::Wry>> {
    let submenu = submenu_with_symbol(
        app,
        "record_screen_menu",
        "Record",
        symbol::RECORD_SCREEN,
        Tint::Label,
    )?;

    for (tray_item, label, glyph, action) in [
        (
            TrayItem::RecordDisplay,
            "Display",
            symbol::DISPLAY,
            Some(HotkeyAction::OpenRecordingPickerDisplay),
        ),
        (
            TrayItem::RecordWindow,
            "Window",
            symbol::WINDOW,
            Some(HotkeyAction::OpenRecordingPickerWindow),
        ),
        (
            TrayItem::RecordArea,
            "Area",
            symbol::AREA,
            Some(HotkeyAction::OpenRecordingPickerArea),
        ),
        (
            TrayItem::RecordCameraOnly,
            "Camera Only",
            symbol::CAMERA,
            None,
        ),
    ] {
        submenu.append(
            row(
                app,
                tray_item,
                label,
                true,
                action.and_then(|action| accelerator_for(hotkeys, action)),
                glyph,
                Tint::Label,
            )?
            .as_ref(),
        )?;
    }

    Ok(submenu)
}

/// Mode lives in its own submenu that carries the current choice in its title,
/// so the top level stays short but never hides which mode is active.
fn create_mode_submenu(
    app: &AppHandle,
    current_mode: RecordingMode,
    hotkeys: &HashMap<HotkeyAction, Hotkey>,
) -> tauri::Result<Submenu<tauri::Wry>> {
    let submenu = submenu_with_symbol(
        app,
        "mode_menu",
        &format!("Mode: {}", mode_label(current_mode)),
        mode_symbol(current_mode),
        Tint::Label,
    )?;

    for (tray_item, mode) in [
        (TrayItem::ModeStudio, RecordingMode::Studio),
        (TrayItem::ModeInstant, RecordingMode::Instant),
        (TrayItem::ModeScreenshot, RecordingMode::Screenshot),
    ] {
        submenu.append(&CheckMenuItem::with_id(
            app,
            tray_item,
            mode_label(mode),
            true,
            current_mode == mode,
            None::<&str>,
        )?)?;
    }

    if let Some(accelerator) = accelerator_for(hotkeys, HotkeyAction::CycleRecordingMode) {
        submenu.append(&PredefinedMenuItem::separator(app)?)?;
        submenu.append(&MenuItem::with_id(
            app,
            "mode_cycle_hint",
            format!("Cycle with {accelerator}"),
            false,
            None::<&str>,
        )?)?;
    }

    Ok(submenu)
}

fn build_tray_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    if should_use_minimal_onboarding_tray_menu(app) {
        let menu = Menu::new(app)?;
        menu.append(
            row(
                app,
                TrayItem::RequestPermissions,
                "Request Permissions",
                true,
                None,
                symbol::PERMISSIONS,
                Tint::Amber,
            )?
            .as_ref(),
        )?;
        menu.append(&PredefinedMenuItem::separator(app)?)?;
        menu.append(&MenuItem::with_id(
            app,
            "version",
            format!("Shelf {}", env!("CARGO_PKG_VERSION")),
            false,
            None::<&str>,
        )?)?;
        menu.append(
            row(
                app,
                TrayItem::Quit,
                "Quit Shelf",
                true,
                Some("Cmd+Q".to_string()),
                symbol::QUIT,
                Tint::Secondary,
            )?
            .as_ref(),
        )?;
        return Ok(menu);
    }

    let settings = RecordingSettingsStore::get(app)
        .ok()
        .flatten()
        .unwrap_or_default();
    let current_mode = settings.mode.unwrap_or_default();
    let is_screenshot_mode = current_mode == RecordingMode::Screenshot;

    let is_recording = app
        .try_state::<TrayMenuCache>()
        .map(|state| state.is_recording.load(Ordering::Relaxed))
        .unwrap_or(false);

    let hotkeys = stored_hotkeys(app);

    let menu = Menu::new(app)?;

    // The one action the menu exists for sits under a line that says what the
    // app is doing right now; everything below is grouped by how often it is
    // needed.
    menu.append(status_row(app, is_recording, current_mode, settings.target.as_ref())?.as_ref())?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;

    let (primary_label, primary_symbol, primary_tint) = if is_recording {
        ("Stop Recording", symbol::STOP, Tint::Red)
    } else if is_screenshot_mode {
        ("Take Screenshot", symbol::CAPTURE, Tint::Label)
    } else {
        ("Start Recording", symbol::START, Tint::Red)
    };
    menu.append(
        row(
            app,
            TrayItem::StartStopRecording,
            primary_label,
            true,
            accelerator_for(&hotkeys, HotkeyAction::ToggleRecording),
            primary_symbol,
            primary_tint,
        )?
        .as_ref(),
    )?;

    menu.append(&create_record_screen_submenu(app, &hotkeys)?)?;
    menu.append(&create_screenshot_submenu(app, &hotkeys)?)?;
    menu.append(&create_mode_submenu(app, current_mode, &hotkeys)?)?;

    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(
        row(
            app,
            TrayItem::ViewAllRecordings,
            "Recordings",
            true,
            None,
            symbol::RECORDINGS,
            Tint::Label,
        )?
        .as_ref(),
    )?;
    menu.append(
        row(
            app,
            TrayItem::ViewAllScreenshots,
            "Screenshots",
            true,
            None,
            symbol::SCREENSHOTS,
            Tint::Label,
        )?
        .as_ref(),
    )?;
    menu.append(
        row(
            app,
            TrayItem::OpenTeleprompter,
            "Teleprompter",
            true,
            None,
            symbol::TELEPROMPTER,
            Tint::Label,
        )?
        .as_ref(),
    )?;

    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(
        row(
            app,
            TrayItem::OpenSettings,
            "Settings…",
            true,
            Some("Cmd+,".to_string()),
            symbol::SETTINGS,
            Tint::Label,
        )?
        .as_ref(),
    )?;
    menu.append(
        row(
            app,
            TrayItem::Quit,
            "Quit Shelf",
            true,
            Some("Cmd+Q".to_string()),
            symbol::QUIT,
            Tint::Secondary,
        )?
        .as_ref(),
    )?;

    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        "version",
        format!("Shelf {}", env!("CARGO_PKG_VERSION")),
        false,
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

pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let is_recording = Arc::new(AtomicBool::new(false));

    app.manage(TrayMenuCache {
        is_recording: is_recording.clone(),
    });

    let menu = build_tray_menu(app)?;
    let app = app.clone();

    let current_mode = get_current_mode(&app);
    let initial_icon = Image::from_bytes(get_mode_icon(current_mode))?;

    // Light/dark switches change the glyph colours, so the menu is rebuilt.
    tray_icons::observe_appearance_changes({
        let app = app.clone();
        move || refresh_tray_menu(&app)
    });

    let tray = TrayIconBuilder::with_id("tray")
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

    // Shelf has no main window: without the tray there is no way to reach the
    // app at all, so this must not fail quietly.
    if let Err(error) = tray {
        tracing::error!("Failed to create the tray icon: {error}");
        return Err(error);
    }

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

    Ok(())
}
