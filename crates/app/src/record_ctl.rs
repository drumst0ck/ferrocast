//! Recording lifecycle (S13): the pure state machine T19 drives.
//!
//! `idle → countdown → recording ⇄ paused → stopping → idle`. Nothing here touches the helper,
//! AppKit or the tray: every transition returns an [`Effect`] the caller performs (show a numeral,
//! send `record.start`/`pause`/`resume`/`stop`, toast a warning), and helper events are fed in
//! through [`RecordCtl::on_event`]. The tray title and the Stop item derive from the state.
//! The AppKit pieces live in `ferrocast_macos::hud`, whose pure helpers (`Countdown`,
//! `disk_level`, `low_disk_message`, `tray_title`) are reused here.

use chrono::NaiveDateTime;
use ferrocast_core::capture::protocol::{RecordEvent, RecordState, RecordStopped, Shareable, Target};
use ferrocast_macos::capture::RecordError;
use ferrocast_macos::hud::{self, Countdown, CountdownStep, DiskLevel};
use std::path::{Path, PathBuf};

/// Where a recording is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    Idle,
    /// Showing the `3…2…1` numerals; nothing is recorded yet.
    Countdown(Countdown),
    Recording,
    Paused,
    /// `record.stop` was sent (user, tray, or the disk guard); waiting for the helper to finish.
    Stopping,
}

/// What the caller must do after a transition.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    None,
    ShowCountdown(u32),
    /// The countdown was cancelled before recording started: hide the numerals.
    CancelCountdown,
    /// Hide the numerals and send `record.start` (with `hud::excluded_window_ids()`).
    StartRecording,
    SendPause,
    SendResume,
    SendStop,
    /// Show this warning text (`Low disk space: N GB left`).
    LowDisk(String),
    /// The recording is over: hide the HUD. `error` is set when the helper failed.
    Ended {
        error: Option<String>,
    },
}

#[derive(Clone, Debug)]
pub struct RecordCtl {
    phase: Phase,
    elapsed: f64,
    dropped: u64,
    low_disk: Option<String>,
    stopped_for_disk: bool,
}

impl RecordCtl {
    pub fn new() -> Self {
        Self { phase: Phase::Idle, elapsed: 0.0, dropped: 0, low_disk: None, stopped_for_disk: false }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// Seconds recorded so far, from the helper's `tick` events.
    pub fn elapsed(&self) -> f64 {
        self.elapsed
    }

    // read by tests; the HUD does not show dropped frames
    #[allow(dead_code)]
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// The `Low disk space: N GB left` text currently showing, if any.
    // read by tests; the warning reaches the user through `Effect::LowDisk`
    #[allow(dead_code)]
    pub fn low_disk_warning(&self) -> Option<&str> {
        self.low_disk.as_deref()
    }

    /// True when the last recording was stopped by the < 200 MB guard (kept until the next `begin`).
    pub fn stopped_for_low_disk(&self) -> bool {
        self.stopped_for_disk
    }

    pub fn is_idle(&self) -> bool {
        self.phase == Phase::Idle
    }

    /// Start a recording: counts `countdown_secs` first (0 starts at once). Ignored unless idle.
    pub fn begin(&mut self, countdown_secs: u32) -> Effect {
        if !self.is_idle() {
            return Effect::None;
        }
        self.elapsed = 0.0;
        self.dropped = 0;
        self.low_disk = None;
        self.stopped_for_disk = false;
        let countdown = Countdown::new(countdown_secs);
        match countdown.current() {
            Some(n) => {
                self.phase = Phase::Countdown(countdown);
                Effect::ShowCountdown(n)
            }
            None => {
                self.phase = Phase::Recording;
                Effect::StartRecording
            }
        }
    }

    /// One second of countdown passed.
    pub fn tick_countdown(&mut self) -> Effect {
        let Phase::Countdown(mut c) = self.phase else { return Effect::None };
        match c.tick() {
            CountdownStep::Show(n) => {
                self.phase = Phase::Countdown(c);
                Effect::ShowCountdown(n)
            }
            CountdownStep::Finished => {
                self.phase = Phase::Recording;
                Effect::StartRecording
            }
        }
    }

    /// Stop (HUD, tray or shortcut). During the countdown it cancels; otherwise it asks the
    /// helper to finish the file. Repeated requests send nothing.
    pub fn request_stop(&mut self) -> Effect {
        match self.phase {
            Phase::Countdown(_) => {
                self.phase = Phase::Idle;
                Effect::CancelCountdown
            }
            Phase::Recording | Phase::Paused => {
                self.phase = Phase::Stopping;
                Effect::SendStop
            }
            Phase::Idle | Phase::Stopping => Effect::None,
        }
    }

    pub fn toggle_pause(&mut self) -> Effect {
        match self.phase {
            Phase::Recording => {
                self.phase = Phase::Paused;
                Effect::SendPause
            }
            Phase::Paused => {
                self.phase = Phase::Recording;
                Effect::SendResume
            }
            _ => Effect::None,
        }
    }

    fn running(&self) -> bool {
        matches!(self.phase, Phase::Recording | Phase::Paused | Phase::Stopping)
    }

    /// Feed one helper event.
    pub fn on_event(&mut self, ev: &RecordEvent) -> Effect {
        if !self.running() {
            return Effect::None;
        }
        match ev {
            RecordEvent::Tick { elapsed, dropped, .. } => {
                self.elapsed = *elapsed;
                self.dropped = *dropped;
                Effect::None
            }
            RecordEvent::State { state, message } => match state {
                RecordState::Recording if self.phase == Phase::Paused => {
                    self.phase = Phase::Recording;
                    Effect::None
                }
                RecordState::Paused if self.phase == Phase::Recording => {
                    self.phase = Phase::Paused;
                    Effect::None
                }
                RecordState::Stopped => self.on_stopped(),
                RecordState::Failed => self.finish(Some(message.clone().unwrap_or_else(|| "The recording failed".into()))),
                _ => Effect::None,
            },
            RecordEvent::Warn { code, free_bytes: Some(free), .. } if code == "low-disk" => match hud::disk_level(*free) {
                DiskLevel::Ok => {
                    self.low_disk = None;
                    Effect::None
                }
                DiskLevel::Low => match hud::low_disk_message(*free) {
                    Some(text) if self.low_disk.as_deref() != Some(text.as_str()) => {
                        self.low_disk = Some(text.clone());
                        Effect::LowDisk(text)
                    }
                    _ => Effect::None,
                },
                DiskLevel::Critical => {
                    self.low_disk = hud::low_disk_message(*free);
                    self.stopped_for_disk = true;
                    self.request_stop()
                }
            },
            RecordEvent::Warn { .. } => Effect::None,
        }
    }

    /// The helper finished the file (`record.stop` replied or a `stopped` state arrived).
    pub fn on_stopped(&mut self) -> Effect {
        self.finish(None)
    }

    fn finish(&mut self, error: Option<String>) -> Effect {
        if self.is_idle() {
            return Effect::None;
        }
        self.phase = Phase::Idle;
        self.elapsed = 0.0;
        self.dropped = 0;
        self.low_disk = None;
        Effect::Ended { error }
    }

    /// Menu-bar title (`●  mm:ss`) while a recording exists; `None` otherwise.
    pub fn tray_title(&self) -> Option<String> {
        self.running().then(|| hud::tray_title(self.elapsed))
    }

    /// Whether the tray menu offers `Stop Recording`.
    pub fn shows_stop_item(&self) -> bool {
        matches!(self.phase, Phase::Recording | Phase::Paused)
    }
}

impl Default for RecordCtl {
    fn default() -> Self {
        Self::new()
    }
}

// ---- T19: what the launcher commands and the recording flow decide (pure unless noted) ----

/// Which recording command was picked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordKind {
    Screen,
    Area,
    Window,
}

