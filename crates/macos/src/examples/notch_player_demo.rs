//! Look at the notch player without a music app:
//! `cargo run -p ferrocast-macos --example notch_player_demo [-- --shots <dir>]`
//! Shows the collapsed pill with a fake track, opens it as if hovered, and closes it again,
//! pumping the run loop so the spring and the timer really run. With `--shots` it also saves a
//! `screencapture` of each step (needs Screen Recording for the terminal; the demo sets
//! `FERROCAST_NOTCH_CAPTURABLE`, honoured by debug builds only, because the real panel asks
//! macOS to stay out of captures). Exit code 0 when
//! nothing crashed; judging the look is for a human.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use ferrocast_core::nowplaying::{Model, PlayState, Snapshot};
use ferrocast_macos::notch_player::{self, Callbacks};
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use objc2_foundation::{NSDate, NSRunLoop};

fn pump(ms: u64) {
    NSRunLoop::currentRunLoop().runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(ms as f64 / 1000.0));
}

/// A cover like the artboard's: a wide picture (so the aspect fill has something to crop).
#[allow(deprecated)] // lockFocus: a demo helper
fn cover() -> Vec<u8> {
    use objc2::AnyThread;
    use objc2_app_kit::{NSBezierPath, NSBitmapImageFileType, NSBitmapImageRep, NSColor, NSGradient, NSImage};
    use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize};
    let img = NSImage::initWithSize(NSImage::alloc(), NSSize::new(320.0, 180.0));
    img.lockFocus();
    let rgb = |r: f64, g: f64, b: f64| NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, 1.0);
    if let Some(g) = NSGradient::initWithStartingColor_endingColor(NSGradient::alloc(), &rgb(0.16, 0.04, 0.06), &rgb(0.94, 0.33, 0.37)) {
        g.drawInRect_angle(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(320.0, 180.0)), 45.0);
    }
    rgb(1.0, 0.7, 0.42).setFill();
    NSBezierPath::bezierPathWithOvalInRect(NSRect::new(NSPoint::new(94.0, 24.0), NSSize::new(132.0, 132.0))).fill();
    rgb(0.1, 0.04, 0.03).setStroke();
    let cross = NSBezierPath::bezierPath();
    cross.moveToPoint(NSPoint::new(94.0, 90.0));
    cross.lineToPoint(NSPoint::new(226.0, 90.0));
    cross.moveToPoint(NSPoint::new(160.0, 24.0));
    cross.lineToPoint(NSPoint::new(160.0, 156.0));
    cross.setLineWidth(4.0);
    cross.stroke();
    img.unlockFocus();
    let tiff = img.TIFFRepresentation().expect("tiff");
    let rep = NSBitmapImageRep::imageRepWithData(&tiff).expect("rep");
    let png = unsafe { rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new()) }.expect("png");
    png.to_vec()
}

fn model(expanded: bool, state: PlayState, art: &Arc<Vec<u8>>, long: bool) -> Model {
    Model {
        visible: true,
        expanded,
        sampled_at: ferrocast_core::nowplaying::mono_secs(),
        snapshot: Some(Snapshot {
            bundle_id: "com.brave.Browser".into(),
            state,
            title: if long { "Rust sin miedo: la serie completa de episodios".into() } else { "ballin de verdad".into() },
            artist: if long { "Código Fuente · Ep. 87".into() } else { "Zell".into() },
            album: String::new(),
            duration: 192.0,
            position: 74.0,
            artwork: Some(art.clone()),
        }),
    }
}

fn main() {
    if std::env::args().any(|a| a == "--shots") {
        std::env::set_var("FERROCAST_NOTCH_CAPTURABLE", "1");
    }
    let mtm = MainThreadMarker::new().expect("main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    let shots = std::env::args().skip_while(|a| a != "--shots").nth(1);
    let hovers = Rc::new(Cell::new(0));
    let h = hovers.clone();
    notch_player::set_callbacks(Callbacks {
        on_hover: Box::new(move |_| h.set(h.get() + 1)),
        on_command: Box::new(|c| println!("command {c:?}")),
        on_seek: Box::new(|s| println!("seek {s}")),
        on_open: Box::new(|| println!("open source")),
    });
    let reduce = std::env::args().any(|a| a == "--reduce-motion");
    let snap = |name: &str| {
        if let Some(dir) = &shots {
            let _ = std::fs::create_dir_all(dir);
            let ok = notch_player::save_png(&format!("{dir}/{name}.png"));
            println!("shot {name}: {}", if ok { "saved" } else { "FAILED" });
        }
    };
    let art = Arc::new(cover());
    // the real feed prepares the look off the main thread before announcing the track
    ferrocast_macos::nowplaying_art::ensure(&art);
    notch_player::render(mtm, &model(false, PlayState::Playing, &art, false), reduce);
    pump(900);
    snap("1-collapsed-playing");
    if let Some(ms) = notch_player::draw_cost_ms(60) {
        println!("draw cost, collapsed: {ms:.2} ms per frame");
    }
    notch_player::render(mtm, &model(true, PlayState::Playing, &art, false), reduce);
    pump(250);
    snap("2a-opening-250ms");
    pump(1100);
    snap("2-expanded-playing");
    if let Some(ms) = notch_player::draw_cost_ms(60) {
        println!("draw cost, expanded: {ms:.2} ms per frame (budget at 120 Hz: 8.33 ms)");
    }
    notch_player::render(mtm, &model(true, PlayState::Paused, &art, false), reduce);
    pump(600);
    snap("3-expanded-paused");
    notch_player::render(mtm, &model(true, PlayState::Playing, &art, true), reduce);
    pump(1300);
    snap("5-expanded-long-title");
    notch_player::render(mtm, &model(false, PlayState::Paused, &art, true), reduce);
    pump(1500);
    snap("4-collapsed-paused");
    notch_player::hide(mtm);
    pump(100);
    println!("ok: shown, expanded, paused, collapsed, hidden (hover events seen: {})", hovers.get());
}
