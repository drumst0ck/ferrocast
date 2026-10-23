//! NDJSON protocol of the `ferrocast-capture` helper: pure serde types, no I/O.

use super::compose::FrameParams;
use super::export::{Codec, Format, Quality, Segment};
use serde::{Deserialize, Serialize};
pub use serde_json::Value;

/// Protocol version answered by `hello`.
pub const PROTOCOL: u32 = 1;

/// A command, tagged by `cmd` on the wire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd")]
pub enum Command {
    #[serde(rename = "hello")]
    Hello,
    #[serde(rename = "shareable")]
    Shareable,
    #[serde(rename = "screenshot")]
    Screenshot {
        target: Target,
        /// Absolute PNG path the helper writes (atomically).
        out: String,
        #[serde(default)]
        cursor: bool,
        #[serde(default)]
        window_shadow: bool,
        #[serde(default)]
        scale: Scale,
    },
    /// Start recording into `<project>/<file>`; both travel in JSON, never in argv.
    #[serde(rename = "record.start")]
    RecordStart {
        /// Absolute project directory (created when missing).
        project: String,
        /// Plain file name inside `project`; the helper writes `<file>.partial` and renames on stop.
        #[serde(default = "default_screen_file")]
        file: String,
        target: Target,
        #[serde(default = "default_fps")]
        fps: u32,
        /// Show the cursor in the video.
        #[serde(default)]
        cursor: bool,
        #[serde(default, skip_serializing_if = "Excludes::is_empty")]
        excludes: Excludes,
    },
    #[serde(rename = "record.stop")]
    RecordStop,
    #[serde(rename = "record.pause")]
    RecordPause,
    #[serde(rename = "record.resume")]
    RecordResume,
    /// Render one preview frame of a project to a PNG. `t` is the source time of the take in
    /// seconds (the caller maps output time with `timeline`); frames go through a file, never stdout.
    #[serde(rename = "render.frame")]
    RenderFrame {
        /// Absolute project directory.
        project: String,
        t: f64,
        /// Output width in pixels; the height follows the project's aspect.
        width: u32,
        /// Absolute PNG path; the helper picks one under `<project>/.cache` when absent.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        out: Option<String>,
        /// Optional `camera`, `cursor`, `ring`, `blur` (T25), evaluated by `capture::compose`.
        /// Absent = the plain styled frame of T24.
        #[serde(flatten)]
        params: FrameParams,
    },
    /// Export the project to a video file (S16). The helper answers with the event `started`
    /// (or an error), streams `progress` / `heartbeat` events and ends with `ok` (path, bytes,
    /// duration) or an error (`cancelled` after a `cancel`). Frames are computed by Rust:
    /// `params_file` is the NDJSON of `export::FrameLine`, one line per output frame.
    #[serde(rename = "render")]
    Render {
        /// Absolute project directory (its `project.json` provides the style).
        project: String,
        /// Absolute output path; the helper writes `<out>.partial` and renames on success.
        out: String,
        format: Format,
        codec: Codec,
        width: u32,
        height: u32,
        fps: u32,
        quality: Quality,
        /// Number of output frames (= lines of `params_file`).
        frames: u32,
        /// Kept source spans and their speed (the audio mix of T33 uses them).
        segments: Vec<Segment>,
        /// Absolute path of the per-frame parameters file.
        params_file: String,
    },
    /// Stop the request with id `target`; the export answers `cancelled` and leaves no output.
    #[serde(rename = "cancel")]
    Cancel { target: u64 },
}

/// Payload of the final `ok` of a `render`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportDone {
    pub path: String,
    pub bytes: u64,
    /// Duration of the written video in seconds.
    pub duration: f64,
    #[serde(default)]
    pub frames: u32,
}

/// What `Helper::export_start` returns once the helper accepted the job.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExportStarted {
    pub total_frames: u32,
}

