//! The recorded input event log (`events.json`), pure serde types.
//!
//! Times are seconds from the first screen sample; coordinates are points
//! relative to the recorded display, top-left origin. Key *contents* exist
//! only for shortcuts (a modifier other than shift); plain typing keeps `t`
//! and `down` only, so a recording can never contain a password.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Current `events.json` format version.
pub const VERSION: u32 = 1;
/// Nominal cursor sampling rate.
pub const CURSOR_HZ: f64 = 60.0;

#[derive(Debug, Clone, PartialEq)]
pub enum EventsError {
    Json(String),
    MissingVersion,
    Newer(u32),
    Privacy(String),
}

impl fmt::Display for EventsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(e) => write!(f, "events.json is not valid: {e}"),
            Self::MissingVersion => write!(f, "events.json has no numeric `version` field"),
            Self::Newer(v) => write!(f, "events.json version {v} was made with a newer Ferrocast."),
            Self::Privacy(e) => write!(f, "events.json rejected: {e}"),
        }
    }
}

impl std::error::Error for EventsError {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScreenInfo {
    /// Pixel size of `screen.mov`.
    pub size: [u32; 2],
    pub scale: f64,
    /// `[x, y, w, h]` of the display in points.
    pub display_frame: [f64; 4],
    /// `[x, y, w, h]` of the recorded area in points, in the SAME space as the
    /// coordinates (relative to the display's top-left, NOT normalised). Written by
    /// Record Area / Record Window, where `screen.mov` covers only this rectangle.
    /// Absent = the video is the whole display.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture_rect: Option<[f64; 4]>,
}

/// Maps display points (the unit of `events.json` coordinates) into the normalised
/// `0..=1` space of `screen.mov`, which is the display or `screen.capture_rect`.
///
/// Built once per take. A missing rect, or one covering the whole display (a window as
/// large as the screen), is the identity space, so display recordings keep computing
/// exactly what they did before `capture_rect` existed.
///
/// Policy for points outside the recorded rectangle: [`CaptureSpace::point`] returns the
/// raw (possibly `< 0` / `> 1`) value and [`CaptureSpace::contains`] says whether it is
/// visible; callers HIDE a cursor or click ring that is outside (it is not in the video,
/// and the plan's cursor is hidden whenever it is not drawn) and use
/// [`CaptureSpace::point_clamped`] where a position is always needed (camera follow,
/// zoom focus). A display recording has no outside: its points clamp to the edge, as before.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CaptureSpace {
    origin: [f64; 2],
    size: [f64; 2],
    /// The rect is a strict sub-area of the display (points outside can be hidden).
    sub_area: bool,
}

/// Slack when comparing points with the rectangle edges (normalised units).
const EDGE_EPS: f64 = 1e-9;

impl CaptureSpace {
    pub fn new(screen: &ScreenInfo) -> Self {
        let [_, _, dw, dh] = screen.display_frame;
        let whole = Self { origin: [0.0, 0.0], size: [dw, dh], sub_area: false };
        match screen.capture_rect {
            Some([x, y, w, h]) if [x, y, w, h].iter().all(|v| v.is_finite()) && w > 0.0 && h > 0.0 => {
                let covers = x <= 0.0 && y <= 0.0 && x + w >= dw && y + h >= dh;
                if covers {
                    whole
                } else {
                    Self { origin: [x, y], size: [w, h], sub_area: true }
                }
            }
            _ => whole,
        }
    }

    /// Size in points of the space (the recorded rectangle, or the display).
    pub fn size(&self) -> [f64; 2] {
        self.size
    }

    /// Whether the space is a strict sub-area of the display.
    pub fn is_sub_area(&self) -> bool {
        self.sub_area
    }

    /// Display point -> normalised position in the recorded video (unclamped).
    pub fn point(&self, x: f64, y: f64) -> [f64; 2] {
        [(x - self.origin[0]) / self.size[0], (y - self.origin[1]) / self.size[1]]
    }

    /// [`Self::point`] clamped to the video (`0..=1`).
    pub fn point_clamped(&self, x: f64, y: f64) -> [f64; 2] {
        let [u, v] = self.point(x, y);
        [u.clamp(0.0, 1.0), v.clamp(0.0, 1.0)]
    }

