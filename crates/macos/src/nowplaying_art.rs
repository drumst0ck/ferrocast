//! Prepares the notch player's cover art off the main thread: the cover is decoded and aspect-filled
//! into a square, and `ferrocast_core::nowplaying_art` derives the vinyl disc, the blurred halo and
//! the tint from it. This runs once per cover, on the helper's reading thread, before the track
//! is announced; the main thread only looks the result up and draws it.

use std::ptr::null_mut;
use std::sync::{Arc, Mutex, OnceLock};

use ferrocast_core::nowplaying_art::{look_from_square, placeholder_look, ArtLook, DISC_PX};
use ferrocast_core::nowplaying_input::cover_crop;
use objc2::AnyThread;
use objc2_app_kit::{NSBitmapImageRep, NSCompositingOperation, NSDeviceRGBColorSpace, NSGraphicsContext, NSImage};
use objc2_foundation::{NSData, NSPoint, NSRect, NSSize};

/// Decode `bytes` and aspect-fill them into `side` x `side` RGBA (top row first).
fn aspect_fill_rgba(bytes: &[u8], side: usize) -> Option<Vec<u8>> {
    unsafe {
        let data = NSData::with_bytes(bytes);
        let img = NSImage::initWithData(NSImage::alloc(), &data)?;
        let size = img.size();
        let crop = cover_crop(size.width, size.height);
        if crop[2] <= 0.0 {
            return None;
        }
        let rep = NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            null_mut(),
            side as isize,
            side as isize,
            8,
            4,
            true,
            false,
            NSDeviceRGBColorSpace,
            0,
            0,
        )?;
        let ctx = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep)?;
        NSGraphicsContext::saveGraphicsState_class();
        NSGraphicsContext::setCurrentContext(Some(&ctx));
        let dest = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(side as f64, side as f64));
        let from = NSRect::new(NSPoint::new(crop[0], crop[1]), NSSize::new(crop[2], crop[3]));
        img.drawInRect_fromRect_operation_fraction(dest, from, NSCompositingOperation::SourceOver, 1.0);
        ctx.flushGraphics();
        NSGraphicsContext::restoreGraphicsState_class();
        let (ptr, bpr) = (rep.bitmapData(), rep.bytesPerRow() as usize);
        if ptr.is_null() || bpr < side * 4 {
            return None;
        }
        let mut out = Vec::with_capacity(side * side * 4);
        for row in 0..side {
            out.extend_from_slice(std::slice::from_raw_parts(ptr.add(row * bpr), side * 4));
        }
        Some(out)
    }
}

/// Everything derived from the cover `bytes`; `None` when they cannot be decoded.
pub fn prepare(bytes: &[u8]) -> Option<ArtLook> {
    // This runs on a plain worker thread with no run loop, so nothing would ever drain the
    // objects AppKit autoreleases while decoding: a pool per cover keeps memory flat.
    let px = objc2::rc::autoreleasepool(|_| aspect_fill_rgba(bytes, DISC_PX))?;
    Some(look_from_square(&px))
}

/// The look of the last cover prepared, keyed by the cover's own `Arc` (the tracker hands out one
/// per cover).
type Cache = Option<(Arc<Vec<u8>>, Arc<ArtLook>)>;
static CACHE: Mutex<Cache> = Mutex::new(None);

/// Prepare the look of `art` unless it is the one already cached. Call it off the main thread,
/// before the snapshot that carries `art` is announced.
pub fn ensure(art: &Arc<Vec<u8>>) {
    if let Ok(g) = CACHE.lock() {
        if g.as_ref().is_some_and(|(k, _)| Arc::ptr_eq(k, art)) {
            return;
        }
    }
    let look = prepare(art).map_or_else(placeholder, Arc::new);
    if let Ok(mut g) = CACHE.lock() {
        *g = Some((art.clone(), look));
    }
}

fn placeholder() -> Arc<ArtLook> {
    static P: OnceLock<Arc<ArtLook>> = OnceLock::new();
    P.get_or_init(|| Arc::new(placeholder_look())).clone()
}