/// Events of a running export. `Done`, `Failed` and `Cancelled` are terminal (built from the final
/// `ok` / error line by [`export_final`]); the others come from `event` lines ([`parse_export_event`]).
#[derive(Debug, Clone, PartialEq)]
pub enum ExportEvent {
    Started {
        total_frames: u32,
    },
    Progress {
        done_frames: u32,
        total_frames: u32,
        eta: Option<f64>,
    },
    /// Sign of life (at least every 30 s while the helper works).
    Heartbeat,
    Done(ExportDone),
    Failed {
        code: ErrorCode,
        message: String,
    },
    Cancelled,
}

impl ExportEvent {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Done(_) | Self::Failed { .. } | Self::Cancelled)
    }
}

#[derive(Deserialize)]
#[serde(tag = "event", rename_all = "lowercase")]
enum ExportWire {
    Started {
        total_frames: u32,
    },
    Progress {
        done_frames: u32,
        total_frames: u32,
        #[serde(default)]
        eta: Option<f64>,
    },
    Heartbeat {},
}

/// Typed view of a `started` / `progress` / `heartbeat` event line; `None` for other events
/// and for malformed ones.
pub fn parse_export_event(name: &str, body: &Value) -> Option<ExportEvent> {
    matches!(name, "started" | "progress" | "heartbeat").then(|| serde_json::from_value::<ExportWire>(body.clone()).ok()).flatten().map(
        |w| match w {
            ExportWire::Started { total_frames } => ExportEvent::Started { total_frames },
            ExportWire::Progress { done_frames, total_frames, eta } => ExportEvent::Progress { done_frames, total_frames, eta },
            ExportWire::Heartbeat {} => ExportEvent::Heartbeat,
        },
    )
}

/// Terminal event of an export from its final line: `ok` -> `Done`, error `cancelled` ->
/// `Cancelled`, any other error -> `Failed`. `None` for event lines or an `ok` that is not an export result.
pub fn export_final(line: &Line) -> Option<ExportEvent> {
    match line {
        Line::Ok { body, .. } => parse_body::<ExportDone>(body).ok().map(ExportEvent::Done),
        Line::Error { code: ErrorCode::Cancelled, .. } => Some(ExportEvent::Cancelled),
        Line::Error { code, message, .. } => Some(ExportEvent::Failed { code: *code, message: message.clone() }),
        Line::Event { .. } => None,
    }
}

/// Payload of a successful `render.frame`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderFrameDone {
    pub path: String,
    pub size: [u32; 2],
    #[serde(default)]
    pub ms: f64,
    /// Served from the helper's frame cache.
    #[serde(default)]
    pub cached: bool,
}

fn default_screen_file() -> String {
    "screen.mov".into()
}

fn default_fps() -> u32 {
    60
}

/// Windows left out of a recording (the app's own HUD).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Excludes {
    #[serde(default)]
    pub window_ids: Vec<u32>,
}

impl Excludes {
    pub fn is_empty(&self) -> bool {
        self.window_ids.is_empty()
    }
}

/// Payload of a successful `record.start`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordStarted {
    /// Host-clock seconds at which the recording timeline starts (t = 0).
    pub started_at_host: f64,
    #[serde(default)]
    pub size: [u32; 2],
    #[serde(default)]
    pub file: String,
}

/// Payload of a successful `record.stop`. `interrupted` is set when the stream ended by itself
/// and the helper finalised what it had (`message` says why).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordStopped {
    pub project: String,
    pub duration: f64,
    pub screen: String,
    pub size: [u32; 2],
    #[serde(default)]
    pub frames: u64,
    #[serde(default)]
    pub dropped: u64,
    #[serde(default)]
    pub interrupted: bool,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecordState {
    Recording,
    Paused,
    Stopped,
    Failed,
    #[serde(other)]
    Unknown,
}

/// Asynchronous events of a recording, tagged by `event` on the wire.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "event", rename_all = "lowercase")]
pub enum RecordEvent {
    Tick {
        elapsed: f64,
        frames: u64,
        dropped: u64,
    },
    State {
        state: RecordState,
        #[serde(default)]
        message: Option<String>,
    },
    Warn {
        code: String,
        #[serde(default)]
        message: Option<String>,
        #[serde(default)]
        free_bytes: Option<u64>,
    },
}

