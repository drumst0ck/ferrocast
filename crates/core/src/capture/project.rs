//! The `<name>.ferrocast` project folder and its `project.json` EDL skeleton.
//!
//! Unknown fields survive load/save (`extra`), every field has a default so old
//! files keep loading, and P3+ fields are omitted when empty so a project that
//! does not use them serializes byte-stably.

use super::MIN_SEGMENT;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

pub const VERSION: u32 = 1;
pub const PROJECT_FILE: &str = "project.json";
pub const EVENTS_FILE: &str = "events.json";
pub const THUMB_FILE: &str = "thumb.png";
pub const EXTENSION: &str = "ferrocast";
pub const NEWER_MESSAGE: &str = "This recording was made with a newer Ferrocast.";

#[derive(Debug)]
pub enum ProjectError {
    Io(io::Error),
    Json(String),
    Newer,
}

impl std::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "project file error: {e}"),
            Self::Json(e) => write!(f, "project.json is not valid: {e}"),
            Self::Newer => f.write_str(NEWER_MESSAGE),
        }
    }
}

impl std::error::Error for ProjectError {}

impl From<io::Error> for ProjectError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Source {
    pub file: String,
    #[serde(default)]
    pub offset: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sources {
    #[serde(default = "default_screen")]
    pub screen: Source,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<Source>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mic: Option<Source>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<Source>,
}

fn default_screen() -> Source {
    Source { file: "screen.mov".into(), offset: 0.0 }
}

impl Default for Sources {
    fn default() -> Self {
        Self { screen: default_screen(), camera: None, mic: None, system: None }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Trim {
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Speed {
    pub start: f64,
    pub end: f64,
    pub factor: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Zoom {
    pub id: String,
    pub start: f64,
    pub end: f64,
    #[serde(default = "auto_mode")]
    pub mode: String,
    #[serde(default = "two")]
    pub scale: f64,
    /// `None` = auto (centre from cursor events), else normalised `[x, y]`.
    #[serde(default)]
    pub focus: Option<[f64; 2]>,
    #[serde(default = "ease_in")]
    pub ease_in: f64,
    #[serde(default = "ease_out")]
    pub ease_out: f64,
}

fn auto_mode() -> String {
    "auto".into()
}
fn two() -> f64 {
    2.0
}
fn ease_in() -> f64 {
    0.6
}
fn ease_out() -> f64 {
    0.7
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CursorStyle {
    pub style: String,
    pub size: f64,
    pub smoothing: f64,
    pub click_effect: String,
    pub hide_idle_after: f64,
    pub motion_blur: f64,
}

impl Default for CursorStyle {
    fn default() -> Self {
        Self { style: "arrow".into(), size: 1.4, smoothing: 0.5, click_effect: "ring".into(), hide_idle_after: 3.0, motion_blur: 0.5 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Shadow {
    pub blur: f64,
    pub opacity: f64,
    pub dy: f64,
}

impl Default for Shadow {
    fn default() -> Self {
        Self { blur: 40.0, opacity: 0.5, dy: 16.0 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Background {
    /// `color` | `gradient` | `image` | `wallpaper`.
    pub kind: String,
    pub stops: Vec<String>,
    pub angle: f64,
}

impl Default for Background {
    fn default() -> Self {
        Self { kind: "gradient".into(), stops: vec!["#1f1c2c".into(), "#928dab".into()], angle: 135.0 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Style {
    pub aspect: String,
    pub padding: f64,
    pub radius: f64,
    pub shadow: Shadow,
    pub background: Background,
}

impl Default for Style {
    fn default() -> Self {
        Self { aspect: "16:9".into(), padding: 0.06, radius: 18.0, shadow: Shadow::default(), background: Background::default() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Audio {
    pub mic_gain: f64,
    pub system_gain: f64,
    pub mute: bool,
}

impl Default for Audio {
    fn default() -> Self {
        Self { mic_gain: 1.0, system_gain: 0.6, mute: false }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Export {
    pub format: String,
    pub codec: String,
    pub size: [u32; 2],
    pub fps: u32,
}

impl Default for Export {
    fn default() -> Self {
        Self { format: "mp4".into(), codec: "h264".into(), size: [1920, 1080], fps: 60 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    #[serde(default = "one")]
    pub version: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub created: String,
    #[serde(default)]
    pub duration: f64,
    #[serde(default)]
    pub sources: Sources,
    #[serde(default)]
    pub trims: Vec<Trim>,
    #[serde(default)]
    pub speeds: Vec<Speed>,
    #[serde(default)]
    pub zooms: Vec<Zoom>,
    #[serde(default)]
    pub cursor: CursorStyle,
    #[serde(default)]
    pub style: Style,
    /// P4 camera layout (placeholder, opaque until implemented).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<Value>,
    #[serde(default)]
    pub audio: Audio,
    /// P6 effects (placeholder).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub loupes: Vec<Value>,
    /// P5 transcript (placeholder).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transcript: Option<Value>,
    #[serde(default)]
    pub export: Export,
    /// Fields written by a newer build, kept on save.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

fn one() -> u32 {
    VERSION
}

impl Project {
    pub fn new(name: &str, created: &str) -> Self {
        Self {
            version: VERSION,
            name: name.into(),
            created: created.into(),
            duration: 0.0,
            sources: Sources::default(),
            trims: vec![],
            speeds: vec![],
            zooms: vec![],
            cursor: CursorStyle::default(),
            style: Style::default(),
            camera: None,
            audio: Audio::default(),
            loupes: vec![],
            transcript: None,
            export: Export::default(),
            extra: BTreeMap::new(),
        }
    }

    /// Create the folder (and parents) and write the initial `project.json`.
    pub fn create(dir: &Path, name: &str, created: &str) -> Result<Self, ProjectError> {
        std::fs::create_dir_all(dir)?;
        let p = Self::new(name, created);
        p.save(dir)?;
        Ok(p)
    }

    pub fn from_json(json: &str) -> Result<Self, ProjectError> {
        let raw: Value = serde_json::from_str(json).map_err(|e| ProjectError::Json(e.to_string()))?;
        if raw.get("version").and_then(Value::as_u64).is_some_and(|v| v > u64::from(VERSION)) {
            return Err(ProjectError::Newer);
        }
        serde_json::from_value(raw).map_err(|e| ProjectError::Json(e.to_string()))
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("project serializes")
    }

    pub fn load(dir: &Path) -> Result<Self, ProjectError> {
        Self::from_json(&std::fs::read_to_string(dir.join(PROJECT_FILE))?)
    }

    /// Atomic save: temp file in the same directory, then rename over the old one.
    pub fn save(&self, dir: &Path) -> Result<(), ProjectError> {
        atomic_write(&dir.join(PROJECT_FILE), self.to_json().as_bytes(), |p, b| std::fs::write(p, b))?;
        Ok(())
    }

    /// Problems with the edit list. `take_exists(file)` says whether a take
    /// (relative file name inside the project) is present.
    pub fn validate(&self, take_exists: impl Fn(&str) -> bool) -> Vec<String> {
        let mut out = vec![];
        if self.version > VERSION {
            out.push(NEWER_MESSAGE.into());
        }
        if !(self.duration.is_finite() && self.duration >= 0.0) {
            out.push("duration must be a non-negative number".into());
        }
        let takes = [
            Some(("screen", &self.sources.screen)),
            self.sources.camera.as_ref().map(|s| ("camera", s)),
            self.sources.mic.as_ref().map(|s| ("mic", s)),
            self.sources.system.as_ref().map(|s| ("system", s)),
        ];
        for (kind, s) in takes.into_iter().flatten() {
            if !take_exists(&s.file) {
                out.push(format!("{kind} take `{}` is missing", s.file));
            }
        }
        let mut prev_end = 0.0;
        for (i, t) in self.trims.iter().enumerate() {
            if t.start < prev_end {
                out.push(format!("trims[{i}]: overlaps or is not sorted"));
            }
            if t.end > self.duration || t.start < 0.0 {
                out.push(format!("trims[{i}]: outside 0..{}", self.duration));
            }
            if t.end - t.start < MIN_SEGMENT {
                out.push(format!("trims[{i}]: shorter than {MIN_SEGMENT} s"));
            }
            prev_end = t.end;
        }
        for (i, s) in self.speeds.iter().enumerate() {
            if !(0.25..=16.0).contains(&s.factor) {
                out.push(format!("speeds[{i}]: factor {} outside 0.25..=16", s.factor));
            }
            let kept = if self.trims.is_empty() {
                s.start >= 0.0 && s.end <= self.duration
            } else {
                self.trims.iter().any(|t| s.start >= t.start && s.end <= t.end)
            };
            if s.end <= s.start || !kept {
                out.push(format!("speeds[{i}]: not inside a kept range"));
            }
        }
        let mut zooms: Vec<&Zoom> = self.zooms.iter().collect();
        zooms.sort_by(|a, b| a.start.total_cmp(&b.start));
        for w in zooms.windows(2) {
            if w[1].start < w[0].end {
                out.push(format!("zooms {} and {} overlap", w[0].id, w[1].id));
            }
        }
        for z in &self.zooms {
            if !(1.0..=6.0).contains(&z.scale) {
                out.push(format!("zoom {}: scale {} outside 1..=6", z.id, z.scale));
            }
            if z.focus.is_some_and(|f| f.iter().any(|v| !(0.0..=1.0).contains(v))) {
                out.push(format!("zoom {}: focus outside 0..=1", z.id));
            }
        }
        for c in &self.style.background.stops {
            if !is_hex_color(c) {
                out.push(format!("background colour `{c}` is not #rrggbb"));
            }
        }
        let [w, h] = self.export.size;
        if w == 0 || h == 0 || w % 2 != 0 || h % 2 != 0 || w > 3840 || h > 2160 {
            out.push(format!("export.size {w}×{h} must be even and at most 3840×2160"));
        }
        if ![24, 30, 60].contains(&self.export.fps) {
            out.push(format!("export.fps {} must be 24, 30 or 60", self.export.fps));
        }
        out
    }
}

fn is_hex_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

/// Write `bytes` to a sibling temp file with `write`, then rename onto `path`.
/// On a failed write the temp file is removed and `path` is untouched.
pub fn atomic_write(path: &Path, bytes: &[u8], write: impl Fn(&Path, &[u8]) -> io::Result<()>) -> io::Result<()> {
    let mut tmp_name = path.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(".tmp");
    let tmp = path.with_file_name(tmp_name);
    if let Err(e) = write(&tmp, bytes).and_then(|()| std::fs::rename(&tmp, path)) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

pub fn thumb_path(dir: &Path) -> PathBuf {
    dir.join(THUMB_FILE)
}

pub fn events_path(dir: &Path) -> PathBuf {
    dir.join(EVENTS_FILE)
}

/// `<name>.ferrocast`, with ` 2`, ` 3`… before the extension while
/// `exists(folder)` says the name is taken. Path separators are replaced.
pub fn project_dir_name(name: &str, exists: impl Fn(&str) -> bool) -> String {
    let clean: String = name.trim().chars().map(|c| if matches!(c, '/' | '\\' | ':') { '-' } else { c }).collect();
    let stem = if clean.is_empty() || clean.chars().all(|c| c == '.') { "Recording".to_string() } else { clean };
    let mut folder = format!("{stem}.{EXTENSION}");
    let mut n = 2;
    while exists(&folder) {
        folder = format!("{stem} {n}.{EXTENSION}");
        n += 1;
    }
    folder
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAN: &str = r##"{"version":1,"name":"Demo","created":"2026-10-23T14:05:22Z","duration":73.2,
 "sources":{"screen":{"file":"screen.mov","offset":0.0}},
 "trims":[{"start":0.0,"end":73.2}],
 "speeds":[{"start":30.0,"end":38.0,"factor":4.0}],
 "zooms":[{"id":"z1","start":1.0,"end":4.6,"mode":"auto","scale":2.0,"focus":null,"ease_in":0.6,"ease_out":0.7}],
 "cursor":{"style":"arrow","size":1.4,"smoothing":0.5,"click_effect":"ring","hide_idle_after":3.0,"motion_blur":0.5},
 "style":{"aspect":"16:9","padding":0.06,"radius":18,"shadow":{"blur":40,"opacity":0.5,"dy":16},
          "background":{"kind":"gradient","stops":["#1f1c2c","#928dab"],"angle":135}},
 "audio":{"mic_gain":1.0,"system_gain":0.6,"mute":false},
 "export":{"format":"mp4","codec":"h264","size":[1920,1080],"fps":60}}"##;

    fn all_present(_: &str) -> bool {
        true
    }

    struct Tmp(PathBuf);
    impl Tmp {
        fn new(tag: &str) -> Self {
            let d = std::env::temp_dir().join(format!("ferrocast-project-test-{}-{tag}", std::process::id()));
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(&d).unwrap();
            Tmp(d)
        }
    }
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn plan_example_loads_and_validates() {
        let p = Project::from_json(PLAN).unwrap();
        assert_eq!(p.name, "Demo");
        assert_eq!(p.zooms[0].focus, None);
        assert_eq!(p.export.fps, 60);
        assert!(p.validate(all_present).is_empty(), "{:?}", p.validate(all_present));
    }

    #[test]
    fn round_trip_is_byte_stable_without_p3_fields() {
        let p = Project::from_json(PLAN).unwrap();
        let first = p.to_json();
        assert!(!first.contains("camera") && !first.contains("loupes") && !first.contains("transcript"));
        let second = Project::from_json(&first).unwrap().to_json();
        assert_eq!(first, second);
        assert_eq!(Project::from_json(&first).unwrap(), p);
    }

    #[test]
    fn unknown_fields_survive_and_missing_fields_default() {
        let p = Project::from_json(r#"{"version":1,"name":"x","future_thing":{"a":[1,2]}}"#).unwrap();
        assert_eq!(p.export, Export::default());
        assert_eq!(p.sources.screen.file, "screen.mov");
        assert!(p.to_json().contains("future_thing"));
        assert_eq!(Project::from_json(&p.to_json()).unwrap(), p);
    }

    #[test]
    fn newer_version_is_rejected_with_the_plan_message() {
        let err = Project::from_json(r#"{"version":2}"#).unwrap_err();
        assert_eq!(err.to_string(), "This recording was made with a newer Ferrocast.");
        assert!(matches!(Project::from_json("{").unwrap_err(), ProjectError::Json(_)));
    }

    #[test]
    fn validate_reports_each_kind_of_problem() {
        let mut p = Project::from_json(PLAN).unwrap();
        p.trims = vec![Trim { start: 0.0, end: 10.0 }, Trim { start: 5.0, end: 90.0 }, Trim { start: 91.0, end: 91.01 }];
        p.speeds = vec![Speed { start: 1.0, end: 2.0, factor: 20.0 }];
        p.zooms.push(Zoom {
            id: "z2".into(),
            start: 2.0,
            end: 5.0,
            mode: "manual".into(),
            scale: 9.0,
            focus: Some([1.5, 0.5]),
            ease_in: 0.6,
            ease_out: 0.7,
        });
        p.style.background.stops = vec!["red".into()];
        p.export.size = [1921, 1080];
        p.export.fps = 50;
        let problems = p.validate(|f| f != "screen.mov").join("\n");
        for needle in [
            "screen take",
            "trims[1]: overlaps",
            "trims[1]: outside",
            "trims[2]: shorter",
            "speeds[0]: factor",
            "zooms z1 and z2 overlap",
            "scale 9",
            "focus outside",
            "`red`",
            "export.size",
            "export.fps",
        ] {
            assert!(problems.contains(needle), "missing `{needle}` in:\n{problems}");
        }
    }

    #[test]
    fn validate_flags_a_newer_version_and_a_missing_optional_take() {
        let mut p = Project::new("x", "t");
        p.duration = 1.0;
        p.version = 3;
        p.sources.mic = Some(Source { file: "mic.m4a".into(), offset: 0.0 });
        let problems = p.validate(|f| f == "screen.mov");
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(problems.iter().any(|m| m == NEWER_MESSAGE));
    }

    #[test]
    fn folder_naming_adds_collision_suffix() {
        assert_eq!(project_dir_name("Demo", |_| false), "Demo.ferrocast");
        let taken = ["Demo.ferrocast", "Demo 2.ferrocast"];
        assert_eq!(project_dir_name("Demo", |n| taken.contains(&n)), "Demo 3.ferrocast");
        assert_eq!(project_dir_name(" a/b:c ", |_| false), "a-b-c.ferrocast");
        assert_eq!(project_dir_name("  ", |_| false), "Recording.ferrocast");
    }

    #[test]
    fn paths_inside_a_project() {
        let d = Path::new("/r/Demo.ferrocast");
        assert_eq!(thumb_path(d), Path::new("/r/Demo.ferrocast/thumb.png"));
        assert_eq!(events_path(d), Path::new("/r/Demo.ferrocast/events.json"));
    }

    #[test]
    fn create_load_save_round_trip_on_disk() {
        let t = Tmp::new("roundtrip");
        let dir = t.0.join("Demo.ferrocast");
        let mut p = Project::create(&dir, "Demo", "2026-10-23T14:05:22Z").unwrap();
        assert_eq!(Project::load(&dir).unwrap(), p);
        p.duration = 5.0;
        p.save(&dir).unwrap();
        assert_eq!(Project::load(&dir).unwrap().duration, 5.0);
        assert!(!dir.join("project.json.tmp").exists());
    }

    #[test]
    fn failed_write_leaves_the_old_file_intact() {
        let t = Tmp::new("failwrite");
        let path = t.0.join(PROJECT_FILE);
        std::fs::write(&path, b"old").unwrap();
        let err = atomic_write(&path, b"new", |p, _| {
            std::fs::write(p, b"par")?; // partial temp content, then failure
            Err(io::Error::other("disk full"))
        })
        .unwrap_err();
        assert_eq!(err.to_string(), "disk full");
        assert_eq!(std::fs::read(&path).unwrap(), b"old");
        assert!(!t.0.join("project.json.tmp").exists());
    }

    #[test]
    fn failed_rename_leaves_the_old_file_intact() {
        let t = Tmp::new("failrename");
        let path = t.0.join(PROJECT_FILE);
        std::fs::create_dir(&path).unwrap(); // rename file over a directory fails
        std::fs::write(path.join("keep"), b"old").unwrap();
        assert!(atomic_write(&path, b"new", |p, b| std::fs::write(p, b)).is_err());
        assert_eq!(std::fs::read(path.join("keep")).unwrap(), b"old");
        assert!(!t.0.join("project.json.tmp").exists());
    }
}
