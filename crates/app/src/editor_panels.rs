//! Editor panels (T27): the pure mapping from panel values to EDL edits, the undo/redo + autosave
//! wiring of the editor state, and the right-hand panel drawing (Zoom / Cursor / Style / Clips).
//!
//! Every change of the project goes through [`Editor::apply`]: the edit runs on a copy, is checked
//! with `timeline::validate` + `Project::validate` (an edit that adds a problem is rejected and its
//! message shown inline), then recorded in `timeline::History` so Undo/Redo cover every EDL field.
//! The worker gets the edited project for the next frame; `project.json` is written by the
//! [`AutosaveClock`] one idle second after the last user change.

use crate::editor_ctl::{Editor, WorkerCmd};
use crate::ui::Palette;
use egui::{Rect, Ui, UiBuilder};
use ferrocast_core::capture::camera::{CameraLayout, CameraShape, Corner, MARGIN_MAX, SIZE_MAX, SIZE_MIN};
use ferrocast_core::capture::project::{AudioTrack, Project, Style, TrackMix, Trim, Zoom, GAIN_DB_MAX, GAIN_DB_MIN};
use ferrocast_core::capture::timeline::{self, EditKind, History, Timeline};
use ferrocast_core::capture::transcript::{self, Options as TranscriptOptions, Transcript, Word};
use ferrocast_core::capture::zoom::{self, ZoomOpts, MAX_SCALE};
use ferrocast_core::capture::MIN_SEGMENT;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Undo steps kept.
pub const UNDO_DEPTH: usize = 200;
/// Edits of one kind closer than this (seconds) are one undo step (slider drags).
pub const COALESCE_WINDOW: f64 = 1.0;
/// `project.json` is written this long after the last change.
pub const AUTOSAVE_IDLE: f64 = 1.0;
/// Shortest zoom the panel creates or leaves behind.
pub const MIN_ZOOM: f64 = 0.2;
/// Length of a zoom added at the playhead.
pub const NEW_ZOOM_LEN: f64 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Zoom,
    Cursor,
    Style,
    Clips,
    /// Only for projects with a camera source (P4).
    Camera,
    /// Only for projects with a microphone or system audio source (P4).
    Audio,
    /// Only for projects with transcribable audio or an existing transcript (P5).
    Transcript,
}

impl Tab {
    pub fn label(self) -> &'static str {
        match self {
            Tab::Zoom => "Zoom",
            Tab::Cursor => "Cursor",
            Tab::Style => "Style",
            Tab::Clips => "Clips",
            Tab::Camera => "Camera",
            Tab::Audio => "Audio",
            Tab::Transcript => "Transcript",
        }
    }

    /// `self` when the project offers it, else the first tab (a stored Camera tab of another project).
    pub fn or_first(self, p: &Project) -> Tab {
        if visible_tabs(p).contains(&self) {
            self
        } else {
            Tab::Zoom
        }
    }
}

/// The tabs of the panel: Camera only with a camera source, Audio only with a microphone or system source,
/// Transcript with transcribable audio (microphone or system) or a transcript already in the project.
pub fn visible_tabs(p: &Project) -> Vec<Tab> {
    let mut v = vec![Tab::Zoom, Tab::Cursor, Tab::Style, Tab::Clips];
    if p.sources.camera.is_some() {
        v.push(Tab::Camera);
    }
    if p.sources.mic.is_some() || p.sources.system.is_some() {
        v.push(Tab::Audio);
    }
    if can_transcribe(p) || p.transcript.is_some() {
        v.push(Tab::Transcript);
    }
    v
}

// ---------------------------------------------------------------------------------------------
// Transcript (P5): pure helpers. The words, the filler/pause detection and the cuts live in
// `ferrocast_core::capture::transcript`; this part only maps them to what the tab shows and does.

/// Languages offered by the picker: `(BCP-47 locale, name)`. The first entry is the system language
/// (the helper's default); which of the others work depends on the on-device models of this Mac.
pub const LOCALES: &[(&str, &str)] = &[
    ("", "System language"),
    ("en-US", "English (US)"),
    ("en-GB", "English (UK)"),
    ("es-ES", "Spanish (Spain)"),
    ("es-MX", "Spanish (Mexico)"),
    ("fr-FR", "French"),
    ("de-DE", "German"),
    ("it-IT", "Italian"),
    ("pt-BR", "Portuguese (Brazil)"),
    ("ja-JP", "Japanese"),
    ("zh-CN", "Chinese (Mandarin)"),
];

/// The locale to send to the helper for picker entry `i` (`None` = the system language).
pub fn locale_arg(i: usize) -> Option<&'static str> {
    LOCALES.get(i).map(|l| l.0).filter(|l| !l.is_empty())
}

/// The audio file to transcribe: the microphone, else the system audio.
pub fn audio_for_transcription(p: &Project) -> Option<&str> {
    p.sources.mic.as_ref().or(p.sources.system.as_ref()).map(|s| s.file.as_str())
}

pub fn can_transcribe(p: &Project) -> bool {
    audio_for_transcription(p).is_some()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptView {
    /// No (usable) transcript: the Transcribe controls.
    Idle,
    /// Words to read and cut.
    Words,
}

pub fn transcript_view(p: &Project) -> TranscriptView {
    if p.transcript.as_ref().is_some_and(|t| !t.words.is_empty()) {
        TranscriptView::Words
    } else {
        TranscriptView::Idle
    }
}

/// Why a transcription produced no transcript, as the tab shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranscribeFailure {
    /// Speech Recognition is not allowed: the tab offers to open the Settings pane.
    Permission,
    Message(String),
}

impl TranscribeFailure {
    pub fn text(&self) -> String {
        match self {
            Self::Permission => {
                "Speech Recognition permission is needed. Open System Settings > Privacy & Security > Speech Recognition and allow Ferrocast."
                    .into()
            }
            Self::Message(m) => m.clone(),
        }
    }
}

/// A helper error as a failure message (a cancel is not a failure: the worker reports it apart).
pub fn map_transcribe_error(e: &ferrocast_macos::capture::TranscribeError) -> TranscribeFailure {
    use ferrocast_macos::capture::TranscribeError;
    match e {
        TranscribeError::PermissionSpeech(_) => TranscribeFailure::Permission,
        TranscribeError::Unavailable(m) => TranscribeFailure::Message(m.clone()),
        other => TranscribeFailure::Message(other.to_string()),
    }
}

/// Index of the word being spoken at source time `t` (`start <= t < end`; gaps belong to no word).
pub fn word_at(t: &Transcript, time: f64) -> Option<usize> {
    if !time.is_finite() {
        return None;
    }
    t.words.iter().position(|w| time >= w.start && time < w.end)
}

/// The kept time closest to source time `t` (itself when kept; on a tie the later one), where the
/// playhead may land when a word that was cut is clicked.
pub fn nearest_kept(tl: &Timeline, t: f64) -> f64 {
    let pieces = tl.pieces();
    if pieces.is_empty() || !t.is_finite() || tl.is_kept(t) {
        return t;
    }
    let prev = pieces.iter().rev().find(|c| c.src_end < t).map(|c| c.src_end);
    let next = pieces.iter().find(|c| c.src_start > t).map(|c| c.src_start);
    match (prev, next) {
        (Some(a), Some(b)) => {
            if t - a < b - t {
                a
            } else {
                b
            }
        }
        (Some(a), None) => a,
        (None, Some(b)) => b,
        (None, None) => t,
    }
}

/// A word is cut when the middle of it is no longer in the timeline.
pub fn word_is_cut(tl: &Timeline, w: &Word) -> bool {
    !tl.is_kept((w.start + w.end) / 2.0)
}

/// The selected words as `(first, last)`, ordered and clamped to `n` words; `None` when empty.
pub fn selection_bounds(sel: Option<(usize, usize)>, n: usize) -> Option<(usize, usize)> {
    let (a, b) = sel?;
    let (lo, hi) = (a.min(b), a.max(b));
    (lo < n).then(|| (lo, hi.min(n - 1)))
}

/// The single source-time cut that removes exactly the selected words (core `word_range_cuts`).
pub fn selection_cut(t: &Transcript, sel: Option<(usize, usize)>) -> Option<(f64, f64)> {
    let (a, b) = selection_bounds(sel, t.words.len())?;
    transcript::word_range_cuts(t, a, b)
}

/// Which words `Remove fillers` would take out: those inside one of its cuts.
pub fn filler_flags(t: &Transcript, duration: f64) -> Vec<bool> {
    let cuts = transcript::remove_fillers(t, &t.language, &TranscriptOptions { duration: Some(duration), ..Default::default() });
    t.words.iter().map(|w| cuts.iter().any(|&(a, b)| (w.start + w.end) / 2.0 >= a && (w.start + w.end) / 2.0 <= b)).collect()
}

/// The three cutting actions of the tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutAction {
    Fillers,
    Pauses,
    Selection,
}

impl CutAction {
    fn nothing(self) -> &'static str {
        match self {
            Self::Fillers => "No filler words found.",
            Self::Pauses => "No long pauses to tighten.",
            Self::Selection => "That selection is already cut.",
        }
    }
}

/// `Removed 3 cuts · 1.4 s`.
pub fn cut_report(cuts: usize, secs: f64) -> String {
    format!("Removed {cuts} cut{} · {secs:.1} s", if cuts == 1 { "" } else { "s" })
}

/// Progress and cancel flag of the transcription in flight. Dropping it (the editor session ends)
/// cancels the work.
pub struct TranscribeRun {
    pub cancel: Arc<AtomicBool>,
    pub done: f64,
    pub total: f64,
    pub cancelling: bool,
}

impl TranscribeRun {
    pub fn new() -> Self {
        Self { cancel: Arc::new(AtomicBool::new(false)), done: 0.0, total: 0.0, cancelling: false }
    }
}

impl Default for TranscribeRun {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TranscribeRun {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// What the transcription thread tells the app (inside `EditorMsg::Transcribe`).
#[derive(Debug)]
pub enum TranscribeMsg {
    Progress {
        done: f64,
        total: f64,
    },
    /// The helper wrote `transcript.json`; the thread read it back.
    Done(Box<Transcript>),
    Failed(TranscribeFailure),
    Cancelled,
    /// The system permission prompt was answered (or dismissed): the app may start the helper, which
    /// reports `permission-speech` itself when Speech Recognition is not allowed.
    PermissionAsked,
}

/// State of the Transcript tab.
#[derive(Default)]
pub struct TranscriptUi {
    /// Index into [`LOCALES`].
    pub locale: usize,
    pub run: Option<TranscribeRun>,
    pub error: Option<TranscribeFailure>,
    /// Selected words, `(anchor, cursor)` in either order.
    pub sel: Option<(usize, usize)>,
    /// Where a shift-click or drag selection starts.
    pub anchor: Option<usize>,
    /// What the last cut action did (or why it did nothing).
    pub notice: Option<String>,
    /// The Transcribe button was pressed: the app starts the worker (it owns the helper path and the bus).
    pub start: bool,
    /// The system permission prompt was already raised in this session (it is raised at most once).
    pub asked: bool,
}

/// `transcript.json` of a project folder; a missing or broken file is no transcript.
pub fn adopt_transcript(dir: &Path) -> Option<Transcript> {
    Transcript::from_json(&std::fs::read_to_string(dir.join("transcript.json")).ok()?).ok()
}

/// The helper call, on the worker thread: blocks until the transcript is written, then reads it.
fn run_transcribe(
    helper: &Path,
    log: impl Fn(&str) + Send + 'static,
    dir: &Path,
    audio: &str,
    locale: Option<&str>,
    cancel: &AtomicBool,
    on_progress: impl FnMut(f64, f64),
) -> TranscribeMsg {
    use ferrocast_macos::capture::{Helper, TranscribeError};
    let mut helper = match Helper::spawn(helper, log) {
        Ok(h) => h,
        Err(e) => return TranscribeMsg::Failed(TranscribeFailure::Message(format!("Could not start the transcriber: {e}"))),
    };
    match helper.transcribe(dir, Some(audio), locale, cancel, on_progress) {
        Ok(_) => match std::fs::read_to_string(dir.join("transcript.json")) {
            Ok(json) => match Transcript::from_json(&json) {
                Ok(t) => TranscribeMsg::Done(Box::new(t)),
                Err(e) => TranscribeMsg::Failed(TranscribeFailure::Message(e)),
            },
            Err(e) => TranscribeMsg::Failed(TranscribeFailure::Message(format!("The transcript could not be read: {e}"))),
        },
        Err(TranscribeError::Cancelled) => TranscribeMsg::Cancelled,
        Err(e) => TranscribeMsg::Failed(map_transcribe_error(&e)),
    }
}

// ---------------------------------------------------------------------------------------------
// Camera and audio edits (P4)

pub enum CameraField {
    Visible(bool),
    Shape(CameraShape),
    Corner(Corner),
    /// Fraction of the output width, clamped to `0.10..=0.40`.
    Size(f64),
    /// Fraction of the output width, clamped to `0..=0.25`.
    Margin(f64),
    Mirror(bool),
}

pub fn set_camera(p: &mut Project, field: CameraField) -> Result<(), String> {
    if p.sources.camera.is_none() {
        return Err("This recording has no camera.".into());
    }
    let c = p.camera.get_or_insert_with(CameraLayout::default);
    match field {
        CameraField::Visible(v) => c.visible = v,
        CameraField::Shape(s) => c.shape = s,
        CameraField::Corner(k) => c.corner = k,
        CameraField::Size(v) => c.size = finite(v)?.clamp(SIZE_MIN, SIZE_MAX),
        CameraField::Margin(v) => c.margin = finite(v)?.clamp(0.0, MARGIN_MAX),
        CameraField::Mirror(v) => c.mirror = v,
    }
    Ok(())
}

pub enum AudioField {
    /// dB, clamped to `-60..=12`.
    Gain(AudioTrack, f64),
    Mute(AudioTrack, bool),
    Master(f64),
}

fn track_name(t: AudioTrack) -> &'static str {
    match t {
        AudioTrack::Mic => "microphone",
        AudioTrack::System => "system audio",
    }
}

fn has_track(p: &Project, t: AudioTrack) -> bool {
    match t {
        AudioTrack::Mic => p.sources.mic.is_some(),
        AudioTrack::System => p.sources.system.is_some(),
    }
}

/// The mix of a track as the panel shows it: the stored one, else the legacy linear gain in dB.
pub fn track_mix(p: &Project, t: AudioTrack) -> TrackMix {
    let (stored, legacy) = match t {
        AudioTrack::Mic => (p.audio.mic, p.audio.mic_gain),
        AudioTrack::System => (p.audio.system, p.audio.system_gain),
    };
    stored.unwrap_or(TrackMix {
        gain_db: if legacy > 0.0 { (20.0 * legacy.log10()).clamp(GAIN_DB_MIN, GAIN_DB_MAX) } else { GAIN_DB_MIN },
        mute: false,
    })
}

pub fn set_audio(p: &mut Project, field: AudioField) -> Result<(), String> {
    let need = |p: &Project, t: AudioTrack| {
        if has_track(p, t) {
            Ok(())
        } else {
            Err(format!("This recording has no {}.", track_name(t)))
        }
    };
    match field {
        AudioField::Master(v) => p.audio.master_db = finite(v)?.clamp(GAIN_DB_MIN, GAIN_DB_MAX),
        AudioField::Gain(t, v) => {
            need(p, t)?;
            let mut m = track_mix(p, t);
            m.gain_db = finite(v)?.clamp(GAIN_DB_MIN, GAIN_DB_MAX);
            store_mix(p, t, m);
        }
        AudioField::Mute(t, on) => {
            need(p, t)?;
            let mut m = track_mix(p, t);
            m.mute = on;
            store_mix(p, t, m);
        }
    }
    Ok(())
}

fn store_mix(p: &mut Project, t: AudioTrack, m: TrackMix) {
    match t {
        AudioTrack::Mic => p.audio.mic = Some(m),
        AudioTrack::System => p.audio.system = Some(m),
    }
}

/// One recorded side file for the read-only list of the Camera / Audio tabs.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceInfo {
    pub label: &'static str,
    pub file: String,
    pub offset: f64,
    /// Seconds of the take (the side files cover the same recording).
    pub duration: f64,
    pub bytes: Option<u64>,
    pub missing: bool,
}

impl SourceInfo {
    /// `camera.mov · 1:05 · 12 MB · +0.10 s`, or the missing-file sentence.
    pub fn text(&self) -> String {
        if self.missing {
            return format!("{} is missing from the project folder. It is left out of the preview and the export.", self.file);
        }
        let size = self.bytes.map(|b| format!(" · {}", crate::record_ctl::format_size(b))).unwrap_or_default();
        format!("{} · {}{size} · {:+.2} s", self.file, crate::record_ctl::format_duration(self.duration), self.offset)
    }
}