    /// Whether a display point is visible in the video. Always true for a display recording.
    pub fn contains(&self, x: f64, y: f64) -> bool {
        if !self.sub_area {
            return true;
        }
        let [u, v] = self.point(x, y);
        (-EDGE_EPS..=1.0 + EDGE_EPS).contains(&u) && (-EDGE_EPS..=1.0 + EDGE_EPS).contains(&v)
    }

    /// Display rect `[x, y, w, h]` (points) -> the same rect normalised to the video, unclamped.
    pub fn rect(&self, r: [f64; 4]) -> [f64; 4] {
        let [x, y] = self.point(r[0], r[1]);
        [x, y, r[2] / self.size[0], r[3] / self.size[1]]
    }
}

/// `[t, x, y, kind]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CursorSample(pub f64, pub f64, pub f64, pub String);

impl CursorSample {
    pub fn t(&self) -> f64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Click {
    pub t: f64,
    pub x: f64,
    pub y: f64,
    pub button: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub down: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub up: bool,
}

impl Click {
    /// A click event without an explicit `up` flag counts as a press.
    pub fn is_down(&self) -> bool {
        self.down || !self.up
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Key {
    pub t: f64,
    #[serde(default)]
    pub down: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mods: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default, rename = "char", skip_serializing_if = "Option::is_none")]
    pub ch: Option<String>,
}

impl Key {
    /// Any modifier except shift makes the key a shortcut.
    pub fn is_shortcut(&self) -> bool {
        self.mods.iter().any(|m| m != "shift")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scroll {
    pub t: f64,
    pub dx: f64,
    pub dy: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowFrame {
    pub t: f64,
    pub id: u64,
    /// `[x, y, w, h]` in points.
    pub frame: [f64; 4],
    #[serde(default)]
    pub app: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Events {
    pub version: u32,
    pub screen: ScreenInfo,
    #[serde(default)]
    pub cursor: Vec<CursorSample>,
    #[serde(default)]
    pub clicks: Vec<Click>,
    #[serde(default)]
    pub keys: Vec<Key>,
    #[serde(default)]
    pub scroll: Vec<Scroll>,
    #[serde(default)]
    pub windows: Vec<WindowFrame>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

impl Events {
    pub fn new(screen: ScreenInfo) -> Self {
        Self { version: VERSION, screen, cursor: vec![], clicks: vec![], keys: vec![], scroll: vec![], windows: vec![] }
    }

    /// Strict decode: the version is checked first (newer is rejected), unknown
    /// fields are ignored, and a plain key carrying its contents is refused.
    pub fn decode(json: &str) -> Result<Self, EventsError> {
        let raw: serde_json::Value = serde_json::from_str(json).map_err(|e| EventsError::Json(e.to_string()))?;
        let version = raw.get("version").and_then(|v| v.as_u64()).ok_or(EventsError::MissingVersion)? as u32;
        if version > VERSION {
            return Err(EventsError::Newer(version));
        }
        let ev: Events = serde_json::from_value(raw).map_err(|e| EventsError::Json(e.to_string()))?;
        if let Some(k) = ev.keys.iter().find(|k| !k.is_shortcut() && (k.code.is_some() || k.ch.is_some())) {
            return Err(EventsError::Privacy(format!("plain key at t={} carries its contents", k.t)));
        }
        Ok(ev)
    }

    pub fn encode(&self) -> String {
        serde_json::to_string(self).expect("events serialize")
    }

    /// Latest timestamp across every stream (0 when empty).
    pub fn duration(&self) -> f64 {
        let last = |v: Option<f64>| v.unwrap_or(0.0);
        [
            last(self.cursor.last().map(|c| c.t())),
            last(self.clicks.last().map(|c| c.t)),
            last(self.keys.last().map(|k| k.t)),
            last(self.scroll.last().map(|s| s.t)),
            last(self.windows.last().map(|w| w.t)),
        ]
        .into_iter()
        .fold(0.0, f64::max)
    }

    /// Problems found: non-finite or negative times, unsorted streams.
    pub fn validate(&self) -> Vec<String> {
        let mut out = vec![];
        let mut check = |name: &str, times: Vec<f64>| {
            for (i, t) in times.iter().enumerate() {
                if !t.is_finite() || *t < 0.0 {
                    out.push(format!("{name}[{i}]: invalid time {t}"));
                } else if i > 0 && *t < times[i - 1] {
                    out.push(format!("{name}[{i}]: time {t} is before {}", times[i - 1]));
                }
            }
        };
        check("cursor", self.cursor.iter().map(|c| c.t()).collect());
        check("clicks", self.clicks.iter().map(|c| c.t).collect());
        check("keys", self.keys.iter().map(|k| k.t).collect());
        check("scroll", self.scroll.iter().map(|s| s.t).collect());
        check("windows", self.windows.iter().map(|w| w.t).collect());
        if !(self.screen.scale.is_finite() && self.screen.scale > 0.0) {
            out.push("screen.scale must be positive".into());
        }
        if let Some([x, y, w, h]) = self.screen.capture_rect {
            let [_, _, dw, dh] = self.screen.display_frame;
            let finite = [x, y, w, h].iter().all(|v| v.is_finite());
            if !finite || w <= 0.0 || h <= 0.0 {
                out.push("screen.capture_rect must be finite with a positive size".into());
            } else if x < 0.0 || y < 0.0 || x + w > dw + 1e-6 || y + h > dh + 1e-6 {
                out.push("screen.capture_rect must lie inside the display".into());
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"version":1,"screen":{"size":[3456,2234],"scale":2.0,"display_frame":[0,0,1728,1117]},
 "cursor":[[0.000,812.5,400.2,"arrow"],[0.016,813.1,401.0,"arrow"]],
 "clicks":[{"t":1.42,"x":812.5,"y":400.2,"button":"left","down":true},{"t":1.51,"up":true,"x":812.5,"y":400.2,"button":"left"}],
 "keys":[{"t":2.10,"down":true,"mods":["cmd"],"code":"KeyC","char":"c"},{"t":2.5,"down":true}],
 "scroll":[{"t":4.2,"dx":0,"dy":-12.0}],
 "windows":[{"t":0.0,"id":4231,"frame":[120,80,1200,800],"app":"Safari"}]}"#;

    #[test]
    fn decodes_the_plan_example() {
        let e = Events::decode(SAMPLE).unwrap();
        assert_eq!(e.screen.size, [3456, 2234]);
        assert_eq!(e.cursor.len(), 2);
        assert_eq!(e.cursor[1], CursorSample(0.016, 813.1, 401.0, "arrow".into()));
        assert!(e.clicks[0].is_down() && !e.clicks[1].is_down());
        assert!(e.keys[0].is_shortcut() && !e.keys[1].is_shortcut());
        assert_eq!(e.windows[0].app, "Safari");
        assert!(e.validate().is_empty());
        assert_eq!(e.duration(), 4.2);
    }

    #[test]
    fn encode_round_trips() {
        let e = Events::decode(SAMPLE).unwrap();
        assert_eq!(Events::decode(&e.encode()).unwrap(), e);
        assert_eq!(Events::decode(&e.encode()).unwrap().encode(), e.encode());
    }

    #[test]
    fn newer_version_is_rejected_with_a_clear_message() {
        let err = Events::decode(r#"{"version":2,"screen":{"size":[1,1],"scale":1,"display_frame":[0,0,1,1]}}"#).unwrap_err();
        assert_eq!(err, EventsError::Newer(2));
        assert!(err.to_string().contains("newer Ferrocast"));
    }

    #[test]
    fn missing_version_and_bad_json_are_errors() {
        assert_eq!(Events::decode("{}").unwrap_err(), EventsError::MissingVersion);
        assert!(matches!(Events::decode("nope").unwrap_err(), EventsError::Json(_)));
        let err = Events::decode(r#"{"version":1}"#).unwrap_err();
        assert!(err.to_string().contains("screen"), "{err}");
    }

    #[test]
    fn unknown_fields_and_missing_streams_are_tolerated() {
        let e = Events::decode(r#"{"version":1,"future":true,"screen":{"size":[1,1],"scale":1,"display_frame":[0,0,1,1],"x":1}}"#).unwrap();
        assert!(e.cursor.is_empty() && e.keys.is_empty());
        assert_eq!(e.duration(), 0.0);
    }

    #[test]
    fn plain_typing_with_contents_is_refused() {
        let json = SAMPLE.replace(r#"{"t":2.5,"down":true}"#, r#"{"t":2.5,"down":true,"char":"p"}"#);
        assert!(matches!(Events::decode(&json).unwrap_err(), EventsError::Privacy(_)));
        // shift alone is still plain typing
        let json = SAMPLE.replace(r#"{"t":2.5,"down":true}"#, r#"{"t":2.5,"down":true,"mods":["shift"],"code":"KeyA"}"#);
        assert!(matches!(Events::decode(&json).unwrap_err(), EventsError::Privacy(_)));
    }

    #[test]
    fn validate_reports_unsorted_and_invalid_times() {
        let mut e = Events::decode(SAMPLE).unwrap();
        e.cursor.push(CursorSample(0.001, 0.0, 0.0, "arrow".into()));
        e.scroll[0].t = -1.0;
        let problems = e.validate();
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(problems[0].starts_with("cursor[2]"));
        assert!(problems[1].starts_with("scroll[0]"));
    }

    fn screen(rect: Option<[f64; 4]>) -> ScreenInfo {
        ScreenInfo { size: [1000, 500], scale: 2.0, display_frame: [0.0, 0.0, 1000.0, 500.0], capture_rect: rect }
    }

    #[test]
    fn capture_rect_is_optional_and_round_trips() {
        let e = Events::decode(SAMPLE).unwrap();
        assert_eq!(e.screen.capture_rect, None);
        assert!(!e.encode().contains("capture_rect"), "absent stays absent");
        let with =
            SAMPLE.replace(r#""display_frame":[0,0,1728,1117]"#, r#""display_frame":[0,0,1728,1117],"capture_rect":[100,50,800,600]"#);
        let e = Events::decode(&with).unwrap();
        assert_eq!(e.screen.capture_rect, Some([100.0, 50.0, 800.0, 600.0]));
        assert!(e.validate().is_empty());
        assert_eq!(Events::decode(&e.encode()).unwrap(), e);
    }

    #[test]
    fn validate_checks_the_capture_rect() {
        let mut e = Events::new(screen(Some([500.0, 250.0, 500.0, 250.0])));
        assert!(e.validate().is_empty(), "the lower-right quarter touches the edges and is valid");
        for bad in [
            [500.0, 250.0, 0.0, 250.0],
            [500.0, 250.0, 500.0, -1.0],
            [-1.0, 0.0, 10.0, 10.0],
            [600.0, 250.0, 500.0, 250.0],
            [0.0, 300.0, 10.0, 250.0],
            [f64::NAN, 0.0, 1.0, 1.0],
        ] {
            e.screen.capture_rect = Some(bad);
            assert_eq!(e.validate().len(), 1, "{bad:?}: {:?}", e.validate());
        }
    }

    #[test]
    fn capture_space_maps_points_and_rects_into_the_rect() {
        let s = CaptureSpace::new(&screen(Some([500.0, 250.0, 500.0, 250.0])));
        assert!(s.is_sub_area());
        assert_eq!(s.point(750.0, 375.0), [0.5, 0.5]);
        assert_eq!(s.point(500.0, 250.0), [0.0, 0.0]);
        assert_eq!(s.point(100.0, 600.0), [-0.8, 1.4]);
        assert_eq!(s.point_clamped(100.0, 600.0), [0.0, 1.0]);
        assert!(s.contains(1000.0, 500.0) && s.contains(500.0, 250.0) && s.contains(750.0, 300.0));
        assert!(!s.contains(499.0, 300.0) && !s.contains(750.0, 251.0 - 2.0) && !s.contains(1001.0, 300.0));
        assert_eq!(s.rect([600.0, 300.0, 100.0, 50.0]), [0.2, 0.2, 0.2, 0.2]);
    }

    #[test]
    fn no_rect_or_a_rect_covering_the_display_is_the_identity() {
        let none = CaptureSpace::new(&screen(None));
        let full = CaptureSpace::new(&screen(Some([0.0, 0.0, 1000.0, 500.0])));
        assert_eq!(none, full);
        assert!(!none.is_sub_area());
        assert_eq!(none.point(250.0, 125.0), [0.25, 0.25]);
        assert!(none.contains(-5.0, 9999.0), "a display recording has no outside");
        assert_eq!(none.rect([100.0, 50.0, 500.0, 250.0]), [0.1, 0.1, 0.5, 0.5]);
        // an invalid rect is ignored rather than dividing by zero
        assert_eq!(CaptureSpace::new(&screen(Some([0.0, 0.0, 0.0, 10.0]))), none);
    }
}
