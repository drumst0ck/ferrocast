//! Dev aid: `cargo run -p ferrocast-macos --example glow_demo` shows the translating glow
//! (a rect outline for 4 s, then the "Translating…" pill for 4 s, then fades out and exits).

use std::time::{Duration, Instant};

use ferrocast_macos::glow::{hide, show, Target};
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use objc2_foundation::{NSDate, NSRunLoop};

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    let start = Instant::now();
    let (mut a, mut b, mut c) = (false, false, false);
    while start.elapsed() < Duration::from_millis(8800) {
        let t = start.elapsed();
        if !a {
            a = true;
            show(mtm, Target::Rects(vec![[200.0, 200.0, 400.0, 60.0]]), false);
        } else if !b && t >= Duration::from_secs(4) {
            b = true;
            show(mtm, Target::Point([800.0, 400.0]), false);
        } else if !c && t >= Duration::from_secs(8) {
            c = true;
            hide(mtm);
        }
        NSRunLoop::currentRunLoop().runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(0.016));
    }
}