/// The look to draw for `art` (`None`: no cover): the prepared one, or the plain record.
pub fn lookup(art: Option<&Arc<Vec<u8>>>) -> Arc<ArtLook> {
    if let (Some(art), Ok(g)) = (art, CACHE.lock()) {
        if let Some((k, look)) = g.as_ref() {
            if Arc::ptr_eq(k, art) {
                return look.clone();
            }
        }
    }
    placeholder()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A solid-colour PNG made by AppKit itself, wide, to prove the decode and the aspect fill.
    fn wide_png() -> Vec<u8> {
        use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep};
        use objc2_foundation::NSDictionary;
        unsafe {
            let (w, h) = (40isize, 20isize);
            let rep = NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
                NSBitmapImageRep::alloc(),
                null_mut(),
                w,
                h,
                8,
                4,
                true,
                false,
                NSDeviceRGBColorSpace,
                0,
                0,
            )
            .unwrap();
            let p = rep.bitmapData();
            let bpr = rep.bytesPerRow() as usize;
            for y in 0..h as usize {
                for x in 0..w as usize {
                    // left half blue, right half red: a square crop keeps half of each
                    let c: [u8; 4] = if x < 20 { [0, 0, 255, 255] } else { [255, 0, 0, 255] };
                    std::ptr::copy_nonoverlapping(c.as_ptr(), p.add(y * bpr + x * 4), 4);
                }
            }
            let png = rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new()).unwrap();
            png.to_vec()
        }
    }

    #[test]
    fn a_cover_is_decoded_aspect_filled_and_turned_into_a_disc() {
        let px = aspect_fill_rgba(&wide_png(), 32).expect("decodes");
        assert_eq!(px.len(), 32 * 32 * 4);
        // a 40x20 image cropped to its centre 20x20: 10 columns of blue, then 10 of red
        let at = |x: usize, y: usize| &px[(y * 32 + x) * 4..(y * 32 + x) * 4 + 3];
        assert!(at(2, 16)[2] > 200 && at(2, 16)[0] < 60, "left is blue: {:?}", at(2, 16));
        assert!(at(29, 16)[0] > 200 && at(29, 16)[2] < 60, "right is red: {:?}", at(29, 16));
        let look = prepare(&wide_png()).expect("a look");
        assert_eq!(look.disc.len(), DISC_PX * DISC_PX * 4);
    }

    fn rss_kb() -> usize {
        let out = std::process::Command::new("ps").args(["-o", "rss=", "-p", &std::process::id().to_string()]).output().unwrap();
        String::from_utf8_lossy(&out.stdout).trim().parse().unwrap()
    }

    #[test]
    #[ignore = "measures process RSS; run alone: cargo test -p ferrocast-macos decoding_many_covers -- --ignored"]
    fn decoding_many_covers_on_a_worker_thread_does_not_grow_memory() {
        let png = wide_png();
        std::thread::spawn(move || {
            for _ in 0..20 {
                prepare(&png); // warm up caches
            }
            let before = rss_kb();
            for _ in 0..100 {
                prepare(&png);
            }
            let grew = rss_kb().saturating_sub(before);
            assert!(grew < 12_000, "RSS grew {grew} KB over 100 covers: autoreleased objects are piling up");
        })
        .join()
        .unwrap();
    }

    #[test]
    fn garbage_is_not_a_cover_and_the_lookup_falls_back_to_the_plain_record() {
        assert!(prepare(&[1, 2, 3, 4]).is_none());
        let junk = Arc::new(vec![1u8, 2, 3, 4]);
        ensure(&junk);
        let look = lookup(Some(&junk));
        assert_eq!(look.tint, ferrocast_core::nowplaying_art::ACCENT);
        assert!(Arc::ptr_eq(&look, &lookup(None)), "no cover: the same plain record");
    }

    #[test]
    fn the_look_is_computed_once_per_cover() {
        let a = Arc::new(wide_png());
        ensure(&a);
        let first = lookup(Some(&a));
        ensure(&a);
        assert!(Arc::ptr_eq(&first, &lookup(Some(&a))), "cached, not recomputed");
        let b = Arc::new(wide_png());
        assert!(!Arc::ptr_eq(&first, &lookup(Some(&b))), "another cover is not the cached one");
    }
}
