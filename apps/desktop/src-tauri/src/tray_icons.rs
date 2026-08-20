//! Glyphs for the tray menu, drawn from SF Symbols at runtime.
//!
//! Why not ship PNGs: `muda` hands menu images to AppKit without marking them
//! as template images, so a baked-in black glyph disappears against the dark
//! menu background. Rendering here lets us pick the colour for the appearance
//! that is actually on screen, and it lets the primary action carry a red
//! accent, which a template image could never do.
//!
//! Everything runs on the main thread (AppKit's rule) and is cached per
//! symbol, tint and appearance, so building the menu stays cheap.

#[cfg(target_os = "macos")]
mod imp {
    use objc2::AllocAnyThread;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use tauri::image::Image;

    /// `muda` scales menu images to 18 points high, so an 18 point box at 2x
    /// maps one to one and stays crisp.
    const BOX_PT: f64 = 18.0;
    const BOX_PX: usize = 36;
    /// Point size of the glyph inside that box. Small enough that even the
    /// wide glyphs (`display`, `photo.on.rectangle`) fit without clipping.
    const POINT_SIZE: f64 = 13.0;

    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    pub enum Tint {
        /// Normal menu text colour.
        Label,
        /// Dimmed, for the status line and other read-only rows.
        Secondary,
        /// The record accent.
        Red,
        /// Idle/ready.
        Green,
        /// Something needs attention.
        Amber,
    }

    impl Tint {
        /// Straight (non-premultiplied) RGBA, 0..1.
        fn rgba(self, dark: bool) -> [f64; 4] {
            match (self, dark) {
                (Tint::Label, true) => [1.0, 1.0, 1.0, 0.92],
                (Tint::Label, false) => [0.0, 0.0, 0.0, 0.85],
                (Tint::Secondary, true) => [1.0, 1.0, 1.0, 0.55],
                (Tint::Secondary, false) => [0.0, 0.0, 0.0, 0.5],
                // Apple's systemRed / systemGreen / systemOrange, dark variant first.
                (Tint::Red, true) => [1.0, 0.27, 0.23, 1.0],
                (Tint::Red, false) => [1.0, 0.23, 0.19, 1.0],
                (Tint::Green, true) => [0.19, 0.82, 0.35, 1.0],
                (Tint::Green, false) => [0.2, 0.78, 0.35, 1.0],
                (Tint::Amber, true) => [1.0, 0.62, 0.04, 1.0],
                (Tint::Amber, false) => [1.0, 0.58, 0.0, 1.0],
            }
        }
    }