/// Typed view of an event line; `None` for events that are not recording events.
pub fn parse_record_event(name: &str, body: &Value) -> Option<RecordEvent> {
    matches!(name, "tick" | "state" | "warn").then(|| serde_json::from_value(body.clone()).ok()).flatten()
}

/// What a screenshot captures, tagged by `kind`. `rect` is display-local points, top-left origin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Target {
    #[serde(rename = "display")]
    Display { display: u32 },
    #[serde(rename = "window")]
    Window { window: u32 },
    #[serde(rename = "rect")]
    Rect { display: u32, rect: [f64; 4] },
}

/// Output scale of a screenshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Scale {
    #[default]
    #[serde(rename = "native")]
    Native,
}

/// Payload of a successful `screenshot`: pixel size of the written PNG and the display scale.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScreenshotDone {
    pub path: String,
    pub size: [u32; 2],
    pub scale: f64,
}

/// One request line: the id chosen by Rust plus the command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub id: u64,
    #[serde(flatten)]
    pub command: Command,
}

/// Error codes of `{"id":N,"error":"<code>","message":"…"}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    #[serde(rename = "unsupported-os")]
    UnsupportedOs,
    #[serde(rename = "permission-screen")]
    PermissionScreen,
    #[serde(rename = "permission-camera")]
    PermissionCamera,
    #[serde(rename = "permission-mic")]
    PermissionMic,
    #[serde(rename = "not-found")]
    NotFound,
    #[serde(rename = "busy")]
    Busy,
    #[serde(rename = "cancelled")]
    Cancelled,
    #[serde(rename = "disk-full")]
    DiskFull,
    #[serde(rename = "invalid")]
    Invalid,
    #[serde(rename = "internal", other)]
    Internal,
}

/// A decoded stdout line.
#[derive(Debug, Clone, PartialEq)]
pub enum Line {
    Ok { id: u64, body: Value },
    Error { id: Option<u64>, code: ErrorCode, message: String },
    Event { id: Option<u64>, name: String, body: Value },
}

/// Payload of a successful `hello`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hello {
    pub protocol: u32,
    pub os: String,
    pub version: String,
    pub sck: bool,
    pub metal: bool,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Display {
    pub id: u32,
    /// x, y, width, height in points.
    pub frame: [f64; 4],
    pub scale: f64,
    #[serde(default)]
    pub main: bool,
    #[serde(default)]
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Window {
    pub id: u32,
    #[serde(default)]
    pub app: String,
    #[serde(default)]
    pub bundle: String,
    #[serde(default)]
    pub title: String,
    pub frame: [f64; 4],
    #[serde(default)]
    pub layer: i32,
    #[serde(default)]
    pub on_screen: bool,
}

/// Payload of a successful `shareable`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Shareable {
    #[serde(default)]
    pub displays: Vec<Display>,
    #[serde(default)]
    pub windows: Vec<Window>,
}

/// One request as a single line without the trailing newline.
pub fn encode_request(req: &Request) -> String {
    serde_json::to_string(req).expect("request serializes")
}

/// Decode one stdout line. Unknown fields are ignored; anything that is not an object with `id`,
/// `ok`, `error` or `event` is an error.
pub fn decode_line(line: &str) -> Result<Line, String> {
    let v: Value = serde_json::from_str(line.trim()).map_err(|e| e.to_string())?;
    let obj = v.as_object().ok_or("not a JSON object")?;
    let id = obj.get("id").and_then(Value::as_u64);
    if let Some(name) = obj.get("event").and_then(Value::as_str) {
        return Ok(Line::Event { id, name: name.to_string(), body: v.clone() });
    }
    if let Some(code) = obj.get("error") {
        let code = serde_json::from_value(code.clone()).unwrap_or(ErrorCode::Internal);
        let message = obj.get("message").and_then(Value::as_str).unwrap_or_default().to_string();
        return Ok(Line::Error { id, code, message });
    }
    match (id, obj.get("ok").and_then(Value::as_bool)) {
        (Some(id), Some(true)) => Ok(Line::Ok { id, body: v.clone() }),
        _ => Err("line is neither a reply nor an event".into()),
    }
}

