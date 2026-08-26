use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Instant,
};
use tokio::sync::OnceCell;

#[derive(Clone)]
pub struct PendingScreenshot {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub channels: u32,
    pub created_at: Instant,
}

/// A display captured just before the area selection overlay went on screen.
///
/// See `cap_recording::screenshot::crop_frozen_display` for why the capture has
/// to happen that early.
pub struct FrozenScreen {
    /// The whole display in physical pixels, cropped to the selection later.
    pub image: image::RgbImage,
    /// A JPEG of the same pixels for the picker to show. Only the image above
    /// ever reaches a saved screenshot, so this copy may be lossy.
    pub preview_path: Option<std::path::PathBuf>,
}

fn remove_preview(frozen: &FrozenScreen) {
    if let Some(path) = &frozen.preview_path {
        let _ = std::fs::remove_file(path);
    }
}

/// Frozen displays, keyed by display id.
///
/// A frozen display lives exactly as long as the picker that owns it: it is put
/// here just before the picker opens and cleared when the picker closes. There
/// is deliberately no expiry. The picker shows the frozen picture, so however
/// long the user takes, what they select from is what they get; an expiry would
/// mean selecting against one picture and receiving another.
pub struct FrozenScreens(Arc<RwLock<HashMap<String, FrozenScreen>>>);

impl Default for FrozenScreens {
    fn default() -> Self {
        Self(Arc::new(RwLock::new(HashMap::new())))
    }
}

impl FrozenScreens {
    /// Stores a frozen display, but only when it can also be shown.
    ///
    /// Without the preview the picker would show the live screen while the
    /// capture came from the frozen image, which is the mismatch this whole
    /// feature exists to avoid. Dropping the image in that case costs the hover
    /// and nothing else.
    pub fn insert(
        &self,
        display_id: String,
        image: image::RgbImage,
        preview_path: Option<std::path::PathBuf>,
    ) {
        let Some(preview_path) = preview_path else {
            tracing::warn!(
                display_id,
                "Discarding the frozen display: it has no preview to show"
            );
            return;
        };

        let mut guard = self.0.write().unwrap();
        for frozen in guard.values() {
            remove_preview(frozen);
        }
        guard.clear();
        guard.insert(
            display_id,
            FrozenScreen {
                image,
                preview_path: Some(preview_path),
            },
        );
    }

    /// The picture the picker should show for this display.
    pub fn preview_path(&self, display_id: &str) -> Option<std::path::PathBuf> {
        self.0.read().unwrap().get(display_id)?.preview_path.clone()
    }

    /// Hands over the frozen display for cropping and retires it: one frozen
    /// image belongs to exactly one capture.
    pub fn take(&self, display_id: &str) -> Option<image::RgbImage> {
        let mut guard = self.0.write().unwrap();
        let frozen = guard.remove(display_id)?;
        remove_preview(&frozen);
        Some(frozen.image)
    }

    pub fn clear(&self) {
        let mut guard = self.0.write().unwrap();
        for frozen in guard.values() {
            remove_preview(frozen);
        }
        guard.clear();
    }

