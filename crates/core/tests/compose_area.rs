//! Record Area / Record Window: `events.json` coordinates are display points, but `screen.mov`
//! covers only `screen.capture_rect`. `fixtures/area_events.json` + `fixtures/area_project.json`
//! are hand-written (not produced by Swift) with numbers that can be checked by hand.
//!
//! Display 1000 x 500 pt at 2x; recorded area = lower-right quarter `[500, 250, 500, 250]`,
//! so `screen.mov` is 1000 x 500 px. A display point (x, y) lands at
//! `((x - 500) / 500, (y - 250) / 250)` of the video:
//!   cursor (750, 375)      -> (0.5, 0.5)   the middle of the video
//!   click  (625, 312.5)    -> (0.25, 0.25) ((625-500)/500, (312.5-250)/250)
//!   cursor/click (100,100) -> (-0.8, -0.6) outside the video: cursor and ring are hidden
//! The zoom (t 0.5..2.0, scale 2, focus (0.5, 0.5)) keeps the cursor at its centre until the
//! pointer leaves at t = 1.0..1.1; the camera then follows the cursor clamped to the edge.
//! Without `capture_rect` the same cursor would read (0.75, 0.75) and the ring (0.625, 0.625).

use ferrocast_core::capture::{compose::frame_params, events::Events, project::Project, timeline};

const PROJECT: &str = include_str!("../fixtures/area_project.json");
const EVENTS: &str = include_str!("../fixtures/area_events.json");

fn load() -> (Project, Events) {
    (serde_json::from_str(PROJECT).expect("project"), Events::decode(EVENTS).expect("events"))
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn fixtures_decode_and_validate() {
    let (p, e) = load();
    assert_eq!(e.screen.capture_rect, Some([500.0, 250.0, 500.0, 250.0]));
    assert!(e.validate().is_empty(), "{:?}", e.validate());
    assert!(timeline::validate(&p).is_empty());
}

#[test]
fn the_cursor_and_camera_use_the_recorded_rect() {
    let (p, e) = load();
    // t = 0.2: before the zoom, cursor in the middle of the video, no camera
    let f = frame_params(&p, &e, 0.2);
    let c = f.cursor.expect("cursor");
    assert!(near(c.x, 0.5) && near(c.y, 0.5), "{c:?}");
    assert!(f.camera.is_none());
    // t = 0.9 (zoom held): camera 0.5 x 0.5 centred on (0.5, 0.5) -> x = y = 0.25
    let f = frame_params(&p, &e, 0.9);
    let cam = f.camera.expect("zoomed");
    assert!((cam.w - 0.5).abs() < 1e-3 && (cam.x - 0.25).abs() < 1e-3 && (cam.y - 0.25).abs() < 1e-3, "{cam:?}");
}

#[test]
fn the_ring_is_at_the_click_inside_the_rect_and_absent_outside() {
    let (p, e) = load();
    let r = frame_params(&p, &e, 0.9).ring.expect("ring for the click at 0.8");
    assert!(near(r.x, 0.25) && near(r.y, 0.25), "{r:?}");
    assert!(r.radius < 36.0 / 1000.0 + 1e-9, "fraction of the 1000 px video: {r:?}");
    // click at (100, 100) at t = 2.2 is outside the area
    assert_eq!(frame_params(&p, &e, 2.3).ring, None);
}

#[test]
fn the_cursor_is_hidden_outside_the_rect() {
    let (p, e) = load();
    assert!(frame_params(&p, &e, 0.5).cursor.is_some());
    assert_eq!(frame_params(&p, &e, 1.5).cursor, None);
    assert_eq!(frame_params(&p, &e, 2.5).cursor, None);
}
