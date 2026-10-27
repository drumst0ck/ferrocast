//! The system screen colour picker (NSColorSampler): a magnifier the user moves over the screen;
//! clicking picks the colour under it and Escape cancels.

use block2::RcBlock;
use objc2_app_kit::{NSColor, NSColorSampler, NSColorSpace};

/// Show the sampler. `done` runs on the main thread with the picked colour as sRGB components
/// `[r, g, b, a]` in 0-1, or `None` when the user cancels. Main thread only.
pub fn sample(done: impl Fn(Option<[f64; 4]>) + 'static) {
    let handler = RcBlock::new(move |color: *mut NSColor| {
        // SAFETY: AppKit passes either nil or a valid NSColor for the duration of the call.
        let picked = unsafe { color.as_ref() }.and_then(|c| {
            let srgb = c.colorUsingColorSpace(&NSColorSpace::sRGBColorSpace())?;
            Some([srgb.redComponent(), srgb.greenComponent(), srgb.blueComponent(), srgb.alphaComponent()])
        });
        done(picked);
    });
    let sampler = NSColorSampler::new();
    unsafe { sampler.showSamplerWithSelectionHandler(&handler) };
}
