//! NDJSON protocol of the `ferrocast-capture` helper: pure serde types, no I/O.

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
}
