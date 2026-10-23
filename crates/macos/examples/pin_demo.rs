//! Manual dev aid (needs a display and a human):
//! `cargo run -p ferrocast-macos --example pin_demo -- path/to.png [scale]`
//! pins the image at pixel size / scale points (scale defaults to 2). Drag it, scroll or pinch to
//! zoom, +/-/0 for keyboard zoom, Esc closes it; the demo exits when every pin is closed.

use ferrocast_macos::pin;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSEventMask};
use objc2_foundation::{NSDate, NSDefaultRunLoopMode};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = std::path::PathBuf::from(args.next().expect("usage: pin_demo <png> [scale]"));
    let scale: f64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(2.0);
    let size = image_size(&path).expect("could not read the PNG header");
    let mtm = MainThreadMarker::new().expect("main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);
    println!("{:?}", pin::open(mtm, &path, size, scale));
    while pin::count() > 0 {
        let until = NSDate::distantFuture();
        if let Some(ev) =
            unsafe { app.nextEventMatchingMask_untilDate_inMode_dequeue(NSEventMask::Any, Some(&until), NSDefaultRunLoopMode, true) }
        {
            app.sendEvent(&ev);
        }
    }
}

/// Width and height from the PNG IHDR chunk.
fn image_size(path: &std::path::Path) -> Option<(u32, u32)> {
    let b = std::fs::read(path).ok()?;
    let be = |i: usize| Some(u32::from_be_bytes(b.get(i..i + 4)?.try_into().ok()?));
    (b.get(1..4)? == b"PNG").then_some(())?;
    Some((be(16)?, be(20)?))
}
