//! Manual dev aid (needs a display and a human): `cargo run -p ferrocast-macos --example selector_demo`
//! opens the capture selection overlay on every display and prints the result.
//! Drag for an area, Space for window mode (one fake window), Esc to cancel.

use ferrocast_core::capture::region::{Rect, Window};
use ferrocast_macos::selector::select;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    NSApplication::sharedApplication(mtm).setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    let windows = vec![Window { id: 1, frame: Rect::new(200.0, 200.0, 600.0, 400.0) }];
    println!("{:?}", select(mtm, windows));
}