/// The camera / microphone / system files of the project and whether each is on disk (disk).
pub fn source_infos(p: &Project, dir: &std::path::Path) -> Vec<SourceInfo> {
    [("Camera", &p.sources.camera), ("Microphone", &p.sources.mic), ("System audio", &p.sources.system)]
        .into_iter()
        .filter_map(|(label, s)| {
            let s = s.as_ref()?;
            let bytes = std::fs::metadata(dir.join(&s.file)).ok().filter(|m| m.is_file()).map(|m| m.len());
            Some(SourceInfo { label, file: s.file.clone(), offset: s.offset, duration: p.duration, bytes, missing: bytes.is_none() })
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Autosave clock (pure)

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SaveStatus {
    Saved,
    Saving,
    Failed(String),
}

/// Debounce of `project.json` writes on a clock the caller supplies (seconds).
#[derive(Debug, Clone)]
pub struct AutosaveClock {
    idle: f64,
    dirty: bool,
    /// Last user change, or last failed attempt (the retry waits one more idle period).
    mark: f64,
    error: Option<String>,
}

impl AutosaveClock {
    pub fn new(idle: f64) -> Self {
        Self { idle, dirty: false, mark: 0.0, error: None }
    }

    /// The user changed something at `now`.
    pub fn changed(&mut self, now: f64) {
        self.dirty = true;
        self.mark = now;
        self.error = None;
    }

    /// A write is owed (changes not on disk).
    pub fn pending(&self) -> bool {
        self.dirty
    }

    /// Time to write: dirty and quiet for the idle period.
    pub fn due(&self, now: f64) -> bool {
        self.dirty && now - self.mark >= self.idle
    }

    /// Seconds until a write is due (None when nothing is owed).
    pub fn wait(&self, now: f64) -> Option<f64> {
        self.dirty.then(|| (self.mark + self.idle - now).max(0.0))
    }

    /// The write succeeded.
    pub fn written(&mut self) {
        self.dirty = false;
        self.error = None;
    }

    /// The write failed: still dirty, retried after another idle period, the text is kept for the footer.
    pub fn failed(&mut self, now: f64, message: String) {
        self.dirty = true;
        self.mark = now;
        self.error = Some(message);
    }

    pub fn status(&self) -> SaveStatus {
        match (&self.error, self.dirty) {
            (Some(e), _) => SaveStatus::Failed(e.clone()),
            (None, true) => SaveStatus::Saving,
            (None, false) => SaveStatus::Saved,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Edits: panel value -> project change (clamped; validation is the caller's)

pub enum ZoomField {
    Start(f64),
    End(f64),
    Scale(f64),
    /// `None` follows the cursor.
    Focus(Option<[f64; 2]>),
    EaseIn(f64),
    EaseOut(f64),
}

fn finite(v: f64) -> Result<f64, String> {
    if v.is_finite() {
        Ok(v)
    } else {
        Err("That is not a number.".into())
    }
}

/// A manual zoom at `at`, [`NEW_ZOOM_LEN`] long, cut short by the next zoom or the end.
pub fn new_zoom(p: &Project, at: f64) -> Result<Zoom, String> {
    let at = finite(at)?.clamp(0.0, p.duration.max(0.0));
    if p.zooms.iter().any(|z| z.start <= at && at < z.end) {
        return Err("There is already a zoom at the playhead.".into());
    }
    let next = p.zooms.iter().filter(|z| z.start > at).map(|z| z.start).fold(f64::INFINITY, f64::min);
    let end = (at + NEW_ZOOM_LEN).min(p.duration).min(next);
    if end - at < MIN_ZOOM {
        return Err("There is no room for a zoom at the playhead.".into());
    }
    let id = (1..).map(|n| format!("m{n}")).find(|id| p.zooms.iter().all(|z| z.id != *id)).unwrap_or_default();
    let zoom = Zoom { id, start: at, end, mode: "manual".into(), scale: 2.0, focus: Some([0.5, 0.5]), ease_in: 0.6, ease_out: 0.7 };
    Ok(zoom)
}

pub fn add_zoom(p: &mut Project, at: f64) -> Result<(), String> {
    let z = new_zoom(p, at)?;
    p.zooms.push(z);
    p.zooms.sort_by(|a, b| a.start.total_cmp(&b.start));
    Ok(())
}

/// Edit one field of zoom `id` (clamped). Editing makes the zoom manual, so `Auto zoom` keeps it.
pub fn set_zoom(p: &mut Project, id: &str, field: ZoomField) -> Result<(), String> {
    let dur = p.duration.max(0.0);
    let z = p.zooms.iter_mut().find(|z| z.id == id).ok_or("That zoom no longer exists.")?;
    let before = z.clone();
    match field {
        ZoomField::Start(v) => z.start = finite(v)?.clamp(0.0, (z.end - MIN_ZOOM).max(0.0)),
        ZoomField::End(v) => z.end = finite(v)?.clamp(z.start + MIN_ZOOM, dur.max(z.start + MIN_ZOOM)),
        ZoomField::Scale(v) => z.scale = finite(v)?.clamp(1.0, MAX_SCALE),
        ZoomField::Focus(f) => {
            z.focus = match f {
                Some([x, y]) => Some([finite(x)?.clamp(0.0, 1.0), finite(y)?.clamp(0.0, 1.0)]),
                None => None,
            }
        }
        ZoomField::EaseIn(v) => z.ease_in = finite(v)?.clamp(0.0, 3.0),
        ZoomField::EaseOut(v) => z.ease_out = finite(v)?.clamp(0.0, 3.0),
    }
    if *z != before {
        z.mode = "manual".into();
    }
    Ok(())
}

pub fn delete_zoom(p: &mut Project, id: &str) -> Result<(), String> {
    let n = p.zooms.len();
    p.zooms.retain(|z| z.id != id);
    if p.zooms.len() == n {
        return Err("That zoom no longer exists.".into());
    }
    Ok(())
}

/// `Auto zoom`: the automatic zooms are replaced by `fresh`; manual ones stay and win overlaps.
pub fn replace_auto(p: &mut Project, fresh: Vec<Zoom>) {
    let mut zooms: Vec<Zoom> = p.zooms.iter().filter(|z| z.mode != "auto").cloned().collect();
    let manual = zooms.clone();
    zooms.extend(fresh.into_iter().filter(|a| manual.iter().all(|m| !(a.start < m.end && m.start < a.end))));
    zooms.sort_by(|a, b| a.start.total_cmp(&b.start));
    p.zooms = zooms;
}

/// Cursor styles `compose` understands: the sprite, or no cursor (`none` and `hidden` are the same).
pub const CURSOR_STYLES: [(&str, &str); 3] = [("arrow", "Default"), ("none", "None"), ("hidden", "Hidden")];

pub enum CursorField {
    Style(String),
    Size(f64),
    Smoothing(f64),
    ClickRing(bool),
    MotionBlur(f64),
}

pub fn set_cursor(p: &mut Project, field: CursorField) -> Result<(), String> {
    let c = &mut p.cursor;
    match field {
        CursorField::Style(s) => {
            if !CURSOR_STYLES.iter().any(|(id, _)| *id == s) {
                return Err(format!("`{s}` is not a cursor style."));
            }
            c.style = s;
        }
        CursorField::Size(v) => c.size = finite(v)?.clamp(0.5, 4.0),
        CursorField::Smoothing(v) => c.smoothing = finite(v)?.clamp(0.0, 1.0),
        CursorField::ClickRing(on) => c.click_effect = if on { "ring" } else { "none" }.into(),
        CursorField::MotionBlur(v) => c.motion_blur = finite(v)?.clamp(0.0, 2.0),
    }
    Ok(())
}

/// Output aspects `render.frame` knows (anything else follows the recording).
pub const ASPECTS: [(&str, &str); 5] = [("16:9", "16:9"), ("4:3", "4:3"), ("1:1", "1:1"), ("9:16", "9:16"), ("source", "Source")];

/// Bundled wallpapers (the same gradients `Render.swift` generates). `Project::validate` only accepts
/// `#rrggbb` stops, so a wallpaper is stored as the gradient it is rather than as kind `wallpaper`.
pub const WALLPAPERS: [(&str, &[&str], f64); 8] = [
    ("Aurora", &["#0f2027", "#2c5364", "#42e695"], 135.0),
    ("Dusk", &["#2b1055", "#d53369", "#ff9a5a"], 160.0),
    ("Ocean", &["#0f4c81", "#1fa2ff", "#a6ffcb"], 120.0),
    ("Graphite", &["#141414", "#3a3a3c"], 135.0),
    ("Sand", &["#f5e6c8", "#d9a066"], 150.0),
    ("Lilac", &["#e0c3fc", "#8ec5fc"], 135.0),
    ("Forest", &["#0b3d2e", "#2e8b57", "#b6e388"], 145.0),
    ("Rose", &["#ffdde1", "#ee9ca7"], 135.0),
];

pub enum StyleField {
    Aspect(String),
    Padding(f64),
    Radius(f64),
    ShadowBlur(f64),
    ShadowOpacity(f64),
    ShadowDy(f64),
    /// `color`, `gradient` or `blur`.
    BgKind(String),
    /// Stop `i` as `#rrggbb`.
    BgStop(usize, String),
    BgAngle(f64),
    Wallpaper(usize),
}

fn hex_ok(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn set_style(p: &mut Project, field: StyleField) -> Result<(), String> {
    let s = &mut p.style;
    match field {
        StyleField::Aspect(a) => {
            if !ASPECTS.iter().any(|(id, _)| *id == a) {
                return Err(format!("`{a}` is not an aspect ratio."));
            }
            s.aspect = a;
        }
        StyleField::Padding(v) => s.padding = finite(v)?.clamp(0.0, 0.45),
        StyleField::Radius(v) => s.radius = finite(v)?.clamp(0.0, 64.0),
        StyleField::ShadowBlur(v) => s.shadow.blur = finite(v)?.clamp(0.0, 120.0),
        StyleField::ShadowOpacity(v) => s.shadow.opacity = finite(v)?.clamp(0.0, 1.0),
        StyleField::ShadowDy(v) => s.shadow.dy = finite(v)?.clamp(0.0, 80.0),
        StyleField::BgKind(k) => {
            if !matches!(k.as_str(), "color" | "gradient" | "blur") {
                return Err(format!("`{k}` is not a background kind."));
            }
            let stops = &mut s.background.stops;
            let want = if k == "gradient" { 2 } else { 1 };
            let defaults = ["#1f1c2c", "#928dab"];
            while stops.len() < want {
                stops.push(defaults[stops.len().min(1)].into());
            }
            s.background.kind = k;
        }
        StyleField::BgStop(i, hex) => {
            if !hex_ok(&hex) {
                return Err(format!("`{hex}` is not a #rrggbb colour."));
            }
            let slot = s.background.stops.get_mut(i).ok_or("That colour does not exist.")?;
            *slot = hex.to_ascii_lowercase();
        }
        StyleField::BgAngle(v) => s.background.angle = finite(v)?.clamp(0.0, 360.0),
        StyleField::Wallpaper(i) => {
            let (_, stops, angle) = WALLPAPERS.get(i).ok_or("That wallpaper does not exist.")?;
            s.background.kind = "gradient".into();
            s.background.stops = stops.iter().map(|c| (*c).to_string()).collect();
            s.background.angle = *angle;
        }
    }
    Ok(())
}

// ---- Clips

/// The kept range (a trim, or the whole recording) holding source time `t`.
pub fn segment_at(p: &Project, t: f64) -> Option<(f64, f64)> {
    let ranges = kept(p);
    ranges.iter().copied().find(|&(s, e)| t >= s && t < e).or_else(|| ranges.iter().copied().find(|&(_, e)| t == e))
}

fn kept(p: &Project) -> Vec<(f64, f64)> {
    if p.trims.is_empty() {
        return if p.duration > 0.0 { vec![(0.0, p.duration)] } else { vec![] };
    }
    let mut r: Vec<(f64, f64)> = p.trims.iter().map(|t| (t.start, t.end)).collect();
    r.sort_by(|a, b| a.0.total_cmp(&b.0));
    r
}

/// Trim in: cut everything before the playhead.
pub fn trim_in(p: &mut Project, t: f64) -> Result<(), String> {
    if timeline::remove_range(p, 0.0, finite(t)?) {
        Ok(())
    } else {
        Err("There is nothing to trim before the playhead.".into())
    }
}

/// Trim out: cut everything after the playhead.
pub fn trim_out(p: &mut Project, t: f64) -> Result<(), String> {
    let end = p.duration;
    if timeline::remove_range(p, finite(t)?, end) {
        Ok(())
    } else {
        Err("There is nothing to trim after the playhead.".into())
    }
}

/// Split the segment under the playhead in two (each at least `MIN_SEGMENT` long).
pub fn split_at(p: &mut Project, t: f64) -> Result<(), String> {
    let t = finite(t)?;
    let ranges = kept(p);
    let Some(i) = ranges.iter().position(|&(s, e)| t - s >= MIN_SEGMENT && e - t >= MIN_SEGMENT) else {
        return Err("Move the playhead inside a segment, away from its ends, to split it.".into());
    };
    let (s, e) = ranges[i];
    let mut next: Vec<Trim> = ranges.iter().map(|&(start, end)| Trim { start, end }).collect();
    next.splice(i..=i, [Trim { start: s, end: t }, Trim { start: t, end: e }]);
    p.trims = next;
    Ok(())
}

/// Delete the segment under the playhead.
pub fn delete_segment(p: &mut Project, t: f64) -> Result<(), String> {
    let (s, e) = segment_at(p, finite(t)?).ok_or("The playhead is not inside a segment.")?;
    if timeline::remove_range(p, s, e) {
        Ok(())
    } else {
        Err("The only segment cannot be deleted.".into())
    }
}

/// Play the segment under the playhead at `factor` (`SPEED_MIN..=SPEED_MAX`; 1 clears it).
pub fn set_segment_speed(p: &mut Project, t: f64, factor: f64) -> Result<(), String> {
    let factor = finite(factor)?;
    if !(ferrocast_core::videoplan::SPEED_MIN..=timeline::SPEED_MAX).contains(&factor) {
        return Err(format!("Speed must be between {}× and {}×.", ferrocast_core::videoplan::SPEED_MIN, timeline::SPEED_MAX));
    }
    let (s, e) = segment_at(p, finite(t)?).ok_or("The playhead is not inside a segment.")?;
    timeline::set_speed(p, s, e, factor);
    Ok(())
}

// ---- Focus picking

fn aspect_ratio(spec: &str, source: f64) -> f64 {
    let parts: Vec<f64> = spec.split([':', '/']).filter_map(|x| x.parse().ok()).collect();
    match parts[..] {
        [w, h] if w > 0.0 && h > 0.0 => w / h,
        _ => source,
    }
}

/// A click inside the preview image (`0..=1` both axes) -> the zoom focus in the normalised video, or
/// `None` when the click is on the padding. `video_aspect` is width/height of the recording;
/// `camera` is the `[x, y, w, h]` crop the frame on screen shows (a zoomed frame).
pub fn preview_to_focus(click: [f64; 2], style: &Style, video_aspect: f64, camera: Option<[f64; 4]>) -> Option<[f64; 2]> {
    let src = if video_aspect.is_finite() && video_aspect > 0.0 { video_aspect } else { 16.0 / 9.0 };
    let (w, h) = (1.0, 1.0 / aspect_ratio(&style.aspect, src));
    let pad = style.padding.clamp(0.0, 0.45) * w;
    let s = ((w - 2.0 * pad) / src).min((h - 2.0 * pad) / 1.0);
    let (fw, fh) = (src * s, s);
    let (fx, fy) = ((w - fw) / 2.0, (h - fh) / 2.0);
    let (u, v) = ((click[0] * w - fx) / fw, (click[1] * h - fy) / fh);
    if !(0.0..=1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
        return None;
    }
    let [cx, cy, cw, ch] = camera.unwrap_or([0.0, 0.0, 1.0, 1.0]);
    Some([(cx + u * cw).clamp(0.0, 1.0), (cy + v * ch).clamp(0.0, 1.0)])
}

// ---------------------------------------------------------------------------------------------
// Editor state: history, autosave

/// What the panels add to the editor state.
pub struct Studio {
    pub history: History,
    pub autosave: AutosaveClock,
    pub tab: Tab,
    /// Inline message of the last rejected edit.
    pub edit_error: Option<String>,
    pub selected_zoom: Option<String>,
    /// The next click on the preview sets the focus of the selected zoom.
    pub picking: bool,
    /// Speed factor offered by the Clips tab.
    pub speed: f64,
    /// The Transcript tab (P5).
    pub tr: TranscriptUi,
    started: Instant,
}

impl Studio {
    pub fn new() -> Self {
        Self {
            history: History::new(UNDO_DEPTH, COALESCE_WINDOW),
            autosave: AutosaveClock::new(AUTOSAVE_IDLE),
            tab: Tab::default(),
            edit_error: None,
            selected_zoom: None,
            picking: false,
            speed: 2.0,
            tr: TranscriptUi::default(),
            started: Instant::now(),
        }
    }
}

impl Default for Studio {
    fn default() -> Self {
        Self::new()
    }
}

/// Problems of `p` as the editor sees them: both validators.
fn problems(p: &Project) -> Vec<String> {
    let mut v = timeline::validate(p);
    for m in p.validate(|_| true) {
        if !v.contains(&m) {
            v.push(m);
        }
    }
    v
}

impl Editor {
    /// Seconds on the editor's own wall clock (the autosave debounce and undo coalescing use it).
    pub fn now(&self) -> f64 {
        self.st.started.elapsed().as_secs_f64()
    }

    /// Run an edit: on a copy, validated, then recorded in the history. Returns whether the project
    /// changed. A rejected edit leaves the project alone and sets `st.edit_error`.
    pub fn apply(&mut self, kind: EditKind, now: f64, f: impl FnOnce(&mut Project) -> Result<(), String>) -> bool {
        let Some(project) = self.project.as_mut() else { return false };
        let mut cand = project.clone();
        if let Err(e) = f(&mut cand) {
            self.st.edit_error = Some(e);
            return false;
        }
        if cand == *project {
            self.st.edit_error = None;
            return false;
        }
        let before = problems(project);
        let new: Vec<String> = problems(&cand).into_iter().filter(|m| !before.contains(m)).collect();
        if !new.is_empty() {
            self.st.edit_error = Some(new.join(" "));
            return false;
        }
        self.st.history.edit(project, kind, now, |p| *p = cand);
        self.st.edit_error = None;
        self.after_change(now);
        true
    }

    /// Start transcribing on a worker thread (never the caller's): `helper` is the capture helper
    /// executable, `notify` delivers the [`TranscribeMsg`]s to the app.
    pub fn start_transcribe(
        &mut self,
        helper: PathBuf,
        log: impl Fn(&str) + Send + 'static,
        notify: Box<dyn Fn(crate::editor_ctl::EditorMsg) + Send>,
    ) -> Result<(), String> {
        if self.st.tr.run.is_some() {
            return Err("A transcription is already running.".into());
        }
        let Some(p) = self.project.as_ref() else { return Err("The project is not loaded yet.".into()) };
        let audio = audio_for_transcription(p).ok_or("This recording has no audio to transcribe.")?.to_owned();
        let locale = locale_arg(self.st.tr.locale).map(str::to_owned);
        let run = TranscribeRun::new();
        let cancel = run.cancel.clone();
        self.st.tr.run = Some(run);
        self.st.tr.error = None;
        self.st.tr.notice = None;
        let dir = self.dir.clone();
        std::thread::spawn(move || {
            let send = |msg| notify(crate::editor_ctl::EditorMsg::Transcribe { dir: dir.clone(), msg });
            let msg = run_transcribe(&helper, log, &dir, &audio, locale.as_deref(), &cancel, |done, total| {
                send(TranscribeMsg::Progress { done, total })
            });
            send(msg);
        });
        Ok(())
    }

    /// Cancel: the helper stops and leaves no `transcript.json`.
    pub fn cancel_transcribe(&mut self) {
        if let Some(r) = self.st.tr.run.as_mut() {
            r.cancel.store(true, Ordering::Relaxed);
            r.cancelling = true;
        }
    }

    /// A message of the transcription thread. The transcript goes into the project through
    /// [`Editor::apply`] (one undo step, saved by the autosave).
    pub fn transcribe_msg(&mut self, msg: TranscribeMsg, now: f64) {
        match msg {
            TranscribeMsg::Progress { done, total } => {
                if let Some(r) = self.st.tr.run.as_mut() {
                    (r.done, r.total) = (done, total);
                }
            }
            TranscribeMsg::Done(t) => {
                self.st.tr.run = None;
                self.st.tr.error = None;
                self.st.tr.sel = None;
                self.st.tr.anchor = None;
                let t = *t;
                self.apply(EditKind::Other, now, move |p| {
                    p.transcript = Some(t);
                    Ok(())
                });
            }
            TranscribeMsg::Failed(f) => {
                self.st.tr.run = None;
                self.st.tr.error = Some(f);
            }
            TranscribeMsg::Cancelled => {
                self.st.tr.run = None;
                self.st.tr.error = None;
            }
            TranscribeMsg::PermissionAsked => {
                // the placeholder run kept the tab on "Starting…" while the prompt was up
                let cancelled = self.st.tr.run.take().is_some_and(|r| r.cancelling);
                self.st.tr.start = !cancelled;
            }
        }
    }

    /// Remove fillers / Tighten pauses / Delete selection: ONE undoable edit through [`Editor::apply`],
    /// with a report (or the reason nothing happened) in `st.tr.notice`.
    pub fn transcript_cut(&mut self, action: CutAction, now: f64) {
        let Some(p) = self.project.as_ref() else { return };
        let Some(t) = p.transcript.as_ref() else { return };
        let opts = TranscriptOptions { duration: Some(p.duration), ..Default::default() };
        let cuts: Vec<(f64, f64)> = match action {
            CutAction::Fillers => transcript::remove_fillers(t, &t.language, &opts),
            CutAction::Pauses => transcript::tighten_pauses(t, &opts),
            CutAction::Selection => selection_cut(t, self.st.tr.sel).into_iter().collect(),
        };
        self.st.tr.notice = None;
        if cuts.is_empty() {
            self.st.tr.notice = Some(action.nothing().into());
            return;
        }
        let before = Timeline::new(p).output_duration();
        let n = cuts.len();
        let changed = self.apply(EditKind::Other, now, |p| {
            *p = transcript::apply_cuts(p, &cuts)?;
            Ok(())
        });
        if changed {
            let after = self.timeline.as_ref().map_or(before, Timeline::output_duration);
            self.st.tr.notice = Some(cut_report(n, (before - after).max(0.0)));
            if action == CutAction::Selection {
                self.st.tr.sel = None;
                self.st.tr.anchor = None;
            }
        } else if self.st.edit_error.is_none() {
            self.st.tr.notice = Some(action.nothing().into());
        }
    }

    /// Width / height of the recorded video (16:9 when `events.json` is missing).
    pub fn video_aspect(&self) -> f64 {
        use ferrocast_core::capture::events::CaptureSpace;
        let Some(e) = self.events.as_ref() else { return 16.0 / 9.0 };
        let [w, h] = CaptureSpace::new(&e.screen).size();
        if w > 0.0 && h > 0.0 {
            w / h
        } else {
            16.0 / 9.0
        }
    }

    /// ⌘Z.
    pub fn undo(&mut self, now: f64) -> bool {
        let done = self.project.as_mut().is_some_and(|p| self.st.history.undo(p));
        if done {
            self.after_change(now);
        }
        done
    }

    /// ⇧⌘Z.
    pub fn redo(&mut self, now: f64) -> bool {
        let done = self.project.as_mut().is_some_and(|p| self.st.history.redo(p));
        if done {
            self.after_change(now);
        }
        done
    }

    /// The project changed: rebuild the timeline, keep the playhead on a kept time, give the worker
    /// the new project, make the next frame render again and start the autosave countdown.
    fn after_change(&mut self, now: f64) {
        let Some(p) = self.project.as_ref() else { return };
        let tl = Timeline::new(p);
        self.t = crate::editor_ctl::snap_kept(&tl, self.t.clamp(0.0, p.duration.max(0.0)), 1);
        self.timeline = Some(tl);
        self.play = None;
        self.st.edit_error = None;
        let cmd = WorkerCmd::Project(Box::new(p.clone()));
        if self.worker.as_ref().is_none_or(|w| w.send(cmd).is_err()) {
            self.error = Some("The preview renderer stopped.".into());
        }
        self.queue.invalidate();
        self.st.autosave.changed(now);
        self.dirty = true;
    }

    /// Write `project.json` (atomically) now; a failure keeps the project dirty and is shown in the footer.
    fn save_now(&mut self, now: f64) {
        let Some(p) = self.project.as_ref() else { return };
        match p.save(&self.dir) {
            Ok(()) => {
                self.st.autosave.written();
                self.dirty = false;
            }
            Err(e) => self.st.autosave.failed(now, format!("Could not save project.json: {e}")),
        }
    }

    /// Once per UI frame: write when the last change is one idle second old.
    pub fn autosave_tick(&mut self, now: f64) {
        if self.st.autosave.due(now) {
            self.save_now(now);
        }
    }

    /// Esc, closing, hiding, switching view: write what is owed right now.
    pub fn flush(&mut self, now: f64) {
        if self.st.autosave.pending() {
            self.save_now(now);
        }
    }

    /// `Saved`, `Saving…` or the error text.
    pub fn save_label(&self) -> String {
        match self.st.autosave.status() {
            SaveStatus::Saved => "Saved".into(),
            SaveStatus::Saving => "Saving…".into(),
            SaveStatus::Failed(e) => e,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Panels (drawing)

/// Width of the right-hand panel.
pub const PANEL_W: f32 = 252.0;

pub fn hex_to_rgb(hex: &str) -> Option<[u8; 3]> {
    if !hex_ok(hex) {
        return None;
    }
    let v = u32::from_str_radix(&hex[1..], 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

pub fn rgb_to_hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

/// `Auto 0:02.00–0:04.00 · 2.0×`.
pub fn zoom_label(z: &Zoom) -> String {
    use crate::editor_ctl::clock;
    let kind = if z.mode == "auto" { "Auto" } else { "Manual" };
    format!("{kind}  {}–{}  ·  {:.1}×", clock(z.start), clock(z.end), z.scale)
}

/// A labelled slider: the label in a fixed column at the left, the rail across the middle and the
/// value box (always the same width) at the right. Returns the new value (in the unit of `v`) when
/// it changed. `scale` shows the value multiplied (0.06 as 6 %).
fn row_slider(
    ui: &mut Ui,
    pal: &Palette,
    label: &str,
    v: f64,
    range: std::ops::RangeInclusive<f64>,
    scale: f64,
    suffix: &str,
) -> Option<f64> {
    let mut shown = v * scale;
    let (lo, hi) = (range.start() * scale, range.end() * scale);
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(egui::vec2(SLIDER_LABEL_W, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.set_min_width(SLIDER_LABEL_W);
            ui.label(egui::RichText::new(label).size(12.5).color(pal.sub_strong));
        });
        let gap = ui.spacing().item_spacing.x;
        let width = (ui.available_width() - SLIDER_VALUE_W - gap - 2.0).max(60.0);
        let sp = ui.spacing_mut();
        sp.interact_size.x = SLIDER_VALUE_W;
        sp.slider_rail_height = 4.0;
        sp.slider_width = width;
        ui.visuals_mut().slider_trailing_fill = true;
        ui.visuals_mut().handle_shape = egui::style::HandleShape::Circle;
        let r = ui.add(egui::Slider::new(&mut shown, lo..=hi).suffix(suffix).max_decimals(2));
        r.changed().then(|| shown / scale)
    })
    .inner
}

/// Label column and value box of [`row_slider`].
const SLIDER_LABEL_W: f32 = 56.0;
const SLIDER_VALUE_W: f32 = 58.0;

fn note(ui: &mut Ui, pal: &Palette, text: &str) {
    ui.label(egui::RichText::new(text).size(11.5).color(pal.sub));
}

/// The right-hand panel: tabs and the active tab's controls. Every change goes through [`Editor::apply`].
pub fn draw_panel(ui: &mut Ui, rect: Rect, pal: &Palette, ed: &mut Editor, auto: &ZoomOpts, fps: f64, now: f64) {
    ui.painter().rect_filled(rect, 8.0, pal.field);
    // tests check that every tab stays inside the panel
    #[cfg(test)]
    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("editor-panel-rect"), rect));
    ui.scope_builder(UiBuilder::new().max_rect(rect.shrink(8.0)), |ui| {
        let tabs = ed.project.as_ref().map_or_else(|| visible_tabs(&Project::new("", "")), visible_tabs);
        // a Camera / Audio tab left over from another project falls back to Zoom
        if let Some(p) = ed.project.as_ref() {
            let tab = ed.st.tab.or_first(p);
            if tab != ed.st.tab {
                ed.st.tab = tab;
                ed.st.picking = false;
            }
        }
        if let Some(t) = tab_strip(ui, pal, &tabs, ed.st.tab) {
            ed.st.tab = t;
            ed.st.picking = false;
        }
        ui.separator();
        let Some(proj) = ed.project.clone() else { return };
        // the reason an edit was refused stays in view above the controls
        if let Some(e) = ed.st.edit_error.clone() {
            ui.label(egui::RichText::new(e).size(12.0).color(pal.danger));
        }
        let tab = ed.st.tab;
        egui::ScrollArea::vertical().id_salt("editor-panel").auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            match tab {
                Tab::Zoom => zoom_tab(ui, pal, ed, &proj, auto, now),
                Tab::Cursor => cursor_tab(ui, pal, ed, &proj, now),
                Tab::Style => style_tab(ui, pal, ed, &proj, now),
                Tab::Clips => clips_tab(ui, pal, ed, &proj, fps, now),
                Tab::Camera => camera_tab(ui, pal, ed, &proj, now),
                Tab::Audio => audio_tab(ui, pal, ed, &proj, now),
                Tab::Transcript => transcript_tab(ui, pal, ed, &proj, fps, now),
            }
        });
    });
}

fn zoom_tab(ui: &mut Ui, pal: &Palette, ed: &mut Editor, proj: &Project, auto: &ZoomOpts, now: f64) {
    ui.horizontal(|ui| {
        if ui.button("Add at playhead").clicked() {
            let at = ed.t;
            if ed.apply(EditKind::Other, now, |p| add_zoom(p, at)) {
                ed.st.selected_zoom = ed.project.as_ref().and_then(|p| p.zooms.iter().find(|z| z.start == at)).map(|z| z.id.clone());
            }
        }
        if ui.button("Auto zoom").on_hover_text("Replace the automatic zooms; manual ones stay").clicked() {
            auto_zoom(ed, auto, now);
        }
    });
    if proj.zooms.is_empty() {
        note(ui, pal, "No zooms. Add one at the playhead or press Auto zoom.");
    }
    for z in &proj.zooms {
        let open = ed.st.selected_zoom.as_deref() == Some(z.id.as_str());
        if ui.selectable_label(open, zoom_label(z)).clicked() {
            ed.st.selected_zoom = (!open).then(|| z.id.clone());
            ed.st.picking = false;
        }
        if open {
            zoom_fields(ui, pal, ed, z, proj.duration, now);
        }
    }
}

/// `Auto zoom`: run the generator again with the settings and replace the automatic zooms.
fn auto_zoom(ed: &mut Editor, auto: &ZoomOpts, now: f64) {
    let Some(events) = ed.events.as_ref() else {
        ed.st.edit_error = Some("This recording has no cursor events, so there is nothing to zoom on.".into());
        return;
    };
    let fresh = zoom::auto_zooms(events, auto);
    if !ed.apply(EditKind::Other, now, |p| {
        replace_auto(p, fresh);
        Ok(())
    }) && ed.st.edit_error.is_none()
    {
        ed.st.edit_error = Some("Auto zoom found nothing new.".into());
    }
}

fn drag(ui: &mut Ui, label: &str, v: f64, range: std::ops::RangeInclusive<f64>, step: f64, suffix: &str) -> Option<f64> {
    let mut x = v;
    ui.horizontal(|ui| {
        ui.add_sized([70.0, 18.0], egui::Label::new(label));
        let r = ui.add(egui::DragValue::new(&mut x).range(range).speed(step).suffix(suffix).max_decimals(2));
        r.changed().then_some(x)
    })
    .inner
}

fn zoom_fields(ui: &mut Ui, pal: &Palette, ed: &mut Editor, z: &Zoom, duration: f64, now: f64) {
    let id = z.id.clone();
    let edit = |ed: &mut Editor, kind: EditKind, field: ZoomField| {
        let id = id.clone();
        ed.apply(kind, now, move |p| set_zoom(p, &id, field));
    };
    if let Some(v) = drag(ui, "Start", z.start, 0.0..=duration, 0.05, " s") {
        edit(ed, EditKind::Zoom, ZoomField::Start(v));
    }
    if let Some(v) = drag(ui, "End", z.end, 0.0..=duration, 0.05, " s") {
        edit(ed, EditKind::Zoom, ZoomField::End(v));
    }
    if let Some(v) = drag(ui, "Scale", z.scale, 1.0..=MAX_SCALE, 0.02, "×") {
        edit(ed, EditKind::Zoom, ZoomField::Scale(v));
    }
    let [fx, fy] = z.focus.unwrap_or([0.5, 0.5]);
    ui.horizontal(|ui| {
        ui.add_sized([70.0, 18.0], egui::Label::new("Focus"));
        let (mut x, mut y) = (fx * 100.0, fy * 100.0);
        let a = ui.add(egui::DragValue::new(&mut x).range(0.0..=100.0).suffix(" %").speed(0.5)).changed();
        let b = ui.add(egui::DragValue::new(&mut y).range(0.0..=100.0).suffix(" %").speed(0.5)).changed();
        if a || b {
            edit(ed, EditKind::Zoom, ZoomField::Focus(Some([x / 100.0, y / 100.0])));
        }
    });
    let follow = z.focus.is_none();
    if follow {
        note(ui, pal, "Following the cursor.");
    }
    ui.horizontal(|ui| {
        let pick = ed.st.picking;
        if ui.selectable_label(pick, "Pick on preview").on_hover_text("Click the preview to set the focus").clicked() {
            ed.st.picking = !pick;
        }
        if ui.add_enabled(!follow, egui::Button::new("Follow cursor")).clicked() {
            edit(ed, EditKind::Other, ZoomField::Focus(None));
        }
    });
    if let Some(v) = drag(ui, "Ease in", z.ease_in, 0.0..=3.0, 0.02, " s") {
        edit(ed, EditKind::Zoom, ZoomField::EaseIn(v));
    }
    if let Some(v) = drag(ui, "Ease out", z.ease_out, 0.0..=3.0, 0.02, " s") {
        edit(ed, EditKind::Zoom, ZoomField::EaseOut(v));
    }
    if ui.button("Delete zoom").clicked() {
        let id = z.id.clone();
        if ed.apply(EditKind::Other, now, move |p| delete_zoom(p, &id)) {
            ed.st.selected_zoom = None;
            ed.st.picking = false;
        }
    }
    ui.separator();
}

/// A click on the preview while picking: set the focus of the selected zoom. `click` is `0..=1` in the
/// preview image; `camera` is the crop the frame on screen shows.
pub fn pick_focus(ed: &mut Editor, click: [f64; 2], camera: Option<[f64; 4]>, now: f64) {
    let (Some(id), Some(style)) = (ed.st.selected_zoom.clone(), ed.project.as_ref().map(|p| p.style.clone())) else { return };
    let aspect = ed.video_aspect();
    match preview_to_focus(click, &style, aspect, camera) {
        Some(f) => {
            ed.st.picking = false;
            ed.apply(EditKind::Other, now, move |p| set_zoom(p, &id, ZoomField::Focus(Some(f))));
        }
        None => ed.st.edit_error = Some("Click inside the video, not on the background.".into()),
    }
}

fn cursor_tab(ui: &mut Ui, pal: &Palette, ed: &mut Editor, proj: &Project, now: f64) {
    let c = &proj.cursor;
    ui.horizontal(|ui| {
        for (id, label) in CURSOR_STYLES {
            if ui.selectable_label(c.style == id, label).clicked() {
                ed.apply(EditKind::Other, now, |p| set_cursor(p, CursorField::Style(id.into())));
            }
        }
    });
    if let Some(v) = row_slider(ui, pal, "Size", c.size, 0.5..=4.0, 1.0, "×") {
        ed.apply(EditKind::Cursor, now, |p| set_cursor(p, CursorField::Size(v)));
    }
    if let Some(v) = row_slider(ui, pal, "Smoothing", c.smoothing, 0.0..=1.0, 100.0, " %") {
        ed.apply(EditKind::Cursor, now, |p| set_cursor(p, CursorField::Smoothing(v)));
    }
    let mut ring = c.click_effect == "ring";
    if ui.checkbox(&mut ring, "Click ring").changed() {
        ed.apply(EditKind::Other, now, |p| set_cursor(p, CursorField::ClickRing(ring)));
    }
    if let Some(v) = row_slider(ui, pal, "Motion blur", c.motion_blur, 0.0..=2.0, 100.0, " %") {
        ed.apply(EditKind::Cursor, now, |p| set_cursor(p, CursorField::MotionBlur(v)));
    }
    note(ui, pal, "The cursor is drawn from the recorded events; None and Hidden remove it.");
}

fn style_tab(ui: &mut Ui, pal: &Palette, ed: &mut Editor, proj: &Project, now: f64) {
    let s = &proj.style;
    ui.horizontal_wrapped(|ui| {
        ui.label("Aspect");
        for (id, label) in ASPECTS {
            if ui.selectable_label(s.aspect == id, label).clicked() {
                ed.apply(EditKind::Other, now, |p| set_style(p, StyleField::Aspect(id.into())));
            }
        }
    });
    background_rows(ui, pal, ed, s, now);
    if let Some(v) = row_slider(ui, pal, "Padding", s.padding, 0.0..=0.45, 100.0, " %") {
        ed.apply(EditKind::Style, now, |p| set_style(p, StyleField::Padding(v)));
    }
    if let Some(v) = row_slider(ui, pal, "Corners", s.radius, 0.0..=64.0, 1.0, " px") {
        ed.apply(EditKind::Style, now, |p| set_style(p, StyleField::Radius(v)));
    }
    if let Some(v) = row_slider(ui, pal, "Shadow", s.shadow.blur, 0.0..=120.0, 1.0, " px") {
        ed.apply(EditKind::Style, now, |p| set_style(p, StyleField::ShadowBlur(v)));
    }
    if let Some(v) = row_slider(ui, pal, "Opacity", s.shadow.opacity, 0.0..=1.0, 100.0, " %") {
        ed.apply(EditKind::Style, now, |p| set_style(p, StyleField::ShadowOpacity(v)));
    }
    if let Some(v) = row_slider(ui, pal, "Offset", s.shadow.dy, 0.0..=80.0, 1.0, " px") {
        ed.apply(EditKind::Style, now, |p| set_style(p, StyleField::ShadowDy(v)));
    }
}

fn background_rows(ui: &mut Ui, pal: &Palette, ed: &mut Editor, s: &Style, now: f64) {
    let bg = &s.background;
    ui.horizontal_wrapped(|ui| {
        ui.label("Background");
        for (id, label) in [("color", "Colour"), ("gradient", "Gradient"), ("blur", "Blur")] {
            if ui.selectable_label(bg.kind == id, label).clicked() {
                ed.apply(EditKind::Other, now, |p| set_style(p, StyleField::BgKind(id.into())));
            }
        }
    });
    let stops = match bg.kind.as_str() {
        "color" => 1,
        "gradient" => bg.stops.len().min(4),
        _ => 0,
    };
    ui.horizontal(|ui| {
        for (i, hex) in bg.stops.iter().take(stops).enumerate() {
            let mut rgb = hex_to_rgb(hex).unwrap_or([0, 0, 0]);
            if ui.color_edit_button_srgb(&mut rgb).changed() {
                ed.apply(EditKind::Style, now, |p| set_style(p, StyleField::BgStop(i, rgb_to_hex(rgb))));
            }
        }
    });
    if bg.kind == "gradient" {
        if let Some(v) = row_slider(ui, pal, "Angle", bg.angle, 0.0..=360.0, 1.0, "°") {
            ed.apply(EditKind::Style, now, |p| set_style(p, StyleField::BgAngle(v)));
        }
        ui.horizontal_wrapped(|ui| {
            for (i, (name, _, _)) in WALLPAPERS.iter().enumerate() {
                if ui.small_button(*name).clicked() {
                    ed.apply(EditKind::Other, now, |p| set_style(p, StyleField::Wallpaper(i)));
                }
            }
        });
    } else if bg.kind == "blur" {
        note(ui, pal, "The recording itself, blurred.");
    }
}

// ---------------------------------------------------------------------------------------------
// Panel widgets of the Camera / Audio / Transcript tabs. They paint with the app palette only
// (no animation), every clickable thing is at least `HIT_H` tall and every state has a word or a
// shape next to its colour.

/// Height of a clickable row in the panel.
pub const HIT_H: f32 = 26.0;
/// Height of a tab chip.
const TAB_H: f32 = 24.0;
/// How much of the accent tints a selected chip (low enough for the accent text on it to keep 4.5:1).
pub const ACCENT_TINT: f32 = 0.10;
/// Horizontal gap between chips.
const CHIP_GAP: f32 = 4.0;

/// The widths of a wrapping chip strip: the chips (their text widths plus `pad` on both sides) are
/// split into the fewest rows that fit `avail`, as evenly as possible (7 chips become 4 + 3), and
/// each row's spare room is shared between its chips, so every row is exactly `avail` wide.
pub fn chip_rows(text_w: &[f32], avail: f32, gap: f32, pad: f32) -> Vec<Vec<f32>> {
    let n = text_w.len();
    if n == 0 {
        return vec![];
    }
    let w: Vec<f32> = text_w.iter().map(|t| t + 2.0 * pad).collect();
    for rows in 1..=n {
        let (base, extra) = (n / rows, n % rows);
        let (mut out, mut i, mut fits) = (Vec::with_capacity(rows), 0, true);
        for r in 0..rows {
            let cnt = base + usize::from(r < extra);
            let row = &w[i..i + cnt];
            i += cnt;
            let need = row.iter().sum::<f32>() + gap * (cnt as f32 - 1.0);
            if need > avail {
                fits = false;
                break;
            }
            let spare = (avail - need) / cnt as f32;
            out.push(row.iter().map(|x| x + spare).collect());
        }
        if fits {
            return out;
        }
    }
    w.iter().map(|_| vec![avail]).collect()
}

fn font(size: f32) -> egui::FontId {
    egui::FontId::proportional(size)
}

/// A button-like painted cell: the response, with the keyboard focus ring and the screen-reader label.
fn cell(ui: &mut Ui, pal: &Palette, rect: Rect, id: egui::Id, label: &str, selected: Option<bool>) -> egui::Response {
    let resp = ui.interact(rect, id, egui::Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
    let enabled = ui.is_enabled();
    resp.widget_info(|| match selected {
        Some(on) => egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, enabled, on, label),
        None => egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label),
    });
    if resp.has_focus() {
        ui.painter().rect_stroke(rect.expand(1.0), 7.0, egui::Stroke::new(2.0, pal.accent.gamma_multiply(0.7)), egui::StrokeKind::Outside);
    }
    resp
}

/// The tab strip: equal-height chips, wrapped into balanced rows that fill the panel width.
fn tab_strip(ui: &mut Ui, pal: &Palette, tabs: &[Tab], current: Tab) -> Option<Tab> {
    let widths: Vec<f32> = tabs.iter().map(|t| ui.painter().layout_no_wrap(t.label().into(), font(12.5), pal.text).size().x).collect();
    let avail = ui.available_width();
    let mut clicked = None;
    let mut i = 0;
    for row in chip_rows(&widths, avail, CHIP_GAP, 9.0) {
        let (strip, _) = ui.allocate_exact_size(egui::vec2(avail, TAB_H), egui::Sense::hover());
        let mut x = strip.min.x;
        for w in row {
            let t = tabs[i];
            i += 1;
            let hit = Rect::from_min_size(egui::pos2(x, strip.min.y), egui::vec2(w, TAB_H));
            x += w + CHIP_GAP;
            let resp = cell(ui, pal, hit, ui.id().with(("tab", t.label())), t.label(), Some(t == current));
            let on = t == current;
            let chip = hit;
            let fill = if on {
                pal.accent.gamma_multiply(ACCENT_TINT)
            } else if resp.hovered() {
                pal.sel
            } else {
                pal.field
            };
            ui.painter().rect_filled(chip, 7.0, fill);
            let color = if on { pal.accent_text } else { pal.sub_strong };
            ui.painter().text(chip.center(), egui::Align2::CENTER_CENTER, t.label(), font(12.5), color);
            if resp.clicked() {
                clicked = Some(t);
            }
        }
    }
    clicked
}

/// A small caption above a control group.
fn caption(ui: &mut Ui, pal: &Palette, text: &str) {
    ui.label(egui::RichText::new(text).size(12.0).color(pal.sub_strong));
}

/// A rounded, hairline-edged container that fills the panel width.
fn card<R>(ui: &mut Ui, pal: &Palette, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(pal.field)
        .stroke(egui::Stroke::new(1.0, pal.sep))
        .corner_radius(8)
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 6.0;
            add(ui)
        })
        .inner
}

/// A full-width (or `width`-wide) secondary button; a disabled one is dimmed and inert.
fn secondary_button(ui: &mut Ui, pal: &Palette, label: &str, width: f32, enabled: bool) -> bool {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, HIT_H), egui::Sense::hover());
    let resp = cell(ui, pal, rect, ui.id().with(("btn", label)), label, None);
    let hover = enabled && resp.hovered();
    ui.painter().rect_filled(rect, 7.0, if hover { pal.sel } else { pal.field.gamma_multiply(1.6) });
    let color = if enabled { pal.text } else { pal.sub };
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, label, font(12.5), color);
    enabled && resp.clicked()
}

/// The main action of a tab: accent fill, white label.
fn primary_button(ui: &mut Ui, pal: &Palette, label: &str) -> bool {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 32.0), egui::Sense::hover());
    let resp = cell(ui, pal, rect, ui.id().with(("primary", label)), label, None);
    ui.painter().rect_filled(rect, 8.0, if resp.hovered() { pal.accent.gamma_multiply(1.15) } else { pal.accent });
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, label, font(13.0), egui::Color32::WHITE);
    resp.clicked()
}

