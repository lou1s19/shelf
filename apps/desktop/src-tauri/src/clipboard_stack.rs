//! Copying one screenshot right after another puts both on the clipboard as a single image,
//! stacked top to bottom. A series of shots then pastes as one picture instead of forcing a
//! paste, a switch back, and another copy for every single one.
//!
//! A run only continues while the clipboard still holds what this app last wrote to it and the
//! previous copy is not older than [`STACK_WINDOW`]. Without those two guards a copy would
//! silently drag along whatever was copied minutes ago, or overwrite what another app put on
//! the clipboard in the meantime.

use std::{
    future::Future,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant},
};

use image::{Rgba, RgbaImage};
use tracing::{debug, warn};

const STACK_WINDOW: Duration = Duration::from_secs(10);
/// Transparent breathing room between two shots, so the seam is visible when both have the
/// same background colour.
const STACK_GAP: u32 = 12;
/// Every screenshot added grows one image that is decoded and encoded again on the next copy.
/// Past these limits that costs seconds and hundreds of megabytes, so the run ends and the
/// next copy starts a fresh one.
const MAX_STACK: usize = 8;
const MAX_STACK_HEIGHT: u32 = 20_000;

struct Run {
    /// The screenshot added last, to notice the same one being copied twice.
    last: PathBuf,
    /// The image built from all of them, once there is more than one.
    stacked: Option<PathBuf>,
    count: usize,
    last_copy: Instant,
    /// The plain text the clipboard carried right after this app's own write, which is the
    /// path of the image. Anything else copied replaces it and ends the run. Deliberately not
    /// the pasteboard's change counter: macOS bumps that on its own when it re-announces the
    /// clipboard for Handoff, which would end a run nobody touched. `None` off macOS, where
    /// the window alone has to do.
    marker: Option<String>,
}

static RUN: Mutex<Option<Run>> = Mutex::new(None);
/// One copy at a time. Building the image, writing it and noting the clipboard as ours belong
/// together: two copies running side by side could otherwise land on the pasteboard in the
/// wrong order and leave the run describing an image that is no longer on it.
static COPY_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Copies a screenshot together with the ones copied just before it, and hands the file to
/// `write`. Returns how many screenshots ended up on the clipboard.
pub async fn copy_stacked<F, Fut>(path: &Path, write: F) -> Result<usize, String>
where
    F: FnOnce(PathBuf) -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    let _guard = COPY_LOCK.lock().await;

    // Reading, decoding and encoding whole screenshots takes long enough to stall the runtime
    // thread it runs on, so it happens off to the side.
    let owned = path.to_path_buf();
    let copy = tokio::task::spawn_blocking(move || extend(&owned))
        .await
        .map_err(|e| format!("Failed to stack the screenshots: {e}"))?;

    match write(copy.path).await {
        Ok(()) => {
            confirm();
            Ok(copy.count)
        }
        Err(e) => {
            // Nothing reached the clipboard, so the screenshot must not count as copied.
            reset();
            Err(e)
        }
    }
}

/// Copies a single screenshot through `write` and ends any run: whoever calls this wants
/// exactly the one picture on the clipboard.
pub async fn copy_alone<F, Fut>(path: &Path, write: F) -> Result<(), String>
where
    F: FnOnce(PathBuf) -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    let _guard = COPY_LOCK.lock().await;
    let result = write(path.to_path_buf()).await;
    reset();
    result
}

struct StackedCopy {
    path: PathBuf,
    count: usize,
}

fn extend(path: &Path) -> StackedCopy {
    let mut guard = lock();

    let started_fresh = |guard: &mut Option<Run>| {
        *guard = Some(Run {
            last: path.to_path_buf(),
            stacked: None,
            count: 1,
            last_copy: Instant::now(),
            marker: None,
        });
        StackedCopy {
            path: path.to_path_buf(),
            count: 1,
        }
    };

    let Some(mut run) = guard.take().and_then(|run| match interrupted(&run) {
        None => Some(run),
        Some(reason) => {
            debug!(reason, count = run.count, "Clipboard stack starts over");
            None
        }
    }) else {
        return started_fresh(&mut guard);
    };

    // Pressing the shortcut twice on the same card is a retry, not a second picture.
    if run.last == path {
        let copy = StackedCopy {
            path: run.stacked.clone().unwrap_or_else(|| run.last.clone()),
            count: run.count,
        };
        *guard = Some(run);
        return copy;
    }

    if run.count >= MAX_STACK {
        return started_fresh(&mut guard);
    }

    let base = run.stacked.clone().unwrap_or_else(|| run.last.clone());
    match stack(&base, path, run.count + 1) {
        Ok(stacked) => {
            run.last = path.to_path_buf();
            run.stacked = Some(stacked.clone());
            run.count += 1;
            debug!(count = run.count, "Clipboard stack grown");
            let copy = StackedCopy {
                path: stacked,
                count: run.count,
            };
            *guard = Some(run);
            copy
        }
        Err(e) => {
            // An image nobody can build is no reason to copy nothing: fall back to the
            // screenshot that was just asked for and start over from there.
            warn!("Failed to stack the copied screenshots: {e}");
            started_fresh(&mut guard)
        }
    }
}

