//! The studio editor view (T26): the pure logic behind the preview, the scrub queue, the timeline
//! widget and the playback clock, plus the render worker that owns the long-lived capture helper.
//!
//! Times are SOURCE seconds of the take unless a name says output. `render.frame` takes source
//! time; the labels map to output time through `timeline::Timeline::to_output`.
//!
//! T26 does not write `project.json`: autosave and every other edit come in T27, so applying the
//! auto zooms only marks the in-memory project dirty.

use ferrocast_core::capture::compose::{Composer, FrameParams};
use ferrocast_core::capture::events::Events;
use ferrocast_core::capture::project::{Project, EVENTS_FILE};
use ferrocast_core::capture::timeline::Timeline;
use ferrocast_core::capture::zoom::{self, ZoomOpts};
use ferrocast_macos::capture::RenderedFrame;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Instant;

/// Widest preview the editor asks for (S15 measures 960 px).
pub const PREVIEW_MAX_WIDTH: u32 = 960;
/// Frames requested per second while playing.
pub const PLAY_FPS: f64 = 30.0;

// ---------------------------------------------------------------------------------------------
// Scrub queue: only the LATEST requested frame is rendered while one is in flight.

/// What to render: a source time and a preview width.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Key {
    pub t: f64,
    pub width: u32,
}

/// A request handed to the render worker. `seq` grows with every request.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Req {
    pub seq: u64,
    pub key: Key,
}

/// Latest-wins coalescing: at most one render is in flight; requests that arrive meanwhile only
/// replace the single pending one. Replies are shown only when newer than the one on screen.
#[derive(Debug, Default)]
pub struct ScrubQueue {
    next_seq: u64,
    in_flight: Option<Req>,
    pending: Option<Key>,
    /// Key of the last frame that rendered, so asking for it again costs nothing.
    last_done: Option<Key>,
    /// A key that failed: not asked again until [`ScrubQueue::invalidate`] (no error flood).
    last_failed: Option<Key>,
    shown_seq: u64,
    /// The in-flight render started before the last [`ScrubQueue::invalidate`].
    stale: bool,
}

impl ScrubQueue {
    pub fn new() -> Self {
        Self::default()
    }

    fn start(&mut self, key: Key) -> Req {
        self.next_seq += 1;
        let req = Req { seq: self.next_seq, key };
        self.in_flight = Some(req);
        req
    }

    /// Ask for a frame. Returns the request to send to the worker now, or `None` when it is
    /// coalesced (a render is in flight), a duplicate, or known to fail.
    pub fn request(&mut self, key: Key) -> Option<Req> {
        if !key.t.is_finite() {
            return None;
        }
        if let Some(flying) = self.in_flight {
            // only the newest wish is kept; wishing for the frame being rendered cancels the older wish
            self.pending = (flying.key != key).then_some(key);
            return None;
        }
        if self.last_done == Some(key) || self.last_failed == Some(key) {
            return None;
        }
        Some(self.start(key))
    }

    /// The worker answered request `seq`. Returns the next request to send (the pending one).
    /// A reply that is not the one in flight is ignored.
    pub fn complete(&mut self, seq: u64, ok: bool) -> Option<Req> {
        let flying = self.in_flight.filter(|f| f.seq == seq)?;
        self.in_flight = None;
        let stale = std::mem::take(&mut self.stale);
        if !ok {
            self.last_failed = Some(flying.key);
        } else if !stale {
            self.last_done = Some(flying.key);
            self.last_failed = None;
        }
        let next = self.pending.take().or(stale.then_some(flying.key))?;
        if self.last_done == Some(next) || self.last_failed == Some(next) {
            return None;
        }
        Some(self.start(next))
    }

    /// Whether the frame of request `seq` may be shown: only if newer than what is on screen.
    pub fn accept(&mut self, seq: u64) -> bool {
        if seq > self.shown_seq {
            self.shown_seq = seq;
            true
        } else {
            false
        }
    }

    /// Forget what was rendered or failed (after an edit, or to retry): the next request renders.
    /// A render already in flight predates the edit: when it returns, the same frame is asked for again.
    pub fn invalidate(&mut self) {
        self.last_done = None;
        self.last_failed = None;
        self.stale = self.in_flight.is_some();
    }

    pub fn busy(&self) -> bool {
        self.in_flight.is_some()
    }
}

// ---------------------------------------------------------------------------------------------
// Timeline geometry

/// Maps source time to x pixels of the timeline strip and back.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Axis {
    pub x0: f32,
    pub x1: f32,
    pub duration: f64,
}

impl Axis {
    /// Source time at pixel `x` (clamped to `0..=duration`).
    pub fn time_at(&self, x: f32) -> f64 {
        let w = f64::from(self.x1 - self.x0);
        if w <= 0.0 || w.is_nan() || !x.is_finite() {
            return 0.0;
        }
        (f64::from(x - self.x0) / w).clamp(0.0, 1.0) * self.duration.max(0.0)
    }

    /// Pixel of source time `t` (clamped to the strip).
    pub fn x_at(&self, t: f64) -> f32 {
        if self.duration <= 0.0 || self.duration.is_nan() || !t.is_finite() {
            return self.x0;
        }
        self.x0 + ((t / self.duration).clamp(0.0, 1.0) as f32) * (self.x1 - self.x0)
    }
}

