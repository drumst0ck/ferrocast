//! S11: a project left with `screen.mov` but no duration/sources (crash, kill -9, older builds) is
//! repaired from a `probe` of its media; anything else, and any probe failure, is left untouched.
use crate::record_ctl::{needs_recovery, recover_project, recover_unfinished};
use ferrocast_core::capture::project::Project;
use ferrocast_core::capture::protocol::{ProbeDone, ProbedFile};
use std::path::{Path, PathBuf};

struct Scratch(PathBuf);
impl Scratch {
    fn new(tag: &str) -> Scratch {
        let p = std::env::temp_dir().join(format!("fc-recov-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Scratch(p)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A project as a killed recording leaves it: `screen.mov` (+ extra files) and a zero duration.
fn broken(root: &Path, folder: &str, files: &[&str]) -> PathBuf {
    let dir = root.join(folder);
    Project::create(&dir, "x", "2026-10-23T14:05:22Z").unwrap();
    for f in files {
        std::fs::write(dir.join(f), b"media").unwrap();
    }
    dir
}

fn pf(file: &str, duration: f64, video: bool, audio: bool) -> ProbedFile {
    ProbedFile { file: file.into(), duration, video, audio, size: if video { [1280, 720] } else { [0, 0] } }
}

fn bytes(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .filter(|e| e.path().is_file())
        .map(|e| (e.file_name().to_string_lossy().into_owned(), std::fs::read(e.path()).unwrap()))
        .collect();
    v.sort();
    v
}

#[test]
fn a_project_with_screen_but_no_duration_is_repaired_from_the_probe() {
    let s = Scratch::new("fix");
    let dir = broken(&s.0, "Demo.ferrocast", &["screen.mov", "camera.mov", "mic.m4a", "system.m4a"]);
    let screen = std::fs::read(dir.join("screen.mov")).unwrap();
    assert!(needs_recovery(&dir));
    let asked = std::cell::RefCell::new(vec![]);
    let fixed = recover_project(&dir, |d, names| {
        assert_eq!(d, dir.as_path());
        *asked.borrow_mut() = names.to_vec();
        Ok(ProbeDone {
            files: vec![
                pf("screen.mov", 42.5, true, false),
                pf("camera.mov", 42.4, true, false),
                pf("mic.m4a", 42.5, false, true),
                pf("system.m4a", 42.5, false, true),
            ],
        })
    });
    assert!(fixed);
    assert_eq!(asked.borrow().clone(), ["screen.mov", "camera.mov", "mic.m4a", "system.m4a"]);
    let p = Project::load(&dir).unwrap();
    assert_eq!(p.duration, 42.5);
    assert_eq!(p.sources.camera.unwrap().file, "camera.mov");
    assert_eq!(p.sources.mic.unwrap().file, "mic.m4a");
    assert_eq!(p.sources.system.unwrap().file, "system.m4a");
    assert!(p.camera.is_some());
    assert_eq!(std::fs::read(dir.join("screen.mov")).unwrap(), screen, "media is never rewritten");
    assert!(!needs_recovery(&dir), "repaired once, not again");
}

#[test]
fn only_sources_the_probe_confirms_are_recorded() {
    let s = Scratch::new("partial");
    let dir = broken(&s.0, "Demo.ferrocast", &["screen.mov", "camera.mov", "mic.m4a"]);
    // camera.mov unreadable (absent from the probe); mic.m4a has no audio track
    assert!(recover_project(&dir, |_, _| Ok(ProbeDone {
        files: vec![pf("screen.mov", 9.0, true, false), pf("mic.m4a", 9.0, true, false)]
    })));
    let p = Project::load(&dir).unwrap();
    assert_eq!(p.duration, 9.0);
    assert!(p.sources.camera.is_none() && p.sources.mic.is_none() && p.sources.system.is_none());
    assert!(dir.join("camera.mov").exists(), "the unreadable file stays on disk");
}

#[test]
fn a_failed_probe_or_unreadable_screen_leaves_every_file_unchanged() {
    let s = Scratch::new("fail");
    let dir = broken(&s.0, "Demo.ferrocast", &["screen.mov", "mic.m4a"]);
    let before = bytes(&dir);
    assert!(!recover_project(&dir, |_, _| Err("helper closed".into())));
    assert_eq!(bytes(&dir), before);
    // the screen movie is not in the reply (damaged): nothing to recover from
    assert!(!recover_project(&dir, |_, _| Ok(ProbeDone { files: vec![pf("mic.m4a", 3.0, false, true)] })));
    assert_eq!(bytes(&dir), before);
    // a zero duration is not a recovery
    assert!(!recover_project(&dir, |_, _| Ok(ProbeDone { files: vec![pf("screen.mov", 0.0, true, false)] })));
    assert_eq!(bytes(&dir), before);
}

#[test]
fn finished_missing_screen_or_unreadable_projects_are_not_candidates_and_never_probed() {
    let s = Scratch::new("skip");
    let done = broken(&s.0, "Done.ferrocast", &["screen.mov"]);
    let mut p = Project::load(&done).unwrap();
    p.duration = 5.0;
    p.save(&done).unwrap();
    let no_screen = broken(&s.0, "NoScreen.ferrocast", &[]);
    let junk = s.0.join("Junk.ferrocast");
    std::fs::create_dir_all(&junk).unwrap();
    std::fs::write(junk.join("screen.mov"), b"x").unwrap();
    std::fs::write(junk.join("project.json"), b"{ not json").unwrap();
    for d in [&done, &no_screen, &junk] {
        assert!(!needs_recovery(d), "{d:?}");
        assert!(!recover_project(d, |_, _| panic!("must not probe")));
    }
    assert_eq!(Project::load(&done).unwrap().duration, 5.0);
    assert_eq!(std::fs::read(junk.join("project.json")).unwrap(), b"{ not json");
}

#[test]
fn recover_unfinished_skips_the_project_being_recorded_and_without_candidates_starts_nothing() {
    let s = Scratch::new("all");
    let live = broken(&s.0, "Live.ferrocast", &["screen.mov"]);
    // no helper exists at this path: with only the live project there is nothing to probe, so nothing runs
    let exe = s.0.join("no-such-ferrocast");
    assert_eq!(recover_unfinished(&s.0, Some(&live), &exe, |_| {}), 0);
    // a candidate exists but the helper cannot start: left untouched
    let other = broken(&s.0, "Other.ferrocast", &["screen.mov"]);
    let before = bytes(&other);
    assert_eq!(recover_unfinished(&s.0, Some(&live), &exe, |_| {}), 0);
    assert_eq!(bytes(&other), before);
    assert!(needs_recovery(&live) && needs_recovery(&other));
}