    type CacheKey = (&'static str, Tint, bool);
    type Bitmap = (Vec<u8>, u32, u32);

    static CACHE: Mutex<Option<HashMap<CacheKey, Option<Bitmap>>>> = Mutex::new(None);

    /// True when the menu bar is drawn dark. Read from the app's effective
    /// appearance, which follows the system setting.
    pub fn is_dark_appearance() -> bool {
        use objc2::MainThreadMarker;
        use objc2_app_kit::{NSAppearanceNameDarkAqua, NSAppearanceNameVibrantDark, NSApplication};

        let Some(mtm) = MainThreadMarker::new() else {
            return false;
        };

        let app = NSApplication::sharedApplication(mtm);
        let appearance = app.effectiveAppearance();
        let name = unsafe { appearance.name() };
        unsafe { &*name == NSAppearanceNameDarkAqua || &*name == NSAppearanceNameVibrantDark }
    }

    /// A tinted SF Symbol, ready for a menu item. `None` when the symbol does
    /// not exist on this macOS version or when called off the main thread; the
    /// caller then falls back to a plain text item.
    pub fn menu_icon(symbol: &'static str, tint: Tint) -> Option<Image<'static>> {
        let dark = is_dark_appearance();
        let key = (symbol, tint, dark);

        let mut guard = CACHE.lock().ok()?;
        let cache = guard.get_or_insert_with(HashMap::new);
        if !cache.contains_key(&key) {
            let rendered = render(symbol, tint.rgba(dark));
            cache.insert(key, rendered);
        }

        let Some((rgba, width, height)) = cache.get(&key)?.as_ref() else {
            // Silent icon-less rows were exactly how a broken pipeline went
            // unnoticed once; say it out loud, once per symbol.
            tracing::warn!("No tray glyph for SF Symbol {symbol:?}");
            return None;
        };
        Some(Image::new_owned(rgba.clone(), *width, *height))
    }

    fn render(symbol: &str, colour: [f64; 4]) -> Option<Bitmap> {
        use objc2::MainThreadMarker;
        use objc2_app_kit::{
            NSBitmapFormat, NSBitmapImageRep, NSColor, NSCompositingOperation,
            NSDeviceRGBColorSpace, NSFontWeightMedium, NSGraphicsContext, NSImage,
            NSImageSymbolConfiguration, NSImageSymbolScale, NSRectFillUsingOperation,
        };
        use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

        // AppKit drawing is main-thread only.
        MainThreadMarker::new()?;

        let name = NSString::from_str(symbol);
        let image =
            unsafe { NSImage::imageWithSystemSymbolName_accessibilityDescription(&name, None) }?;
        let config = unsafe {
            NSImageSymbolConfiguration::configurationWithPointSize_weight_scale(
                POINT_SIZE,
                NSFontWeightMedium,
                NSImageSymbolScale::Medium,
            )
        };
        let image = unsafe { image.imageWithSymbolConfiguration(&config) }?;

        let rep = unsafe {
            NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bitmapFormat_bytesPerRow_bitsPerPixel(
                NSBitmapImageRep::alloc(),
                std::ptr::null_mut(),
                BOX_PX as isize,
                BOX_PX as isize,
                8,
                4,
                true,
                false,
                NSDeviceRGBColorSpace,
                // Straight alpha is not an option: Core Graphics refuses to
                // build a drawing context for a non-premultiplied bitmap, and
                // `NSGraphicsContext` then hands back nil. The pixels are
                // un-premultiplied further down instead.
                NSBitmapFormat::empty(),
                0,
                0,
            )
        }?;
        // The bitmap holds 2x pixels for the box, so drawing in point space
        // lands on the pixel grid.
        unsafe { rep.setSize(NSSize::new(BOX_PT, BOX_PT)) };

        let context = unsafe { NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep) }?;
        unsafe {
            NSGraphicsContext::saveGraphicsState_class();
            NSGraphicsContext::setCurrentContext(Some(&context));
        }

        // Symbols carry their own size and the wide ones (`display`,
        // `photo.on.rectangle`) are broader than the box, so they are scaled to
        // fit and centred rather than clipped.
        let glyph_size = unsafe { image.size() };
        let scale = (BOX_PT / glyph_size.width)
            .min(BOX_PT / glyph_size.height)
            .min(1.0);
        let drawn = NSSize::new(glyph_size.width * scale, glyph_size.height * scale);
        let origin = NSPoint::new((BOX_PT - drawn.width) / 2.0, (BOX_PT - drawn.height) / 2.0);
        let target = NSRect::new(origin, drawn);

        unsafe {
            image.drawInRect_fromRect_operation_fraction(
                target,
                NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0)),
                NSCompositingOperation::SourceOver,
                1.0,
            );

            // Symbols arrive as black glyphs with alpha; painting the colour
            // over them with source-atop keeps the shape and swaps the colour.
            let [r, g, b, a] = colour;
            NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, a).set();
            NSRectFillUsingOperation(target, NSCompositingOperation::SourceAtop);