/// Ruler ticks: source times spaced by a round step that leaves at least `min_px` between ticks.
pub fn ruler_ticks(duration: f64, width_px: f32, min_px: f32) -> Vec<f64> {
    const STEPS: [f64; 16] = [0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0, 900.0, 1800.0, 3600.0];
    if !(duration.is_finite() && duration > 0.0 && width_px > 0.0) {
        return vec![];
    }
    let per_s = f64::from(width_px) / duration;
    let step = STEPS.iter().copied().find(|s| s * per_s >= f64::from(min_px)).unwrap_or(STEPS[STEPS.len() - 1]);
    (0..10_000).map(|i| f64::from(i) * step).take_while(|t| *t <= duration + 1e-9).collect()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BarKind {
    /// A kept span at 1x.
    Kept,
    /// A kept span played at this factor.
    Speed(f64),
    Zoom,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bar {
    pub start: f64,
    pub end: f64,
    pub kind: BarKind,
}

/// The trim/split/speed track: one bar per timeline piece (cuts are the gaps between bars).
pub fn segment_bars(tl: &Timeline) -> Vec<Bar> {
    tl.pieces()
        .iter()
        .map(|c| Bar {
            start: c.src_start,
            end: c.src_end,
            kind: if (c.factor - 1.0).abs() < 1e-9 { BarKind::Kept } else { BarKind::Speed(c.factor) },
        })
        .collect()
}

/// The zoom track: one bar per `project.zooms` entry, clamped to the recording.
pub fn zoom_bars(p: &Project) -> Vec<Bar> {
    let dur = if p.duration.is_finite() { p.duration.max(0.0) } else { 0.0 };
    p.zooms
        .iter()
        .filter(|z| z.start.is_finite() && z.end.is_finite())
        .map(|z| Bar { start: z.start.max(0.0).min(dur), end: z.end.max(0.0).min(dur), kind: BarKind::Zoom })
        .filter(|b| b.end > b.start)
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Playhead movement

/// A time inside a cut moves to the next kept start (`dir >= 0`) or the previous kept end (`dir < 0`).
pub fn snap_kept(tl: &Timeline, t: f64, dir: i32) -> f64 {
    let pieces = tl.pieces();
    if pieces.is_empty() || !t.is_finite() || tl.is_kept(t) {
        return t;
    }
    if dir >= 0 {
        pieces.iter().find(|c| c.src_start > t).map_or_else(|| pieces[pieces.len() - 1].src_end, |c| c.src_start)
    } else {
        pieces.iter().rev().find(|c| c.src_end < t).map_or_else(|| pieces[0].src_start, |c| c.src_end)
    }
}

/// Last frame time that can be rendered: one frame before the end of the last kept span.
pub fn last_frame(tl: &Timeline, duration: f64, fps: f64) -> f64 {
    let end = tl.pieces().last().map_or(duration, |c| c.src_end);
    let frame = if fps > 0.0 { 1.0 / fps } else { 1.0 / 60.0 };
    (end - frame).max(0.0)
}

/// First kept time (Home).
pub fn first_time(tl: &Timeline) -> f64 {
    tl.pieces().first().map_or(0.0, |c| c.src_start)
}

/// Move `frames` frames on the `fps` grid from `t` (⟵/⟶), landing on a kept time.
pub fn step_frames(tl: &Timeline, t: f64, frames: i32, fps: f64, duration: f64) -> f64 {
    let fps = if fps > 0.0 { fps } else { 60.0 };
    let last = last_frame(tl, duration, fps);
    let grid = (t * fps).round() + f64::from(frames);
    let moved = (grid / fps).clamp(0.0, last.max(0.0));
    snap_kept(tl, moved, frames.signum()).min(last)
}

/// Move `secs` whole seconds (⇧⟵/⇧⟶), landing on a kept time.
pub fn step_seconds(tl: &Timeline, t: f64, secs: i32, fps: f64, duration: f64) -> f64 {
    let last = last_frame(tl, duration, fps);
    let moved = (t + f64::from(secs)).clamp(0.0, last.max(0.0));
    snap_kept(tl, moved, secs.signum()).min(last)
}

/// Playback by wall clock at 1x in OUTPUT time, so speed segments play at their factor and cuts are skipped.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayClock {
    wall0: f64,
    out0: f64,
}

impl PlayClock {
    pub fn start(now: f64, t_source: f64, tl: &Timeline) -> Self {
        Self { wall0: now, out0: tl.to_output(t_source) }
    }

    /// Source time at wall time `now` (never past `last`) and whether the end was reached.
    pub fn at(&self, now: f64, tl: &Timeline, last: f64) -> (f64, bool) {
        let out = self.out0 + (now - self.wall0).max(0.0);
        if out >= tl.output_duration() {
            return (last, true);
        }
        let t = tl.to_source(out);
        if t >= last {
            (last, true)
        } else {
            (t, false)
        }
    }
}

/// Whether a playback frame is due: at most `fps` requests per second.
pub fn frame_due(last: Option<f64>, now: f64, fps: f64) -> bool {
    match last {
        Some(l) if now >= l => now - l >= 1.0 / fps.max(1.0),
        _ => true,
    }
}

// ---------------------------------------------------------------------------------------------
// Labels and layout

/// `m:ss.cc` (`h:mm:ss.cc` from one hour); hundredths are truncated so `0:59.999` never shows `0:60.00`.
pub fn clock(t: f64) -> String {
    let t = if t.is_finite() && t > 0.0 { t } else { 0.0 };
    let cs = (t * 100.0 + 1e-6).floor() as u64;
    let (secs, cs) = (cs / 100, cs % 100);
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}.{cs:02}")
    } else {
        format!("{m}:{s:02}.{cs:02}")
    }
}

/// Preview width for an area `avail_pts` points wide: the on-screen pixel width, in steps of 32 px
/// so a resizing window does not re-render at every pixel, capped at [`PREVIEW_MAX_WIDTH`].
pub fn preview_width(avail_pts: f32, pixels_per_point: f32) -> u32 {
    if !avail_pts.is_finite() || !pixels_per_point.is_finite() {
        return 64;
    }
    let px = (avail_pts * pixels_per_point / 32.0).round().max(0.0) as u32 * 32;
    px.clamp(64, PREVIEW_MAX_WIDTH)
}

/// The largest `aspect` (w/h) rectangle that fits `avail` (w, h).
pub fn fit(avail: (f32, f32), aspect: f32) -> (f32, f32) {
    let (w, h) = avail;
    if !(aspect > 0.0 && aspect.is_finite() && w > 0.0 && h > 0.0) {
        return (0.0, 0.0);
    }
    if w / h > aspect {
        (h * aspect, h)
    } else {
        (w, w / aspect)
    }
}

// ---------------------------------------------------------------------------------------------
// Render worker

/// Renders one frame: `(project dir, source time, width, params)`.
pub type Renderer = Box<dyn FnMut(&Path, f64, u32, FrameParams) -> Result<RenderedFrame, String> + Send>;

pub enum WorkerCmd {
    Frame(Req),
    /// The edited project (T27): the frames after this one are composed and styled from it.
    Project(Box<Project>),
}

/// What the worker tells the app (wrapped in `Msg::Editor`).
pub enum EditorMsg {
    Opened {
        dir: PathBuf,
        result: Result<Box<Opened>, String>,
    },
    Frame {
        dir: PathBuf,
        seq: u64,
        result: Result<FrameOut, String>,
    },
    /// The MP4 export thread (T28): progress, the result, the destination picked in the save dialog.
    Export {
        dir: PathBuf,
        msg: crate::editor_export::ExportMsg,
    },
}

pub struct Opened {
    pub project: Project,
    /// The project had no zooms and `zoom::auto_zooms` produced these (in memory only).
    pub auto_zoomed: bool,
    pub note: Option<String>,
    /// `events.json` (None when missing or unreadable): the panels need it for `Auto zoom` and the focus picker.
    pub events: Option<Events>,
}

pub struct FrameOut {
    pub image: egui::ColorImage,
    /// Source time of the request this frame answers.
    pub t: f64,
    /// Helper-reported render time.
    pub helper_ms: f64,
    /// Render + PNG read + decode, as the app saw it.
    pub total_ms: f64,
}

/// Load the project and `events.json` (auto zoom when the project has none), then render the
/// frames asked for until the sender is dropped.
pub fn spawn_worker(
    dir: PathBuf,
    auto_zoom: Option<ZoomOpts>,
    mut renderer: Renderer,
    notify: Box<dyn Fn(EditorMsg) + Send>,
) -> Sender<WorkerCmd> {
    let (tx, rx) = channel::<WorkerCmd>();
    std::thread::spawn(move || {
        let loaded = load(&dir, auto_zoom.as_ref());
        let (project, events) = match loaded {
            Ok((project, events, opened)) => {
                notify(EditorMsg::Opened { dir: dir.clone(), result: Ok(Box::new(opened)) });
                (project, events)
            }
            Err(e) => {
                notify(EditorMsg::Opened { dir: dir.clone(), result: Err(e) });
                return;
            }
        };
        serve(&rx, &dir, &project, events.as_ref(), &mut renderer, &*notify);
    });
    tx
}

type Loaded = (Project, Option<Events>, Opened);

fn load(dir: &Path, auto_zoom: Option<&ZoomOpts>) -> Result<Loaded, String> {
    let mut project = Project::load(dir).map_err(|e| format!("This recording cannot be opened: {e}"))?;
    if !dir.join(&project.sources.screen.file).is_file() {
        return Err(format!("The recording's video ({}) is missing.", project.sources.screen.file));
    }
    if !(project.duration.is_finite() && project.duration > 0.0) {
        return Err("This recording has no duration.".into());
    }
    // a file that parses can still be a broken edit list (inverted trim, speed outside the kept
    // ranges): refuse it with the first reason instead of opening an empty timeline
    let mut problems = project.validate(|f| dir.join(f).is_file());
    problems.extend(ferrocast_core::capture::timeline::validate(&project));
    if let Some(first) = problems.first() {
        return Err(format!("This recording cannot be opened: project.json is not valid: {first}"));
    }
    let mut note = None;
    let events = match std::fs::read_to_string(dir.join(EVENTS_FILE)) {
        Ok(json) => match Events::decode(&json) {
            Ok(e) => Some(e),
            Err(e) => {
                note = Some(format!("events.json could not be read ({e}): no cursor or zoom."));
                None
            }
        },
        Err(_) => {
            note = Some("events.json is missing: no cursor or zoom.".into());
            None
        }
    };
    let mut auto_zoomed = false;
    if let (Some(opts), Some(ev)) = (auto_zoom, events.as_ref()) {
        if project.zooms.is_empty() {
            project.zooms = zoom::auto_zooms(ev, opts);
            auto_zoomed = !project.zooms.is_empty();
        }
    }
    Ok((project.clone(), events.clone(), Opened { project, auto_zoomed, note, events }))
}