/// `label` on the left, a switch on the right; the whole row is the click target.
fn toggle_row(ui: &mut Ui, pal: &Palette, label: &str, on: bool) -> bool {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), HIT_H), egui::Sense::hover());
    let resp = ui.interact(rect, ui.id().with(("toggle", label)), egui::Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
    let enabled = ui.is_enabled();
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, enabled, on, label));
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, 7.0, pal.field);
    }
    p.text(egui::pos2(rect.min.x + 2.0, rect.center().y), egui::Align2::LEFT_CENTER, label, font(13.0), pal.text);
    let track = Rect::from_center_size(egui::pos2(rect.max.x - 22.0, rect.center().y), egui::vec2(34.0, 20.0));
    let off = if pal.text.r() > 128 { egui::Color32::from_gray(74) } else { egui::Color32::from_gray(200) };
    p.rect_filled(track, 10.0, if on { pal.accent } else { off });
    let kx = if on { track.max.x - 10.0 } else { track.min.x + 10.0 };
    p.circle_filled(egui::pos2(kx, track.center().y), 7.5, egui::Color32::WHITE);
    if resp.has_focus() {
        p.rect_stroke(rect.expand(1.0), 7.0, egui::Stroke::new(2.0, pal.accent.gamma_multiply(0.7)), egui::StrokeKind::Outside);
    }
    resp.clicked()
}

