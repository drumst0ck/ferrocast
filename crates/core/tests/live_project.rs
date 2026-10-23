//! Validates a project written by the editor autosave on a real recording.
//! Run: `FERROCAST_LIVE_PROJECT=<folder> cargo test -p ferrocast-core --locked --test live_project -- --ignored --nocapture`

use ferrocast_core::capture::{project::Project, timeline};
use std::path::PathBuf;

#[test]
#[ignore = "needs FERROCAST_LIVE_PROJECT=<project folder>"]
fn a_saved_project_decodes_and_validates() {
    let Some(dir) = std::env::var_os("FERROCAST_LIVE_PROJECT").map(PathBuf::from) else {
        eprintln!("SKIP: FERROCAST_LIVE_PROJECT is not set");
        return;
    };
    let p = Project::load(&dir).expect("project.json decodes");
    let problems = p.validate(|f| dir.join(f).is_file());
    assert!(problems.is_empty(), "Project::validate: {problems:?}");
    let tl = timeline::validate(&p);
    assert!(tl.is_empty(), "timeline::validate: {tl:?}");
    eprintln!(
        "ok: {} zooms, {} trims, {} speeds, output {:.3} s of {:.3} s",
        p.zooms.len(),
        p.trims.len(),
        p.speeds.len(),
        timeline::Timeline::new(&p).output_duration(),
        p.duration
    );
}