/// Typed view of an `ok` body.
pub fn parse_body<T: serde::de::DeserializeOwned>(body: &Value) -> Result<T, String> {
    serde_json::from_value(body.clone()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::super::compose::Camera;
    use super::*;

    #[test]
    fn request_round_trips_with_tag() {
        let r = Request { id: 7, command: Command::Shareable };
        let s = encode_request(&r);
        assert_eq!(s, r#"{"id":7,"cmd":"shareable"}"#);
        assert_eq!(serde_json::from_str::<Request>(&s).unwrap(), r);
        assert_eq!(encode_request(&Request { id: 1, command: Command::Hello }), r#"{"id":1,"cmd":"hello"}"#);
    }

    #[test]
    fn hello_reply_tolerates_unknown_fields() {
        let l =
            decode_line(r#"{"id":1,"ok":true,"protocol":1,"os":"27.0.0","version":"0.1","sck":true,"metal":false,"future":[1]}"#).unwrap();
        let Line::Ok { id, body } = l else { panic!("{l:?}") };
        assert_eq!(id, 1);
        let h: Hello = parse_body(&body).unwrap();
        assert_eq!((h.protocol, h.os.as_str(), h.sck, h.metal), (1, "27.0.0", true, false));
    }

    #[test]
    fn shareable_reply_parses() {
        let l = decode_line(r#"{"id":2,"ok":true,"displays":[{"id":1,"frame":[0,0,1728,1117],"scale":2.0,"main":true,"name":"Built-in"}],"windows":[{"id":4231,"app":"Safari","bundle":"com.apple.Safari","title":"Docs","frame":[120,80,1200,800],"layer":0,"on_screen":true}]}"#).unwrap();
        let Line::Ok { body, .. } = l else { panic!() };
        let s: Shareable = parse_body(&body).unwrap();
        assert_eq!(s.displays[0].frame, [0.0, 0.0, 1728.0, 1117.0]);
        assert_eq!(s.windows[0].id, 4231);
    }

    #[test]
    fn errors_map_codes_and_unknown_to_internal() {
        let l = decode_line(r#"{"id":3,"error":"permission-screen","message":"off"}"#).unwrap();
        assert_eq!(l, Line::Error { id: Some(3), code: ErrorCode::PermissionScreen, message: "off".into() });
        let l = decode_line(r#"{"error":"unsupported-os"}"#).unwrap();
        assert_eq!(l, Line::Error { id: None, code: ErrorCode::UnsupportedOs, message: String::new() });
        let Line::Error { code, .. } = decode_line(r#"{"id":1,"error":"weird"}"#).unwrap() else { panic!() };
        assert_eq!(code, ErrorCode::Internal);
    }

    #[test]
    fn events_and_garbage() {
        let l = decode_line(r#"{"event":"tick","id":5,"t":1.5}"#).unwrap();
        assert!(matches!(l, Line::Event { id: Some(5), ref name, .. } if name == "tick"));
        assert!(decode_line("not json").is_err());
        assert!(decode_line("[1]").is_err());
        assert!(decode_line(r#"{"id":1}"#).is_err());
    }

    #[test]
    fn screenshot_request_encodes_each_target() {
        let c = |target| Command::Screenshot {
            target,
            out: "/tmp/a b/é.png".into(),
            cursor: false,
            window_shadow: false,
            scale: Scale::Native,
        };
        let s = encode_request(&Request { id: 3, command: c(Target::Rect { display: 1, rect: [100.0, 100.0, 640.0, 400.0] }) });
        assert_eq!(
            s,
            r#"{"id":3,"cmd":"screenshot","target":{"kind":"rect","display":1,"rect":[100.0,100.0,640.0,400.0]},"out":"/tmp/a b/é.png","cursor":false,"window_shadow":false,"scale":"native"}"#
        );
        let s = encode_request(&Request { id: 4, command: c(Target::Window { window: 4231 }) });
        assert!(s.contains(r#""target":{"kind":"window","window":4231}"#), "{s}");
        let r = Request { id: 5, command: c(Target::Display { display: 2 }) };
        assert_eq!(serde_json::from_str::<Request>(&encode_request(&r)).unwrap(), r);
    }

    #[test]
    fn screenshot_request_defaults_optional_fields() {
        let r: Request =
            serde_json::from_str(r#"{"id":1,"cmd":"screenshot","target":{"kind":"display","display":1},"out":"/x.png"}"#).unwrap();
        let Command::Screenshot { cursor, window_shadow, scale, .. } = r.command else { panic!() };
        assert_eq!((cursor, window_shadow, scale), (false, false, Scale::Native));
    }

    #[test]
    fn screenshot_reply_parses_and_ignores_unknown_fields() {
        let Line::Ok { body, .. } = decode_line(r#"{"id":3,"ok":true,"path":"/x.png","size":[1280,800],"scale":2.0,"extra":1}"#).unwrap()
        else {
            panic!()
        };
        let d: ScreenshotDone = parse_body(&body).unwrap();
        assert_eq!((d.path.as_str(), d.size, d.scale), ("/x.png", [1280, 800], 2.0));
        assert!(parse_body::<ScreenshotDone>(&serde_json::json!({"ok":true})).is_err());
    }

    #[test]
    fn record_start_encodes_project_and_file_in_json_and_defaults_the_rest() {
        let c = Command::RecordStart {
            project: "/Users/me/Movies/Ferrocast Recordings/Demo é.ferrocast".into(),
            file: "screen.mov".into(),
            target: Target::Display { display: 1 },
            fps: 60,
            cursor: true,
            excludes: Excludes::default(),
        };
        let s = encode_request(&Request { id: 5, command: c.clone() });
        assert_eq!(
            s,
            r#"{"id":5,"cmd":"record.start","project":"/Users/me/Movies/Ferrocast Recordings/Demo é.ferrocast","file":"screen.mov","target":{"kind":"display","display":1},"fps":60,"cursor":true}"#
        );
        assert_eq!(serde_json::from_str::<Request>(&s).unwrap().command, c);
        let r: Request =
            serde_json::from_str(r#"{"id":1,"cmd":"record.start","project":"/p","target":{"kind":"window","window":9}}"#).unwrap();
        let Command::RecordStart { file, fps, cursor, excludes, .. } = r.command else { panic!() };
        assert_eq!((file.as_str(), fps, cursor, excludes.window_ids.len()), ("screen.mov", 60, false, 0));
        let with = Command::RecordStart {
            project: "/p".into(),
            file: "a.mov".into(),
            target: Target::Rect { display: 2, rect: [0.0, 0.0, 64.0, 48.0] },
            fps: 30,
            cursor: false,
            excludes: Excludes { window_ids: vec![9001] },
        };
        let s = encode_request(&Request { id: 6, command: with.clone() });
        assert!(s.ends_with(r#""excludes":{"window_ids":[9001]}}"#), "{s}");
        assert_eq!(serde_json::from_str::<Request>(&s).unwrap().command, with);
    }

    #[test]
    fn record_control_commands_are_bare() {
        for (c, name) in
            [(Command::RecordStop, "record.stop"), (Command::RecordPause, "record.pause"), (Command::RecordResume, "record.resume")]
        {
            let s = encode_request(&Request { id: 8, command: c.clone() });
            assert_eq!(s, format!(r#"{{"id":8,"cmd":"{name}"}}"#));
            assert_eq!(serde_json::from_str::<Request>(&s).unwrap().command, c);
        }
    }

    #[test]
    fn record_replies_parse_and_tolerate_unknown_fields() {
        let Line::Ok { body, .. } =
            decode_line(r#"{"id":5,"ok":true,"started_at_host":123456.789,"size":[3456,2234],"file":"screen.mov","x":1}"#).unwrap()
        else {
            panic!()
        };
        let st: RecordStarted = parse_body(&body).unwrap();
        assert_eq!((st.started_at_host, st.size, st.file.as_str()), (123456.789, [3456, 2234], "screen.mov"));
        let Line::Ok { body, .. } = decode_line(
            r#"{"id":8,"ok":true,"project":"/p","duration":73.2,"screen":"screen.mov","size":[3456,2234],"frames":4000,"dropped":2}"#,
        )
        .unwrap() else {
            panic!()
        };
        let d: RecordStopped = parse_body(&body).unwrap();
        assert_eq!((d.duration, d.frames, d.dropped, d.interrupted, d.message), (73.2, 4000, 2, false, None));
        assert!(parse_body::<RecordStopped>(&serde_json::json!({"ok":true})).is_err());
        let Line::Ok { body, .. } = decode_line(
            r#"{"id":8,"ok":true,"project":"/p","duration":1.0,"screen":"screen.mov","size":[2,2],"interrupted":true,"message":"display lost"}"#,
        )
        .unwrap() else {
            panic!()
        };
        let d: RecordStopped = parse_body(&body).unwrap();
        assert!(d.interrupted && d.message.as_deref() == Some("display lost"));
    }

    #[test]
    fn record_events_decode_by_name() {
        let ev = |l: &str| {
            let Line::Event { name, body, .. } = decode_line(l).unwrap() else { panic!() };
            parse_record_event(&name, &body)
        };
        assert_eq!(
            ev(r#"{"event":"tick","id":5,"elapsed":12.5,"frames":700,"dropped":1,"future":0}"#),
            Some(RecordEvent::Tick { elapsed: 12.5, frames: 700, dropped: 1 })
        );
        assert_eq!(
            ev(r#"{"event":"state","id":5,"state":"paused","t":3.0}"#),
            Some(RecordEvent::State { state: RecordState::Paused, message: None })
        );
        assert_eq!(
            ev(r#"{"event":"state","id":5,"state":"failed","message":"display lost"}"#),
            Some(RecordEvent::State { state: RecordState::Failed, message: Some("display lost".into()) })
        );
        assert_eq!(
            ev(r#"{"event":"warn","id":5,"code":"low-disk","free_bytes":900000000}"#),
            Some(RecordEvent::Warn { code: "low-disk".into(), message: None, free_bytes: Some(900_000_000) })
        );
        assert_eq!(ev(r#"{"event":"progress","id":9,"fraction":0.4}"#), None);
        assert_eq!(ev(r#"{"event":"tick","id":5}"#), None);
        assert_eq!(
            ev(r#"{"event":"state","id":5,"state":"brand-new"}"#),
            Some(RecordEvent::State { state: RecordState::Unknown, message: None })
        );
    }

    #[test]
    fn render_frame_request_encodes_project_in_json_and_omits_absent_out() {
        let c = Command::RenderFrame {
            project: "/Users/me/Demo é.ferrocast".into(),
            t: 12.34,
            width: 960,
            out: None,
            params: FrameParams::default(),
        };
        let s = encode_request(&Request { id: 9, command: c.clone() });
        assert_eq!(s, r#"{"id":9,"cmd":"render.frame","project":"/Users/me/Demo é.ferrocast","t":12.34,"width":960}"#);
        assert_eq!(serde_json::from_str::<Request>(&s).unwrap().command, c);
        let with = Command::RenderFrame {
            project: "/p".into(),
            t: 0.0,
            width: 64,
            out: Some("/p/.cache/f-0.png".into()),
            params: FrameParams::default(),
        };
        let s = encode_request(&Request { id: 10, command: with.clone() });
        assert!(s.ends_with(r#""width":64,"out":"/p/.cache/f-0.png"}"#), "{s}");
        assert_eq!(serde_json::from_str::<Request>(&s).unwrap().command, with);
    }

    #[test]
    fn render_frame_request_rejects_missing_required_fields() {
        assert!(serde_json::from_str::<Request>(r#"{"id":1,"cmd":"render.frame","project":"/p","t":1.0}"#).is_err());
        assert!(serde_json::from_str::<Request>(r#"{"id":1,"cmd":"render.frame","t":1.0,"width":10}"#).is_err());
    }

    #[test]
    fn render_frame_reply_parses_and_tolerates_unknown_fields() {
        let Line::Ok { body, .. } =
            decode_line(r#"{"id":9,"ok":true,"path":"/p/.cache/f.png","size":[960,540],"ms":22.5,"cached":true,"future":1}"#).unwrap()
        else {
            panic!()
        };
        let d: RenderFrameDone = parse_body(&body).unwrap();
        assert_eq!((d.path.as_str(), d.size, d.ms, d.cached), ("/p/.cache/f.png", [960, 540], 22.5, true));
        let Line::Ok { body, .. } = decode_line(r#"{"id":9,"ok":true,"path":"/x.png","size":[2,2]}"#).unwrap() else { panic!() };
        let d: RenderFrameDone = parse_body(&body).unwrap();
        assert_eq!((d.ms, d.cached), (0.0, false));
        assert!(parse_body::<RenderFrameDone>(&serde_json::json!({"ok":true})).is_err());
    }

    #[test]
    fn render_frame_without_p3_fields_decodes_to_noop_params() {
        // A T24 request (no camera/cursor/ring/blur) keeps meaning exactly what it meant (S19).
        let r: Request = serde_json::from_str(r#"{"id":9,"cmd":"render.frame","project":"/p","t":1.5,"width":960}"#).unwrap();
        let Command::RenderFrame { params, .. } = r.command else { panic!() };
        assert!(params.is_noop());
    }

    #[test]
    fn render_frame_carries_camera_cursor_ring_and_blur_as_flat_optional_fields() {
        let line = r#"{"id":9,"cmd":"render.frame","project":"/p","t":1.5,"width":960,"camera":{"x":0.25,"y":0.25,"w":0.5,"h":0.5},"cursor":{"x":0.5,"y":0.4,"kind":"ibeam","size":1.4,"opacity":1.0},"ring":{"x":0.3,"y":0.4,"radius":0.01,"alpha":0.5},"blur":{"samples":4,"shutter":0.5,"dir":[1.0,0.0],"length":0.004}}"#;
        let r: Request = serde_json::from_str(line).unwrap();
        let Command::RenderFrame { params, .. } = &r.command else { panic!() };
        assert_eq!(params.camera, Some(Camera { x: 0.25, y: 0.25, w: 0.5, h: 0.5 }));
        assert_eq!(params.cursor.as_ref().map(|c| c.kind.as_str()), Some("ibeam"));
        assert_eq!(params.ring.as_ref().map(|r| r.alpha), Some(0.5));
        assert_eq!(params.blur.as_ref().map(|b| b.samples), Some(4));
        // encode puts them back as top-level keys (no nesting under "params")
        let v: serde_json::Value = serde_json::from_str(&encode_request(&r)).unwrap();
        for k in ["camera", "cursor", "ring", "blur"] {
            assert!(v.get(k).is_some(), "{k} missing");
        }
        assert!(v.get("params").is_none());
        assert_eq!(serde_json::from_str::<Request>(&encode_request(&r)).unwrap(), r);
    }

    fn render_command() -> Command {
        Command::Render {
            project: "/Users/me/Demo é.ferrocast".into(),
            out: "/Users/me/Movies/Demo é.mp4".into(),
            format: Format::Mp4,
            codec: Codec::Hevc,
            width: 3840,
            height: 2160,
            fps: 60,
            quality: Quality::High,
            frames: 4392,
            segments: vec![Segment { src_start: 0.0, src_end: 73.2, speed: 1.0 }],
            params_file: "/Users/me/Demo é.ferrocast/.cache/export-10.ndjson".into(),
        }
    }

    #[test]
    fn render_request_keeps_paths_in_json_and_round_trips() {
        let r = Request { id: 10, command: render_command() };
        let s = encode_request(&r);
        assert_eq!(
            s,
            r#"{"id":10,"cmd":"render","project":"/Users/me/Demo é.ferrocast","out":"/Users/me/Movies/Demo é.mp4","format":"mp4","codec":"hevc","width":3840,"height":2160,"fps":60,"quality":"high","frames":4392,"segments":[{"src_start":0.0,"src_end":73.2,"speed":1.0}],"params_file":"/Users/me/Demo é.ferrocast/.cache/export-10.ndjson"}"#
        );
        assert_eq!(serde_json::from_str::<Request>(&s).unwrap(), r);
        // the request stays small however long the video is: per-frame data is in the file
        assert!(s.len() < 600);
    }

    #[test]
    fn render_request_rejects_unknown_codec_and_missing_fields() {
        let ok = encode_request(&Request { id: 1, command: render_command() });
        assert!(serde_json::from_str::<Request>(&ok.replace("hevc", "vp9")).is_err());
        assert!(serde_json::from_str::<Request>(&ok.replace("\"mp4\"", "\"avi\"")).is_err());
        assert!(serde_json::from_str::<Request>(&ok.replace(r#""frames":4392,"#, "")).is_err());
    }

    #[test]
    fn cancel_request_names_its_target() {
        let r = Request { id: 11, command: Command::Cancel { target: 10 } };
        let s = encode_request(&r);
        assert_eq!(s, r#"{"id":11,"cmd":"cancel","target":10}"#);
        assert_eq!(serde_json::from_str::<Request>(&s).unwrap(), r);
    }

    fn event(l: &str) -> Option<ExportEvent> {
        let Line::Event { name, body, .. } = decode_line(l).unwrap() else { panic!() };
        parse_export_event(&name, &body)
    }

    #[test]
    fn export_events_decode_by_name() {
        assert_eq!(event(r#"{"event":"started","id":10,"total_frames":4392}"#), Some(ExportEvent::Started { total_frames: 4392 }));
        assert_eq!(
            event(r#"{"event":"progress","id":10,"done_frames":100,"total_frames":4392,"eta":18.5,"future":1}"#),
            Some(ExportEvent::Progress { done_frames: 100, total_frames: 4392, eta: Some(18.5) })
        );
        assert_eq!(
            event(r#"{"event":"progress","id":10,"done_frames":0,"total_frames":10}"#),
            Some(ExportEvent::Progress { done_frames: 0, total_frames: 10, eta: None })
        );
        assert_eq!(event(r#"{"event":"heartbeat","id":10}"#), Some(ExportEvent::Heartbeat));
        // not export events / malformed
        assert_eq!(event(r#"{"event":"tick","id":5,"elapsed":1.0,"frames":1,"dropped":0}"#), None);
        assert_eq!(event(r#"{"event":"progress","id":10,"done_frames":"x","total_frames":1}"#), None);
        assert_eq!(event(r#"{"event":"started","id":10}"#), None);
        assert!(!ExportEvent::Heartbeat.is_terminal() && ExportEvent::Cancelled.is_terminal());
    }

    #[test]
    fn export_final_maps_ok_cancel_and_errors() {
        let f = |l: &str| export_final(&decode_line(l).unwrap());
        assert_eq!(
            f(r#"{"id":10,"ok":true,"path":"/o.mp4","bytes":88000000,"duration":73.2,"frames":4392}"#),
            Some(ExportEvent::Done(ExportDone { path: "/o.mp4".into(), bytes: 88_000_000, duration: 73.2, frames: 4392 }))
        );
        assert_eq!(f(r#"{"id":10,"error":"cancelled","message":"Export cancelled"}"#), Some(ExportEvent::Cancelled));
        assert_eq!(
            f(r#"{"id":10,"error":"disk-full","message":"No space left"}"#),
            Some(ExportEvent::Failed { code: ErrorCode::DiskFull, message: "No space left".into() })
        );
        assert_eq!(f(r#"{"event":"progress","id":10,"done_frames":1,"total_frames":2}"#), None);
        assert_eq!(f(r#"{"id":10,"ok":true}"#), None, "an ok without the export fields is not a result");
    }

    #[test]
    fn render_frame_partial_p3_fields_leave_the_others_absent() {
        let r: Request =
            serde_json::from_str(r#"{"id":1,"cmd":"render.frame","project":"/p","t":0,"width":64,"camera":{"x":0,"y":0,"w":0.5,"h":0.5}}"#)
                .unwrap();
        let s = encode_request(&r);
        assert!(s.contains(r#""camera""#) && !s.contains("cursor") && !s.contains("ring") && !s.contains("blur"), "{s}");
    }
}
