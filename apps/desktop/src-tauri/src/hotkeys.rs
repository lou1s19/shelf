use crate::{
    App, ArcLock, RequestOpenRecordingPicker, RequestStartRecording, recording,
    recording_settings::{RecordingSettingsStore, RecordingTargetMode},
    tray,
    windows::ShowCapWindow,
};
use cap_recording::feeds::microphone::MicrophoneFeed;
use cap_recording::screen_capture::ScreenCaptureTarget;
use global_hotkey::HotKeyState;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};
use tauri_plugin_store::StoreExt;
use tauri_specta::Event;
use tracing::{debug, error, instrument};

#[derive(Serialize, Deserialize, Type, PartialEq, Clone, Copy, Debug)]
pub struct Hotkey {
    #[specta(type = String)]
    code: Code,
    meta: bool,
    ctrl: bool,
    alt: bool,
    shift: bool,
}

impl From<Hotkey> for Shortcut {
    fn from(hotkey: Hotkey) -> Self {
        let mut modifiers = Modifiers::empty();

        if hotkey.meta {
            modifiers |= Modifiers::META;
        }
        if hotkey.ctrl {
            modifiers |= Modifiers::CONTROL;
        }
        if hotkey.alt {
            modifiers |= Modifiers::ALT;
        }
        if hotkey.shift {
            modifiers |= Modifiers::SHIFT;
        }

        Shortcut::new(Some(modifiers), hotkey.code)
    }
}

#[derive(Serialize, Deserialize, Type, PartialEq, Eq, Hash, Clone, Copy, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::enum_variant_names)]
pub enum HotkeyAction {
    ToggleRecording,
    // Retired in favour of `ToggleRecording`. The variants stay so that stored
    // entries keep their own name: without them every retired name would
    // deserialize into `Other`, collapse onto one map key and leave a
    // system-wide shortcut registered that does nothing.
    StartStudioRecording,
    StartInstantRecording,
    StopRecording,
    RestartRecording,
    TogglePauseRecording,
    CycleRecordingMode,
    OpenRecordingPicker,
    OpenRecordingPickerDisplay,
    OpenRecordingPickerWindow,
    OpenRecordingPickerArea,
    ScreenshotDisplay,
    ScreenshotWindow,
    ScreenshotArea,
    ScreenshotAreaOcr,
    #[serde(other)]
    Other,
}

#[derive(Serialize, Deserialize, Type, Default)]
pub struct HotkeysStore {
    hotkeys: HashMap<HotkeyAction, Hotkey>,
}

/// Actions that were replaced by [`HotkeyAction::ToggleRecording`], in the order
/// they are taken over.
const RETIRED_RECORDING_ACTIONS: [HotkeyAction; 3] = [
    HotkeyAction::StopRecording,
    HotkeyAction::StartStudioRecording,
    HotkeyAction::StartInstantRecording,
];

impl HotkeysStore {
    pub fn get(app: &AppHandle) -> Result<Option<Self>, String> {
        let Ok(Some(store)) = app.store("store").map(|s| s.get("hotkeys")) else {
            return Ok(None);
        };

        let mut store: Option<Self> = serde_json::from_value(store).map_err(|e| e.to_string())?;
        if let Some(store) = store.as_mut() {
            store.migrate();
        }

        Ok(store)
    }

    /// Moves a shortcut from a retired action onto the toggle and drops entries
    /// that can no longer do anything. `Other` collects every unknown name, so
    /// keeping it would register a shortcut that swallows the keys silently.
    fn migrate(&mut self) {
        if !self.hotkeys.contains_key(&HotkeyAction::ToggleRecording)
            && let Some(hotkey) = RETIRED_RECORDING_ACTIONS
                .iter()
                .find_map(|action| self.hotkeys.get(action).copied())
        {
            self.hotkeys.insert(HotkeyAction::ToggleRecording, hotkey);
        }

        for action in RETIRED_RECORDING_ACTIONS {
            self.hotkeys.remove(&action);
        }
        self.hotkeys.remove(&HotkeyAction::Other);
    }
}

#[derive(Serialize, Type, tauri_specta::Event, Debug, Clone)]
pub struct OnEscapePress;

