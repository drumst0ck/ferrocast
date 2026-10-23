//! Client for the `ferrocast-capture` Swift helper (NDJSON over stdin/stdout, see
//! `ferrocast_core::capture::protocol`). Text and paths never travel in argv.

use anyhow::{anyhow, bail, Result};
use ferrocast_core::capture::compose::FrameParams;
use ferrocast_core::capture::protocol::{
    self, Command, ErrorCode, Excludes, Hello, Line, RecordEvent, RecordStarted, RecordStopped, RenderFrameDone, Request, Scale,
    ScreenshotDone, Shareable, Target,
};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const HELPER_NAME: &str = "ferrocast-capture";

/// Where the helper lives: next to the running executable. Pass the exe already resolved through
/// `instance::resolve_exe` (that function lives in the app crate).
pub fn helper_path(exe: &Path) -> PathBuf {
    exe.with_file_name(HELPER_NAME)
}

fn is_executable(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// Screen capture is offered on macOS 14+ with the helper present.
pub fn available(exe: &Path) -> bool {
    crate::system::os_at_least(14) && is_executable(&helper_path(exe))
}

/// Per-command reply timeout.
pub fn timeout_for(cmd: &Command) -> Duration {
    Duration::from_secs(match cmd {
        Command::Hello => 5,
        Command::Shareable => 10,
        Command::Screenshot { .. } => 20,
        // A preview frame includes loading the asset on the first request (220 ms measured), far below this.
        Command::RenderFrame { .. } => 10,
        Command::RecordStart { .. } | Command::RecordPause | Command::RecordResume => 10,
        // The helper may wait 5 s for stopCapture and 8 s for finishWriting: the client must outlast both,
        // otherwise it kills the helper and the recording is lost.
        Command::RecordStop => 20,
    })
}

/// A request that did not produce an `ok` reply.
#[derive(Debug)]
pub enum CallError {
    Helper { code: ErrorCode, message: String },
    Timeout,
    Closed,
}

impl std::fmt::Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CallError::Helper { code, message } => write!(f, "capture helper error {code:?}: {message}"),
            CallError::Timeout => write!(f, "capture helper timed out"),
            CallError::Closed => write!(f, "capture helper closed"),
        }
    }
}
impl std::error::Error for CallError {}

/// Why a screenshot failed. `PermissionDenied` is the S4 case: the app shows the
/// Screen Recording toast and opens the Settings pane.
#[derive(Debug, PartialEq, Eq)]
pub enum ScreenshotError {
    PermissionDenied,
    NotFound(String),
    Unsupported,
    Timeout,
    Other(String),
}

impl From<CallError> for ScreenshotError {
    fn from(e: CallError) -> Self {
        match e {
            CallError::Helper { code: ErrorCode::PermissionScreen, .. } => Self::PermissionDenied,
            CallError::Helper { code: ErrorCode::NotFound, message } => Self::NotFound(message),
            CallError::Helper { code: ErrorCode::UnsupportedOs, .. } => Self::Unsupported,
            CallError::Helper { code, message } => Self::Other(format!("{code:?}: {message}")),
            CallError::Timeout => Self::Timeout,
            CallError::Closed => Self::Other("capture helper closed".into()),
        }
    }
}

impl std::fmt::Display for ScreenshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PermissionDenied => write!(f, "Screen Recording is not granted"),
            Self::NotFound(m) => write!(f, "capture target not found: {m}"),
            Self::Unsupported => write!(f, "screen capture needs macOS 14"),
            Self::Timeout => write!(f, "screenshot timed out"),
            Self::Other(m) => write!(f, "screenshot failed: {m}"),
        }
    }
}
impl std::error::Error for ScreenshotError {}

/// Why a recording request failed. `PermissionDenied` is the S4 case (toast + Settings pane).
#[derive(Debug, PartialEq, Eq)]
pub enum RecordError {
    PermissionDenied,
    NotFound(String),
    Unsupported,
    /// A recording is already running in this helper.
    Busy,
    Timeout,
    /// The helper exited; a recording in progress is lost.
    Closed,
    Other(String),
}

