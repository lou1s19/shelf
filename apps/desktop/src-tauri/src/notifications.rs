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
pub fn send_failure_notification(app: &tauri::AppHandle, title: &str, body: &str) {
    let enable_notifications = GeneralSettingsStore::get(app)
        .map(|settings| settings.is_some_and(|s| s.enable_notifications))
        .unwrap_or(false);

    if !enable_notifications {
        return;
    }

    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .ok();

    AppSounds::Notification.play();
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