/// Cmd+C over a pinned card. The overlay is a non-activating panel and never
/// becomes key window, so it receives no key events of its own. The shortcut is
/// therefore registered globally, but only while the overlay armed it.
#[derive(Serialize, Type, tauri_specta::Event, Debug, Clone)]
pub struct OnPinCopyPress;

#[derive(Default)]
pub struct PinCopyShortcut(Mutex<bool>);

/// Built the same way as the stored hotkeys instead of parsing a string: the
/// parser picks its own modifier bit, and the handler then never matched.
fn pin_copy_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::META), Code::KeyC)
}

#[tauri::command(async)]
#[specta::specta]
#[instrument(skip(app))]
pub fn set_pin_copy_shortcut_active(app: AppHandle, active: bool) {
    let state = app.state::<PinCopyShortcut>();
    let global_shortcut = app.global_shortcut();

    let mut registered = state.0.lock().unwrap_or_else(|e| e.into_inner());
    if *registered == active {
        return;
    }

    if active {
        match global_shortcut.register(pin_copy_shortcut()) {
            Ok(()) => {
                *registered = true;
                debug!("Grabbed the pin copy shortcut");
            }
            Err(err) => error!("Failed to grab the pin copy shortcut: {err}"),
        }
    } else {
        if let Err(err) = global_shortcut.unregister(pin_copy_shortcut()) {
            debug!("Failed to release the pin copy shortcut: {err}");
        }
        *registered = false;
        debug!("Released the pin copy shortcut");
    }
}

pub type HotkeysState = Mutex<HotkeysStore>;

/// An area screenshot to text request only counts for this long, so an abandoned area
/// selection can't turn the next ordinary screenshot into an OCR capture.
const PENDING_OCR_CAPTURE_TIMEOUT: Duration = Duration::from_secs(120);

/// Tracks whether the pending area capture was started by the OCR hotkey.
#[derive(Default)]
pub struct PendingOcrCapture(Mutex<Option<Instant>>);

impl PendingOcrCapture {
    fn mark(&self) {
        *self.0.lock().unwrap() = Some(Instant::now());
    }

    /// Whether an OCR request is still pending, without consuming it.
    pub fn is_active(&self) -> bool {
        let mut requested_at = self.0.lock().unwrap();

        match *requested_at {
            Some(at) if at.elapsed() < PENDING_OCR_CAPTURE_TIMEOUT => true,
            Some(_) => {
                *requested_at = None;
                false
            }
            None => false,
        }
    }

    /// Clears the pending request and reports whether it was still valid.
    pub fn take(&self) -> bool {
        let requested_at = self.0.lock().unwrap().take();

        requested_at.is_some_and(|at| at.elapsed() < PENDING_OCR_CAPTURE_TIMEOUT)
    }
}

const RECORDING_START_SAFETY_STORE_KEY: &str = "recording_start_safety";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct RecordingStartSafetySettings {
    confirm_before_recording_without_microphone: bool,
}

impl Default for RecordingStartSafetySettings {
    fn default() -> Self {
        Self {
            confirm_before_recording_without_microphone: true,
        }
    }
}

fn should_confirm_without_microphone(enabled: bool, microphone_available: bool) -> bool {
    enabled && !microphone_available
}

fn should_confirm_direct_recording(app: &AppHandle) -> bool {
    let enabled = app
        .store("store")
        .ok()
        .and_then(|store| store.get(RECORDING_START_SAFETY_STORE_KEY))
        .and_then(|value| serde_json::from_value::<RecordingStartSafetySettings>(value).ok())
        .unwrap_or_default()
        .confirm_before_recording_without_microphone;

    if !enabled {
        return false;
    }

    let microphone_name = RecordingSettingsStore::get(app)
        .ok()
        .flatten()
        .and_then(|settings| settings.mic_name);
    let microphone_available = microphone_name
        .as_deref()
        .is_some_and(|name| MicrophoneFeed::list().contains_key(name));

    should_confirm_without_microphone(enabled, microphone_available)
}