pub const OPEN_LAST_ID: &str = "builtin:open-last-recording";
pub const RECORDINGS_ID: &str = "builtin:recordings";
/// S12: the start sheet text for `warn input-monitoring-off`; recording goes on.
pub const INPUT_MONITORING_TOAST: &str = "Keyboard shortcuts will not be shown: Input Monitoring is off.";

impl RecordKind {
    pub const ALL: [RecordKind; 3] = [RecordKind::Screen, RecordKind::Area, RecordKind::Window];

    pub fn command_id(self) -> &'static str {
        match self {
            RecordKind::Screen => "builtin:record-screen",
            RecordKind::Area => "builtin:record-area",
            RecordKind::Window => "builtin:record-window",
        }
    }

    pub fn from_command_id(id: &str) -> Option<RecordKind> {
        Self::ALL.into_iter().find(|k| k.command_id() == id)
    }

    /// The screenshot kind whose target picker this command reuses.
    pub fn capture_kind(self) -> crate::capture_ctl::CaptureKind {
        use crate::capture_ctl::CaptureKind;
        match self {
            RecordKind::Screen => CaptureKind::Screen,
            RecordKind::Area => CaptureKind::Area,
            RecordKind::Window => CaptureKind::Window,
        }
    }
}

/// Commands that need macOS 14 and the helper (S1, S3): every capture and recording command.
pub fn is_gated_command(id: &str) -> bool {
    crate::capture_ctl::CaptureKind::from_command_id(id).is_some()
        || RecordKind::from_command_id(id).is_some()
        || id == OPEN_LAST_ID
        || id == RECORDINGS_ID
}

/// `Recording YYYY-MM-DD at HH.MM.SS`, the project's name (the folder adds `.ferrocast`).
pub fn project_name(now: NaiveDateTime) -> String {
    format!("Recording {}", now.format("%Y-%m-%d at %H.%M.%S"))
}

