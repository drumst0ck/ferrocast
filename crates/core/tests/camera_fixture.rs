//! `fixtures/camera_project.json`: a P4 project (camera overlay + mic/system/camera takes).

use ferrocast_core::capture::{
    camera::{self, Corner},
    project::{AudioTrack, Project},
    timeline,
};

const FIXTURE: &str = include_str!("../fixtures/camera_project.json");

#[test]
fn camera_fixture_decodes_and_maps_audio() {
    let p: Project = serde_json::from_str(FIXTURE).expect("decode");
    assert!(timeline::validate(&p).is_empty(), "{:?}", timeline::validate(&p));
    assert!(p.validate(|_| true).is_empty());
    let cam = p.camera.unwrap();
    assert_eq!(cam.corner, Corner::TopRight);
    let r = camera::rect(1920.0, 1080.0, &cam);
    assert_eq!((r.w, r.x, r.y), (480.0, 1920.0 - 480.0 - 58.0, 58.0));
    // trim 1..9, 2x over 4..6; mic starts 0.2 s after the screen: file time = screen time - 0.2
    let m = timeline::audio_segments(&p, AudioTrack::Mic);
    assert_eq!(m.len(), 3);
    assert!((m[0].src_start - 0.8).abs() < 1e-12 && m[0].out_start == 0.0 && m[0].speed == 1.0);
    assert_eq!(m[1].speed, 2.0);
    assert!((m.last().unwrap().out_end() - timeline::Timeline::new(&p).output_duration()).abs() < 1e-9);
}

#[test]
fn camera_fixture_round_trip_is_stable() {
    let p: Project = serde_json::from_str(FIXTURE).unwrap();
    let a = p.to_json();
    assert_eq!(Project::from_json(&a).unwrap().to_json(), a);
}
