//! Client for the `ferrocast-capture` Swift helper (NDJSON over stdin/stdout, see
//! `ferrocast_core::capture::protocol`). Text and paths never travel in argv.

use anyhow::{anyhow, bail, Result};
use ferrocast_core::capture::compose::FrameParams;
use ferrocast_core::capture::events::Events;
use ferrocast_core::capture::export::{self, ExportOptions, ExportPlan};
use ferrocast_core::capture::project::{self as capture_project, Project};
use ferrocast_core::capture::protocol::{
    self, Command, DeviceChoice, Devices, ErrorCode, Excludes, ExportDone, ExportEvent, ExportStarted, FrameFormat, Hello, Line, MediaKind,
    MediaPermissions, RecordEvent, RecordStarted, RecordStopped, RenderFrameDone, Request, Scale, ScreenshotDone, Shareable, Target,
};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const HELPER_NAME: &str = "ferrocast-capture";

/// S12: the toast for the helper error `permission-camera` (the app shows it and may open [`CAMERA_PANE`]).
pub const CAMERA_PERMISSION_TOAST: &str =
    "Camera is off for Ferrocast. Turn it on in System Settings \u{2192} Privacy & Security \u{2192} Camera.";
/// S12: the toast for the helper error `permission-microphone` (the app may open [`MICROPHONE_PANE`]).
pub const MICROPHONE_PERMISSION_TOAST: &str =
    "Microphone is off for Ferrocast. Turn it on in System Settings \u{2192} Privacy & Security \u{2192} Microphone.";
/// Settings panes for the S12 toasts.
pub const CAMERA_PANE: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_Camera";
pub const MICROPHONE_PANE: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone";

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
        Command::Shareable | Command::Devices => 10,
        // `permissions` may show the system prompt and waits for the user's answer.
        Command::Permissions { .. } => 120,
        Command::Screenshot { .. } => 20,
        // A preview frame includes loading the asset on the first request (220 ms measured), far below this.
        Command::RenderFrame { .. } => 10,
        Command::RecordStart { .. } | Command::RecordPause | Command::RecordResume => 10,
        // The helper may wait 5 s for stopCapture and 8 s for finishWriting: the client must outlast both,
        // otherwise it kills the helper and the recording is lost.
        Command::RecordStop => 20,
        // `render`: only the wait for `started` (the helper validates and opens the writer); the
        // export itself has no deadline, `Helper::export_wait` watches its heartbeats instead.
        Command::Render { .. } => 15,
        Command::Cancel { .. } => 5,
        // `transcribe` has no reply deadline: `Helper::transcribe` watches for signs of life instead.
        Command::Transcribe { .. } => TRANSCRIBE_SILENCE.as_secs(),
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
    /// S12: Camera access is off; show [`CAMERA_PERMISSION_TOAST`].
    PermissionCamera,
    /// S12: Microphone access is off; show [`MICROPHONE_PERMISSION_TOAST`].
    PermissionMicrophone,
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
            CallError::Helper { code: ErrorCode::PermissionCamera, .. } => Self::PermissionCamera,
            CallError::Helper { code: ErrorCode::PermissionMic, .. } => Self::PermissionMicrophone,
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
            Self::PermissionCamera => f.write_str(CAMERA_PERMISSION_TOAST),
            Self::PermissionMicrophone => f.write_str(MICROPHONE_PERMISSION_TOAST),
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
    /// `Some(bytes per row)` when the file holds raw premultiplied RGBA8 pixels instead of a PNG.
    pub stride: Option<u32>,
    /// How the helper decoded the take frame (diagnostics).
    pub decode: Option<String>,
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

/// Why an export did not produce a file. `Display` carries the helper's own message so the editor can show it.
#[derive(Debug, PartialEq, Eq)]
pub enum ExportError {
    /// The request or the project was refused.
    Invalid(String),
    NotFound(String),
    DiskFull(String),
    /// The user cancelled: no output file is left.
    Cancelled,
    /// The helper went silent (no progress or heartbeat) and was killed.
    Timeout,
    /// The helper exited during the export.
    Closed,
    Other(String),
}

impl ExportError {
    fn from_helper(code: ErrorCode, message: String) -> Self {
        match code {
            ErrorCode::Invalid => Self::Invalid(message),
            ErrorCode::NotFound => Self::NotFound(message),
            ErrorCode::DiskFull => Self::DiskFull(message),
            ErrorCode::Cancelled => Self::Cancelled,
            code => Self::Other(format!("{code:?}: {message}")),
        }
    }
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(m) | Self::NotFound(m) => write!(f, "{m}"),
            Self::DiskFull(m) => write!(f, "The disk is full: {m}"),
            Self::Cancelled => write!(f, "Export cancelled"),
            Self::Timeout => write!(f, "The export stopped responding and was cancelled."),
            Self::Closed => write!(f, "The capture helper closed during the export."),
            Self::Other(m) => write!(f, "Export failed: {m}"),
        }
    }
}
impl std::error::Error for ExportError {}

/// Longest silence (no `progress`, no `heartbeat`) before an export is declared hung. The helper
/// promises a sign of life at least every 30 s.
pub const EXPORT_SILENCE: Duration = Duration::from_secs(45);

/// `<project>/<file>.partial`: what the helper writes until a clean stop renames it.
/// Files the P4 sources write next to the screen take.
const SIDE_FILES: [&str; 3] = ["camera.mov", "mic.m4a", "system.m4a"];

/// Optional sources of a recording (P4); all off = the plain screen recording.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RecordOptions {
    pub camera: Option<DeviceChoice>,
    pub microphone: Option<DeviceChoice>,
    pub system_audio: bool,
}

fn recording_partial(project: &Path, file: &str) -> PathBuf {
    partial_path(&project.join(file))
}

/// Temp file the helper writes before renaming to the final path.
fn partial_path(out: &Path) -> PathBuf {
    let mut s = out.as_os_str().to_owned();
    s.push(".partial");
    PathBuf::from(s)
}

/// Export `project_dir` to `out` with a short-lived helper (the editor's Export sheet): plan and
/// per-frame parameters are computed here (`capture::export`), the helper renders and encodes.
/// `progress(done_frames, total_frames, eta_seconds)` is called as events arrive; setting `cancel`
/// stops the export and leaves no output. The params file is always removed.
pub fn export_project(
    helper: &Path,
    log: impl Fn(&str) + Send + 'static,
    project_dir: &Path,
    out: &Path,
    opts: &ExportOptions,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u32, u32, Option<f64>),
) -> std::result::Result<ExportDone, ExportError> {
    let invalid = |m: String| ExportError::Invalid(m);
    let project = Project::load(project_dir).map_err(|e| invalid(e.to_string()))?;
    let events_text = std::fs::read_to_string(capture_project::events_path(project_dir))
        .map_err(|e| invalid(format!("events.json could not be read: {e}")))?;
    let events = Events::decode(&events_text).map_err(|e| invalid(e.to_string()))?;
    let plan = export::export_plan(&project, export::source_pixels(&events), opts).map_err(|e| invalid(e.to_string()))?;
    if cancel.load(Ordering::Relaxed) {
        return Err(ExportError::Cancelled);
    }
    let cache = project_dir.join(".cache");
    std::fs::create_dir_all(&cache).map_err(|e| invalid(format!("could not create {}: {e}", cache.display())))?;
    let params = cache.join(format!("export-{}.ndjson", std::process::id()));
    let written = std::fs::File::create(&params).and_then(|f| {
        let mut w = std::io::BufWriter::new(f);
        export::write_frame_params(&project, &events, &plan, &mut w)?;
        w.flush()
    });
    let result = written.map_err(|e| invalid(format!("could not write the frame parameters: {e}"))).and_then(|_| {
        let mut h = Helper::spawn(helper, log).map_err(|e| ExportError::Other(e.to_string()))?;
        let (_, rx) = h.export_start(project_dir, out, &plan, &params)?;
        h.export_wait(&rx, cancel, |ev| {
            if let ExportEvent::Progress { done_frames, total_frames, eta } = ev {
                progress(*done_frames, *total_frames, *eta);
            }
        })
    });
    let _ = std::fs::remove_file(&params);
    result
}