/// Display (top-left points) the HUD and countdown go to: the target's display, or the display
/// holding the centre of a window. `None` lets the panels use the main screen.
pub fn display_frame_for(target: &Target, shareable: &Shareable) -> Option<[f64; 4]> {
    let by_id = |id: u32| shareable.displays.iter().find(|d| d.id == id).map(|d| d.frame);
    match target {
        Target::Display { display } | Target::Rect { display, .. } => by_id(*display),
        Target::Window { window } => {
            let w = shareable.windows.iter().find(|w| w.id == *window)?;
            let (cx, cy) = (w.frame[0] + w.frame[2] / 2.0, w.frame[1] + w.frame[3] / 2.0);
            shareable
                .displays
                .iter()
                .find(|d| cx >= d.frame[0] && cx < d.frame[0] + d.frame[2] && cy >= d.frame[1] && cy < d.frame[1] + d.frame[3])
                .map(|d| d.frame)
        }
    }
}

/// Why `record.start` failed, as the app shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StartFailure {
    /// S4: the permission toast and the Settings pane.
    Permission,
    Message(String),
}

pub fn start_failure(e: &RecordError) -> StartFailure {
    match e {
        RecordError::PermissionDenied => StartFailure::Permission,
        RecordError::Busy => StartFailure::Message("A recording is already running".into()),
        other => StartFailure::Message(format!("Recording failed: {other}")),
    }
}

/// Toast for a helper `warn` code (`None`: nothing to say; disk warnings go through [`RecordCtl`]).
pub fn warn_toast(code: &str) -> Option<&'static str> {
    (code == "input-monitoring-off").then_some(INPUT_MONITORING_TOAST)
}

/// What the user is told once the project is finished.
pub fn saved_toast(name: &str, duration: f64, interrupted: bool, message: Option<&str>, low_disk: bool) -> String {
    let len = format_duration(duration);
    if low_disk {
        format!("Recording stopped: low disk space. {name} ({len}) was kept")
    } else if interrupted {
        match message {
            Some(m) if !m.is_empty() => format!("Recording stopped early: {m}. {name} ({len}) was kept"),
            _ => format!("Recording stopped early. {name} ({len}) was kept"),
        }
    } else {
        format!("Recording saved: {name} ({len})")
    }
}

/// One project folder in the Recordings browser.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordingEntry {
    pub path: PathBuf,
    /// Folder name without `.ferrocast`.
    pub name: String,
    /// Seconds from `project.json`; `None` when unknown (not finished or unreadable).
    pub duration: Option<f64>,
    /// Bytes of the files directly inside the folder.
    pub bytes: u64,
    /// Unix seconds: `project.json` `created`, else the folder's modification time.
    pub created: i64,
}