#[derive(Clone, Copy)]
enum Glyph {
    Circle,
    Rounded,
    Square,
}

fn paint_glyph(p: &egui::Painter, at: egui::Pos2, g: Glyph, color: egui::Color32) {
    let s = egui::Stroke::new(1.5, color);
    let r = Rect::from_center_size(at, egui::vec2(12.0, 12.0));
    match g {
        Glyph::Circle => {
            p.circle_stroke(at, 6.0, s);
        }
        Glyph::Rounded => {
            p.rect_stroke(r, 3.5, s, egui::StrokeKind::Inside);
        }
        Glyph::Square => {
            p.rect_stroke(r, 0.5, s, egui::StrokeKind::Inside);
        }
    }
}

/// An equal-width segmented control (the look of the Settings bars), each segment with a small
/// drawn shape and its name. Returns the clicked index.
fn shape_segments(ui: &mut Ui, pal: &Palette, items: &[(Glyph, &str)], current: usize) -> Option<usize> {
    let (track, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), HIT_H), egui::Sense::hover());
    let seg_w = (track.width() - 4.0) / items.len() as f32;
    ui.painter().rect_filled(track, 8.0, pal.field.gamma_multiply(1.3));
    let mut clicked = None;
    for (i, (glyph, label)) in items.iter().enumerate() {
        let hit = Rect::from_min_size(egui::pos2(track.min.x + 2.0 + i as f32 * seg_w, track.min.y), egui::vec2(seg_w, HIT_H));
        let resp = cell(ui, pal, hit, ui.id().with(("seg", *label)), label, Some(i == current));
        let on = i == current;
        if on {
            ui.painter().rect_filled(hit.shrink2(egui::vec2(0.0, 2.0)), 6.0, pal.sel.gamma_multiply(1.8));
        } else if resp.hovered() {
            ui.painter().rect_filled(hit.shrink2(egui::vec2(0.0, 2.0)), 6.0, pal.field);
        }
        let color = if on { pal.text } else { pal.sub_strong };
        let galley = ui.painter().layout_no_wrap((*label).into(), font(12.0), color);
        let total = 12.0 + 6.0 + galley.size().x;
        let x0 = hit.center().x - total / 2.0;
        paint_glyph(ui.painter(), egui::pos2(x0 + 6.0, hit.center().y), *glyph, color);
        ui.painter().galley(egui::pos2(x0 + 18.0, hit.center().y - galley.size().y / 2.0), galley, color);
        if resp.clicked() {
            clicked = Some(i);
        }
    }
    clicked
}

/// The four corners as a 2 x 2 grid of cells, each showing a mini frame with the camera bubble in
/// that corner; the active one is filled and outlined. Returns the clicked corner.
fn corner_picker(ui: &mut Ui, pal: &Palette, current: Corner) -> Option<Corner> {
    let corners = [
        (Corner::TopLeft, "Top left"),
        (Corner::TopRight, "Top right"),
        (Corner::BottomLeft, "Bottom left"),
        (Corner::BottomRight, "Bottom right"),
    ];
    let w = (ui.available_width() - CHIP_GAP) / 2.0;
    let mut clicked = None;
    let (area, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 2.0 * 32.0 + CHIP_GAP), egui::Sense::hover());
    for (i, (corner, label)) in corners.iter().enumerate() {
        let (col, row) = ((i % 2) as f32, (i / 2) as f32);
        let hit =
            Rect::from_min_size(egui::pos2(area.min.x + col * (w + CHIP_GAP), area.min.y + row * (32.0 + CHIP_GAP)), egui::vec2(w, 32.0));
        let resp = cell(ui, pal, hit, ui.id().with(("corner", *label)), label, Some(*corner == current));
        let on = *corner == current;
        let fill = if on {
            pal.accent.gamma_multiply(ACCENT_TINT)
        } else if resp.hovered() {
            pal.sel
        } else {
            pal.field.gamma_multiply(1.3)
        };
        let p = ui.painter();
        p.rect_filled(hit, 7.0, fill);
        if on {
            p.rect_stroke(hit, 7.0, egui::Stroke::new(1.0, pal.accent.gamma_multiply(0.7)), egui::StrokeKind::Inside);
        }
        // the bubble sits in the cell's own corner, like the camera in the frame
        let bubble = Rect::from_min_size(
            egui::pos2(
                if col == 0.0 { hit.min.x + 6.0 } else { hit.max.x - 6.0 - 12.0 },
                if row == 0.0 { hit.min.y + 5.0 } else { hit.max.y - 5.0 - 9.0 },
            ),
            egui::vec2(12.0, 9.0),
        );
        let ink = if on { pal.accent_text } else { pal.sub_strong };
        if on {
            p.rect_filled(bubble, 3.0, ink);
        } else {
            p.rect_stroke(bubble, 3.0, egui::Stroke::new(1.2, ink), egui::StrokeKind::Inside);
        }
        p.text(hit.center(), egui::Align2::CENTER_CENTER, *label, font(12.0), if on { pal.text } else { pal.sub_strong });
        if resp.clicked() {
            clicked = Some(*corner);
        }
    }
    clicked
}

/// The recorded side files of the project, as quiet secondary text (a missing file in the danger colour).
fn source_rows(ui: &mut Ui, pal: &Palette, ed: &Editor, proj: &Project, only: &[&str]) {
    for info in source_infos(proj, &ed.dir).iter().filter(|i| only.contains(&i.label)) {
        ui.label(egui::RichText::new(info.label).size(11.5).color(pal.sub_strong));
        let color = if info.missing { pal.danger } else { pal.sub_strong };
        ui.label(egui::RichText::new(info.text()).size(11.5).color(color));
    }
}

fn camera_tab(ui: &mut Ui, pal: &Palette, ed: &mut Editor, proj: &Project, now: f64) {
    let c = proj.camera.unwrap_or_default();
    if toggle_row(ui, pal, "Show camera", c.visible) {
        ed.apply(EditKind::Other, now, |p| set_camera(p, CameraField::Visible(!c.visible)));
    }
    ui.add_space(4.0);
    caption(ui, pal, "Shape");
    let shapes = [CameraShape::Circle, CameraShape::Rounded, CameraShape::Square];
    let items = [(Glyph::Circle, "Circle"), (Glyph::Rounded, "Rounded"), (Glyph::Square, "Square")];
    let cur = shapes.iter().position(|s| *s == c.shape).unwrap_or(1);
    if let Some(i) = shape_segments(ui, pal, &items, cur) {
        ed.apply(EditKind::Other, now, |p| set_camera(p, CameraField::Shape(shapes[i])));
    }
    ui.add_space(4.0);
    caption(ui, pal, "Corner");
    if let Some(corner) = corner_picker(ui, pal, c.corner) {
        ed.apply(EditKind::Other, now, |p| set_camera(p, CameraField::Corner(corner)));
    }
    ui.add_space(4.0);
    if let Some(v) = row_slider(ui, pal, "Size", c.size, SIZE_MIN..=SIZE_MAX, 100.0, " %") {
        ed.apply(EditKind::Style, now, |p| set_camera(p, CameraField::Size(v)));
    }
    if let Some(v) = row_slider(ui, pal, "Margin", c.margin, 0.0..=MARGIN_MAX, 100.0, " %") {
        ed.apply(EditKind::Style, now, |p| set_camera(p, CameraField::Margin(v)));
    }
    if toggle_row(ui, pal, "Mirror", c.mirror) {
        ed.apply(EditKind::Other, now, |p| set_camera(p, CameraField::Mirror(!c.mirror)));
    }
    ui.add_space(6.0);
    source_rows(ui, pal, ed, proj, &["Camera"]);
    note(ui, pal, "The camera is recorded as its own file, so it can be moved, resized or hidden without re-recording.");
}