impl From<CallError> for RecordError {
    fn from(e: CallError) -> Self {
        match e {
            CallError::Helper { code: ErrorCode::PermissionScreen, .. } => Self::PermissionDenied,
            CallError::Helper { code: ErrorCode::NotFound, message } => Self::NotFound(message),
            CallError::Helper { code: ErrorCode::UnsupportedOs, .. } => Self::Unsupported,
            CallError::Helper { code: ErrorCode::Busy, .. } => Self::Busy,
            CallError::Helper { code, message } => Self::Other(format!("{code:?}: {message}")),
            CallError::Timeout => Self::Timeout,
            CallError::Closed => Self::Closed,
        }
    }
}

impl std::fmt::Display for RecordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PermissionDenied => write!(f, "Screen Recording is not granted"),
            Self::NotFound(m) => write!(f, "recording target not found: {m}"),
            Self::Unsupported => write!(f, "screen recording needs macOS 14"),
            Self::Busy => write!(f, "a recording is already running"),
            Self::Timeout => write!(f, "recording request timed out"),
            Self::Closed => write!(f, "capture helper closed during the recording"),
            Self::Other(m) => write!(f, "recording failed: {m}"),
        }
    }
}
impl std::error::Error for RecordError {}

/// A preview frame the helper rendered (`render.frame`).
#[derive(Debug, Clone, PartialEq)]
pub struct RenderedFrame {
    pub path: PathBuf,
    pub size: [u32; 2],
    /// Time the helper spent (0 when it did not say).
    pub ms: f64,
    /// Served from the helper's frame cache.
    pub cached: bool,
}

/// Why a preview frame could not be rendered. `Display` carries the helper's own message so the
/// editor can show it.
#[derive(Debug, PartialEq, Eq)]
pub enum RenderError {
    /// The request or the project was refused (bad project.json, width 0…).
    Invalid(String),
    /// A take is missing.
    NotFound(String),
    Timeout,
    /// The helper exited (the next request needs a new helper).
    Closed,
    Other(String),
}

impl From<CallError> for RenderError {
    fn from(e: CallError) -> Self {
        match e {
            CallError::Helper { code: ErrorCode::Invalid, message } => Self::Invalid(message),
            CallError::Helper { code: ErrorCode::NotFound, message } => Self::NotFound(message),
            CallError::Helper { code, message } => Self::Other(format!("{code:?}: {message}")),
            CallError::Timeout => Self::Timeout,
            CallError::Closed => Self::Closed,
        }
    }
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(m) | Self::NotFound(m) => write!(f, "{m}"),
            Self::Timeout => write!(f, "the preview timed out"),
            Self::Closed => write!(f, "the capture helper closed"),
            Self::Other(m) => write!(f, "preview failed: {m}"),
        }
    }
}
impl std::error::Error for RenderError {}

/// `<project>/<file>.partial`: what the helper writes until a clean stop renames it.
fn recording_partial(project: &Path, file: &str) -> PathBuf {
    partial_path(&project.join(file))
}

/// Temp file the helper writes before renaming to the final path.
fn partial_path(out: &Path) -> PathBuf {
    let mut s = out.as_os_str().to_owned();
    s.push(".partial");
    PathBuf::from(s)
}

/// Take one screenshot with a short-lived helper (see [`Helper::screenshot`]).
pub fn screenshot(
    exe: &Path,
    log: impl Fn(&str) + Send + 'static,
    target: Target,
    out: &Path,
    cursor: bool,
) -> std::result::Result<ScreenshotDone, ScreenshotError> {
    if !available(exe) {
        return Err(ScreenshotError::Unsupported);
    }
    let mut h = Helper::spawn(&helper_path(exe), log).map_err(|e| ScreenshotError::Other(e.to_string()))?;
    h.screenshot(target, out, cursor)
}

pub struct Helper {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<Line>,
    next_id: u64,
    /// Where recording events go; set by `record_start`, cleared on stop and when stdout closes.
    recording_events: Arc<Mutex<Option<Sender<RecordEvent>>>>,
    /// Partial file of the recording in progress, removed whenever the recording is lost.
    recording_partial: Option<PathBuf>,
}