/// `m:ss`, or `h:mm:ss` from one hour.
pub fn format_duration(secs: f64) -> String {
    let t = if secs.is_finite() && secs > 0.0 { secs.round() as u64 } else { 0 };
    let (h, m, s) = (t / 3600, (t / 60) % 60, t % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// Decimal units, like the disk warning: `812 KB`, `48 MB`, `1.2 GB`.
pub fn format_size(bytes: u64) -> String {
    const KB: f64 = 1e3;
    let b = bytes as f64;
    if b < KB {
        format!("{bytes} B")
    } else if b < 1e6 {
        format!("{:.0} KB", b / KB)
    } else if b < 1e9 {
        format!("{:.0} MB", b / 1e6)
    } else {
        format!("{:.1} GB", b / 1e9)
    }
}

/// `1:05 · 48 MB` (just the size when the duration is unknown).
pub fn entry_subtitle(e: &RecordingEntry) -> String {
    match e.duration {
        Some(d) => format!("{} · {}", format_duration(d), format_size(e.bytes)),
        None => format_size(e.bytes),
    }
}

/// `23 Oct 2026, 14:05`.
pub fn format_date(t: NaiveDateTime) -> String {
    t.format("%d %b %Y, %H:%M").to_string()
}

/// Newest first; equal times fall back to the name so the order is stable.
pub fn sort_newest_first(v: &mut [RecordingEntry]) {
    v.sort_by(|a, b| b.created.cmp(&a.created).then_with(|| a.name.cmp(&b.name)));
}

/// Reads one `.ferrocast` folder (disk). `None` when it is not a project folder.
pub fn read_entry(dir: &Path) -> Option<RecordingEntry> {
    use ferrocast_core::capture::project::{Project, EXTENSION};
    if !dir.is_dir() || dir.extension().and_then(|e| e.to_str()) != Some(EXTENSION) {
        return None;
    }
    let name = dir.file_stem()?.to_string_lossy().to_string();
    let project = Project::load(dir).ok();
    let bytes = std::fs::read_dir(dir)
        .map(|rd| rd.flatten().filter_map(|e| e.metadata().ok()).filter(|m| m.is_file()).map(|m| m.len()).sum())
        .unwrap_or(0);
    let from_project = project.as_ref().and_then(|p| chrono::DateTime::parse_from_rfc3339(&p.created).ok()).map(|d| d.timestamp());
    let created = from_project.or_else(|| {
        let modified = std::fs::metadata(dir).ok()?.modified().ok()?;
        Some(modified.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() as i64)
    })?;
    let duration = project.map(|p| p.duration).filter(|d| *d > 0.0);
    Some(RecordingEntry { path: dir.to_path_buf(), name, duration, bytes, created })
}

/// Every project folder in `dir`, newest first (disk).
pub fn scan(dir: &Path) -> Vec<RecordingEntry> {
    let mut v: Vec<RecordingEntry> =
        std::fs::read_dir(dir).map(|rd| rd.flatten().filter_map(|e| read_entry(&e.path())).collect()).unwrap_or_default();
    sort_newest_first(&mut v);
    v
}

/// The newest project folder in `dir`.
pub fn newest(dir: &Path) -> Option<PathBuf> {
    scan(dir).into_iter().next().map(|e| e.path)
}

/// Removes a project folder that never got a `screen.mov` (failed or cancelled start). Only
/// `.ferrocast` folders are touched. Returns whether it was removed (disk).
pub fn discard_unfinished(project: &Path) -> bool {
    use ferrocast_core::capture::project::EXTENSION;
    let is_project = project.extension().and_then(|e| e.to_str()) == Some(EXTENSION);
    if !is_project || project.join("screen.mov").exists() {
        return false;
    }
    std::fs::remove_dir_all(project).is_ok()
}

/// After a clean stop: record the duration in `project.json` and make `thumb.png` (disk).
/// The thumbnail is best effort: a missing one only costs the Recordings browser its picture.
pub fn finalize_project(project: &Path, done: &RecordStopped) -> Result<(), String> {
    use ferrocast_core::capture::project::Project;
    let mut p = Project::load(project).map_err(|e| e.to_string())?;
    p.duration = done.duration;
    p.sources.screen.file = done.screen.clone();
    p.save(project).map_err(|e| e.to_string())?;
    make_thumb(project, &done.screen);
    Ok(())
}

/// `thumb.png` (480 px) from the recording through Quick Look (`qlmanage`, no ffmpeg needed).
// TODO(T19): Quick Look picks its own poster frame, which is usually the first one but is not
// guaranteed; an exact first frame needs a helper command (`render.frame` in T24 will give it).
fn make_thumb(project: &Path, screen: &str) -> bool {
    use ferrocast_core::capture::project::thumb_path;
    let mut cmd = std::process::Command::new("/usr/bin/qlmanage");
    cmd.args(["-t", "-s", "480", "-o"]).arg(project).arg(project.join(screen));
    if crate::route::run_limited(&mut cmd, 20).is_err() {
        return false;
    }
    let mut made = project.join(screen).into_os_string();
    made.push(".png");
    std::fs::rename(made, thumb_path(project)).is_ok()
}

// ---- the live session: a helper kept alive for one recording (not runnable without a grant) ----

/// What the app thread asks the session thread to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    Pause,
    Resume,
    Stop,
}

/// A finished project, as the app reports it.
#[derive(Clone, Debug, PartialEq)]
pub struct Saved {
    pub project: PathBuf,
    pub duration: f64,
    pub interrupted: bool,
    pub message: Option<String>,
}

/// Everything the session thread and the HUD send back to the app (`Msg::Record`). `u64` is the
/// session number: messages of an older recording are ignored.
pub enum RecordMsg {
    /// One second of countdown passed.
    CountdownTick(u64),
    /// HUD Pause/Resume button.
    TogglePause,
    /// HUD Stop button.
    Stop,
    Started(u64, Result<(), StartFailure>),
    Event(u64, RecordEvent),
    Finished(u64, Result<Saved, String>),
}

/// App-side runtime of the current recording.
#[derive(Default)]
pub struct Runtime {
    /// Session number of the current/last recording.
    pub gen: u64,
    /// Chosen but not started yet (the countdown runs).
    pub pending: Option<Pending>,
    pub cmds: Option<std::sync::mpsc::Sender<Cmd>>,
    /// Project folder being written.
    pub project: Option<PathBuf>,
    /// Content of the Recordings browser (reloaded when it opens and after a trash).
    pub recordings: Vec<RecordingEntry>,
}

#[derive(Clone, Debug)]
pub struct Pending {
    pub target: Target,
    /// Display (top-left points) for the countdown and the HUD.
    pub display: Option<[f64; 4]>,
}

/// Inputs of the session thread.
pub struct Session {
    pub exe: PathBuf,
    pub project: PathBuf,
    pub target: Target,
    pub fps: u32,
    pub cursor: bool,
    pub excludes: Vec<u32>,
    pub gen: u64,
}

/// Session thread: spawn a helper, `record.start`, forward events, obey [`Cmd`]s, `record.stop`,
/// finalise the project. Ends with `Started(Err)` or exactly one `Finished`.
pub fn run_session(s: Session, cmds: std::sync::mpsc::Receiver<Cmd>, bus: crate::services::Bus, log: impl Fn(&str) + Send + 'static) {
    use crate::services::Msg;
    use std::sync::mpsc::{RecvTimeoutError, TryRecvError};
    let gen = s.gen;
    let say = |m: RecordMsg| bus.send(Msg::Record(m));
    let mut helper = match ferrocast_macos::capture::Helper::spawn(&ferrocast_macos::capture::helper_path(&s.exe), log) {
        Ok(h) => h,
        Err(e) => return say(RecordMsg::Started(gen, Err(StartFailure::Message(format!("Recording failed: {e}"))))),
    };
    let events = match helper.record_start(&s.project, "screen.mov", s.target, s.fps, s.cursor, s.excludes) {
        Ok((_, rx)) => rx,
        Err(e) => return say(RecordMsg::Started(gen, Err(start_failure(&e)))),
    };
    say(RecordMsg::Started(gen, Ok(())));
    let finish = |helper: &mut ferrocast_macos::capture::Helper| {
        let result = helper.record_stop().map_err(|e| e.to_string()).and_then(|done| {
            finalize_project(&s.project, &done)?;
            Ok(Saved { project: s.project.clone(), duration: done.duration, interrupted: done.interrupted, message: done.message })
        });
        say(RecordMsg::Finished(gen, result));
    };
    loop {
        match events.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(ev) => {
                // the helper finalised the file by itself (stream lost): collect what it kept
                let lost = matches!(ev, RecordEvent::State { state: RecordState::Failed, .. });
                say(RecordMsg::Event(gen, ev));
                if lost {
                    return finish(&mut helper);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                return say(RecordMsg::Finished(gen, Err("The capture helper stopped unexpectedly".into())))
            }
        }
        loop {
            match cmds.try_recv() {
                Ok(Cmd::Pause) => drop(helper.record_pause()),
                Ok(Cmd::Resume) => drop(helper.record_resume()),
                Ok(Cmd::Stop) => return finish(&mut helper),
                Err(TryRecvError::Empty) => break,
                // the app gave the recording up: dropping the helper ends it
                Err(TryRecvError::Disconnected) => return,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tick(elapsed: f64, dropped: u64) -> RecordEvent {
        RecordEvent::Tick { elapsed, frames: 0, dropped }
    }
    fn state(s: RecordState) -> RecordEvent {
        RecordEvent::State { state: s, message: None }
    }
    fn low_disk(free: u64) -> RecordEvent {
        RecordEvent::Warn { code: "low-disk".into(), message: None, free_bytes: Some(free) }
    }
    fn recording() -> RecordCtl {
        let mut c = RecordCtl::new();
        assert_eq!(c.begin(0), Effect::StartRecording);
        c
    }

    #[test]
    fn starts_idle_with_no_tray_state() {
        let c = RecordCtl::new();
        assert!(c.is_idle());
        assert_eq!(c.tray_title(), None);
        assert!(!c.shows_stop_item());
        assert_eq!(c.elapsed(), 0.0);
    }

    #[test]
    fn countdown_runs_3_2_1_then_starts_recording() {
        let mut c = RecordCtl::new();
        assert_eq!(c.begin(3), Effect::ShowCountdown(3));
        assert!(matches!(c.phase(), Phase::Countdown(_)));
        assert_eq!(c.tray_title(), None, "nothing is recording during the countdown");
        assert_eq!(c.tick_countdown(), Effect::ShowCountdown(2));
        assert_eq!(c.tick_countdown(), Effect::ShowCountdown(1));
        assert_eq!(c.tick_countdown(), Effect::StartRecording);
        assert_eq!(c.phase(), Phase::Recording);
        assert_eq!(c.tick_countdown(), Effect::None, "ticks outside a countdown do nothing");
    }

    #[test]
    fn zero_countdown_starts_at_once() {
        let mut c = RecordCtl::new();
        assert_eq!(c.begin(0), Effect::StartRecording);
        assert_eq!(c.phase(), Phase::Recording);
    }

    #[test]
    fn begin_is_ignored_unless_idle() {
        let mut c = recording();
        assert_eq!(c.begin(3), Effect::None);
        assert_eq!(c.phase(), Phase::Recording);
    }

    #[test]
    fn stop_during_countdown_cancels_it() {
        let mut c = RecordCtl::new();
        c.begin(3);
        assert_eq!(c.request_stop(), Effect::CancelCountdown);
        assert!(c.is_idle());
    }

    #[test]
    fn pause_and_resume_toggle() {
        let mut c = recording();
        assert_eq!(c.toggle_pause(), Effect::SendPause);
        assert_eq!(c.phase(), Phase::Paused);
        assert_eq!(c.toggle_pause(), Effect::SendResume);
        assert_eq!(c.phase(), Phase::Recording);
        let mut idle = RecordCtl::new();
        assert_eq!(idle.toggle_pause(), Effect::None);
    }

    #[test]
    fn stop_goes_through_stopping_and_back_to_idle() {
        let mut c = recording();
        assert_eq!(c.request_stop(), Effect::SendStop);
        assert_eq!(c.phase(), Phase::Stopping);
        assert_eq!(c.request_stop(), Effect::None, "a second Stop is not sent twice");
        assert_eq!(c.toggle_pause(), Effect::None, "no pause while stopping");
        assert_eq!(c.on_stopped(), Effect::Ended { error: None });
        assert!(c.is_idle());
    }

    #[test]
    fn stop_from_paused_works() {
        let mut c = recording();
        c.toggle_pause();
        assert_eq!(c.request_stop(), Effect::SendStop);
    }

    #[test]
    fn ticks_update_elapsed_and_dropped_only_while_a_recording_runs() {
        let mut idle = RecordCtl::new();
        idle.on_event(&tick(5.0, 2));
        assert_eq!((idle.elapsed(), idle.dropped()), (0.0, 0));
        let mut c = recording();
        assert_eq!(c.on_event(&tick(12.4, 3)), Effect::None);
        assert_eq!((c.elapsed(), c.dropped()), (12.4, 3));
        c.toggle_pause();
        c.on_event(&tick(12.4, 3));
        assert_eq!(c.elapsed(), 12.4);
    }

    #[test]
    fn tray_title_follows_elapsed_while_recording_or_paused() {
        let mut c = recording();
        c.on_event(&tick(12.0, 0));
        assert_eq!(c.tray_title().as_deref(), Some("\u{25CF}  00:12"));
        assert!(c.shows_stop_item());
        c.toggle_pause();
        assert_eq!(c.tray_title().as_deref(), Some("\u{25CF}  00:12"));
        assert!(c.shows_stop_item());
        c.request_stop();
        assert!(c.tray_title().is_some(), "title stays until the file is finished");
        assert!(!c.shows_stop_item(), "Stop Recording disappears once stopping");
        c.on_stopped();
        assert_eq!(c.tray_title(), None);
    }

    #[test]
    fn helper_state_events_confirm_pause_and_resume() {
        let mut c = recording();
        c.on_event(&state(RecordState::Paused));
        assert_eq!(c.phase(), Phase::Paused);
        c.on_event(&state(RecordState::Recording));
        assert_eq!(c.phase(), Phase::Recording);
    }

    #[test]
    fn helper_stopped_or_failed_ends_the_recording() {
        let mut c = recording();
        assert_eq!(c.on_event(&state(RecordState::Stopped)), Effect::Ended { error: None });
        assert!(c.is_idle());
        let mut c = recording();
        let failed = RecordEvent::State { state: RecordState::Failed, message: Some("writer failed".into()) };
        assert_eq!(c.on_event(&failed), Effect::Ended { error: Some("writer failed".into()) });
        assert!(c.is_idle());
    }

    #[test]
    fn low_disk_warns_under_2_gb_once_and_keeps_recording() {
        let mut c = recording();
        assert_eq!(c.on_event(&low_disk(1_500_000_000)), Effect::LowDisk("Low disk space: 1.5 GB left".into()));
        assert_eq!(c.phase(), Phase::Recording);
        assert_eq!(c.low_disk_warning(), Some("Low disk space: 1.5 GB left"));
        assert_eq!(c.on_event(&low_disk(1_500_000_000)), Effect::None, "same text is not repeated");
        assert_eq!(c.on_event(&low_disk(1_400_000_000)), Effect::LowDisk("Low disk space: 1.4 GB left".into()));
    }

    #[test]
    fn low_disk_above_2_gb_is_not_a_warning() {
        let mut c = recording();
        assert_eq!(c.on_event(&low_disk(2_000_000_000)), Effect::None);
        assert_eq!(c.low_disk_warning(), None);
    }

    #[test]
    fn under_200_mb_stops_cleanly() {
        let mut c = recording();
        assert_eq!(c.on_event(&low_disk(199_999_999)), Effect::SendStop);
        assert_eq!(c.phase(), Phase::Stopping);
        assert!(c.stopped_for_low_disk());
        assert_eq!(c.on_event(&low_disk(100_000_000)), Effect::None, "already stopping");
        c.on_stopped();
        assert!(c.stopped_for_low_disk(), "the caller can still tell why it stopped");
        c.begin(0);
        assert!(!c.stopped_for_low_disk(), "a new recording starts clean");
        assert_eq!(c.low_disk_warning(), None);
    }

    #[test]
    fn other_warnings_are_ignored() {
        let mut c = recording();
        let w = RecordEvent::Warn { code: "dropped-frames".into(), message: Some("3 frames dropped".into()), free_bytes: None };
        assert_eq!(c.on_event(&w), Effect::None);
        assert_eq!(c.phase(), Phase::Recording);
    }

    #[test]
    fn ended_resets_the_counters_for_the_next_recording() {
        let mut c = recording();
        c.on_event(&tick(30.0, 4));
        c.on_stopped();
        assert_eq!((c.elapsed(), c.dropped()), (0.0, 0));
        c.begin(0);
        assert_eq!(c.tray_title().as_deref(), Some("\u{25CF}  00:00"));
    }

    // ---- T19 ----

    use chrono::NaiveDate;
    use ferrocast_core::capture::protocol::{Display, Window};

    fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32, sec: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, mo, d).unwrap().and_hms_opt(h, mi, sec).unwrap()
    }

    fn two_displays() -> Shareable {
        let display = |id, x, main| Display { id, frame: [x, 0.0, 1000.0, 800.0], scale: 2.0, main, name: String::new() };
        let win = |id, x, y| Window {
            id,
            app: String::new(),
            bundle: String::new(),
            title: String::new(),
            frame: [x, y, 200.0, 100.0],
            layer: 0,
            on_screen: true,
        };
        Shareable {
            displays: vec![display(1, 0.0, true), display(2, 1000.0, false)],
            windows: vec![win(10, 100.0, 100.0), win(11, 1100.0, 50.0), win(12, 900.0, 50.0)],
        }
    }

    #[test]
    fn record_commands_have_stable_ids_and_reuse_the_capture_pickers() {
        use crate::capture_ctl::CaptureKind;
        for k in RecordKind::ALL {
            assert_eq!(RecordKind::from_command_id(k.command_id()), Some(k));
        }
        assert_eq!(RecordKind::from_command_id("builtin:capture-area"), None);
        assert_eq!(RecordKind::Screen.capture_kind(), CaptureKind::Screen);
        assert_eq!(RecordKind::Area.capture_kind(), CaptureKind::Area);
        assert_eq!(RecordKind::Window.capture_kind(), CaptureKind::Window);
    }

    #[test]
    fn every_capture_and_recording_command_is_gated_together() {
        for id in [
            "builtin:capture-area",
            "builtin:capture-scrolling",
            "builtin:record-screen",
            "builtin:record-window",
            OPEN_LAST_ID,
            RECORDINGS_ID,
        ] {
            assert!(is_gated_command(id), "{id}");
        }
        assert!(!is_gated_command("builtin:notes"));
        assert!(!is_gated_command("builtin:ferrocast-settings"));
    }

    #[test]
    fn project_names_carry_the_start_time() {
        assert_eq!(project_name(at(2026, 10, 23, 14, 5, 22)), "Recording 2026-10-23 at 14.05.22");
    }

    #[test]
    fn hud_display_follows_the_target() {
        let s = two_displays();
        assert_eq!(display_frame_for(&Target::Display { display: 2 }, &s), Some([1000.0, 0.0, 1000.0, 800.0]));
        assert_eq!(display_frame_for(&Target::Rect { display: 1, rect: [0.0, 0.0, 10.0, 10.0] }, &s), Some([0.0, 0.0, 1000.0, 800.0]));
        // a window belongs to the display holding its centre
        assert_eq!(display_frame_for(&Target::Window { window: 10 }, &s), Some([0.0, 0.0, 1000.0, 800.0]));
        assert_eq!(display_frame_for(&Target::Window { window: 11 }, &s), Some([1000.0, 0.0, 1000.0, 800.0]));
        assert_eq!(
            display_frame_for(&Target::Window { window: 12 }, &s),
            Some([1000.0, 0.0, 1000.0, 800.0]),
            "centre x 1000 is the first column of display 2"
        );
        assert_eq!(display_frame_for(&Target::Window { window: 99 }, &s), None);
        assert_eq!(display_frame_for(&Target::Display { display: 7 }, &s), None);
    }

    #[test]
    fn start_failures_map_to_the_s4_text_or_a_message() {
        assert_eq!(start_failure(&RecordError::PermissionDenied), StartFailure::Permission);
        assert_eq!(start_failure(&RecordError::Busy), StartFailure::Message("A recording is already running".into()));
        assert_eq!(start_failure(&RecordError::Timeout), StartFailure::Message("Recording failed: recording request timed out".into()));
    }

    #[test]
    fn only_input_monitoring_has_a_start_toast() {
        assert_eq!(warn_toast("input-monitoring-off"), Some("Keyboard shortcuts will not be shown: Input Monitoring is off."));
        assert_eq!(warn_toast("low-disk"), None);
        assert_eq!(warn_toast("dropped-frames"), None);
    }

    #[test]
    fn saved_toast_says_why_a_recording_ended() {
        assert_eq!(saved_toast("Demo", 65.4, false, None, false), "Recording saved: Demo (1:05)");
        assert_eq!(saved_toast("Demo", 65.0, false, None, true), "Recording stopped: low disk space. Demo (1:05) was kept");
        assert_eq!(
            saved_toast("Demo", 3.0, true, Some("display lost"), false),
            "Recording stopped early: display lost. Demo (0:03) was kept"
        );
        assert_eq!(saved_toast("Demo", 3.0, true, None, false), "Recording stopped early. Demo (0:03) was kept");
    }

    #[test]
    fn durations_sizes_and_dates_read_like_labels() {
        assert_eq!(format_duration(0.0), "0:00");
        assert_eq!(format_duration(59.6), "1:00");
        assert_eq!(format_duration(65.0), "1:05");
        assert_eq!(format_duration(3725.0), "1:02:05");
        assert_eq!(format_duration(f64::NAN), "0:00");
        assert_eq!(format_duration(-4.0), "0:00");
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(999), "999 B");
        assert_eq!(format_size(48_300), "48 KB");
        assert_eq!(format_size(48_300_000), "48 MB");
        assert_eq!(format_size(1_250_000_000), "1.2 GB");
        assert_eq!(format_date(at(2026, 10, 3, 9, 5, 0)), "03 Oct 2026, 09:05");
    }

    fn entry(name: &str, created: i64, duration: Option<f64>) -> RecordingEntry {
        RecordingEntry { path: PathBuf::from(format!("/r/{name}.ferrocast")), name: name.into(), duration, bytes: 48_000_000, created }
    }

    #[test]
    fn entries_sort_newest_first_with_a_stable_tie_break() {
        let mut v = vec![entry("b", 100, None), entry("old", 10, None), entry("a", 100, None), entry("new", 500, None)];
        sort_newest_first(&mut v);
        assert_eq!(v.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["new", "a", "b", "old"]);
    }

    #[test]
    fn subtitle_shows_duration_and_size_or_size_alone() {
        assert_eq!(entry_subtitle(&entry("a", 0, Some(65.0))), "1:05 · 48 MB");
        assert_eq!(entry_subtitle(&entry("a", 0, None)), "48 MB");
    }

    /// A scratch folder under the system temp dir, removed on drop.
    struct Scratch(PathBuf);
    impl Scratch {
        fn new(tag: &str) -> Scratch {
            let p = std::env::temp_dir().join(format!("fc-rec-{tag}-{}", std::process::id()));
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

    fn project(root: &Path, folder: &str, created: &str, duration: f64, bytes: usize) -> PathBuf {
        use ferrocast_core::capture::project::Project;
        let dir = root.join(folder);
        let mut p = Project::create(&dir, "x", created).unwrap();
        p.duration = duration;
        p.save(&dir).unwrap();
        std::fs::write(dir.join("screen.mov"), vec![0u8; bytes]).unwrap();
        dir
    }

    #[test]
    fn scan_lists_projects_newest_first_and_skips_everything_else() {
        let s = Scratch::new("scan");
        project(&s.0, "Old.ferrocast", "2026-10-01T10:00:00Z", 12.0, 1000);
        project(&s.0, "New.ferrocast", "2026-10-23T14:05:22Z", 73.2, 5000);
        std::fs::create_dir_all(s.0.join("Not a project")).unwrap();
        std::fs::write(s.0.join("stray.ferrocast"), "a file, not a folder").unwrap();
        std::fs::write(s.0.join("notes.txt"), "x").unwrap();
        let v = scan(&s.0);
        assert_eq!(v.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["New", "Old"]);
        assert_eq!(v[0].duration, Some(73.2));
        assert!(v[0].bytes >= 5000, "screen.mov counts toward the size");
        assert_eq!(v[0].created, chrono::DateTime::parse_from_rfc3339("2026-10-23T14:05:22Z").unwrap().timestamp());
        assert_eq!(newest(&s.0), Some(s.0.join("New.ferrocast")));
        assert_eq!(newest(&s.0.join("missing")), None);
    }

    #[test]
    fn an_unfinished_project_has_no_duration_and_a_broken_one_still_lists() {
        let s = Scratch::new("odd");
        project(&s.0, "Fresh.ferrocast", "2026-10-23T14:05:22Z", 0.0, 10);
        let broken = s.0.join("Broken.ferrocast");
        std::fs::create_dir_all(&broken).unwrap();
        std::fs::write(broken.join("project.json"), "{not json").unwrap();
        let v = scan(&s.0);
        assert_eq!(v.len(), 2);
        assert_eq!(v.iter().find(|e| e.name == "Fresh").unwrap().duration, None);
        assert_eq!(v.iter().find(|e| e.name == "Broken").unwrap().duration, None, "falls back to the folder time");
    }

    #[test]
    fn discard_removes_only_a_ferrocast_folder_without_a_recording() {
        use ferrocast_core::capture::project::Project;
        let s = Scratch::new("discard");
        let empty = s.0.join("Empty.ferrocast");
        Project::create(&empty, "x", "2026-10-23T14:05:22Z").unwrap();
        assert!(discard_unfinished(&empty));
        assert!(!empty.exists());
        let kept = project(&s.0, "Kept.ferrocast", "2026-10-23T14:05:22Z", 1.0, 10);
        assert!(!discard_unfinished(&kept), "a folder with screen.mov is never removed");
        assert!(kept.exists());
        let other = s.0.join("Documents");
        std::fs::create_dir_all(&other).unwrap();
        assert!(!discard_unfinished(&other), "not a project folder");
        assert!(other.exists());
    }

    #[test]
    fn finalize_writes_the_duration_into_project_json() {
        use ferrocast_core::capture::project::Project;
        let s = Scratch::new("final");
        let dir = project(&s.0, "Demo.ferrocast", "2026-10-23T14:05:22Z", 0.0, 10);
        let done = RecordStopped {
            project: dir.to_string_lossy().into(),
            duration: 73.2,
            screen: "screen.mov".into(),
            size: [3456, 2234],
            frames: 0,
            dropped: 0,
            interrupted: false,
            message: None,
            sources: None,
        };
        finalize_project(&dir, &done).unwrap();
        assert_eq!(Project::load(&dir).unwrap().duration, 73.2);
        assert_eq!(read_entry(&dir).unwrap().duration, Some(73.2));
        assert!(finalize_project(&s.0.join("nope.ferrocast"), &done).is_err());
    }
}
