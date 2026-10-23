//! T25 render fixture: `render_zoom_project.json` (a zoom at 0.5..1.6 s) plus `render_events.json`
//! (written by the Swift `EventLog` through `target/capture-spike/render-events-spike/build.sh`)
//! are what the capture smoke script renders. `render_zoom_params.json` holds the `render.frame`
//! fields `capture::compose` evaluates for four source times; the smoke script feeds them to the
//! helper, so the maths stays in Rust. This test fails when the file drifts from the code; run it
//! with `UPDATE_COMPOSE_FIXTURE=1` to rewrite the file after an intended change.

use ferrocast_core::capture::{compose::frame_params, events::Events, project::Project, timeline};
use serde_json::{json, Value};

const PROJECT: &str = include_str!("../fixtures/render_zoom_project.json");
const EVENTS: &str = include_str!("../fixtures/render_events.json");
const PARAMS_FILE: &str = "fixtures/render_zoom_params.json";
const PARAMS: &str = include_str!("../fixtures/render_zoom_params.json");

/// Before the zoom, mid-move at full zoom (blur), at full zoom just after the click (ring), after zoom-out.
const TIMES: [f64; 4] = [0.2, 0.85, 1.2, 2.5];

fn load() -> (Project, Events) {
    (serde_json::from_str(PROJECT).expect("project"), Events::decode(EVENTS).expect("events"))
}

fn computed() -> Value {
    let (p, e) = load();
    Value::Array(TIMES.iter().map(|&t| json!({"t": t, "params": frame_params(&p, &e, t)})).collect())
}

fn close(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => (x.as_f64().unwrap() - y.as_f64().unwrap()).abs() < 1e-9,
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(a, b)| close(a, b)),
        (Value::Object(x), Value::Object(y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| close(v, w))),
        _ => a == b,
    }
}

#[test]
fn fixtures_decode_and_validate() {
    let (p, e) = load();
    assert!(timeline::validate(&p).is_empty(), "{:?}", timeline::validate(&p));
    assert!(p.validate(|f| f == "screen.mov").is_empty(), "{:?}", p.validate(|f| f == "screen.mov"));
    assert!(e.validate().is_empty(), "{:?}", e.validate());
    assert_eq!((e.screen.size, e.screen.display_frame), ([320, 200], [0.0, 0.0, 320.0, 200.0]));
    assert!(p.loupes.is_empty() && p.extra.is_empty());
}

#[test]
fn the_four_frames_have_the_expected_shape() {
    let (p, e) = load();
    let f = |t| frame_params(&p, &e, t);
    // before the zoom: whole display, resting arrow, nothing else
    let a = f(0.2);
    assert!(a.camera.is_none() && a.ring.is_none() && a.blur.is_none());
    let c = a.cursor.unwrap();
    assert_eq!((c.kind.as_str(), c.x, c.y), ("arrow", 60.0 / 320.0, 0.25));
    // mid move: zoomed, blurred
    let b = f(0.85);
    assert!(b.camera.is_some_and(|c| c.w < 0.6) && b.blur.is_some() && b.ring.is_none(), "{b:?}");
    // after the click: zoomed, ring at the raw click, hand
    let r = f(1.2);
    let ring = r.ring.expect("ring");
    assert_eq!((ring.x, ring.y), (200.0 / 320.0, 110.0 / 200.0));
    assert_eq!(r.cursor.unwrap().kind, "hand");
    assert!(r.camera.is_some_and(|c| (c.w - 0.5).abs() < 1e-3) && r.blur.is_none());
    // after the zoom-out (1.6 + 0.3 s): back to the whole display
    let z = f(2.5);
    assert!(z.camera.is_none() && z.ring.is_none() && z.blur.is_none() && z.cursor.is_some());
}

#[test]
fn the_params_file_matches_the_code() {
    if std::env::var_os("UPDATE_COMPOSE_FIXTURE").is_some() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(PARAMS_FILE);
        std::fs::write(path, serde_json::to_string_pretty(&computed()).unwrap() + "\n").unwrap();
        return;
    }
    let want: Value = serde_json::from_str(PARAMS).expect("params file");
    assert!(close(&computed(), &want), "{PARAMS_FILE} drifted from compose::frame_params; rerun with UPDATE_COMPOSE_FIXTURE=1");
}