/// The mute switch of a track card: a pill that says `Mute` or `Muted` (so the state is a word, not only a colour).
fn mute_pill(ui: &mut Ui, pal: &Palette, track: &str, muted: bool) -> bool {
    let label = if muted { "Muted" } else { "Mute" };
    let w = ui.painter().layout_no_wrap(label.into(), font(12.0), pal.text).size().x + 24.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w.max(60.0), 24.0), egui::Sense::hover());
    let resp = cell(ui, pal, rect, ui.id().with(("mute", track)), label, Some(muted));
    let (fill, ink) = if muted {
        (pal.accent.gamma_multiply(ACCENT_TINT), pal.accent_text)
    } else if resp.hovered() {
        (pal.sel, pal.sub_strong)
    } else {
        (pal.field.gamma_multiply(1.3), pal.sub_strong)
    };
    ui.painter().rect_filled(rect, 12.0, fill);
    if muted {
        ui.painter().rect_stroke(rect, 12.0, egui::Stroke::new(1.0, pal.accent.gamma_multiply(0.6)), egui::StrokeKind::Inside);
    }
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, label, font(12.0), ink);
    resp.clicked()
}

fn audio_tab(ui: &mut Ui, pal: &Palette, ed: &mut Editor, proj: &Project, now: f64) {
    let infos = source_infos(proj, &ed.dir);
    for (track, label, present) in
        [(AudioTrack::Mic, "Microphone", proj.sources.mic.is_some()), (AudioTrack::System, "System audio", proj.sources.system.is_some())]
    {
        if !present {
            continue;
        }
        let m = track_mix(proj, track);
        card(ui, pal, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(label).size(13.0).strong().color(pal.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if mute_pill(ui, pal, label, m.mute) {
                        ed.apply(EditKind::Other, now, |p| set_audio(p, AudioField::Mute(track, !m.mute)));
                    }
                });
            });
            if let Some(info) = infos.iter().find(|i| i.label == label) {
                let color = if info.missing { pal.danger } else { pal.sub_strong };
                ui.label(egui::RichText::new(info.text()).size(11.5).color(color));
            }
            // a muted track keeps its gain, shown dimmed
            ui.scope(|ui| {
                if m.mute {
                    ui.set_opacity(0.7);
                }
                if let Some(v) = row_slider(ui, pal, "Gain", m.gain_db, GAIN_DB_MIN..=GAIN_DB_MAX, 1.0, " dB") {
                    ed.apply(EditKind::Audio, now, |p| set_audio(p, AudioField::Gain(track, v)));
                }
            });
        });
    }
    ui.add_space(4.0);
    card(ui, pal, |ui| {
        ui.label(egui::RichText::new("Master").size(13.0).strong().color(pal.text));
        if let Some(v) = row_slider(ui, pal, "Gain", proj.audio.master_db, GAIN_DB_MIN..=GAIN_DB_MAX, 1.0, " dB") {
            ed.apply(EditKind::Audio, now, |p| set_audio(p, AudioField::Master(v)));
        }
    });
    ui.add_space(2.0);
    note(ui, pal, "The mix is applied when you export; the preview is silent.");
}

/// A message card: an `!` badge and the text, tinted with the danger colour (the words carry the meaning).
fn error_card(ui: &mut Ui, pal: &Palette, text: &str) {
    egui::Frame::new()
        .fill(pal.danger.gamma_multiply(0.12))
        .stroke(egui::Stroke::new(1.0, pal.danger.gamma_multiply(0.4)))
        .corner_radius(8)
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(egui::RichText::new(text).size(12.0).color(pal.text));
        });
}

fn transcript_tab(ui: &mut Ui, pal: &Palette, ed: &mut Editor, proj: &Project, fps: f64, now: f64) {
    if transcript_view(proj) == TranscriptView::Words {
        return transcript_words(ui, pal, ed, proj, fps, now);
    }
    if proj.transcript.is_some() {
        note(ui, pal, "No words were found in the audio.");
    }
    if let Some(f) = ed.st.tr.error.clone() {
        error_card(ui, pal, &f.text());
        if f == TranscribeFailure::Permission && secondary_button(ui, pal, "Open Speech Recognition settings", ui.available_width(), true) {
            crate::capture_ctl::open_pane(crate::capture_ctl::SPEECH_PANE);
        }
        ui.add_space(4.0);
    }
    if let Some(r) = ed.st.tr.run.as_ref() {
        let (done, total, cancelling) = (r.done, r.total, r.cancelling);
        let frac = if total > 0.0 { (done / total).clamp(0.0, 1.0) as f32 } else { 0.0 };
        let text = if cancelling {
            "Cancelling…".to_string()
        } else if total > 0.0 {
            format!("Transcribing… {:.0} %", frac * 100.0)
        } else {
            "Starting…".to_string()
        };
        let mut cancel = false;
        card(ui, pal, |ui| {
            ui.label(egui::RichText::new(text).size(13.0).color(pal.text));
            let (bar, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 6.0), egui::Sense::hover());
            ui.painter().rect_filled(bar, 3.0, pal.sep);
            if frac > 0.0 {
                ui.painter().rect_filled(Rect::from_min_size(bar.min, egui::vec2(bar.width() * frac, bar.height())), 3.0, pal.accent);
            }
            ui.add_space(2.0);
            cancel = secondary_button(ui, pal, "Cancel", ui.available_width(), !cancelling);
        });
        if cancel {
            ed.cancel_transcribe();
        }
        ui.add_space(2.0);
        note(ui, pal, "Closing the editor cancels it.");
        return;
    }
    if !can_transcribe(proj) {
        note(ui, pal, "This recording has no audio to transcribe.");
        return;
    }
    note(ui, pal, "Turn the audio into words you can click, select and cut.");
    ui.add_space(4.0);
    caption(ui, pal, "Language");
    let mut loc = ed.st.tr.locale.min(LOCALES.len() - 1);
    egui::ComboBox::from_id_salt("transcript-locale").width(ui.available_width()).selected_text(LOCALES[loc].1).show_ui(ui, |ui| {
        for (i, (_, name)) in LOCALES.iter().enumerate() {
            ui.selectable_value(&mut loc, i, *name);
        }
    });
    ed.st.tr.locale = loc;
    ui.add_space(4.0);
    if primary_button(ui, pal, "Transcribe") {
        ed.st.tr.start = true;
    }
    note(ui, pal, "Transcription runs on this Mac. Nothing leaves it.");
}

/// Height of one line of the word flow.
const WORD_LINE_H: f32 = 22.0;

/// The colour of a filler word: amber, dark enough on the light panel to read.
pub fn filler_color(dark: bool) -> egui::Color32 {
    if dark {
        egui::Color32::from_rgb(232, 163, 76)
    } else {
        egui::Color32::from_rgb(150, 82, 0)
    }
}

fn transcript_words(ui: &mut Ui, pal: &Palette, ed: &mut Editor, proj: &Project, fps: f64, now: f64) {
    use crate::editor_ctl::clock;
    use egui::{Sense, Vec2};
    let Some(tr) = proj.transcript.as_ref() else { return };
    let n = tr.words.len();
    let sel = selection_bounds(ed.st.tr.sel, n);
    let (mut fillers, mut pauses, mut delete) = (false, false, false);
    // the two automatic edits side by side, the selection edit under them
    let half = (ui.available_width() - CHIP_GAP) / 2.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = CHIP_GAP;
        fillers = secondary_button(ui, pal, "Remove fillers", half, true);
        pauses = secondary_button(ui, pal, "Tighten pauses", half, true);
    });
    delete = secondary_button(ui, pal, "Delete selection", ui.available_width(), sel.is_some()) || delete;
    if let Some(msg) = ed.st.tr.notice.as_deref() {
        note(ui, pal, msg);
    }
    // both lines keep their height whatever the selection is, so the words never move under a drag
    let status = match sel {
        Some((a, b)) => {
            let k = b - a + 1;
            format!("{k} word{} selected · {}–{}", if k == 1 { "" } else { "s" }, clock(tr.words[a].start), clock(tr.words[b].end))
        }
        None => "No selection".to_string(),
    };
    note(ui, pal, &status);
    let tl = ed.timeline.clone();
    let cur = word_at(tr, ed.t);
    let flags = filler_flags(tr, proj.duration);
    let shift = ui.input(|i| i.modifiers.shift);
    let filler = filler_color(ui.visuals().dark_mode);
    let (mut click, mut drag_start, mut dragging) = (None, None, false);
    let mut rects: Vec<(usize, Rect)> = Vec::with_capacity(n);
    egui::Frame::new().fill(pal.field).stroke(egui::Stroke::new(1.0, pal.sep)).corner_radius(8).inner_margin(egui::Margin::same(10)).show(
        ui,
        |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                // roomy lines (the wrap ignores the vertical spacing, so the height is the word's own)
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
                for (i, w) in tr.words.iter().enumerate() {
                    let cut = tl.as_ref().is_some_and(|tl| word_is_cut(tl, w));
                    let is_filler = flags.get(i).copied().unwrap_or(false);
                    let color = if cut {
                        pal.sub
                    } else if is_filler {
                        filler
                    } else {
                        pal.text
                    };
                    let mut rt = egui::RichText::new(w.text.as_str()).size(13.5).line_height(Some(WORD_LINE_H)).color(color);
                    if cut {
                        rt = rt.strikethrough();
                    }
                    if is_filler && !cut {
                        rt = rt.italics();
                    }
                    if sel.is_some_and(|(a, b)| (a..=b).contains(&i)) {
                        rt = rt.background_color(pal.accent.gamma_multiply(0.38));
                    } else if cur == Some(i) {
                        // the word under the playhead: a soft tint and an underline
                        rt = rt.background_color(pal.accent.gamma_multiply(0.20)).underline();
                    }
                    let r = ui.add(egui::Label::new(rt).selectable(false).sense(Sense::click_and_drag()));
                    if r.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if r.clicked() {
                        click = Some(i);
                    }
                    if r.drag_started() {
                        drag_start = Some(i);
                    }
                    dragging |= r.dragged();
                    rects.push((i, r.rect));
                }
            });
        },
    );
    if let Some(i) = drag_start {
        ed.st.tr.anchor = Some(i);
        ed.st.tr.sel = Some((i, i));
    }
    if dragging {
        let pos = ui.input(|i| i.pointer.interact_pos());
        let hit = pos.and_then(|p| rects.iter().find(|(_, r)| r.expand2(Vec2::new(3.0, 1.0)).contains(p)).map(|(i, _)| *i));
        if let (Some(a), Some(j)) = (ed.st.tr.anchor, hit) {
            ed.st.tr.sel = Some((a, j));
        }
    }
    if let Some(i) = click {
        if shift {
            let a = ed.st.tr.anchor.unwrap_or(i);
            ed.st.tr.anchor = Some(a);
            ed.st.tr.sel = Some((a, i));
        } else {
            ed.st.tr.anchor = Some(i);
            ed.st.tr.sel = None;
            if let Some(tl) = tl.as_ref() {
                ed.seek(nearest_kept(tl, tr.words[i].start), now, fps);
            }
        }
    }
    if delete {
        ed.transcript_cut(CutAction::Selection, now);
    }
    if fillers {
        ed.transcript_cut(CutAction::Fillers, now);
    }
    if pauses {
        ed.transcript_cut(CutAction::Pauses, now);
    }
    ui.add_space(2.0);
    // the hint sits under the words, so the first lines of the transcript are in view in the small panel
    note(ui, pal, "Click a word to move the playhead. Shift-click or drag to select.");
    note(ui, pal, "Cut words are struck through. Filler words are orange.");
}