            NSGraphicsContext::restoreGraphicsState_class();
        }

        let data = unsafe { rep.bitmapData() };
        if data.is_null() {
            return None;
        }

        let bytes_per_row = unsafe { rep.bytesPerRow() } as usize;
        let mut rgba = Vec::with_capacity(BOX_PX * BOX_PX * 4);
        for row in 0..BOX_PX {
            let start = unsafe { data.add(row * bytes_per_row) };
            let slice = unsafe { std::slice::from_raw_parts(start, BOX_PX * 4) };
            // Back to straight alpha: the consumer treats these bytes as plain
            // RGBA, and premultiplied values would darken every soft edge.
            for pixel in slice.chunks_exact(4) {
                let alpha = pixel[3];
                let straight = |value: u8| match alpha {
                    0 => 0,
                    _ => ((value as u16 * 255) / alpha as u16).min(255) as u8,
                };
                rgba.extend_from_slice(&[
                    straight(pixel[0]),
                    straight(pixel[1]),
                    straight(pixel[2]),
                    alpha,
                ]);
            }
        }

        Some((rgba, BOX_PX as u32, BOX_PX as u32))
    }

    /// Drops the cache so the next menu build re-renders in the new colours.
    pub fn clear_cache() {
        if let Ok(mut guard) = CACHE.lock() {
            *guard = None;
        }
    }

    /// Calls `on_change` whenever the system switches between light and dark.
    /// A menu-bar-only app has no window, so Tauri's per-window theme event
    /// never fires; the system-wide notification is the only signal there is.
    pub fn observe_appearance_changes(on_change: impl Fn() + 'static) {
        use block2::RcBlock;
        use objc2::MainThreadMarker;
        use objc2_foundation::{
            NSDistributedNotificationCenter, NSNotification, NSOperationQueue, NSString,
        };
        use std::ptr::NonNull;

        if MainThreadMarker::new().is_none() {
            tracing::warn!("Tray appearance observer must be registered on the main thread");
            return;
        }

        let block = RcBlock::new(move |_: NonNull<NSNotification>| {
            clear_cache();
            on_change();
        });

        let observer = unsafe {
            NSDistributedNotificationCenter::defaultCenter()
                .addObserverForName_object_queue_usingBlock(
                    Some(&NSString::from_str(
                        "AppleInterfaceThemeChangedNotification",
                    )),
                    None,
                    Some(&NSOperationQueue::mainQueue()),
                    &block,
                )
        };

        // The observer has to outlive this call and there is nothing to
        // unregister it from: the tray exists for as long as the app does.
        std::mem::forget(observer);
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use tauri::image::Image;

    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    pub enum Tint {
        Label,
        Secondary,
        Red,
        Green,
        Amber,
    }

    /// Other platforms keep the plain text menu: SF Symbols are macOS only.
    pub fn menu_icon(_symbol: &'static str, _tint: Tint) -> Option<Image<'static>> {
        None
    }

    pub fn clear_cache() {}

    pub fn is_dark_appearance() -> bool {
        false
    }

    pub fn observe_appearance_changes(_on_change: impl Fn() + 'static) {}
}

pub use imp::{Tint, menu_icon, observe_appearance_changes};

/// Symbol names, kept in one place so a typo shows up here and not as a
/// silently icon-less row.
pub mod symbol {
    pub const READY: &str = "circle.fill";
    pub const RECORDING: &str = "record.circle.fill";
    pub const WARNING: &str = "exclamationmark.triangle.fill";
    pub const START: &str = "record.circle";
    pub const STOP: &str = "stop.circle";
    pub const CAPTURE: &str = "camera.viewfinder";
    pub const RECORD_SCREEN: &str = "rectangle.on.rectangle";
    pub const DISPLAY: &str = "display";
    pub const WINDOW: &str = "macwindow";
    pub const AREA: &str = "rectangle.dashed";
    pub const CAMERA: &str = "video";
    pub const MODE_STUDIO: &str = "slider.horizontal.below.rectangle";
    pub const MODE_SCREENSHOT: &str = "camera";
    pub const MODE_INSTANT: &str = "bolt.fill";
    pub const RECORDINGS: &str = "film";
    pub const SCREENSHOTS: &str = "photo.on.rectangle";
    pub const TELEPROMPTER: &str = "text.alignleft";
    pub const SETTINGS: &str = "gearshape";
    pub const QUIT: &str = "power";
    pub const PERMISSIONS: &str = "lock.shield";
}
