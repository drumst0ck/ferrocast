//! `fixtures/p6_project.json`: a project that uses every P6 field (loupes, split layout, look,
//! cut-out, face following). The golden S19 tests live in `p6_golden.rs`.

use ferrocast_core::capture::{
    camera::{self, CameraPlacement, SplitSide},
    lut::{self, Lut3D},
    project::Project,
    timeline::{self, EditKind, History},
    zoom,
};

const FIXTURE: &str = include_str!("../fixtures/p6_project.json");

/// A hand-written 2x2x2 `.cube` (a warm tint), the format a user would drop into a project.
const CUBE: &str = "# warm tint\r\nTITLE \"tint\"\r\nLUT_3D_SIZE 2\r\n\
0 0 0\r\n1 0 0\r\n0 1 0\r\n1 1 0\r\n0 0 0.8\r\n1 0 0.8\r\n0 1 0.8\r\n1 1 0.8\r\n";

fn project() -> Project {
    Project::from_json(FIXTURE).expect("decode")
}

#[test]
fn p6_fixture_decodes_validates_and_uses_every_field() {
    let p = project();
    assert!(timeline::validate(&p).is_empty(), "{:?}", timeline::validate(&p));
    assert!(p.validate(|_| true).is_empty(), "{:?}", p.validate(|_| true));
    assert_eq!(p.loupes.len(), 3);
    assert_eq!((p.loupes[0].x, p.loupes[0].y), (Some(0.3), Some(0.6)));
    assert_eq!((p.loupes[1].x, p.loupes[2].scale, p.loupes[2].radius), (None, 2.0, 0.15));
    let c = p.camera.as_ref().unwrap();
    assert_eq!((c.layout, c.split.side, c.split.ratio, c.split.gap), (CameraPlacement::Split, SplitSide::Left, 0.3, 0.02));
    let look = c.look.as_ref().unwrap();
    assert_eq!((look.name.as_deref(), look.file.as_deref(), look.intensity), (Some("cinematic"), None, 0.75));
    assert!(c.cutout && c.follow_face && c.follow_zoom == Some(1.8));
    assert!(lut::bundled_look(look.name.as_deref().unwrap()).is_some());
}

#[test]
fn p6_fixture_round_trips_and_keeps_unknown_loupe_fields() {
    let p = project();
    let out = p.to_json();
    assert_eq!(Project::from_json(&out).unwrap(), p);
    assert_eq!(Project::from_json(&out).unwrap().to_json(), out);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["loupes"][1]["future_field"], serde_json::json!({"keep": true}));
    assert_eq!(v["camera"]["layout"], "split");
    assert_eq!(v["camera"]["split"], serde_json::json!({"side":"left","ratio":0.3,"gap":0.02}));
    assert_eq!(v["camera"]["look"], serde_json::json!({"name":"cinematic","intensity":0.75}));
    assert_eq!(
        v["loupes"][2],
        serde_json::json!({"id":"l3","start":16.0,"end":19.0,"scale":2.0,"radius":0.15,"ease_in":0.25,"ease_out":0.25})
    );
}

#[test]
fn p6_fixture_maps_loupes_through_cuts_and_speed_and_splits_the_canvas() {
    let p = project();
    // kept 0..8 and 12..20 (2x over 14..18): l2 5..10 loses 8..10 to the cut; l3 16..19 is sped up
    let spans = zoom::loupe_output_spans(&p);
    let got: Vec<(&str, f64, f64)> = spans.iter().map(|s| (s.id.as_str(), s.out_start, s.out_end)).collect();
    assert_eq!(got, [("l1", 1.0, 4.0), ("l2", 5.0, 8.0), ("l3", 11.0, 13.0)]);
    let (screen, cam) = camera::split_rects(1920.0, 1080.0, p.camera.as_ref().unwrap(), 16.0 / 9.0);
    assert_eq!((cam.x, cam.w, cam.h), (0.0, 576.0, 1080.0));
    assert!(screen.x >= cam.x + cam.w);
}

#[test]
fn an_old_project_gains_p6_effects_through_undoable_edits_and_loses_them_on_undo() {
    let mut p = Project::from_json(include_str!("../fixtures/camera_project.json")).unwrap();
    let before = p.to_json();
    let mut h = History::new(20, 0.5);
    let mut id = String::new();
    assert!(h.edit(&mut p, EditKind::Loupe, 1.0, |p| id = zoom::add_loupe(p, 2.0).unwrap()));
    assert!(p.to_json().contains("loupes"));
    assert!(p.validate(|_| true).is_empty(), "{:?}", p.validate(|_| true));
    assert!(h.undo(&mut p));
    assert_eq!(p.to_json(), before, "undo restores the exact pre-P6 bytes");
    assert!(!id.is_empty());
}

#[test]
fn a_custom_cube_parses_and_applies() {
    let l = lut::parse_cube(CUBE).unwrap();
    assert_eq!((l.size, l.title.as_deref()), (2, Some("tint")));
    let o = l.sample([1.0, 1.0, 1.0]);
    assert_eq!(o, [1.0, 1.0, 0.8]);
    assert_eq!(l.with_intensity(0.0).data, Lut3D::identity(2).data);
}
