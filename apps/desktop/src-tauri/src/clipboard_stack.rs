//! Copying one screenshot right after another puts both on the clipboard as a single image,
//! stacked top to bottom. A series of shots then pastes as one picture instead of forcing a
//! paste, a switch back, and another copy for every single one.
//!
//! A run only continues while the clipboard still holds what this app last wrote to it and the
//! previous copy is not older than [`STACK_WINDOW`]. Without those two guards a copy would
//! silently drag along whatever was copied minutes ago, or overwrite what another app put on
//! the clipboard in the meantime.

use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant},
};

use image::{Rgba, RgbaImage};
use tracing::warn;

const STACK_WINDOW: Duration = Duration::from_secs(10);
/// Transparent breathing room between two shots, so the seam is visible when both have the
/// same background colour.
const STACK_GAP: u32 = 12;

struct Run {
    paths: Vec<PathBuf>,
    stacked: Option<PathBuf>,
    last_copy: Instant,
    /// What the pasteboard counted right after this app's own write. Anything else writing to
    /// the clipboard moves that number on and ends the run. Always 0 off macOS, where the
    /// window alone has to do.
    change_count: i64,
}

static RUN: Mutex<Option<Run>> = Mutex::new(None);

pub struct StackedCopy {
    /// The file to hand to the clipboard: the screenshot itself for the first copy of a run,
    /// the stacked image after that.
    pub path: PathBuf,
    /// How many screenshots that file holds.
    pub count: usize,
}

/// Adds a screenshot to the current run, or starts a new one, and returns what belongs on the
/// clipboard now. Call [`confirm`] once the write actually happened.
pub fn extend(path: &Path) -> StackedCopy {
    let mut guard = lock();

    let mut run = match guard.take() {
        Some(run) if continues(&run) => run,
        _ => Run {
            paths: Vec::new(),
            stacked: None,
            last_copy: Instant::now(),
            change_count: 0,
        },
    };

    // Pressing the shortcut twice on the same card is a retry, not a second picture.
    if run.paths.last().map(PathBuf::as_path) == Some(path) {
        let copy = StackedCopy {
            path: run.stacked.clone().unwrap_or_else(|| path.to_path_buf()),
            count: run.paths.len(),
        };
        *guard = Some(run);
        return copy;
    }

    run.paths.push(path.to_path_buf());

    if run.paths.len() > 1 {
        match stack(&run.paths) {
            Ok(stacked) => run.stacked = Some(stacked),
            Err(e) => {
                // A picture nobody can build is no reason to copy nothing: fall back to the
                // screenshot that was just asked for and start over from there.
                warn!("Failed to stack the copied screenshots: {e}");
                run.paths = vec![path.to_path_buf()];
                run.stacked = None;
            }
        }
    }

    let copy = StackedCopy {
        path: run
            .stacked
            .clone()
            .unwrap_or_else(|| run.paths[0].clone()),
        count: run.paths.len(),
    };
    *guard = Some(run);
    copy
}

/// Marks the clipboard as this app's own, so the next copy may extend the run.
pub fn confirm() {
    let mut guard = lock();
    if let Some(run) = guard.as_mut() {
        run.last_copy = Instant::now();
        run.change_count = clipboard_change_count();
    }
}

/// Ends the run. Used by everything that writes to the clipboard without stacking.
pub fn reset() {
    *lock() = None;
}

fn lock() -> std::sync::MutexGuard<'static, Option<Run>> {
    // A panic while stacking must not take the feature down for the rest of the session.
    RUN.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn continues(run: &Run) -> bool {
    run.last_copy.elapsed() < STACK_WINDOW && run.change_count == clipboard_change_count()
}

#[cfg(target_os = "macos")]
fn clipboard_change_count() -> i64 {
    use cocoa::appkit::NSPasteboard;
    use cocoa::base::{id, nil};

    unsafe {
        let pasteboard: id = NSPasteboard::generalPasteboard(nil);
        if pasteboard == nil {
            return 0;
        }
        pasteboard.changeCount() as i64
    }
}

#[cfg(not(target_os = "macos"))]
fn clipboard_change_count() -> i64 {
    0
}

/// Draws the screenshots onto one canvas, top to bottom, each centred on the widest one.
fn stack(paths: &[PathBuf]) -> Result<PathBuf, String> {
    let images = paths
        .iter()
        .map(|path| {
            image::open(path)
                .map(|image| image.to_rgba8())
                .map_err(|e| format!("Failed to read {}: {e}", path.display()))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let width = images.iter().map(|image| image.width()).max().unwrap_or(0);
    let height = images
        .iter()
        .map(|image| image.height())
        .sum::<u32>()
        .saturating_add(STACK_GAP * (images.len() as u32 - 1));

    if width == 0 || height == 0 {
        return Err("Nothing to stack".to_string());
    }

    let mut canvas = RgbaImage::from_pixel(width, height, Rgba([0, 0, 0, 0]));
    let mut y = 0i64;
    for image in &images {
        let x = ((width - image.width()) / 2) as i64;
        image::imageops::replace(&mut canvas, image, x, y);
        y += image.height() as i64 + STACK_GAP as i64;
    }

    let target = stacked_file_path(images.len())?;
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