/// Hand a line that belongs to the running export to its sink. Returns whether it was consumed:
/// events of the export id, and its final `ok` / error line (which also ends the stream).
fn route_export_line(sink: &ExportSink, line: &Line) -> bool {
    let mut guard = sink.lock().unwrap();
    let Some((id, tx)) = guard.as_ref() else { return false };
    match line {
        Line::Event { id: Some(eid), name, body } if eid == id => {
            if let Some(ev) = protocol::parse_export_event(name, body) {
                let _ = tx.send(ev);
                return true;
            }
            false
        }
        Line::Ok { id: rid, .. } | Line::Error { id: Some(rid), .. } if rid == id => {
            if let Some(ev) = protocol::export_final(line) {
                let _ = tx.send(ev);
                guard.take();
                return true;
            }
            false
        }
        _ => false,
    }
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

type ExportSink = Arc<Mutex<Option<(u64, Sender<ExportEvent>)>>>;

pub struct Helper {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<Line>,
    next_id: u64,
    /// Where recording events go; set by `record_start`, cleared on stop and when stdout closes.
    recording_events: Arc<Mutex<Option<Sender<RecordEvent>>>>,
    /// Id and event sink of the export in progress; the reader thread routes its lines here.
    export_events: ExportSink,
    /// Request id and `<out>.partial` of the export in progress.
    export: Option<(u64, PathBuf)>,
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
        let export_events: ExportSink = Arc::default();
        let exports = export_events.clone();
        std::thread::spawn(move || {
            for l in BufReader::new(stdout).lines().map_while(|l| l.ok()) {
                match protocol::decode_line(&l) {
                    Ok(line) => {
                        if route_export_line(&exports, &line) {
                            continue;
                        }
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
            // stdout closed: end the event streams so receivers see the disconnect. An export
            // that never got its final line is reported as failed rather than just vanishing.
            subscriber.lock().unwrap().take();
            if let Some((_, sink)) = exports.lock().unwrap().take() {
                let _ = sink.send(ExportEvent::Failed { code: ErrorCode::Internal, message: "the capture helper closed".into() });
            }
        });
        std::thread::spawn(move || {
            for l in BufReader::new(stderr).lines().map_while(|l| l.ok()) {
                log(&l);
            }
        });
        Ok(Helper { child, stdin, lines, next_id: 1, recording_events, export_events, export: None, recording_partial: None })
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
        self.render_frame_as(project_dir, t_source, width, params, None)
    }

    /// Like [`Helper::render_frame`] but the helper writes raw RGBA8 pixels (`stride` is set in the
    /// result): no PNG encode in the helper and no decode here. The file is reused by the next call,
    /// so read it before asking for another frame.
    pub fn render_frame_raw(
        &mut self,
        project_dir: &Path,
        t_source: f64,
        width: u32,
        params: FrameParams,
    ) -> std::result::Result<RenderedFrame, RenderError> {
        let f = self.render_frame_as(project_dir, t_source, width, params, Some(FrameFormat::Raw))?;
        if f.stride.is_none() {
            return Err(RenderError::Other("the capture helper is too old for raw preview frames".into()));
        }
        Ok(f)
    }

    fn render_frame_as(
        &mut self,
        project_dir: &Path,
        t_source: f64,
        width: u32,
        params: FrameParams,
        format: Option<FrameFormat>,
    ) -> std::result::Result<RenderedFrame, RenderError> {
        if width == 0 {
            return Err(RenderError::Invalid("preview width must be at least 1 px".into()));
        }
        let command =
            Command::RenderFrame { project: project_dir.to_string_lossy().into_owned(), t: t_source, width, out: None, format, params };
        let timeout = timeout_for(&command);
        let body = self.call(command, timeout).map_err(RenderError::from)?;
        let done =
            protocol::parse_body::<RenderFrameDone>(&body).map_err(|e| RenderError::Other(format!("bad render.frame reply: {e}")))?;
        let stride = (done.format == Some(FrameFormat::Raw)).then_some(done.stride).flatten();
        Ok(RenderedFrame { path: PathBuf::from(done.path), size: done.size, ms: done.ms, cached: done.cached, stride, decode: done.decode })
    }

    /// Cameras and microphones (no permission prompt; names may be empty without a grant).
    pub fn devices(&mut self) -> Result<Devices> {
        let body = self.call(Command::Devices, timeout_for(&Command::Devices))?;
        protocol::parse_body(&body).map_err(|e| anyhow!(e))
    }

    /// Camera / Microphone permission states. The kinds in `request` that are still undecided make
    /// the system show its prompt (this call waits for the answer); the others are only reported.
    pub fn media_permissions(&mut self, request: &[MediaKind]) -> Result<MediaPermissions> {
        let command = Command::Permissions { request: request.to_vec() };
        let timeout = timeout_for(&command);
        let body = self.call(command, timeout)?;
        protocol::parse_body(&body).map_err(|e| anyhow!(e))
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
        self.record_start_with(project, file, target, fps, cursor, exclude_windows, RecordOptions::default())
    }

    /// [`Helper::record_start`] with the optional P4 sources. Each one is independent; the files
    /// (`camera.mov`, `mic.m4a`, `system.m4a`) and their offsets come back in `record_stop().sources`.
    /// A denied camera or microphone fails with `PermissionCamera` / `PermissionMicrophone` before any file exists.
    #[allow(clippy::too_many_arguments)]
    pub fn record_start_with(
        &mut self,
        project: &Path,
        file: &str,
        target: Target,
        fps: u32,
        cursor: bool,
        exclude_windows: Vec<u32>,
        options: RecordOptions,
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
            camera: options.camera,
            microphone: options.microphone,
            system_audio: options.system_audio,
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
                // Busy: the files belong to the recording that is already running.
                if e != RecordError::Busy {
                    for side in SIDE_FILES {
                        let _ = std::fs::remove_file(recording_partial(project, side));
                    }
                }
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

    /// Start exporting `project_dir` to `out` (a plain path in JSON). `params_file` is the NDJSON of
    /// `export::FrameLine` written by `export::write_frame_params` (the caller removes it afterwards).
    /// Returns once the helper accepted the job (`started`); the channel then carries `Progress` /
    /// `Heartbeat` and ends with one terminal event. Drive it with [`Helper::export_wait`].
    /// On failure no `<out>.partial` is left.
    pub fn export_start(
        &mut self,
        project_dir: &Path,
        out: &Path,
        plan: &ExportPlan,
        params_file: &Path,
    ) -> std::result::Result<(ExportStarted, Receiver<ExportEvent>), ExportError> {
        if self.export.is_some() {
            return Err(ExportError::Other("an export is already running".into()));
        }
        let command = Command::Render {
            project: project_dir.to_string_lossy().into_owned(),
            out: out.to_string_lossy().into_owned(),
            format: plan.format,
            codec: plan.codec,
            width: plan.size[0],
            height: plan.size[1],
            fps: plan.fps,
            quality: plan.quality,
            frames: plan.frames,
            segments: plan.segments.clone(),
            audio: plan.audio.clone(),
            params_file: params_file.to_string_lossy().into_owned(),
        };
        let timeout = timeout_for(&command);
        let id = self.next_id;
        self.next_id += 1;
        let partial = partial_path(out);
        let (tx, rx) = channel();
        *self.export_events.lock().unwrap() = Some((id, tx));
        let line = protocol::encode_request(&Request { id, command });
        if writeln!(self.stdin, "{line}").and_then(|_| self.stdin.flush()).is_err() {
            self.export_events.lock().unwrap().take();
            return Err(ExportError::Closed);
        }
        let deadline = Instant::now() + timeout;
        let result = loop {
            match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(ExportEvent::Started { total_frames }) => break Ok(ExportStarted { total_frames }),
                Ok(ExportEvent::Failed { code, message }) => break Err(ExportError::from_helper(code, message)),
                Ok(ExportEvent::Cancelled) => break Err(ExportError::Cancelled),
                Ok(ExportEvent::Done(_)) => break Err(ExportError::Other("the export finished before it started".into())),
                Ok(_) => {}
                Err(RecvTimeoutError::Timeout) => {
                    self.kill();
                    break Err(ExportError::Timeout);
                }
                Err(RecvTimeoutError::Disconnected) => break Err(ExportError::Closed),
            }
        };
        match result {
            Ok(started) => {
                self.export = Some((id, partial));
                Ok((started, rx))
            }
            Err(e) => {
                self.export_events.lock().unwrap().take();
                let _ = std::fs::remove_file(partial);
                Err(e)
            }
        }
    }

    /// Ask the helper to stop the running export; it answers `Cancelled` on the event channel.
    pub fn export_cancel(&mut self) -> std::result::Result<(), ExportError> {
        let Some((target, _)) = self.export.as_ref() else { return Ok(()) };
        let line = protocol::encode_request(&Request { id: self.next_id, command: Command::Cancel { target: *target } });
        self.next_id += 1;
        writeln!(self.stdin, "{line}").and_then(|_| self.stdin.flush()).map_err(|_| ExportError::Closed)
    }

    /// Follow an export to its end: `on_event` sees every non-terminal event (progress for the UI),
    /// `cancel` set to true sends `cancel` once, and a helper silent for [`EXPORT_SILENCE`] is
    /// killed. Whatever the outcome, no `<out>.partial` is left behind unless the export succeeded.
    pub fn export_wait(
        &mut self,
        events: &Receiver<ExportEvent>,
        cancel: &AtomicBool,
        on_event: impl FnMut(&ExportEvent),
    ) -> std::result::Result<ExportDone, ExportError> {
        self.export_wait_with(events, cancel, EXPORT_SILENCE, on_event)
    }

    fn export_wait_with(
        &mut self,
        events: &Receiver<ExportEvent>,
        cancel: &AtomicBool,
        silence: Duration,
        mut on_event: impl FnMut(&ExportEvent),
    ) -> std::result::Result<ExportDone, ExportError> {
        let mut last_sign = Instant::now();
        let mut cancel_sent = false;
        let result = loop {
            if cancel.load(Ordering::Relaxed) && !cancel_sent {
                cancel_sent = true;
                if let Err(e) = self.export_cancel() {
                    break Err(e);
                }
            }
            match events.recv_timeout(Duration::from_millis(50)) {
                Ok(ExportEvent::Done(done)) => break Ok(done),
                Ok(ExportEvent::Failed { code, message }) => break Err(ExportError::from_helper(code, message)),
                Ok(ExportEvent::Cancelled) => break Err(ExportError::Cancelled),
                Ok(ev) => {
                    last_sign = Instant::now();
                    on_event(&ev);
                }
                Err(RecvTimeoutError::Timeout) if last_sign.elapsed() > silence => {
                    self.kill();
                    break Err(ExportError::Timeout);
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break Err(ExportError::Closed),
            }
        };
        self.export_events.lock().unwrap().take();
        if let Some((_, partial)) = self.export.take() {
            if result.is_err() {
                let _ = std::fs::remove_file(partial);
            }
        }
        result
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
    use ferrocast_core::capture::protocol::{MediaAccess, RecordState};
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
    fn p4_project(p: &Path, name: &str) -> PathBuf {
        let project = dir_of(p).join(name);
        std::fs::create_dir_all(&project).unwrap();
        project
    }

    #[test]
    fn record_start_without_p4_options_sends_the_old_line() {
        let p = fake("p4-old", r#"read l; echo "$l" > "$(dirname "$0")/start.json"; echo '{"id":1,"ok":true,"started_at_host":1.0}'"#);
        let project = p4_project(&p, "old.ferrocast");
        let mut h = Helper::spawn(&p, |_| {}).unwrap();
        h.record_start(&project, "screen.mov", Target::Display { display: 1 }, 60, false, Vec::new()).unwrap();
        let wire = std::fs::read_to_string(dir_of(&p).join("start.json")).unwrap();
        assert!(!wire.contains("camera") && !wire.contains("microphone") && !wire.contains("system_audio"), "{wire}");
    }

    #[test]
    fn record_start_with_sources_sends_them_and_stop_lists_the_files() {
        let p = fake(
            "p4-src",
            r#"read l; echo "$l" > "$(dirname "$0")/start.json"
echo '{"id":1,"ok":true,"started_at_host":10.5,"size":[200,100],"file":"screen.mov"}'
echo '{"event":"warn","id":1,"code":"camera-lost","message":"unplugged"}'
read l; echo '{"id":2,"ok":true,"project":"/p","duration":2.0,"screen":"screen.mov","size":[200,100],"sources":{"screen":{"file":"screen.mov","offset":0},"mic":{"file":"mic.m4a","offset":0.01},"system":{"file":"system.m4a","offset":-0.002}}}'"#,
        );
        let project = p4_project(&p, "src.ferrocast");
        let mut h = Helper::spawn(&p, |_| {}).unwrap();
        let opts = RecordOptions {
            camera: Some(DeviceChoice { device: Some("cam-1".into()) }),
            microphone: Some(DeviceChoice::default()),
            system_audio: true,
        };
        let (_, events) = h.record_start_with(&project, "screen.mov", Target::Display { display: 1 }, 60, false, Vec::new(), opts).unwrap();
        assert_eq!(
            events.recv_timeout(Duration::from_secs(2)).unwrap(),
            RecordEvent::Warn { code: "camera-lost".into(), message: Some("unplugged".into()), free_bytes: None }
        );
        let done = h.record_stop().unwrap();
        let s = done.sources.unwrap();
        assert_eq!((s.camera, s.mic.unwrap().offset, s.system.unwrap().file.as_str()), (None, 0.01, "system.m4a"));
        let wire = std::fs::read_to_string(dir_of(&p).join("start.json")).unwrap();
        assert!(wire.contains(r#""camera":{"device":"cam-1"}"#) && wire.contains(r#""microphone":{}"#), "{wire}");
        assert!(wire.contains(r#""system_audio":true"#), "{wire}");
    }

    #[test]
    fn camera_and_microphone_permission_errors_map_and_remove_every_partial() {
        for (code, want) in
            [("permission-camera", RecordError::PermissionCamera), ("permission-microphone", RecordError::PermissionMicrophone)]
        {
            let p = fake(&format!("p4-{code}"), &format!(r#"read l; echo '{{"id":1,"error":"{code}","message":"off"}}'; exit 1"#));
            let project = p4_project(&p, "perm.ferrocast");
            for f in ["screen.mov.partial", "camera.mov.partial", "mic.m4a.partial", "system.m4a.partial"] {
                std::fs::write(project.join(f), b"half").unwrap();
            }
            let opts =
                RecordOptions { camera: Some(DeviceChoice::default()), microphone: Some(DeviceChoice::default()), system_audio: true };
            let mut h = Helper::spawn(&p, |_| {}).unwrap();
            let e = h.record_start_with(&project, "screen.mov", Target::Display { display: 1 }, 60, false, Vec::new(), opts).unwrap_err();
            assert_eq!(e, want);
            assert_eq!(std::fs::read_dir(&project).unwrap().count(), 0, "no file left for {code}");
        }
    }

    #[test]
    fn permission_toasts_are_the_s12_texts() {
        assert_eq!(
            CAMERA_PERMISSION_TOAST,
            "Camera is off for Ferrocast. Turn it on in System Settings \u{2192} Privacy & Security \u{2192} Camera."
        );
        assert_eq!(
            MICROPHONE_PERMISSION_TOAST,
            "Microphone is off for Ferrocast. Turn it on in System Settings \u{2192} Privacy & Security \u{2192} Microphone."
        );
        assert_eq!(RecordError::PermissionCamera.to_string(), CAMERA_PERMISSION_TOAST);
        assert_eq!(RecordError::PermissionMicrophone.to_string(), MICROPHONE_PERMISSION_TOAST);
    }

    #[test]
    fn devices_and_permissions_are_typed() {
        let p = fake(
            "p4-dev",
            r#"read l; echo "$l" > "$(dirname "$0")/l1.json"
echo '{"id":1,"ok":true,"cameras":[{"id":"c","name":"","position":"unspecified","default":true}],"microphones":[]}'
read l; echo "$l" > "$(dirname "$0")/l2.json"
echo '{"id":2,"ok":true,"camera":"notDetermined","microphone":"denied"}'"#,
        );
        let mut h = Helper::spawn(&p, |_| {}).unwrap();
        let d = h.devices().unwrap();
        assert_eq!((d.cameras.len(), d.cameras[0].default, d.microphones.len()), (1, true, 0));
        let perm = h.media_permissions(&[MediaKind::Camera]).unwrap();
        assert_eq!((perm.camera, perm.microphone), (MediaAccess::NotDetermined, MediaAccess::Denied));
        assert_eq!(std::fs::read_to_string(dir_of(&p).join("l1.json")).unwrap().trim(), r#"{"id":1,"cmd":"devices"}"#);
        assert_eq!(
            std::fs::read_to_string(dir_of(&p).join("l2.json")).unwrap().trim(),
            r#"{"id":2,"cmd":"permissions","request":["camera"]}"#
        );
    }

    #[test]
    fn p4_timeouts_let_the_user_answer_a_prompt() {
        assert_eq!(timeout_for(&Command::Devices), Duration::from_secs(10));
        assert!(timeout_for(&Command::Permissions { request: vec![MediaKind::Camera] }) >= Duration::from_secs(60));
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
        assert_eq!(
            f,
            RenderedFrame {
                path: PathBuf::from("/p/.cache/f-1.png"),
                size: [960, 540],
                ms: 12.5,
                cached: true,
                stride: None,
                decode: None
            }
        );
        let wire = std::fs::read_to_string(p.with_file_name("req.json")).unwrap();
        assert!(wire.contains(r#""cmd":"render.frame""#) && wire.contains("Demo.ferrocast") && wire.contains(r#""width":960"#), "{wire}");
        assert!(wire.contains(r#""camera":{"x":0.25"#) && wire.contains(r#""t":1.5"#), "{wire}");
    }

    #[test]
    fn render_frame_raw_asks_for_raw_and_reports_the_row_stride() {
        let p = fake(
            "raw",
            r#"read l; echo "$l" > "$(dirname "$0")/req.json"
echo '{"id":1,"ok":true,"path":"/p/.cache/preview.rgba","size":[960,540],"ms":9.5,"format":"raw","stride":3840,"decode":"advance"}'"#,
        );
        let mut h = Helper::spawn(&p, |_| {}).unwrap();
        let f = h.render_frame_raw(Path::new("/p"), 2.5, 960, FrameParams::default()).unwrap();
        assert_eq!(f.stride, Some(3840));
        assert_eq!((f.size, f.decode.as_deref()), ([960, 540], Some("advance")));
        let wire = std::fs::read_to_string(p.with_file_name("req.json")).unwrap();
        assert!(wire.contains(r#""format":"raw""#), "{wire}");
        // the PNG call never sends it
        let p = fake("png", r#"read l; echo "$l" > "$(dirname "$0")/req.json"; echo '{"id":1,"ok":true,"path":"/x.png","size":[2,2]}'"#);
        let f = Helper::spawn(&p, |_| {}).unwrap().render_frame(Path::new("/p"), 0.0, 2, FrameParams::default()).unwrap();
        assert_eq!(f.stride, None);
        assert!(!std::fs::read_to_string(p.with_file_name("req.json")).unwrap().contains("format"));
    }

    #[test]
    fn a_raw_request_answered_with_a_png_reply_is_an_error() {
        // the helper is older than the app: it ignores `format` and sends a PNG path; the editor must not read it as pixels
        let p = fake("oldhelper", r#"read l; echo '{"id":1,"ok":true,"path":"/x.png","size":[2,2]}'"#);
        let e = Helper::spawn(&p, |_| {}).unwrap().render_frame_raw(Path::new("/p"), 0.0, 2, FrameParams::default()).unwrap_err();
        assert!(matches!(e, RenderError::Other(_)), "{e:?}");
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

#[cfg(test)]
mod export_tests {
    use super::*;
    use ferrocast_core::capture::export::{Codec, Format, Quality, Segment};
    use std::os::unix::fs::PermissionsExt;

    fn fake(name: &str, body: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fc-export-test-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("helper.sh");
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    fn plan() -> ExportPlan {
        ExportPlan {
            segments: vec![Segment { src_start: 0.0, src_end: 2.0, speed: 1.0 }],
            audio: vec![],
            duration: 2.0,
            frames: 60,
            size: [960, 540],
            fps: 30,
            format: Format::Mp4,
            codec: Codec::H264,
            quality: Quality::Standard,
        }
    }

    fn out_of(p: &Path) -> PathBuf {
        p.with_file_name("Demo é.mp4")
    }

    const OK_SCRIPT: &str = r#"read l; echo "$l" > "$(dirname "$0")/req.json"
echo '{"event":"started","id":1,"total_frames":60}'
echo '{"event":"progress","id":1,"done_frames":30,"total_frames":60,"eta":1.0}'
echo '{"event":"heartbeat","id":1}'
echo '{"id":1,"ok":true,"path":"/o.mp4","bytes":1234,"duration":2.0,"frames":60}'"#;

    #[test]
    fn export_streams_progress_and_ends_with_the_result() {
        let p = fake("ok", OK_SCRIPT);
        let mut h = Helper::spawn(&p, |_| {}).unwrap();
        let (started, rx) =
            h.export_start(&p.with_file_name("Demo.ferrocast"), &out_of(&p), &plan(), Path::new("/p/params.ndjson")).unwrap();
        assert_eq!(started.total_frames, 60);
        let mut seen = vec![];
        let done = h.export_wait(&rx, &AtomicBool::new(false), |e| seen.push(e.clone())).unwrap();
        assert_eq!((done.path.as_str(), done.bytes, done.duration), ("/o.mp4", 1234, 2.0));
        assert_eq!(seen, vec![ExportEvent::Progress { done_frames: 30, total_frames: 60, eta: Some(1.0) }, ExportEvent::Heartbeat]);
        let wire = std::fs::read_to_string(p.with_file_name("req.json")).unwrap();
        assert!(wire.contains(r#""cmd":"render""#) && wire.contains("Demo é.mp4") && wire.contains("params.ndjson"), "{wire}");
        assert!(wire.contains(r#""codec":"h264""#) && wire.contains(r#""width":960"#) && wire.contains(r#""frames":60"#), "{wire}");
    }

    #[test]
    fn gif_plan_travels_as_format_gif() {
        let p = fake("gif", OK_SCRIPT);
        let mut h = Helper::spawn(&p, |_| {}).unwrap();
        let gif = ExportPlan { format: Format::Gif, size: [960, 540], fps: 15, frames: 30, ..plan() };
        let (_, rx) = h.export_start(&dir_of(&p), &p.with_file_name("Demo.gif"), &gif, Path::new("/p/params.ndjson")).unwrap();
        h.export_wait(&rx, &AtomicBool::new(false), |_| {}).unwrap();
        let wire = std::fs::read_to_string(p.with_file_name("req.json")).unwrap();
        assert!(wire.contains(r#""format":"gif""#) && wire.contains(r#""fps":15"#) && wire.contains("Demo.gif"), "{wire}");
    }

    #[test]
    fn audio_inputs_travel_in_the_render_request_only_when_present() {
        use ferrocast_core::capture::export::{AudioInput, AudioKind, AudioRun};
        let p = fake("audio", OK_SCRIPT);
        let mut h = Helper::spawn(&p, |_| {}).unwrap();
        let (_, rx) = h.export_start(&dir_of(&p), &out_of(&p), &plan(), Path::new("/p/params.ndjson")).unwrap();
        h.export_wait(&rx, &AtomicBool::new(false), |_| {}).unwrap();
        let wire = std::fs::read_to_string(p.with_file_name("req.json")).unwrap();
        assert!(!wire.contains(r#""audio":"#), "{wire}");
        let with = ExportPlan {
            audio: vec![AudioInput {
                track: AudioKind::System,
                file: "system.m4a".into(),
                gain: 0.6,
                segments: vec![AudioRun { src_start: 0.0, src_end: 2.0, out_start: 0.0, speed: 1.0 }],
            }],
            ..plan()
        };
        let mut h = Helper::spawn(&p, |_| {}).unwrap();
        let (_, rx) = h.export_start(&dir_of(&p), &out_of(&p), &with, Path::new("/p/params.ndjson")).unwrap();
        h.export_wait(&rx, &AtomicBool::new(false), |_| {}).unwrap();
        let wire = std::fs::read_to_string(p.with_file_name("req.json")).unwrap();
        assert!(wire.contains(r#""audio":[{"track":"system","file":"system.m4a","gain":0.6,"segments":[{"src_start":0.0,"src_end":2.0,"out_start":0.0,"speed":1.0}]}]"#), "{wire}");
    }

    #[test]
    fn refused_export_maps_the_error_and_removes_the_partial() {
        let p = fake("refused", r#"read l; echo '{"id":1,"error":"invalid","message":"width is odd"}'"#);
        let out = out_of(&p);
        std::fs::write(partial_path(&out), b"half").unwrap();
        let mut h = Helper::spawn(&p, |_| {}).unwrap();
        let e = h.export_start(&dir_of(&p), &out, &plan(), Path::new("/p/x")).unwrap_err();
        assert_eq!(e, ExportError::Invalid("width is odd".into()));
        assert!(e.to_string().contains("width is odd"));
        assert!(!out.exists() && !partial_path(&out).exists());
    }

    fn dir_of(p: &Path) -> PathBuf {
        p.parent().unwrap().to_path_buf()
    }

    #[test]
    fn cancel_sends_the_target_and_leaves_no_output() {
        let p = fake(
            "cancel",
            r#"read l
echo '{"event":"started","id":1,"total_frames":600}'
echo '{"event":"progress","id":1,"done_frames":5,"total_frames":600}'
read c; echo "$c" > "$(dirname "$0")/cancel.json"
echo '{"id":1,"error":"cancelled","message":"Export cancelled"}'
echo '{"id":2,"ok":true}'"#,
        );
        let out = out_of(&p);
        let mut h = Helper::spawn(&p, |_| {}).unwrap();
        let (_, rx) = h.export_start(&dir_of(&p), &out, &plan(), Path::new("/p/x")).unwrap();
        std::fs::write(partial_path(&out), b"half").unwrap();
        let cancel = AtomicBool::new(false);
        let e = h
            .export_wait(&rx, &cancel, |ev| {
                if matches!(ev, ExportEvent::Progress { .. }) {
                    cancel.store(true, Ordering::Relaxed);
                }
            })
            .unwrap_err();
        assert_eq!(e, ExportError::Cancelled);
        assert!(!out.exists() && !partial_path(&out).exists(), "no output and no .partial");
        let wire = std::fs::read_to_string(p.with_file_name("cancel.json")).unwrap();
        assert_eq!(wire.trim(), r#"{"id":2,"cmd":"cancel","target":1}"#);
        // the helper is reusable: a second export may start
        assert!(h.export.is_none());
    }

    #[test]
    fn failure_after_start_is_reported_and_cleans_up() {
        let p = fake(
            "disk",
            r#"read l; echo '{"event":"started","id":1,"total_frames":10}'
echo '{"id":1,"error":"disk-full","message":"No space left on device"}'"#,
        );
        let out = out_of(&p);
        let mut h = Helper::spawn(&p, |_| {}).unwrap();
        let (_, rx) = h.export_start(&dir_of(&p), &out, &plan(), Path::new("/p/x")).unwrap();
        std::fs::write(partial_path(&out), b"half").unwrap();
        let e = h.export_wait(&rx, &AtomicBool::new(false), |_| {}).unwrap_err();
        assert_eq!(e, ExportError::DiskFull("No space left on device".into()));
        assert!(!partial_path(&out).exists());
    }

    #[test]
    fn a_silent_helper_is_killed_and_its_partial_removed() {
        let p = fake("silent", r#"read l; echo '{"event":"started","id":1,"total_frames":10}'; sleep 30"#);
        let out = out_of(&p);
        let mut h = Helper::spawn(&p, |_| {}).unwrap();
        let (_, rx) = h.export_start(&dir_of(&p), &out, &plan(), Path::new("/p/x")).unwrap();
        std::fs::write(partial_path(&out), b"half").unwrap();
        let t = Instant::now();
        let e = h.export_wait_with(&rx, &AtomicBool::new(false), Duration::from_millis(300), |_| {}).unwrap_err();
        assert_eq!(e, ExportError::Timeout);
        assert!(t.elapsed() < Duration::from_secs(5));
        assert!(h.child.try_wait().unwrap().is_some(), "helper killed");
        assert!(!partial_path(&out).exists());
    }

    #[test]
    fn heartbeats_keep_a_slow_export_alive() {
        let p = fake(
            "beats",
            r#"read l; echo '{"event":"started","id":1,"total_frames":10}'
for i in 1 2 3 4 5 6; do sleep 0.1; echo '{"event":"heartbeat","id":1}'; done
echo '{"id":1,"ok":true,"path":"/o.mp4","bytes":1,"duration":0.3}'"#,
        );
        let mut h = Helper::spawn(&p, |_| {}).unwrap();
        let (_, rx) = h.export_start(&dir_of(&p), &out_of(&p), &plan(), Path::new("/p/x")).unwrap();
        let r = h.export_wait_with(&rx, &AtomicBool::new(false), Duration::from_millis(400), |_| {});
        assert!(r.is_ok(), "{r:?}");
    }

    #[test]
    fn helper_dying_mid_export_reports_failure_and_removes_the_partial() {
        let p = fake("die", r#"read l; echo '{"event":"started","id":1,"total_frames":10}'; exit 1"#);
        let out = out_of(&p);
        let mut h = Helper::spawn(&p, |_| {}).unwrap();
        let (_, rx) = h.export_start(&dir_of(&p), &out, &plan(), Path::new("/p/x")).unwrap();
        std::fs::write(partial_path(&out), b"half").unwrap();
        let e = h.export_wait(&rx, &AtomicBool::new(false), |_| {}).unwrap_err();
        assert!(matches!(e, ExportError::Other(_) | ExportError::Closed), "{e:?}");
        assert!(!partial_path(&out).exists());
    }

    #[test]
    fn render_events_of_other_requests_and_record_events_do_not_leak_into_an_export() {
        let p = fake(
            "other",
            r#"read l; echo '{"event":"progress","id":99,"done_frames":1,"total_frames":2}'
echo '{"event":"started","id":1,"total_frames":4}'
echo '{"event":"tick","id":1,"elapsed":1.0,"frames":1,"dropped":0}'
echo '{"id":1,"ok":true,"path":"/o.mp4","bytes":1,"duration":0.1}'"#,
        );
        let mut h = Helper::spawn(&p, |_| {}).unwrap();
        let (_, rx) = h.export_start(&dir_of(&p), &out_of(&p), &plan(), Path::new("/p/x")).unwrap();
        let mut n = 0;
        h.export_wait(&rx, &AtomicBool::new(false), |_| n += 1).unwrap();
        assert_eq!(n, 0);
    }

    /// A project folder with a 2 s project.json and a minimal events.json, plus a fake helper that
    /// echoes the number of lines of the params file it was given.
    fn project_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fc-export-proj-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut p = Project::new("t", "2026-01-01T00:00:00Z");
        p.duration = 2.0;
        p.save(&dir).unwrap();
        std::fs::write(
            dir.join("events.json"),
            r#"{"version":1,"screen":{"size":[1600,1000],"scale":2.0,"display_frame":[0,0,800,500]},"cursor":[[0.0,100,100,"arrow"],[1.0,300,200,"arrow"]]}"#,
        )
        .unwrap();
        dir
    }

    #[test]
    fn export_project_plans_writes_params_and_cleans_up() {
        let p = fake(
            "project",
            r#"read l
f=$(printf '%s' "$l" | sed 's/.*"params_file":"\([^"]*\)".*/\1/')
wc -l < "$f" | tr -d ' ' > "$(dirname "$0")/lines.txt"
echo "$l" > "$(dirname "$0")/req.json"
echo '{"event":"started","id":1,"total_frames":60}'
echo '{"event":"progress","id":1,"done_frames":60,"total_frames":60,"eta":0.0}'
echo '{"id":1,"ok":true,"path":"/o.mp4","bytes":10,"duration":2.0,"frames":60}'"#,
        );
        let dir = project_dir("ok");
        let opts = ExportOptions { format: Format::Mp4, codec: Codec::Hevc, width: 960, fps: 30, quality: Quality::High };
        let mut seen = vec![];
        let done = export_project(&p, |_| {}, &dir, &out_of(&p), &opts, &AtomicBool::new(false), |d, t, _| seen.push((d, t))).unwrap();
        assert_eq!(done.bytes, 10);
        assert_eq!(seen, vec![(60, 60)]);
        assert_eq!(std::fs::read_to_string(p.with_file_name("lines.txt")).unwrap().trim(), "60", "one params line per frame");
        let wire = std::fs::read_to_string(p.with_file_name("req.json")).unwrap();
        assert!(
            wire.contains(r#""codec":"hevc""#) && wire.contains(r#""width":960,"height":540"#) && wire.contains(r#""quality":"high""#),
            "{wire}"
        );
        let leftovers = std::fs::read_dir(dir.join(".cache")).unwrap().count();
        assert_eq!(leftovers, 0, "params file removed");
    }

    #[test]
    fn export_project_clamps_gif_limits_before_the_helper_sees_them() {
        let p = fake(
            "gifproject",
            r#"read l
echo "$l" > "$(dirname "$0")/req.json"
echo '{"event":"started","id":1,"total_frames":30}'
echo '{"id":1,"ok":true,"path":"/o.gif","bytes":10,"duration":2.0,"frames":30}'"#,
        );
        let dir = project_dir("gif");
        let opts = ExportOptions { format: Format::Gif, codec: Codec::H264, width: 1920, fps: 60, quality: Quality::Standard };
        export_project(&p, |_| {}, &dir, &p.with_file_name("o.gif"), &opts, &AtomicBool::new(false), |_, _, _| {}).unwrap();
        let wire = std::fs::read_to_string(p.with_file_name("req.json")).unwrap();
        assert!(wire.contains(r#""format":"gif""#) && wire.contains(r#""width":960"#) && wire.contains(r#""fps":15"#), "{wire}");
    }

    #[test]
    fn export_project_refuses_bad_options_before_starting_a_helper() {
        let dir = project_dir("bad");
        let opts = ExportOptions { format: Format::Mp4, codec: Codec::H264, width: 5000, fps: 30, quality: Quality::Standard };
        let e = export_project(
            Path::new("/nonexistent/helper"),
            |_| {},
            &dir,
            &dir.join("o.mp4"),
            &opts,
            &AtomicBool::new(false),
            |_, _, _| {},
        )
        .unwrap_err();
        assert!(matches!(e, ExportError::Invalid(ref m) if m.contains("5000")), "{e:?}");
        let cancelled = ExportOptions { width: 960, ..opts };
        let e = export_project(
            Path::new("/nonexistent/helper"),
            |_| {},
            &dir,
            &dir.join("o.mp4"),
            &cancelled,
            &AtomicBool::new(true),
            |_, _, _| {},
        )
        .unwrap_err();
        assert_eq!(e, ExportError::Cancelled);
        let missing = std::env::temp_dir().join("fc-export-no-such-project");
        let e =
            export_project(Path::new("/x"), |_| {}, &missing, &missing.join("o.mp4"), &cancelled, &AtomicBool::new(false), |_, _, _| {})
                .unwrap_err();
        assert!(matches!(e, ExportError::Invalid(_)));
    }

    #[test]
    fn render_has_no_deadline_beyond_the_start_ack() {
        let c = Command::Cancel { target: 1 };
        assert!(timeout_for(&c) <= Duration::from_secs(10));
    }
}

/// Live export tests against the real helper and the synthetic editor fixture
/// (`scripts/capture-smoke.sh --keep` writes it). They skip when either is missing; run them with
/// `cargo test -p ferrocast-macos --lib export_live -- --ignored --nocapture`. Outputs stay in
/// `target/capture-spike/t28/out/` for looking at.
#[cfg(test)]
mod export_live_tests {
    use super::*;
    use ferrocast_core::capture::export::{Codec, Format, Quality};
    use ferrocast_core::capture::project::{Speed, Trim, Zoom};

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn helper() -> Option<PathBuf> {
        let p = std::env::var_os("FERROCAST_CAPTURE_HELPER")
            .map(PathBuf::from)
            .unwrap_or_else(|| root().join("target/capture-spike/t28/ferrocast-capture"));
        is_executable(&p).then_some(p)
    }

    /// A scratch copy of the fixture: its screen.mov (symlink), events.json and a project edited by `edit`.
    fn scratch(name: &str, edit: impl FnOnce(&mut Project)) -> Option<PathBuf> {
        let fixture = root().join("target/capture-spike/editor-fixture/Take.ferrocast");
        if !fixture.join("screen.mov").exists() {
            eprintln!("SKIP: no fixture at {}", fixture.display());
            return None;
        }
        let dir = root().join("target/capture-spike/t28/out").join(format!("{name}.ferrocast"));
        std::fs::create_dir_all(&dir).unwrap();
        let _ = std::fs::remove_file(dir.join("screen.mov"));
        std::os::unix::fs::symlink(fixture.join("screen.mov").canonicalize().unwrap(), dir.join("screen.mov")).unwrap();
        std::fs::copy(fixture.join("events.json"), dir.join("events.json")).unwrap();
        let mut p = Project::load(&fixture).unwrap();
        p.trims.clear();
        p.zooms = vec![Zoom {
            id: "z1".into(),
            start: 0.6,
            end: 2.2,
            mode: "manual".into(),
            scale: 2.5,
            focus: Some([0.625, 0.55]),
            ease_in: 0.4,
            ease_out: 0.4,
        }];
        edit(&mut p);
        p.save(&dir).unwrap();
        Some(dir)
    }

    fn probe(path: &Path) -> (String, u32, u32, f64, u32) {
        let o = std::process::Command::new("ffprobe")
            .args(["-v", "error", "-select_streams", "v:0", "-count_frames", "-show_entries"])
            .arg("stream=codec_name,width,height,nb_read_frames:format=duration")
            .args(["-of", "default=nw=1"])
            .arg(path)
            .output()
            .expect("ffprobe");
        let text = String::from_utf8(o.stdout).unwrap();
        let get =
            |k: &str| text.lines().find_map(|l| l.strip_prefix(&format!("{k}="))).unwrap_or_else(|| panic!("{k} in {text}")).to_string();
        (
            get("codec_name"),
            get("width").parse().unwrap(),
            get("height").parse().unwrap(),
            get("duration").parse().unwrap(),
            get("nb_read_frames").parse().unwrap(),
        )
    }

    fn have_ffprobe() -> bool {
        std::process::Command::new("ffprobe").arg("-version").output().is_ok()
    }

    #[test]
    #[ignore = "needs the built helper and the editor fixture"]
    fn export_live_mp4_matches_output_duration() {
        let Some(h) = helper() else { return eprintln!("SKIP: no helper") };
        let Some(dir) = scratch("speed", |p| {
            p.trims = vec![Trim { start: 0.0, end: 3.0 }];
            p.speeds = vec![Speed { start: 1.0, end: 2.0, factor: 2.0 }];
        }) else {
            return;
        };
        let want = ferrocast_core::capture::timeline::Timeline::new(&Project::load(&dir).unwrap()).output_duration();
        assert_eq!(want, 2.5);
        for (codec, fps, width) in [(Codec::H264, 30, 960), (Codec::Hevc, 60, 640)] {
            let out = dir.with_file_name(format!("speed-{codec:?}-{fps}.mp4").to_lowercase());
            let opts = ExportOptions { format: Format::Mp4, codec, width, fps, quality: Quality::Standard };
            let t = Instant::now();
            let mut last = (0, 0);
            let done =
                export_project(&h, |l| eprintln!("helper: {l}"), &dir, &out, &opts, &AtomicBool::new(false), |d, n, _| last = (d, n))
                    .unwrap_or_else(|e| panic!("{e}"));
            let secs = t.elapsed().as_secs_f64();
            eprintln!(
                "EXPORT {codec:?} {width}px @{fps}: {} frames in {secs:.2} s = {:.1} frames/s, {} bytes",
                done.frames,
                f64::from(done.frames) / secs,
                done.bytes
            );
            assert!(out.exists() && !partial_path(&out).exists());
            assert_eq!(last.0, last.1, "progress reached the end");
            assert!((done.duration - want).abs() <= 1.0 / f64::from(fps));
            if have_ffprobe() {
                let (name, w, hh, dur, frames) = probe(&out);
                eprintln!("ffprobe: {name} {w}x{hh} duration {dur:.4} frames {frames}");
                assert_eq!(name, if codec == Codec::Hevc { "hevc" } else { "h264" });
                assert_eq!([w, hh], [width, width * 9 / 16 / 2 * 2]);
                assert!((dur - want).abs() <= 1.0 / f64::from(fps), "duration {dur} vs {want}");
                assert_eq!(frames, done.frames);
            }
        }
    }

    /// (width, height, loop count if the NETSCAPE extension is there, per-frame delays in cs).
    fn parse_gif(path: &Path) -> (u16, u16, Option<u16>, Vec<u16>) {
        let d = std::fs::read(path).unwrap();
        assert!(d.starts_with(b"GIF89a") || d.starts_with(b"GIF87a"), "not a GIF");
        let (w, h) = (u16::from_le_bytes([d[6], d[7]]), u16::from_le_bytes([d[8], d[9]]));
        let mut i = 13 + if d[10] & 0x80 != 0 { 3 << ((d[10] & 7) + 1) } else { 0 };
        let (mut looped, mut delays) = (None, vec![]);
        let skip_blocks = |mut i: usize| {
            while d[i] != 0 {
                i += d[i] as usize + 1;
            }
            i + 1
        };
        while i < d.len() {
            match d[i] {
                0x21 if d[i + 1] == 0xF9 => {
                    delays.push(u16::from_le_bytes([d[i + 4], d[i + 5]]));
                    i = skip_blocks(i + 2);
                }
                0x21 if d[i + 1] == 0xFF => {
                    if &d[i + 3..i + 14] == b"NETSCAPE2.0" && d[i + 14] == 3 && d[i + 15] == 1 {
                        looped = Some(u16::from_le_bytes([d[i + 16], d[i + 17]]));
                    }
                    i = skip_blocks(i + 2);
                }
                0x21 => i = skip_blocks(i + 2),
                0x2C => {
                    let packed = d[i + 9];
                    i += 10 + if packed & 0x80 != 0 { 3 << ((packed & 7) + 1) } else { 0 } + 1;
                    i = skip_blocks(i);
                }
                0x3B => break,
                b => panic!("unexpected GIF byte {b:#x} at {i}"),
            }
        }
        (w, h, looped, delays)
    }

    #[test]
    #[ignore = "needs the built helper and the editor fixture"]
    fn export_live_gif_matches_output_duration_and_loops_forever() {
        let Some(h) = helper() else { return eprintln!("SKIP: no helper") };
        let Some(dir) = scratch("gif", |p| {
            p.trims = vec![Trim { start: 0.0, end: 3.0 }];
            p.speeds = vec![Speed { start: 1.0, end: 2.0, factor: 2.0 }];
        }) else {
            return;
        };
        let want = ferrocast_core::capture::timeline::Timeline::new(&Project::load(&dir).unwrap()).output_duration();
        assert_eq!(want, 2.5);
        for (width, fps) in [(480, 15), (960, 15), (1920, 30)] {
            let out = dir.with_file_name(format!("gif-{width}-{fps}.gif"));
            let opts = ExportOptions { format: Format::Gif, codec: Codec::H264, width, fps, quality: Quality::Standard };
            let t = Instant::now();
            let mut last = (0, 0);
            let done =
                export_project(&h, |l| eprintln!("helper: {l}"), &dir, &out, &opts, &AtomicBool::new(false), |d, n, _| last = (d, n))
                    .unwrap_or_else(|e| panic!("{e}"));
            let secs = t.elapsed().as_secs_f64();
            let (gw, gh, looped, delays) = parse_gif(&out);
            let total = f64::from(delays.iter().map(|&c| u32::from(c)).sum::<u32>()) / 100.0;
            eprintln!(
                "GIF {width}px @{fps}: {gw}x{gh}, {} frames, delays {:?}.., loop {looped:?}, total {total:.2} s (want {want}), {} bytes, {secs:.2} s = {:.1} frames/s",
                delays.len(),
                &delays[..delays.len().min(6)],
                done.bytes,
                f64::from(done.frames) / secs
            );
            assert!(out.exists() && !partial_path(&out).exists());
            assert_eq!(last.0, last.1, "progress reached the end");
            assert_eq!(u32::from(gw), width.min(960), "the helper never exceeds the clamped width");
            assert_eq!(delays.len() as u32, done.frames);
            assert_eq!(delays.len(), 38, "2.5 s at 15 fps = 37.5 -> 38 frames");
            assert_eq!(looped, Some(0), "loop forever");
            assert!((total - want).abs() <= 1.0 / 15.0, "total {total} vs {want}");
            assert!((done.duration - total).abs() < 1e-9);
        }
    }

    #[test]
    #[ignore = "needs the built helper and the editor fixture"]
    fn export_live_gif_cancel_leaves_nothing() {
        let Some(h) = helper() else { return eprintln!("SKIP: no helper") };
        let Some(dir) = scratch("gifcancel", |_| {}) else { return };
        let out = dir.with_file_name("cancel.gif");
        let _ = std::fs::remove_file(&out);
        let opts = ExportOptions { format: Format::Gif, codec: Codec::H264, width: 960, fps: 15, quality: Quality::Standard };
        let cancel = AtomicBool::new(false);
        let e = export_project(
            &h,
            |_| {},
            &dir,
            &out,
            &opts,
            &cancel,
            |d, _, _| {
                if d >= 5 {
                    cancel.store(true, Ordering::Relaxed);
                }
            },
        )
        .unwrap_err();
        assert_eq!(e, ExportError::Cancelled);
        assert!(!out.exists() && !partial_path(&out).exists(), "no output and no .partial");
    }

    #[test]
    #[ignore = "needs the built helper and the editor fixture"]
    fn export_live_cancel_leaves_nothing() {
        let Some(h) = helper() else { return eprintln!("SKIP: no helper") };
        let Some(dir) = scratch("cancel", |_| {}) else { return };
        let out = dir.with_file_name("cancel.mp4");
        let _ = std::fs::remove_file(&out);
        let opts = ExportOptions { format: Format::Mp4, codec: Codec::H264, width: 1920, fps: 60, quality: Quality::Standard };
        let cancel = AtomicBool::new(false);
        let e = export_project(
            &h,
            |_| {},
            &dir,
            &out,
            &opts,
            &cancel,
            |d, _, _| {
                if d >= 10 {
                    cancel.store(true, Ordering::Relaxed);
                }
            },
        )
        .unwrap_err();
        assert_eq!(e, ExportError::Cancelled);
        assert!(!out.exists() && !partial_path(&out).exists(), "no output and no .partial");
        assert_eq!(std::fs::read_dir(dir.join(".cache")).map(|d| d.count()).unwrap_or(0), 0, "params file removed");
    }
}

// ---- P5: on-device transcription -------------------------------------------------------------

use ferrocast_core::capture::protocol::{TranscribeDone, TranscribeEvent};

/// Sentence the app shows when the helper answers `transcription-unavailable`; `{language}` is the
/// human name of the locale. The helper builds its message from the same template.
pub const TRANSCRIPTION_UNAVAILABLE_TEMPLATE: &str = "On-device transcription is not available for {language} on this Mac.";

/// [`TRANSCRIPTION_UNAVAILABLE_TEMPLATE`] with the language filled in.
pub fn transcription_unavailable_message(language: &str) -> String {
    TRANSCRIPTION_UNAVAILABLE_TEMPLATE.replace("{language}", language)
}

/// Longest silence (no progress, no heartbeat, no reply) before a transcription is declared hung.
/// The helper sends a heartbeat at least every 15 s while the recogniser works.
pub const TRANSCRIBE_SILENCE: Duration = Duration::from_secs(90);

/// Why a transcription did not produce a transcript. `Display` is the text the app shows.
#[derive(Debug, PartialEq, Eq)]
pub enum TranscribeError {
    /// Speech Recognition is not authorized; the helper did no work and raised no prompt.
    PermissionSpeech(String),
    /// No on-device recogniser for the locale; carries the sentence of [`TRANSCRIPTION_UNAVAILABLE_TEMPLATE`].
    Unavailable(String),
    Invalid(String),
    NotFound(String),
    Cancelled,
    /// The helper went silent and was killed.
    Timeout,
    Closed,
    Other(String),
}

impl TranscribeError {
    fn from_helper(code: ErrorCode, message: String) -> Self {
        match code {
            ErrorCode::PermissionSpeech => Self::PermissionSpeech(message),
            ErrorCode::TranscriptionUnavailable => Self::Unavailable(message),
            ErrorCode::Invalid => Self::Invalid(message),
            ErrorCode::NotFound => Self::NotFound(message),
            ErrorCode::Cancelled => Self::Cancelled,
            code => Self::Other(format!("{code:?}: {message}")),
        }
    }
}

impl std::fmt::Display for TranscribeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PermissionSpeech(_) => {
                write!(f, "Speech Recognition is not allowed for Ferrocast. Allow it in System Settings > Privacy & Security.")
            }
            Self::Unavailable(m) | Self::Invalid(m) | Self::NotFound(m) => write!(f, "{m}"),
            Self::Cancelled => write!(f, "Transcription cancelled"),
            Self::Timeout => write!(f, "The transcription stopped responding and was cancelled."),
            Self::Closed => write!(f, "The capture helper closed during the transcription."),
            Self::Other(m) => write!(f, "Transcription failed: {m}"),
        }
    }
}
impl std::error::Error for TranscribeError {}

impl Helper {
    /// Transcribe `<project_dir>/<audio>` on this Mac (`audio` default `mic.m4a`, `locale` default the
    /// system one) and wait for the result: the helper writes `<project_dir>/transcript.json`.
    /// `on_progress(done_secs, total_secs)` is called for each progress event; setting `cancel` sends
    /// `cancel` once. Blocking: run it on a worker thread (like `export_project`). Unlike `export_start`
    /// there is no start acknowledgement to wait for (a permission error is the first and only line),
    /// so this returns the final result instead of a receiver. No `transcript.json.partial` is left on failure.
    pub fn transcribe(
        &mut self,
        project_dir: &Path,
        audio: Option<&str>,
        locale: Option<&str>,
        cancel: &AtomicBool,
        on_progress: impl FnMut(f64, f64),
    ) -> std::result::Result<TranscribeDone, TranscribeError> {
        self.transcribe_with(project_dir, audio, locale, cancel, TRANSCRIBE_SILENCE, on_progress)
    }

    fn transcribe_with(
        &mut self,
        project_dir: &Path,
        audio: Option<&str>,
        locale: Option<&str>,
        cancel: &AtomicBool,
        silence: Duration,
        mut on_progress: impl FnMut(f64, f64),
    ) -> std::result::Result<TranscribeDone, TranscribeError> {
        let id = self.next_id;
        self.next_id += 1;
        let command = Command::Transcribe {
            project: project_dir.to_string_lossy().into_owned(),
            audio: audio.map(str::to_owned),
            locale: locale.map(str::to_owned),
        };
        let line = protocol::encode_request(&Request { id, command });
        let result = if writeln!(self.stdin, "{line}").and_then(|_| self.stdin.flush()).is_err() {
            Err(TranscribeError::Closed)
        } else {
            let mut last_sign = Instant::now();
            let mut cancel_sent = false;
            loop {
                if cancel.load(Ordering::Relaxed) && !cancel_sent {
                    cancel_sent = true;
                    let cid = self.next_id;
                    self.next_id += 1;
                    let line = protocol::encode_request(&Request { id: cid, command: Command::Cancel { target: id } });
                    if writeln!(self.stdin, "{line}").and_then(|_| self.stdin.flush()).is_err() {
                        break Err(TranscribeError::Closed);
                    }
                }
                match self.lines.recv_timeout(Duration::from_millis(50)) {
                    Ok(Line::Ok { id: rid, body }) if rid == id => {
                        break protocol::parse_body::<TranscribeDone>(&body).map_err(|e| TranscribeError::Other(format!("bad result: {e}")))
                    }
                    Ok(Line::Error { id: rid, code, message }) if rid.is_none_or(|r| r == id) => {
                        break Err(TranscribeError::from_helper(code, message))
                    }
                    Ok(Line::Event { id: Some(eid), name, body }) if eid == id => {
                        last_sign = Instant::now();
                        if let Some(TranscribeEvent::Progress { done_secs, total_secs }) = protocol::parse_transcribe_event(&name, &body) {
                            on_progress(done_secs, total_secs);
                        }
                    }
                    Ok(_) => {}
                    Err(RecvTimeoutError::Timeout) if last_sign.elapsed() > silence => {
                        self.kill();
                        break Err(TranscribeError::Timeout);
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break Err(TranscribeError::Closed),
                }
            }
        };
        if result.is_err() {
            let _ = std::fs::remove_file(partial_path(&project_dir.join("transcript.json")));
        }
        result
    }
}

#[cfg(test)]
mod transcribe_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn fake(name: &str, body: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fc-transcribe-test-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("helper.sh");
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    fn run(p: &Path, cancel: &AtomicBool, silence: Duration) -> (std::result::Result<TranscribeDone, TranscribeError>, Vec<(f64, f64)>) {
        let mut h = Helper::spawn(p, |_| {}).unwrap();
        let mut seen = vec![];
        let r = h.transcribe_with(p.parent().unwrap(), Some("mic.m4a"), Some("en-US"), cancel, silence, |d, t| seen.push((d, t)));
        (r, seen)
    }

    const LONG: Duration = Duration::from_secs(30);

    #[test]
    fn progress_streams_and_the_result_is_typed() {
        let p = fake(
            "ok",
            r#"read l; echo "$l" > "$(dirname "$0")/req.json"
echo '{"event":"progress","id":1,"done_secs":1.5,"total_secs":3.0}'
echo '{"event":"heartbeat","id":1}'
echo '{"event":"progress","id":7,"done_secs":9,"total_secs":9}'
echo '{"id":1,"ok":true,"language":"en-US","words":4,"path":"/p/transcript.json"}'"#,
        );
        let (r, seen) = run(&p, &AtomicBool::new(false), LONG);
        assert_eq!(r.unwrap(), TranscribeDone { language: "en-US".into(), words: 4, path: "/p/transcript.json".into() });
        assert_eq!(seen, vec![(1.5, 3.0)], "only the progress of this request, never other ids");
        let wire = std::fs::read_to_string(p.with_file_name("req.json")).unwrap();
        assert!(
            wire.starts_with(r#"{"id":1,"cmd":"transcribe","project":""#) && wire.contains(r#""audio":"mic.m4a","locale":"en-US"}"#),
            "{wire}"
        );
    }

    #[test]
    fn permission_and_unavailable_map_to_their_own_errors() {
        let p = fake("perm", r#"read l; echo '{"id":1,"error":"permission-speech","message":"Speech Recognition is not authorized"}'"#);
        let (r, _) = run(&p, &AtomicBool::new(false), LONG);
        assert_eq!(r.unwrap_err(), TranscribeError::PermissionSpeech("Speech Recognition is not authorized".into()));
        let msg = transcription_unavailable_message("Spanish (Spain)");
        assert_eq!(msg, "On-device transcription is not available for Spanish (Spain) on this Mac.");
        let p = fake("unavail", &format!("read l; echo '{{\"id\":1,\"error\":\"transcription-unavailable\",\"message\":\"{msg}\"}}'"));
        let (r, _) = run(&p, &AtomicBool::new(false), LONG);
        let e = r.unwrap_err();
        assert_eq!(e, TranscribeError::Unavailable(msg.clone()));
        assert_eq!(e.to_string(), msg, "the app shows the sentence as is");
    }

    #[test]
    fn other_helper_errors_map_and_a_bad_result_is_other() {
        let p = fake("nf", r#"read l; echo '{"id":1,"error":"not-found","message":"mic.m4a is not in the project"}'"#);
        assert_eq!(
            run(&p, &AtomicBool::new(false), LONG).0.unwrap_err(),
            TranscribeError::NotFound("mic.m4a is not in the project".into())
        );
        let p = fake("inv", r#"read l; echo '{"id":1,"error":"invalid","message":"bad audio name"}'"#);
        assert_eq!(run(&p, &AtomicBool::new(false), LONG).0.unwrap_err(), TranscribeError::Invalid("bad audio name".into()));
        let p = fake("bad", r#"read l; echo '{"id":1,"ok":true}'"#);
        assert!(matches!(run(&p, &AtomicBool::new(false), LONG).0.unwrap_err(), TranscribeError::Other(_)));
    }

    #[test]
    fn failures_remove_the_partial_transcript() {
        let p = fake("partial", r#"read l; echo '{"id":1,"error":"internal","message":"boom"}'"#);
        let partial = p.with_file_name("transcript.json.partial");
        std::fs::write(&partial, b"half").unwrap();
        assert!(matches!(run(&p, &AtomicBool::new(false), LONG).0.unwrap_err(), TranscribeError::Other(_)));
        assert!(!partial.exists());
    }

    #[test]
    fn cancel_sends_the_target_and_reports_cancelled() {
        let p = fake(
            "cancel",
            r#"read l
echo '{"event":"progress","id":1,"done_secs":1,"total_secs":60}'
read c; echo "$c" > "$(dirname "$0")/cancel.json"
echo '{"id":1,"error":"cancelled","message":"Transcription cancelled"}'"#,
        );
        let cancel = AtomicBool::new(false);
        let mut h = Helper::spawn(&p, |_| {}).unwrap();
        let r = h.transcribe_with(p.parent().unwrap(), None, None, &cancel, LONG, |_, _| cancel.store(true, Ordering::Relaxed));
        assert_eq!(r.unwrap_err(), TranscribeError::Cancelled);
        let wire = std::fs::read_to_string(p.with_file_name("cancel.json")).unwrap();
        assert_eq!(wire.trim(), r#"{"id":2,"cmd":"cancel","target":1}"#);
    }

    #[test]
    fn a_silent_helper_is_killed_and_a_dead_one_reports_closed() {
        let p = fake("silent", "read l; sleep 30");
        let t = Instant::now();
        assert_eq!(run(&p, &AtomicBool::new(false), Duration::from_millis(300)).0.unwrap_err(), TranscribeError::Timeout);
        assert!(t.elapsed() < Duration::from_secs(10));
        let p = fake("dead", "read l; exit 3");
        assert_eq!(run(&p, &AtomicBool::new(false), LONG).0.unwrap_err(), TranscribeError::Closed);
    }

    #[test]
    fn heartbeats_keep_a_slow_transcription_alive() {
        let p = fake(
            "beat",
            r#"read l
for i in 1 2 3 4; do sleep 0.2; echo '{"event":"heartbeat","id":1}'; done
echo '{"id":1,"ok":true,"language":"en-US","words":0,"path":"/p/t.json"}'"#,
        );
        assert!(run(&p, &AtomicBool::new(false), Duration::from_millis(500)).0.is_ok());
    }
}
