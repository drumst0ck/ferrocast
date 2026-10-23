//! `fixtures/render_project.json` is the project the capture smoke script renders with the
//! helper's `render.frame` (T24). It must stay a valid, P6-free EDL for the Rust side.

use ferrocast_core::capture::{project::Project, timeline};

const FIXTURE: &str = include_str!("../fixtures/render_project.json");

#[test]
fn render_fixture_decodes_and_validates() {
    let p: Project = serde_json::from_str(FIXTURE).expect("decode");
    assert!(timeline::validate(&p).is_empty(), "{:?}", timeline::validate(&p));
    assert!(p.validate(|f| f == "screen.mov").is_empty(), "{:?}", p.validate(|f| f == "screen.mov"));
    assert_eq!(p.sources.screen.file, "screen.mov");
    assert_eq!(p.style.aspect, "16:9");
    assert_eq!(p.style.background.kind, "gradient");
    assert!(p.loupes.is_empty() && p.camera.is_none() && p.transcript.is_none() && p.extra.is_empty());
}

#[test]
fn render_fixture_survives_a_save_round_trip_without_p6_fields() {
    let p: Project = serde_json::from_str(FIXTURE).unwrap();
    let s = serde_json::to_string(&p).unwrap();
    assert!(!s.contains("loupes") && !s.contains("camera") && !s.contains("transcript"), "{s}");
    assert_eq!(serde_json::from_str::<Project>(&s).unwrap(), p);
}