    /// Deletes previews left behind by a previous run.
    ///
    /// The images never outlive the process in memory, but their files do if it
    /// dies without cleaning up, and a whole display can hold anything that was
    /// on screen.
    pub fn remove_orphaned_previews() {
        let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else {
            return;
        };

        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(FROZEN_PREVIEW_PREFIX) && name.ends_with(".jpg") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

/// Shared with the writer in `hotkeys` so cleanup cannot miss a file.
pub const FROZEN_PREVIEW_PREFIX: &str = "shelf-frozen-";

#[cfg(test)]
mod frozen_screens_tests {
    use super::*;

    fn image() -> image::RgbImage {
        image::RgbImage::new(4, 4)
    }

    fn preview() -> Option<std::path::PathBuf> {
        Some(std::env::temp_dir().join("shelf-frozen-test-not-written.jpg"))
    }

    #[test]
    fn a_frozen_display_is_handed_out_once() {
        let screens = FrozenScreens::default();
        screens.insert("1".into(), image(), preview());

        assert!(screens.take("1").is_some());
        // The second capture must fall back to the live screen rather than
        // silently reuse an image the picker is no longer showing.
        assert!(screens.take("1").is_none());
    }

    #[test]
    fn a_display_without_a_preview_is_not_kept() {
        let screens = FrozenScreens::default();
        screens.insert("1".into(), image(), None);

        // Nothing to show means nothing to cut from, otherwise the user would
        // select against the live screen and receive the frozen one.
        assert!(screens.take("1").is_none());
        assert!(screens.preview_path("1").is_none());
    }

    #[test]
    fn only_the_asked_for_display_comes_back() {
        let screens = FrozenScreens::default();
        screens.insert("1".into(), image(), preview());

        assert!(screens.take("2").is_none());
        assert!(screens.take("1").is_some());
    }

    #[test]
    fn a_new_capture_replaces_the_previous_one() {
        let screens = FrozenScreens::default();
        screens.insert("1".into(), image(), preview());
        screens.insert("2".into(), image(), preview());

        assert!(
            screens.take("1").is_none(),
            "the earlier display outlived its picker"
        );
        assert!(screens.take("2").is_some());
    }

    #[test]
    fn whatever_the_picker_shows_is_what_gets_cut() {
        let screens = FrozenScreens::default();
        screens.insert("1".into(), image(), preview());

        // The two must agree at all times: a preview on screen means an image
        // ready to crop, and no preview means none.
        assert_eq!(
            screens.preview_path("1").is_some(),
            screens.take("1").is_some()
        );
        assert_eq!(
            screens.preview_path("1").is_some(),
            screens.take("1").is_some()
        );
    }

    #[test]
    fn clearing_leaves_nothing_for_a_later_capture() {
        let screens = FrozenScreens::default();
        screens.insert("1".into(), image(), preview());
        screens.clear();

        assert!(screens.take("1").is_none());
        assert!(screens.preview_path("1").is_none());
    }
}

pub struct PendingScreenshots(pub Arc<RwLock<HashMap<String, PendingScreenshot>>>);

impl Default for PendingScreenshots {
    fn default() -> Self {
        Self(Arc::new(RwLock::new(HashMap::new())))
    }
}

impl PendingScreenshots {
    pub fn insert(&self, key: String, screenshot: PendingScreenshot) {
        let mut guard = self.0.write().unwrap();
        guard.retain(|_, v| v.created_at.elapsed() < std::time::Duration::from_secs(10));
        guard.insert(key, screenshot);
    }

    pub fn remove(&self, key: &str) -> Option<PendingScreenshot> {
        self.0.write().unwrap().remove(key)
    }

    pub fn get(&self, key: &str) -> Option<PendingScreenshot> {
        self.0.read().unwrap().get(key).cloned()
    }
}

pub struct SharedGpuContext {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub adapter: Arc<wgpu::Adapter>,
    pub instance: Arc<wgpu::Instance>,
    pub is_software_adapter: bool,
    pub background_cache: Arc<cap_rendering::BackgroundTextureCache>,
}

static GPU: OnceCell<Option<SharedGpuContext>> = OnceCell::const_new();

/// Marks the crash sentinel while GPU adapter/device initialisation is in flight, so
/// a process death inside that window is attributable to graphics bring-up. Dropping
/// the guard (including during unwind from a caught panic) disarms the marker — a
/// survivable failure is not a GPU crash.
struct GpuInitPhaseGuard;

impl GpuInitPhaseGuard {
    fn arm() -> Self {
        crate::crash_sentinel::enter_gpu_init_phase();
        Self
    }
}

impl Drop for GpuInitPhaseGuard {
    fn drop(&mut self) {
        crate::crash_sentinel::exit_gpu_init_phase();
    }
}

async fn init_gpu_inner() -> Option<SharedGpuContext> {
    let _gpu_init_phase = GpuInitPhaseGuard::arm();

    let instance = cap_rendering::create_wgpu_instance().await;

    let force_software_adapter = cap_rendering::force_software_wgpu_adapter();
    if force_software_adapter {
        tracing::warn!("Forcing software WGPU adapter for shared context");
    }

    let hardware_adapter = if force_software_adapter {
        None
    } else {
        instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .ok()
    };

    let (adapter, is_software_adapter) = if let Some(adapter) = hardware_adapter {
        let adapter_info = adapter.get_info();
        let is_software_adapter = cap_rendering::is_software_wgpu_adapter(&adapter_info);

        if is_software_adapter {
            tracing::warn!(
                adapter_name = adapter_info.name,
                adapter_backend = ?adapter_info.backend,
                adapter_device_type = ?adapter_info.device_type,
                "Selected shared-context adapter behaves like a software renderer"
            );
        } else {
            tracing::info!(
                adapter_name = adapter_info.name,
                adapter_backend = ?adapter_info.backend,
                adapter_device_type = ?adapter_info.device_type,
                "Using hardware GPU adapter for shared context"
            );
        }

        (adapter, is_software_adapter)
    } else {
        tracing::warn!(
            "No hardware GPU adapter found, attempting software fallback for shared context"
        );
        let software_adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                force_fallback_adapter: true,
                compatible_surface: None,
            })
            .await
            .ok()?;

        let adapter_info = software_adapter.get_info();

        tracing::info!(
            adapter_name = adapter_info.name,
            adapter_backend = ?adapter_info.backend,
            adapter_device_type = ?adapter_info.device_type,
            "Using software adapter for shared context (CPU rendering - performance may be reduced)"
        );
        (software_adapter, true)
    };

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("cap-shared-gpu-device"),
            required_features: wgpu::Features::empty(),
            ..Default::default()
        })
        .await
        .ok()?;

    Some(SharedGpuContext {
        device: Arc::new(device),
        queue: Arc::new(queue),
        adapter: Arc::new(adapter),
        instance: Arc::new(instance),
        is_software_adapter,
        background_cache: Arc::new(cap_rendering::BackgroundTextureCache::default()),
    })
}

pub async fn get_shared_gpu() -> Option<&'static SharedGpuContext> {
    GPU.get_or_init(|| async {
        let result = tokio::spawn(init_gpu_inner()).await;

        match result {
            Ok(ctx) => ctx,
            Err(e) => {
                if e.is_panic() {
                    tracing::error!(
                        "GPU initialization panicked (wgpu internal error). \
                         The app will continue without GPU acceleration."
                    );
                } else {
                    tracing::error!(
                        error = %e,
                        "GPU initialization task failed"
                    );
                }
                None
            }
        }
    })
    .await
    .as_ref()
}

pub fn prewarm_gpu() {
    tokio::spawn(async {
        get_shared_gpu().await;
    });
}