async fn confirm_direct_recording_without_microphone(app: &AppHandle) -> bool {
    if !should_confirm_direct_recording(app) {
        return true;
    }

    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .message("This recording will not include your voice.")
        .title("No microphone detected")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Record without microphone".to_string(),
            "Cancel".to_string(),
        ))
        .show(move |confirmed| {
            let _ = sender.send(confirmed);
        });

    receiver.await.unwrap_or(false)
}

async fn start_recording_from_hotkey(
    app: AppHandle,
    mode: cap_recording::RecordingMode,
) -> Result<(), String> {
    if app
        .state::<ArcLock<App>>()
        .read()
        .await
        .is_recording_active_or_pending()
    {
        let _ = RequestStartRecording { mode }.emit(&app);
        return Ok(());
    }

    if confirm_direct_recording_without_microphone(&app).await {
        let _ = RequestStartRecording { mode }.emit(&app);
    }

    Ok(())
}

fn current_recording_mode(app: &AppHandle) -> cap_recording::RecordingMode {
    RecordingSettingsStore::get(app)
        .ok()
        .flatten()
        .and_then(|s| s.mode)
        .unwrap_or_default()
}

async fn screenshot_current_display(app: &AppHandle) -> Result<(), String> {
    use scap_targets::Display;

    let display = Display::get_containing_cursor().unwrap_or_else(Display::primary);
    let target = ScreenCaptureTarget::Display { id: display.id() };

    match recording::take_screenshot(app.clone(), target.clone()).await {
        Ok(path) => {
            if crate::automation::should_open_screenshot_editor(app, &target) {
                let _ = ShowCapWindow::ScreenshotEditor { path }.show(app).await;
            }
            Ok(())
        }
        Err(e) => Err(format!("Failed to take screenshot: {e}")),
    }
}

/// One shortcut for both directions: it stops a recording that is running or
/// starting up, otherwise it starts one in the currently selected mode.
async fn toggle_recording_from_hotkey(app: AppHandle) -> Result<(), String> {
    let recording_in_progress = app
        .state::<ArcLock<App>>()
        .read()
        .await
        .is_recording_active_or_pending();

    if recording_in_progress {
        // Covers the pending state too, so the shortcut never starts a second
        // recording during the countdown.
        return recording::stop_recording(app.clone(), app.state()).await;
    }

    // Bound before the match so the borrow of `app` ends and the start path can
    // take ownership of the handle.
    let mode = current_recording_mode(&app);

    match mode {
        // Screenshot mode has no recording to toggle, so the shortcut does what
        // the mode says and captures the display under the cursor.
        cap_recording::RecordingMode::Screenshot => screenshot_current_display(&app).await,
        mode => start_recording_from_hotkey(app, mode).await,
    }
}

pub fn init(app: &AppHandle) {
    app.plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(|app, shortcut, event| {
                if !matches!(event.state(), HotKeyState::Pressed) {
                    return;
                }

                debug!(%shortcut, "Global shortcut arrived");

                // No modifier check: this shortcut is only registered while the
                // overlay armed it, so any C arriving here is that one.
                if shortcut.key == Code::KeyC
                    && app
                        .try_state::<PinCopyShortcut>()
                        .is_some_and(|state| *state.0.lock().unwrap_or_else(|e| e.into_inner()))
                {
                    debug!("Pin copy shortcut fired");
                    OnPinCopyPress.emit(app).ok();
                }

                if shortcut.key == Code::Escape {
                    OnEscapePress.emit(app).ok();
                }

                if shortcut.key == Code::Comma && shortcut.mods == Modifiers::META {
                    let app = app.clone();
                    tokio::spawn(async move {
                        let _ = ShowCapWindow::Settings { page: None }.show(&app).await;
                    });
                }

                let state = app.state::<HotkeysState>();
                let store = state.lock().unwrap();

                for (action, hotkey) in &store.hotkeys {
                    if &Shortcut::from(*hotkey) == shortcut {
                        tracing::debug!(?action, %shortcut, "Hotkey fired");
                        tokio::spawn(handle_hotkey(app.clone(), *action));
                    }
                }
            })
            .build(),
    )
    .unwrap();

    let store = match HotkeysStore::get(app) {
        Ok(Some(s)) => s,
        Ok(None) => HotkeysStore::default(),
        Err(e) => {
            eprintln!("Failed to load hotkeys store: {e}");
            HotkeysStore::default()
        }
    };

    let global_shortcut = app.global_shortcut();
    for (action, hotkey) in &store.hotkeys {
        let shortcut = Shortcut::from(*hotkey);
        // Registration failures used to be discarded, which made a shortcut the
        // system refuses look like a broken feature.
        match global_shortcut.register(shortcut) {
            Ok(()) => tracing::debug!(?action, %shortcut, "Registered hotkey"),
            Err(error) => {
                tracing::warn!(?action, %shortcut, %error, "Could not register hotkey")
            }
        }
    }

    app.manage(Mutex::new(store));
}

