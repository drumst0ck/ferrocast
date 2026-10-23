//! S19 guarantee: P6 effects are optional EDL fields. A project that does not use them
//! serialises exactly as it did before P6 (the literals below were captured from the
//! pre-P6 serializer), and loading + saving an old project changes nothing.

use ferrocast_core::capture::project::Project;

const OLD_EMPTY: &str = r##"{"version":1,"name":"Golden","created":"2026-01-01T00:00:00Z","duration":12.0,"sources":{"screen":{"file":"screen.mov","offset":0.0}},"trims":[],"speeds":[],"zooms":[],"cursor":{"style":"arrow","size":1.4,"smoothing":0.5,"click_effect":"ring","hide_idle_after":3.0,"motion_blur":0.5},"style":{"aspect":"16:9","padding":0.06,"radius":18.0,"shadow":{"blur":40.0,"opacity":0.5,"dy":16.0},"background":{"kind":"gradient","stops":["#1f1c2c","#928dab"],"angle":135.0}},"audio":{"mic_gain":1.0,"system_gain":0.6,"mute":false},"export":{"format":"mp4","codec":"h264","size":[1920,1080],"fps":60}}"##;
const OLD_CAMERA: &str = r##"{"version":1,"name":"Camera fixture","created":"2026-10-23T14:05:22Z","duration":10.0,"sources":{"screen":{"file":"screen.mov","offset":0.0},"camera":{"file":"camera.mov","offset":0.1},"mic":{"file":"mic.m4a","offset":0.2},"system":{"file":"system.m4a","offset":-0.05}},"trims":[{"start":1.0,"end":9.0}],"speeds":[{"start":4.0,"end":6.0,"factor":2.0}],"zooms":[],"cursor":{"style":"arrow","size":1.4,"smoothing":0.5,"click_effect":"ring","hide_idle_after":3.0,"motion_blur":0.5},"style":{"aspect":"16:9","padding":0.06,"radius":18.0,"shadow":{"blur":40.0,"opacity":0.5,"dy":16.0},"background":{"kind":"gradient","stops":["#1f1c2c","#928dab"],"angle":135.0}},"camera":{"shape":"rounded","corner":"top_right","size":0.25,"margin":0.03,"mirror":false,"visible":true},"audio":{"mic_gain":1.0,"system_gain":0.6,"mute":false,"mic":{"gain_db":3.0,"mute":false},"system":{"gain_db":-6.0,"mute":true},"master_db":-1.5},"export":{"format":"mp4","codec":"h264","size":[1920,1080],"fps":30}}"##;
const CAMERA_FIXTURE: &str = include_str!("../fixtures/camera_project.json");

#[test]
fn a_project_without_p6_fields_serialises_byte_identically() {
    let mut p = Project::new("Golden", "2026-01-01T00:00:00Z");
    p.duration = 12.0;
    assert_eq!(serde_json::to_string(&p).unwrap(), OLD_EMPTY);
}

#[test]
fn loading_an_old_camera_project_and_saving_changes_nothing() {
    let p = Project::from_json(CAMERA_FIXTURE).unwrap();
    assert_eq!(serde_json::to_string(&p).unwrap(), OLD_CAMERA);
    let again = Project::from_json(&p.to_json()).unwrap();
    assert_eq!(again, p);
    assert_eq!(again.to_json(), p.to_json());
    let cam = serde_json::to_value(p.camera.as_ref().unwrap()).unwrap();
    let keys: Vec<&str> = cam.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(keys, ["corner", "margin", "mirror", "shape", "size", "visible"], "no P6 key may leak into old camera layouts");
}