/// The helper reads the style (background, padding, radius, shadow, aspect) from the `project.json`
/// it is given, and the request carries only camera/cursor/ring/blur. To preview an edit before the
/// autosave writes the real file, the worker renders from a scratch folder that holds the EDITED
/// `project.json` and a link to the take. When it cannot be made the real folder is used.
struct Preview {
    dir: PathBuf,
    scratch: bool,
}

impl Preview {
    fn new(real: &Path, project: &Project) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ferrocast-preview-{}-{n}", std::process::id()));
        let file = real.join(&project.sources.screen.file);
        let made = std::fs::create_dir_all(&dir)
            .and_then(|()| std::os::unix::fs::symlink(&file, dir.join(&project.sources.screen.file)))
            .and_then(|()| project.save(&dir).map_err(std::io::Error::other))
            .is_ok();
        if made {
            Self { dir, scratch: true }
        } else {
            let _ = std::fs::remove_dir_all(&dir);
            Self { dir: real.to_path_buf(), scratch: false }
        }
    }

    /// Show `project` in the next frames (a failed write keeps the previous look).
    fn update(&self, project: &Project) {
        if self.scratch {
            let _ = project.save(&self.dir);
        }
    }
}

impl Drop for Preview {
    fn drop(&mut self) {
        if self.scratch {
            // the folder holds a link to the take: remove_dir_all removes the link, not the take
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }
}

fn serve(
    rx: &Receiver<WorkerCmd>,
    dir: &Path,
    project: &Project,
    events: Option<&Events>,
    renderer: &mut Renderer,
    notify: &dyn Fn(EditorMsg),
) {
    let preview = Preview::new(dir, project);
    let mut project = project.clone();
    loop {
        let composer = events.map(|e| Composer::new(&project, e));
        let edited = loop {
            match rx.recv() {
                Err(_) => return,
                Ok(WorkerCmd::Project(p)) => break *p,
                Ok(WorkerCmd::Frame(req)) => {
                    let started = Instant::now();
                    let params = composer.as_ref().map_or_else(FrameParams::default, |c| c.at(req.key.t));
                    let result = renderer(&preview.dir, req.key.t, req.key.width, params).and_then(|f| {
                        let bytes = std::fs::read(&f.path).map_err(|e| format!("preview frame unreadable: {e}"))?;
                        let image = crate::services::decode_png(&bytes).ok_or("preview frame is not a PNG")?;
                        Ok(FrameOut { image, t: req.key.t, helper_ms: f.ms, total_ms: started.elapsed().as_secs_f64() * 1000.0 })
                    });
                    notify(EditorMsg::Frame { dir: dir.to_path_buf(), seq: req.seq, result });
                }
            }
        };
        preview.update(&edited);
        project = edited;
    }
}

// ---------------------------------------------------------------------------------------------
// Editor state (the UI side)

/// Frames shown per second over the recent past.
#[derive(Debug, Default)]
pub struct Rate {
    shown: u64,
    since: Option<Instant>,
    pub last_ms: f64,
    /// The helper's own share of `last_ms`.
    pub last_helper_ms: f64,
}

impl Rate {
    pub fn tick(&mut self, ms: f64, helper_ms: f64) {
        self.shown += 1;
        self.last_ms = ms;
        self.last_helper_ms = helper_ms;
        self.since.get_or_insert_with(Instant::now);
    }

    /// Frames per second since the first frame (None before two frames).
    pub fn fps(&self) -> Option<f64> {
        let s = self.since?.elapsed().as_secs_f64();
        (self.shown >= 2 && s > 0.0).then(|| (self.shown - 1) as f64 / s)
    }
}

pub struct Editor {
    pub dir: PathBuf,
    /// None while the project loads.
    pub project: Option<Project>,
    pub timeline: Option<Timeline>,
    pub error: Option<String>,
    pub note: Option<String>,
    /// Playhead, source seconds.
    pub t: f64,
    pub queue: ScrubQueue,
    pub play: Option<PlayClock>,
    pub last_play_req: Option<f64>,
    pub tex: Option<egui::TextureHandle>,
    /// Source time of the frame on screen.
    pub shown_t: Option<f64>,
    /// The project differs from `project.json` (T26 only sets it for the auto zooms; nothing is written yet).
    pub dirty: bool,
    pub worker: Option<Sender<WorkerCmd>>,
    pub rate: Rate,
    /// `events.json` for `Auto zoom` and the focus picker (set when the project opens).
    pub events: Option<Events>,
    /// Panels, history and autosave (T27).
    pub st: crate::editor_panels::Studio,
    /// The Export sheet and the export in progress (T28).
    pub export: crate::editor_export::ExportSheet,
}

impl Editor {
    pub fn new(dir: PathBuf, worker: Option<Sender<WorkerCmd>>) -> Self {
        Self {
            dir,
            project: None,
            timeline: None,
            error: None,
            note: None,
            t: 0.0,
            queue: ScrubQueue::new(),
            play: None,
            last_play_req: None,
            tex: None,
            shown_t: None,
            dirty: false,
            worker,
            rate: Rate::default(),
            events: None,
            st: crate::editor_panels::Studio::new(),
            export: crate::editor_export::ExportSheet::default(),
        }
    }