fn open_area_screenshot_picker(app: &AppHandle) -> Result<(), String> {
    RecordingSettingsStore::set_mode(app, cap_recording::RecordingMode::Screenshot)
        .map_err(|e| format!("Failed to set screenshot mode: {e}"))?;

    tray::update_tray_icon_for_mode(app, cap_recording::RecordingMode::Screenshot);

    let _ = RequestOpenRecordingPicker {
        target_mode: Some(RecordingTargetMode::Area),
    }
    .emit(app);

    Ok(())
}

async fn handle_hotkey(app: AppHandle, action: HotkeyAction) -> Result<(), String> {
    match action {
        HotkeyAction::ToggleRecording => toggle_recording_from_hotkey(app).await,
        HotkeyAction::StartStudioRecording => {
            start_recording_from_hotkey(app, cap_recording::RecordingMode::Studio).await
        }
        HotkeyAction::StartInstantRecording => {
            start_recording_from_hotkey(app, cap_recording::RecordingMode::Instant).await
        }
        HotkeyAction::StopRecording => recording::stop_recording(app.clone(), app.state()).await,
        HotkeyAction::RestartRecording => recording::restart_recording(app.clone(), app.state())
            .await
            .map(|_| ()),
        HotkeyAction::TogglePauseRecording => {
            recording::toggle_pause_recording(app.clone(), app.state()).await
        }
        HotkeyAction::CycleRecordingMode => {
            let next = match current_recording_mode(&app) {
                cap_recording::RecordingMode::Studio => cap_recording::RecordingMode::Instant,
                cap_recording::RecordingMode::Instant => cap_recording::RecordingMode::Screenshot,
                cap_recording::RecordingMode::Screenshot => cap_recording::RecordingMode::Studio,
            };

            RecordingSettingsStore::set_mode(&app, next)
                .map_err(|e| format!("Failed to cycle mode: {e}"))?;

            tray::update_tray_icon_for_mode(&app, next);

            Ok(())
        }
        HotkeyAction::OpenRecordingPicker => {
            let _ = RequestOpenRecordingPicker { target_mode: None }.emit(&app);
            Ok(())
        }
        HotkeyAction::OpenRecordingPickerDisplay => {
            let _ = RequestOpenRecordingPicker {
                target_mode: Some(RecordingTargetMode::Display),
            }
            .emit(&app);
            Ok(())
        }
        HotkeyAction::OpenRecordingPickerWindow => {
            let _ = RequestOpenRecordingPicker {
                target_mode: Some(RecordingTargetMode::Window),
            }
            .emit(&app);
            Ok(())
        }
        HotkeyAction::OpenRecordingPickerArea => {
            let _ = RequestOpenRecordingPicker {
                target_mode: Some(RecordingTargetMode::Area),
            }
            .emit(&app);
            Ok(())
        }
        HotkeyAction::ScreenshotDisplay => screenshot_current_display(&app).await,
        HotkeyAction::ScreenshotWindow => {
            use scap_targets::Window;

            let target = {
                let window = Window::get_topmost_at_cursor()
                    .ok_or_else(|| "No window found under cursor".to_string())?;
                ScreenCaptureTarget::Window { id: window.id() }
            };

            match recording::take_screenshot(app.clone(), target.clone()).await {
                Ok(path) => {
                    if crate::automation::should_open_screenshot_editor(&app, &target) {
                        let _ = ShowCapWindow::ScreenshotEditor { path }.show(&app).await;
                    }
                    Ok(())
                }
                Err(e) => Err(format!("Failed to take screenshot: {e}")),
            }
        }
        HotkeyAction::ScreenshotArea => open_area_screenshot_picker(&app),
        HotkeyAction::ScreenshotAreaOcr => {
            app.state::<PendingOcrCapture>().mark();

            open_area_screenshot_picker(&app)
        }
        HotkeyAction::Other => Ok(()),
    }
}

