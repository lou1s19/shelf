use crate::{AppSounds, general_settings::GeneralSettingsStore};
use tauri_plugin_notification::NotificationExt;

#[allow(unused)]
pub enum NotificationType {
    VideoSaved,
    VideoCopiedToClipboard,
    VideoSaveFailed,
    VideoCopyFailed,
    ScreenshotSaved,
    ScreenshotCopiedToClipboard,
    ScreenshotSaveFailed,
    ScreenshotCopyFailed,
    TextCopiedToClipboard,
    TextCopyFailed,
    TextRecognitionFailed,
    ScreenshotAreaMissing,
}

impl NotificationType {
    fn details(&self) -> (&'static str, &'static str, bool) {
        match self {
            NotificationType::VideoSaved => ("Video Saved", "Video saved successfully", false),
            NotificationType::VideoCopiedToClipboard => {
                ("Video Copied", "Video copied to clipboard", false)
            }
            NotificationType::VideoSaveFailed => (
                "Save Failed",
                "Unable to save video. Please try again",
                true,
            ),
            NotificationType::VideoCopyFailed => (
                "Copy Failed",
                "Unable to copy video to clipboard. Please try again",
                true,
            ),
            NotificationType::ScreenshotSaved => {
                ("Screenshot Saved", "Screenshot saved successfully", false)
            }
            NotificationType::ScreenshotCopiedToClipboard => {
                ("Screenshot Copied", "Screenshot copied to clipboard", false)
            }
            NotificationType::ScreenshotAreaMissing => (
                "No Area Yet",
                "Pick an area once with the area screenshot shortcut, then this one repeats it",
                true,
            ),
            NotificationType::ScreenshotSaveFailed => (
                "Save Failed",
                "Unable to save screenshot. Please try again",
                true,
            ),
            NotificationType::ScreenshotCopyFailed => (
                "Copy Failed",
                "Unable to copy screenshot to clipboard. Please try again",
                true,
            ),
            NotificationType::TextCopiedToClipboard => {
                ("Text Copied", "Recognized text copied to clipboard", false)
            }
            NotificationType::TextCopyFailed => (
                "Copy Failed",
                "Unable to copy text to clipboard. Please try again",
                true,
            ),
            NotificationType::TextRecognitionFailed => (
                "No Text Found",
                "No text was recognized in the screenshot",
                true,
            ),
        }
    }
}

/// Failures that the main window used to show as a toast. Without that window
/// a failed recording start would be visible only in the log, so it goes to the
/// system notification centre instead, with the real reason attached.
///
/// Deliberately ignores the "system notifications" setting: that switch is for
/// the routine "saved / copied" messages. A recording that refused to start has
/// to reach the user, otherwise pressing the tray item appears to do nothing.
/// If the notification itself cannot be delivered (permission denied at OS
/// level), a native dialog takes over as the last resort.
pub fn send_failure_notification(app: &tauri::AppHandle, title: &str, body: &str) {
    let shown = app
        .notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .is_ok();

    if shown {
        AppSounds::Notification.play();
        return;
    }

    tracing::warn!(
        title,
        "Notification could not be delivered; falling back to a dialog"
    );

    use tauri_plugin_dialog::DialogExt;
    app.dialog()
        .message(body.to_string())
        .title(title.to_string())
        .kind(tauri_plugin_dialog::MessageDialogKind::Error)
        .show(|_| {});
}

pub fn send_notification(app: &tauri::AppHandle, notification_type: NotificationType) {
    let enable_notifications = GeneralSettingsStore::get(app)
        .map(|settings| settings.is_some_and(|s| s.enable_notifications))
        .unwrap_or(false);

    if !enable_notifications {
        return;
    }

    let (title, body, _is_error) = notification_type.details();

    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .ok();

    let skip_sound = matches!(
        notification_type,
        NotificationType::ScreenshotSaved
            | NotificationType::ScreenshotCopiedToClipboard
            | NotificationType::ScreenshotSaveFailed
            | NotificationType::ScreenshotCopyFailed
            | NotificationType::TextCopiedToClipboard
            | NotificationType::TextCopyFailed
            | NotificationType::TextRecognitionFailed
    );

    if !skip_sound {
        AppSounds::Notification.play();
    }
}