    pub fn opened(&mut self, o: Result<Opened, String>) {
        match o {
            Ok(o) => {
                self.timeline = Some(Timeline::new(&o.project));
                self.dirty = o.auto_zoomed;
                self.note = o.note;
                self.events = o.events;
                self.t = self.timeline.as_ref().map_or(0.0, first_time);
                self.project = Some(o.project);
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// Send the request to the worker (a vanished worker is reported as an error).
    pub fn send(&mut self, req: Req) {
        if self.worker.as_ref().is_none_or(|w| w.send(WorkerCmd::Frame(req)).is_err()) {
            self.error = Some("The preview renderer stopped.".into());
        }
    }

    /// Ask for the frame at the playhead (coalesced).
    pub fn want(&mut self, width: u32) {
        if self.project.is_none() || self.error.is_some() {
            return;
        }
        if let Some(req) = self.queue.request(Key { t: self.t, width }) {
            self.send(req);
        }
    }

    /// A frame (or its failure) came back. Returns the image and its source time to show, if it is
    /// newer than the one on screen.
    pub fn frame_back(&mut self, seq: u64, result: Result<FrameOut, String>) -> Option<(egui::ColorImage, f64)> {
        let ok = result.is_ok();
        if let Some(next) = self.queue.complete(seq, ok) {
            self.send(next);
        }
        match result {
            Ok(f) => {
                if !self.queue.accept(seq) {
                    return None;
                }
                self.rate.tick(f.total_ms, f.helper_ms);
                self.note = None;
                Some((f.image, f.t))
            }
            Err(e) => {
                self.note = Some(e);
                None
            }
        }
    }
}

impl Editor {
    /// Last time that can be shown for this project.
    pub fn last(&self, fps: f64) -> f64 {
        match (&self.project, &self.timeline) {
            (Some(p), Some(tl)) => last_frame(tl, p.duration, fps),
            _ => 0.0,
        }
    }

    /// Move the playhead to source time `t` (clamped, never inside a cut). While playing, the clock restarts there.
    pub fn seek(&mut self, t: f64, now: f64, fps: f64) {
        let Some(tl) = &self.timeline else { return };
        let t = snap_kept(tl, t.clamp(0.0, self.last(fps)), 1).min(self.last(fps));
        self.t = t;
        if self.play.is_some() {
            self.play = Some(PlayClock::start(now, t, tl));
        }
    }

    pub fn playing(&self) -> bool {
        self.play.is_some()
    }

    /// Space: pause, or play from the playhead (from the start when it is at the end).
    pub fn toggle_play(&mut self, now: f64, fps: f64) {
        if self.play.take().is_some() {
            return;
        }
        let Some(tl) = &self.timeline else { return };
        if self.t >= self.last(fps) - 1e-9 {
            self.t = first_time(tl);
        }
        self.play = Some(PlayClock::start(now, self.t, tl));
        self.last_play_req = None;
    }

    /// Once per UI frame: advance the playhead by the wall clock while playing, then ask for the
    /// frame at the playhead through the scrub queue (playback asks at most [`PLAY_FPS`] times a second).
    pub fn tick(&mut self, now: f64, fps: f64, width: u32) {
        let (Some(tl), Some(clock)) = (&self.timeline, self.play) else {
            self.want(width);
            return;
        };
        let (t, finished) = clock.at(now, tl, self.last(fps));
        self.t = t;
        if finished {
            self.play = None;
        }
        if finished || frame_due(self.last_play_req, now, PLAY_FPS) {
            self.last_play_req = Some(now);
            self.want(width);
        }
    }
}

/// A renderer backed by one long-lived capture helper, respawned once if it died.
pub fn helper_renderer(exe: PathBuf, log: impl Fn(&str) + Send + Clone + 'static) -> Renderer {
    use ferrocast_macos::capture::{available, helper_path, Helper, RenderError};
    let mut helper: Option<Helper> = None;
    Box::new(move |dir, t, width, params| {
        for attempt in 0..2 {
            if helper.is_none() {
                if !available(&exe) {
                    return Err("Screen capture is not available in this build.".into());
                }
                helper =
                    Some(Helper::spawn(&helper_path(&exe), log.clone()).map_err(|e| format!("The capture helper did not start: {e}"))?);
            }
            let Some(h) = helper.as_mut() else { continue };
            match h.render_frame(dir, t, width, params.clone()) {
                Ok(f) => return Ok(f),
                // a helper that exited is replaced once; a timeout already killed it and is reported as is
                Err(RenderError::Closed) if attempt == 0 => helper = None,
                Err(e) => {
                    if matches!(e, RenderError::Closed | RenderError::Timeout) {
                        helper = None;
                    }
                    return Err(e.to_string());
                }
            }
        }
        Err("The capture helper keeps closing.".into())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrocast_core::capture::project::{Speed, Trim, Zoom};

    fn k(t: f64) -> Key {
        Key { t, width: 960 }
    }

    fn project(trims: &[(f64, f64)], speeds: &[(f64, f64, f64)]) -> Project {
        let mut p = Project::new("t", "2026-01-01T00:00:00Z");
        p.duration = 10.0;
        p.trims = trims.iter().map(|&(start, end)| Trim { start, end }).collect();
        p.speeds = speeds.iter().map(|&(start, end, factor)| Speed { start, end, factor }).collect();
        p
    }

    fn zoom(start: f64, end: f64) -> Zoom {
        serde_json::from_value(serde_json::json!({"id":"z","start":start,"end":end,"mode":"auto","scale":2.0})).unwrap()
    }

    // ---- ScrubQueue

    #[test]
    fn the_first_request_goes_out_at_once() {
        let mut q = ScrubQueue::new();
        let r = q.request(k(1.0)).unwrap();
        assert_eq!((r.seq, r.key), (1, k(1.0)));
        assert!(q.busy());
    }

    #[test]
    fn latest_wins_while_a_render_is_in_flight() {
        let mut q = ScrubQueue::new();
        let first = q.request(k(1.0)).unwrap();
        // a drag floods requests: none of them goes out, only the last one is kept
        for i in 2..200 {
            assert_eq!(q.request(k(i as f64)), None, "no request flood");
        }
        let next = q.complete(first.seq, true).expect("the pending time goes out");
        assert_eq!(next.key, k(199.0));
        assert_eq!(next.seq, 2);
        assert_eq!(q.complete(next.seq, true), None, "nothing left");
        assert!(!q.busy());
    }

    #[test]
    fn a_frame_older_than_the_one_shown_is_never_shown() {
        let mut q = ScrubQueue::new();
        let a = q.request(k(1.0)).unwrap();
        q.request(k(2.0));
        let b = q.complete(a.seq, true).unwrap();
        assert!(q.accept(b.seq), "the newer frame shows");
        assert!(!q.accept(a.seq), "the stale one does not show after it");
        assert!(!q.accept(b.seq), "nor twice");
    }

    #[test]
    fn a_reply_that_is_not_in_flight_is_ignored() {
        let mut q = ScrubQueue::new();
        let a = q.request(k(1.0)).unwrap();
        q.request(k(2.0));
        assert_eq!(q.complete(a.seq + 7, true), None);
        assert!(q.busy(), "the real request is still in flight");
        assert_eq!(q.complete(a.seq, true).map(|r| r.key), Some(k(2.0)));
    }

    #[test]
    fn asking_again_for_the_frame_in_flight_or_on_screen_costs_nothing() {
        let mut q = ScrubQueue::new();
        let a = q.request(k(1.0)).unwrap();
        assert_eq!(q.request(k(1.0)), None, "same key in flight");
        assert_eq!(q.complete(a.seq, true), None);
        assert_eq!(q.request(k(1.0)), None, "same key already rendered");
        assert!(q.request(Key { t: 1.0, width: 480 }).is_some(), "another width is another frame");
    }

    #[test]
    fn moving_back_to_the_frame_in_flight_drops_the_pending_one() {
        let mut q = ScrubQueue::new();
        let a = q.request(k(1.0)).unwrap();
        q.request(k(5.0));
        q.request(k(1.0));
        assert_eq!(q.complete(a.seq, true), None);
    }

    #[test]
    fn a_failed_frame_is_not_asked_again_until_invalidated() {
        let mut q = ScrubQueue::new();
        let a = q.request(k(1.0)).unwrap();
        assert_eq!(q.complete(a.seq, false), None);
        assert_eq!(q.request(k(1.0)), None, "no retry flood on the failing key");
        assert!(q.request(k(2.0)).is_some(), "another time still works");
        q.invalidate();
        let mut q2 = ScrubQueue::new();
        let a = q2.request(k(1.0)).unwrap();
        q2.complete(a.seq, false);
        q2.invalidate();
        assert!(q2.request(k(1.0)).is_some(), "invalidate allows a retry");
    }

    #[test]
    fn non_finite_times_are_not_requested() {
        let mut q = ScrubQueue::new();
        assert_eq!(q.request(k(f64::NAN)), None);
        assert_eq!(q.request(k(f64::INFINITY)), None);
        assert!(!q.busy());
    }

    // ---- axis, ruler, bars

    #[test]
    fn axis_maps_pixels_and_time_both_ways_and_clamps() {
        let a = Axis { x0: 100.0, x1: 500.0, duration: 20.0 };
        assert_eq!(a.time_at(100.0), 0.0);
        assert_eq!(a.time_at(300.0), 10.0);
        assert_eq!(a.time_at(500.0), 20.0);
        assert_eq!(a.time_at(-50.0), 0.0);
        assert_eq!(a.time_at(900.0), 20.0);
        assert_eq!(a.x_at(5.0), 200.0);
        assert_eq!(a.x_at(99.0), 500.0, "clamped to the strip");
        assert_eq!(a.x_at(-1.0), 100.0);
        for t in [0.0, 1.25, 7.5, 20.0] {
            assert!((a.time_at(a.x_at(t)) - t).abs() < 1e-4, "{t}");
        }
        let flat = Axis { x0: 10.0, x1: 10.0, duration: 5.0 };
        assert_eq!((flat.time_at(10.0), flat.x_at(3.0)), (0.0, 10.0), "a zero-width strip does not divide by zero");
    }

    #[test]
    fn ruler_steps_are_round_and_keep_their_distance() {
        let t = ruler_ticks(10.0, 600.0, 60.0);
        assert_eq!(t, (0..=10).map(f64::from).collect::<Vec<_>>());
        let t = ruler_ticks(600.0, 600.0, 60.0);
        assert_eq!(t.first(), Some(&0.0));
        assert_eq!(t[1], 60.0, "1 px per second needs at least 60 s between ticks");
        assert!(ruler_ticks(0.0, 600.0, 60.0).is_empty());
        assert!(ruler_ticks(10.0, 0.0, 60.0).is_empty());
    }

    #[test]
    fn segment_bars_follow_trims_and_speeds() {
        let tl = Timeline::new(&project(&[(1.0, 4.0), (6.0, 9.0)], &[(2.0, 3.0, 2.0)]));
        let bars = segment_bars(&tl);
        let got: Vec<(f64, f64, BarKind)> = bars.iter().map(|b| (b.start, b.end, b.kind)).collect();
        assert_eq!(
            got,
            vec![(1.0, 2.0, BarKind::Kept), (2.0, 3.0, BarKind::Speed(2.0)), (3.0, 4.0, BarKind::Kept), (6.0, 9.0, BarKind::Kept)]
        );
        assert_eq!(segment_bars(&Timeline::new(&project(&[], &[]))).len(), 1, "no trims = the whole recording");
    }

    #[test]
    fn zoom_bars_clamp_to_the_recording_and_skip_empty_ones() {
        let mut p = project(&[], &[]);
        let mut nan = zoom(0.0, 2.0);
        nan.start = f64::NAN;
        p.zooms = vec![zoom(1.0, 3.0), zoom(9.0, 14.0), zoom(4.0, 4.0), nan];
        let bars = zoom_bars(&p);
        let got: Vec<(f64, f64)> = bars.iter().map(|b| (b.start, b.end)).collect();
        assert_eq!(got, vec![(1.0, 3.0), (9.0, 10.0)]);
        assert!(bars.iter().all(|b| b.kind == BarKind::Zoom));
    }

    // ---- playhead

    #[test]
    fn frame_steps_land_on_the_frame_grid_and_clamp() {
        let tl = Timeline::new(&project(&[], &[]));
        let fps = 60.0;
        let t = step_frames(&tl, 1.0, 1, fps, 10.0);
        assert!((t - (1.0 + 1.0 / 60.0)).abs() < 1e-9);
        let t = step_frames(&tl, 1.0, -1, fps, 10.0);
        assert!((t - (1.0 - 1.0 / 60.0)).abs() < 1e-9);
        // an off-grid playhead (after a click) snaps to the grid first
        let t = step_frames(&tl, 1.004, 1, fps, 10.0);
        assert!((t - (61.0 / 60.0)).abs() < 1e-9, "{t}");
        assert_eq!(step_frames(&tl, 0.0, -1, fps, 10.0), 0.0);
        let end = last_frame(&tl, 10.0, fps);
        assert!((end - (10.0 - 1.0 / 60.0)).abs() < 1e-9);
        assert_eq!(step_frames(&tl, end, 1, fps, 10.0), end);
    }

    #[test]
    fn second_steps_move_one_second_and_clamp() {
        let tl = Timeline::new(&project(&[], &[]));
        assert_eq!(step_seconds(&tl, 3.5, 1, 60.0, 10.0), 4.5);
        assert_eq!(step_seconds(&tl, 3.5, -1, 60.0, 10.0), 2.5);
        assert_eq!(step_seconds(&tl, 0.4, -1, 60.0, 10.0), 0.0);
        assert!((step_seconds(&tl, 9.5, 1, 60.0, 10.0) - last_frame(&tl, 10.0, 60.0)).abs() < 1e-9);
    }

    #[test]
    fn stepping_skips_cuts() {
        let tl = Timeline::new(&project(&[(1.0, 4.0), (6.0, 9.0)], &[]));
        assert_eq!(snap_kept(&tl, 5.0, 1), 6.0);
        assert_eq!(snap_kept(&tl, 5.0, -1), 4.0);
        assert_eq!(snap_kept(&tl, 2.0, 1), 2.0, "kept times stay");
        assert_eq!(snap_kept(&tl, 0.2, -1), 1.0, "before the first kept span");
        assert_eq!(snap_kept(&tl, 9.5, 1), 9.0, "after the last");
        let t = step_seconds(&tl, 3.5, 1, 60.0, 10.0);
        assert_eq!(t, 6.0, "4.5 is in the cut: forward goes to the next kept start");
        assert_eq!(step_seconds(&tl, 6.5, -1, 60.0, 10.0), 4.0, "5.5 is in the cut: back goes to the previous kept end");
        assert_eq!(first_time(&tl), 1.0);
        assert!((last_frame(&tl, 10.0, 60.0) - (9.0 - 1.0 / 60.0)).abs() < 1e-9);
    }

    #[test]
    fn the_play_clock_advances_in_output_time_through_speeds_and_cuts() {
        let tl = Timeline::new(&project(&[(1.0, 4.0), (6.0, 9.0)], &[(2.0, 4.0, 2.0)]));
        let last = last_frame(&tl, 10.0, 60.0);
        let c = PlayClock::start(100.0, 1.0, &tl);
        assert_eq!(c.at(100.0, &tl, last), (1.0, false));
        assert_eq!(c.at(100.5, &tl, last), (1.5, false), "1x");
        // output 1.0 s = source 2.0 s; then 2x: output 1.5 s = source 3.0 s
        let (t, fin) = c.at(101.5, &tl, last);
        assert!((t - 3.0).abs() < 1e-9 && !fin, "{t}");
        // the 2 s sped-up span lasts 1 s of output: source 4.0 at output 2.0, then the cut is skipped to 6.0
        let (t, _) = c.at(102.0 + 0.5, &tl, last);
        assert!((t - 6.5).abs() < 1e-9, "{t}");
        let (t, fin) = c.at(200.0, &tl, last);
        assert_eq!((t, fin), (last, true), "stops at the last frame");
        let c = PlayClock::start(10.0, 6.0, &tl);
        assert!((c.at(10.5, &tl, last).0 - 6.5).abs() < 1e-9, "starting after a cut");
    }

    #[test]
    fn playback_requests_are_limited_to_the_play_rate() {
        assert!(frame_due(None, 5.0, 24.0));
        assert!(!frame_due(Some(5.0), 5.01, 24.0));
        assert!(frame_due(Some(5.0), 5.0 + 1.0 / 24.0, 24.0));
        assert!(frame_due(Some(5.0), 4.0, 24.0), "a clock that went backwards does not stall");
    }

    // ---- labels and layout

    #[test]
    fn clock_labels() {
        assert_eq!(clock(0.0), "0:00.00");
        assert_eq!(clock(3.256), "0:03.25");
        assert_eq!(clock(59.999), "0:59.99");
        assert_eq!(clock(75.5), "1:15.50");
        assert_eq!(clock(3600.0 + 61.0 + 0.07), "1:01:01.07");
        assert_eq!(clock(-4.0), "0:00.00");
        assert_eq!(clock(f64::NAN), "0:00.00");
    }

    #[test]
    fn output_time_labels_go_through_the_timeline() {
        let tl = Timeline::new(&project(&[(1.0, 4.0), (6.0, 9.0)], &[(2.0, 4.0, 2.0)]));
        assert_eq!(clock(tl.to_output(6.5)), "0:02.50");
        assert_eq!(clock(tl.output_duration()), "0:05.00");
    }

    #[test]
    fn preview_width_follows_the_screen_size_in_steps_and_is_capped() {
        assert_eq!(preview_width(700.0, 1.0), 704);
        assert_eq!(preview_width(700.0, 2.0), 960, "capped (S15)");
        assert_eq!(preview_width(300.0, 2.0), 608);
        assert_eq!(preview_width(1.0, 1.0), 64, "never below 64 px");
        assert_eq!(preview_width(f32::NAN, 2.0), 64);
        assert_eq!(preview_width(701.0, 1.0), preview_width(703.0, 1.0), "a 2 px resize does not re-render");
    }

    #[test]
    fn fit_keeps_the_aspect_inside_the_area() {
        assert_eq!(fit((800.0, 600.0), 16.0 / 9.0), (800.0, 450.0));
        assert_eq!(fit((400.0, 600.0), 2.0), (400.0, 200.0));
        let (w, h) = fit((300.0, 300.0), 0.5);
        assert_eq!((w, h), (150.0, 300.0));
        assert_eq!(fit((300.0, 300.0), 0.0), (0.0, 0.0));
    }

    // ---- the editor state and the worker

    fn open_editor_state(rx_out: &mut Option<std::sync::mpsc::Receiver<WorkerCmd>>) -> Editor {
        let (tx, rx) = channel();
        *rx_out = Some(rx);
        let mut ed = Editor::new(PathBuf::from("/p"), Some(tx));
        let mut p = project(&[], &[]);
        p.zooms = vec![zoom(1.0, 2.0)];
        ed.opened(Ok(Opened { project: p, auto_zoomed: true, note: None, events: None }));
        ed
    }

    fn sent(rx: &Receiver<WorkerCmd>) -> Vec<Req> {
        rx.try_iter().filter_map(|c| if let WorkerCmd::Frame(r) = c { Some(r) } else { None }).collect()
    }

    #[test]
    fn opening_marks_the_auto_zoomed_project_dirty_and_starts_at_the_first_kept_time() {
        let mut rx = None;
        let ed = open_editor_state(&mut rx);
        assert!(ed.dirty, "auto zooms are in memory only: nothing is written in T26");
        assert_eq!(ed.t, 0.0);
        assert!(ed.timeline.is_some() && ed.error.is_none());
        let mut ed = Editor::new(PathBuf::from("/p"), None);
        ed.opened(Err("no such recording".into()));
        assert_eq!(ed.error.as_deref(), Some("no such recording"));
        ed.want(960);
        assert!(!ed.queue.busy(), "nothing is requested for a project that did not open");
    }

    #[test]
    fn scrubbing_sends_one_request_and_then_only_the_latest_time() {
        let mut rx = None;
        let mut ed = open_editor_state(&mut rx);
        let rx = rx.unwrap();
        for i in 1..=50 {
            ed.seek(i as f64 * 0.1, 0.0, 60.0);
            ed.tick(0.0, 60.0, 960);
        }
        let first = sent(&rx);
        assert_eq!(first.len(), 1, "no flood while the first render is in flight: {first:?}");
        assert!((first[0].key.t - 0.1).abs() < 1e-9);
        // the first frame comes back: the latest wanted time (5.0) goes out, not the 49 in between
        let out = FrameOut { image: egui::ColorImage::example(), t: 0.1, helper_ms: 1.0, total_ms: 2.0 };
        assert!(ed.frame_back(first[0].seq, Ok(out)).is_some());
        let second = sent(&rx);
        assert_eq!(second.len(), 1);
        assert!((second[0].key.t - 5.0).abs() < 1e-9, "{second:?}");
        // a late reply of the old request does not replace the newer frame
        let late = FrameOut { image: egui::ColorImage::example(), t: 0.1, helper_ms: 1.0, total_ms: 2.0 };
        assert!(ed.frame_back(first[0].seq, Ok(late)).is_none());
        let new = FrameOut { image: egui::ColorImage::example(), t: 5.0, helper_ms: 1.0, total_ms: 2.0 };
        assert_eq!(ed.frame_back(second[0].seq, Ok(new)).map(|(_, t)| t), Some(5.0));
    }

    #[test]
    fn a_render_failure_keeps_the_helpers_message_and_does_not_flood() {
        let mut rx = None;
        let mut ed = open_editor_state(&mut rx);
        let rx = rx.unwrap();
        ed.tick(0.0, 60.0, 960);
        let r = sent(&rx).pop().unwrap();
        assert!(ed.frame_back(r.seq, Err("screen.mov is missing".into())).is_none());
        assert_eq!(ed.note.as_deref(), Some("screen.mov is missing"));
        for _ in 0..5 {
            ed.tick(0.0, 60.0, 960);
        }
        assert!(sent(&rx).is_empty(), "the failing frame is not asked again every UI frame");
        ed.seek(1.0, 0.0, 60.0);
        ed.tick(0.0, 60.0, 960);
        assert_eq!(sent(&rx).len(), 1, "another time is still tried");
    }

    #[test]
    fn playback_advances_by_the_clock_and_asks_at_the_play_rate() {
        let mut rx = None;
        let mut ed = open_editor_state(&mut rx);
        let rx = rx.unwrap();
        ed.toggle_play(10.0, 60.0);
        assert!(ed.playing());
        ed.tick(10.0, 60.0, 960);
        let r = sent(&rx).pop().unwrap();
        ed.frame_back(r.seq, Ok(FrameOut { image: egui::ColorImage::example(), t: r.key.t, helper_ms: 1.0, total_ms: 2.0 }));
        ed.tick(10.01, 60.0, 960);
        assert!(sent(&rx).is_empty(), "10 ms later is not a frame yet");
        ed.tick(10.5, 60.0, 960);
        let r = sent(&rx).pop().unwrap();
        assert!((r.key.t - 0.5).abs() < 1e-9, "1x by wall clock: {r:?}");
        ed.toggle_play(10.6, 60.0);
        assert!(!ed.playing());
        // playing past the end stops at the last frame
        ed.seek(5.0, 11.0, 60.0);
        ed.toggle_play(11.0, 60.0);
        ed.tick(99.0, 60.0, 960);
        assert!(!ed.playing());
        assert_eq!(ed.t, ed.last(60.0));
        // Space at the end restarts from the beginning
        ed.toggle_play(100.0, 60.0);
        assert!(ed.playing() && ed.t == 0.0);
    }

    #[test]
    fn seeking_never_lands_in_a_cut_or_past_the_end() {
        let mut ed = Editor::new(PathBuf::from("/p"), None);
        ed.opened(Ok(Opened { project: project(&[(1.0, 4.0), (6.0, 9.0)], &[]), auto_zoomed: false, note: None, events: None }));
        assert_eq!(ed.t, 1.0, "starts at the first kept time");
        ed.seek(5.0, 0.0, 60.0);
        assert_eq!(ed.t, 6.0);
        ed.seek(99.0, 0.0, 60.0);
        assert!((ed.t - (9.0 - 1.0 / 60.0)).abs() < 1e-9);
        ed.seek(-3.0, 0.0, 60.0);
        assert_eq!(ed.t, 1.0);
    }

    fn fixture_dir(tag: &str, project_json: &str) -> PathBuf {
        let fx = Path::new(env!("CARGO_MANIFEST_DIR")).join("../core/fixtures");
        let d = std::env::temp_dir().join(format!("fc-editor-{}-{tag}", std::process::id())).join("Take.ferrocast");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::copy(fx.join(project_json), d.join("project.json")).unwrap();
        std::fs::copy(fx.join("render_events.json"), d.join("events.json")).unwrap();
        std::fs::write(d.join("screen.mov"), b"not a movie: the fake renderer never opens it").unwrap();
        d
    }

    type Calls = std::sync::Arc<std::sync::Mutex<Vec<(f64, u32, FrameParams)>>>;

    fn fake_renderer(dir: &Path) -> (Renderer, Calls) {
        let calls: Calls = Default::default();
        let log = calls.clone();
        let png = dir.join("fake.png");
        image::RgbaImage::from_pixel(8, 4, image::Rgba([10, 20, 30, 255])).save(&png).unwrap();
        let r: Renderer = Box::new(move |_, t, w, params| {
            log.lock().unwrap().push((t, w, params));
            Ok(RenderedFrame { path: png.clone(), size: [8, 4], ms: 1.5, cached: false })
        });
        (r, calls)
    }

    fn run_worker(dir: &Path, auto: Option<ZoomOpts>, asks: &[f64]) -> (Opened, Vec<FrameOut>, Calls) {
        let (r, calls) = fake_renderer(dir);
        let (mtx, mrx) = channel();
        let mtx = std::sync::Mutex::new(mtx);
        let tx = spawn_worker(dir.to_path_buf(), auto, r, Box::new(move |m| mtx.lock().unwrap().send(m).unwrap()));
        let opened = match mrx.recv_timeout(std::time::Duration::from_secs(10)).unwrap() {
            EditorMsg::Opened { result, .. } => *result.unwrap(),
            _ => panic!("the first message is Opened"),
        };
        let mut frames = vec![];
        for (i, t) in asks.iter().enumerate() {
            tx.send(WorkerCmd::Frame(Req { seq: i as u64 + 1, key: Key { t: *t, width: 960 } })).unwrap();
            match mrx.recv_timeout(std::time::Duration::from_secs(10)).unwrap() {
                EditorMsg::Frame { seq, result, .. } => {
                    assert_eq!(seq, i as u64 + 1);
                    frames.push(result.unwrap());
                }
                _ => panic!("a frame answers a request"),
            }
        }
        (opened, frames, calls)
    }

    #[test]
    fn the_worker_applies_auto_zoom_in_memory_once_and_leaves_project_json_alone() {
        let d = fixture_dir("auto", "render_project.json");
        let before = std::fs::read(d.join("project.json")).unwrap();
        let events = Events::decode(&std::fs::read_to_string(d.join("events.json")).unwrap()).unwrap();
        let expected = zoom::auto_zooms(&events, &ZoomOpts::default());
        assert!(!expected.is_empty(), "the fixture's clicks produce a zoom");
        let (o, _, _) = run_worker(&d, Some(ZoomOpts::default()), &[]);
        assert!(o.auto_zoomed);
        assert_eq!(o.project.zooms, expected);
        assert_eq!(std::fs::read(d.join("project.json")).unwrap(), before, "T26 writes nothing; autosave is T27");
        let (o, _, _) = run_worker(&d, None, &[]);
        assert!(!o.auto_zoomed && o.project.zooms.is_empty(), "auto zoom off");
    }

    #[test]
    fn a_project_that_already_has_zooms_keeps_them_and_frames_carry_the_composers_params() {
        let d = fixture_dir("zoomed", "render_zoom_project.json");
        let (o, frames, calls) = run_worker(&d, Some(ZoomOpts::default()), &[0.1, 1.0]);
        assert!(!o.auto_zoomed);
        assert_eq!(o.project.zooms.len(), 1, "existing zooms are not replaced");
        assert_eq!((frames[0].image.size, frames[0].t), ([8, 4], 0.1));
        assert!(frames[0].helper_ms == 1.5);
        let events = Events::decode(&std::fs::read_to_string(d.join("events.json")).unwrap()).unwrap();
        let comp = Composer::new(&o.project, &events);
        let calls = calls.lock().unwrap();
        assert_eq!(calls[1].2, comp.at(1.0), "frame params come from Composer::at(t)");
        assert!(calls[1].2.camera.is_some(), "t = 1.0 is inside the zoom");
        assert!(calls[0].2.camera.is_none(), "t = 0.1 is before it");
        assert_eq!(calls[1].1, 960);
    }

    #[test]
    fn a_missing_or_broken_project_reports_a_message_and_stops() {
        let d = std::env::temp_dir().join(format!("fc-editor-{}-missing", std::process::id())).join("None.ferrocast");
        let (r, _) = fake_renderer(&std::env::temp_dir());
        let (mtx, mrx) = channel();
        let mtx = std::sync::Mutex::new(mtx);
        let _tx = spawn_worker(d, None, r, Box::new(move |m| mtx.lock().unwrap().send(m).unwrap()));
        match mrx.recv_timeout(std::time::Duration::from_secs(10)).unwrap() {
            EditorMsg::Opened { result: Err(e), .. } => assert!(e.contains("cannot be opened"), "{e}"),
            _ => panic!("expected an error"),
        }
        // no screen.mov: the message names it
        let d = fixture_dir("novideo", "render_project.json");
        std::fs::remove_file(d.join("screen.mov")).unwrap();
        let (r, _) = fake_renderer(&std::env::temp_dir());
        let (mtx, mrx) = channel();
        let mtx = std::sync::Mutex::new(mtx);
        let _tx = spawn_worker(d, None, r, Box::new(move |m| mtx.lock().unwrap().send(m).unwrap()));
        match mrx.recv_timeout(std::time::Duration::from_secs(10)).unwrap() {
            EditorMsg::Opened { result: Err(e), .. } => assert!(e.contains("screen.mov") && e.contains("missing"), "{e}"),
            _ => panic!("expected an error"),
        }
    }

    /// Found live: a hand-edited or damaged `project.json` that still parses (trim with start > end)
    /// opened an empty 0:00 editor instead of an error.
    #[test]
    fn a_project_that_parses_but_is_not_a_valid_edit_list_is_refused_with_the_reason() {
        let d = fixture_dir("invalid", "render_project.json");
        let mut p = Project::load(&d).unwrap();
        p.trims = vec![ferrocast_core::capture::project::Trim { start: 5.0, end: 2.0 }];
        p.save(&d).unwrap();
        let err = load(&d, None).err().expect("an inverted trim is refused");
        assert!(err.contains("project.json is not valid") && err.contains("trims[0]"), "{err}");
        // the untouched fixture still opens
        let ok = fixture_dir("valid", "render_project.json");
        assert!(load(&ok, None).is_ok());
    }

    #[test]
    fn a_renderer_error_reaches_the_app_as_the_frame_result() {
        let d = fixture_dir("fail", "render_project.json");
        let r: Renderer = Box::new(|_, _, _, _| Err("take frame timed out".into()));
        let (mtx, mrx) = channel();
        let mtx = std::sync::Mutex::new(mtx);
        let tx = spawn_worker(d, None, r, Box::new(move |m| mtx.lock().unwrap().send(m).unwrap()));
        let _opened = mrx.recv_timeout(std::time::Duration::from_secs(10)).unwrap();
        tx.send(WorkerCmd::Frame(Req { seq: 1, key: Key { t: 0.0, width: 960 } })).unwrap();
        match mrx.recv_timeout(std::time::Duration::from_secs(10)).unwrap() {
            EditorMsg::Frame { result: Err(e), .. } => assert_eq!(e, "take frame timed out"),
            _ => panic!("expected the failure"),
        }
    }

    #[test]
    fn edited_projects_render_from_a_scratch_folder_and_the_real_project_json_is_untouched() {
        let d = fixture_dir("scratch", "render_project.json");
        let before = std::fs::read(d.join("project.json")).unwrap();
        let seen: std::sync::Arc<std::sync::Mutex<Vec<(PathBuf, String, bool)>>> = Default::default();
        let log = seen.clone();
        let png = d.join("fake.png");
        image::RgbaImage::from_pixel(8, 4, image::Rgba([1, 2, 3, 255])).save(&png).unwrap();
        let r: Renderer = Box::new(move |dir, _, _, _| {
            let json = std::fs::read_to_string(dir.join("project.json")).unwrap_or_default();
            log.lock().unwrap().push((dir.to_path_buf(), json, dir.join("screen.mov").is_file()));
            Ok(RenderedFrame { path: png.clone(), size: [8, 4], ms: 1.0, cached: false })
        });
        let (mtx, mrx) = channel();
        let mtx = std::sync::Mutex::new(mtx);
        let tx = spawn_worker(d.clone(), None, r, Box::new(move |m| mtx.lock().unwrap().send(m).unwrap()));
        let EditorMsg::Opened { result, .. } = mrx.recv_timeout(std::time::Duration::from_secs(10)).unwrap() else {
            panic!("Opened first")
        };
        let mut edited = result.unwrap().project;
        let ask = |seq: u64| {
            tx.send(WorkerCmd::Frame(Req { seq, key: Key { t: 0.1, width: 960 } })).unwrap();
            let EditorMsg::Frame { result, .. } = mrx.recv_timeout(std::time::Duration::from_secs(10)).unwrap() else { panic!("a frame") };
            result.unwrap();
        };
        ask(1);
        edited.style.padding = 0.31;
        tx.send(WorkerCmd::Project(Box::new(edited))).unwrap();
        ask(2);
        let seen = seen.lock().unwrap().clone();
        assert_eq!(seen.len(), 2);
        assert_ne!(seen[0].0, d, "frames come from the scratch folder, not the project folder");
        assert_eq!(seen[0].0, seen[1].0);
        assert!(seen[0].2 && seen[1].2, "the take is reachable from the scratch folder");
        assert!(!seen[0].1.contains("0.31") && seen[1].1.contains("0.31"), "the second frame is styled from the edited project");
        assert_eq!(std::fs::read(d.join("project.json")).unwrap(), before, "the worker never writes the real project.json");
        drop(tx);
        let gone = (0..100).any(|_| {
            std::thread::sleep(std::time::Duration::from_millis(50));
            !seen[0].0.exists()
        });
        assert!(gone, "the scratch folder is removed when the editor closes");
        assert!(d.join("screen.mov").is_file(), "removing it does not touch the take");
    }

    /// Live check against the REAL helper on the fixture project (no Screen Recording needed: it
    /// renders an existing movie). Skipped when the helper or the fixture is missing; build them with
    /// `swiftc ... -o target/capture-spike/ferrocast-capture` and `scripts/capture-smoke.sh <helper> --keep`.
    /// Run: `cargo test -p ferrocast --locked live_helper -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs target/capture-spike/ferrocast-capture and the editor fixture (scripts/capture-smoke.sh --keep)"]
    fn live_helper_renders_the_fixture_and_plays_at_the_target_rate() {
        use std::time::{Duration, Instant};
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/capture-spike");
        // FERROCAST_LIVE_TAKE=<project folder> measures a real recording instead (playback is
        // then cut after 6 s of wall time and the take length is not asserted)
        let real = std::env::var_os("FERROCAST_LIVE_TAKE").map(std::path::PathBuf::from);
        let dir = real.clone().unwrap_or_else(|| root.join("editor-fixture/Take.ferrocast"));
        if !root.join("ferrocast-capture").is_file() || !dir.join("screen.mov").is_file() {
            eprintln!("SKIP: target/capture-spike/ferrocast-capture or the editor fixture is missing");
            return;
        }
        let (mtx, mrx) = channel();
        let mtx = std::sync::Mutex::new(mtx);
        let tx = spawn_worker(
            dir.clone(),
            Some(ZoomOpts::default()),
            // `helper_path` replaces the file name: any name next to the helper works
            helper_renderer(root.join("ferrocast"), |l| eprintln!("helper: {l}")),
            Box::new(move |m| {
                let _ = mtx.lock().unwrap().send(m);
            }),
        );
        let wait = || mrx.recv_timeout(Duration::from_secs(20)).expect("the worker answers");
        let opened = match wait() {
            EditorMsg::Opened { result, .. } => *result.expect("the fixture opens"),
            _ => panic!("Opened comes first"),
        };
        assert!(opened.auto_zoomed && !opened.project.zooms.is_empty(), "auto zoom applied at open");
        let zoom_t = opened.project.zooms[0].start + 0.5 * (opened.project.zooms[0].end - opened.project.zooms[0].start);
        let mut ed = Editor::new(dir.clone(), Some(tx.clone()));
        ed.opened(Ok(opened));
        // one frame through the whole path: a PNG back from the helper, decoded
        let ask = |ed: &mut Editor, t: f64| -> FrameOut {
            ed.seek(t, 0.0, 60.0);
            ed.want(960);
            loop {
                if let EditorMsg::Frame { seq, result, .. } = wait() {
                    let out = result.expect("frame renders");
                    let copy = FrameOut { image: out.image.clone(), ..out };
                    ed.frame_back(seq, Ok(out));
                    return copy;
                }
            }
        };
        let cold = ask(&mut ed, 0.0);
        assert_eq!(cold.image.size, [960, 540], "960 px wide, 16:9");
        assert!(cold.image.pixels.windows(2).any(|w| w[0] != w[1]), "not a flat image");
        let zoomed = ask(&mut ed, zoom_t);
        assert_ne!(cold.image.pixels, zoomed.image.pixels, "the zoom camera changes the frame");
        eprintln!(
            "first frame: helper {:.0} ms, with decode {:.0} ms; zoomed frame: helper {:.0} ms, with decode {:.0} ms",
            cold.helper_ms, cold.total_ms, zoomed.helper_ms, zoomed.total_ms
        );
        // throughput of the renderer alone: 60 distinct frames back to back (nothing cached)
        let t0 = Instant::now();
        let mut helper_ms = 0.0;
        for i in 0..60u64 {
            tx.send(WorkerCmd::Frame(Req { seq: 1000 + i, key: Key { t: 0.01 + i as f64 * 0.047, width: 960 } })).unwrap();
            loop {
                if let EditorMsg::Frame { result, .. } = wait() {
                    helper_ms += result.expect("frame renders").helper_ms;
                    break;
                }
            }
        }
        let secs = t0.elapsed().as_secs_f64();
        eprintln!(
            "renderer alone, 60 frames at 960 px: {:.1} fps (helper mean {:.1} ms/frame, wall {:.1} ms/frame incl. PNG read+decode)",
            60.0 / secs,
            helper_ms / 60.0,
            secs * 1000.0 / 60.0
        );
        // playback through the real pipeline: wall clock, ScrubQueue, frame skipping, 24 fps request cap
        ed.seek(0.0, 0.0, 60.0);
        let t0 = Instant::now();
        ed.rate = Rate::default();
        ed.toggle_play(0.0, 60.0);
        let mut shown = 0u32;
        loop {
            let now = t0.elapsed().as_secs_f64();
            ed.tick(now, 60.0, 960);
            while let Ok(m) = mrx.try_recv() {
                if let EditorMsg::Frame { seq, result, .. } = m {
                    if ed.frame_back(seq, result).is_some() {
                        shown += 1;
                    }
                }
            }
            if !ed.playing() && !ed.queue.busy() {
                break;
            }
            if real.is_some() && now > 6.0 {
                break;
            }
            assert!(now < 15.0, "playback of a 3 s take must end");
            std::thread::sleep(Duration::from_millis(2));
        }
        let secs = t0.elapsed().as_secs_f64();
        eprintln!(
            "playback of the 3 s fixture: {shown} frames shown in {secs:.2} s = {:.1} fps (request cap {PLAY_FPS} fps)",
            f64::from(shown) / secs
        );
        assert!(shown >= 10, "playback shows frames: {shown}");
        assert!(real.is_some() || (secs > 2.5 && secs < 4.5), "plays in real time, not faster: {secs}");
    }
}