impl Helper {
    /// Start the helper; every stderr line goes to `log` (the app appends it to `ferrocast.log`).
    pub fn spawn(path: &Path, log: impl Fn(&str) + Send + 'static) -> Result<Helper> {
        let mut child = std::process::Command::new(path).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
        let stdin = child.stdin.take().ok_or_else(|| anyhow!("no stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("no stdout"))?;
        let stderr = child.stderr.take().ok_or_else(|| anyhow!("no stderr"))?;
        let (tx, lines) = channel();
        let recording_events: Arc<Mutex<Option<Sender<RecordEvent>>>> = Arc::default();
        let subscriber = recording_events.clone();
        std::thread::spawn(move || {
            for l in BufReader::new(stdout).lines().map_while(|l| l.ok()) {
                match protocol::decode_line(&l) {
                    Ok(line) => {
                        if let Line::Event { name, body, .. } = &line {
                            if let Some(ev) = protocol::parse_record_event(name, body) {
                                if let Some(sink) = subscriber.lock().unwrap().as_ref() {
                                    let _ = sink.send(ev);
                                    continue;
                                }
                            }
                        }
                        if tx.send(line).is_err() {
                            break;
                        }
                    }
                    Err(e) => eprintln!("capture helper: bad line ({e}): {l}"),
                }
            }
            // stdout closed: end the event stream so receivers see the disconnect.
            subscriber.lock().unwrap().take();
        });
        std::thread::spawn(move || {
            for l in BufReader::new(stderr).lines().map_while(|l| l.ok()) {
                log(&l);
            }
        });
        Ok(Helper { child, stdin, lines, next_id: 1, recording_events, recording_partial: None })
    }

    /// Send one command and wait for its reply body; kills the helper on timeout.
    pub fn call(&mut self, command: Command, timeout: Duration) -> std::result::Result<protocol::Value, CallError> {
        let id = self.next_id;
        self.next_id += 1;
        let line = protocol::encode_request(&Request { id, command });
        if writeln!(self.stdin, "{line}").and_then(|_| self.stdin.flush()).is_err() {
            return Err(CallError::Closed);
        }
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.lines.recv_timeout(left) {
                Ok(Line::Ok { id: rid, body }) if rid == id => return Ok(body),
                Ok(Line::Error { id: rid, code, message }) if rid.is_none_or(|r| r == id) => {
                    return Err(CallError::Helper { code, message })
                }
                Ok(_) => {}
                Err(RecvTimeoutError::Timeout) => {
                    self.kill();
                    return Err(CallError::Timeout);
                }
                Err(RecvTimeoutError::Disconnected) => return Err(CallError::Closed),
            }
        }
    }

    pub fn hello(&mut self) -> Result<Hello> {
        let body = self.call(Command::Hello, timeout_for(&Command::Hello))?;
        protocol::parse_body(&body).map_err(|e| anyhow!(e))
    }

    pub fn shareable(&mut self) -> Result<Shareable> {
        let body = self.call(Command::Shareable, timeout_for(&Command::Shareable))?;
        protocol::parse_body(&body).map_err(|e| anyhow!(e))
    }

    /// Capture `target` into the PNG at `out`. On any failure no file (nor partial) is left at `out`.
    pub fn screenshot(&mut self, target: Target, out: &Path, cursor: bool) -> std::result::Result<ScreenshotDone, ScreenshotError> {
        let command =
            Command::Screenshot { target, out: out.to_string_lossy().into_owned(), cursor, window_shadow: false, scale: Scale::Native };
        let timeout = timeout_for(&command);
        let result = self.call(command, timeout).map_err(ScreenshotError::from).and_then(|body| {
            protocol::parse_body::<ScreenshotDone>(&body).map_err(|e| ScreenshotError::Other(format!("bad screenshot reply: {e}")))
        });
        if result.is_err() {
            let _ = std::fs::remove_file(partial_path(out));
        }
        result
    }

    /// Render the preview frame of `project_dir` at SOURCE time `t_source` (map output time with
    /// `timeline` first), `width` px wide, with the camera/cursor/ring/blur `params` that
    /// `capture::compose::Composer` computed. The helper stays alive between calls and caches assets.
    pub fn render_frame(
        &mut self,
        project_dir: &Path,
        t_source: f64,
        width: u32,
        params: FrameParams,
    ) -> std::result::Result<RenderedFrame, RenderError> {
        if width == 0 {
            return Err(RenderError::Invalid("preview width must be at least 1 px".into()));
        }
        let command = Command::RenderFrame { project: project_dir.to_string_lossy().into_owned(), t: t_source, width, out: None, params };
        let timeout = timeout_for(&command);
        let body = self.call(command, timeout).map_err(RenderError::from)?;
        let done =
            protocol::parse_body::<RenderFrameDone>(&body).map_err(|e| RenderError::Other(format!("bad render.frame reply: {e}")))?;
        Ok(RenderedFrame { path: PathBuf::from(done.path), size: done.size, ms: done.ms, cached: done.cached })
    }

    /// Start recording `target` into `<project>/<file>`. Events (`state`, `tick`, `warn`) arrive on
    /// the returned channel until the recording stops or the helper exits. On failure no partial
    /// file is left. The helper keeps running: call [`Helper::record_stop`] to finish.
    pub fn record_start(
        &mut self,
        project: &Path,
        file: &str,
        target: Target,
        fps: u32,
        cursor: bool,
        exclude_windows: Vec<u32>,
    ) -> std::result::Result<(RecordStarted, Receiver<RecordEvent>), RecordError> {
        if file.is_empty() || file == "." || file == ".." || file.contains(['/', '\0']) {
            return Err(RecordError::Other(format!("invalid recording file name: {file:?}")));
        }
        let command = Command::RecordStart {
            project: project.to_string_lossy().into_owned(),
            file: file.to_string(),
            target,
            fps,
            cursor,
            excludes: Excludes { window_ids: exclude_windows },
        };
        let timeout = timeout_for(&command);
        let (tx, rx) = channel();
        *self.recording_events.lock().unwrap() = Some(tx);
        let partial = recording_partial(project, file);
        let result = self.call(command, timeout).map_err(RecordError::from).and_then(|body| {
            protocol::parse_body::<RecordStarted>(&body).map_err(|e| RecordError::Other(format!("bad record.start reply: {e}")))
        });
        match result {
            Ok(started) => {
                self.recording_partial = Some(partial);
                Ok((started, rx))
            }
            Err(e) => {
                self.recording_events.lock().unwrap().take();
                let _ = std::fs::remove_file(partial);
                Err(e)
            }
        }
    }

    fn record_control(&mut self, command: Command) -> std::result::Result<protocol::Value, RecordError> {
        let timeout = timeout_for(&command);
        self.call(command, timeout).map_err(|e| {
            let e = RecordError::from(e);
            if matches!(e, RecordError::Closed | RecordError::Timeout) {
                self.discard_recording();
            }
            e
        })
    }

    /// The recording is gone (helper exited or was killed): close the event stream, drop the partial.
    fn discard_recording(&mut self) {
        self.recording_events.lock().unwrap().take();
        if let Some(p) = self.recording_partial.take() {
            let _ = std::fs::remove_file(p);
        }
    }

    pub fn record_pause(&mut self) -> std::result::Result<(), RecordError> {
        self.record_control(Command::RecordPause).map(|_| ())
    }

    pub fn record_resume(&mut self) -> std::result::Result<(), RecordError> {
        self.record_control(Command::RecordResume).map(|_| ())
    }

    /// Finish the recording: `screen.mov` is complete when this returns `Ok`. If the helper dies
    /// or hangs, the partial file is removed and the error says so.
    pub fn record_stop(&mut self) -> std::result::Result<RecordStopped, RecordError> {
        let body = self.record_control(Command::RecordStop)?;
        self.recording_partial = None;
        let done = protocol::parse_body::<RecordStopped>(&body).map_err(|e| RecordError::Other(format!("bad record.stop reply: {e}")));
        // Events the helper emitted before the reply are already queued; closing the sender ends the stream.
        self.recording_events.lock().unwrap().take();
        done
    }

    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        self.kill();
    }
}