#[tauri::command(async)]
#[specta::specta]
#[instrument(skip(app))]
pub fn set_hotkey(
    app: AppHandle,
    action: HotkeyAction,
    hotkey: Option<Hotkey>,
) -> Result<(), String> {
    let global_shortcut = app.global_shortcut();
    let state = app.state::<HotkeysState>();
    let mut store = state.lock().unwrap();

    let prev = store.hotkeys.get(&action).cloned();

    if let Some(hotkey) = hotkey {
        store.hotkeys.insert(action, hotkey);
    } else {
        store.hotkeys.remove(&action);
    }

    if let Some(prev) = prev
        && !store.hotkeys.values().any(|h| h == &prev)
    {
        global_shortcut.unregister(Shortcut::from(prev)).ok();
    }

    if let Some(hotkey) = hotkey {
        let shortcut = Shortcut::from(hotkey);
        // Two actions may share one shortcut; the handler fans out to all of
        // them. Registering the same one twice fails, so an existing
        // registration counts as success.
        if !global_shortcut.is_registered(shortcut)
            && let Err(error) = global_shortcut.register(shortcut)
        {
            // The caller reverts on this. Swallowing it left a shortcut that
            // looked set everywhere but never reached the app.
            store.hotkeys.remove(&action);
            if let Some(prev) = prev {
                store.hotkeys.insert(action, prev);
                global_shortcut.register(Shortcut::from(prev)).ok();
            }
            return Err(format!("{error}"));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Hotkey, HotkeyAction, HotkeysStore, should_confirm_without_microphone};
    use tauri_plugin_global_shortcut::Code;

    fn hotkey(code: Code) -> Hotkey {
        Hotkey {
            code,
            meta: false,
            ctrl: true,
            alt: false,
            shift: true,
        }
    }

    fn store_with(entries: &[(HotkeyAction, Hotkey)]) -> HotkeysStore {
        let mut store = HotkeysStore::default();
        for (action, key) in entries {
            store.hotkeys.insert(*action, *key);
        }
        store
    }

    #[test]
    fn migration_moves_stop_recording_onto_the_toggle() {
        let binding = hotkey(Code::KeyS);
        let mut store = store_with(&[(HotkeyAction::StopRecording, binding)]);

        store.migrate();

        assert_eq!(
            store.hotkeys.get(&HotkeyAction::ToggleRecording),
            Some(&binding)
        );
        assert!(!store.hotkeys.contains_key(&HotkeyAction::StopRecording));
    }

    #[test]
    fn migration_keeps_an_existing_toggle_binding() {
        let toggle = hotkey(Code::KeyT);
        let mut store = store_with(&[
            (HotkeyAction::ToggleRecording, toggle),
            (HotkeyAction::StopRecording, hotkey(Code::KeyS)),
        ]);

        store.migrate();

        assert_eq!(
            store.hotkeys.get(&HotkeyAction::ToggleRecording),
            Some(&toggle)
        );
    }

    #[test]
    fn migration_drops_unknown_actions() {
        let mut store = store_with(&[(HotkeyAction::Other, hotkey(Code::KeyX))]);

        store.migrate();

        assert!(store.hotkeys.is_empty());
    }

    #[test]
    fn confirms_when_enabled_without_microphone() {
        assert!(should_confirm_without_microphone(true, false));
    }

    #[test]
    fn skips_confirmation_with_selected_microphone() {
        assert!(!should_confirm_without_microphone(true, true));
    }

    #[test]
    fn skips_confirmation_when_disabled() {
        assert!(!should_confirm_without_microphone(false, false));
    }
}
