//! Audited AppKit boundary. All objects stay on the calling worker thread and are
//! released inside its autorelease pool; no views, windows, or system font copies.
use objc2::{AnyThread, rc::autoreleasepool, runtime::AnyObject};
use objc2_app_kit::{
    NSBitmapFormat, NSBitmapImageRep, NSDeviceRGBColorSpace, NSFont, NSFontAttributeName,
    NSGraphicsContext, NSStringDrawing,
};
use objc2_foundation::{NSDictionary, NSPoint, NSString};

pub(super) fn raster(text: &str) -> Option<egui::ColorImage> {
    if text.is_empty() || text.len() > 128 {
        return None;
    }
    autoreleasepool(|_| {
        let font = NSFont::fontWithName_size(&NSString::from_str("AppleColorEmoji"), 48.0)?;
        // SAFETY: A null planes pointer asks AppKit to own the storage. Dimensions,
        // stride and RGBA format are fixed and consistent (64 * 64 * 4 bytes).
        // The retained bitmap outlives every read/write of its pixel pointer.
        unsafe {
            let bitmap = NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bitmapFormat_bytesPerRow_bitsPerPixel(NSBitmapImageRep::alloc(), std::ptr::null_mut(), 64, 64, 8, 4, true, false, NSDeviceRGBColorSpace, NSBitmapFormat::empty(), 256, 32)?;
            let pixels = bitmap.bitmapData();
            if pixels.is_null() || bitmap.bytesPerRow() != 256 || bitmap.pixelsHigh() != 64 {
                return None;
            }
            std::ptr::write_bytes(pixels, 0, 64 * 256);
            let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&bitmap)?;
            // The attribute dictionary has the documented NSFont key/value type.
            let attrs = NSDictionary::<NSString, AnyObject>::from_slices(
                &[NSFontAttributeName],
                &[font.as_ref()],
            );
            let text = NSString::from_str(text);
            let size = text.sizeWithAttributes(Some(&attrs));
            // Never crop a sequence that the OS renders as separate wide glyphs.
            if size.width > 64.0 || size.height > 64.0 {
                return None;
            }
            NSGraphicsContext::saveGraphicsState_class();
            NSGraphicsContext::setCurrentContext(Some(&context));
            text.drawAtPoint_withAttributes(
                NSPoint::new((64.0 - size.width) / 2.0, (64.0 - size.height) / 2.0),
                Some(&attrs),
            );
            NSGraphicsContext::restoreGraphicsState_class();
            let pixels = std::slice::from_raw_parts(pixels, 64 * 256);
            // Missing-glyph boxes and monochrome text are not color emoji fallbacks.
            if !pixels
                .as_chunks::<4>()
                .0
                .iter()
                .any(|p| p[3] > 0 && (p[0] != p[1] || p[1] != p[2]))
            {
                return None;
            }
            Some(egui::ColorImage::from_rgba_premultiplied([64, 64], pixels))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_os_rasterizer_preserves_color_and_complete_sequences() {
        std::thread::spawn(|| {
            for text in ["🌙", "👩🏽‍💻", "🇨🇿"] {
                let image = raster(text).expect("installed Apple color emoji");
                assert_eq!(image.size, [64, 64]);
                assert!(image.pixels.iter().any(|p| p.a() > 0 && p.r() != p.b()));
            }
            assert!(raster("").is_none());
            assert!(raster(&"👍".repeat(33)).is_none());
            assert!(raster("plain text").is_none());
        })
        .join()
        .unwrap();
    }
}