fn clips_tab(ui: &mut Ui, pal: &Palette, ed: &mut Editor, proj: &Project, fps: f64, now: f64) {
    use crate::editor_ctl::clock;
    let t = ed.t;
    ui.horizontal_wrapped(|ui| {
        if ui.button("Trim in").on_hover_text("Cut everything before the playhead").clicked() {
            ed.apply(EditKind::Other, now, |p| trim_in(p, t));
        }
        if ui.button("Trim out").on_hover_text("Cut everything after the playhead").clicked() {
            ed.apply(EditKind::Other, now, |p| trim_out(p, t));
        }
        if ui.button("Split").on_hover_text("Split the segment at the playhead").clicked() {
            ed.apply(EditKind::Split, now, |p| split_at(p, t));
        }
        if ui.button("Delete segment").clicked() {
            ed.apply(EditKind::Other, now, |p| delete_segment(p, t));
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Speed");
        for f in [0.25, 0.5, 1.0, 2.0, 4.0, 8.0, 16.0] {
            if ui.selectable_label((ed.st.speed - f).abs() < 1e-9, format!("{f}×")).clicked() {
                ed.st.speed = f;
            }
        }
    });
    ui.horizontal(|ui| {
        let mut f = ed.st.speed;
        if ui
            .add(egui::DragValue::new(&mut f).range(ferrocast_core::videoplan::SPEED_MIN..=timeline::SPEED_MAX).speed(0.05).suffix("×"))
            .changed()
        {
            ed.st.speed = f;
        }
        if ui.button("Apply to segment").clicked() {
            let f = ed.st.speed;
            ed.apply(EditKind::Other, now, |p| set_segment_speed(p, t, f));
        }
    });
    ui.separator();
    let Some(tl) = ed.timeline.clone() else { return };
    note(ui, pal, "Segments (click to move the playhead)");
    for (i, b) in crate::editor_ctl::segment_bars(&tl).iter().enumerate() {
        let here = ed.t >= b.start && ed.t < b.end;
        let speed = match b.kind {
            crate::editor_ctl::BarKind::Speed(f) => format!("  ·  {f}×"),
            _ => String::new(),
        };
        if ui.selectable_label(here, format!("{}  {}–{}{speed}", i + 1, clock(b.start), clock(b.end))).clicked() {
            ed.seek(b.start, now, fps);
        }
    }
    note(ui, pal, &format!("{} → {} after the edits", clock(proj.duration), clock(tl.output_duration())));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor_ctl::{Opened, Req};
    use std::path::{Path, PathBuf};
    use std::sync::mpsc::{channel, Receiver};

    fn base() -> Project {
        let mut p = Project::new("t", "2026-01-01T00:00:00Z");
        p.duration = 10.0;
        p
    }

    fn zoom(id: &str, start: f64, end: f64, mode: &str) -> Zoom {
        serde_json::from_value(serde_json::json!({"id":id,"start":start,"end":end,"mode":mode,"scale":2.0})).unwrap()
    }

    // ---- AutosaveClock

    #[test]
    fn nothing_is_written_before_one_idle_second_or_when_clean() {
        let mut c = AutosaveClock::new(1.0);
        assert!(!c.due(1000.0) && !c.pending() && c.wait(0.0).is_none(), "a clean project never writes");
        assert_eq!(c.status(), SaveStatus::Saved);
        c.changed(10.0);
        assert_eq!(c.status(), SaveStatus::Saving);
        assert!(!c.due(10.0) && !c.due(10.99), "no write before 1 s of quiet");
        assert!(c.due(11.0));
        assert!((c.wait(10.25).unwrap() - 0.75).abs() < 1e-9);
    }

    #[test]
    fn a_burst_of_changes_is_one_write_after_the_last_one() {
        let mut c = AutosaveClock::new(1.0);
        for t in [0.0, 0.4, 0.8, 1.5, 2.2] {
            c.changed(t);
            assert!(!c.due(t), "never while the user keeps editing");
        }
        assert!(!c.due(3.19) && c.due(3.2), "1 s after the LAST change");
        c.written();
        assert!(!c.due(99.0) && c.status() == SaveStatus::Saved, "one write per burst");
    }

    #[test]
    fn a_failed_write_stays_dirty_shows_its_text_and_retries_later() {
        let mut c = AutosaveClock::new(1.0);
        c.changed(0.0);
        c.failed(1.0, "disk full".into());
        assert_eq!(c.status(), SaveStatus::Failed("disk full".into()));
        assert!(c.pending(), "still owed");
        assert!(!c.due(1.5), "no retry flood: one more idle second");
        assert!(c.due(2.0));
        c.written();
        assert_eq!(c.status(), SaveStatus::Saved);
        c.failed(5.0, "x".into());
        c.changed(6.0);
        assert_eq!(c.status(), SaveStatus::Saving, "a new change clears the old error");
    }

    // ---- zoom edits

    #[test]
    fn a_manual_zoom_fits_the_gap_and_gets_a_fresh_id() {
        let mut p = base();
        p.zooms = vec![zoom("m1", 5.0, 6.0, "manual")];
        add_zoom(&mut p, 4.0).unwrap();
        let z = p.zooms.iter().find(|z| z.id == "m2").unwrap();
        assert_eq!(
            (z.start, z.end, z.mode.as_str(), z.scale, z.focus),
            (4.0, 5.0, "manual", 2.0, Some([0.5, 0.5])),
            "cut short by the next zoom"
        );
        assert_eq!(add_zoom(&mut p, 5.5).unwrap_err(), "There is already a zoom at the playhead.");
        assert!(add_zoom(&mut p, 9.95).unwrap_err().contains("no room"), "a sliver is refused");
        let mut q = base();
        add_zoom(&mut q, 3.0).unwrap();
        assert_eq!((q.zooms[0].start, q.zooms[0].end), (3.0, 5.0), "NEW_ZOOM_LEN");
    }

    #[test]
    fn zoom_fields_are_clamped_and_editing_makes_a_zoom_manual() {
        let mut p = base();
        p.zooms = vec![zoom("a", 2.0, 4.0, "auto")];
        set_zoom(&mut p, "a", ZoomField::Scale(9.0)).unwrap();
        assert_eq!((p.zooms[0].scale, p.zooms[0].mode.as_str()), (6.0, "manual"));
        set_zoom(&mut p, "a", ZoomField::Scale(0.1)).unwrap();
        assert_eq!(p.zooms[0].scale, 1.0);
        set_zoom(&mut p, "a", ZoomField::Start(99.0)).unwrap();
        assert!((p.zooms[0].start - (4.0 - MIN_ZOOM)).abs() < 1e-9, "start stops before end");
        set_zoom(&mut p, "a", ZoomField::End(99.0)).unwrap();
        assert_eq!(p.zooms[0].end, 10.0, "end stops at the recording end");
        set_zoom(&mut p, "a", ZoomField::Focus(Some([-1.0, 3.0]))).unwrap();
        assert_eq!(p.zooms[0].focus, Some([0.0, 1.0]));
        set_zoom(&mut p, "a", ZoomField::Focus(None)).unwrap();
        assert_eq!(p.zooms[0].focus, None, "follow the cursor");
        assert!(set_zoom(&mut p, "a", ZoomField::Scale(f64::NAN)).is_err());
        assert!(set_zoom(&mut p, "zz", ZoomField::Scale(2.0)).is_err());
        // an auto zoom set to what it already is stays auto
        let mut q = base();
        q.zooms = vec![zoom("a", 2.0, 4.0, "auto")];
        set_zoom(&mut q, "a", ZoomField::Scale(2.0)).unwrap();
        assert_eq!(q.zooms[0].mode, "auto");
        delete_zoom(&mut q, "a").unwrap();
        assert!(q.zooms.is_empty() && delete_zoom(&mut q, "a").is_err());
    }

    #[test]
    fn auto_zoom_replaces_auto_zooms_and_keeps_manual_ones() {
        let mut p = base();
        p.zooms = vec![zoom("a1", 1.0, 2.0, "auto"), zoom("m1", 4.0, 6.0, "manual")];
        replace_auto(&mut p, vec![zoom("n1", 0.5, 1.5, "auto"), zoom("n2", 5.0, 7.0, "auto"), zoom("n3", 8.0, 9.0, "auto")]);
        let ids: Vec<&str> = p.zooms.iter().map(|z| z.id.as_str()).collect();
        assert_eq!(ids, ["n1", "m1", "n3"], "old auto gone, manual kept, the overlapping new one dropped");
        assert!(p.validate(|_| true).iter().all(|m| !m.contains("overlap")));
    }

    // ---- cursor and style

    #[test]
    fn cursor_values_are_clamped_and_styles_are_the_ones_compose_knows() {
        let mut p = base();
        set_cursor(&mut p, CursorField::Size(99.0)).unwrap();
        set_cursor(&mut p, CursorField::Smoothing(-1.0)).unwrap();
        set_cursor(&mut p, CursorField::MotionBlur(5.0)).unwrap();
        assert_eq!((p.cursor.size, p.cursor.smoothing, p.cursor.motion_blur), (4.0, 0.0, 2.0));
        set_cursor(&mut p, CursorField::ClickRing(false)).unwrap();
        assert_eq!(p.cursor.click_effect, "none");
        set_cursor(&mut p, CursorField::ClickRing(true)).unwrap();
        assert_eq!(p.cursor.click_effect, "ring");
        for s in ["none", "hidden", "arrow"] {
            set_cursor(&mut p, CursorField::Style(s.into())).unwrap();
            assert_eq!(p.cursor.style, s);
        }
        assert!(set_cursor(&mut p, CursorField::Style("laser".into())).is_err());
    }

    #[test]
    fn style_values_are_clamped_colours_checked_and_wallpapers_are_valid_gradients() {
        let mut p = base();
        set_style(&mut p, StyleField::Padding(0.9)).unwrap();
        set_style(&mut p, StyleField::Radius(-4.0)).unwrap();
        set_style(&mut p, StyleField::ShadowOpacity(7.0)).unwrap();
        assert_eq!((p.style.padding, p.style.radius, p.style.shadow.opacity), (0.45, 0.0, 1.0));
        assert!(set_style(&mut p, StyleField::Aspect("5:4".into())).is_err());
        set_style(&mut p, StyleField::Aspect("9:16".into())).unwrap();
        assert!(set_style(&mut p, StyleField::BgStop(0, "red".into())).is_err(), "only #rrggbb");
        set_style(&mut p, StyleField::BgStop(0, "#AABBCC".into())).unwrap();
        assert_eq!(p.style.background.stops[0], "#aabbcc");
        assert!(set_style(&mut p, StyleField::BgStop(9, "#000000".into())).is_err());
        set_style(&mut p, StyleField::BgKind("color".into())).unwrap();
        set_style(&mut p, StyleField::BgKind("blur".into())).unwrap();
        assert!(set_style(&mut p, StyleField::BgKind("video".into())).is_err());
        for i in 0..WALLPAPERS.len() {
            set_style(&mut p, StyleField::Wallpaper(i)).unwrap();
            assert!(p.validate(|_| true).is_empty(), "wallpaper {i}: {:?}", p.validate(|_| true));
            assert_eq!(p.style.background.kind, "gradient");
        }
        let mut one = base();
        one.style.background.stops = vec!["#112233".into()];
        set_style(&mut one, StyleField::BgKind("gradient".into())).unwrap();
        assert_eq!(one.style.background.stops.len(), 2, "a gradient has two colours");
    }

    // ---- clips

    #[test]
    fn trim_split_delete_and_speed_work_on_the_segment_under_the_playhead() {
        let mut p = base();
        trim_in(&mut p, 2.0).unwrap();
        trim_out(&mut p, 8.0).unwrap();
        assert_eq!(p.trims, vec![Trim { start: 2.0, end: 8.0 }]);
        assert!(trim_in(&mut p, 1.0).is_err(), "nothing before 1 s is left to cut");
        split_at(&mut p, 5.0).unwrap();
        assert_eq!(p.trims, vec![Trim { start: 2.0, end: 5.0 }, Trim { start: 5.0, end: 8.0 }]);
        assert!(split_at(&mut p, 5.0).is_err(), "not at a boundary");
        assert!(split_at(&mut p, 9.0).is_err(), "not in a cut");
        set_segment_speed(&mut p, 6.0, 4.0).unwrap();
        assert_eq!((p.speeds[0].start, p.speeds[0].end, p.speeds[0].factor), (5.0, 8.0, 4.0));
        assert!(set_segment_speed(&mut p, 6.0, 32.0).unwrap_err().contains("between"));
        assert!(set_segment_speed(&mut p, 6.0, 0.1).is_err());
        set_segment_speed(&mut p, 6.0, 1.0).unwrap();
        assert!(p.speeds.is_empty(), "1x clears the speed");
        delete_segment(&mut p, 3.0).unwrap();
        assert_eq!(p.trims, vec![Trim { start: 5.0, end: 8.0 }]);
        assert!(delete_segment(&mut p, 6.0).is_err(), "the only segment stays");
        assert!(delete_segment(&mut p, 1.0).is_err(), "a cut is not a segment");
        assert!(problems(&p).is_empty(), "{:?}", problems(&p));
    }

    // ---- focus picking

    #[test]
    fn a_click_on_the_preview_maps_to_the_video_through_padding_and_camera() {
        let mut st = Style { padding: 0.0, ..Style::default() };
        let c = preview_to_focus([0.25, 0.5], &st, 16.0 / 9.0, None).unwrap();
        assert!((c[0] - 0.25).abs() < 1e-9 && (c[1] - 0.5).abs() < 1e-9, "no padding: identity {c:?}");
        st.padding = 0.1;
        let c = preview_to_focus([0.5, 0.5], &st, 16.0 / 9.0, None).unwrap();
        assert!((c[0] - 0.5).abs() < 1e-9 && (c[1] - 0.5).abs() < 1e-9, "the centre stays the centre");
        assert!(preview_to_focus([0.02, 0.5], &st, 16.0 / 9.0, None).is_none(), "the padding is not the video");
        // the 16:9 canvas has 10 % padding on every side: the height limits, so the video starts at x = 0.1783
        let left = (1.0 - (1.0 / (16.0 / 9.0) - 0.2) * 16.0 / 9.0) / 2.0;
        assert!(preview_to_focus([left - 1e-3, 0.5], &st, 16.0 / 9.0, None).is_none());
        let edge = preview_to_focus([left + 1e-6, 0.5], &st, 16.0 / 9.0, None).unwrap();
        assert!(edge[0] < 0.001, "{edge:?}");
        // zoomed frame: the preview shows the top-left quarter
        st.padding = 0.0;
        let c = preview_to_focus([0.5, 0.5], &st, 16.0 / 9.0, Some([0.0, 0.0, 0.5, 0.5])).unwrap();
        assert!((c[0] - 0.25).abs() < 1e-9 && (c[1] - 0.25).abs() < 1e-9, "{c:?}");
        // a 1:1 canvas around a 16:9 video letterboxes it vertically
        st.aspect = "1:1".into();
        assert!(preview_to_focus([0.5, 0.05], &st, 16.0 / 9.0, None).is_none());
        assert!(preview_to_focus([0.5, 0.5], &st, 16.0 / 9.0, None).is_some());
    }

    // ---- the editor state: history, validation, worker, autosave

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("fc-panels-{}-{tag}", std::process::id())).join("Take.ferrocast");
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn editor(dir: &Path, auto_zoomed: bool) -> (Editor, Receiver<WorkerCmd>) {
        let (tx, rx) = channel();
        let mut ed = Editor::new(dir.to_path_buf(), Some(tx));
        let mut p = base();
        p.zooms = vec![zoom("a", 1.0, 2.0, "auto")];
        p.save(dir).unwrap();
        ed.opened(Ok(Opened { project: p, auto_zoomed, note: None, events: None }));
        (ed, rx)
    }

    fn disk(dir: &Path) -> Project {
        Project::load(dir).unwrap()
    }

    fn project_cmds(rx: &Receiver<WorkerCmd>) -> Vec<Project> {
        rx.try_iter().filter_map(|c| if let WorkerCmd::Project(p) = c { Some(*p) } else { None }).collect()
    }

    #[test]
    fn every_kind_of_edit_undoes_and_redoes_to_identical_json() {
        let (mut ed, _rx) = editor(&temp("undo"), false);
        let start = ed.project.as_ref().unwrap().to_json();
        let mut now = 0.0;
        let mut step = |ed: &mut Editor, kind, f: &dyn Fn(&mut Project) -> Result<(), String>| {
            now += 10.0; // far apart: no coalescing
            assert!(ed.apply(kind, now, f), "{:?}", ed.st.edit_error);
        };
        step(&mut ed, EditKind::Zoom, &|p| add_zoom(p, 5.0));
        step(&mut ed, EditKind::Zoom, &|p| set_zoom(p, "a", ZoomField::Scale(3.0)));
        step(&mut ed, EditKind::Cursor, &|p| set_cursor(p, CursorField::Size(2.0)));
        step(&mut ed, EditKind::Style, &|p| set_style(p, StyleField::Padding(0.2)));
        step(&mut ed, EditKind::Trim, &|p| trim_in(p, 0.5));
        step(&mut ed, EditKind::Split, &|p| split_at(p, 4.0));
        step(&mut ed, EditKind::Speed, &|p| set_segment_speed(p, 6.0, 2.0));
        step(&mut ed, EditKind::Zoom, &|p| delete_zoom(p, "a"));
        let end = ed.project.as_ref().unwrap().to_json();
        let mut n = 0;
        while ed.undo(100.0) {
            n += 1;
        }
        assert_eq!(n, 8, "one undo step per edit");
        assert_eq!(ed.project.as_ref().unwrap().to_json(), start, "all the way back");
        assert!(!ed.st.history.can_undo() && ed.st.history.can_redo());
        while ed.redo(101.0) {}
        assert_eq!(ed.project.as_ref().unwrap().to_json(), end, "and forward again");
        assert_eq!(ed.timeline.as_ref().unwrap().pieces().len(), 2, "the timeline follows the project");
    }

    #[test]
    fn dragging_a_slider_is_one_undo_step() {
        let (mut ed, _rx) = editor(&temp("drag"), false);
        let start = ed.project.as_ref().unwrap().to_json();
        for i in 1..=30 {
            assert!(ed.apply(EditKind::Style, 5.0 + f64::from(i) * 0.016, |p| set_style(p, StyleField::Padding(f64::from(i) / 100.0))));
        }
        assert_eq!(ed.st.history.undo_len(), 1, "30 drag edits inside the window merge");
        assert!(ed.undo(6.0));
        assert_eq!(ed.project.as_ref().unwrap().to_json(), start);
        // a pause longer than the window starts a new step
        ed.apply(EditKind::Style, 10.0, |p| set_style(p, StyleField::Padding(0.1)));
        ed.apply(EditKind::Style, 12.0, |p| set_style(p, StyleField::Padding(0.2)));
        assert_eq!(ed.st.history.undo_len(), 2);
    }

    #[test]
    fn an_invalid_edit_is_rejected_with_its_message_and_nothing_changes() {
        let dir = temp("invalid");
        let (mut ed, rx) = editor(&dir, false);
        let json = ed.project.as_ref().unwrap().to_json();
        project_cmds(&rx);
        // a zoom on top of zoom `a` (1..2)
        let ok = ed.apply(EditKind::Zoom, 1.0, |p| {
            p.zooms.push(zoom("b", 1.5, 3.0, "manual"));
            Ok(())
        });
        assert!(!ok);
        assert!(ed.st.edit_error.as_deref().is_some_and(|e| e.contains("overlap")), "{:?}", ed.st.edit_error);
        assert_eq!(ed.project.as_ref().unwrap().to_json(), json, "not applied");
        assert!(!ed.st.history.can_undo() && !ed.st.autosave.pending(), "nothing recorded, nothing to save");
        assert!(project_cmds(&rx).is_empty(), "the worker never saw it");
        // a refused edit function reports its own text
        assert!(!ed.apply(EditKind::Trim, 2.0, |p| trim_in(p, 0.0)));
        assert!(ed.st.edit_error.as_deref().is_some_and(|e| e.contains("nothing to trim")));
        // a later good edit clears the message
        assert!(ed.apply(EditKind::Cursor, 3.0, |p| set_cursor(p, CursorField::Size(2.0))));
        assert!(ed.st.edit_error.is_none());
        // a project that was already invalid does not block unrelated edits
        let (mut bad, _rx) = editor(&temp("invalid2"), false);
        bad.project.as_mut().unwrap().export.fps = 25;
        assert!(bad.apply(EditKind::Style, 1.0, |p| set_style(p, StyleField::Radius(4.0))), "{:?}", bad.st.edit_error);
    }

    #[test]
    fn an_edit_reaches_the_worker_and_the_same_frame_is_rendered_again() {
        let (mut ed, rx) = editor(&temp("render"), false);
        ed.want(960);
        let first = rx.try_iter().find_map(|c| if let WorkerCmd::Frame(r) = c { Some(r) } else { None }).unwrap();
        let fake = |t: f64| crate::editor_ctl::FrameOut { image: egui::ColorImage::example(), t, helper_ms: 1.0, total_ms: 1.0 };
        assert!(ed.frame_back(first.seq, Ok(fake(0.0))).is_some());
        ed.want(960);
        assert!(rx.try_iter().next().is_none(), "the same frame is not asked for twice");
        assert!(ed.apply(EditKind::Style, 1.0, |p| set_style(p, StyleField::Padding(0.3))));
        let cmds: Vec<WorkerCmd> = rx.try_iter().collect();
        assert!(matches!(&cmds[0], WorkerCmd::Project(p) if p.style.padding == 0.3), "the edited project first");
        ed.want(960);
        let again: Vec<Req> = rx.try_iter().filter_map(|c| if let WorkerCmd::Frame(r) = c { Some(r) } else { None }).collect();
        assert_eq!(again.len(), 1, "the frame at the playhead is rendered again after an edit");
        // an edit while a render is in flight: the stale reply is followed by a fresh request
        assert!(ed.apply(EditKind::Style, 3.0, |p| set_style(p, StyleField::Padding(0.4))));
        assert!(rx.try_iter().all(|c| matches!(c, WorkerCmd::Project(_))), "coalesced: nothing new while in flight");
        ed.frame_back(again[0].seq, Ok(fake(0.0)));
        let next: Vec<Req> = rx.try_iter().filter_map(|c| if let WorkerCmd::Frame(r) = c { Some(r) } else { None }).collect();
        assert_eq!(next.len(), 1, "the frame that predates the edit is replaced");
    }

    #[test]
    fn autosave_writes_one_idle_second_after_the_last_change_and_only_then() {
        let dir = temp("autosave");
        let (mut ed, _rx) = editor(&dir, false);
        let before = std::fs::read(dir.join("project.json")).unwrap();
        ed.apply(EditKind::Style, 100.0, |p| set_style(p, StyleField::Radius(30.0)));
        assert_eq!(ed.save_label(), "Saving…");
        ed.autosave_tick(100.5);
        assert_eq!(std::fs::read(dir.join("project.json")).unwrap(), before, "no write before 1 s idle");
        ed.apply(EditKind::Style, 100.9, |p| set_style(p, StyleField::Radius(31.0)));
        ed.autosave_tick(101.5);
        assert_eq!(std::fs::read(dir.join("project.json")).unwrap(), before, "the second change moved the deadline");
        ed.autosave_tick(101.9);
        assert_eq!(disk(&dir).style.radius, 31.0, "written 1 s after the LAST change");
        assert!(disk(&dir).validate(|_| true).is_empty());
        assert_eq!(ed.save_label(), "Saved");
        assert!(!dir.join("project.json.tmp").exists(), "atomic: no temp file left");
        // one write per burst: a sentinel survives later ticks
        std::fs::write(dir.join("project.json"), b"sentinel").unwrap();
        ed.autosave_tick(110.0);
        ed.autosave_tick(120.0);
        assert_eq!(std::fs::read(dir.join("project.json")).unwrap(), b"sentinel");
    }

    #[test]
    fn opening_a_project_with_auto_zoom_writes_nothing_until_the_user_changes_something() {
        let dir = temp("openonly");
        let (mut ed, _rx) = editor(&dir, true);
        base().save(&dir).unwrap(); // on disk: the recording as it was made, without zooms
        let before = std::fs::read(dir.join("project.json")).unwrap();
        assert!(ed.dirty && !ed.project.as_ref().unwrap().zooms.is_empty(), "the auto zooms are in memory");
        for t in [1.0, 5.0, 60.0] {
            ed.autosave_tick(t);
        }
        ed.flush(61.0);
        assert_eq!(std::fs::read(dir.join("project.json")).unwrap(), before, "a project the user only opened is not rewritten");
        assert_eq!(ed.save_label(), "Saved");
        ed.apply(EditKind::Cursor, 70.0, |p| set_cursor(p, CursorField::Size(2.0)));
        ed.autosave_tick(71.0);
        let saved = disk(&dir);
        assert_eq!(saved.cursor.size, 2.0);
        assert_eq!(saved.zooms.len(), 1, "after the first change the auto zooms are persisted too");
        assert!(!ed.dirty);
    }

    #[test]
    fn a_failed_write_keeps_the_project_dirty_with_the_error_and_recovers() {
        let dir = temp("failwrite");
        let (mut ed, _rx) = editor(&dir, false);
        // the folder vanishes (an external drive was unplugged)
        let moved = dir.with_file_name("Gone.ferrocast");
        std::fs::rename(&dir, &moved).unwrap();
        ed.apply(EditKind::Cursor, 10.0, |p| set_cursor(p, CursorField::Size(3.0)));
        ed.autosave_tick(11.0);
        assert!(ed.save_label().starts_with("Could not save project.json"), "{}", ed.save_label());
        assert!(ed.dirty && ed.st.autosave.pending());
        ed.autosave_tick(11.5);
        assert!(ed.save_label().starts_with("Could not save"), "no flood: still the same error");
        std::fs::rename(&moved, &dir).unwrap();
        ed.autosave_tick(12.0);
        assert_eq!(ed.save_label(), "Saved");
        assert_eq!(disk(&dir).cursor.size, 3.0);
    }

    #[test]
    fn flush_writes_at_once_and_is_a_no_op_when_clean() {
        let dir = temp("flush");
        let (mut ed, _rx) = editor(&dir, false);
        ed.apply(EditKind::Style, 1.0, |p| set_style(p, StyleField::Radius(9.0)));
        ed.flush(1.1);
        assert_eq!(disk(&dir).style.radius, 9.0, "Esc does not wait for the second");
        std::fs::write(dir.join("project.json"), b"sentinel").unwrap();
        ed.flush(2.0);
        assert_eq!(std::fs::read(dir.join("project.json")).unwrap(), b"sentinel");
    }

    #[test]
    fn undo_is_saved_too_and_edits_stay_valid_for_the_core_decoder() {
        let dir = temp("undosave");
        let (mut ed, _rx) = editor(&dir, false);
        ed.apply(EditKind::Style, 1.0, |p| set_style(p, StyleField::Radius(9.0)));
        ed.flush(2.0);
        assert!(ed.undo(3.0));
        ed.autosave_tick(4.0);
        assert_eq!(disk(&dir).style.radius, Style::default().radius);
        assert!(!ed.undo(5.0), "nothing more to undo");
    }

    /// End to end against the REAL helper on the synthetic fixture (a copy under `target/capture-spike/t27-e2e`):
    /// a style edit changes the next rendered frame, `project.json` changes on disk only after the debounce,
    /// still decodes and validates, and Undo brings the first frame back exactly.
    /// Run: `cargo test -p ferrocast --locked live_helper_style -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs target/capture-spike/ferrocast-capture and the editor fixture (scripts/capture-smoke.sh --keep)"]
    fn live_helper_style_edit_changes_the_next_frame_and_autosaves() {
        use crate::editor_ctl::{helper_renderer, EditorMsg, FrameOut};
        use std::time::Duration;
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/capture-spike");
        let fixture = root.join("editor-fixture/Take.ferrocast");
        if !root.join("ferrocast-capture").is_file() || !fixture.join("screen.mov").is_file() {
            eprintln!("SKIP: target/capture-spike/ferrocast-capture or the editor fixture is missing");
            return;
        }
        let dir = root.join("t27-e2e/Take.ferrocast");
        std::fs::create_dir_all(&dir).unwrap();
        for f in ["project.json", "events.json", "screen.mov"] {
            std::fs::copy(fixture.join(f), dir.join(f)).unwrap();
        }
        let on_disk = || std::fs::read(dir.join("project.json")).unwrap();
        let original = on_disk();
        let (mtx, mrx) = channel();
        let mtx = std::sync::Mutex::new(mtx);
        let tx = crate::editor_ctl::spawn_worker(
            dir.clone(),
            Some(ZoomOpts::default()),
            helper_renderer(root.join("ferrocast"), |l| eprintln!("helper: {l}")),
            Box::new(move |m| {
                let _ = mtx.lock().unwrap().send(m);
            }),
        );
        let wait = || mrx.recv_timeout(Duration::from_secs(30)).expect("the worker answers");
        let opened = match wait() {
            EditorMsg::Opened { result, .. } => *result.expect("the fixture opens"),
            _ => panic!("Opened comes first"),
        };
        assert!(opened.auto_zoomed && opened.events.is_some());
        let mut ed = Editor::new(dir.clone(), Some(tx));
        ed.opened(Ok(opened));
        let frame = |ed: &mut Editor, t: f64| -> FrameOut {
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
        let cold = frame(&mut ed, 0.5);
        let differing = |a: &FrameOut, b: &FrameOut| a.image.pixels.iter().zip(&b.image.pixels).filter(|(x, y)| x != y).count();
        eprintln!("frame 1 (before the edit): {:?} px, helper {:.0} ms", cold.image.size, cold.helper_ms);

        // the edit: Dusk wallpaper + 20 % padding (two edits of different kinds: two undo steps)
        let t0 = ed.now();
        assert!(ed.apply(EditKind::Other, t0, |p| set_style(p, StyleField::Wallpaper(1))));
        assert!(ed.apply(EditKind::Style, t0 + 0.1, |p| set_style(p, StyleField::Padding(0.2))));
        let edited = frame(&mut ed, 0.5);
        let n = differing(&cold, &edited);
        eprintln!(
            "frame 2 (after the edit): {:?} px, helper {:.0} ms, {n} of {} pixels differ",
            edited.image.size,
            edited.helper_ms,
            cold.image.pixels.len()
        );
        assert!(n > cold.image.pixels.len() / 10, "the style edit must change the rendered frame: {n} pixels differ");
        assert_eq!(on_disk(), original, "nothing is written yet: the debounce has not elapsed");
        eprintln!("project.json right after the edit: unchanged ({} bytes)", original.len());

        // the debounce: nothing at +0.5 s, written after one idle second
        std::thread::sleep(Duration::from_millis(500));
        ed.autosave_tick(ed.now());
        assert_eq!(on_disk(), original, "still inside the idle second");
        std::thread::sleep(Duration::from_millis(700));
        ed.autosave_tick(ed.now());
        let saved = on_disk();
        assert_ne!(saved, original, "project.json changed after the debounce");
        let p = Project::from_json(&String::from_utf8(saved.clone()).unwrap()).expect("still decodes with the core model");
        let problems = p.validate(|f| dir.join(f).is_file());
        eprintln!(
            "project.json after the debounce: {} -> {} bytes; background {:?} angle {}, padding {}, zooms {}; validate() = {problems:?}; timeline::validate() = {:?}",
            original.len(),
            saved.len(),
            p.style.background.stops,
            p.style.background.angle,
            p.style.padding,
            p.zooms.len(),
            timeline::validate(&p)
        );
        assert!(problems.is_empty() && timeline::validate(&p).is_empty());
        assert_eq!((p.style.background.stops[0].as_str(), p.style.padding), ("#2b1055", 0.2));
        assert!(!p.zooms.is_empty(), "the auto zooms are persisted together with the user's first change");
        assert!(!dir.join("project.json.tmp").exists(), "atomic write left no temp file");
        assert_eq!(ed.save_label(), "Saved");

        // undo both edits: the first frame comes back exactly
        let t1 = ed.now();
        assert!(ed.undo(t1) && ed.undo(t1 + 0.1));
        let back = frame(&mut ed, 0.5);
        eprintln!("frame 3 (after Undo x2): {} of {} pixels differ from frame 1", differing(&cold, &back), cold.image.pixels.len());
        assert_eq!(cold.image.pixels, back.image.pixels, "undo restores the first frame");
        assert!(ed.st.autosave.pending(), "the undo is owed to disk");
        ed.flush(ed.now());
        assert_eq!(Project::load(&dir).unwrap().style, Project::from_json(&String::from_utf8(original).unwrap()).unwrap().style);
        eprintln!("after Undo x2 + flush: project.json style equals the original again");
    }

    // ---- P4: camera and audio panels

    fn with_sources() -> Project {
        use ferrocast_core::capture::camera::CameraLayout;
        use ferrocast_core::capture::project::{Source, TrackMix};
        let mut p = base();
        p.sources.camera = Some(Source { file: "camera.mov".into(), offset: 0.1 });
        p.sources.mic = Some(Source { file: "mic.m4a".into(), offset: 0.2 });
        p.sources.system = Some(Source { file: "system.m4a".into(), offset: 0.0 });
        p.camera = Some(CameraLayout::default());
        p.audio.mic = Some(TrackMix::default());
        p.audio.system = Some(TrackMix::default());
        p
    }

    #[test]
    fn the_camera_and_audio_tabs_exist_only_for_projects_with_those_sources() {
        use ferrocast_core::capture::project::Source;
        let plain = [Tab::Zoom, Tab::Cursor, Tab::Style, Tab::Clips];
        assert_eq!(visible_tabs(&base()), plain, "no camera and no audio: neither tab");
        let mut cam = base();
        cam.sources.camera = Some(Source { file: "camera.mov".into(), offset: 0.0 });
        assert_eq!(visible_tabs(&cam), [Tab::Zoom, Tab::Cursor, Tab::Style, Tab::Clips, Tab::Camera]);
        let mut mic = base();
        mic.sources.mic = Some(Source { file: "mic.m4a".into(), offset: 0.0 });
        assert_eq!(visible_tabs(&mic), [Tab::Zoom, Tab::Cursor, Tab::Style, Tab::Clips, Tab::Audio, Tab::Transcript]);
        let mut sys = base();
        sys.sources.system = Some(Source { file: "system.m4a".into(), offset: 0.0 });
        assert!(visible_tabs(&sys).contains(&Tab::Audio) && !visible_tabs(&sys).contains(&Tab::Camera));
        assert_eq!(
            visible_tabs(&with_sources()),
            [Tab::Zoom, Tab::Cursor, Tab::Style, Tab::Clips, Tab::Camera, Tab::Audio, Tab::Transcript]
        );
        // a stored tab the project no longer offers falls back to the first one
        assert_eq!(Tab::Camera.or_first(&base()), Tab::Zoom);
        assert_eq!(Tab::Camera.or_first(&with_sources()), Tab::Camera);
        assert_eq!(Tab::Clips.or_first(&base()), Tab::Clips);
    }

    #[test]
    fn camera_edits_map_to_the_layout_and_clamp() {
        use ferrocast_core::capture::camera::{CameraShape, Corner, MARGIN_MAX, SIZE_MAX, SIZE_MIN};
        let mut p = with_sources();
        set_camera(&mut p, CameraField::Shape(CameraShape::Square)).unwrap();
        set_camera(&mut p, CameraField::Corner(Corner::TopLeft)).unwrap();
        set_camera(&mut p, CameraField::Mirror(false)).unwrap();
        set_camera(&mut p, CameraField::Visible(false)).unwrap();
        set_camera(&mut p, CameraField::Size(0.3)).unwrap();
        set_camera(&mut p, CameraField::Margin(0.05)).unwrap();
        let c = p.camera.unwrap();
        assert_eq!(
            (c.shape, c.corner, c.mirror, c.visible, c.size, c.margin),
            (CameraShape::Square, Corner::TopLeft, false, false, 0.3, 0.05)
        );
        set_camera(&mut p, CameraField::Size(0.9)).unwrap();
        assert_eq!(p.camera.unwrap().size, SIZE_MAX);
        set_camera(&mut p, CameraField::Size(0.0)).unwrap();
        assert_eq!(p.camera.unwrap().size, SIZE_MIN);
        set_camera(&mut p, CameraField::Margin(5.0)).unwrap();
        assert_eq!(p.camera.unwrap().margin, MARGIN_MAX);
        set_camera(&mut p, CameraField::Margin(-1.0)).unwrap();
        assert_eq!(p.camera.unwrap().margin, 0.0);
        assert!(set_camera(&mut p, CameraField::Size(f64::NAN)).is_err());
        assert!(set_camera(&mut p, CameraField::Margin(f64::INFINITY)).is_err());
        assert!(p.validate(|_| true).is_empty(), "everything the panel writes validates");
        // a camera source without a stored layout gets the default one first
        let mut no_layout = with_sources();
        no_layout.camera = None;
        set_camera(&mut no_layout, CameraField::Mirror(false)).unwrap();
        assert!(!no_layout.camera.unwrap().mirror);
        // no camera source: nothing to edit
        assert!(set_camera(&mut base(), CameraField::Visible(false)).unwrap_err().contains("no camera"));
    }

    #[test]
    fn audio_edits_map_to_the_mix_and_clamp() {
        use ferrocast_core::capture::project::AudioTrack;
        let mut p = with_sources();
        set_audio(&mut p, AudioField::Gain(AudioTrack::Mic, 6.5)).unwrap();
        set_audio(&mut p, AudioField::Mute(AudioTrack::System, true)).unwrap();
        set_audio(&mut p, AudioField::Master(-3.0)).unwrap();
        assert_eq!((p.audio.mic.unwrap().gain_db, p.audio.system.unwrap().mute, p.audio.master_db), (6.5, true, -3.0));
        set_audio(&mut p, AudioField::Gain(AudioTrack::Mic, 40.0)).unwrap();
        assert_eq!(p.audio.mic.unwrap().gain_db, 12.0);
        set_audio(&mut p, AudioField::Gain(AudioTrack::System, -300.0)).unwrap();
        assert_eq!(p.audio.system.unwrap().gain_db, -60.0);
        set_audio(&mut p, AudioField::Master(99.0)).unwrap();
        assert_eq!(p.audio.master_db, 12.0);
        assert!(set_audio(&mut p, AudioField::Gain(AudioTrack::Mic, f64::NAN)).is_err());
        assert!(set_audio(&mut p, AudioField::Master(f64::NEG_INFINITY)).is_err());
        assert!(p.validate(|_| true).is_empty());
        // a track that was not recorded cannot be edited
        let mut only_mic = with_sources();
        only_mic.sources.system = None;
        assert!(set_audio(&mut only_mic, AudioField::Mute(AudioTrack::System, true)).unwrap_err().contains("system audio"));
        // a project with the legacy linear gain shows it in dB and keeps it when only muted
        let mut legacy = with_sources();
        legacy.audio.system = None;
        assert!((track_mix(&legacy, AudioTrack::System).gain_db - 20.0 * 0.6f64.log10()).abs() < 1e-9);
        set_audio(&mut legacy, AudioField::Mute(AudioTrack::System, true)).unwrap();
        let m = legacy.audio.system.unwrap();
        assert!(m.mute && (m.gain_db - 20.0 * 0.6f64.log10()).abs() < 1e-9);
    }

    #[test]
    fn a_gain_drag_is_one_undo_step_and_autosave_writes_the_fields() {
        use ferrocast_core::capture::project::AudioTrack;
        let dir = temp("p4-undo");
        let (tx, _rx) = channel();
        let mut ed = Editor::new(dir.clone(), Some(tx));
        let p = with_sources();
        p.save(&dir).unwrap();
        ed.opened(Ok(Opened { project: p.clone(), auto_zoomed: false, note: None, events: None }));
        for (i, v) in [-1.0, -2.0, -3.5].into_iter().enumerate() {
            assert!(ed.apply(EditKind::Audio, 10.0 + i as f64 * 0.1, |p| set_audio(p, AudioField::Gain(AudioTrack::Mic, v))));
        }
        for (i, v) in [0.25, 0.3].into_iter().enumerate() {
            assert!(ed.apply(EditKind::Style, 20.0 + i as f64 * 0.1, |p| set_camera(p, CameraField::Size(v))));
        }
        assert!(
            ed.apply(EditKind::Other, 30.0, |p| set_camera(p, CameraField::Shape(ferrocast_core::capture::camera::CameraShape::Square)))
        );
        ed.flush(31.0);
        let saved = disk(&dir);
        assert_eq!(saved.audio.mic.unwrap().gain_db, -3.5);
        assert_eq!(saved.camera.unwrap().size, 0.3);
        assert!(ed.undo(32.0), "the shape");
        assert!(ed.undo(32.0), "both size drags");
        assert_eq!(ed.project.as_ref().unwrap().camera.unwrap().size, 0.20);
        assert!(ed.undo(32.0), "the three gain drags");
        assert_eq!(ed.project.as_ref().unwrap().audio.mic.unwrap().gain_db, 0.0);
        assert!(!ed.undo(32.0), "nothing left: one step per drag");
        // an invalid value is refused with a message and leaves the project alone
        let before = ed.project.clone();
        assert!(!ed.apply(EditKind::Audio, 40.0, |p| set_audio(p, AudioField::Master(f64::NAN))));
        assert!(ed.st.edit_error.is_some() && ed.project == before);
    }

    #[test]
    fn source_infos_report_name_offset_and_missing_files() {
        let dir = temp("p4-info");
        std::fs::write(dir.join("mic.m4a"), vec![0u8; 2048]).unwrap();
        let p = with_sources();
        let infos = source_infos(&p, &dir);
        assert_eq!(infos.iter().map(|i| i.label).collect::<Vec<_>>(), ["Camera", "Microphone", "System audio"]);
        assert!(infos[0].missing && infos[2].missing && !infos[1].missing);
        assert!(infos[0].text().contains("camera.mov") && infos[0].text().contains("missing"), "{}", infos[0].text());
        assert!(
            infos[1].text().contains("mic.m4a") && infos[1].text().contains("2 KB") && infos[1].text().contains("+0.20 s"),
            "{}",
            infos[1].text()
        );
        assert!(source_infos(&base(), &dir).is_empty());
    }

    // ---- P5: the Transcript tab (pure logic, then the editor wiring)

    use ferrocast_core::capture::transcript::{Transcript, Word};

    fn words(list: &[(&str, f64, f64)]) -> Transcript {
        Transcript {
            language: "en-US".into(),
            words: list.iter().map(|&(t, s, e)| Word { text: t.into(), start: s, end: e, confidence: 0.95 }).collect(),
            ..Default::default()
        }
    }

    /// "so um this is a test ... done": a filler at 1.0 and a 2.1 s pause before the last word.
    fn speech() -> Transcript {
        words(&[
            ("so", 0.5, 0.8),
            ("um", 1.0, 1.3),
            ("this", 1.5, 1.8),
            ("is", 1.9, 2.1),
            ("a", 2.2, 2.3),
            ("test", 2.4, 2.9),
            ("done", 5.0, 5.4),
        ])
    }

    fn talking() -> Project {
        let mut p = with_sources();
        p.transcript = Some(speech());
        p
    }

    #[test]
    fn the_transcript_tab_needs_audio_or_an_existing_transcript() {
        use ferrocast_core::capture::project::Source;
        assert!(!visible_tabs(&base()).contains(&Tab::Transcript), "no audio, no transcript: no tab");
        let mut mic = base();
        mic.sources.mic = Some(Source { file: "mic.m4a".into(), offset: 0.0 });
        assert!(visible_tabs(&mic).contains(&Tab::Transcript));
        let mut sys = base();
        sys.sources.system = Some(Source { file: "system.m4a".into(), offset: 0.0 });
        assert!(visible_tabs(&sys).contains(&Tab::Transcript));
        let mut had = base();
        had.transcript = Some(speech());
        assert!(visible_tabs(&had).contains(&Tab::Transcript), "a transcript alone keeps the tab");
        assert_eq!(Tab::Transcript.label(), "Transcript");
        assert_eq!(Tab::Transcript.or_first(&base()), Tab::Zoom);
    }

    #[test]
    fn the_transcribe_button_state_follows_the_project() {
        use ferrocast_core::capture::project::Source;
        assert_eq!(audio_for_transcription(&base()), None);
        assert!(!can_transcribe(&base()));
        let mut sys = base();
        sys.sources.system = Some(Source { file: "system.m4a".into(), offset: 0.0 });
        assert_eq!(audio_for_transcription(&sys), Some("system.m4a"));
        assert_eq!(audio_for_transcription(&with_sources()), Some("mic.m4a"), "the microphone wins");
        assert!(can_transcribe(&sys));
        assert_eq!(transcript_view(&base()), TranscriptView::Idle);
        assert_eq!(transcript_view(&talking()), TranscriptView::Words);
        let mut empty = with_sources();
        empty.transcript = Some(words(&[]));
        assert_eq!(transcript_view(&empty), TranscriptView::Idle, "a transcript without words offers to transcribe again");
        assert_eq!(locale_arg(0), None, "the first entry is the system language");
        assert!(LOCALES.len() > 4 && locale_arg(1).is_some());
    }

    #[test]
    fn helper_errors_become_clear_messages() {
        use ferrocast_macos::capture::{transcription_unavailable_message, TranscribeError};
        let perm = map_transcribe_error(&TranscribeError::PermissionSpeech("Speech Recognition is not authorized".into()));
        assert_eq!(perm, TranscribeFailure::Permission);
        assert_eq!(
            perm.text(),
            "Speech Recognition permission is needed. Open System Settings > Privacy & Security > Speech Recognition and allow Ferrocast."
        );
        let m = transcription_unavailable_message("Spanish (Spain)");
        assert_eq!(map_transcribe_error(&TranscribeError::Unavailable(m.clone())).text(), m, "the helper's sentence as is");
        assert_eq!(map_transcribe_error(&TranscribeError::Timeout), TranscribeFailure::Message(TranscribeError::Timeout.to_string()));
        assert!(map_transcribe_error(&TranscribeError::Other("boom".into())).text().contains("boom"));
        assert_eq!(crate::capture_ctl::SPEECH_PANE, "x-apple.systempreferences:com.apple.preference.security?Privacy_SpeechRecognition");
    }

    #[test]
    fn the_word_under_the_playhead_and_the_seek_target() {
        let t = speech();
        assert_eq!(word_at(&t, 0.6), Some(0));
        assert_eq!(word_at(&t, 1.0), Some(1), "a word starts inclusive");
        assert_eq!(word_at(&t, 0.9), None, "a gap belongs to no word");
        assert_eq!(word_at(&t, 99.0), None);
        assert_eq!(word_at(&t, f64::NAN), None);
        // a cut from 1.0 to 1.4 (the "um"): a click on it lands on the nearest kept time
        let mut p = talking();
        assert!(timeline::remove_range(&mut p, 1.0, 1.4));
        let tl = Timeline::new(&p);
        assert_eq!(nearest_kept(&tl, 0.6), 0.6, "kept time stays");
        assert_eq!(nearest_kept(&tl, 1.1), 1.0, "closer to the end of the previous piece");
        assert_eq!(nearest_kept(&tl, 1.3), 1.4, "closer to the start of the next piece");
        assert!(word_is_cut(&tl, &speech().words[1]) && !word_is_cut(&tl, &speech().words[0]));
        // a word half inside a cut by its middle is cut
        let flags: Vec<bool> = speech().words.iter().map(|w| word_is_cut(&tl, w)).collect();
        assert_eq!(flags, [false, true, false, false, false, false, false]);
    }

    #[test]
    fn a_selection_is_one_range_and_one_cut() {
        let t = speech();
        assert_eq!(selection_bounds(None, 7), None);
        assert_eq!(selection_bounds(Some((3, 1)), 7), Some((1, 3)), "dragging backwards");
        assert_eq!(selection_bounds(Some((5, 40)), 7), Some((5, 6)), "clamped to the words");
        assert_eq!(selection_bounds(Some((8, 9)), 7), None);
        assert_eq!(selection_bounds(Some((0, 0)), 0), None);
        let (a, b) = selection_cut(&t, Some((3, 1))).unwrap();
        let want = ferrocast_core::capture::transcript::word_range_cuts(&t, 1, 3).unwrap();
        assert_eq!((a, b), want, "exactly the core cut for the words");
        assert!(a < 1.0 && b > 2.1, "{a} {b}");
        assert_eq!(selection_cut(&t, None), None);
    }

    #[test]
    fn filler_words_are_flagged_like_the_button_would_cut_them() {
        let flags = filler_flags(&speech(), 10.0);
        assert_eq!(flags, [false, true, false, false, false, false, false]);
        assert_eq!(filler_flags(&words(&[]), 10.0), Vec::<bool>::new());
    }

    fn talking_editor(tag: &str) -> (Editor, Receiver<WorkerCmd>, PathBuf) {
        let dir = temp(tag);
        let (tx, rx) = channel();
        let mut ed = Editor::new(dir.clone(), Some(tx));
        let p = talking();
        p.save(&dir).unwrap();
        ed.opened(Ok(Opened { project: p, auto_zoomed: false, note: None, events: None }));
        (ed, rx, dir)
    }

    fn out_len(ed: &Editor) -> f64 {
        ed.timeline.as_ref().unwrap().output_duration()
    }

    #[test]
    fn each_cut_action_is_one_undo_step_with_a_report() {
        let (mut ed, rx, dir) = talking_editor("tr-cuts");
        let start = ed.project.as_ref().unwrap().to_json();
        ed.transcript_cut(CutAction::Fillers, 10.0);
        assert_eq!(ed.st.history.undo_len(), 1);
        let notice = ed.st.tr.notice.clone().unwrap();
        assert!(notice.starts_with("Removed 1 cut") && notice.contains("0.3 s"), "{notice}");
        assert!((out_len(&ed) - 9.64).abs() < 0.2, "{}", out_len(&ed));
        assert!(!project_cmds(&rx).is_empty(), "the worker got the edited project");
        // nothing left to remove: a message, no new undo step
        ed.transcript_cut(CutAction::Fillers, 20.0);
        assert_eq!(ed.st.history.undo_len(), 1);
        assert_eq!(ed.st.tr.notice.as_deref(), Some("No filler words found."));
        // the 2.1 s pause
        let before = out_len(&ed);
        ed.transcript_cut(CutAction::Pauses, 30.0);
        assert_eq!(ed.st.history.undo_len(), 2);
        assert!(before - out_len(&ed) > 1.5, "{} -> {}", before, out_len(&ed));
        assert!(ed.st.tr.notice.as_deref().unwrap().starts_with("Removed "));
        ed.transcript_cut(CutAction::Pauses, 40.0);
        assert_eq!(ed.st.tr.notice.as_deref(), Some("No long pauses to tighten."));
        assert_eq!(ed.st.history.undo_len(), 2);
        // a selection
        ed.st.tr.sel = Some((2, 4));
        ed.transcript_cut(CutAction::Selection, 50.0);
        assert_eq!(ed.st.history.undo_len(), 3);
        assert_eq!(ed.st.tr.sel, None, "the selection is consumed");
        let tl = ed.timeline.clone().unwrap();
        let cut: Vec<bool> = speech().words.iter().map(|w| word_is_cut(&tl, w)).collect();
        assert_eq!(cut, [false, true, true, true, true, false, false]);
        // the same selection again: already cut
        ed.st.tr.sel = Some((2, 4));
        ed.transcript_cut(CutAction::Selection, 60.0);
        assert_eq!(ed.st.history.undo_len(), 3);
        assert_eq!(ed.st.tr.notice.as_deref(), Some("That selection is already cut."));
        // autosave writes the cuts, the transcript stays
        ed.flush(61.0);
        assert!(disk(&dir).transcript.is_some());
        assert_eq!(disk(&dir).trims, ed.project.as_ref().unwrap().trims);
        for _ in 0..3 {
            assert!(ed.undo(70.0));
        }
        assert_eq!(ed.project.as_ref().unwrap().to_json(), start, "three undos restore the project byte for byte");
    }

    #[test]
    fn a_cut_that_would_delete_everything_is_refused_with_its_message() {
        let (mut ed, _rx, _dir) = talking_editor("tr-all");
        ed.st.tr.sel = Some((0, 6));
        ed.transcript_cut(CutAction::Selection, 10.0);
        // the selection ends at the last word, the take goes on to 10 s: something is left, so this is a normal cut
        assert_eq!(ed.st.history.undo_len(), 1);
        let mut p = talking();
        p.duration = 5.4;
        p.transcript = Some(words(&[("only", 0.0, 5.4)]));
        let (tx, _rx2) = channel();
        let mut ed2 = Editor::new(temp("tr-all2"), Some(tx));
        ed2.opened(Ok(Opened { project: p, auto_zoomed: false, note: None, events: None }));
        ed2.st.tr.sel = Some((0, 0));
        ed2.transcript_cut(CutAction::Selection, 10.0);
        assert_eq!(ed2.st.history.undo_len(), 0);
        assert!(ed2.st.edit_error.as_deref().unwrap().contains("delete everything"), "{:?}", ed2.st.edit_error);
    }

    #[test]
    fn a_finished_transcription_is_stored_through_the_history_and_autosaved() {
        let dir = temp("tr-done");
        let (tx, rx) = channel();
        let mut ed = Editor::new(dir.clone(), Some(tx));
        let p = with_sources();
        p.save(&dir).unwrap();
        ed.opened(Ok(Opened { project: p, auto_zoomed: false, note: None, events: None }));
        ed.st.tr.run = Some(TranscribeRun::new());
        ed.transcribe_msg(TranscribeMsg::Progress { done: 1.0, total: 4.0 }, 1.0);
        assert_eq!(ed.st.tr.run.as_ref().map(|r| (r.done, r.total)), Some((1.0, 4.0)));
        ed.transcribe_msg(TranscribeMsg::Done(Box::new(speech())), 2.0);
        assert!(ed.st.tr.run.is_none() && ed.st.tr.error.is_none());
        assert_eq!(ed.project.as_ref().unwrap().transcript, Some(speech()));
        assert_eq!(ed.st.history.undo_len(), 1);
        assert!(ed.st.autosave.pending());
        assert!(!project_cmds(&rx).is_empty());
        ed.flush(3.0);
        assert_eq!(disk(&dir).transcript, Some(speech()), "autosave writes `transcript`");
        assert!(ed.undo(4.0));
        assert!(ed.project.as_ref().unwrap().transcript.is_none(), "one undo removes it again");
        ed.transcribe_msg(TranscribeMsg::Failed(TranscribeFailure::Message("nope".into())), 5.0);
        assert_eq!(ed.st.tr.error, Some(TranscribeFailure::Message("nope".into())));
        ed.transcribe_msg(TranscribeMsg::Cancelled, 6.0);
        assert!(ed.st.tr.run.is_none() && ed.st.tr.error.is_none(), "a cancel is not an error");
    }

    #[test]
    fn answering_the_permission_prompt_starts_the_transcription_unless_cancelled() {
        let (mut ed, _rx, _dir) = talking_editor("tr-asked");
        ed.st.tr.run = Some(TranscribeRun::new());
        ed.transcribe_msg(TranscribeMsg::PermissionAsked, 1.0);
        assert!(ed.st.tr.run.is_none() && ed.st.tr.start, "the app may now start the helper");
        ed.st.tr.start = false;
        ed.st.tr.run = Some(TranscribeRun::new());
        ed.cancel_transcribe();
        ed.transcribe_msg(TranscribeMsg::PermissionAsked, 2.0);
        assert!(ed.st.tr.run.is_none() && !ed.st.tr.start, "a cancel during the prompt starts nothing");
    }

    #[test]
    fn an_existing_transcript_file_is_loaded_when_the_project_has_none() {
        let dir = temp("tr-file");
        std::fs::write(dir.join("transcript.json"), serde_json::to_string(&speech()).unwrap()).unwrap();
        let (tx, _rx) = channel();
        let mut ed = Editor::new(dir.clone(), Some(tx));
        let p = with_sources();
        p.save(&dir).unwrap();
        ed.opened(Ok(Opened { project: p, auto_zoomed: false, note: None, events: None }));
        assert_eq!(ed.project.as_ref().unwrap().transcript, Some(speech()));
        assert!(ed.st.autosave.pending(), "the adopted transcript is saved into project.json");
        assert!(!ed.st.history.can_undo(), "loading is not an undo step");
        // a broken file is ignored
        let dir2 = temp("tr-file-bad");
        std::fs::write(dir2.join("transcript.json"), "{nope").unwrap();
        let (tx, _rx) = channel();
        let mut ed2 = Editor::new(dir2.clone(), Some(tx));
        ed2.opened(Ok(Opened { project: with_sources(), auto_zoomed: false, note: None, events: None }));
        assert!(ed2.project.as_ref().unwrap().transcript.is_none());
        // a project that already has one keeps it
        let mut ed3 = Editor::new(dir.clone(), None);
        let mut q = with_sources();
        q.transcript = Some(words(&[("x", 0.0, 0.1)]));
        ed3.opened(Ok(Opened { project: q.clone(), auto_zoomed: false, note: None, events: None }));
        assert_eq!(ed3.project.unwrap().transcript, q.transcript);
    }

    fn fake_helper(tag: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp(tag).parent().unwrap().to_path_buf();
        let p = dir.join("helper.sh");
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    fn run_fake(tag: &str, body: &str, cancel_now: bool) -> (Vec<TranscribeMsg>, std::thread::ThreadId) {
        let (tx, _rx) = channel();
        let dir = temp(tag);
        let mut ed = Editor::new(dir, Some(tx));
        ed.opened(Ok(Opened { project: with_sources(), auto_zoomed: false, note: None, events: None }));
        let (ntx, nrx) = channel();
        let threads = std::sync::Arc::new(std::sync::Mutex::new(None));
        let t2 = threads.clone();
        let notify: Box<dyn Fn(crate::editor_ctl::EditorMsg) + Send> = Box::new(move |m| {
            *t2.lock().unwrap() = Some(std::thread::current().id());
            let _ = ntx.send(m);
        });
        ed.start_transcribe(fake_helper(tag, body), |_| {}, notify).unwrap();
        assert!(ed.st.tr.run.is_some(), "running right away");
        assert!(ed.start_transcribe(PathBuf::from("/nonexistent"), |_| {}, Box::new(|_| {})).is_err(), "one at a time");
        if cancel_now {
            ed.cancel_transcribe();
            assert!(ed.st.tr.run.as_ref().unwrap().cancelling);
        }
        let mut out = vec![];
        while let Ok(crate::editor_ctl::EditorMsg::Transcribe { msg, .. }) = nrx.recv_timeout(std::time::Duration::from_secs(10)) {
            let end = !matches!(msg, TranscribeMsg::Progress { .. });
            out.push(msg);
            if end {
                break;
            }
        }
        let worker = threads.lock().unwrap().unwrap();
        (out, worker)
    }

    #[test]
    fn transcription_runs_on_a_worker_thread_and_reports_progress_then_the_transcript() {
        let dir = temp("tr-run");
        let body = format!(
            r#"read l
echo '{{"event":"progress","id":1,"done_secs":1.5,"total_secs":3.0}}'
printf '%s' '{}' > '{}/transcript.json'
echo '{{"id":1,"ok":true,"language":"en-US","words":7,"path":"{}/transcript.json"}}'"#,
            serde_json::to_string(&speech()).unwrap(),
            dir.display(),
            dir.display()
        );
        let (msgs, worker) = run_fake("tr-run", &body, false);
        assert_ne!(worker, std::thread::current().id(), "never the calling (main) thread");
        assert!(matches!(msgs[0], TranscribeMsg::Progress { done, total } if done == 1.5 && total == 3.0), "{msgs:?}");
        assert!(matches!(msgs.last(), Some(TranscribeMsg::Done(t)) if **t == speech()), "{msgs:?}");
    }

    #[test]
    fn permission_and_cancel_come_back_as_their_own_messages() {
        let (msgs, _) = run_fake("tr-perm", r#"read l; echo '{"id":1,"error":"permission-speech","message":"nope"}'"#, false);
        assert!(matches!(msgs.last(), Some(TranscribeMsg::Failed(TranscribeFailure::Permission))), "{msgs:?}");
        let (msgs, _) =
            run_fake("tr-cancel", r#"read l; read c; echo '{"id":1,"error":"cancelled","message":"Transcription cancelled"}'"#, true);
        assert!(matches!(msgs.last(), Some(TranscribeMsg::Cancelled)), "{msgs:?}");
        // a missing helper is a message, not a crash
        let (tx, _rx) = channel();
        let mut ed = Editor::new(temp("tr-missing"), Some(tx));
        ed.opened(Ok(Opened { project: with_sources(), auto_zoomed: false, note: None, events: None }));
        let (ntx, nrx) = channel();
        ed.start_transcribe(PathBuf::from("/nonexistent/helper"), |_| {}, Box::new(move |m| drop(ntx.send(m)))).unwrap();
        let Ok(crate::editor_ctl::EditorMsg::Transcribe { msg: TranscribeMsg::Failed(f), .. }) =
            nrx.recv_timeout(std::time::Duration::from_secs(10))
        else {
            panic!("expected a failure")
        };
        assert!(!f.text().is_empty());
        // no audio at all: refused before any thread starts
        let mut none = Editor::new(temp("tr-noaudio"), None);
        none.opened(Ok(Opened { project: base(), auto_zoomed: false, note: None, events: None }));
        assert!(none.start_transcribe(PathBuf::from("/x"), |_| {}, Box::new(|_| {})).is_err());
    }
}
