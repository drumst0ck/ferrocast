//! P6 render side (T38-T42): the per-frame numbers the helper draws. Pure Rust; the Swift side
//! only draws what these tests pin down.

use ferrocast_core::capture::{
    camera::{self, CameraPlacement, SplitSide},
    compose::{CameraFx, Composer, FrameParams},
    events::{CursorSample, Events, ScreenInfo},
    facetrack::{FaceTrack, ReframeOpts, Reframer},
    lut,
    project::{Loupe, Project, Source},
    zoom,
};
use std::path::{Path, PathBuf};

/// 2000 x 1000 px screen on a 1000 x 500 pt display.
fn events() -> Events {
    let mut e = Events::new(ScreenInfo { size: [2000, 1000], scale: 2.0, display_frame: [0.0, 0.0, 1000.0, 500.0], capture_rect: None });
    e.cursor = (0..=300).map(|i| CursorSample(i as f64 / 60.0, 100.0 + i as f64, 200.0, "arrow".into())).collect();
    e
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("p6render-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn project() -> Project {
    let mut p = Project::new("t", "2026-01-01T00:00:00Z");
    p.duration = 5.0;
    p.sources.camera = Some(Source { file: "camera.mov".into(), offset: 0.0 });
    p.camera = Some(camera::CameraLayout::default());
    p
}

fn loupe(start: f64, end: f64) -> Loupe {
    Loupe {
        id: "l1".into(),
        start,
        end,
        scale: 2.5,
        radius: 0.12,
        x: None,
        y: None,
        ease_in: 0.25,
        ease_out: 0.25,
        extra: Default::default(),
    }
}

fn with_camera_file(dir: &Path) {
    std::fs::write(dir.join("camera.mov"), b"x").unwrap();
}

#[test]
fn loupe_params_use_loupe_at_with_cursor_and_are_omitted_otherwise() {
    let mut p = project();
    p.loupes = vec![loupe(1.0, 3.0)];
    let e = events();
    let c = Composer::new(&p, &e);
    assert!(c.at(0.5).loupe.is_none());
    assert!(c.at(3.5).loupe.is_none());
    let f = c.at(2.0);
    let l = f.loupe.expect("active at 2.0");
    let want = zoom::loupe_at_with_cursor(&p, 2.0, Some([(100.0 + 120.0) / 1000.0, 200.0 / 500.0])).unwrap();
    assert!((l.cx - want.cx).abs() < 0.05 && (l.cy - want.cy).abs() < 0.05, "{l:?} vs {want:?}");
    assert_eq!((l.radius, l.scale, l.alpha), (0.12, 2.5, 1.0));
    let json = serde_json::to_value(&f).unwrap();
    assert_eq!(json["loupe"].as_object().unwrap().keys().cloned().collect::<Vec<_>>(), ["alpha", "cx", "cy", "radius", "scale"]);
    // a fixed focus wins over the cursor and a fading loupe reports its alpha
    p.loupes[0].x = Some(0.25);
    p.loupes[0].y = Some(0.75);
    let l = Composer::new(&p, &e).at(1.1).loupe.unwrap();
    assert_eq!((l.cx, l.cy), (0.25, 0.75));
    assert!(l.alpha > 0.0 && l.alpha < 1.0);
    // no loupe: not even the key
    let none = serde_json::to_string(&Composer::new(&project(), &e).at(2.0)).unwrap();
    assert!(!none.contains("loupe") && !none.contains("look") && !none.contains("split") && !none.contains("camera_crop"), "{none}");
}

#[test]
fn look_is_resolved_once_to_a_scratch_table_the_frames_only_point_at() {
    let dir = scratch("look");
    with_camera_file(&dir);
    let mut p = project();
    p.camera.as_mut().unwrap().look = Some(camera::Look { name: Some("cinematic".into()), file: None, intensity: 1.0 });
    let fx = CameraFx::prepare(&p, &dir, &events(), [1920, 1080]).unwrap();
    let look = Composer::new(&p, &events()).with_camera_fx(fx).at(1.0).look.expect("look param");
    assert_eq!(look.size, 17);
    let bytes = std::fs::read(&look.file).unwrap();
    assert_eq!(bytes.len(), 17 * 17 * 17 * 4 * 4, "RGBA f32 little endian");
    assert!(look.file.starts_with(dir.to_str().unwrap()) && look.file.contains(".cache"), "{}", look.file);
    let want = lut::bundled_look("cinematic").unwrap().to_rgba_f32();
    assert_eq!(f32::from_le_bytes(bytes[0..4].try_into().unwrap()), want[0]);
    assert_eq!(look.hash.len(), 64);
    // the same look again: same file and hash; another intensity: another hash
    let again = CameraFx::prepare(&p, &dir, &events(), [1920, 1080]).unwrap();
    let a = Composer::new(&p, &events()).with_camera_fx(again).at(2.0).look.unwrap();
    assert_eq!((a.file.as_str(), a.hash.as_str()), (look.file.as_str(), look.hash.as_str()));
    p.camera.as_mut().unwrap().look.as_mut().unwrap().intensity = 0.5;
    let half =
        Composer::new(&p, &events()).with_camera_fx(CameraFx::prepare(&p, &dir, &events(), [1920, 1080]).unwrap()).at(0.0).look.unwrap();
    assert_ne!(half.hash, look.hash);
    // intensity 0 is the identity: nothing to apply
    p.camera.as_mut().unwrap().look.as_mut().unwrap().intensity = 0.0;
    assert!(CameraFx::prepare(&p, &dir, &events(), [1920, 1080]).unwrap().is_empty());
}

#[test]
fn a_project_cube_file_is_parsed_and_bad_ones_are_a_clear_error() {
    let dir = scratch("cube");
    with_camera_file(&dir);
    let mut p = project();
    let look = |f: &str| camera::Look { name: None, file: Some(f.into()), intensity: 1.0 };
    let cube = "LUT_3D_SIZE 2\n0 0 0\n1 0 0\n0 1 0\n1 1 0\n0 0 1\n1 0 1\n0 1 1\n1 1 1\n";
    std::fs::write(dir.join("my.cube"), cube).unwrap();
    p.camera.as_mut().unwrap().look = Some(look("my.cube"));
    let fx = CameraFx::prepare(&p, &dir, &events(), [1920, 1080]).unwrap();
    let l = Composer::new(&p, &events()).with_camera_fx(fx).at(0.0).look.unwrap();
    assert_eq!(l.size, 2);
    // a domain other than 0..1 is resampled into a 0..1 table (CIColorCube cannot take a domain)
    std::fs::write(dir.join("dom.cube"), format!("DOMAIN_MAX 2 2 2\n{cube}")).unwrap();
    p.camera.as_mut().unwrap().look = Some(look("dom.cube"));
    let d =
        Composer::new(&p, &events()).with_camera_fx(CameraFx::prepare(&p, &dir, &events(), [1920, 1080]).unwrap()).at(0.0).look.unwrap();
    let bytes = std::fs::read(&d.file).unwrap();
    let last = f32::from_le_bytes(bytes[bytes.len() - 16..bytes.len() - 12].try_into().unwrap());
    assert!((last - 0.5).abs() < 1e-6, "input 1.0 maps to the middle of a 0..2 domain, got {last}");
    std::fs::write(dir.join("bad.cube"), "LUT_3D_SIZE 2\n0 0 0\n").unwrap();
    p.camera.as_mut().unwrap().look = Some(look("bad.cube"));
    let e = CameraFx::prepare(&p, &dir, &events(), [1920, 1080]).unwrap_err();
    assert!(e.contains("bad.cube"), "{e}");
    p.camera.as_mut().unwrap().look = Some(look("missing.cube"));
    assert!(CameraFx::prepare(&p, &dir, &events(), [1920, 1080]).unwrap_err().contains("missing.cube"));
}

#[test]
fn split_rects_come_from_camera_split_rects_for_every_side() {
    let dir = scratch("split");
    with_camera_file(&dir);
    for side in [SplitSide::Left, SplitSide::Right, SplitSide::Top, SplitSide::Bottom] {
        let mut p = project();
        let c = p.camera.as_mut().unwrap();
        c.layout = CameraPlacement::Split;
        c.split.side = side;
        let fx = CameraFx::prepare(&p, &dir, &events(), [1920, 1080]).unwrap();
        let s = Composer::new(&p, &events()).with_camera_fx(fx).at(1.0).split.expect("split");
        let (sr, cr) = camera::split_rects(1920.0, 1080.0, p.camera.as_ref().unwrap(), 2.0);
        assert_eq!([s.screen.x, s.screen.y, s.screen.w, s.screen.h], [sr.x, sr.y, sr.w, sr.h], "{side:?}");
        assert_eq!([s.camera.x, s.camera.y, s.camera.w, s.camera.h], [cr.x, cr.y, cr.w, cr.h], "{side:?}");
        assert_eq!(s.canvas, [1920, 1080]);
    }
    // overlay layout, hidden camera and a missing camera file never produce a split
    let mut p = project();
    assert!(CameraFx::prepare(&p, &dir, &events(), [1920, 1080]).unwrap().is_empty());
    p.camera.as_mut().unwrap().layout = CameraPlacement::Split;
    p.camera.as_mut().unwrap().visible = false;
    assert!(CameraFx::prepare(&p, &dir, &events(), [1920, 1080]).unwrap().is_empty());
    p.camera.as_mut().unwrap().visible = true;
    let empty = scratch("split-nofile");
    assert!(CameraFx::prepare(&p, &empty, &events(), [1920, 1080]).unwrap().is_empty());
}

fn track_file(dir: &Path, w: u32, h: u32) -> FaceTrack {
    let samples: Vec<String> = (0..40)
        .map(|i| format!(r#"{{"t":{},"x":{},"y":0.3,"w":0.1,"h":0.2,"confidence":1}}"#, i as f64 / 10.0, 0.1 + 0.02 * i as f64))
        .collect();
    let text = format!(r#"{{"version":1,"fps":10,"width":{w},"height":{h},"samples":[{}]}}"#, samples.join(","));
    std::fs::write(dir.join("camera_track.json"), &text).unwrap();
    FaceTrack::parse(&text).unwrap()
}

#[test]
fn follow_face_crops_in_camera_pixels_with_the_offset_and_default_zoom() {
    let dir = scratch("face");
    with_camera_file(&dir);
    let track = track_file(&dir, 1280, 720);
    let mut p = project();
    p.sources.camera = Some(Source { file: "camera.mov".into(), offset: 0.5 });
    p.camera.as_mut().unwrap().follow_face = true;
    let fx = CameraFx::prepare(&p, &dir, &events(), [1920, 1080]).unwrap();
    let ev = events();
    let comp = Composer::new(&p, &ev).with_camera_fx(fx);
    let reframer = Reframer::new(&track, 1280.0, 720.0, &ReframeOpts { zoom: 1.4, crop_aspect: 1.0, ..ReframeOpts::default() });
    let got = comp.at(2.0).camera_crop.expect("crop");
    let want = reframer.at(2.0 - 0.5);
    assert_eq!((got.x, got.y, got.w, got.h), (want.x, want.y, want.w, want.h), "screen time minus the camera offset");
    assert!((got.w - 720.0 / 1.4).abs() < 1e-9 && got.w == got.h, "square crop of the default zoom 1.4");
    // the crop moves with the face
    assert!(comp.at(3.5).camera_crop.unwrap().x > got.x + 10.0);
    // an explicit follow_zoom wins
    p.camera.as_mut().unwrap().follow_zoom = Some(2.0);
    let z = Composer::new(&p, &events())
        .with_camera_fx(CameraFx::prepare(&p, &dir, &events(), [1920, 1080]).unwrap())
        .at(2.0)
        .camera_crop
        .unwrap();
    assert!((z.w - 360.0).abs() < 1e-9);
}

#[test]
fn follow_face_in_a_split_crops_to_the_camera_part_aspect() {
    let dir = scratch("face-split");
    with_camera_file(&dir);
    track_file(&dir, 1280, 720);
    let mut p = project();
    let c = p.camera.as_mut().unwrap();
    c.follow_face = true;
    c.layout = CameraPlacement::Split;
    let fx = CameraFx::prepare(&p, &dir, &events(), [1920, 1080]).unwrap();
    let f = Composer::new(&p, &events()).with_camera_fx(fx).at(1.0);
    let (cam, crop) = (f.split.unwrap().camera, f.camera_crop.unwrap());
    assert!((crop.w / crop.h - cam.w / cam.h).abs() < 1e-9, "{crop:?} vs {cam:?}");
}

#[test]
fn without_a_track_file_or_follow_face_there_is_no_crop_and_no_error() {
    let dir = scratch("face-none");
    with_camera_file(&dir);
    let mut p = project();
    p.camera.as_mut().unwrap().follow_face = true;
    let fx = CameraFx::prepare(&p, &dir, &events(), [1920, 1080]).unwrap();
    assert!(Composer::new(&p, &events()).with_camera_fx(fx).at(1.0).camera_crop.is_none());
    // a corrupt track and one without the camera size also fall back to the uncropped camera
    std::fs::write(dir.join("camera_track.json"), "not json").unwrap();
    assert!(CameraFx::prepare(&p, &dir, &events(), [1920, 1080]).unwrap().is_empty());
    std::fs::write(dir.join("camera_track.json"), r#"{"fps":10,"samples":[{"t":0,"x":0.4,"y":0.4,"w":0.1,"h":0.1}]}"#).unwrap();
    assert!(CameraFx::prepare(&p, &dir, &events(), [1920, 1080]).unwrap().is_empty());
    // a track next to a project that does not ask for it is ignored
    track_file(&dir, 1280, 720);
    p.camera.as_mut().unwrap().follow_face = false;
    assert!(CameraFx::prepare(&p, &dir, &events(), [1920, 1080]).unwrap().is_empty());
}

#[test]
fn old_projects_keep_the_exact_old_params_json() {
    // S19: nothing new appears for a project that uses none of the P6 fields
    let p = Project::new("t", "2026-01-01T00:00:00Z");
    assert_eq!(
        serde_json::to_string(&Composer::new(&p, &events()).at(1.0)).unwrap(),
        serde_json::to_string(&FrameParams { cursor: Composer::new(&p, &events()).at(1.0).cursor, ..FrameParams::default() }).unwrap()
    );
}