/// Marks the clipboard as this app's own, so the next copy may extend the run.
fn confirm() {
    let mut guard = lock();
    if let Some(run) = guard.as_mut() {
        run.last_copy = Instant::now();
        run.marker = clipboard_marker();
        debug!(count = run.count, marker = ?run.marker, "Clipboard is ours");
    }
}

fn reset() {
    *lock() = None;
}

fn lock() -> std::sync::MutexGuard<'static, Option<Run>> {
    // A panic while stacking must not take the feature down for the rest of the session.
    RUN.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Why the run cannot go on, or `None` while it can.
fn interrupted(run: &Run) -> Option<&'static str> {
    let idle = run.last_copy.elapsed();
    if idle >= STACK_WINDOW {
        return Some("last copy too long ago");
    }

    let now = clipboard_marker();
    if run.marker != now {
        debug!(ours = ?run.marker, ?now, "Clipboard written elsewhere");
        return Some("clipboard written elsewhere");
    }

    None
}

#[cfg(target_os = "macos")]
fn clipboard_marker() -> Option<String> {
    use cocoa::appkit::{NSPasteboard, NSPasteboardTypeString};
    use cocoa::base::{id, nil};
    use cocoa::foundation::NSString;

    unsafe {
        let pasteboard: id = NSPasteboard::generalPasteboard(nil);
        if pasteboard == nil {
            return None;
        }
        let value: id = pasteboard.stringForType(NSPasteboardTypeString);
        if value == nil {
            return None;
        }
        Some(
            std::ffi::CStr::from_ptr(value.UTF8String())
                .to_string_lossy()
                .into_owned(),
        )
    }
}

#[cfg(not(target_os = "macos"))]
fn clipboard_marker() -> Option<String> {
    None
}

/// Draws `addition` under `base`, both centred on the width of the wider one. `base` is the
/// image built by the copy before, so every copy handles two images, not the whole run.
fn stack(base: &Path, addition: &Path, count: usize) -> Result<PathBuf, String> {
    let read = |path: &Path| {
        image::open(path)
            .map(|image| image.to_rgba8())
            .map_err(|e| format!("Failed to read {}: {e}", path.display()))
    };

    let top = read(base)?;
    let bottom = read(addition)?;

    let width = top.width().max(bottom.width());
    let height = top
        .height()
        .saturating_add(bottom.height())
        .saturating_add(STACK_GAP);

    if width == 0 || height == 0 {
        return Err("Nothing to stack".to_string());
    }
    if height > MAX_STACK_HEIGHT {
        return Err(format!("Stack would be {height} pixels tall"));
    }

    let mut canvas = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 0]));
    image::imageops::replace(&mut canvas, &top, ((width - top.width()) / 2) as i64, 0);
    image::imageops::replace(
        &mut canvas,
        &bottom,
        ((width - bottom.width()) / 2) as i64,
        (top.height() + STACK_GAP) as i64,
    );

    let target = stacked_file_path(count)?;
    canvas
        .save(&target)
        .map_err(|e| format!("Failed to write the stacked image: {e}"))?;

    Ok(target)
}

/// The stacked image leaves the app the same way a dragged screenshot does: through a file in
/// the shared temp folder, so a text field or a terminal pastes a readable name.
fn stacked_file_path(count: usize) -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join(crate::recording::SHARED_FILE_DIR_NAME);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create the shared files directory: {e}"))?;
    crate::recording::prune_shared_files(&dir);

    let target = dir.join(format!("Shelf {count} screenshots.png"));
    let _ = std::fs::remove_file(&target);
    Ok(target)
}