/// Run `f` against a short-lived helper.
pub fn with_helper<T>(exe: &Path, log: impl Fn(&str) + Send + 'static, f: impl FnOnce(&mut Helper) -> Result<T>) -> Result<T> {
    if !available(exe) {
        bail!("screen capture is not available");
    }
    f(&mut Helper::spawn(&helper_path(exe), log)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrocast_core::capture::protocol::RecordState;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::{Arc, Mutex};

    fn fake(name: &str, body: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fc-capture-test-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("helper.sh");
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    #[test]
    fn helper_path_is_next_to_exe() {
        assert_eq!(
            helper_path(Path::new("/A/Ferrocast.app/Contents/MacOS/ferrocast")),
            PathBuf::from("/A/Ferrocast.app/Contents/MacOS/ferrocast-capture")
        );
        assert!(!available(Path::new("/nonexistent/ferrocast")));
    }

    #[test]
    fn frames_replies_and_logs_stderr() {
        let p = fake(
            "frame",
            r#"while read l; do echo noise >&2; echo '{"event":"x","id":1}'; echo '{"id":1,"ok":true,"protocol":1,"os":"14.0.0","version":"t","sck":true,"metal":true}'; done"#,
        );
        let logs = Arc::new(Mutex::new(Vec::new()));
        let l2 = logs.clone();
        let mut h = Helper::spawn(&p, move |s| l2.lock().unwrap().push(s.to_string())).unwrap();
        let hello = h.hello().unwrap();
        assert_eq!(hello.os, "14.0.0");
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(logs.lock().unwrap().as_slice(), ["noise"]);
    }

    #[test]
    fn helper_error_is_structured() {
        let p = fake("err", r#"read l; echo '{"id":1,"error":"permission-screen","message":"off"}'; exit 1"#);
        let mut h = Helper::spawn(&p, |_| {}).unwrap();
        match h.call(Command::Shareable, Duration::from_secs(3)) {
            Err(CallError::Helper { code, message }) => assert_eq!((code, message.as_str()), (ErrorCode::PermissionScreen, "off")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn screenshot_permission_denied_maps_and_leaves_no_file() {
        let p = fake("shot-perm", r#"read l; echo '{"id":1,"error":"permission-screen","message":"off"}'; exit 1"#);
        let out = p.with_file_name("a.png");
        std::fs::write(partial_path(&out), b"half").unwrap();
        let mut h = Helper::spawn(&p, |_| {}).unwrap();
        let r = h.screenshot(Target::Display { display: 1 }, &out, false);
        assert_eq!(r.unwrap_err(), ScreenshotError::PermissionDenied);
        assert!(!out.exists() && !partial_path(&out).exists());
    }

    #[test]
    fn screenshot_ok_and_other_errors_map() {
        let p = fake("shot-ok", r#"read l; echo '{"id":1,"ok":true,"path":"/x.png","size":[1280,800],"scale":2.0}'"#);
        let d = Helper::spawn(&p, |_| {}).unwrap().screenshot(Target::Window { window: 9 }, Path::new("/x.png"), false).unwrap();
        assert_eq!((d.size, d.scale), ([1280, 800], 2.0));
        let p = fake("shot-nf", r#"read l; echo '{"id":1,"error":"not-found","message":"window 9"}'"#);
        let e = Helper::spawn(&p, |_| {}).unwrap().screenshot(Target::Window { window: 9 }, Path::new("/x.png"), false).unwrap_err();
        assert_eq!(e, ScreenshotError::NotFound("window 9".into()));
        let p = fake("shot-int", r#"read l; echo '{"id":1,"error":"internal","message":"boom"}'"#);
        let e = Helper::spawn(&p, |_| {}).unwrap().screenshot(Target::Window { window: 9 }, Path::new("/x.png"), false).unwrap_err();
        assert!(matches!(e, ScreenshotError::Other(m) if m.contains("boom")));
    }

    #[test]
    fn timeout_kills_the_helper() {
        let p = fake("hang", "read l; sleep 30");
        let mut h = Helper::spawn(&p, |_| {}).unwrap();
        let t = Instant::now();
        assert!(matches!(h.call(Command::Hello, Duration::from_millis(300)), Err(CallError::Timeout)));
        assert!(t.elapsed() < Duration::from_secs(5));
        assert!(h.child.try_wait().unwrap().is_some());
    }

    #[test]
    fn closed_helper_reports_closed() {
        let p = fake("exit", "exit 0");
        let mut h = Helper::spawn(&p, |_| {}).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        assert!(matches!(h.call(Command::Hello, Duration::from_secs(2)), Err(CallError::Closed)));
    }

    fn dir_of(p: &Path) -> PathBuf {
        p.parent().unwrap().to_path_buf()
    }

    #[test]
    fn recording_streams_events_and_stops_with_a_typed_reply() {
        let p = fake(
            "rec-ok",
            r#"read l; echo "$l" > "$(dirname "$0")/start.json"
echo '{"id":1,"ok":true,"started_at_host":10.5,"size":[200,100],"file":"screen.mov"}'
echo '{"event":"state","id":1,"state":"recording","t":0.0}'
echo '{"event":"tick","id":1,"elapsed":1.0,"frames":3,"dropped":0}'
read l; echo '{"id":2,"ok":true}'
read l; echo '{"id":3,"ok":true}'
read l; echo '{"event":"state","id":1,"state":"stopped"}'; echo '{"id":4,"ok":true,"project":"/p","duration":2.0,"screen":"screen.mov","size":[200,100],"frames":9,"dropped":1}'"#,
        );
        let project = dir_of(&p).join("My Demo.ferrocast");
        let mut h = Helper::spawn(&p, |_| {}).unwrap();
        let (started, events) = h.record_start(&project, "screen.mov", Target::Display { display: 1 }, 60, true, Vec::new()).unwrap();
        assert_eq!((started.started_at_host, started.size), (10.5, [200, 100]));
        assert_eq!(
            events.recv_timeout(Duration::from_secs(2)).unwrap(),
            RecordEvent::State { state: RecordState::Recording, message: None }
        );
        assert_eq!(events.recv_timeout(Duration::from_secs(2)).unwrap(), RecordEvent::Tick { elapsed: 1.0, frames: 3, dropped: 0 });
        h.record_pause().unwrap();
        h.record_resume().unwrap();
        let done = h.record_stop().unwrap();
        assert_eq!((done.duration, done.frames, done.dropped), (2.0, 9, 1));
        assert_eq!(events.recv_timeout(Duration::from_secs(2)).unwrap(), RecordEvent::State { state: RecordState::Stopped, message: None });
        assert!(events.recv_timeout(Duration::from_secs(2)).is_err(), "channel closes after stop");
        let wire = std::fs::read_to_string(dir_of(&p).join("start.json")).unwrap();
        assert!(
            wire.contains(r#""cmd":"record.start""#) && wire.contains("My Demo.ferrocast") && wire.contains(r#""cursor":true"#),
            "{wire}"
        );
    }

    #[test]
    fn recording_permission_denied_maps_and_removes_the_partial() {
        let p = fake("rec-perm", r#"read l; echo '{"id":1,"error":"permission-screen","message":"off"}'; exit 1"#);
        let project = dir_of(&p).join("perm.ferrocast");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(project.join("screen.mov.partial"), b"half").unwrap();
        let mut h = Helper::spawn(&p, |_| {}).unwrap();
        let e = h.record_start(&project, "screen.mov", Target::Display { display: 1 }, 60, false, Vec::new()).unwrap_err();
        assert_eq!(e, RecordError::PermissionDenied);
        assert!(!project.join("screen.mov.partial").exists() && !project.join("screen.mov").exists());
    }

    #[test]
    fn recording_busy_and_not_found_map() {
        let p = fake("rec-busy", r#"read l; echo '{"id":1,"error":"busy","message":"already recording"}'"#);
        let e = Helper::spawn(&p, |_| {})
            .unwrap()
            .record_start(&dir_of(&p), "screen.mov", Target::Window { window: 9 }, 30, false, vec![1])
            .unwrap_err();
        assert_eq!(e, RecordError::Busy);
        let p = fake("rec-nf", r#"read l; echo '{"id":1,"error":"not-found","message":"display 7 not found"}'"#);
        let e = Helper::spawn(&p, |_| {})
            .unwrap()
            .record_start(&dir_of(&p), "screen.mov", Target::Display { display: 7 }, 30, false, Vec::new())
            .unwrap_err();
        assert_eq!(e, RecordError::NotFound("display 7 not found".into()));
    }

    #[test]
    fn helper_dying_mid_recording_reports_closed_and_leaves_no_partial() {
        let p = fake(
            "rec-die",
            r#"read l; echo '{"id":1,"ok":true,"started_at_host":1.0}'
echo '{"event":"tick","id":1,"elapsed":1.0,"frames":1,"dropped":0}'
read l; exit 1"#,
        );
        let project = dir_of(&p).join("die.ferrocast");
        std::fs::create_dir_all(&project).unwrap();
        let mut h = Helper::spawn(&p, |_| {}).unwrap();
        let (_, events) = h.record_start(&project, "screen.mov", Target::Display { display: 1 }, 60, false, Vec::new()).unwrap();
        std::fs::write(project.join("screen.mov.partial"), b"half").unwrap();
        assert_eq!(h.record_stop().unwrap_err(), RecordError::Closed);
        assert!(!project.join("screen.mov.partial").exists());
        assert!(matches!(events.recv_timeout(Duration::from_secs(2)), Ok(RecordEvent::Tick { .. })));
        assert!(events.recv_timeout(Duration::from_secs(2)).is_err());
    }

    #[test]
    fn recording_file_name_must_be_a_plain_name() {
        let p = fake("rec-name", "read l; sleep 5");
        let mut h = Helper::spawn(&p, |_| {}).unwrap();
        for bad in ["../x.mov", "a/b.mov", "", ".."] {
            let e = h.record_start(&dir_of(&p), bad, Target::Display { display: 1 }, 60, false, Vec::new()).unwrap_err();
            assert!(matches!(e, RecordError::Other(_)), "{bad}: {e:?}");
        }
    }

    #[test]
    fn non_recording_events_do_not_reach_the_recording_channel() {
        let p = fake(
            "rec-other",
            r#"read l; echo '{"event":"progress","id":9,"fraction":0.5}'; echo '{"id":1,"ok":true,"started_at_host":2.0}'"#,
        );
        let mut h = Helper::spawn(&p, |_| {}).unwrap();
        let (_, events) = h.record_start(&dir_of(&p), "screen.mov", Target::Display { display: 1 }, 60, false, Vec::new()).unwrap();
        assert!(events.recv_timeout(Duration::from_millis(300)).is_err());
    }
}

#[cfg(test)]
mod render_frame_tests {
    use super::*;
    use ferrocast_core::capture::compose::{Camera, FrameParams};
    use std::os::unix::fs::PermissionsExt;

    fn fake(name: &str, body: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fc-render-test-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("helper.sh");
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    #[test]
    fn render_frame_parses_the_reply_and_sends_params_on_the_wire() {
        let p = fake(
            "ok",
            r#"read l; echo "$l" > "$(dirname "$0")/req.json"
echo '{"id":1,"ok":true,"path":"/p/.cache/f-1.png","size":[960,540],"ms":12.5,"cached":true}'"#,
        );
        let mut h = Helper::spawn(&p, |_| {}).unwrap();
        let params = FrameParams { camera: Some(Camera { x: 0.25, y: 0.25, w: 0.5, h: 0.5 }), ..FrameParams::default() };
        let f = h.render_frame(Path::new("/p/Demo.ferrocast"), 1.5, 960, params).unwrap();
        assert_eq!(f, RenderedFrame { path: PathBuf::from("/p/.cache/f-1.png"), size: [960, 540], ms: 12.5, cached: true });
        let wire = std::fs::read_to_string(p.with_file_name("req.json")).unwrap();
        assert!(wire.contains(r#""cmd":"render.frame""#) && wire.contains("Demo.ferrocast") && wire.contains(r#""width":960"#), "{wire}");
        assert!(wire.contains(r#""camera":{"x":0.25"#) && wire.contains(r#""t":1.5"#), "{wire}");
    }

    #[test]
    fn one_long_lived_helper_serves_many_frames() {
        let p =
            fake("many", r#"n=0; while read l; do n=$((n+1)); echo "{\"id\":$n,\"ok\":true,\"path\":\"/f$n.png\",\"size\":[10,5]}"; done"#);
        let mut h = Helper::spawn(&p, |_| {}).unwrap();
        for i in 1..=3 {
            let f = h.render_frame(Path::new("/p"), i as f64, 10, FrameParams::default()).unwrap();
            assert_eq!(f.path, PathBuf::from(format!("/f{i}.png")));
            assert!(!f.cached && f.ms == 0.0, "missing optional fields default");
        }
    }

    #[test]
    fn helper_failures_map_and_keep_the_helpers_message() {
        let p = fake("inv", r#"read l; echo '{"id":1,"error":"invalid","message":"project.json: bad zoom"}'"#);
        let e = Helper::spawn(&p, |_| {}).unwrap().render_frame(Path::new("/p"), 0.0, 960, FrameParams::default()).unwrap_err();
        assert_eq!(e, RenderError::Invalid("project.json: bad zoom".into()));
        assert!(e.to_string().contains("bad zoom"), "{e}");
        let p = fake("nf", r#"read l; echo '{"id":1,"error":"not-found","message":"screen.mov is missing"}'"#);
        let e = Helper::spawn(&p, |_| {}).unwrap().render_frame(Path::new("/p"), 0.0, 960, FrameParams::default()).unwrap_err();
        assert_eq!(e, RenderError::NotFound("screen.mov is missing".into()));
        let p = fake("int", r#"read l; echo '{"id":1,"error":"internal","message":"boom"}'"#);
        let e = Helper::spawn(&p, |_| {}).unwrap().render_frame(Path::new("/p"), 0.0, 960, FrameParams::default()).unwrap_err();
        assert!(matches!(&e, RenderError::Other(m) if m.contains("boom")), "{e:?}");
    }

    #[test]
    fn a_dead_helper_is_closed_and_a_bad_reply_is_other() {
        let p = fake("dead", "exit 0");
        let mut h = Helper::spawn(&p, |_| {}).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(h.render_frame(Path::new("/p"), 0.0, 960, FrameParams::default()).unwrap_err(), RenderError::Closed);
        let p = fake("bad", r#"read l; echo '{"id":1,"ok":true}'"#);
        let e = Helper::spawn(&p, |_| {}).unwrap().render_frame(Path::new("/p"), 0.0, 960, FrameParams::default()).unwrap_err();
        assert!(matches!(e, RenderError::Other(_)), "{e:?}");
    }

    #[test]
    fn a_zero_width_is_refused_before_the_helper_is_asked() {
        let p = fake("zero", "read l; sleep 5");
        let e = Helper::spawn(&p, |_| {}).unwrap().render_frame(Path::new("/p"), 0.0, 0, FrameParams::default()).unwrap_err();
        assert!(matches!(e, RenderError::Invalid(_)), "{e:?}");
    }
}

#[cfg(test)]
mod stop_timeout_tests {
    use super::*;

    #[test]
    fn record_stop_outlasts_the_helpers_own_stop_and_finish_waits() {
        // 5 s stopCapture + 8 s finishWriting inside the helper.
        assert!(timeout_for(&Command::RecordStop) > Duration::from_secs(13));
    }
}
