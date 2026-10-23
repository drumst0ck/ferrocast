//! Manual dev aid (needs a display and a human, never run headless):
//! `cargo run -p ferrocast-macos --example hud_demo`
//! shows the `3…2…1` countdown, then the recording HUD. Try Pause/Resume, drag the HUD, and click
//! Stop (or wait 60 s) to exit. Take a screenshot or screen recording meanwhile: neither panel
//! should appear in it. Prints the window ids a caller would pass as `excludes.window_ids`.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Instant;

use ferrocast_macos::hud;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSEventMask};
use objc2_foundation::{NSDate, NSDefaultRunLoopMode};

fn pump(app: &NSApplication, seconds: f64) {
    let until = NSDate::dateWithTimeIntervalSinceNow(seconds);
    while let Some(ev) =
        unsafe { app.nextEventMatchingMask_untilDate_inMode_dequeue(NSEventMask::Any, Some(&until), NSDefaultRunLoopMode, true) }
    {
        app.sendEvent(&ev);
    }
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let mut countdown = hud::Countdown::new(3);
    while let Some(n) = countdown.current() {
        hud::countdown_show(mtm, None, n);
        println!("countdown {n}, excluded ids: {:?}", hud::excluded_window_ids());
        pump(&app, 1.0);
        countdown.tick();
    }
    hud::countdown_hide(mtm);

    let paused = Rc::new(Cell::new(false));
    let stopped = Rc::new(Cell::new(false));
    let (p, s) = (paused.clone(), stopped.clone());
    hud::hud_show(mtm, None, Box::new(move || p.set(!p.get())), Box::new(move || s.set(true)));
    println!("hud shown, excluded ids: {:?}", hud::excluded_window_ids());

    let (mut elapsed, mut last) = (0.0_f64, Instant::now());
    while !stopped.get() && elapsed < 60.0 {
        pump(&app, 0.1);
        let now = Instant::now();
        if !paused.get() {
            elapsed += now.duration_since(last).as_secs_f64();
        }
        last = now;
        hud::hud_update(elapsed, paused.get());
    }
    hud::hud_hide(mtm);
    pump(&app, 0.2);
}
