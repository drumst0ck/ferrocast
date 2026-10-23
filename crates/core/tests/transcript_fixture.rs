//! Transcript fixture + project compatibility: a project without a transcript serialises
//! exactly as before the field had a type; one with it round-trips.

use ferrocast_core::capture::project::Project;
use ferrocast_core::capture::transcript::{apply_cuts, remove_fillers, tighten_pauses, Options, Transcript};

const TRANSCRIPT: &str = include_str!("../fixtures/transcript.json");

/// Verified once against `to_json` output captured before `transcript` was typed (identical);
/// kept as a stability check: no `transcript` key and a fixed point under reload.
#[test]
fn fixtures_without_transcript_serialize_unchanged() {
    for (name, src) in [
        ("area_project", include_str!("../fixtures/area_project.json")),
        ("render_project", include_str!("../fixtures/render_project.json")),
        ("render_zoom_project", include_str!("../fixtures/render_zoom_project.json")),
    ] {
        let p = Project::from_json(src).unwrap();
        assert!(p.transcript.is_none(), "{name}");
        let out = p.to_json();
        assert!(!out.contains("transcript"), "{name}");
        let again = Project::from_json(&out).unwrap();
        assert_eq!(again.to_json(), out, "{name}");
    }
}

#[test]
fn project_with_transcript_roundtrips_and_edits() {
    let t = Transcript::from_json(TRANSCRIPT).unwrap();
    let mut p = Project::new("t", "2025-01-01T00:00:00Z");
    p.duration = 6.0;
    p.transcript = Some(t.clone());
    let json = p.to_json();
    assert!(json.contains("\"transcript\""));
    assert_eq!(Project::from_json(&json).unwrap().transcript, Some(t.clone()));

    let o = Options { duration: Some(6.0), ..Options::default() };
    let fillers = remove_fillers(&t, "en", &o);
    assert_eq!(fillers.len(), 1); // "um," only; "I like this." keeps "like"
    let mut cuts = fillers;
    cuts.extend(tighten_pauses(&t, &o)); // 1.9 -> 3.2 gap and the 0.4 lead stay/shrink
    let q = apply_cuts(&p, &cuts).unwrap();
    assert!(q.trims.len() >= 2);
    assert!(q.transcript.is_some());
}
