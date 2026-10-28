//! Launcher application: state, views, keyboard handling, actions, drawing.

use crate::instance;
use crate::items::*;
use crate::notion_io;
use crate::services::{self, now_secs, Bus, EffectKind, Hotkeys, Msg, Shared};
use crate::ui::{self, palette, IconCache, Palette};
use egui::{Align2, Color32, Context, FontId, Id, Key, Modifiers, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, UiBuilder, Vec2};
use ferrocast_core::calc;
use ferrocast_core::color::Rgba;
use ferrocast_core::i18n::{tr, trf, trn};
use ferrocast_core::models::*;
use ferrocast_core::placeholders::{self, Target};
use ferrocast_core::ranking::{self, Frecency, Rankable};
use ferrocast_core::script::{self, Mode, ScriptCommand};
use ferrocast_core::storage::Store;
use ferrocast_macos::{apps, clipboard, keyboard, running, system, windows};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct ArgDef {
    pub name: String,
    pub options: Vec<(String, String)>,
    pub optional: bool,
    pub password: bool,
}

#[derive(Clone, Debug)]
pub enum ArgTarget {
    Quicklink(String),
    Snippet(String),
    Script(usize),
}

/// A countdown started from Root Search.
pub struct RunningTimer {
    pub ends_at: Instant,
    pub secs: u64,
    pub label: String,
    pub cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

/// What the reorder window will do with the files once the order is chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReorderPurpose {
    MergePdf,
    CreatePdf,
    JoinVideos,
    MergeAudio,
}

/// The existing job a reorder window runs, with the files in the chosen order.
#[derive(Clone, Debug, PartialEq)]
pub enum ReorderJob {
    Tool(ferrocast_core::convert::FileTool, Vec<PathBuf>),
    Image(ferrocast_core::convert::ImageOp, Vec<PathBuf>),
}

/// Pure mapping from the split window's state to the job that Enter starts; refuses (with the
/// message to show) when the cuts leave fewer than two clips.
pub fn split_job(duration: f64, points: &[f64], parts: usize) -> Result<ferrocast_core::convert::FileTool, String> {
    let ranges = ferrocast_core::videoplan::split_ranges(duration, points, parts);
    if ranges.len() < 2 {
        return Err("Nothing to split: add a cut point (M) or set two or more equal parts".into());
    }
    Ok(ferrocast_core::convert::FileTool::SplitPoints(ranges, true))
}

/// One area of the video redaction window: a rectangle (displayed pixels) covered from `start`
/// to `end` seconds, solid black or blurred.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    pub rect: ferrocast_core::croprect::Rect,
    pub start: f64,
    pub end: f64,
    pub blur: bool,
}

/// Most areas the redaction window holds (the core's limit).
pub const MAX_REDACT_AREAS: usize = ferrocast_core::redactplan::MAX_AREAS;

/// The job for the redaction window, or the message that names what is wrong (the window stays open).
pub fn redact_job(areas: &[Area], duration: f64, size: (u32, u32)) -> Result<ferrocast_core::convert::FileTool, String> {
    let list: Vec<ferrocast_core::redactplan::Redaction> = areas
        .iter()
        .map(|a| {
            let (x, y, w, h) = ferrocast_core::croprect::tuple(a.rect);
            ferrocast_core::redactplan::Redaction { x, y, w, h, start: a.start, end: a.end, blur: a.blur }
        })
        .collect();
    ferrocast_core::redactplan::validate(&list, duration, size.0, size.1).map_err(|m| format!("Redact: {m}"))?;
    Ok(ferrocast_core::convert::FileTool::RedactVideoMany(list))
}

/// Blur radius range of the redaction window's slider, in pixels.
pub const REDACT_BLUR_RANGE: std::ops::RangeInclusive<u32> = 2..=80;
/// Radius the slider starts at (the old fixed value).
pub const REDACT_BLUR_DEFAULT: u32 = 24;

/// The job for the image redaction window, or the message that names what is wrong.
pub fn redact_job_image(
    areas: &[ferrocast_core::croprect::Rect],
    blur: bool,
    radius: u32,
    color: usize,
) -> Result<ferrocast_core::convert::ImageOp, String> {
    use ferrocast_core::redact::{Area, RedactSpec, Source, Style};
    if areas.is_empty() {
        return Err("Nothing to redact: drag a rectangle on the picture first".into());
    }
    let list = areas.iter().map(|r| Area { x: r.x, y: r.y, w: r.w, h: r.h }).collect();
    let style = if blur {
        Style::Blur(radius.clamp(*REDACT_BLUR_RANGE.start(), *REDACT_BLUR_RANGE.end()) as f32)
    } else {
        Style::Solid(ferrocast_core::annotate::COLORS[color.min(5)])
    };
    Ok(ferrocast_core::convert::ImageOp::RedactWith(RedactSpec { source: Source::Areas(list), style }))
}

/// The job for the background window, or the message that names what is wrong (the window stays
/// open). An image backdrop must be a readable picture.
pub fn frame_job(opts: &ferrocast_core::frame::FrameOpts) -> Result<ferrocast_core::convert::ImageOp, String> {
    if let ferrocast_core::frame::Backdrop::Image(path) = &opts.backdrop {
        if path.is_empty() {
            return Err("Background: choose a picture first".into());
        }
        let p = std::path::Path::new(path);
        if !p.is_file() {
            return Err(format!("Background: {path} was not found"));
        }
        if services::image_size(p).is_none() {
            return Err("Background: could not read that picture".into());
        }
    }
    Ok(ferrocast_core::convert::ImageOp::FrameWith(opts.clone()))
}

/// `m:ss` for a time in seconds (whole seconds, rounded down; 0:00 for unusable numbers).
/// The ZIP output for the "Zip" action. Built by hand because the planner does not offer ZIP for
/// known types; `ditto` zips any file or folder and keeps the original.
pub fn zip_target() -> ferrocast_core::convert::Target {
    ferrocast_core::convert::Target { ext: "zip", label: "ZIP", engine: ferrocast_core::convert::Engine::Archive, needs: None }
}

pub fn clock(secs: f64) -> String {
    let s = if secs.is_finite() && secs > 0.0 { secs as u64 } else { 0 };
    format!("{}:{:02}", s / 60, s % 60)
}

/// A new area: a centred third of the frame, active for the whole clip.
pub fn new_area(size: (u32, u32), duration: f64) -> Area {
    let rect = ferrocast_core::croprect::with_size(ferrocast_core::croprect::full(size.0, size.1), size.0 / 3, size.1 / 3, size.0, size.1);
    Area { rect, start: 0.0, end: duration.max(0.0), blur: false }
}

/// Most ranges the bleep window holds (the core's limit).
pub const MAX_BLEEP_RANGES: usize = ferrocast_core::media::MAX_BLEEPS;

/// Default new bleep range: one second centred in the clip, clamped to short clips.
pub fn new_bleep_range(duration: f64) -> (f64, f64) {
    let d = if duration.is_finite() { duration.max(0.0) } else { 0.0 };
    let len = d.min(1.0);
    let start = ((d - len) / 2.0 * 100.0).round() / 100.0;
    (start, (start + len).min(d))
}

/// The job for the bleep window, or the message that names what is wrong (the window stays open).
pub fn bleep_job(ranges: &[(f64, f64)], duration: f64) -> Result<ferrocast_core::convert::FileTool, String> {
    ferrocast_core::media::validate_bleeps(ranges, duration).map(ferrocast_core::convert::FileTool::BleepMany)
}

/// `0:01.5` style time with tenths, for the range list.
pub fn clock_tenths(secs: f64) -> String {
    let t = if secs.is_finite() && secs > 0.0 { (secs * 10.0).round() as u64 } else { 0 };
    format!("{}:{:02}.{}", t / 600, t / 10 % 60, t % 10)
}

/// Media mode of the Compress window: audio or video, and the probe result (read off the UI
/// thread: `loaded` is false until it arrives, `info` None when the file could not be read).
#[derive(Clone, Debug, PartialEq)]
pub struct MediaWin {
    pub video: bool,
    pub loaded: bool,
    pub info: Option<ferrocast_core::compress::MediaInfo>,
}

/// Largest size target the media window accepts, in KB (about 4 GB).
pub const MEDIA_MAX_KB: u32 = 4_000_000;

/// Pure mapping from the media window's fields to the job Enter starts: same fields as the image
/// window, a larger size range, resize for video only, and a refusal (with the reason) when the
/// target leaves too little bitrate or the file has not been read yet.
#[allow(clippy::too_many_arguments)]
pub fn compress_media_job(
    video: bool,
    orig_bytes: u64,
    info: Option<&ferrocast_core::compress::MediaInfo>,
    mode: usize,
    kb: u32,
    percent: u8,
    preset: usize,
    max_edge: Option<u32>,
    resize: bool,
) -> Result<ferrocast_core::convert::FileTool, String> {
    use ferrocast_core::compress::Goal;
    let big = mode == 0 && kb > 100_000 && kb <= MEDIA_MAX_KB;
    let (goal, r) = compress_goal(mode, if big { 100_000 } else { kb }, percent, preset, max_edge, resize && video)?;
    let goal = if big { Goal::TargetBytes(kb as u64 * 1024) } else { goal };
    let info = info.ok_or("Compress: still reading the file")?;
    ferrocast_core::compress::plan(video, orig_bytes, info, goal, &r).map_err(|e| format!("Compress: {e}"))?;
    Ok(ferrocast_core::convert::FileTool::Compress(goal, r))
}

/// Pure mapping from the Compress window's fields to the goal and resize, or the reason it cannot run.
/// `mode`: 0 size in KB (10..=100000), 1 percent smaller (1..=95), 2 preset (0 light, 1 balanced, 2 strong).
pub fn compress_goal(
    mode: usize,
    kb: u32,
    percent: u8,
    preset: usize,
    max_edge: Option<u32>,
    resize: bool,
) -> Result<(ferrocast_core::compress::Goal, ferrocast_core::compress::Resize), String> {
    use ferrocast_core::compress::{Goal, Preset, Resize};
    let goal = match mode {
        0 if (10..=100_000).contains(&kb) => Goal::TargetBytes(kb as u64 * 1024),
        0 => return Err("Compress: the target size must be between 10 and 100000 KB".into()),
        1 if (1..=95).contains(&percent) => Goal::Percent(percent),
        1 => return Err("Compress: the reduction must be between 1 and 95 %".into()),
        2 => Goal::Preset(match preset {
            0 => Preset::Light,
            1 => Preset::Balanced,
            2 => Preset::Strong,
            _ => return Err("Compress: choose a preset".into()),
        }),
        _ => return Err("Compress: choose a goal".into()),
    };
    let mut r = Resize::default();
    if resize {
        let e = max_edge.unwrap_or(1600);
        if !(64..=16_000).contains(&e) {
            return Err("Compress: the longest edge must be between 64 and 16000 px".into());
        }
        r.max_edge = Some(e);
    }
    Ok((goal, r))
}

/// Thumbnails were decoded for `loaded` (the order the collage had when the job started); the
/// pictures may have been dragged into another order since. Returns `items` rearranged to the
/// `now` order, or `None` when the file sets differ or an item is missing (a stale result).
pub fn align_to_current<T>(now: &[PathBuf], loaded: &[PathBuf], items: Vec<T>) -> Option<Vec<T>> {
    if now.len() != loaded.len() || items.len() != loaded.len() {
        return None;
    }
    let mut slots: Vec<Option<T>> = items.into_iter().map(Some).collect();
    let mut taken = vec![false; loaded.len()];
    let mut out = Vec::with_capacity(now.len());
    for f in now {
        let i = (0..loaded.len()).find(|&i| !taken[i] && loaded[i] == *f)?;
        taken[i] = true;
        out.push(slots[i].take()?);
    }
    Some(out)
}

/// Pure mapping from the window's purpose to the job that Enter starts.
pub fn reorder_job(purpose: ReorderPurpose, files: Vec<PathBuf>) -> ReorderJob {
    use ferrocast_core::convert::{FileTool, ImageOp};
    match purpose {
        ReorderPurpose::MergePdf | ReorderPurpose::MergeAudio => ReorderJob::Tool(FileTool::Merge, files),
        ReorderPurpose::JoinVideos => ReorderJob::Tool(FileTool::Join, files),
        ReorderPurpose::CreatePdf => ReorderJob::Image(ImageOp::CombinePdf, files),
    }
}

/// How the Edit Metadata window reads and writes a file.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MetaKind {
    /// Audio, video or GIF through ffmpeg.
    Media,
    /// PDF document attributes.
    Pdf,
}

/// The window that edits this file's metadata, if it has one: audio, video, GIF or PDF.
pub(crate) fn meta_kind(p: &std::path::Path) -> Option<MetaKind> {
    use ferrocast_core::convert::{ext, family_of, Family};
    match family_of(p) {
        Family::Audio | Family::Video => Some(MetaKind::Media),
        Family::Pdf => Some(MetaKind::Pdf),
        Family::Image if ext(p) == "gif" => Some(MetaKind::Media),
        _ => None,
    }
}

#[derive(Clone)]
pub enum View {
    Root,
    Clipboard,
    Snippets,
    Quicklinks,
    Emoji,
    Files,
    Windows,
    Processes,
    Timers,
    CalcHistory,
    Shelf,
    Convert(Vec<PathBuf>),
    /// Annotation canvas: original file, its pixel size, the drawing and the pen state.
    Annotate {
        file: PathBuf,
        size: (u32, u32),
        doc: ferrocast_core::annotate::Doc,
        tool: ferrocast_core::annotate::Tool,
        color: usize,
        sel: Option<usize>,
    },
    /// Edit Metadata window: `original` is None while the tags are read (or when that failed, with
    /// `error`); `working` is what the rows edit.
    EditMetadata {
        file: PathBuf,
        kind: MetaKind,
        original: Option<Vec<ferrocast_core::metaform::Field>>,
        working: Vec<ferrocast_core::metaform::Field>,
        remove_all: bool,
        error: Option<String>,
    },
    /// Video speed window: file, duration once probed and the chosen factor.
    VideoSpeed {
        file: PathBuf,
        duration: Option<f64>,
        factor: f64,
    },
    /// Video redaction window: up to 64 movable areas, each with its own time range and style;
    /// `sel` is the area being edited and `ref_t` the time of the frame shown behind them.
    VideoRedact {
        file: PathBuf,
        size: Option<(u32, u32)>,
        aspect: Option<(u32, u32)>,
        duration: f64,
        areas: Vec<Area>,
        sel: usize,
        ref_t: f64,
    },
    /// Video crop window: file, pixel size once probed, the rectangle and an optional locked aspect,
    /// plus the duration and the time of the reference frame shown behind the rectangle.
    VideoCrop {
        file: PathBuf,
        size: Option<(u32, u32)>,
        rect: ferrocast_core::croprect::Rect,
        aspect: Option<(u32, u32)>,
        duration: f64,
        ref_t: f64,
    },
    VideoSnapshot {
        file: PathBuf,
        trim: Option<ferrocast_core::videoplan::Trim>,
    },
    /// Video split window: the timeline of the trim viewer with cut points (sorted, on the frame
    /// grid) and an "equal parts" count (2..=50) used only while there are no points.
    VideoSplit {
        file: PathBuf,
        trim: Option<ferrocast_core::videoplan::Trim>,
        points: Vec<f64>,
        parts: usize,
    },
    /// Audio trim window: the file, its timeline state and the waveform samples (both None while
    /// the audio is being read).
    AudioTrim {
        file: PathBuf,
        trim: Option<ferrocast_core::videoplan::Trim>,
        audio: Option<Arc<crate::media_tools::AudioSamples>>,
    },
    /// Bleep window: waveform peaks (None while the audio is read), the clip length and the
    /// ranges to replace with a tone; `sel` is the selected range.
    BleepAudio {
        file: PathBuf,
        duration: f64,
        peaks: Option<Arc<Vec<(i16, i16)>>>,
        ranges: Vec<(f64, f64)>,
        sel: usize,
    },
    /// Normalize window: the measured integrated loudness (None while measuring, NaN when it could
    /// not be measured) and the target, loudness range and true-peak settings.
    NormalizeAudio {
        file: PathBuf,
        loudness: Option<f64>,
        target: f64,
        range: f64,
        peak: f64,
    },
    /// Audio channels window: mono or stereo output and the left/right volume in dB.
    ChannelsAudio {
        file: PathBuf,
        in_channels: Option<u32>,
        stereo: bool,
        left_db: f64,
        right_db: f64,
    },
    /// Audio to video window: waveform or a chosen still picture, and the canvas shape. `picking`
    /// is true while the native chooser is open (one at a time).
    VisualizeAudio {
        file: PathBuf,
        orient: ferrocast_core::media::Canvas,
        style: crate::media_tools::VisStyle,
        image: Option<PathBuf>,
        picking: bool,
    },
    /// Video trim viewer: the file and its timeline state (None while the video is being probed).
    VideoTrim {
        file: PathBuf,
        trim: Option<ferrocast_core::videoplan::Trim>,
    },
    /// Collage editor: the pictures, the options and the colour index.
    CollageImage {
        files: Vec<PathBuf>,
        opts: ferrocast_core::collage::CollageOpts,
        color: usize,
    },
    /// Background editor: file, pixel size, options and the colour index.
    FrameImage {
        file: PathBuf,
        size: (u32, u32),
        opts: ferrocast_core::frame::FrameOpts,
        color: usize,
        /// The native picture chooser is open (one dialog at a time).
        picking: bool,
    },
    /// Redaction editor: file, pixel size, the areas, the selected one and the style
    /// (`radius` is the blur radius in pixels, used when `blur` is on).
    RedactImage {
        file: PathBuf,
        size: (u32, u32),
        areas: Vec<ferrocast_core::croprect::Rect>,
        sel: Option<usize>,
        blur: bool,
        radius: u32,
        color: usize,
    },
    /// Crop editor: file, pixel size, the rectangle and an optional locked aspect.
    CropImage {
        file: PathBuf,
        size: (u32, u32),
        rect: ferrocast_core::croprect::Rect,
        aspect: Option<(u32, u32)>,
    },
    /// Compress window for images: size of the first file in bytes and pixels (probed off the UI
    /// thread), the goal (mode 0 size in KB, 1 percent smaller, 2 preset) and the optional resize.
    CompressImage {
        files: Vec<PathBuf>,
        orig_bytes: u64,
        size: Option<(u32, u32)>,
        mode: usize,
        kb: u32,
        percent: u8,
        preset: usize,
        max_edge: Option<u32>,
        resize: bool,
        /// Some: the window compresses audio or video instead of images.
        media: Option<MediaWin>,
    },
    /// Live-preview photo editor: original file, its pixel size and the control values.
    EditPhoto {
        file: PathBuf,
        size: (u32, u32),
        opts: ferrocast_core::photo::PhotoOpts,
    },
    /// Visual page organizer for one PDF: `n` source pages, the edited plan.
    PdfOrganize {
        file: PathBuf,
        n: usize,
        plan: ferrocast_core::pdfplan::PdfPlan,
    },
    /// Choose the order of several files before merging / joining / combining them.
    Reorder {
        purpose: ReorderPurpose,
        files: Vec<PathBuf>,
        cursor: usize,
    },
    Notes,
    NotionSearch,
    /// Search Obsidian: notes of the chosen vault with a preview pane (state in `App::ov`).
    ObsidianSearch,
    /// Add to Notion form: optional title, the text, and where it goes (the capture target or a one-off).
    NotionAdd {
        title: String,
        text: String,
        target: ferrocast_core::models::NotionTarget,
    },
    /// New Obsidian Note form: optional title and the text.
    ObsidianNew {
        title: String,
        text: String,
    },
    /// Append to Daily Note form: only the text.
    ObsidianDaily {
        text: String,
    },
    Colors(Rgba),
    SnippetForm {
        id: Option<String>,
        name: String,
        text: String,
        keyword: String,
    },
    QuicklinkForm {
        id: Option<String>,
        name: String,
        link: String,
        open_with: String,
    },
    Args {
        target: ArgTarget,
        title: String,
        defs: Vec<ArgDef>,
        values: Vec<String>,
    },
    Output {
        run: u64,
        title: String,
        text: String,
        done: bool,
    },
    Confirm {
        msg: String,
        act: Act,
    },
    Settings,
    Alias {
        id: String,
        name: String,
        value: String,
    },
    Hotkey {
        id: String,
        name: String,
        err: String,
    },
    CreateScript {
        title: String,
        mode: usize,
    },
    Welcome,
    Shortcuts,
    About,
    /// A finished translation shown to the user.
    Translation {
        lang: String,
        text: String,
    },
    Ai {
        prompt: String,
        asked: String,
        answer: String,
        history: Vec<(String, String)>,
        busy: bool,
        run: u64,
    },
    /// Radial menu shown under the pointer during a file drag; `center` is in screen points.
    Wheel {
        files: Vec<PathBuf>,
        center: Pos2,
        segs: Vec<WheelSeg>,
        kind: ferrocast_core::wheel::WheelKind,
        /// Segment chosen with the arrow keys.
        sel: Option<usize>,
    },
    /// A newer stable release is available (ask-first mode, or a manual check).
    Update {
        release: ferrocast_core::update::Release,
    },
    /// Capture window (S7): the saved picture and its actions.
    CaptureResult {
        file: PathBuf,
        size: (u32, u32),
    },
    /// Recordings browser (T19): the projects of the recordings folder, newest first.
    Recordings,
    /// Studio editor (T26): the recording folder being edited; its state lives in `App::editor`.
    Editor {
        project: PathBuf,
    },
    /// A list screen of the running Raycast extension (state in `App::ext`).
    ExtList,
    /// A detail (or not-supported) screen of the running Raycast extension.
    ExtDetail,
    /// A form of the running Raycast extension (fields drawn natively, values in `App::ext`).
    ExtForm,
}

/// One wheel segment: label, SF symbol, and what releasing on it does.
#[derive(Clone)]
pub struct WheelSeg {
    pub label: String,
    pub symbol: &'static str,
    pub pick: WheelPick,
    /// Optional tool to install before this segment can run (shown dimmed).
    pub needs: Option<&'static str>,
}

#[derive(Clone)]
pub enum WheelPick {
    Convert(ferrocast_core::convert::Target),
    Image(ferrocast_core::convert::ImageOp),
    File(ferrocast_core::convert::FileTool),
    Act(Act),
    /// Translate the dragged text into this language.
    Translate(String),
}

#[derive(Clone, Copy, PartialEq)]
enum Shape {
    List,
    Form,
    Single,
}

impl View {
    fn shape(&self) -> Shape {
        match self {
            View::Root
            | View::Clipboard
            | View::Snippets
            | View::Quicklinks
            | View::Emoji
            | View::Files
            | View::Windows
            | View::Processes
            | View::Timers
            | View::CalcHistory
            | View::Shelf
            | View::Convert(_)
            | View::Notes
            | View::NotionSearch
            | View::ObsidianSearch
            | View::Recordings
            | View::ExtList
            | View::Colors(_) => Shape::List,
            View::SnippetForm { .. }
            | View::QuicklinkForm { .. }
            | View::NotionAdd { .. }
            | View::ObsidianNew { .. }
            | View::ObsidianDaily { .. }
            | View::ExtForm
            | View::Settings => Shape::Form,
            _ => Shape::Single,
        }
    }
    /// Footer key chips (label, key), left to right, for the tool windows; empty for every other view.
    /// The primary action (what Enter does in that window) comes first, then the most useful other
    /// keys, and Back last; at most four chips so the footer fits next to the title.
    fn footer_hints(&self) -> Vec<(&'static str, &'static str)> {
        const BACK: (&str, &str) = ("Back", "esc");
        const SAVE: (&str, &str) = ("Save copy", "↵");
        let trim = vec![("Trim", "↵"), ("Start", "I"), ("End", "O"), BACK];
        match self {
            View::CompressImage { .. } => vec![("Compress", "↵"), BACK],
            View::EditMetadata { .. }
            | View::VideoSpeed { .. }
            | View::ChannelsAudio { .. }
            | View::BleepAudio { .. }
            | View::FrameImage { .. }
            | View::EditPhoto { .. }
            | View::VideoRedact { .. } => vec![SAVE, BACK],
            View::NormalizeAudio { .. } => vec![("Normalize", "↵"), BACK],
            View::VisualizeAudio { .. } => vec![("Make video", "↵"), BACK],
            View::VideoTrim { .. } | View::AudioTrim { .. } => trim,
            View::VideoSnapshot { .. } => vec![("Save PNG", "↵"), ("Frame", "←/→"), BACK],
            View::VideoSplit { .. } => vec![("Split", "↵"), ("Cut here", "M"), ("Remove last", "⌫"), BACK],
            View::CollageImage { .. } => vec![("Save collage", "↵"), BACK],
            View::RedactImage { .. } => vec![SAVE, ("Remove", "⌫"), BACK],
            View::CropImage { .. } | View::VideoCrop { .. } => vec![("Crop", "↵"), BACK],
            View::Annotate { .. } => vec![SAVE, ("Undo", "⌘Z"), ("Delete", "⌫"), BACK],
            View::CaptureResult { .. } => vec![("Copy", "↵"), ("Save", "⌘S"), ("Annotate", "⌘⇧A"), BACK],
            View::Recordings => vec![("Open", "↵"), ("Show in Finder", "⌘↵"), ("Trash", "⌘⌫"), BACK],
            // the footer drops chips from the second to last one backwards when they do not fit, so the
            // ones that matter most come first: Play, Export and Full screen outlast Frame, Undo and Redo
            View::Editor { .. } => {
                vec![("Play", "space"), ("Export", "⌘E"), ("Full screen", "⌃⌘F"), ("Frame", "←/→"), ("Undo", "⌘Z"), ("Redo", "⇧⌘Z"), BACK]
            }
            View::Reorder { purpose, .. } => {
                let primary = match purpose {
                    ReorderPurpose::JoinVideos => "Join",
                    ReorderPurpose::CreatePdf => "Create PDF",
                    ReorderPurpose::MergePdf | ReorderPurpose::MergeAudio => "Merge",
                };
                vec![(primary, "↵"), ("Move", "⌘↑/↓"), BACK]
            }
            View::PdfOrganize { .. } => vec![SAVE, ("Rotate", "R"), ("Remove", "⌫"), BACK],
            View::NotionAdd { .. } | View::ObsidianNew { .. } | View::ObsidianDaily { .. } => vec![("Send", "⌘↵"), BACK],
            View::Settings => vec![("Tabs", "⌘1–0"), ("Find", "⌘F"), BACK],
            _ => vec![],
        }
    }
    fn title(&self) -> &'static str {
        match self {
            View::Root => "Ferrocast",
            View::Clipboard => "Clipboard History",
            View::Snippets => "Search Snippets",
            View::Quicklinks => "Search Quicklinks",
            View::Emoji => "Search Emoji & Symbols",
            View::Files => "Search Files",
            View::Windows => "Switch Windows",
            View::Processes => "Kill Process",
            View::Timers => "Timers",
            View::CalcHistory => "Calculator History",
            View::Shelf => "File Shelf",
            View::PdfOrganize { .. } => "Organize PDF",
            View::Reorder { .. } => "Choose Order",
            View::Annotate { .. } => "Annotate Image",
            View::CaptureResult { .. } => "Capture",
            View::Recordings => "Recordings",
            View::Editor { .. } => "Editor",
            View::EditPhoto { .. } => "Edit Photo",
            View::CropImage { .. } => "Crop Image",
            View::CompressImage { media: Some(_), .. } => "Compress Audio and Video",
            View::CompressImage { .. } => "Compress Images",
            View::RedactImage { .. } => "Redact Image",
            View::FrameImage { .. } => "Add Background",
            View::CollageImage { .. } => "Make a Collage",
            View::VideoTrim { .. } => "Trim Video",
            View::AudioTrim { .. } => "Trim Audio",
            View::BleepAudio { .. } => "Bleep Audio",
            View::NormalizeAudio { .. } => "Normalize Audio",
            View::ChannelsAudio { .. } => "Audio Channels",
            View::VisualizeAudio { .. } => "Audio to Video",
            View::VideoSnapshot { .. } => "Video Snapshots",
            View::VideoSplit { .. } => "Split Video",
            View::VideoCrop { .. } => "Crop Video",
            View::VideoRedact { .. } => "Redact Video",
            View::VideoSpeed { .. } => "Change Video Speed",
            View::EditMetadata { .. } => "Edit Metadata",
            View::Convert(_) => "Convert",
            View::Notes => "Search Notes",
            View::NotionSearch => "Search Notion",
            View::ObsidianSearch => "Search Obsidian",
            View::NotionAdd { .. } => "Add to Notion",
            View::ObsidianNew { .. } => "New Obsidian Note",
            View::ObsidianDaily { .. } => "Append to Daily Note",
            View::Colors(_) => "Copy Color as…",
            View::SnippetForm { .. } => "Snippet",
            View::QuicklinkForm { .. } => "Quicklink",
            View::Args { .. } => "Arguments",
            View::Output { .. } => "Output",
            View::Confirm { .. } => "Confirm",
            View::Settings => "Settings",
            View::Alias { .. } => "Set Alias",
            View::Hotkey { .. } => "Set Hotkey",
            View::CreateScript { .. } => "Create Script Command",
            View::Ai { .. } => "Ask AI",
            View::Welcome => "Welcome to Ferrocast",
            View::Shortcuts => "Keyboard Shortcuts",
            View::About => "About Ferrocast",
            View::Translation { .. } => "Translation",
            View::Wheel { .. } => "Wheel",
            View::Update { .. } => "Update available",
            View::ExtList | View::ExtDetail | View::ExtForm => "Raycast Extension",
        }
    }
    fn placeholder(&self) -> &'static str {
        match self {
            View::Root => "Search apps, commands, files… or calculate",
            View::Clipboard => "Search clipboard history…",
            View::Snippets => "Search snippets…",
            View::Quicklinks => "Search quicklinks…",
            View::Emoji => "Search emoji by name…",
            View::Files => "Search files by name…",
            View::Windows => "Search open windows…",
            View::Processes => "Search processes…",
            View::Timers => "Search timers…",
            View::CalcHistory => "Search past calculations…",
            View::Shelf => "Search the shelf…",
            View::Convert(_) => "Convert to…",
            View::Notes => "Search notes…",
            View::NotionSearch => "Search Notion…",
            View::ObsidianSearch => "Search your vault…",
            View::Recordings => "Search recordings…",
            _ => "Filter…",
        }
    }
}

enum RunKind {
    Output,
    Compact(String),
    Silent(String),
    Inline(PathBuf),
}

struct ActionPanel {
    actions: Vec<Action>,
    query: String,
    sel: usize,
    item: Item,
}

#[derive(Default)]
struct Keys {
    up: bool,
    down: bool,
    enter: bool,
    /// The Enter press is an auto-repeat of a held key.
    repeat: bool,
    cmd_enter: bool,
    esc: bool,
    backspace: bool,
    cmd_k: bool,
    /// Tour navigation.
    left: bool,
    right: bool,
}

/// Bus messages handled per frame; a flood (a chatty background job) must not freeze the UI.
pub(crate) const MAX_MSGS_PER_FRAME: usize = 200;

const NOTION_NO_TOKEN: &str = "Paste your Notion token first.";

/// Notion search box and results shown in Settings → Integrations.
#[derive(Default)]
pub struct NotionUi {
    pub query: String,
    /// The token being typed; cleared once saved (never shared with the AI key field).
    pub token: String,
    /// Id of the latest search; an answer carrying an older id is dropped.
    pub request: u64,
    pub searching: bool,
    pub results: Vec<ferrocast_core::notion::NotionItem>,
}

/// State of the Search Notion view (not Settings): results, paging and the request in flight.
#[derive(Default)]
pub struct NotionView {
    pub items: Vec<ferrocast_core::notion::NotionItem>,
    pub has_more: bool,
    pub next_cursor: Option<String>,
    /// Id of the newest request; an answer carrying an older id is dropped.
    pub request: u64,
    pub searching: bool,
    /// Trimmed query of the newest request (`None`: nothing asked since the view opened).
    pub asked: Option<String>,
    /// Query that produced `items` (Load more continues it).
    pub shown: String,
    /// Latest typed query and the egui time it last changed (debounce).
    pub seen: String,
    pub seen_at: f64,
    /// `Some(false)` once the Keychain answered that there is no token.
    pub connected: Option<bool>,
}

/// State of Add to Notion: a send in flight and the title property name of each database target
/// (read once per target id and kept for the session).
#[derive(Default)]
pub struct NotionAdd {
    pub sending: bool,
    pub schema: std::sync::Arc<std::sync::Mutex<HashMap<String, String>>>,
}

/// State of the New Obsidian Note / Append to Daily Note commands.
#[derive(Default)]
pub struct ObsidianCompose {
    /// The precondition check of a send is running on a thread.
    pub sending: bool,
}

const OBSIDIAN_NO_VAULT: &str = "Choose your Obsidian vault in Settings → Integrations.";

/// Settings → Integrations → Obsidian: what the background scan found (None = still looking).
#[derive(Default)]
pub struct ObsidianUi {
    /// A scan was started this session (the tab starts one the first time it is drawn).
    pub started: bool,
    pub scanning: bool,
    pub vaults: Option<Vec<ferrocast_core::obsidian::Vault>>,
    /// Vault Obsidian reports as open.
    pub open: Option<std::path::PathBuf>,
    pub installed: Option<bool>,
    /// The folder picker is open (one at a time).
    pub picking: bool,
}

/// Status line of the Obsidian app row.
pub fn obsidian_status(installed: Option<bool>) -> &'static str {
    match installed {
        None => "Checking for Obsidian…",
        Some(true) => "Obsidian is installed",
        Some(false) => "Obsidian is not installed. Download it from obsidian.md.",
    }
}

/// Folder name of a vault path: the name Obsidian and `obsidian://` URIs use.
fn vault_name(path: &std::path::Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// State of the Search Obsidian view. The index cache outlives the view, so reopening shows the last
/// results at once while a refresh runs; everything else is reset on open.
#[derive(Default)]
pub struct ObsidianView {
    /// The last index and the vault it belongs to.
    pub cache: Option<(PathBuf, Arc<Vec<ferrocast_core::obsidian::NoteEntry>>)>,
    /// The vault whose folder was not found by the last index.
    pub missing: Option<PathBuf>,
    /// Vault the newest index request was made for (None: ask again on the next tick).
    pub requested_for: Option<PathBuf>,
    pub indexing: bool,
    /// Id of the newest index request; older answers are dropped.
    pub index_request: u64,
    /// Content hits (relative paths) for the query they answer.
    pub content: Option<(String, Vec<String>)>,
    pub content_request: u64,
    /// Trimmed query of the newest content scan.
    pub content_asked: Option<String>,
    /// Latest typed query and the egui time it last changed (debounce).
    pub seen: String,
    pub seen_at: f64,
    /// ⌘Y hides the preview pane (the list then takes the full width).
    pub hide_preview: bool,
    /// Note whose preview was last requested, and the id of that request.
    pub preview_for: Option<String>,
    pub preview_request: u64,
    /// The note the answer belongs to and its blocks (`Err`: unreadable).
    pub preview: Option<(String, Result<Vec<ferrocast_core::obsidian::Block>, ()>)>,
    /// Selected note waiting out the preview pause, and the egui time it was first seen.
    pub pv_seen: Option<String>,
    pub pv_seen_at: f64,
}

/// Wait after the last keystroke before Search Notion asks the API (seconds of egui time).
const NOTION_DEBOUNCE: f64 = 0.25;
const NOTION_PAGE: u32 = 20;
/// Wait after the last keystroke before Search Obsidian scans note contents (seconds of egui time).
const OBSIDIAN_DEBOUNCE: f64 = 0.25;
/// Wait before the selected note is read for the preview (seconds of egui time).
const OBSIDIAN_PREVIEW_PAUSE: f64 = 0.12;
/// Notes listed per section.
const OBSIDIAN_LIMIT: usize = 30;

/// `[[stem]]` for a note: Obsidian resolves links by file name, so the stem is the target, with the
/// title as the shown text when it differs (`[[stem|Title]]`). Folders are left out, as Obsidian does.
fn wikilink(rel: &str, title: &str) -> String {
    let file = rel.rsplit('/').next().unwrap_or(rel);
    let stem = file.rsplit_once('.').map(|(s, _)| s).unwrap_or(file);
    if title.is_empty() || title == stem {
        format!("[[{stem}]]")
    } else {
        format!("[[{stem}|{title}]]")
    }
}

/// True while a visible floating panel of ours (pin, HUD, countdown, shelf, notes) is on screen:
/// `NSApplication.hide` would take it away too (live bug: Pin vanished the moment it opened).
/// The launcher is a plain window, not a panel.
fn other_panel_visible(mtm: objc2::MainThreadMarker) -> bool {
    use objc2::runtime::NSObjectProtocol;
    use objc2::ClassType;
    objc2_app_kit::NSApplication::sharedApplication(mtm)
        .windows()
        .iter()
        .any(|w| w.isVisible() && w.isKindOfClass(objc2_app_kit::NSPanel::class()))
}

pub struct App {
    /// iCloud sync state (see `sync_ctl`).
    sync: SyncCtl,
    /// Ask AI conversations (chats.json).
    chats: Vec<Chat>,
    /// Id of the chat the open Ask AI view writes to.
    ai_chat: Option<String>,
    /// The update poller thread once started (never more than one).
    pub(crate) update_poller: Option<std::thread::JoinHandle<()>>,
    ctx: Context,
    shared: Arc<Shared>,
    bus: Bus,
    rx: Receiver<Msg>,
    store: Store,
    settings: Settings,
    snippets: Vec<Snippet>,
    quicklinks: Vec<Quicklink>,
    notes: Vec<Note>,
    /// The floating Notes window: open note, history, palette mode, debounced saving.
    notes_ctl: crate::notes_ctl::NotesCtl,
    frecency: Frecency,
    /// Newest release found by a check that is newer than the running version (drives Settings).
    update_available: Option<ferrocast_core::update::Release>,
    apps: Vec<apps::AppInfo>,
    apps_at: Instant,
    scripts: Vec<ScriptCommand>,
    base: Vec<Item>,
    /// Raycast extensions: discovery, the running command's process and screens.
    pub(crate) ext: ext_glue::ExtState,
    hotkeys: Option<Hotkeys>,
    tiling: Option<ferrocast_macos::tiling::Handle>,
    tiling_paused: bool,
    /// The window the quick-action bar currently hangs above.
    quick_window: Option<u64>,
    /// Tiling shortcuts that could not be registered, shown in Settings.
    pub(crate) tiling_hotkey_issues: Vec<String>,
    /// Layout the menu bar submenu shows as chosen, and the settings layout it last followed.
    tray_layout: u8,
    layout_seen: u8,
    /// Whether the tiling hotkeys are currently registered.
    tiling_hotkeys: bool,
    /// Screenshot commands: availability and the once-per-session shortcut sentence.
    capture: crate::capture_ctl::CaptureState,
    /// The select-then-act flow in progress (Capture Area, Window, Screen and Scrolling), at most one.
    selection: crate::capture_ctl::PendingSelection,
    /// Opt-in Enter on a Finder selection (tap, in-flight guard, injectable system hooks).
    finder: services::FinderEnterState,
    _tray: Option<tray_icon::TrayIcon>,
    /// Recording lifecycle (S13); T19 drives it, the tray mirrors it.
    record: crate::record_ctl::RecordCtl,
    /// Whether the tray currently shows the recording icon and `Stop Recording` item.
    tray_rec_shown: bool,
    /// Live recording flow state (T19): session number, pending target, command channel, browser rows.
    rt: crate::record_ctl::Runtime,
    /// The studio editor session (T26), present while `View::Editor` is on the stack.
    editor: Option<crate::editor_ctl::Editor>,
    /// The second menu-bar item that toggles Notes; present while `notes_menu_bar_button` is on.
    /// Dropping it removes the item.
    notes_tray: Option<Box<dyn std::any::Any>>,
    icons: IconCache,
    stack: Vec<View>,
    query: String,
    prev_query: String,
    selected: usize,
    scroll_to_sel: bool,
    panel: Option<ActionPanel>,
    toast: Option<(String, Instant)>,
    /// The toast is user or extension text: shown as typed.
    toast_user: bool,
    /// The last "Reset this tab": (tab, the settings before it, when); Undo restores that tab's fields.
    settings_undo: Option<(usize, Settings, Instant)>,
    prev_pid: Option<i32>,
    /// Side effects of delivering a translation; replaced by a recorder in tests.
    fx: Effects,
    /// Counter of translations started from the wheel; carried by `Msg::Translated` and `Msg::EffectStarted`.
    translate_run: u64,
    /// The run whose translation has already arrived (a late `EffectStarted` for it is ended at once).
    translated_run: Option<u64>,
    /// The translating effect currently on screen or in a page.
    translating_effect: Option<ActiveEffect>,
    /// When the current translation started (the safety timeout counts from here).
    translate_started: Instant,
    /// The "Apple Translation needs macOS 26" notice was shown this session.
    apple_fallback_told: bool,
    /// What Settings → Translate shows per wheel language for the Apple engine.
    pub(crate) apple_langs: std::collections::HashMap<String, ferrocast_core::translate::AppleLang>,
    /// Browser whose page a marked replace is running in: cleaned if the replace fails.
    web_pending: Option<ferrocast_core::translate::Browser>,
    visible: bool,
    had_focus: bool,
    /// First-run tour in progress: the window stays open and never hides on blur until it ends.
    onboarding: bool,
    shown_at: Instant,
    hidden_at: Instant,
    cur_height: f32,
    /// Normal or maximised to the screen (the editor's Full screen button); see `editor_ctl::WindowMode`.
    win: crate::editor_ctl::WindowMode,
    files: (String, Vec<PathBuf>),
    files_req: String,
    query_changed_at: Instant,
    win_cache: Vec<running::WindowInfo>,
    proc_cache: Vec<ferrocast_macos::process::ProcInfo>,
    /// Running countdowns: when they end, their length, label, and a cancel flag.
    timers: Vec<RunningTimer>,
    calc_history: Vec<ferrocast_core::models::CalcEntry>,
    recent_emoji: Vec<String>,
    /// Files kept on the File Shelf (persisted in shelf.json).
    shelf: Vec<PathBuf>,
    /// The app icon, decoded once for the footer.
    app_icon: Option<egui::TextureHandle>,
    /// Whether the 22 MB Unicode fallback font has been loaded.
    wide_font: bool,
    /// Set by show(); cleared by the first frame drawn afterwards (FERROCAST_PROFILE).
    pub(crate) show_t: Option<Instant>,
    /// Last push/pop/reset, to debounce clicks across view changes.
    pub(crate) nav_at: Instant,
    /// Warm Claude Code process while Ask AI is open (ClaudeCode provider only).
    claude: Option<std::sync::Arc<services::ClaudeSession>>,
    /// The text a Translate wheel is about; never logged.
    wheel_text: Option<String>,
    /// Model lists for the Settings picker, by `services::models_key`; `None` while loading.
    /// Optional conversion tools found on this Mac (probed once).
    tools: ferrocast_core::convert::Tools,
    pub(crate) ai_models: std::collections::HashMap<String, Option<Result<Vec<String>, String>>>,
    runs: HashMap<u64, RunKind>,
    next_run: u64,
    inline: HashMap<PathBuf, (String, Instant)>,
    focus_first: bool,
    dark: bool,
    /// Saved custom themes (`themes.json`); `settings.custom_theme` names the selected one.
    pub(crate) custom_themes: Vec<ferrocast_core::themes::CustomTheme>,
    started: bool,
    act_item: Option<Item>,
    scratch: String,
    key_scratch: String,
    /// Settings → Integrations: Notion search box, latest request id and its results.
    notion: NotionUi,
    /// Search Notion command view.
    nv: NotionView,
    /// Search Obsidian command view.
    pub(crate) ov: ObsidianView,
    /// Add to Notion commands.
    na: NotionAdd,
    /// New Obsidian Note / daily note commands.
    pub(crate) oc: ObsidianCompose,
    /// Settings → Integrations → Obsidian.
    pub(crate) obsidian: ObsidianUi,
    rates: ferrocast_core::currency::Rates,
    rates_checked_at: Instant,
    /// Result list for the current query/view, reused across frames until something changes.
    sec_cache: Option<(String, Vec<Section>)>,
    sec_dirty: bool,
    /// First-run tour: current slide and its animation.
    tour: tour::Tour,
    /// First-launch sunrise over the tour's first slide; `None` once played (or never started).
    intro: Option<intro::Intro>,
}

/// San Francisco (the system face) first, Apple Symbols for ⌘⌥⇧ glyphs. Arial Unicode, the
/// 22 MB fallback for scripts San Francisco lacks (CJK, Arabic, Hebrew, Thai…), costs about
/// 40 MB of memory, so it is only added once such text actually shows up (see `App::ensure_wide_font`).
fn load_fonts(ctx: &Context, wide: bool) {
    let mut fonts = egui::FontDefinitions::default();
    if let Ok(bytes) = std::fs::read("/System/Library/Fonts/SFNS.ttf") {
        fonts.font_data.insert("sf-pro".into(), Arc::new(egui::FontData::from_owned(bytes)));
        fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "sf-pro".into());
    }
    if let Ok(bytes) = std::fs::read("/System/Library/Fonts/SFNSMono.ttf") {
        fonts.font_data.insert("sf-mono".into(), Arc::new(egui::FontData::from_owned(bytes)));
        fonts.families.entry(egui::FontFamily::Monospace).or_default().insert(0, "sf-mono".into());
    }
    let mut extra = vec![("apple-symbols", "/System/Library/Fonts/Apple Symbols.ttf")];
    if wide {
        extra.push(("arial-unicode", "/System/Library/Fonts/Supplemental/Arial Unicode.ttf"));
    }
    for (name, path) in extra {
        if let Ok(bytes) = std::fs::read(path) {
            fonts.font_data.insert(name.into(), Arc::new(egui::FontData::from_owned(bytes)));
            for fam in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts.families.entry(fam).or_default().push(name.into());
            }
        }
    }
    ctx.set_fonts(fonts);
}

/// The language the `language` setting stands for on this Mac (`"system"` follows macOS).
pub(crate) fn resolve_language(setting: &str) -> ferrocast_core::i18n::Lang {
    ferrocast_core::i18n::resolve(setting, &ferrocast_macos::apps::preferred_languages())
}

/// `keywords` plus the English `title` when the interface language shows another one, so a search
/// in English still finds a command whose title is translated.
fn with_english_title(mut keywords: Vec<String>, title: &str) -> Vec<String> {
    if tr(title) != title {
        keywords.push(title.to_string());
    }
    keywords
}

/// Scripts outside what San Francisco and Apple Symbols cover.
pub(crate) fn needs_wide_font(text: &str) -> bool {
    text.chars().any(|c| {
        let u = c as u32;
        (0x0590..0x2000).contains(&u) || (0x2E80..0x1F000).contains(&u)
    })
}

/// SF Symbol for each system command (white on the grey tile).
fn system_symbol(c: &system::SystemCommand) -> &'static str {
    use system::SystemCommand::*;
    match c {
        LockScreen => "sf:lock",
        Sleep => "sf:moon.zzz",
        SleepDisplays => "sf:display",
        Restart => "sf:arrow.counterclockwise",
        ShutDown => "sf:power",
        LogOut => "sf:rectangle.portrait.and.arrow.right",
        ScreenSaver => "sf:sparkles.tv",
        PlayPause => "sf:playpause",
        NextTrack => "sf:forward",
        PreviousTrack => "sf:backward",
        ToggleMute => "sf:speaker.slash",
        VolumeUp => "sf:speaker.wave.3",
        VolumeDown => "sf:speaker.wave.1",
        Volume(_) => "sf:speaker.wave.2",
        ShowDesktop => "sf:menubar.dock.rectangle",
        ToggleAppearance => "sf:circle.lefthalf.filled",
        OpenTrash | EmptyTrash => "sf:trash",
        EjectAll => "sf:eject",
        ToggleHiddenFiles | UnhideAll => "sf:eye",
        HideAllExceptFrontmost => "sf:eye.slash",
        QuitAllApps => "sf:xmark.circle",
        DismissNotifications => "sf:bell.slash",
        ToggleWifi => "sf:wifi",
        KeepAwake => "sf:cup.and.saucer",
    }
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, hidden: bool) -> App {
        let ctx = cc.egui_ctx.clone();
        let prof = std::env::var_os("FERROCAST_PROFILE").is_some();
        let t = Instant::now();
        load_fonts(&ctx, false);
        if prof {
            eprintln!("fonts {:?}", t.elapsed());
        }
        let t = Instant::now();
        let store = Store::default_location();
        let calc_history = store.load("calc-history.json");
        let recent_emoji = store.load("recent-emoji.json");
        let shelf: Vec<PathBuf> = store.load("shelf.json");
        let data = store.load_all();
        // the interface language, and the wide font its glyphs need (CJK)
        let language = resolve_language(&data.settings.language);
        ferrocast_core::i18n::set_language(language);
        let wide_font = language.needs_wide_font();
        if wide_font {
            load_fonts(&ctx, true);
        }
        let store_themes = ferrocast_core::themes::load_with_ids(&store);
        if prof {
            eprintln!("data {:?}", t.elapsed());
        }
        let t = Instant::now();
        let (tx, rx) = std::sync::mpsc::channel();
        let bus = Bus { tx, ctx: ctx.clone() };
        services::ensure_scripts_dir(&store);
        let shared = Arc::new(Shared {
            store: store.clone(),
            clipboard: Mutex::new(data.clipboard.clone()),
            snippets: RwLock::new(data.snippets.clone()),
            settings: RwLock::new(data.settings.clone()),
            ignored_clipboard_count: Default::default(),
            visible: Default::default(),
            keywords: Default::default(),
            clip_dirty: Default::default(),
        });
        services::start_clipboard(shared.clone(), bus.clone());
        services::start_wheel(shared.clone(), bus.clone());
        let update_poller = services::start_update_poller(shared.clone(), bus.clone());
        {
            // the sync timer; every tick is ignored while sync is off
            let b = bus.clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(sync_ctl::SYNC_EVERY);
                b.send(Msg::SyncTick);
            });
        }
        if let Some(exe) = instance::running_exe() {
            // leftovers of a previous self-update; file removal off the startup path
            let root = store.root.clone();
            std::thread::spawn(move || instance::cleanup_update_leftovers(&root, &exe));
        }
        {
            // the floating panel reports its edits; the app persists them
            let b = bus.clone();
            ferrocast_macos::shelf::set_on_change(Box::new(move |v| b.send(Msg::ShelfChanged(v))));
            // same for the Notes window and its palette: the callbacks only send a Msg
            crate::notes_ctl::install_callbacks(&bus);
        }
        if prof {
            eprintln!("services {:?}", t.elapsed());
        }
        let t = Instant::now();
        let mut hotkeys = Hotkeys::new(bus.clone());
        let mut toast = None;
        if let Some(h) = hotkeys.as_mut() {
            let errs = h.apply(&data.settings);
            if let Some(e) = errs.first() {
                toast = Some((format!("Hotkey error: {e}"), Instant::now()));
            }
        }
        let tray = if data.settings.show_menu_bar_icon { services::make_tray(bus.clone()) } else { None };
        let b2 = bus.clone();
        let apps_cache = store.path("apps-cache.plist");
        if prof {
            eprintln!("hotkeys {:?}", t.elapsed());
        }
        std::thread::spawn(move || {
            let t = Instant::now();
            let a = apps::list_apps_cached(Some(&apps_cache));
            if std::env::var_os("FERROCAST_PROFILE").is_some() {
                eprintln!("apps scan {:?} ({} apps)", t.elapsed(), a.len());
            }
            b2.send(Msg::Apps(a))
        });
        let store_rates: ferrocast_core::currency::Rates = store.load("rates.json");
        if store_rates.is_stale(now_secs()) {
            services::refresh_rates(bus.clone());
        }
        let notes_ctl = crate::notes_ctl::NotesCtl::new(Box::new(crate::notes_ctl::MacFx::new(bus.clone(), shared.clone())));
        let mut app = App {
            sync: SyncCtl::default(),
            chats: store.load("chats.json"),
            ai_chat: None,
            update_poller,
            icons: IconCache::new(store.images_dir()).with_bus(bus.clone()),
            ctx,
            shared,
            bus,
            rx,
            store,
            settings: data.settings,
            snippets: data.snippets,
            quicklinks: data.quicklinks,
            notes: data.notes,
            notes_ctl,
            frecency: data.frecency,
            update_available: None,
            apps: vec![],
            apps_at: Instant::now(),
            scripts: vec![],
            base: vec![],
            ext: Default::default(),
            hotkeys,
            tiling: None,
            tiling_paused: false,
            quick_window: None,
            tiling_hotkey_issues: vec![],
            tray_layout: 1,
            layout_seen: 1,
            tiling_hotkeys: false,
            capture: crate::capture_ctl::CaptureState::detect(instance::running_exe().unwrap_or_default()),
            selection: Default::default(),
            finder: Default::default(),
            _tray: tray,
            record: Default::default(),
            tray_rec_shown: false,
            rt: Default::default(),
            editor: None,
            notes_tray: None,
            stack: vec![View::Root],
            query: String::new(),
            prev_query: String::new(),
            selected: 0,
            scroll_to_sel: false,
            panel: None,
            toast,
            toast_user: false,
            settings_undo: None,
            prev_pid: None,
            fx: Effects::real(),
            translate_run: 0,
            translated_run: None,
            translating_effect: None,
            translate_started: Instant::now(),
            apple_fallback_told: false,
            apple_langs: Default::default(),
            web_pending: None,
            visible: !hidden,
            had_focus: false,
            onboarding: false,
            shown_at: Instant::now(),
            hidden_at: Instant::now(),
            cur_height: ui::HEIGHT,
            win: Default::default(),
            files: (String::new(), vec![]),
            files_req: String::new(),
            query_changed_at: Instant::now(),
            win_cache: vec![],
            proc_cache: vec![],
            timers: vec![],
            calc_history,
            recent_emoji,
            shelf,
            app_icon: None,
            wide_font,
            show_t: None,
            nav_at: Instant::now(),
            claude: None,
            wheel_text: None,
            ai_models: Default::default(),
            tools: ferrocast_core::convert::Tools::detect(),
            runs: HashMap::new(),
            next_run: 1,
            inline: HashMap::new(),
            focus_first: false,
            dark: true,
            custom_themes: store_themes,
            started: false,
            act_item: None,
            scratch: String::new(),
            key_scratch: String::new(),
            notion: NotionUi::default(),
            nv: NotionView::default(),
            ov: ObsidianView::default(),
            na: NotionAdd::default(),
            oc: ObsidianCompose::default(),
            obsidian: ObsidianUi::default(),
            rates: store_rates,
            rates_checked_at: Instant::now(),
            sec_cache: None,
            sec_dirty: true,
            tour: tour::Tour::new(),
            intro: None,
        };
        app.shared.visible.store(app.visible, Ordering::Relaxed);
        if tour::first_run(&mut app.settings, hidden) {
            let _ = app.store.save("settings.json", &app.settings);
            app.stack.push(View::Welcome);
            // only this first-run path plays the intro; reopening "Welcome" from search does not
            app.begin_intro(tour::reduce_pref());
            // Permission prompts (the snippet tap asks for Input Monitoring) wait for the tour's
            // permissions slide, and the window drops its always-on-top level so the system
            // dialogs and System Settings can sit above it.
            app.onboarding = true;
            App::set_dock_icon(true);
            app.ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::Normal));
        } else {
            services::start_expansion(app.shared.clone());
        }
        app.reload_scripts();
        app.sync_tiling();
        app.set_theme();
        // Recently Deleted notes past their retention go once per start
        app.notes_do(|c, d| c.startup(d));
        app.sync_notes_tray();
        app.rebuild();
        if app.settings.enter_on_finder {
            // at startup never show the system prompt again; the user can toggle the setting
            app.set_finder_enter(true, false);
        }
        app
    }

    /// Turn the Enter-on-Finder tap on or off. Off removes it; on needs Accessibility (asked once
    /// with `prompt`), and without it nothing is created and a toast explains why.
    pub(crate) fn set_finder_enter(&mut self, on: bool, prompt: bool) {
        if !on {
            // dropping the handle stops the tap thread and removes the tap and the observer
            self.finder.tap = None;
            self.finder.busy = false;
            self.finder.inflight.end();
            return;
        }
        if self.finder.tap.is_some() {
            return;
        }
        if !(self.finder.hooks.trusted)(prompt) {
            self.finder_log("finder-enter: tap NOT started: Accessibility is not granted");
            self.toast(services::FINDER_ENTER_NEEDS_ACCESS);
            return;
        }
        let (root, log) = (self.store.root.clone(), self.finder.hooks.log.clone());
        let tap_log: Arc<dyn Fn(&str) + Send + Sync> = Arc::new(move |line| log(&root, line));
        match (self.finder.hooks.start)(self.finder.inflight.clone(), self.bus.clone(), tap_log) {
            Ok(tap) => {
                self.finder.tap = Some(tap);
                self.finder_log("finder-enter: tap started");
            }
            Err(e) => {
                eprintln!("ferrocast: Enter on Finder: {e}");
                self.finder_log(&format!("finder-enter: tap NOT started: {e}"));
                self.toast(services::FINDER_ENTER_NEEDS_ACCESS);
            }
        }
    }

    /// One line in ferrocast.log through the injected hook (never key content or file paths).
    fn finder_log(&self, line: &str) {
        (self.finder.hooks.log)(&self.store.root, line);
    }

    /// Give the swallowed Return back to Finder. The tap keeps passing Returns through for a
    /// moment longer, so the re-post can never meet a second interception even if the marker
    /// were lost on the way.
    fn finder_repost(&mut self) {
        (self.finder.hooks.repost)();
        // free the busy flag, but keep passing Returns through for 300 ms (restarts on each re-post)
        self.finder.inflight.end();
        self.finder.inflight.hold_from(Instant::now());
    }

    /// The tap swallowed a Return: read the Finder selection off the UI thread.
    fn on_finder_enter(&mut self) {
        if self.finder.tap.is_none() {
            // a late message from a tap that was just turned off: the key was consumed, give it back
            self.finder_repost();
            return;
        }
        if self.finder.busy {
            // one at a time: the extra Return is passed through, never queued
            (self.finder.hooks.repost)();
            return;
        }
        self.finder.busy = true;
        self.finder_log("finder-enter: Return intercepted, reading the Finder selection");
        let (bus, read) = (self.bus.clone(), self.finder.hooks.selection.clone());
        std::thread::spawn(move || {
            let sel =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| read())).unwrap_or_else(|_| Err("selection read failed".into()));
            bus.send(Msg::FinderEnterDone(sel));
        });
    }

    /// The selection read finished: open the Convert wheel, or give the Return back to Finder.
    fn on_finder_enter_done(&mut self, sel: Result<Vec<PathBuf>, String>) {
        self.finder.busy = false;
        use ferrocast_core::finder_enter::{outcome, EnterOutcome};
        match outcome(&sel) {
            EnterOutcome::OpenWheel(files) if !self.wheel_segments(ferrocast_core::wheel::WheelKind::Convert, &files).is_empty() => {
                self.finder_log(&format!("finder-enter: selection read ok, {} item(s) -> opening the wheel", files.len()));
                self.finder.inflight.end();
                self.show();
                self.reset_to_root();
                self.perform(Act::OpenWheel(files), true);
            }
            _ => {
                match &sel {
                    Err(e) => {
                        eprintln!("ferrocast: Enter on Finder: selection: {e}");
                        self.finder_log(&format!("finder-enter: selection read FAILED: {e} -> giving the Return back"));
                        let due = self.finder.last_failure_notice.is_none_or(|t| t.elapsed() >= Duration::from_secs(60));
                        if due {
                            self.finder.last_failure_notice = Some(Instant::now());
                            (self.finder.hooks.notify)("Ferrocast", &ferrocast_core::finder_enter::finder_read_failure_message(e));
                        }
                    }
                    Ok(f) if f.is_empty() => self.finder_log("finder-enter: selection read ok, empty -> giving the Return back"),
                    Ok(f) => self.finder_log(&format!(
                        "finder-enter: selection read ok, {} item(s), no conversion available -> giving the Return back",
                        f.len()
                    )),
                }
                self.finder_repost();
            }
        }
    }

    // ------------------------------------------------------------------ data

    /// The selected custom theme, when `settings.custom_theme` names a saved one.
    pub(crate) fn active_theme(&self) -> Option<&ferrocast_core::themes::CustomTheme> {
        let name = self.settings.custom_theme.as_deref()?;
        self.custom_themes.iter().find(|t| t.name.eq_ignore_ascii_case(name))
    }

    fn set_theme(&mut self) {
        use ferrocast_core::themes::ThemeBase;
        // a custom theme forces light or dark from its base (egui's own widgets and the Notes window follow it)
        let base = self.active_theme().map(|t| t.base);
        let (pref, notes) = match base {
            Some(ThemeBase::Dark) => (egui::ThemePreference::Dark, notes_appearance(Theme::Dark)),
            Some(ThemeBase::Light) => (egui::ThemePreference::Light, notes_appearance(Theme::Light)),
            None => (
                match self.settings.theme {
                    Theme::System => egui::ThemePreference::System,
                    Theme::Dark => egui::ThemePreference::Dark,
                    Theme::Light => egui::ThemePreference::Light,
                },
                notes_appearance(self.settings.theme),
            ),
        };
        self.ctx.set_theme(pref);
        (self.fx.notes_appearance)(notes);
    }

    /// Persist the custom themes (`themes.json`).
    pub(crate) fn save_themes(&mut self) {
        let v = ferrocast_core::themes::ThemeFile { themes: self.custom_themes.clone() };
        self.persist("themes.json", &v);
    }

    fn script_dirs(&self) -> Vec<PathBuf> {
        let mut v = vec![self.store.scripts_dir()];
        v.extend(self.settings.script_directories.iter().map(|d| {
            if let Some(r) = d.strip_prefix("~/") {
                dirs::home_dir().unwrap_or_default().join(r)
            } else {
                PathBuf::from(d)
            }
        }));
        v
    }

    fn reload_scripts(&mut self) {
        self.scripts = services::load_scripts(&self.script_dirs());
        self.rebuild();
    }

    fn save_settings(&mut self) {
        // a run in progress stops writing as soon as sync is turned off
        self.sync.flag.store(self.settings.sync_enabled, Ordering::SeqCst);
        let v = self.settings.clone();
        self.persist("settings.json", &v);
        *self.shared.settings.write().unwrap() = self.settings.clone();
        self.rebuild();
        self.sync_tiling();
    }

    /// Tell the tiling worker the current settings; the worker is only started once tiling or
    /// hover activation is switched on, so with the defaults nothing runs.
    fn sync_tiling(&mut self) {
        let s = &self.settings;
        let on = s.tiling_enabled || s.tiling_hover_activate;
        if self.tiling.is_none() && on {
            self.tiling = (self.fx.tiling_spawn)(&self.bus);
            if let Some(h) = &self.tiling {
                h.send(ferrocast_macos::tiling::Cmd::MinSizes(self.store.load("tiling-min-sizes.json")));
            }
        } else if !on {
            // dropping the handle ends the worker: nothing observes windows while tiling is off
            if self.tiling.take().is_some() {
                self.apply_overlay(ferrocast_macos::tiling::Overlay::default());
            }
        }
        let s = &self.settings;
        let cfg = ferrocast_macos::tiling::Config {
            enabled: s.tiling_enabled,
            paused: self.tiling_paused,
            layout: s.tiling_layout,
            display_layouts: s.tiling_display_layouts.iter().filter_map(|(k, v)| Some((k.parse().ok()?, *v))).collect(),
            gap: ferrocast_core::tiling::clamp_gap(s.tiling_gap),
            excluded: s.tiling_excluded.clone(),
            hover_activate: s.tiling_hover_activate,
            border: s.tiling_border,
            border_color: draw::parse_hex(&s.tiling_border_color).unwrap_or([89, 212, 153]),
            quick_actions: s.tiling_quick_actions,
        };
        if let Some(h) = &self.tiling {
            h.send(ferrocast_macos::tiling::Cmd::Config(cfg));
        }
        let (enabled, paused) = (s.tiling_enabled, self.tiling_paused);
        if self.settings.tiling_layout != self.layout_seen {
            self.layout_seen = self.settings.tiling_layout;
            self.tray_layout = self.layout_seen;
        }
        services::set_tray_tiling(self._tray.as_ref(), enabled.then_some(paused), self.tray_layout, self.tray_rec_shown);
        if self.tiling_hotkeys != enabled {
            self.tiling_hotkeys = enabled;
            self.apply_hotkeys();
        }
    }

    /// Draw (or clear) the border and the quick-action bar; also remembers which window the bar is for.
    fn apply_overlay(&mut self, o: ferrocast_macos::tiling::Overlay) {
        self.quick_window = o.bar.as_ref().map(|b| b.window);
        (self.fx.tiling_overlay)(&o);
    }

    pub(crate) fn run_tiling(&mut self, a: ferrocast_core::tiling::Action, pid: Option<i32>) {
        self.run_tiling_on(a, pid, None);
    }

    /// `window` is the tracked window a quick-action button was clicked on (`None`: the focused one).
    pub(crate) fn run_tiling_on(&mut self, a: ferrocast_core::tiling::Action, pid: Option<i32>, window: Option<u64>) {
        use ferrocast_core::tiling::Action;
        instance::log(
            &self.store.root,
            &format!("tiling command {} (pid {pid:?}, window {window:?}, worker running: {})", a.id(), self.tiling.is_some()),
        );
        use ferrocast_macos::tiling::Cmd;
        match a {
            Action::Toggle => {
                self.settings.tiling_enabled = !self.settings.tiling_enabled;
                self.save_settings();
                let msg = if self.settings.tiling_enabled { "Tiling on" } else { "Tiling off" };
                self.toast(msg);
            }
            Action::Pause => {
                self.tiling_paused = !self.tiling_paused;
                self.sync_tiling();
                let msg = if self.tiling_paused { "Tiling paused" } else { "Tiling resumed" };
                self.toast(msg);
            }
            _ if !self.settings.tiling_enabled => {
                services::notify("Window Tiling", "Turn on automatic tiling in Settings → Advanced first");
            }
            Action::Layout(n) => {
                if let Some(h) = &self.tiling {
                    h.send(Cmd::Layout { n, pid });
                }
            }
            Action::FocusMode if window.is_none() => {
                if let Some(h) = &self.tiling {
                    h.send(Cmd::Focus { pid });
                }
            }
            Action::FocusMode | Action::FloatToggle | Action::NextDisplay | Action::SwapMain | Action::ExcludeApp => {
                if let Some(h) = &self.tiling {
                    h.send(Cmd::Act { action: a, window, pid });
                }
            }
            Action::Move(d) => {
                if let Some(h) = &self.tiling {
                    h.send(Cmd::Move { dir: d, pid });
                }
            }
        }
    }

    fn tiling_event(&mut self, e: ferrocast_macos::tiling::Event) {
        use ferrocast_macos::tiling::Event;
        match e {
            Event::DisplayLayout { display, layout } => {
                // store first: the config sent to the worker must carry the new layout, or it reverts
                self.settings.tiling_display_layouts.insert(display.to_string(), layout);
                self.tray_layout = layout;
                self.sync_tiling();
                let v = self.settings.clone();
                self.persist("settings.json", &v);
                *self.shared.settings.write().unwrap() = self.settings.clone();
                let name = ferrocast_core::tiling::Layout::from_number(layout).map_or("", |l| l.name());
                self.toast =
                    Some((trf("Layout {layout}: {name}", &[("layout", &layout.to_string()), ("name", &tr(name))]), Instant::now()));
            }
            Event::MinSizes(m) => self.persist("tiling-min-sizes.json", &m),
            Event::Overlay(o) => self.apply_overlay(o),
            Event::Log(l) => instance::log(&self.store.root, &l),
            Event::Exclude(app) => {
                if !self.settings.tiling_excluded.contains(&app) {
                    self.settings.tiling_excluded.push(app.clone());
                    self.save_settings();
                }
                self.toast(trf("{app} is no longer tiled", &[("app", &app)]));
            }
            Event::Notice(m) => services::notify("Window Tiling", &m),
            Event::StageManager(on) => {
                let m = if on { "Stage Manager is on: tiling is paused" } else { "Stage Manager is off: tiling resumed" };
                self.toast(m);
            }
        }
    }
    fn save_snippets(&mut self) {
        let v = self.snippets.clone();
        self.persist("snippets.json", &v);
        *self.shared.snippets.write().unwrap() = self.snippets.clone();
        self.shared.refresh_keywords();
    }
    fn save_quicklinks(&mut self) {
        let v = self.quicklinks.clone();
        self.persist("quicklinks.json", &v);
        self.rebuild();
    }
    fn send_ai(&mut self) {
        let run = self.next_run;
        self.next_run += 1;
        let (history, question) = match self.stack.last_mut() {
            Some(View::Ai { prompt, asked, answer, history, busy, run: r }) if !prompt.trim().is_empty() && !*busy => {
                // The previous turn becomes context unless it failed.
                if !asked.is_empty() && !answer.is_empty() && !answer.contains("⚠ ") {
                    history.push((std::mem::take(asked), std::mem::take(answer)));
                }
                *busy = true;
                *r = run;
                *asked = std::mem::take(prompt);
                answer.clear();
                (history.clone(), asked.clone())
            }
            _ => return,
        };
        if self.settings.ai_provider == ferrocast_core::models::AiProvider::ClaudeCode {
            // The warm session already holds the conversation; send only the new turn.
            if self.claude.as_ref().is_none_or(|c| !c.is_alive()) {
                match services::ClaudeSession::start(&self.settings, self.bus.clone()) {
                    Ok(c) => self.claude = Some(c),
                    Err(e) => {
                        self.bus.send(Msg::AiDone { run, error: Some(e) });
                        return;
                    }
                }
            }
            if let Err(e) = self.claude.as_ref().unwrap().ask(run, &question) {
                self.bus.send(Msg::AiDone { run, error: Some(e) });
            }
            return;
        }
        services::ask_ai(&self.settings, history, question, run, self.bus.clone());
    }

    /// Start Claude Code in the background so the first answer does not pay its boot time.
    fn warm_claude(&mut self) {
        if self.settings.ai_provider == ferrocast_core::models::AiProvider::ClaudeCode && self.claude.as_ref().is_none_or(|c| !c.is_alive())
        {
            if let Ok(c) = services::ClaudeSession::start(&self.settings, self.bus.clone()) {
                self.claude = Some(c);
            }
        }
    }

    /// Drop the warm session once Ask AI is no longer on the stack.
    fn cool_claude(&mut self) {
        if !self.stack.iter().any(|v| matches!(v, View::Ai { .. })) {
            if let Some(c) = self.claude.take() {
                c.kill();
            }
        }
    }

    fn save_notes(&mut self) {
        let v = self.notes.clone();
        self.persist("notes.json", &v);
    }
    fn save_frecency(&self) {
        if let Err(e) = self.store.save("frecency.json", &self.frecency) {
            instance::log(&self.store.root, &format!("saving frecency.json failed: {e:#}"));
        }
    }
    fn apply_hotkeys(&mut self) {
        let mut lines = vec![];
        if let Some(h) = self.hotkeys.as_mut() {
            let errors = h.apply(&self.settings);
            if let Some(e) = errors.first() {
                self.toast = Some((format!("Hotkey error: {e}"), Instant::now()));
            }
            let (log, mut failed) = services::tiling_hotkey_report(&h.results);
            failed.extend(h.skipped.iter().cloned());
            if self.settings.tiling_enabled || !failed.is_empty() {
                lines.push(log);
            }
            for e in &errors {
                lines.push(format!("hotkey error: {e}"));
            }
            self.tiling_hotkey_issues = failed;
        }
        for l in lines {
            instance::log(&self.store.root, &l);
        }
    }

    /// Rebuild the cached list of root-searchable items.
    /// Load the Unicode fallback font the first time non-Latin text needs it.
    pub(crate) fn ensure_wide_font(&mut self, text: &str) {
        if !self.wide_font && needs_wide_font(text) {
            self.wide_font = true;
            load_fonts(&self.ctx, true);
        }
    }

    /// Switch the interface language now: strings are looked up each frame, so only what is cached
    /// (launcher rows, menus) is rebuilt, and the wide font is added for Japanese and Chinese.
    pub(crate) fn apply_language(&mut self) {
        let lang = resolve_language(&self.settings.language);
        if lang == ferrocast_core::i18n::language() {
            return;
        }
        ferrocast_core::i18n::set_language(lang);
        if lang.needs_wide_font() && !self.wide_font {
            self.wide_font = true;
            load_fonts(&self.ctx, true);
        }
        if let Some(tray) = self._tray.as_ref() {
            services::refresh_tray_menu(tray, self.tray_rec_shown);
        }
        self.rebuild();
        self.ctx.request_repaint();
    }

    fn rebuild(&mut self) {
        self.sec_dirty = true;
        let mut v = vec![];
        for a in &self.apps {
            v.push(Item {
                id: format!("app:{}", a.bundle_id.clone().unwrap_or_else(|| a.path.to_string_lossy().to_string())),
                title: a.name.clone(),
                subtitle: String::new(),
                icon: Icon::App(a.path.to_string_lossy().to_string()),
                accessory: tr("Application").into_owned(),
                keywords: a.keywords.clone(),
                kind: Kind::App { path: a.path.to_string_lossy().to_string() },
            });
        }
        for b in Builtin::ALL {
            if self.capture_hidden(*b) {
                continue;
            }
            let (g, c) = b.glyph();
            v.push(Item {
                id: b.id(),
                title: tr(b.title()).into_owned(),
                subtitle: if b.extension() == b.title() { String::new() } else { tr(b.extension()).into_owned() },
                icon: Icon::Glyph(g, c),
                accessory: tr("Command").into_owned(),
                keywords: with_english_title(b.keywords(), b.title()),
                kind: Kind::Builtin(*b),
            });
        }
        for c in windows::WindowCommand::ALL {
            v.push(Item {
                id: c.id(),
                title: tr(c.title()).into_owned(),
                subtitle: tr("Window Management").into_owned(),
                icon: Icon::Glyph("sf:rectangle.lefthalf.filled", [89, 212, 153]),
                accessory: tr("Command").into_owned(),
                keywords: with_english_title(vec!["window".into(), "resize".into(), "move".into()], c.title()),
                kind: Kind::Window(*c),
            });
        }
        for a in ferrocast_core::tiling::Action::ALL {
            v.push(Item {
                id: a.id(),
                title: tr(&a.title()).into_owned(),
                subtitle: tr("Window Tiling").into_owned(),
                icon: Icon::Glyph("sf:rectangle.split.2x2", [89, 212, 153]),
                accessory: tr("Command").into_owned(),
                keywords: with_english_title(vec!["window".into(), "tile".into(), "tiling".into(), "layout".into()], &a.title()),
                kind: Kind::Tiling(a),
            });
        }
        for c in system::SystemCommand::ALL {
            v.push(Item {
                id: c.id(),
                title: tr(&c.title()).into_owned(),
                subtitle: tr("System").into_owned(),
                icon: Icon::Glyph(system_symbol(c), [90, 90, 105]),
                accessory: tr("Command").into_owned(),
                keywords: with_english_title(c.keywords().iter().map(|s| s.to_string()).collect(), &c.title()),
                kind: Kind::System(*c),
            });
        }
        for q in &self.quicklinks {
            v.push(Item {
                id: format!("quicklink:{}", q.id),
                title: q.name.clone(),
                subtitle: q.link.clone(),
                icon: Icon::Glyph("sf:link", [86, 194, 255]),
                accessory: tr("Quicklink").into_owned(),
                keywords: vec![],
                kind: Kind::Quicklink { id: q.id.clone(), arg: None },
            });
        }
        for (i, s) in self.scripts.iter().enumerate() {
            let sub = self.inline.get(&s.path).map(|(t, _)| t.clone()).or_else(|| s.package_name.clone()).unwrap_or_default();
            v.push(Item {
                id: s.id(),
                title: s.title.clone(),
                subtitle: sub,
                icon: match s.icon.clone().filter(|i| i.chars().count() <= 2 && !i.is_ascii()) {
                    Some(e) => Icon::Emoji(e),
                    None => Icon::Glyph("sf:terminal", [60, 60, 70]),
                },
                accessory: tr("Script Command").into_owned(),
                keywords: s.description.iter().cloned().collect(),
                kind: Kind::Script(i),
            });
        }
        self.ext_ensure_loaded();
        v.extend(self.ext_command_items());
        v.retain(|i| !self.settings.is_disabled(&i.id));
        self.base = v;
    }

    // ------------------------------------------------------------- full screen

    /// The editor's Full screen button and ⌃⌘F. Not `ViewportCommand::Fullscreen`: the window is a borderless,
    /// non-resizable, always-on-top panel, where macOS native full screen does not apply (see
    /// `editor_ctl::WindowMode`), so the window is maximised to its screen's visible frame instead; a second
    /// press goes back to the editor-sized window.
    pub(crate) fn toggle_fullscreen(&mut self) {
        let current = self.ctx.input(|i| i.viewport().outer_rect.or(i.viewport().inner_rect));
        let screen = self.window_screen();
        let normal = Vec2::new(ui::WIDTH, self.cur_height);
        if let Some(cmd) = self.win.toggle(current, screen, normal) {
            self.send_window(cmd);
        }
    }

    /// Back to the normal size when maximised; a no-op otherwise, so it is safe to call whenever.
    pub(crate) fn leave_fullscreen(&mut self) {
        if let Some(cmd) = self.win.leave() {
            self.send_window(cmd);
        }
    }

    /// Per frame: the editor view grows the window once (see `editor_ctl::WindowMode`) and every other view
    /// (Esc, Back, a hidden launcher) puts the launcher's size and place back once. Quiet in between.
    pub(crate) fn sync_window(&mut self) {
        // a hidden launcher is not showing the editor: its window is already back to the launcher's size
        let in_editor = self.visible && matches!(self.view(), View::Editor { .. });
        let normal = Vec2::new(ui::WIDTH, self.cur_height);
        let ctx = self.ctx.clone();
        let screen = self.window_screen();
        let cmd = self.win.sync(in_editor, || ctx.input(|i| i.viewport().outer_rect.or(i.viewport().inner_rect)), || screen, normal);
        if let Some(cmd) = cmd {
            self.send_window(cmd);
        }
    }

    fn send_window(&self, cmd: crate::editor_ctl::WindowCmd) {
        use crate::editor_ctl::WindowCmd;
        match cmd {
            WindowCmd::Place { pos, size } => {
                self.ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos));
                self.ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
            }
            WindowCmd::Restore { pos, size } => {
                self.ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
                if let Some(p) = pos.or_else(launcher_origin) {
                    self.ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(p));
                }
            }
        }
    }

    /// The visible frame (below the menu bar, beside the Dock) of the screen the window is on.
    fn window_screen(&self) -> Option<Rect> {
        // tests never ask the real screens: a test sets `test-screen` or has none
        #[cfg(test)]
        return self.ctx.data(|d| d.get_temp::<Rect>(Id::new("test-screen")));
        #[cfg(not(test))]
        key_window_screen().or_else(|| mouse_top_left().map(|(_, r)| r))
    }

    // ------------------------------------------------------------- show/hide

    fn show(&mut self) {
        self.sec_dirty = true;
        self.sync_on_show();
        if self.hidden_at.elapsed() > Duration::from_secs(90) {
            self.stack = vec![View::Root];
            self.query.clear();
        }
        self.prev_pid = running::frontmost().map(|f| f.pid).filter(|p| *p != std::process::id() as i32);
        self.visible = true;
        self.had_focus = false;
        self.shown_at = Instant::now();
        self.show_t = Some(Instant::now());
        self.panel = None;
        self.shared.visible.store(true, Ordering::Relaxed);
        if let Some(p) = launcher_origin() {
            self.ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(p));
        }
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let app = objc2_app_kit::NSApplication::sharedApplication(mtm);
            app.unhide(None);
            #[allow(deprecated)]
            app.activateIgnoringOtherApps(true);
        }
        self.ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        self.ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        if self.apps_at.elapsed() > Duration::from_secs(600) {
            self.apps_at = Instant::now();
            let b = self.bus.clone();
            let apps_cache = self.store.path("apps-cache.plist");
            std::thread::spawn(move || b.send(Msg::Apps(apps::list_apps_cached(Some(&apps_cache)))));
        }
        self.ctx.request_repaint();
    }

    /// Ends the first-run guard (window level back to always-on-top) and starts the services that
    /// were held back so no permission prompt appeared before the tour reached its slide.
    fn finish_onboarding(&mut self) {
        if !self.onboarding {
            return;
        }
        self.onboarding = false;
        Self::set_dock_icon(false);
        self.ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(egui::WindowLevel::AlwaysOnTop));
        services::start_expansion(self.shared.clone());
    }

    fn hide(&mut self) {
        self.editor_flush();
        // a maximised editor goes back to the launcher's size before the window disappears
        self.leave_fullscreen();
        self.visible = false;
        self.hidden_at = Instant::now();
        self.shared.visible.store(false, Ordering::Relaxed);
        self.ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        // hiding the whole app would also hide the floating shelf and Notes panels
        // (the same goes for every other floating panel: a pinned capture, the recording HUD)
        if let Some(mtm) = objc2::MainThreadMarker::new()
            .filter(|m| !ferrocast_macos::shelf::is_visible() && !ferrocast_macos::notes::is_visible() && !other_panel_visible(*m))
        {
            objc2_app_kit::NSApplication::sharedApplication(mtm).hide(None);
        }
        if let Some(pid) = self.prev_pid {
            running::activate(pid);
        }
    }

    /// Activation policy of the app: during the first-run tour it gets a Dock icon, so the user can
    /// come back to it after System Settings covers it; afterwards it is a menu-less accessory again.
    fn set_dock_icon(on: bool) {
        use objc2_app_kit::NSApplicationActivationPolicy as P;
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            let policy = if on { P::Regular } else { P::Accessory };
            objc2_app_kit::NSApplication::sharedApplication(mtm).setActivationPolicy(policy);
        }
    }

    fn toggle(&mut self) {
        if self.onboarding && self.visible {
            // the tour never hides: the hotkey just brings it back in front of System Settings
            if let Some(mtm) = objc2::MainThreadMarker::new() {
                #[allow(deprecated)]
                objc2_app_kit::NSApplication::sharedApplication(mtm).activateIgnoringOtherApps(true);
            }
            self.ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        } else if self.visible {
            self.hide();
        } else {
            self.show();
        }
    }

    /// Persist a data file; a failed write is surfaced instead of dropped on the floor.
    fn persist<T: serde::Serialize>(&mut self, name: &str, value: &T) {
        if let Err(e) = self.store.save(name, value) {
            instance::log(&self.store.root, &format!("saving {name} failed: {e:#}"));
            self.toast(trf("Could not save {name}: {e}", &[("name", name), ("e", &e.to_string())]));
        }
        self.sync_mark(name);
    }

    // ------------------------------------------------------------ Notion (Settings → Integrations)

    /// Save the token typed in Settings (trimmed; empty removes it). The token only ever goes to the Keychain.
    fn notion_save_token(&mut self, raw: &str) {
        let token = raw.trim();
        let ok = notion_io::set_token(token);
        self.ctx.data_mut(|d| d.insert_temp(Id::new("notion_has_token"), !token.is_empty() && ok));
        self.toast(if !ok {
            "Could not update Keychain"
        } else if token.is_empty() {
            "Notion token removed"
        } else {
            "Notion token saved to Keychain"
        });
    }

    /// GET users/me on a thread; the answer comes back as `Msg::NotionTested`.
    fn notion_test(&mut self) {
        let (read, url, bus) = (notion_io::token_reader(), notion_io::url(ferrocast_core::notion::me_url()), self.bus.clone());
        std::thread::spawn(move || {
            let r = match read() {
                None => Err(NOTION_NO_TOKEN.to_string()),
                Some(t) => notion_io::request(&t, "GET", &url, None)
                    .and_then(|b| ferrocast_core::notion::parse_me(&b))
                    .map_err(|e| e.user_message()),
            };
            bus.send(Msg::NotionTested(r));
        });
    }

    fn handle_notion_tested(&mut self, r: Result<ferrocast_core::notion::BotInfo, String>) {
        self.toast(match r {
            Ok(b) => match b.workspace.filter(|w| !w.is_empty()) {
                Some(w) => trf("Connected to Notion as {name} · {workspace}", &[("name", &b.name), ("workspace", &w)]),
                None => trf("Connected to Notion as {name}", &[("name", &b.name)]),
            },
            Err(e) => e,
        });
    }

    /// POST search (8 results) on a thread; the answer comes back as `Msg::NotionSearched`.
    fn notion_search(&mut self) {
        self.notion.request += 1;
        self.notion.searching = true;
        let request = self.notion.request;
        let body = ferrocast_core::notion::search_body(self.notion.query.trim(), None, 8);
        let (read, url, bus) = (notion_io::token_reader(), notion_io::url(ferrocast_core::notion::search_url()), self.bus.clone());
        std::thread::spawn(move || {
            let result = match read() {
                None => Err(NOTION_NO_TOKEN.to_string()),
                Some(t) => notion_io::request(&t, "POST", &url, Some(&body))
                    .and_then(|b| ferrocast_core::notion::parse_search(&b))
                    .map_err(|e| e.user_message()),
            };
            bus.send(Msg::NotionSearched { request, result });
        });
    }

    fn handle_notion_searched(&mut self, request: u64, result: Result<ferrocast_core::notion::SearchPage, String>) {
        if request != self.notion.request {
            return; // a newer search is in flight
        }
        self.notion.searching = false;
        match result {
            Ok(page) => {
                self.notion.results = page.items.into_iter().take(8).collect();
                if self.notion.results.is_empty() {
                    self.toast("Nothing found. Share the page with your integration: ••• → Connections.");
                }
            }
            Err(e) => self.toast(e),
        }
    }

    /// Make result `i` the capture target.
    fn notion_pick(&mut self, i: usize) {
        let Some(item) = self.notion.results.get(i).cloned() else { return };
        self.notion_set_target(item);
    }

    fn notion_set_target(&mut self, item: ferrocast_core::notion::NotionItem) {
        let kind = if item.kind == ferrocast_core::notion::ItemKind::Page { "page" } else { "data_source" };
        let mut s = self.settings.clone();
        s.notion_target = Some(ferrocast_core::models::NotionTarget { id: item.id, kind: kind.into(), title: item.title.clone() });
        self.commit_settings(s);
        self.toast(trf("Capture target set to {name}", &[("name", &item.title)]));
    }

    // ------------------------------------------------------------ Search Notion (command)

    fn notion_view_open(&mut self) {
        self.nv = NotionView::default();
    }

    /// Debounce and fire the Search Notion request. `now` is the egui time in seconds: the first
    /// request (the 20 latest pages) goes out at once, later ones 250 ms after the last keystroke.
    pub(crate) fn tick_notion_view(&mut self, now: f64) {
        if !self.visible || !matches!(self.view(), View::NotionSearch) || self.nv.connected == Some(false) {
            return;
        }
        let q = self.query.trim().to_string();
        if self.nv.asked.as_deref() == Some(q.as_str()) {
            return;
        }
        if self.nv.asked.is_some() {
            if self.nv.seen != q {
                self.nv.seen = q.clone();
                self.nv.seen_at = now;
            }
            let wait = self.nv.seen_at + NOTION_DEBOUNCE - now;
            if wait > 0.0 {
                self.ctx.request_repaint_after(Duration::from_secs_f64(wait + 0.01));
                return;
            }
        }
        self.notion_view_request(q, None);
    }

    /// POST search on a thread; with `cursor` the page is appended to the list. No token, no request.
    fn notion_view_request(&mut self, query: String, cursor: Option<String>) {
        self.nv.request += 1;
        self.nv.searching = true;
        self.nv.asked = Some(query.clone());
        let (request, append) = (self.nv.request, cursor.is_some());
        let body = ferrocast_core::notion::search_body(&query, cursor.as_deref(), NOTION_PAGE);
        let (read, url, bus) = (notion_io::token_reader(), notion_io::url(ferrocast_core::notion::search_url()), self.bus.clone());
        std::thread::spawn(move || {
            let result = match read() {
                None => Err(services::NotionViewError::NoToken),
                Some(t) => notion_io::request(&t, "POST", &url, Some(&body))
                    .and_then(|b| ferrocast_core::notion::parse_search(&b))
                    .map_err(|e| services::NotionViewError::Failed(e.user_message())),
            };
            bus.send(Msg::NotionViewSearched { request, append, result });
        });
    }

    fn notion_load_more(&mut self) {
        if self.nv.searching {
            return;
        }
        if let Some(c) = self.nv.next_cursor.clone() {
            let q = self.nv.shown.clone();
            self.notion_view_request(q, Some(c));
        }
    }

    fn handle_notion_view_searched(
        &mut self,
        request: u64,
        append: bool,
        result: Result<ferrocast_core::notion::SearchPage, services::NotionViewError>,
    ) {
        if matches!(result, Err(services::NotionViewError::NoToken)) {
            self.nv.connected = Some(false);
            self.nv.searching = false;
            self.nv.items.clear();
            self.nv.has_more = false;
            self.invalidate_sections();
            return;
        }
        if request != self.nv.request {
            return;
        }
        self.nv.searching = false;
        self.nv.connected = Some(true);
        match result {
            Ok(page) => {
                if append {
                    self.nv.items.extend(page.items);
                } else {
                    self.nv.items = page.items;
                    self.nv.shown = self.nv.asked.clone().unwrap_or_default();
                    self.selected = 0;
                }
                self.nv.has_more = page.has_more && page.next_cursor.is_some();
                self.nv.next_cursor = page.next_cursor;
                self.invalidate_sections();
            }
            Err(services::NotionViewError::Failed(msg)) => self.toast(msg),
            Err(services::NotionViewError::NoToken) => {}
        }
    }

    fn notion_open_url(&mut self, url: &str) {
        if url.starts_with("https://") || url.starts_with("http://") {
            notion_io::open_url(url);
        } else {
            self.toast("This Notion result has no link to open.");
            return;
        }
        self.hide();
        self.reset_to_root();
    }

    fn notion_view_sections(&self) -> Vec<Section> {
        if self.nv.connected == Some(false) {
            return vec![Section {
                title: tr("Connect Notion to search your pages").into_owned(),
                items: vec![Item {
                    id: "notion:connect".into(),
                    title: tr("Open Settings → Integrations").into_owned(),
                    subtitle: String::new(),
                    icon: Icon::Glyph("sf:gearshape", [120, 120, 130]),
                    accessory: String::new(),
                    keywords: vec![],
                    kind: Kind::NotionConnect,
                }],
            }];
        }
        let now = chrono::Utc::now();
        let mut items: Vec<Item> = self
            .nv
            .items
            .iter()
            .map(|n| {
                let database = n.kind == ferrocast_core::notion::ItemKind::DataSource;
                Item {
                    id: format!("notion:{}", n.id),
                    title: if n.title.is_empty() { "Untitled".into() } else { n.title.clone() },
                    subtitle: if database { "Database" } else { "Page" }.into(),
                    icon: match &n.icon_emoji {
                        Some(e) => Icon::Emoji(e.clone()),
                        None if database => Icon::Glyph("sf:tablecells", [120, 120, 130]),
                        None => Icon::Glyph("sf:doc.text", [120, 120, 130]),
                    },
                    accessory: n.last_edited.as_deref().map(|t| notion_io::relative_time(t, now)).unwrap_or_default(),
                    keywords: vec![],
                    kind: Kind::Notion(n.clone()),
                }
            })
            .collect();
        if self.nv.has_more {
            items.push(Item {
                id: "notion:more".into(),
                title: tr("Load more").into_owned(),
                subtitle: String::new(),
                icon: Icon::Glyph("sf:ellipsis", [120, 120, 130]),
                accessory: String::new(),
                keywords: vec![],
                kind: Kind::NotionLoadMore,
            });
        }
        if items.is_empty() {
            return vec![];
        }
        vec![Section { title: tr(if self.nv.shown.is_empty() { "Recently edited" } else { "Results" }).into_owned(), items }]
    }

    // ------------------------------------------------------------ Search Obsidian (command)

    /// Open the view: forget what the last visit asked, keep the cached index, and refresh it.
    fn obsidian_view_open(&mut self) {
        let ov = &mut self.ov;
        ov.requested_for = None;
        ov.content = None;
        ov.content_asked = None;
        ov.content_request += 1;
        ov.seen.clear();
        ov.preview_for = None;
        ov.preview = None;
        ov.pv_seen = None;
        self.obsidian_ensure_index();
    }

    /// Index the vault on a thread unless the newest request already covers it. Never walks on the
    /// UI thread: a cloud-synced vault can take seconds on first access.
    fn obsidian_ensure_index(&mut self) {
        let Some(vault) = self.obsidian_vault() else { return };
        if self.ov.requested_for.as_ref() == Some(&vault.path) {
            return;
        }
        self.ov.requested_for = Some(vault.path.clone());
        self.ov.index_request += 1;
        self.ov.indexing = true;
        let (request, bus, root) = (self.ov.index_request, self.bus.clone(), vault.path);
        std::thread::spawn(move || {
            let notes = root.is_dir().then(|| ferrocast_core::obsidian::index_vault(&root));
            bus.send(Msg::ObsidianIndexed { request, vault: root, notes });
        });
    }

    fn handle_obsidian_indexed(&mut self, request: u64, vault: PathBuf, notes: Option<Vec<ferrocast_core::obsidian::NoteEntry>>) {
        if request != self.ov.index_request || self.settings.obsidian_vault.as_ref() != Some(&vault) {
            return;
        }
        self.ov.indexing = false;
        match notes {
            Some(n) => {
                self.ov.missing = None;
                self.ov.cache = Some((vault, Arc::new(n)));
            }
            None => {
                self.ov.missing = Some(vault);
                self.ov.cache = None;
            }
        }
        self.invalidate_sections();
    }

    /// The cached index when it belongs to the chosen vault.
    fn obsidian_entries(&self) -> Option<Arc<Vec<ferrocast_core::obsidian::NoteEntry>>> {
        let vault = self.settings.obsidian_vault.as_ref()?;
        self.ov.cache.as_ref().filter(|(p, _)| p == vault).map(|(_, e)| e.clone())
    }

    fn obsidian_entry(&self, rel: &str) -> Option<ferrocast_core::obsidian::NoteEntry> {
        self.obsidian_entries()?.iter().find(|e| e.rel_path == rel).cloned()
    }

    /// Debounce the content scan, keep the index fresh and read the preview of the selected note.
    /// `now` is the egui time in seconds.
    pub(crate) fn tick_obsidian_view(&mut self, now: f64) {
        if !self.visible || !matches!(self.view(), View::ObsidianSearch) {
            return;
        }
        // the vault may have been chosen in Settings while this view waited underneath
        self.obsidian_ensure_index();
        let q = self.query.trim().to_string();
        if q.chars().count() < 3 {
            if self.ov.content.is_some() || self.ov.content_asked.is_some() {
                self.ov.content = None;
                self.ov.content_asked = None;
                self.ov.content_request += 1; // an answer still in flight is stale now
                self.invalidate_sections();
            }
        } else if self.ov.content_asked.as_deref() != Some(q.as_str()) {
            if self.ov.seen != q {
                self.ov.seen = q.clone();
                self.ov.seen_at = now;
            }
            let wait = self.ov.seen_at + OBSIDIAN_DEBOUNCE - now;
            if wait > 0.0 {
                self.ctx.request_repaint_after(Duration::from_secs_f64(wait + 0.01));
            } else if let Some(entries) = self.obsidian_entries() {
                self.obsidian_content_request(q, entries);
            }
        }
        self.tick_obsidian_preview(now);
    }

    fn obsidian_content_request(&mut self, query: String, entries: Arc<Vec<ferrocast_core::obsidian::NoteEntry>>) {
        let Some(vault) = self.obsidian_vault() else { return };
        self.ov.content_request += 1;
        self.ov.content_asked = Some(query.clone());
        let (request, bus) = (self.ov.content_request, self.bus.clone());
        std::thread::spawn(move || {
            let hits = services::content_scan(&vault.path, &entries, &query, OBSIDIAN_LIMIT, &services::is_dataless);
            bus.send(Msg::ObsidianContent { request, query, hits });
        });
    }

    fn handle_obsidian_content(&mut self, request: u64, query: String, hits: Vec<String>) {
        if request != self.ov.content_request {
            return;
        }
        self.ov.content = Some((query, hits));
        self.invalidate_sections();
    }

    /// Read the selected note once the selection has rested for a moment (holding an arrow key must not
    /// read, or make a cloud provider download, every note it passes).
    fn tick_obsidian_preview(&mut self, now: f64) {
        if self.ov.hide_preview {
            return;
        }
        let rel = self.sections_cached().iter().flat_map(|s| s.items.iter()).nth(self.selected).and_then(|i| match &i.kind {
            Kind::ObsidianNote(rel) => Some(rel.clone()),
            _ => None,
        });
        let Some(rel) = rel else {
            self.ov.pv_seen = None;
            return;
        };
        if self.ov.preview_for.as_deref() == Some(rel.as_str()) {
            self.ov.pv_seen = None;
            return;
        }
        if self.ov.pv_seen.as_deref() != Some(rel.as_str()) {
            self.ov.pv_seen = Some(rel.clone());
            self.ov.pv_seen_at = now;
        }
        let wait = self.ov.pv_seen_at + OBSIDIAN_PREVIEW_PAUSE - now;
        if wait > 0.0 {
            self.ctx.request_repaint_after(Duration::from_secs_f64(wait + 0.01));
            return;
        }
        let Some(vault) = self.obsidian_vault() else { return };
        self.ov.preview_request += 1;
        self.ov.preview_for = Some(rel.clone());
        self.ov.preview = None;
        self.ov.pv_seen = None;
        let (request, bus, path) = (self.ov.preview_request, self.bus.clone(), vault.path.join(&rel));
        std::thread::spawn(move || bus.send(Msg::ObsidianPreview { request, result: services::read_preview(&path) }));
    }

    fn handle_obsidian_preview(&mut self, request: u64, result: Result<Vec<ferrocast_core::obsidian::Block>, ()>) {
        if request != self.ov.preview_request {
            return;
        }
        if let Some(rel) = self.ov.preview_for.clone() {
            self.ov.preview = Some((rel, result));
        }
    }

    pub(crate) fn obsidian_toggle_preview(&mut self) {
        self.ov.hide_preview = !self.ov.hide_preview;
    }

    fn obsidian_view_sections(&self) -> Vec<Section> {
        let setup = |title: &str| {
            vec![Section {
                title: tr(title).into_owned(),
                items: vec![Item {
                    id: "obsidian:setup".into(),
                    title: tr("Open Settings → Integrations").into_owned(),
                    subtitle: String::new(),
                    icon: Icon::Glyph("sf:gearshape", [120, 120, 130]),
                    accessory: String::new(),
                    keywords: vec![],
                    kind: Kind::ObsidianSetup,
                }],
            }]
        };
        let Some(vault) = self.obsidian_vault() else { return setup("Choose your Obsidian vault") };
        if self.ov.missing.as_ref() == Some(&vault.path) {
            return setup("The vault folder was not found");
        }
        let Some(entries) = self.obsidian_entries() else { return vec![] };
        let q = self.query.trim();
        let now = chrono::Utc::now();
        let row = |e: &ferrocast_core::obsidian::NoteEntry| Item {
            id: format!("obsidian:{}", e.rel_path),
            title: e.title.clone(),
            subtitle: e.rel_path.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default(),
            icon: Icon::Glyph("sf:doc.text", [124, 58, 237]),
            accessory: chrono::DateTime::from_timestamp(e.modified as i64, 0)
                .map(|t| notion_io::relative_time(&t.to_rfc3339(), now))
                .unwrap_or_default(),
            keywords: vec![],
            kind: Kind::ObsidianNote(e.rel_path.clone()),
        };
        let found = ferrocast_core::obsidian::search(&entries, q, OBSIDIAN_LIMIT);
        let mut out = vec![];
        if !found.is_empty() {
            out.push(Section {
                title: tr(if q.is_empty() { "Recent" } else { "Notes" }).into_owned(),
                items: found.iter().map(|&i| row(&entries[i])).collect(),
            });
        }
        if q.chars().count() >= 3 {
            if let Some((_, rels)) = self.ov.content.as_ref().filter(|(cq, _)| cq == q) {
                let shown: std::collections::HashSet<&str> = found.iter().map(|&i| entries[i].rel_path.as_str()).collect();
                let items: Vec<Item> = rels
                    .iter()
                    .filter(|r| !shown.contains(r.as_str()))
                    .filter_map(|r| entries.iter().find(|e| &e.rel_path == r))
                    .map(row)
                    .collect();
                if !items.is_empty() {
                    out.push(Section { title: tr("In notes").into_owned(), items });
                }
            }
        }
        out
    }

    /// Enter: open the note in Obsidian. Obsidian known to be missing: say so and open nothing. Not yet
    /// detected (`None`): try anyway, the system answers if nothing handles `obsidian://`.
    fn obsidian_open_note(&mut self, rel: &str) {
        let Some(vault) = self.obsidian_vault() else { return };
        self.obsidian_open_uri(ferrocast_core::obsidian::uri_open(&vault.name, rel));
    }

    fn obsidian_search_in_app(&mut self, query: &str) {
        let Some(vault) = self.obsidian_vault() else { return };
        if query.trim().is_empty() {
            self.toast("Type something to search for in Obsidian.");
            return;
        }
        self.obsidian_open_uri(ferrocast_core::obsidian::uri_search(&vault.name, query));
    }

    fn obsidian_open_uri(&mut self, uri: String) {
        if self.obsidian_installed() == Some(false) {
            self.toast("Obsidian is not installed. Download it from obsidian.md.");
            return;
        }
        if !ferrocast_core::obsidian::uri_len_ok(&uri) {
            self.toast("That link is too long for Obsidian (over 8,000 characters).");
            return;
        }
        notion_io::open_url(&uri);
        self.hide();
        self.reset_to_root();
    }

    // ------------------------------------------------------------ Add to Notion (commands)

    fn notion_open_settings_tab(&mut self) {
        self.ctx.data_mut(|d| d.insert_temp(Id::new("settings_tab"), draw::INTEGRATIONS_TAB));
        self.push(View::Settings);
    }

    /// Token and target checks shared by both commands: the target to write to, or a toast and
    /// Settings → Integrations when something is missing. `one_off` replaces the stored capture target.
    fn notion_add_target(&mut self, one_off: Option<ferrocast_core::models::NotionTarget>) -> Option<ferrocast_core::models::NotionTarget> {
        if !notion_io::has_token() {
            self.toast("Connect Notion first: Settings → Integrations.");
            self.notion_open_settings_tab();
            return None;
        }
        let target = one_off.or_else(|| self.settings.notion_target.clone()).filter(|t| !t.id.is_empty());
        if target.is_none() {
            self.toast("Choose a capture target in Settings → Integrations.");
            self.notion_open_settings_tab();
        }
        target
    }

    fn notion_add_open(&mut self, one_off: Option<ferrocast_core::models::NotionTarget>) {
        if let Some(target) = self.notion_add_target(one_off) {
            self.push(View::NotionAdd { title: String::new(), text: String::new(), target });
        }
    }

    fn notion_add_clipboard(&mut self) {
        let Some(target) = self.notion_add_target(None) else { return };
        match notion_io::read_clipboard().filter(|t| !t.trim().is_empty()) {
            Some(text) => self.notion_add_start(notion_io::Capture { target, title: String::new(), text }),
            None => self.toast("The clipboard has no text."),
        }
    }

    /// Send the form (⌘↵). The text stays in the form until Notion accepted it.
    pub(crate) fn notion_add_submit(&mut self) {
        let View::NotionAdd { title, text, target } = self.view().clone() else { return };
        if self.na.sending {
            return;
        }
        if title.trim().is_empty() && text.trim().is_empty() {
            self.toast("Write something to send first.");
            return;
        }
        self.notion_add_start(notion_io::Capture { target, title, text });
    }

    /// Send on a thread; the answer comes back as `Msg::NotionAdded`.
    fn notion_add_start(&mut self, capture: notion_io::Capture) {
        self.na.sending = true;
        self.toast(trf("Adding to {name}…", &[("name", &capture.target.title)]));
        let (read, map, schema, bus) = (notion_io::token_reader(), notion_io::url_mapper(), self.na.schema.clone(), self.bus.clone());
        std::thread::spawn(move || {
            let result = match read() {
                None => Err(services::NotionAddError::NoToken),
                Some(t) => notion_io::send_capture(&t, &capture, &*map, &schema)
                    .map(|()| services::NotionAddDone { target_title: capture.target.title.clone() })
                    .map_err(services::NotionAddError::Failed),
            };
            bus.send(Msg::NotionAdded(result));
        });
    }

    fn handle_notion_added(&mut self, result: Result<services::NotionAddDone, services::NotionAddError>) {
        self.na.sending = false;
        match result {
            Ok(done) => {
                // the toast has no action button, so there is no "Open" link here
                self.toast(trf("Added to {name}", &[("name", &done.target_title)]));
                if let View::NotionAdd { title, text, .. } = self.stack.last_mut().unwrap() {
                    title.clear();
                    text.clear();
                }
            }
            Err(services::NotionAddError::NoToken) => {
                self.toast("Connect Notion first: Settings → Integrations.");
                self.notion_open_settings_tab();
            }
            Err(services::NotionAddError::Failed(msg)) => self.toast(msg),
        }
    }

    // ------------------------------------------------------------ New note / daily note (commands)

    /// The chosen vault, or a toast and Settings → Integrations (first precondition, no I/O).
    fn obsidian_compose_vault(&mut self) -> Option<ferrocast_core::obsidian::Vault> {
        let vault = self.obsidian_vault();
        if vault.is_none() {
            self.toast(OBSIDIAN_NO_VAULT);
            self.notion_open_settings_tab();
        }
        vault
    }

    /// ⌘↵ on the New Obsidian Note form.
    pub(crate) fn obsidian_new_submit(&mut self) {
        let View::ObsidianNew { title, text } = self.view().clone() else { return };
        if title.trim().is_empty() && text.trim().is_empty() {
            self.toast("Write something to send first.");
            return;
        }
        self.obsidian_send(services::ObsidianJob::New { title, text });
    }

    /// ⌘↵ on the Append to Daily Note form.
    pub(crate) fn obsidian_daily_submit(&mut self) {
        let View::ObsidianDaily { text } = self.view().clone() else { return };
        if text.trim().is_empty() {
            self.toast("Write something to send first.");
            return;
        }
        self.obsidian_send(services::ObsidianJob::Daily { text });
    }

    /// Check the preconditions on a thread (disk reads) and answer with `Msg::ObsidianSent`; the link is
    /// opened (and the form cleared) only from the answer, on the UI thread.
    fn obsidian_send(&mut self, job: services::ObsidianJob) {
        if self.oc.sending {
            return;
        }
        let Some(vault) = self.obsidian_compose_vault() else { return };
        self.oc.sending = true;
        let installed = self.obsidian_installed();
        let bus = self.bus.clone();
        std::thread::spawn(move || {
            let now = chrono::Local::now().naive_local();
            bus.send(Msg::ObsidianSent(services::obsidian_prepare(&job, &vault.path, &vault.name, installed, now)));
        });
    }

    fn handle_obsidian_sent(&mut self, r: services::ObsidianPrepared) {
        self.oc.sending = false;
        let uri = match r.result {
            Ok(uri) => uri,
            Err(refusal) => {
                self.toast(refusal.message());
                if matches!(refusal, services::ObsidianRefusal::FolderMissing) {
                    self.notion_open_settings_tab();
                }
                return;
            }
        };
        notion_io::open_url(&uri);
        let was_new = r.kind == services::ObsidianKind::New;
        self.toast(if was_new { "Sent to Obsidian" } else { "Added to today's daily note" });
        match self.stack.last_mut() {
            Some(View::ObsidianNew { title, text }) if was_new => {
                title.clear();
                text.clear();
            }
            Some(View::ObsidianDaily { text }) if !was_new => text.clear(),
            _ => {}
        }
    }

    // ------------------------------------------------------------ Obsidian (Settings → Integrations)

    /// Vaults found by the last scan (None while it has not answered).
    #[cfg(test)]
    pub fn obsidian_vaults(&self) -> Option<Vec<ferrocast_core::obsidian::Vault>> {
        self.obsidian.vaults.clone()
    }

    /// The selected vault (by path; the name is the folder name). T8 and T9 use this.
    pub fn obsidian_vault(&self) -> Option<ferrocast_core::obsidian::Vault> {
        let path = self.settings.obsidian_vault.clone()?;
        Some(ferrocast_core::obsidian::Vault { id: String::new(), name: vault_name(&path), path })
    }

    /// Whether the Obsidian app was found by the last scan (None: not scanned yet). Blocking detection
    /// for worker threads is `services::obsidian_installed_now`.
    pub fn obsidian_installed(&self) -> Option<bool> {
        self.obsidian.installed
    }

    /// Scan `obsidian.json` and look for the app on a thread; the answer is `Msg::ObsidianScanned`.
    fn obsidian_scan_with(&mut self, config: std::path::PathBuf, detect: impl FnOnce() -> bool + Send + 'static) {
        self.obsidian.started = true;
        self.obsidian.scanning = true;
        let bus = self.bus.clone();
        std::thread::spawn(move || {
            let (vaults, open) = services::scan_obsidian_config(&config);
            bus.send(Msg::ObsidianScanned(services::ObsidianScan { vaults, open, installed: detect() }));
        });
    }

    pub(crate) fn obsidian_scan(&mut self) {
        let config = ferrocast_core::obsidian::config_path(&dirs::home_dir().unwrap_or_default());
        self.obsidian_scan_with(config, services::obsidian_installed_now);
    }

    fn handle_obsidian_scanned(&mut self, r: services::ObsidianScan) {
        self.obsidian.scanning = false;
        self.obsidian.vaults = Some(r.vaults);
        self.obsidian.open = r.open;
        self.obsidian.installed = Some(r.installed);
    }

    fn obsidian_set(&mut self, path: std::path::PathBuf) {
        let name = vault_name(&path);
        let mut s = self.settings.clone();
        s.obsidian_vault = Some(path);
        self.commit_settings(s);
        self.toast(trf("Obsidian vault set to {name}", &[("name", &name)]));
    }

    fn obsidian_pick(&mut self, i: usize) {
        let Some(v) = self.obsidian.vaults.as_ref().and_then(|v| v.get(i)).cloned() else { return };
        self.obsidian_set(v.path);
    }

    /// Open the folder picker on a thread (it is modal and slow); the answer is `Msg::ObsidianFolder`.
    fn obsidian_choose_folder_with(&mut self, picker: impl FnOnce() -> Option<std::path::PathBuf> + Send + 'static) {
        if self.obsidian.picking {
            return;
        }
        self.obsidian.picking = true;
        let bus = self.bus.clone();
        std::thread::spawn(move || bus.send(Msg::ObsidianFolder(picker())));
    }

    pub(crate) fn obsidian_choose_folder(&mut self) {
        self.obsidian_choose_folder_with(|| apps::choose_folder("Choose your Obsidian vault folder"));
    }

    fn handle_obsidian_folder(&mut self, p: Option<std::path::PathBuf>) {
        self.obsidian.picking = false;
        let Some(p) = p else { return };
        if !p.is_dir() {
            self.toast("That is not a folder. Choose your Obsidian vault folder.");
            return;
        }
        // "/a/b/" and "/a/b" are the same vault
        self.obsidian_set(p.components().collect());
    }

    fn obsidian_clear(&mut self) {
        let mut s = self.settings.clone();
        s.obsidian_vault = None;
        self.commit_settings(s);
        self.toast("Obsidian vault cleared");
    }

    fn notion_clear_target(&mut self) {
        let mut s = self.settings.clone();
        s.notion_target = None;
        self.commit_settings(s);
        self.toast("Capture target cleared");
    }

    /// A toast made of text the user (or an extension) wrote: shown as typed.
    pub(crate) fn toast_verbatim(&mut self, s: impl Into<String>) {
        self.toast(s);
        self.toast_user = true;
    }

    fn toast(&mut self, s: impl Into<String>) {
        self.toast_user = false;
        self.toast = Some((s.into(), Instant::now()));
        self.ctx.request_repaint_after(Duration::from_millis(2600));
    }

    /// Runs `f` on the Notes controller with the stores it edits, then does what it asks for.
    fn notes_do(&mut self, f: impl FnOnce(&mut crate::notes_ctl::NotesCtl, &mut crate::notes_ctl::Data) -> crate::notes_ctl::Out) {
        let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
        let mut d = crate::notes_ctl::Data {
            notes: &mut self.notes,
            settings: &mut self.settings,
            quicklinks: &mut self.quicklinks,
            now: now_secs(),
            now_ms,
        };
        let out = f(&mut self.notes_ctl, &mut d);
        self.sec_dirty = true;
        if out.save_notes {
            self.save_notes();
        }
        if out.save_quicklinks {
            self.save_quicklinks();
        }
        if out.save_settings {
            self.save_settings();
        }
        if let Some(t) = out.toast {
            // the launcher is usually hidden while the Notes window is in use
            if self.visible {
                self.toast(t);
            } else {
                services::notify("Notes", &t);
            }
        }
    }

    /// Pushes the Notes settings (zoom, Esc behavior, Format Bar) to the window and brings the
    /// menu-bar item in line with `notes_menu_bar_button`.
    pub(crate) fn apply_notes_settings(&mut self) {
        self.notes_do(|c, d| {
            c.apply_settings(d);
            crate::notes_ctl::Out::default()
        });
        self.sync_notes_tray();
    }

    /// Creates the Notes toggle in the menu bar when the setting is on and none exists, removes it
    /// (by dropping it) when the setting is off. Main thread only.
    /// Mirror the recording state in the menu bar: `●  mm:ss`, red-dot icon, `Stop Recording`.
    pub(crate) fn sync_recording_tray(&mut self) {
        let (title, stop) = (self.record.tray_title(), self.record.shows_stop_item());
        if let Some(tray) = self._tray.as_ref() {
            services::set_tray_recording(tray, title.as_deref(), stop, &mut self.tray_rec_shown);
        }
    }

    pub(crate) fn sync_notes_tray(&mut self) {
        match (self.settings.notes_menu_bar_button, self.notes_tray.is_some()) {
            (true, false) => self.notes_tray = (self.fx.notes_tray)(&self.bus),
            (false, true) => self.notes_tray = None,
            _ => {}
        }
    }

    /// Writes pending notes and exits.
    fn quit(&mut self) -> ! {
        self.editor_flush();
        self.notes_do(|c, d| c.flush(d));
        // process::exit skips destructors: stop the extension sidecar (and its children) by hand
        self.ext_kill();
        std::process::exit(0)
    }

    // ------------------------------------------------------------ navigation

    fn view(&self) -> &View {
        self.stack.last().unwrap()
    }

    fn push(&mut self, v: View) {
        self.editor_flush();
        self.sec_dirty = true;
        self.nav_at = Instant::now();
        if matches!(v, View::Windows) {
            self.win_cache = running::list_windows();
        }
        if matches!(v, View::Processes) {
            self.proc_cache = ferrocast_macos::process::list();
        }
        if matches!(v, View::Ai { .. }) {
            self.warm_claude();
        }
        if matches!(v, View::Welcome) {
            self.tour_opened();
        }
        self.stack.push(v);
        self.query.clear();
        self.prev_query.clear();
        self.selected = 0;
        self.scroll_to_sel = true;
        self.panel = None;
        self.focus_first = true;
    }

    /// Forget the reference-frame bookkeeping of a previous video window.
    fn ref_frame_reset(&mut self) {
        self.ctx.data_mut(|d| {
            d.remove::<(PathBuf, f64)>(Id::new("ref-flight"));
            d.remove::<(PathBuf, f64)>(Id::new("ref-asked"));
            d.remove::<(PathBuf, f64)>(Id::new("ref-shown"));
        });
    }

    /// Record that the texture behind the rectangles shows `file` at `t`.
    fn ref_frame_shown(&mut self, file: &std::path::Path, t: f64) {
        self.ctx.data_mut(|d| {
            d.insert_temp(Id::new("ref-shown"), (file.to_path_buf(), t));
            d.insert_temp(Id::new("ref-asked"), (file.to_path_buf(), t));
        });
    }

    /// Ask for the frame at the slider's time when it is not the one shown: off the UI thread, one
    /// request in flight, and only the newest time is asked for once the reply arrives.
    pub(crate) fn pump_ref_frame(&mut self) {
        let (View::VideoCrop { file, size: Some(_), duration, ref_t, .. } | View::VideoRedact { file, size: Some(_), duration, ref_t, .. }) =
            self.view()
        else {
            return;
        };
        let (file, t) = (file.clone(), ferrocast_core::videoplan::frame_time(*duration, *ref_t));
        let Some(ff) = self.tools.ffmpeg else { return };
        let in_flight = self.ctx.data(|d| d.get_temp::<(PathBuf, f64)>(Id::new("ref-flight"))).is_some();
        let shown = self.ctx.data(|d| d.get_temp::<(PathBuf, f64)>(Id::new("ref-shown")));
        if in_flight || shown.is_some_and(|(f, s)| f == file && s == t) {
            return;
        }
        self.ctx.data_mut(|d| {
            d.insert_temp(Id::new("ref-flight"), (file.clone(), t));
            d.insert_temp(Id::new("ref-asked"), (file.clone(), t));
        });
        let bus = self.bus.clone();
        std::thread::spawn(move || {
            let png = crate::media_tools::preview_frame(ff, &file, t);
            bus.send(Msg::RefFrame(file, t, png));
        });
    }

    /// Write the editor's pending edits now (Esc, closing, hiding, switching view, quitting).
    fn editor_flush(&mut self) {
        if let Some(ed) = self.editor.as_mut() {
            ed.flush(ed.now());
        }
    }

    fn pop(&mut self) {
        self.ext_before_pop();
        self.editor_flush();
        // Escape (or going back) also ends a running translating effect
        self.end_effect(false);
        self.sec_dirty = true;
        self.nav_at = Instant::now();
        if self.stack.len() > 1 {
            self.stack.pop();
            self.query.clear();
            self.prev_query.clear();
            self.selected = 0;
            self.scroll_to_sel = true;
            self.panel = None;
            self.cool_claude();
        } else {
            self.hide();
        }
    }

    /// Segments for a wheel over these files.
    pub(crate) fn wheel_segments(&self, kind: ferrocast_core::wheel::WheelKind, files: &[PathBuf]) -> Vec<WheelSeg> {
        use ferrocast_core::convert::targets;
        if kind == ferrocast_core::wheel::WheelKind::Translate {
            return ferrocast_core::translate::languages(&self.settings.translate_languages)
                .into_iter()
                .map(|l| WheelSeg { label: l.clone(), symbol: "sf:globe", needs: None, pick: WheelPick::Translate(l) })
                .collect();
        }
        let Some(first) = files.first() else { return vec![] };
        match kind {
            ferrocast_core::wheel::WheelKind::Convert => {
                // every output stays on the ring; ones that need an uninstalled tool are dimmed
                targets(first, &self.tools)
                    .into_iter()
                    .map(|t| WheelSeg {
                        label: t.label.to_string(),
                        symbol: target_symbol(t.ext),
                        needs: t.needs,
                        pick: WheelPick::Convert(t),
                    })
                    .collect()
            }
            ferrocast_core::wheel::WheelKind::Actions => {
                let f = files.to_vec();
                let paths = f.iter().map(|p| p.to_string_lossy().to_string()).collect::<Vec<_>>().join("\n");
                let seg = |label: &str, symbol: &'static str, act: Act| WheelSeg {
                    label: label.into(),
                    symbol,
                    needs: None,
                    pick: WheelPick::Act(act),
                };
                // the six Tangerine actions plus Zip (user addition), in the order of ferrocast_core::tools::ACTIONS
                ferrocast_core::tools::ACTIONS
                    .into_iter()
                    .map(|a| {
                        use ferrocast_core::tools::ActionId;
                        match a {
                            ActionId::AirDrop => seg("AirDrop", "sf:dot.radiowaves.left.and.right", Act::AirDrop(f.clone())),
                            ActionId::Float => seg("Float", "sf:tray.full", Act::AddToShelf(f.clone())),
                            ActionId::Copy => seg("Copy", "sf:doc.on.doc", Act::CopyFiles(f.clone())),
                            ActionId::Move => seg("Move to…", "sf:folder", Act::MoveFiles(f.clone())),
                            ActionId::CopyPath => seg("Copy Path", "sf:link", Act::CopyText(paths.clone())),
                            ActionId::OpenTerminal => seg("Terminal", "sf:terminal", Act::OpenInTerminal(f[0].clone())),
                            ActionId::Zip => seg("Zip", "sf:doc.zipper", Act::ZipFiles(f.clone())),
                        }
                    })
                    .collect()
            }
            ferrocast_core::wheel::WheelKind::Translate => unreachable!("handled above"),
            ferrocast_core::wheel::WheelKind::Tools => {
                use ferrocast_core::convert::ext;
                use ferrocast_core::tools::tools_for;
                // one list per extension family (jpeg/tif aliases handled by tools_for)
                let multi = files.len() > 1;
                let list = tools_for(&ext(first), multi);
                let same = !list.is_empty() && files.iter().all(|f| tools_for(&ext(f), multi) == list);
                if same {
                    list.into_iter().take(12).map(|t| tool_segment(t, files)).collect()
                } else {
                    vec![WheelSeg {
                        label: "Open tools…".into(),
                        symbol: "sf:wrench.and.screwdriver",
                        needs: None,
                        pick: WheelPick::Act(Act::Convert(files.to_vec())),
                    }]
                }
            }
        }
    }

    /// Show the wheel centred under the pointer, without moving the drag's focus elsewhere.
    fn show_wheel(&mut self, kind: ferrocast_core::wheel::WheelKind, files: Vec<PathBuf>) {
        let segs = self.wheel_segments(kind, &files);
        instance::log(&self.store.root, &wheel_log_line("open", kind, &segs, files.len()));
        if segs.is_empty() {
            return;
        }
        // the wheel is positioned for the launcher's size: a maximised window goes back first
        self.leave_fullscreen();
        let size = Vec2::new(ui::WIDTH, ui::HEIGHT);
        let center = match mouse_top_left() {
            Some((m, screen)) => {
                let min = Pos2::new(
                    (m.x - size.x / 2.0).clamp(screen.min.x, (screen.max.x - size.x).max(screen.min.x)),
                    (m.y - size.y / 2.0).clamp(screen.min.y, (screen.max.y - size.y).max(screen.min.y)),
                );
                self.ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(min));
                min + size / 2.0
            }
            None => Pos2::ZERO,
        };
        self.prev_pid = running::frontmost().map(|f| f.pid).filter(|p| *p != std::process::id() as i32);
        self.stack = vec![View::Root, View::Wheel { files, center, segs, kind, sel: None }];
        self.ctx.data_mut(|d| d.remove::<draw::WheelAnim>(Id::new("wheel_anim")));
        self.panel = None;
        self.visible = true;
        self.had_focus = false;
        self.shown_at = Instant::now();
        self.shared.visible.store(true, Ordering::Relaxed);
        if let Some(mtm) = objc2::MainThreadMarker::new() {
            objc2_app_kit::NSApplication::sharedApplication(mtm).unhide(None);
        }
        self.ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        self.ctx.request_repaint();
    }

    /// Mouse released: run the segment under the pointer, if any, and close the wheel.
    fn release_wheel(&mut self) {
        let Some(View::Wheel { center, segs, sel, .. }) = self.stack.last().cloned() else { return };
        let picked = mouse_top_left().and_then(|(m, _)| wheel_hit(m - center, segs.len())).or(sel);
        self.apply_wheel(picked);
    }

    /// Switch an open wheel to another kind, keeping its position.
    pub(crate) fn switch_wheel(&mut self, kind: ferrocast_core::wheel::WheelKind) {
        let Some(View::Wheel { files, kind: current, .. }) = self.stack.last().cloned() else { return };
        // a text wheel has nothing to switch to, and file wheels never become one
        if current == ferrocast_core::wheel::WheelKind::Translate || kind == ferrocast_core::wheel::WheelKind::Translate {
            return;
        }
        let segs = self.wheel_segments(kind, &files);
        instance::log(&self.store.root, &wheel_log_line("switch", kind, &segs, files.len()));
        if segs.is_empty() {
            return;
        }
        if let Some(View::Wheel { segs: s, kind: k, sel, .. }) = self.stack.last_mut() {
            *s = segs;
            *k = kind;
            *sel = None;
        }
        self.ctx.request_repaint();
    }

    /// Close the wheel and run segment `picked` (None cancels).
    pub(crate) fn apply_wheel(&mut self, picked: Option<usize>) {
        let Some(View::Wheel { files, segs, kind, center, .. }) = self.stack.last().cloned() else { return };
        // The release that picks a segment also ends the drag: macOS delivers the file DROP to the
        // launcher right after. Remember when the gesture ended so that drop can be ignored.
        self.ctx.data_mut(|d| d.insert_temp(Id::new("wheel_end"), Instant::now()));
        self.ctx.data_mut(|d| d.remove::<draw::WheelAnim>(Id::new("wheel_anim")));
        self.hide();
        self.reset_to_root();
        let Some(i) = picked.filter(|i| *i < segs.len()) else { return };
        let (label, pick, needs) = (segs[i].label.clone(), segs[i].pick.clone(), segs[i].needs);
        match pick {
            WheelPick::Translate(lang) => {
                if let Some(text) = self.wheel_text.take() {
                    // Apple chosen but unusable here (macOS < 26, no helper): the chat provider translates
                    let mut chosen = self.settings.clone();
                    let mut fallback_note = None;
                    if ferrocast_core::translate::apple_plan(&chosen, (self.fx.apple_available)())
                        == ferrocast_core::translate::ApplePlan::FallBackToChat
                    {
                        chosen = ferrocast_core::translate::without_apple(&chosen);
                        if !std::mem::replace(&mut self.apple_fallback_told, true) {
                            fallback_note = Some("Apple Translation needs macOS 26; using the chat provider.".to_string());
                        }
                    }
                    let engine = ferrocast_core::translate::engine_settings(&chosen);
                    // a new translation ends a running effect first
                    self.end_effect(false);
                    self.translate_run += 1;
                    let run = self.translate_run;
                    self.translated_run = None;
                    self.translate_started = Instant::now();
                    if self.settings.translate_effect {
                        let drop = effect_drop_point(center);
                        (self.fx.effect_probe)(self.prev_pid, drop, run, &self.bus);
                    }
                    let working = format!("Translating to {lang} with {}…", ferrocast_core::translate::engine_label(&chosen));
                    self.toast(fallback_note.unwrap_or(working));
                    (self.fx.translate_start)(&engine, text, lang, self.prev_pid, self.settings.translate_output, run, &self.bus);
                }
            }
            WheelPick::Act(Act::Convert(_)) if needs.is_some() => self.toast(trf("{label}: not available yet", &[("label", &label)])),
            WheelPick::File(tool) => {
                self.toast(format!("{}…", tr(&tool.label())));
                services::file_tool_files(files, tool, self.tools.ffmpeg, self.bus.clone());
            }
            WheelPick::Image(ferrocast_core::convert::ImageOp::Collage) if files.len() >= 2 => {
                self.perform(Act::CollageImages(files), false);
            }
            WheelPick::Image(ferrocast_core::convert::ImageOp::Annotate) if files.len() == 1 => {
                self.perform(Act::AnnotateCanvas(files[0].clone()), false);
            }
            WheelPick::Image(op) => {
                self.toast(format!("{}…", op.label()));
                services::image_op_files(files, op, self.bus.clone());
            }
            WheelPick::Convert(t) if t.needs.is_some() => {
                let n = t.needs.unwrap_or_default();
                let cmd = n.split_once('(').map(|(_, c)| c.trim_end_matches(')').to_string()).unwrap_or_else(|| n.to_string());
                self.copy_text(&cmd);
                self.toast(trf("Needs {n}. Install command copied: {cmd}", &[("n", n), ("cmd", &cmd)]));
            }
            WheelPick::Convert(t) => {
                self.toast(trf("Converting to {label}…", &[("label", t.label)]));
                services::convert_files(files, t, self.tools, self.bus.clone());
            }
            WheelPick::Act(a) => self.perform(a, false),
        }
        instance::log(&self.store.root, &wheel_pick_log_line(kind, &label, self.view().title(), self.visible));
    }

    fn reset_to_root(&mut self) {
        self.ext_leave_root();
        self.editor_flush();
        self.sec_dirty = true;
        self.nav_at = Instant::now();
        self.stack = vec![View::Root];
        self.query.clear();
        self.selected = 0;
        self.scroll_to_sel = true;
        self.panel = None;
        self.cool_claude();
    }

    // ------------------------------------------------------------- messages

    fn drain(&mut self) {
        self.editor_gc();
        let mut handled = 0usize;
        while handled < MAX_MSGS_PER_FRAME {
            let Ok(m) = self.rx.try_recv() else { break };
            handled += 1;
            // icon bytes only feed the icon cache: the list holds icon descriptors, not pixels
            if !matches!(m, Msg::IconBytes(..) | Msg::Editor(..)) {
                self.sec_dirty = true;
            }
            match m {
                Msg::Wake => {}
                Msg::Ext(id, ev) => self.ext_event(id, ev),
                Msg::ExtNodeFound(n) => self.ext.node_found(n),
                Msg::ExtNodeDownloaded(r) => match r {
                    Ok(n) => {
                        self.ext.download_finished(Some(n));
                        self.toast("Node.js downloaded and verified");
                    }
                    Err(e) => {
                        self.ext.downloading = false;
                        self.toast(trf("Node.js download failed: {e}", &[("e", &e.to_string())]));
                    }
                },
                // a URL comes from outside: the launcher steps aside like for any Notes command
                Msg::Notes(m @ crate::notes_ctl::NMsg::Url(_)) => self.open_notes_window(|c, d| c.handle(m, d)),
                Msg::Notes(m) => self.notes_do(|c, d| c.handle(m, d)),
                Msg::UpdateChecked { manual, result } => {
                    let exe = instance::running_exe().unwrap_or_default();
                    self.handle_update_checked(
                        manual,
                        result,
                        ferrocast_core::update::channel_of(&exe),
                        services::parent_writable_cached(),
                    );
                    if matches!(self.view(), View::Update { .. }) && !self.visible {
                        // show() may reset the stack after a long hide: put the view back on top
                        let v = self.view().clone();
                        self.show();
                        if !matches!(self.view(), View::Update { .. }) {
                            self.push(v);
                        }
                    }
                }
                Msg::UpdateInstalled { path } => {
                    // exit first: it releases the instance lock the relaunched copy needs
                    let _ = std::process::Command::new("/bin/sh")
                        .arg("-c")
                        .arg("sleep 1; open -n \"$1\"")
                        .arg("ferrocast-relaunch")
                        .arg(&path)
                        .stdin(std::process::Stdio::null())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn();
                    self.quit();
                }
                Msg::UpdateFailed(e) => {
                    instance::log(&self.store.root, &format!("update failed: {e}"));
                    self.toast(trf("Update failed: {e}", &[("e", &e.to_string())]));
                }
                Msg::NotionTested(r) => self.handle_notion_tested(r),
                Msg::NotionSearched { request, result } => self.handle_notion_searched(request, result),
                Msg::NotionViewSearched { request, append, result } => self.handle_notion_view_searched(request, append, result),
                Msg::NotionAdded(r) => self.handle_notion_added(r),
                Msg::ObsidianSent(r) => self.handle_obsidian_sent(r),
                Msg::ObsidianIndexed { request, vault, notes } => self.handle_obsidian_indexed(request, vault, notes),
                Msg::ObsidianContent { request, query, hits } => self.handle_obsidian_content(request, query, hits),
                Msg::ObsidianPreview { request, result } => self.handle_obsidian_preview(request, result),
                Msg::ObsidianScanned(r) => self.handle_obsidian_scanned(r),
                Msg::ObsidianFolder(p) => self.handle_obsidian_folder(p),
                Msg::Toast(t) => {
                    // A sign-in or sign-out may just have finished; let Settings re-read the Keychain.
                    self.ctx.data_mut(|d| d.remove::<bool>(Id::new("chatgpt_signed")));
                    self.toast(t)
                }
                Msg::Apps(a) => {
                    self.apps = a;
                    self.rebuild();
                }
                Msg::Files(q, f) => self.files = (q, f),
                Msg::AnnotateLoaded(file, png) => {
                    if matches!(self.view(), View::Annotate { file: f, .. } if *f == file) {
                        match png.as_deref().and_then(|b| image::load_from_memory(b).ok()).map(|i| i.to_rgba8()) {
                            Some(rgba) => {
                                let (w, h) = (rgba.width() as usize, rgba.height() as usize);
                                let ci = egui::ColorImage::from_rgba_unmultiplied([w, h], rgba.as_raw());
                                let tex = self.ctx.load_texture("annot-view", ci, egui::TextureOptions::LINEAR);
                                self.ctx.data_mut(|d| {
                                    d.insert_temp(Id::new("annot-base"), std::sync::Arc::new((rgba.into_raw(), w, h)));
                                    d.insert_temp(Id::new("annot-tex"), tex);
                                    d.remove::<ferrocast_core::annotate::Doc>(Id::new("annot-last"));
                                });
                            }
                            None => self.toast("Annotate: could not read the image"),
                        }
                    }
                }
                Msg::VideoRedactReady(file, ready) => {
                    if matches!(self.view(), View::VideoRedact { file: f, .. } if *f == file) {
                        if let Some((dims, d, png)) = ready {
                            if let Some(img) = services::decode_png(&png) {
                                let tex = self.ctx.load_texture("crop-view", img, egui::TextureOptions::LINEAR);
                                self.ctx.data_mut(|data| data.insert_temp(Id::new("crop-tex"), tex));
                                let t = ferrocast_core::videoplan::frame_time(d, d * 0.1);
                                self.ref_frame_shown(&file, t);
                                if let Some(View::VideoRedact { size, areas, sel, duration, ref_t, .. }) = self.stack.last_mut() {
                                    *size = Some(dims);
                                    *areas = vec![new_area(dims, d)];
                                    (*sel, *duration, *ref_t) = (0, d, t);
                                }
                            } else {
                                self.toast("Redact: could not decode the preview");
                            }
                        } else {
                            self.toast("Redact: could not read the video");
                        }
                    }
                }
                Msg::VideoCropReady(file, ready) => {
                    if matches!(self.view(), View::VideoCrop { file: f, .. } if *f == file) {
                        match ready {
                            Some((dims, d, png)) => {
                                let t = ferrocast_core::videoplan::frame_time(d, d * 0.1);
                                if let Some(img) = services::decode_png(&png) {
                                    let tex = self.ctx.load_texture("crop-view", img, egui::TextureOptions::LINEAR);
                                    self.ctx.data_mut(|data| data.insert_temp(Id::new("crop-tex"), tex));
                                    self.ref_frame_shown(&file, t);
                                }
                                if let Some(View::VideoCrop { size, rect, duration, ref_t, .. }) = self.stack.last_mut() {
                                    *size = Some(dims);
                                    *rect = ferrocast_core::croprect::full(dims.0, dims.1);
                                    (*duration, *ref_t) = (d, t);
                                }
                            }
                            None => self.toast("Crop: could not read the video"),
                        }
                    }
                }
                Msg::RefFrame(file, t, png) => {
                    // one request in flight, newest wins: the reply is used only for this file and the
                    // time asked for last; the pump then asks again if the slider has moved on
                    let flight: Option<(PathBuf, f64)> = self.ctx.data(|d| d.get_temp(Id::new("ref-flight")));
                    if flight.as_ref().is_some_and(|(f, _)| *f == file) {
                        self.ctx.data_mut(|d| d.remove::<(PathBuf, f64)>(Id::new("ref-flight")));
                    }
                    let asked: Option<(PathBuf, f64)> = self.ctx.data(|d| d.get_temp(Id::new("ref-asked")));
                    let current = matches!(self.view(), View::VideoCrop { file: f, .. } | View::VideoRedact { file: f, .. } if *f == file);
                    if current && asked.is_some_and(|(f, a)| f == file && a == t) {
                        if let Some(img) = png.as_deref().and_then(services::decode_png) {
                            let tex = self.ctx.load_texture("crop-view", img, egui::TextureOptions::LINEAR);
                            self.ctx.data_mut(|d| d.insert_temp(Id::new("crop-tex"), tex));
                            self.ref_frame_shown(&file, t);
                        }
                    }
                    self.pump_ref_frame();
                    self.ctx.request_repaint();
                }
                Msg::MetadataLoaded(file, res) => {
                    if let Some(View::EditMetadata { file: f, original, working, error, .. }) = self.stack.last_mut() {
                        if *f == file {
                            match res {
                                Ok(fields) => (*working, *original, *error) = (fields.clone(), Some(fields), None),
                                Err(e) => *error = Some(e),
                            }
                        }
                    }
                }
                Msg::LoudnessMeasured(file, l) => {
                    if let Some(View::NormalizeAudio { file: f, loudness, .. }) = self.stack.last_mut() {
                        if *f == file {
                            *loudness = Some(l.filter(|l| l.is_finite()).unwrap_or(f64::NAN));
                            if l.is_none_or(|l| !l.is_finite()) {
                                self.toast("Normalize: could not measure the loudness of this file");
                            }
                        }
                    }
                }
                Msg::ChannelsProbed(file, n) => {
                    if let Some(View::ChannelsAudio { file: f, in_channels, .. }) = self.stack.last_mut() {
                        if *f == file {
                            *in_channels = n;
                        }
                    }
                }
                Msg::VideoDuration(file, d) => {
                    if matches!(self.view(), View::VideoSpeed { file: f, .. } if *f == file) {
                        match d.filter(|d| d.is_finite() && *d > 0.0) {
                            Some(d) => {
                                if let Some(View::VideoSpeed { duration, .. }) = self.stack.last_mut() {
                                    *duration = Some(d);
                                }
                            }
                            None => self.toast("Speed: could not read the video"),
                        }
                    }
                }
                Msg::VideoInfo(file, info) => {
                    if matches!(self.view(), View::VideoTrim { file: f, .. } | View::VideoSnapshot { file: f, .. } | View::VideoSplit { file: f, .. } if *f == file)
                    {
                        match info.and_then(|(d, r)| ferrocast_core::videoplan::Trim::new(d, r)) {
                            Some(t) => {
                                if let Some(
                                    View::VideoTrim { trim, .. } | View::VideoSnapshot { trim, .. } | View::VideoSplit { trim, .. },
                                ) = self.stack.last_mut()
                                {
                                    *trim = Some(t);
                                }
                            }
                            None => self.toast("Trim: could not read the video"),
                        }
                    }
                }
                Msg::AudioLoaded(file, res) => {
                    // a stale waveform cache must never outlive the samples it was built from
                    self.ctx.data_mut(|d| d.remove::<crate::media_tools::PeaksCache>(Id::new("at-peaks")));
                    if matches!(self.view(), View::AudioTrim { file: f, .. } if *f == file) {
                        match res {
                            Ok(a) => {
                                let t = ferrocast_core::videoplan::Trim::new(a.duration, 100.0);
                                if let (Some(t), Some(View::AudioTrim { trim, audio, .. })) = (t, self.stack.last_mut()) {
                                    (*trim, *audio) = (Some(t), Some(Arc::new(a)));
                                } else {
                                    self.pop();
                                    self.toast("Trim Audio: this audio is too short");
                                }
                            }
                            Err(e) => {
                                self.pop();
                                self.toast(trf("Trim Audio: {e}", &[("e", &e.to_string())]));
                            }
                        }
                    }
                    self.ctx.request_repaint();
                }
                Msg::BleepAudioLoaded(file, res) => {
                    if matches!(self.view(), View::BleepAudio { file: f, .. } if *f == file) {
                        match res {
                            Ok(a) => {
                                let covered = (a.samples.len() as f64 / f64::from(a.rate.max(1)) / a.duration).clamp(0.0, 1.0);
                                let real = ((800.0 * covered).ceil() as usize).clamp(1, 800);
                                let mut bars = ferrocast_core::waveform::peaks(&a.samples, real);
                                bars.resize(800, (0, 0));
                                if let Some(View::BleepAudio { duration, peaks, ranges, sel, .. }) = self.stack.last_mut() {
                                    *duration = a.duration;
                                    *peaks = Some(Arc::new(bars));
                                    *ranges = vec![new_bleep_range(a.duration)];
                                    *sel = 0;
                                }
                            }
                            Err(e) => {
                                self.pop();
                                self.toast(trf("Bleep Audio: {e}", &[("e", &e.to_string())]));
                            }
                        }
                    }
                    self.ctx.request_repaint();
                }
                Msg::VideoFrame(file, t, png) => {
                    if matches!(self.view(), View::VideoTrim { file: f, .. } | View::VideoSnapshot { file: f, .. } | View::VideoSplit { file: f, .. } if *f == file)
                    {
                        self.ctx.data_mut(|d| d.insert_temp(Id::new("vt-busy"), false));
                    }
                    if matches!(self.view(), View::VideoTrim { file: f, .. } | View::VideoSnapshot { file: f, .. } | View::VideoSplit { file: f, .. } if *f == file)
                    {
                        if let Some(img) = png.as_deref().and_then(services::decode_png) {
                            let tex = self.ctx.load_texture("vt-view", img, egui::TextureOptions::LINEAR);
                            self.ctx.data_mut(|d| {
                                d.insert_temp(Id::new("vt-tex"), tex);
                                d.insert_temp(Id::new("vt-shown"), t);
                            });
                        }
                    }
                    self.ctx.request_repaint();
                }
                Msg::CollageLoaded(files, thumbs) => {
                    let current = match self.view() {
                        View::CollageImage { files: f, .. } => Some(f.clone()),
                        _ => None,
                    };
                    let same_files = |a: &[PathBuf]| {
                        let (mut x, mut y) = (a.to_vec(), files.clone());
                        x.sort();
                        y.sort();
                        x == y
                    };
                    // a result for another collage is ignored silently, as before
                    if let Some(current) = current.filter(|c| same_files(c)) {
                        let imgs: Vec<image::RgbaImage> =
                            thumbs.into_iter().filter_map(|(w, h, px)| image::RgbaImage::from_raw(w as u32, h as u32, px)).collect();
                        // the pictures may have been dragged into a new order while they loaded
                        let imgs = align_to_current(&current, &files, imgs);
                        if let Some(imgs) = imgs {
                            self.ctx.data_mut(|d| d.remove::<std::sync::Arc<Vec<egui::TextureHandle>>>(Id::new("collage-strip")));
                            self.ctx.data_mut(|d| {
                                d.insert_temp(Id::new("collage-base"), std::sync::Arc::new(imgs));
                                d.remove::<ferrocast_core::collage::CollageOpts>(Id::new("collage-last"));
                            });
                        } else {
                            self.toast("Collage: could not read every image");
                        }
                    }
                }
                Msg::FrameLoaded(file, png) => {
                    if matches!(self.view(), View::FrameImage { file: f, .. } if *f == file) {
                        match png.as_deref().and_then(|b| image::load_from_memory(b).ok()).map(|i| i.to_rgba8()) {
                            Some(rgba) => self.ctx.data_mut(|d| {
                                d.insert_temp(Id::new("frame-base"), std::sync::Arc::new(rgba));
                                d.remove::<(ferrocast_core::frame::FrameOpts, usize)>(Id::new("frame-last"));
                            }),
                            None => self.toast("Background: could not read the image"),
                        }
                    }
                }
                Msg::FramePicked(file, picked) => {
                    let mut load = None;
                    if let Some(View::FrameImage { file: f, opts, picking, .. }) = self.stack.last_mut() {
                        if *f == file {
                            *picking = false;
                            if let Some(p) = picked {
                                let path = p.to_string_lossy().to_string();
                                opts.backdrop = ferrocast_core::frame::Backdrop::Image(path.clone());
                                load = Some((p, path));
                            }
                        }
                    }
                    if let Some((p, path)) = load {
                        let bus = self.bus.clone();
                        std::thread::spawn(move || bus.send(Msg::FrameBgLoaded(file, path, services::display_png(&p, 600))));
                    }
                }
                Msg::FrameBgLoaded(file, path, png) => {
                    use ferrocast_core::frame::Backdrop;
                    let current = matches!(self.view(), View::FrameImage { file: f, opts, .. }
                        if *f == file && opts.backdrop == Backdrop::Image(path.clone()));
                    if current {
                        match png.as_deref().and_then(|b| image::load_from_memory(b).ok()).map(|i| i.to_rgba8()) {
                            Some(rgba) => self.ctx.data_mut(|d| {
                                d.insert_temp(Id::new("frame-bg"), std::sync::Arc::new((path, rgba)));
                                d.remove::<(ferrocast_core::frame::FrameOpts, usize)>(Id::new("frame-last"));
                            }),
                            None => {
                                if let Some(View::FrameImage { opts, .. }) = self.stack.last_mut() {
                                    opts.backdrop = Backdrop::Image(String::new());
                                }
                                self.toast("Background: could not read that picture");
                            }
                        }
                    }
                }
                Msg::RedactLoaded(file, png) => {
                    if matches!(self.view(), View::RedactImage { file: f, .. } if *f == file) {
                        match png.as_deref().and_then(services::decode_png) {
                            Some(img) => {
                                let tex = self.ctx.load_texture("redact-view", img, egui::TextureOptions::LINEAR);
                                self.ctx.data_mut(|d| d.insert_temp(Id::new("redact-tex"), tex));
                            }
                            None => self.toast("Redact: could not read the image"),
                        }
                    }
                }
                Msg::CompressProbed(file, size) => {
                    if let Some(View::CompressImage { files, size: s, .. }) = self.stack.last_mut() {
                        if files.first() == Some(&file) {
                            *s = size;
                        }
                    }
                }
                Msg::MediaProbed(file, info) => {
                    if let Some(View::CompressImage { files, media: Some(m), .. }) = self.stack.last_mut() {
                        if files.first() == Some(&file) {
                            (m.loaded, m.info) = (true, info);
                        }
                    }
                }
                Msg::CropLoaded(file, png) => {
                    if matches!(self.view(), View::CropImage { file: f, .. } if *f == file) {
                        match png.as_deref().and_then(services::decode_png) {
                            Some(img) => {
                                let tex = self.ctx.load_texture("crop-view", img, egui::TextureOptions::LINEAR);
                                self.ctx.data_mut(|d| d.insert_temp(Id::new("crop-tex"), tex));
                            }
                            None => self.toast("Crop: could not read the image"),
                        }
                    }
                }
                Msg::EditLoaded(file, png) => {
                    if matches!(self.view(), View::EditPhoto { file: f, .. } if *f == file) {
                        match png.as_deref().and_then(|b| image::load_from_memory(b).ok()).map(|i| i.to_rgba8()) {
                            Some(rgba) => {
                                let (w, h) = (rgba.width() as usize, rgba.height() as usize);
                                self.ctx.data_mut(|d| {
                                    d.insert_temp(Id::new("edit-base"), std::sync::Arc::new((rgba.into_raw(), w, h)));
                                    d.remove::<ferrocast_core::photo::PhotoOpts>(Id::new("edit-last"));
                                });
                            }
                            None => self.toast("Edit Photo: could not read the image"),
                        }
                    }
                }
                Msg::VisualPicked(file, picked) => {
                    if let Some(View::VisualizeAudio { file: f, image, picking, .. }) = self.stack.last_mut() {
                        if *f == file {
                            *picking = false;
                            if picked.is_some() {
                                *image = picked;
                            }
                        }
                    }
                }
                Msg::StampChosen(file, at, picked) => {
                    let Some((pw, ph, rgba)) = picked else { continue };
                    let Some(img) = ferrocast_core::annotate::Img::new(pw, ph, rgba) else { continue };
                    if let Some(View::Annotate { file: f, size, doc, color, .. }) = self.stack.last_mut() {
                        if *f == file {
                            // a quarter of the picture's width, keeping the stamp's own proportions
                            let w = size.0 as f32 * 0.25;
                            let style = ferrocast_core::annotate::Style {
                                color: ferrocast_core::annotate::COLORS[(*color).min(5)],
                                ..Default::default()
                            };
                            doc.add(ferrocast_core::annotate::Shape::Stamp { at, size: (w, w * ph as f32 / pw as f32), img }, style);
                        }
                    }
                }
                Msg::PdfThumb(file, i, png) => {
                    // only for the organizer that asked for it
                    if matches!(self.view(), View::PdfOrganize { file: f, .. } if *f == file) {
                        if let Some(img) = png.as_deref().and_then(services::decode_png) {
                            let tex = self.ctx.load_texture(format!("pdfthumb-{i}"), img, egui::TextureOptions::LINEAR);
                            self.ctx.data_mut(|d| d.insert_temp(Id::new(("pdfthumb", i)), tex));
                        }
                    }
                }
                Msg::Moved(pairs) => {
                    // keep the file results pointing at the new places
                    for (old, new) in &pairs {
                        for f in self.files.1.iter_mut() {
                            if f == old {
                                *f = new.clone();
                            }
                        }
                    }
                    self.toast(trn("Moved {n} item", "Moved {n} items", pairs.len() as u64));
                }
                Msg::WheelOpen(kind, files) => {
                    self.wheel_text = None;
                    self.show_wheel(kind, files)
                }
                Msg::WheelOpenText(text) => {
                    if text.chars().count() > ferrocast_core::translate::MAX_CHARS {
                        self.toast("Selection too long to translate");
                    } else {
                        self.wheel_text = Some(text);
                        self.show_wheel(ferrocast_core::wheel::WheelKind::Translate, vec![]);
                    }
                }
                Msg::Translated { run, lang, source_pid, result, ms, target } => {
                    use ferrocast_core::models::TranslateOutput;
                    use ferrocast_core::translate::{focus_guard, outcome, plan, Delivery, Outcome};
                    let engine = ferrocast_core::translate::engine_label(&self.settings);
                    instance::log(&self.store.root, &translate_log_line(&lang, &engine, &result, ms));
                    if run == self.translate_run {
                        self.translated_run = Some(run);
                    }
                    // the effect of THIS translation: an overlay goes before any delivery, a page effect
                    // is ended by the route below (replaced by the marked script, or unmarked)
                    let mine = self.translating_effect.as_ref().is_some_and(|e| e.run == run);
                    let web_fx = match self.translating_effect.as_ref().map(|e| &e.kind) {
                        Some(EffectKind::Web(b)) if mine => Some(*b),
                        _ => None,
                    };
                    if mine && web_fx.is_none() {
                        self.end_effect(false);
                    }
                    let mut replaced = false;
                    // the translation can arrive seconds later: never write into an app the user moved to
                    let front = (self.fx.frontmost)();
                    let output = self.settings.translate_output;
                    if output != TranslateOutput::Auto {
                        // make a non-Auto setting visible in the log: it never routes in place
                        instance::log(
                            &self.store.root,
                            &format!("translate deliver {lang} -> {output:?} (the setting is not Auto, so nothing is replaced in place)"),
                        );
                    }
                    match outcome(result, &lang, output) {
                        Outcome::Show { text, lang } if output == TranslateOutput::Auto => {
                            let delivery = plan(output, target, front, source_pid);
                            instance::log(&self.store.root, &translate_deliver_line(&lang, delivery, ""));
                            match (delivery, source_pid) {
                                (Delivery::SetSelection, Some(pid)) => {
                                    self.toast("Replacing\u{2026}");
                                    (self.fx.ax_replace)(pid, &lang, &text, &self.bus);
                                }
                                (Delivery::WebReplace(b), _) => {
                                    let js = match web_fx {
                                        Some(fxb) => {
                                            if fxb != b {
                                                (self.fx.web_script)(fxb, &ferrocast_core::translate::web_unmark_js());
                                            }
                                            replaced = true;
                                            self.web_pending = Some(b);
                                            ferrocast_core::translate::web_replace_marked_js(&text)
                                        }
                                        None => ferrocast_core::translate::web_replace_js(&text),
                                    };
                                    (self.fx.web_replace)(b, &lang, &text, &js, &self.bus)
                                }
                                _ => self.show_translation(lang, text),
                            }
                        }
                        other => match focus_guard(other, &lang, source_pid, front) {
                            Outcome::Show { text, lang } => self.show_translation(lang, text),
                            Outcome::Paste(t) => self.paste_text(&t),
                            Outcome::Copy(t, toast) => {
                                self.copy_text(&t);
                                self.toast(toast);
                            }
                            Outcome::Fail(toast) => self.toast(toast),
                        },
                    }
                    if web_fx.is_some() {
                        self.end_effect(replaced);
                    }
                }
                Msg::EffectStarted { run, kind } => {
                    if run != self.translate_run {
                        continue; // an older translation's probe
                    }
                    if self.translated_run == Some(run) {
                        // the translation won the race: end it before it is ever shown
                        self.finish_effect(kind, "hidden");
                        continue;
                    }
                    self.end_effect(false);
                    match kind {
                        EffectKind::None => self.finish_effect(kind, "hidden"),
                        kind => {
                            if let EffectKind::Overlay(t) = &kind {
                                (self.fx.glow_show)(t);
                            }
                            self.translating_effect = Some(ActiveEffect { run, kind });
                            let left = EFFECT_TIMEOUT.saturating_sub(self.translate_started.elapsed());
                            self.ctx.request_repaint_after(left + Duration::from_secs(1));
                        }
                    }
                }
                Msg::AxReplaced { lang, text, ok } => {
                    if ok {
                        instance::log(&self.store.root, &format!("translate deliver {lang} -> set-selection ok"));
                        self.toast("Replaced");
                    } else {
                        instance::log(&self.store.root, &format!("translate deliver {lang} -> set-selection fallback-paste"));
                        (self.fx.paste)(self, &text);
                        self.toast("Pasted");
                    }
                }
                Msg::WebReplaced { lang, text, result } => {
                    use ferrocast_core::translate::{WebResult, JS_PERMISSION_HINT};
                    let pending = self.web_pending.take();
                    if let (Some(b), false) = (pending, matches!(result, WebResult::Replaced)) {
                        // the page must be left clean when the window is shown instead
                        (self.fx.web_script)(b, &ferrocast_core::translate::web_unmark_js());
                    }
                    let (toast, reason) = match result {
                        WebResult::Replaced => {
                            instance::log(&self.store.root, &format!("translate deliver {lang} -> web-replace ok"));
                            self.toast("Replaced in the page");
                            return;
                        }
                        WebResult::NoSelection => ("The page selection was lost; translation shown instead".to_string(), "no-selection"),
                        WebResult::Failed(e) => {
                            let l = e.to_lowercase();
                            if l.contains("javascript") || l.contains("apple events") || l.contains("not allowed") {
                                (JS_PERMISSION_HINT.to_string(), "js-permission")
                            } else {
                                (e.lines().next().unwrap_or("").trim().to_string(), "error")
                            }
                        }
                    };
                    instance::log(&self.store.root, &format!("translate deliver {lang} -> web-replace fallback-show: {reason}"));
                    self.show_translation(lang, text);
                    self.toast(toast);
                }
                Msg::WheelRelease => self.release_wheel(),
                Msg::WheelSwitch(k) => self.switch_wheel(k),
                Msg::ShelfChanged(v) => {
                    self.shelf = v;
                    let v = self.shelf.clone();
                    self.persist("shelf.json", &v);
                }
                Msg::FinderSelection(sel) => match sel {
                    Ok(files) if !files.is_empty() => {
                        self.show();
                        self.reset_to_root();
                        self.perform(Act::OpenWheel(files), true);
                    }
                    Ok(_) => {
                        self.show();
                        self.toast("Nothing is selected in Finder");
                    }
                    Err(e) => {
                        self.show();
                        self.toast(trf("Finder selection: {e} (allow Automation for Finder in System Settings)", &[("e", &e.to_string())]));
                    }
                },
                Msg::FinderEnter => self.on_finder_enter(),
                Msg::FinderEnterDone(sel) => self.on_finder_enter_done(sel),
                Msg::ConvertFiles(files) => {
                    self.show();
                    self.reset_to_root();
                    self.push(View::Convert(files));
                }
                Msg::AiModels(k, r) => {
                    self.ai_models.insert(k, Some(r));
                }
                Msg::AppleLanguage(lang, state) => {
                    self.apple_langs.insert(lang, state);
                }
                Msg::ChatGptDeviceCode(code) => {
                    self.show();
                    self.push(View::Output {
                        run: 0,
                        title: tr("Sign in with ChatGPT").into_owned(),
                        text: format!(
                            "Your code:\n\n    {code}\n\nIt is already on your clipboard. A browser tab with {} should be open; paste the code there and approve. This screen can be closed; Ferrocast keeps waiting in the background and will tell you when it is done.",
                            ferrocast_core::chatgpt::DEVICE_VERIFICATION_URL
                        ),
                        done: true,
                    });
                }
                Msg::ThemePath { export, path } => self.theme_file_picked(export, path),
                Msg::IconBytes(k, b) => {
                    if !self.icons.map.contains_key(&k) {
                        self.icons.prefetched.insert(k, b);
                    }
                }
                Msg::AiChunk { run, text } => {
                    for v in self.stack.iter_mut() {
                        if let View::Ai { answer, run: r, .. } = v {
                            if *r == run {
                                answer.push_str(&text);
                            }
                        }
                    }
                }
                Msg::SyncTick => {
                    if self.sync_on() {
                        self.sync_all();
                    }
                }
                Msg::SyncDone(done) => self.sync_done(*done),
                Msg::AiDone { run, error } => {
                    let ok = error.is_none();
                    for v in self.stack.iter_mut() {
                        if let View::Ai { answer, busy, run: r, .. } = v {
                            if *r == run {
                                *busy = false;
                                if let Some(e) = &error {
                                    if !answer.is_empty() {
                                        answer.push('\n');
                                    }
                                    answer.push_str(&format!("⚠ {e}"));
                                }
                            }
                        }
                    }
                    if ok {
                        self.save_ai_chat();
                    }
                }
                Msg::ImportFile(path) => {
                    let outcome = std::fs::read_to_string(&path)
                        .map_err(|e| format!("Could not read the file: {e}"))
                        .and_then(|t| ferrocast_core::import::parse_snippets(&t));
                    let text = match outcome {
                        Ok(incoming) => {
                            let (added, conflicts) = ferrocast_core::import::merge_snippets(&mut self.snippets, incoming);
                            self.save_snippets();
                            format!(
                                "Imported {added} snippet(s){}",
                                if conflicts > 0 { format!(", {conflicts} keyword(s) already in use were cleared") } else { String::new() }
                            )
                        }
                        Err(e) => {
                            instance::log(&self.store.root, &format!("snippet import failed: {e}"));
                            e
                        }
                    };
                    if self.visible {
                        self.toast(text)
                    } else {
                        services::notify("Ferrocast", &text)
                    }
                }
                Msg::Rates(r) => {
                    let _ = self.store.save("rates.json", &r);
                    self.rates = r;
                }
                Msg::Tiling(e) => self.tiling_event(e),
                Msg::TilingQuick(q) => {
                    let w = self.quick_window;
                    self.run_tiling_on(q.action(), None, w);
                }
                Msg::ColorPicked(Some(c)) => {
                    let hex = c.formats()[0].1.clone();
                    // Not marked as our own write, so the colour also lands in the clipboard history.
                    notion_io::write_clipboard(&hex);
                    self.toast(trf("Copied {hex}", &[("hex", &hex)]));
                }
                Msg::ColorPicked(None) => {}
                Msg::Tray(id) => match id.as_str() {
                    "open" => self.show(),
                    "tiling_pause" => self.run_tiling(ferrocast_core::tiling::Action::Pause, None),
                    id if id.starts_with("tiling_layout_") => {
                        if let Some(n) = id.strip_prefix("tiling_layout_").and_then(|n| n.parse::<u8>().ok()) {
                            self.tray_layout = n;
                            self.sync_tiling();
                            self.run_tiling(ferrocast_core::tiling::Action::Layout(n), None);
                        }
                    }
                    "clipboard" => {
                        self.show();
                        self.reset_to_root();
                        self.push(View::Clipboard);
                    }
                    "settings" => {
                        self.show();
                        self.reset_to_root();
                        self.push(View::Settings);
                    }
                    "check_updates" => self.check_updates(),
                    "notes_toggle" => self.open_notes_window(|c, d| c.toggle(d)),
                    "stop_recording" => {
                        let effect = self.record.request_stop();
                        self.perform_record(effect);
                    }
                    "quit" => self.quit(),
                    _ => {}
                },
                Msg::Hotkey(id) => {
                    let target = self.hotkeys.as_ref().and_then(|h| h.map.get(&id).cloned());
                    match target.as_deref() {
                        Some("__launcher") => self.toggle(),
                        // a stored capture shortcut on a Mac that cannot capture: say why, once
                        Some(cmd) if crate::record_ctl::is_gated_command(cmd) && !self.base.iter().any(|i| i.id == cmd) => {
                            self.capture_gate();
                        }
                        Some(cmd) => {
                            if let Some(item) = self.base.iter().find(|i| i.id == cmd).cloned() {
                                self.prev_pid = running::frontmost().map(|f| f.pid).filter(|p| *p != std::process::id() as i32);
                                // the Notes commands open their own floating window, not the launcher
                                let own_window = matches!(
                                    item.kind,
                                    Kind::Builtin(
                                        Builtin::Notes
                                            | Builtin::CreateNote
                                            | Builtin::CreateDictatedNote
                                            | Builtin::CaptureArea
                                            | Builtin::CaptureWindow
                                            | Builtin::CaptureScreen
                                            | Builtin::CaptureScrolling
                                            | Builtin::RecordScreen
                                            | Builtin::RecordArea
                                            | Builtin::RecordWindow
                                            | Builtin::OpenLastRecording
                                    )
                                );
                                let needs_ui =
                                    !own_window && matches!(item.kind, Kind::Builtin(_) | Kind::Quicklink { .. } | Kind::Script(_));
                                if needs_ui && !self.visible {
                                    self.show();
                                }
                                self.run_item(&item);
                            }
                        }
                        None => {}
                    }
                }
                Msg::Record(m) => self.handle_record(m),
                Msg::Editor(m) => self.handle_editor(*m),
                Msg::Capture(outcome) => self.finish_capture(outcome),
                Msg::CaptureListed(flow, listed) => self.capture_listed(flow, listed),
                Msg::CaptureSelected(flow, selection, shareable) => self.capture_selected(flow, selection, *shareable),
                Msg::CaptureScroll(outcome, note) => {
                    self.finish_capture(outcome);
                    if let Some(n) = note {
                        self.capture_say(n);
                    }
                }
                Msg::OcrDone(text, toast) => {
                    if let Some(t) = text {
                        clipboard::write_text(&t);
                        self.shared.mark_clipboard_write();
                    }
                    self.capture_say(toast);
                }
                Msg::Script { run, text, code } => match self.runs.remove(&run) {
                    Some(RunKind::Output) => {
                        for v in self.stack.iter_mut() {
                            if let View::Output { run: r, text: t, done, .. } = v {
                                if *r == run {
                                    *t = if code == 0 { text.clone() } else { format!("{text}\n[exit code {code}]") };
                                    *done = true;
                                }
                            }
                        }
                    }
                    Some(RunKind::Compact(title)) => {
                        let line = script::last_line(&text);
                        let line = if line.is_empty() { format!("{title} finished") } else { line };
                        if self.visible {
                            self.toast(line);
                        } else {
                            services::notify(&title, &line);
                        }
                    }
                    Some(RunKind::Silent(title)) => {
                        let line = script::last_line(&text);
                        services::notify(&title, if line.is_empty() { "Done" } else { &line });
                    }
                    Some(RunKind::Inline(p)) => {
                        self.inline.insert(p, (script::first_line(&text), Instant::now()));
                        self.rebuild();
                    }
                    None => {}
                },
            }
        }
        if handled >= MAX_MSGS_PER_FRAME {
            // more is waiting: come back next frame instead of holding this one
            self.ctx.request_repaint();
        }
    }

    fn tick_inline(&mut self) {
        let mut due = vec![];
        for (i, s) in self.scripts.iter().enumerate() {
            // Confirmation-marked scripts may never auto-run in the background.
            if s.mode == Mode::Inline && !s.needs_confirmation {
                if let Some(secs) = s.refresh_secs() {
                    let fresh = self.inline.get(&s.path).is_some_and(|(_, t)| t.elapsed().as_secs() < secs);
                    if !fresh && !self.runs.values().any(|r| matches!(r, RunKind::Inline(p) if *p == s.path)) {
                        due.push(i);
                    }
                }
            }
        }
        for i in due {
            let s = self.scripts[i].clone();
            let id = self.next_run;
            self.next_run += 1;
            self.runs.insert(id, RunKind::Inline(s.path.clone()));
            services::run_script(&s, vec![], id, self.bus.clone());
        }
        if self.scripts.iter().any(|s| s.mode == Mode::Inline) {
            self.ctx.request_repaint_after(Duration::from_secs(10));
        }
    }

    // ----------------------------------------------------------- list data

    fn rankable<'a>(&'a self, i: &'a Item) -> Rankable<'a> {
        Rankable { id: &i.id, title: &i.title, subtitle: &i.subtitle, keywords: &i.keywords, alias: self.settings.alias(&i.id) }
    }

    fn root_sections(&self) -> Vec<Section> {
        let q = self.query.trim();
        let now = now_secs();
        let mut out = vec![];
        if q.is_empty() {
            let favs: Vec<Item> = self.settings.favorites.iter().filter_map(|f| self.base.iter().find(|i| &i.id == f).cloned()).collect();
            if !favs.is_empty() {
                out.push(Section { title: tr("Favorites").into_owned(), items: favs });
            }
            let mut recent: Vec<&Item> =
                self.base.iter().filter(|i| !self.settings.is_favorite(&i.id) && self.frecency.last_used(&i.id).is_some()).collect();
            recent.sort_by(|a, b| self.frecency.score(&b.id, now).total_cmp(&self.frecency.score(&a.id, now)));
            recent.truncate(8);
            if !recent.is_empty() {
                out.push(Section { title: tr("Suggestions").into_owned(), items: recent.into_iter().cloned().collect() });
            }
            let cmds: Vec<Item> = self
                .base
                .iter()
                .filter(|i| matches!(i.kind, Kind::Builtin(_)) && !self.settings.is_favorite(&i.id))
                .take(10)
                .cloned()
                .collect();
            out.push(Section { title: tr("Commands").into_owned(), items: cmds });
            let apps: Vec<Item> = self
                .base
                .iter()
                .filter(|i| matches!(i.kind, Kind::App { .. }) && !self.settings.is_favorite(&i.id))
                .take(30)
                .cloned()
                .collect();
            if !apps.is_empty() {
                out.push(Section { title: tr("Applications").into_owned(), items: apps });
            }
            return out;
        }
        // calculator / color / url
        let mut top = vec![];
        if let Some((question, answer)) = ferrocast_core::timezones::evaluate(q, chrono::Utc::now()) {
            let r = calc::CalcResult { question, value: 0.0, unit: None, display: answer.clone(), raw: answer };
            top.push(Item {
                id: "time".into(),
                title: r.display.clone(),
                subtitle: r.question.clone(),
                icon: Icon::Glyph("sf:timer", [255, 99, 99]),
                accessory: tr("Time").into_owned(),
                keywords: vec![],
                kind: Kind::Calc(r),
            });
        }
        if let Some(r) = calc::evaluate_with_rates(q, self.settings.rem_px, Some(&self.rates)) {
            top.push(Item {
                id: "calc".into(),
                title: r.display.clone(),
                subtitle: r.question.clone(),
                icon: Icon::Glyph("sf:equal", [255, 99, 99]),
                accessory: tr("Calculator").into_owned(),
                keywords: vec![],
                kind: Kind::Calc(r),
            });
        }
        if let Some(c) = Rgba::parse(q) {
            top.push(Item {
                id: "color".into(),
                title: c.formats()[0].1.clone(),
                subtitle: q.into(),
                icon: Icon::Color(c),
                accessory: tr("Color").into_owned(),
                keywords: vec![],
                kind: Kind::Color(c),
            });
        }
        if let Some(u) = looks_like_url(q) {
            top.push(Item {
                id: "url".into(),
                title: trf("Open {u}", &[("u", &u)]),
                subtitle: String::new(),
                icon: Icon::Glyph("sf:globe", [86, 194, 255]),
                accessory: tr("URL").into_owned(),
                keywords: vec![],
                kind: Kind::Url(u),
            });
        }
        if let Some(t) = ferrocast_core::timer::parse(q) {
            let what = if t.label.is_empty() {
                ferrocast_core::timer::describe(t.secs)
            } else {
                format!("{} ({})", t.label, ferrocast_core::timer::describe(t.secs))
            };
            top.push(Item {
                id: "timer".into(),
                title: format!("Start {what} timer"),
                subtitle: String::new(),
                icon: Icon::Glyph("sf:timer", [245, 165, 36]),
                accessory: tr("Timer").into_owned(),
                keywords: vec![],
                kind: Kind::Timer(t.secs, t.label),
            });
        }
        if !top.is_empty() {
            out.push(Section { title: tr("Result").into_owned(), items: top });
        }
        // alias + argument: "g rust"
        if let Some((first, rest)) = q.split_once(' ') {
            if let Some(it) = self.base.iter().find(|i| {
                self.settings.alias(&i.id).is_some_and(|a| a.eq_ignore_ascii_case(first)) && matches!(i.kind, Kind::Quicklink { .. })
            }) {
                let mut it = it.clone();
                if let Kind::Quicklink { id, .. } = &it.kind {
                    it.kind = Kind::Quicklink { id: id.clone(), arg: Some(rest.trim().to_string()) };
                }
                out.push(Section { title: tr("Quicklink").into_owned(), items: vec![it] });
            }
        }
        let mut scored: Vec<(f64, &Item)> = self
            .base
            .iter()
            .filter_map(|i| ranking::score(&self.rankable(i), q, &self.frecency, now, self.settings.sensitivity).map(|s| (s, i)))
            .collect();
        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
        scored.truncate(40);
        if !scored.is_empty() {
            out.push(Section { title: tr("Results").into_owned(), items: scored.into_iter().map(|(_, i)| i.clone()).collect() });
        }
        if self.settings.file_search_in_root && self.files.0 == q && !self.files.1.is_empty() {
            out.push(Section { title: tr("Files").into_owned(), items: self.files.1.iter().take(8).map(|p| file_item(p)).collect() });
        }
        let mut fb = vec![Item {
            id: "fallback:web".into(),
            title: trf("Search the web for “{q}”", &[("q", q)]),
            subtitle: String::new(),
            icon: Icon::Glyph("sf:magnifyingglass", [120, 120, 130]),
            accessory: tr("Fallback").into_owned(),
            keywords: vec![],
            kind: Kind::WebSearch(q.to_string()),
        }];
        fb.push(Item {
            id: "fallback:files".into(),
            title: trf("Search files for “{q}”", &[("q", q)]),
            subtitle: String::new(),
            icon: Icon::Glyph("sf:magnifyingglass", [120, 120, 130]),
            accessory: tr("Fallback").into_owned(),
            keywords: vec![],
            kind: Kind::Prefilled(Builtin::SearchFiles, q.to_string()),
        });
        fb.push(Item {
            id: "fallback:emoji".into(),
            title: trf("Search emoji for “{q}”", &[("q", q)]),
            subtitle: String::new(),
            icon: Icon::Glyph("sf:face.smiling", [255, 197, 49]),
            accessory: tr("Fallback").into_owned(),
            keywords: vec![],
            kind: Kind::Prefilled(Builtin::SearchEmoji, q.to_string()),
        });
        if q.split_whitespace().count() <= 3 && q.chars().all(|c| c.is_alphabetic() || c == ' ' || c == '-' || c == '\'') {
            fb.push(Item {
                id: "fallback:dict".into(),
                title: trf("Look up “{q}” in Dictionary", &[("q", q)]),
                subtitle: String::new(),
                icon: Icon::Glyph("sf:character.book.closed", [120, 120, 130]),
                accessory: tr("Fallback").into_owned(),
                keywords: vec![],
                kind: Kind::Url(format!("dict://{}", q.replace(' ', "%20"))),
            });
        }
        fb.push(Item {
            id: "fallback:ai".into(),
            title: trf("Ask AI: “{q}”", &[("q", q)]),
            subtitle: String::new(),
            icon: Icon::Glyph("sf:sparkles", [168, 110, 255]),
            accessory: tr("Fallback").into_owned(),
            keywords: vec![],
            kind: Kind::Prefilled(Builtin::AskAi, q.to_string()),
        });
        out.push(Section { title: tr("Use with…").into_owned(), items: fb });
        out
    }

    fn matches(&self, q: &str, title: &str, extra: &str) -> Option<i64> {
        if q.is_empty() {
            return Some(0);
        }
        ferrocast_core::fuzzy::fuzzy_match(q, title, self.settings.sensitivity)
            .map(|m| m.score + 100)
            .or_else(|| ferrocast_core::fuzzy::fuzzy_match(q, extra, self.settings.sensitivity).map(|m| m.score))
    }

    fn sorted(&self, v: Vec<(i64, Item)>, title: &str) -> Vec<Section> {
        let mut v = v;
        if !self.query.trim().is_empty() {
            v.sort_by_key(|a| std::cmp::Reverse(a.0));
        }
        vec![Section { title: tr(title).into_owned(), items: v.into_iter().map(|(_, i)| i).collect() }]
    }

    /// Anything that can change the result list calls this; the next frame recomputes.
    pub(crate) fn invalidate_sections(&mut self) {
        self.sec_dirty = true;
    }

    /// `sections()` with a one-entry cache keyed on query and view. Building the list runs the
    /// fuzzy matcher, the calculator and clipboard filtering, so it must not repeat every frame.
    pub(crate) fn sections_cached(&mut self) -> Vec<Section> {
        let key = format!("{}\u{0}{}\u{0}{}", self.query, self.stack.len(), self.view().title());
        if !self.sec_dirty {
            if let Some((k, v)) = &self.sec_cache {
                if *k == key {
                    return v.clone();
                }
            }
        }
        let t0 = Instant::now();
        let v = self.sections();
        if std::env::var_os("FERROCAST_PROFILE").is_some() {
            eprintln!("sections {:?} q={:?}", t0.elapsed(), self.query);
        }
        if !self.wide_font && v.iter().flat_map(|s| s.items.iter()).any(|i| needs_wide_font(&i.title) || needs_wide_font(&i.subtitle)) {
            self.wide_font = true;
            load_fonts(&self.ctx, true);
        }
        // Fetch the icons this list will show before the painter asks for them.
        let want: Vec<String> = v
            .iter()
            .flat_map(|s| s.items.iter())
            .filter_map(|i| match &i.icon {
                Icon::App(p) => {
                    let k = format!("app:{p}");
                    (!self.icons.map.contains_key(&k) && !self.icons.prefetched.contains_key(&k) && self.icons.inflight.insert(k))
                        .then(|| p.clone())
                }
                _ => None,
            })
            .take(40)
            .collect();
        if !want.is_empty() {
            services::prefetch_app_icons(want, self.bus.clone());
        }
        self.sec_cache = Some((key, v.clone()));
        self.sec_dirty = false;
        v
    }

    fn sections(&self) -> Vec<Section> {
        let q = self.query.trim().to_string();
        match self.view() {
            View::Root => self.root_sections(),
            View::ExtList => self.ext_sections(&q),
            View::Clipboard => {
                let list = self.shared.clipboard.lock().unwrap().clone();
                let now = now_secs();
                let (mut pinned, mut today, mut yest, mut older) = (vec![], vec![], vec![], vec![]);
                for e in list {
                    let hay = format!("{} {}", e.display_title(), e.content);
                    if !q.is_empty() && !hay.to_lowercase().contains(&q.to_lowercase()) {
                        continue;
                    }
                    let icon = match e.kind {
                        ClipKind::Image => Icon::ClipImage(e.content.clone()),
                        ClipKind::Color => {
                            Rgba::parse(&e.content).map(Icon::Color).unwrap_or(Icon::Glyph("sf:doc.on.clipboard", [255, 99, 99]))
                        }
                        ClipKind::Link => Icon::Glyph("sf:link", [86, 194, 255]),
                        ClipKind::File => Icon::Glyph("sf:doc", [120, 120, 130]),
                        ClipKind::Text => Icon::Glyph("sf:text.alignleft", [90, 90, 105]),
                    };
                    let age = now - e.copied_at;
                    let pinned_flag = e.pinned;
                    let it = Item {
                        id: format!("clip:{}", e.id),
                        title: e.display_title(),
                        subtitle: String::new(),
                        icon,
                        accessory: e.source_app.clone().unwrap_or_default(),
                        keywords: vec![],
                        kind: Kind::Clip(e.id.clone()),
                    };
                    if pinned_flag {
                        pinned.push(it)
                    } else if age < 86400 {
                        today.push(it)
                    } else if age < 2 * 86400 {
                        yest.push(it)
                    } else {
                        older.push(it)
                    }
                }
                [("Pinned", pinned), ("Today", today), ("Yesterday", yest), ("Earlier", older)]
                    .into_iter()
                    .filter(|(_, v)| !v.is_empty())
                    .map(|(t, items)| Section { title: tr(t).into_owned(), items })
                    .collect()
            }
            View::Snippets => {
                let v = self
                    .snippets
                    .iter()
                    .filter_map(|s| {
                        self.matches(&q, &s.name, &s.text).map(|sc| {
                            (
                                sc,
                                Item {
                                    id: format!("snippet:{}", s.id),
                                    title: s.name.clone(),
                                    subtitle: s.text.lines().next().unwrap_or("").chars().take(60).collect(),
                                    icon: Icon::Glyph("sf:scissors", [255, 167, 38]),
                                    accessory: s.keyword.clone(),
                                    keywords: vec![],
                                    kind: Kind::Snippet(s.id.clone()),
                                },
                            )
                        })
                    })
                    .collect();
                self.sorted(v, "Snippets")
            }
            View::Quicklinks => {
                let v = self
                    .quicklinks
                    .iter()
                    .filter_map(|l| {
                        self.matches(&q, &l.name, &l.link).map(|sc| {
                            (
                                sc,
                                Item {
                                    id: format!("quicklink:{}", l.id),
                                    title: l.name.clone(),
                                    subtitle: l.link.chars().take(70).collect(),
                                    icon: Icon::Glyph("sf:link", [86, 194, 255]),
                                    accessory: String::new(),
                                    keywords: vec![],
                                    kind: Kind::Quicklink { id: l.id.clone(), arg: None },
                                },
                            )
                        })
                    })
                    .collect();
                self.sorted(v, "Quicklinks")
            }
            View::Emoji => {
                let ql = q.to_lowercase();
                // Rank before truncating so a name that starts with the query is never cut off
                // by weaker matches that happen to come first in the catalogue.
                let mut hits: Vec<_> =
                    emojis::iter().filter(|e| ql.is_empty() || e.name().contains(&ql) || e.shortcodes().any(|s| s.contains(&ql))).collect();
                hits.sort_by_key(|e| !e.name().starts_with(&ql));
                hits.truncate(300);
                let recent: Vec<Item> = if ql.is_empty() {
                    self.recent_emoji
                        .iter()
                        .filter_map(|r| emojis::get(r))
                        .map(|e| Item {
                            id: format!("emoji:{}", e.as_str()),
                            title: e.name().to_string(),
                            subtitle: e.shortcode().map(|s| format!(":{s}:")).unwrap_or_default(),
                            icon: Icon::Emoji(e.as_str().to_string()),
                            accessory: tr("Recent").into_owned(),
                            keywords: vec![],
                            kind: Kind::Emoji(e.as_str()),
                        })
                        .collect()
                } else {
                    vec![]
                };
                let v: Vec<(i64, Item)> = hits
                    .into_iter()
                    .map(|e| {
                        (
                            if e.name().starts_with(&ql) { 1 } else { 0 },
                            Item {
                                id: format!("emoji:{}", e.as_str()),
                                title: e.name().to_string(),
                                subtitle: e.shortcode().map(|s| format!(":{s}:")).unwrap_or_default(),
                                icon: Icon::Emoji(e.as_str().to_string()),
                                accessory: tr(&humanize_group(&format!("{:?}", e.group()))).into_owned(),
                                keywords: vec![],
                                kind: Kind::Emoji(e.as_str()),
                            },
                        )
                    })
                    .collect();
                let mut out = self.sorted(v, "Emoji");
                if !recent.is_empty() {
                    out.insert(0, Section { title: tr("Recently used").into_owned(), items: recent });
                }
                out
            }
            View::Files => {
                if self.files.0 == q && !self.files.1.is_empty() {
                    vec![Section { title: tr("Files").into_owned(), items: self.files.1.iter().map(|p| file_item(p)).collect() }]
                } else {
                    vec![]
                }
            }
            View::Windows => {
                let v = self
                    .win_cache
                    .iter()
                    .filter_map(|w| {
                        let t = if w.title.is_empty() { w.app.clone() } else { w.title.clone() };
                        self.matches(&q, &t, &w.app).map(|sc| {
                            (
                                sc,
                                Item {
                                    id: format!("win:{}", w.id),
                                    title: t,
                                    subtitle: w.app.clone(),
                                    icon: Icon::Glyph("sf:macwindow.on.rectangle", [89, 212, 153]),
                                    accessory: String::new(),
                                    keywords: vec![],
                                    kind: Kind::OpenWindow(w.clone()),
                                },
                            )
                        })
                    })
                    .collect();
                self.sorted(v, "Windows")
            }
            View::Processes => {
                let v = self
                    .proc_cache
                    .iter()
                    .filter_map(|pr| {
                        self.matches(&q, &pr.name, &pr.path).map(|sc| {
                            (
                                sc,
                                Item {
                                    id: format!("proc:{}", pr.pid),
                                    title: pr.name.clone(),
                                    subtitle: format!("PID {}", pr.pid),
                                    icon: pr.bundle().map(Icon::App).unwrap_or(Icon::Glyph("sf:chevron.right", [108, 108, 120])),
                                    accessory: format!("{:.0}% CPU · {:.0} MB", pr.cpu, pr.mem_mb),
                                    keywords: vec![],
                                    kind: Kind::Process(pr.clone()),
                                },
                            )
                        })
                    })
                    .collect();
                self.sorted(v, "Processes")
            }
            View::Convert(files) => {
                let files = files.clone();
                let Some(first) = files.first() else { return vec![] };
                let names: Vec<String> = files.iter().filter_map(|f| f.file_name().map(|n| n.to_string_lossy().to_string())).collect();
                let sub = if files.len() == 1 { names[0].clone() } else { format!("{} files · {}", files.len(), names.join(", ")) };
                let v: Vec<(i64, Item)> = ferrocast_core::convert::targets(first, &self.tools)
                    .into_iter()
                    .filter_map(|t| {
                        self.matches(&q, t.label, t.ext).map(|sc| {
                            (
                                sc,
                                Item {
                                    id: format!("convert:{}", t.ext),
                                    title: t.label.to_string(),
                                    subtitle: sub.clone(),
                                    icon: Icon::Glyph(
                                        match ferrocast_core::convert::family_of(first) {
                                            ferrocast_core::convert::Family::Image | ferrocast_core::convert::Family::Pdf => "sf:photo",
                                            ferrocast_core::convert::Family::Audio => "sf:waveform",
                                            ferrocast_core::convert::Family::Video => "sf:film",
                                            ferrocast_core::convert::Family::Archive | ferrocast_core::convert::Family::Folder => {
                                                "sf:archivebox"
                                            }
                                            _ => "sf:doc.text",
                                        },
                                        [255, 140, 60],
                                    ),
                                    accessory: match t.needs {
                                        Some(n) => format!("Needs {}", n.split(' ').next().unwrap_or(n)),
                                        None => format!(".{} · {}", t.ext, t.engine.label()),
                                    },
                                    keywords: vec![],
                                    kind: Kind::ConvertTo(files.clone(), t),
                                },
                            )
                        })
                    })
                    .collect();
                let mut out = self.sorted(v, "Convert to");
                // image tools when every file is an image
                use ferrocast_core::convert::{family_of, Family, ImageOp};
                if files.iter().all(|f| family_of(f) == Family::Image) {
                    // a typed size or dimensions ("500kb", "40%", "800x600") becomes a tool of its own
                    let typed = ferrocast_core::convert::parse_image_query(&q);
                    let typed_op = typed.clone();
                    let tools: Vec<Item> = typed
                        .into_iter()
                        .chain(ImageOp::offered(files.len()))
                        // a query that parsed as a tool is shown as typed; fuzzy matching would hide any with parameters
                        .filter(|op| {
                            typed_op.as_ref() == Some(op)
                                || self
                                    .matches(
                                        &q,
                                        &op.label(),
                                        match op {
                                            ImageOp::Compress(_) => "compress smaller jpeg",
                                            ImageOp::Resize(_) => "resize scale",
                                            ImageOp::Crop(..) => "crop aspect",
                                            ImageOp::StripMetadata => "metadata exif gps strip",
                                            ImageOp::CombinePdf => "pdf combine merge",
                                            ImageOp::Rotate(_) => "rotate turn",
                                            ImageOp::Flip(_) => "flip mirror",
                                            ImageOp::Background(_) => "background pad square",
                                            ImageOp::Collage | ImageOp::CollageWith(_) => "collage grid row column featured",
                                            ImageOp::FrameWith(_) => "frame background blur shadow padding",
                                            ImageOp::RedactWith(_) => "redact hide blur privacy area",
                                            ImageOp::EditWith(_) => {
                                                "edit photo brightness contrast saturation warmth clarity sharpen grain denoise dehaze"
                                            }
                                            ImageOp::Redact(_) => "redact hide blur privacy face text",
                                            ImageOp::Adjust(_) => "edit adjust photo brightness contrast black white grayscale",
                                            ImageOp::Annotate => "annotate markup draw arrow text preview",
                                            ImageOp::CompressToKb(_) | ImageOp::CompressByPercent(_) | ImageOp::CompressWith(..) => {
                                                "compress size smaller"
                                            }
                                            ImageOp::CropExact(..) | ImageOp::CropRect(..) => "crop exact dimensions pixels",
                                            ImageOp::SetMeta(_) => "metadata exif camera make model gps location tag",
                                        },
                                    )
                                    .is_some()
                        })
                        .map(|op| Item {
                            id: format!("imgtool:{}", op.suffix()),
                            title: op.label(),
                            subtitle: sub.clone(),
                            icon: Icon::Glyph(
                                match op {
                                    ImageOp::Compress(_) => "sf:arrow.down.right.and.arrow.up.left",
                                    ImageOp::Resize(_) => "sf:arrow.up.left.and.down.right.magnifyingglass",
                                    ImageOp::Crop(..) => "sf:crop",
                                    ImageOp::StripMetadata => "sf:eye.slash",
                                    ImageOp::CombinePdf => "sf:doc.richtext",
                                    ImageOp::Rotate(_) => "sf:rotate.right",
                                    ImageOp::Flip(_) => "sf:arrow.left.and.right.righttriangle.left.righttriangle.right",
                                    ImageOp::Background(_) => "sf:square.dashed",
                                    ImageOp::Collage | ImageOp::CollageWith(_) => "sf:square.grid.2x2",
                                    ImageOp::FrameWith(_) => "sf:square.dashed",
                                    ImageOp::RedactWith(_) => "sf:eye.slash.fill",
                                    ImageOp::EditWith(_) => "sf:slider.horizontal.3",
                                    ImageOp::Redact(_) => "sf:eye.slash.fill",
                                    ImageOp::Adjust(_) => "sf:slider.horizontal.3",
                                    ImageOp::Annotate => "sf:pencil.tip.crop.circle",
                                    ImageOp::CompressToKb(_) | ImageOp::CompressByPercent(_) | ImageOp::CompressWith(..) => {
                                        "sf:arrow.down.right.and.arrow.up.left"
                                    }
                                    ImageOp::CropExact(..) | ImageOp::CropRect(..) => "sf:crop",
                                    ImageOp::SetMeta(_) => "sf:tag",
                                },
                                [255, 140, 60],
                            ),
                            accessory: tr("Image tool").into_owned(),
                            keywords: vec![],
                            kind: Kind::ImageTool(files.clone(), op),
                        })
                        .collect();
                    if !tools.is_empty() {
                        out.push(Section { title: tr("Image tools").into_owned(), items: tools });
                    }
                }
                // trim, split and merge for audio, video and PDF
                let fam = family_of(first);
                if files.iter().all(|f| family_of(f) == fam) && matches!(fam, Family::Audio | Family::Video | Family::Pdf | Family::Archive)
                {
                    use ferrocast_core::convert::FileTool;
                    let typed = ferrocast_core::convert::parse_tool_query(&q);
                    let items: Vec<Item> = typed
                        .iter()
                        .cloned()
                        .chain(FileTool::offered(fam, files.len()))
                        .filter(|t| typed.as_ref() == Some(t) || self.matches(&q, &t.label(), "trim split merge cut join").is_some())
                        .map(|t| Item {
                            id: format!("filetool:{}", t.label()),
                            title: t.label(),
                            subtitle: sub.clone(),
                            icon: Icon::Glyph(
                                match t {
                                    FileTool::Trim(..) | FileTool::TrimExact(..) => "sf:scissors",
                                    FileTool::SplitEvery(_) | FileTool::SplitPoints(..) | FileTool::SplitPages => "sf:square.split.2x1",
                                    FileTool::Merge | FileTool::Join => "sf:rectangle.stack",
                                    FileTool::Mute => "sf:speaker.slash",
                                    FileTool::Speed(_) => "sf:speedometer",
                                    FileTool::Snapshot(_) => "sf:camera",
                                    FileTool::CropVideo(..) => "sf:crop",
                                    FileTool::RedactVideo(..)
                                    | FileTool::RedactVideoMany(..)
                                    | FileTool::Bleep(..)
                                    | FileTool::BleepMany(..) => "sf:eye.slash",
                                    FileTool::Normalize(..) | FileTool::Channels(..) => "sf:speaker.wave.2",
                                    FileTool::Visualize(_) | FileTool::VisualizeImage(..) => "sf:waveform",
                                    FileTool::TrimSilence => "sf:scissors",
                                    FileTool::OrganizePdf(_) => "sf:doc.on.doc",
                                    FileTool::Extract => "sf:archivebox",
                                    FileTool::EditMetadata(..) => "sf:tag",
                                    FileTool::Compress(..) | FileTool::CompressPdf(_) => "sf:arrow.down.right.and.arrow.up.left",
                                },
                                [255, 140, 60],
                            ),
                            accessory: if matches!(t, FileTool::CompressPdf(_)) && self.tools.gs.is_none() {
                                "Needs Ghostscript".into()
                            } else {
                                "File tool".into()
                            },
                            keywords: vec![],
                            kind: Kind::FileTool(files.clone(), t),
                        })
                        .collect();
                    if !items.is_empty() {
                        out.push(Section { title: tr("File tools").into_owned(), items });
                    }
                }
                out.extend(self.editor_section(&files, &q, &sub));
                out.extend(self.missing_tools_section(&files, &q, &sub));
                out.retain(|sec| !sec.items.is_empty());
                out
            }
            View::Recordings => {
                let v: Vec<(i64, Item)> = self
                    .rt
                    .recordings
                    .iter()
                    .filter_map(|e| {
                        self.matches(&q, &e.name, "").map(|sc| {
                            let date = chrono::DateTime::from_timestamp(e.created, 0)
                                .map(|d| crate::record_ctl::format_date(d.with_timezone(&chrono::Local).naive_local()))
                                .unwrap_or_default();
                            (
                                sc,
                                Item {
                                    id: format!("rec:{}", e.path.display()),
                                    title: e.name.clone(),
                                    subtitle: crate::record_ctl::entry_subtitle(e),
                                    icon: Icon::Glyph("sf:film", [86, 194, 255]),
                                    accessory: date,
                                    keywords: vec![],
                                    kind: Kind::Recording(e.path.clone()),
                                },
                            )
                        })
                    })
                    .collect();
                self.sorted(v, "Recordings")
            }
            View::Shelf => {
                let v: Vec<(i64, Item)> = self
                    .shelf
                    .iter()
                    .filter(|p| p.exists())
                    .filter_map(|p| {
                        let it = file_item(p);
                        self.matches(&q, &it.title, &it.subtitle).map(|sc| (sc, it))
                    })
                    .collect();
                self.sorted(v, "On the shelf")
            }
            View::CalcHistory => {
                let now = now_secs();
                let v: Vec<(i64, Item)> = self
                    .calc_history
                    .iter()
                    .enumerate()
                    .filter_map(|(i, e)| {
                        self.matches(&q, &e.question, &e.display).map(|sc| {
                            (
                                sc,
                                Item {
                                    id: format!("calc:{i}"),
                                    title: e.display.clone(),
                                    subtitle: e.question.clone(),
                                    icon: Icon::Glyph("sf:equal", [120, 120, 130]),
                                    accessory: services::ago(now - e.at),
                                    keywords: vec![],
                                    kind: Kind::Calc(ferrocast_core::calc::CalcResult {
                                        question: e.question.clone(),
                                        value: e.value,
                                        unit: e.unit.clone(),
                                        display: e.display.clone(),
                                        raw: e.raw.clone(),
                                    }),
                                },
                            )
                        })
                    })
                    .collect();
                self.sorted(v, "History")
            }
            View::Timers => {
                let now = Instant::now();
                let v: Vec<(i64, Item)> = self
                    .timers
                    .iter()
                    .enumerate()
                    .filter_map(|(i, t)| {
                        let title =
                            if t.label.is_empty() { format!("{} timer", ferrocast_core::timer::describe(t.secs)) } else { t.label.clone() };
                        self.matches(&q, &title, "").map(|sc| {
                            let left = t.ends_at.saturating_duration_since(now).as_secs();
                            (
                                sc,
                                Item {
                                    id: format!("timer:{i}"),
                                    title,
                                    subtitle: format!(
                                        "ends at {}",
                                        chrono::Local::now()
                                            .checked_add_signed(chrono::Duration::seconds(left as i64))
                                            .map(|d| d.format("%H:%M").to_string())
                                            .unwrap_or_default()
                                    ),
                                    icon: Icon::Glyph("sf:timer", [245, 165, 36]),
                                    accessory: ferrocast_core::timer::clock(left),
                                    keywords: vec![],
                                    kind: Kind::RunningTimer(i),
                                },
                            )
                        })
                    })
                    .collect();
                self.sorted(v, "Running")
            }
            View::Notes => {
                // pinned first (with their slot), then recent; title and content are searched
                let rows = ferrocast_core::note_actions::browse_rows(&self.notes, &q, None, now_secs());
                let item = |r: &ferrocast_core::note_actions::BrowseRow| Item {
                    id: format!("note:{}", r.id),
                    title: r.title.clone(),
                    subtitle: r.subtitle.clone(),
                    icon: Icon::Glyph(if r.pinned { "sf:pin.fill" } else { "sf:pencil" }, [255, 197, 49]),
                    accessory: r.slot.map(|n| format!("⌘{n}")).unwrap_or_default(),
                    keywords: vec![],
                    kind: Kind::Note(r.id.clone()),
                };
                let (pinned, rest): (Vec<_>, Vec<_>) = rows.iter().partition(|r| r.pinned);
                let mut out = vec![];
                if !pinned.is_empty() {
                    out.push(Section { title: tr("Pinned").into_owned(), items: pinned.into_iter().map(item).collect() });
                }
                if !rest.is_empty() {
                    out.push(Section { title: tr("Notes").into_owned(), items: rest.into_iter().map(item).collect() });
                }
                out
            }
            View::NotionSearch => self.notion_view_sections(),
            View::ObsidianSearch => self.obsidian_view_sections(),
            View::Colors(c) => vec![Section {
                title: tr("Formats").into_owned(),
                items: c
                    .formats()
                    .into_iter()
                    .map(|(label, val)| Item {
                        id: format!("cf:{label}"),
                        title: val.clone(),
                        subtitle: label.into(),
                        icon: Icon::Color(*c),
                        accessory: String::new(),
                        keywords: vec![],
                        kind: Kind::ColorFormat(val),
                    })
                    .collect(),
            }],
            _ => vec![],
        }
    }

    /// Editor windows offered for a single file in the Convert view (same windows as the file actions).
    fn editor_section(&self, files: &[PathBuf], q: &str, sub: &str) -> Option<Section> {
        use ferrocast_core::convert::{ext, family_of, Family};
        let mut list: Vec<(&str, &str, Act)> = vec![];
        if files.len() >= 2 {
            let family = family_of(&files[0]);
            if files.iter().all(|f| family_of(f) == family) {
                let purpose = match family {
                    Family::Pdf => Some(ReorderPurpose::MergePdf),
                    Family::Audio => Some(ReorderPurpose::MergeAudio),
                    Family::Video => Some(ReorderPurpose::JoinVideos),
                    Family::Image => Some(ReorderPurpose::CreatePdf),
                    _ => None,
                };
                if let Some(pu) = purpose {
                    list.push(("Choose order…", "order reorder sort merge join combine", Act::ReorderFiles(pu, files.to_vec())));
                }
            }
        }
        if !files.is_empty() && [Family::Audio, Family::Video].iter().any(|fam| files.iter().all(|f| family_of(f) == *fam)) {
            list.push(("Compress…", "compress smaller size resize reduce bitrate", Act::CompressMedia(files.to_vec())));
        }
        if files.len() >= 2 && files.iter().all(|f| family_of(f) == Family::Image) {
            list.push(("Compress…", "compress smaller size resize reduce", Act::CompressImages(files.to_vec())));
        }
        let [p] = files else { return Self::editor_items(list, self, q, sub) };
        let p = p.clone();
        if meta_kind(&p).is_some() {
            list.push(("Edit metadata…", "metadata tags edit remove title artist author exif", Act::EditMetadata(p.clone())));
        }
        match family_of(&p) {
            Family::Video => list.extend([
                ("Trim Video…", "trim cut frame timeline", Act::TrimVideo(p.clone())),
                ("Crop Video…", "crop video frame size", Act::CropVideo(p.clone())),
                ("Change Speed…", "speed slow fast pitch", Act::SpeedVideo(p.clone())),
                ("Split Video…", "split cut parts clips chapters", Act::SplitVideo(p.clone())),
                ("Video Snapshots…", "snapshot frame screenshot png", Act::SnapshotVideo(p.clone())),
                ("Redact Video…", "redact hide blur privacy area", Act::RedactVideo(p.clone())),
            ]),
            Family::Image => list.extend([
                ("Compress…", "compress smaller size resize reduce", Act::CompressImages(vec![p.clone()])),
                ("Annotate…", "annotate markup draw arrow text", Act::AnnotateCanvas(p.clone())),
                ("Edit Photo…", "edit photo brightness contrast", Act::EditPhoto(p.clone())),
                ("Crop…", "crop aspect exact", Act::CropImage(p.clone())),
                ("Redact Areas…", "redact hide blur privacy", Act::RedactImage(p.clone())),
                ("Add Background…", "background frame blur shadow", Act::FrameImage(p.clone())),
            ]),
            Family::Audio => list.extend([
                ("Trim Audio…", "trim cut waveform silence start end", Act::TrimAudio(p.clone())),
                ("Bleep…", "bleep censor mute beep swear tone ranges", Act::BleepAudio(p.clone())),
                ("Normalize…", "normalize loudness lufs volume level peak", Act::NormalizeAudio(p.clone())),
                ("Channels…", "channels mono stereo left right volume balance", Act::AudioChannels(p.clone())),
                ("Audio to Video…", "audio video visualizer waveform picture image mp4", Act::VisualizeAudio(p.clone())),
            ]),
            _ => {}
        }
        if ext(&p) == "pdf" {
            list.push(("Organize Pages…", "organize pages rotate reorder remove", Act::OrganizePdf(p.clone())));
        }
        Self::editor_items(list, self, q, sub)
    }

    fn editor_items(list: Vec<(&str, &str, Act)>, app: &App, q: &str, sub: &str) -> Option<Section> {
        let items: Vec<Item> = list
            .into_iter()
            .filter(|(title, kw, _)| app.matches(q, title, kw).is_some())
            .map(|(title, _, act)| Item {
                id: format!("editor:{title}"),
                title: title.to_string(),
                subtitle: sub.to_string(),
                icon: Icon::Glyph("sf:slider.horizontal.3", [255, 140, 60]),
                accessory: tr("Editor").into_owned(),
                keywords: vec![],
                kind: Kind::Editor(act),
            })
            .collect();
        (!items.is_empty()).then_some(Section { title: tr("Editors").into_owned(), items })
    }

    /// Tools the format table lists but Ferrocast cannot do yet; shown, not hidden (parity plan A2).
    fn missing_tools_section(&self, files: &[PathBuf], q: &str, sub: &str) -> Option<Section> {
        use ferrocast_core::tools::{implemented, Impl};
        self.tools_section_where(files, q, sub, |t| implemented(t) == Impl::Missing)
    }

    /// The "Tools" rows for the tools `is_missing` selects (the real table, or a synthetic one in tests).
    pub(crate) fn tools_section_where(
        &self,
        files: &[PathBuf],
        q: &str,
        sub: &str,
        is_missing: impl Fn(ferrocast_core::tools::ToolId) -> bool,
    ) -> Option<Section> {
        use ferrocast_core::convert::ext;
        use ferrocast_core::tools::tools_for;
        let first = files.first()?;
        let multi = files.len() > 1;
        let list = tools_for(&ext(first), multi);
        if files.iter().any(|f| tools_for(&ext(f), multi) != list) {
            return None;
        }
        let items: Vec<Item> = list
            .into_iter()
            .filter(|t| is_missing(*t))
            .filter(|t| self.matches(q, tool_label(*t), "tool").is_some())
            .map(|t| Item {
                id: format!("tool:{}", tool_label(t)),
                title: tool_label(t).to_string(),
                subtitle: sub.to_string(),
                icon: Icon::Glyph(tool_symbol(t), [255, 140, 60]),
                accessory: tr("Not available yet").into_owned(),
                keywords: vec![],
                kind: Kind::Editor(Act::Convert(files.to_vec())),
            })
            .collect();
        (!items.is_empty()).then_some(Section { title: tr("Tools").into_owned(), items })
    }

    // --------------------------------------------------------------- actions

    /// Open the action panel for `item` (⌘K, the footer "Actions" hint, a right click).
    pub(crate) fn open_panel(&mut self, item: Item) {
        let actions = self.actions_for(&item);
        self.panel = Some(ActionPanel { actions, query: String::new(), sel: 0, item });
    }

    /// Hover buttons for the common per-item actions of a row; each one carries the `Act` of the
    /// keyboard shortcut it mirrors, taken from `actions_for` so the two cannot drift apart.
    pub(crate) fn row_buttons(&self, item: &Item) -> Vec<RowButton> {
        let own_view = matches!(
            (&item.kind, self.view()),
            (Kind::Clip(_), View::Clipboard)
                | (Kind::Snippet(_), View::Snippets)
                | (Kind::Quicklink { .. }, View::Quicklinks)
                | (Kind::Note(_), View::Notes)
        );
        if !own_view {
            return vec![];
        }
        let acts = self.actions_for(item);
        let pick = |label: &'static str, f: fn(&Act) -> bool| {
            acts.iter().find(|a| f(&a.kind)).map(|a| RowButton {
                label,
                tip: match a.shortcut {
                    Some((m, k)) => format!("{}  {}", tr(&a.title), shortcut_label(m, k)),
                    None => tr(&a.title).into_owned(),
                },
                kind: a.kind.clone(),
                destructive: a.destructive,
            })
        };
        match &item.kind {
            Kind::Clip(_) => {
                let unpin = acts.iter().any(|a| a.title == "Unpin Entry");
                [
                    pick(if unpin { "Unpin" } else { "Pin" }, |a| matches!(a, Act::ClipPin(_))),
                    pick("Delete", |a| matches!(a, Act::ClipDelete(_))),
                ]
                .into_iter()
                .flatten()
                .collect()
            }
            Kind::Note(_) => {
                let unpin = acts.iter().any(|a| a.title == "Unpin Note");
                [
                    pick(if unpin { "Unpin" } else { "Pin" }, |a| matches!(a, Act::NotePin(_))),
                    pick("Delete", |a| matches!(a, Act::NoteDelete(_))),
                ]
                .into_iter()
                .flatten()
                .collect()
            }
            Kind::Snippet(_) => {
                [pick("Edit", |a| matches!(a, Act::EditSnippet(_))), pick("Delete", |a| matches!(a, Act::DeleteSnippet(_)))]
                    .into_iter()
                    .flatten()
                    .collect()
            }
            Kind::Quicklink { .. } => {
                [pick("Edit", |a| matches!(a, Act::EditQuicklink(_))), pick("Delete", |a| matches!(a, Act::DeleteQuicklink(_)))]
                    .into_iter()
                    .flatten()
                    .collect()
            }
            _ => vec![],
        }
    }

    /// View-level mouse controls: "Clear History…" in the clipboard, "+ New" in snippets and quicklinks.
    pub(crate) fn header_buttons(&self) -> Vec<RowButton> {
        match self.view() {
            View::Clipboard => vec![RowButton {
                label: "Clear History…",
                tip: trf("Delete All Entries  {shortcut} (pinned entries stay)", &[("shortcut", &shortcut_label(CTRL_SHIFT, Key::X))]),
                kind: Act::ClipDeleteAll,
                destructive: true,
            }],
            View::Snippets => vec![RowButton {
                label: "+ New",
                tip: trf("Create Snippet  {shortcut}", &[("shortcut", &shortcut_label(CMD, Key::N))]),
                kind: Act::CreateSnippet,
                destructive: false,
            }],
            View::Quicklinks => vec![RowButton {
                label: "+ New",
                tip: trf("Create Quicklink  {shortcut}", &[("shortcut", &shortcut_label(CMD, Key::N))]),
                kind: Act::CreateQuicklink,
                destructive: false,
            }],
            View::Notes => vec![RowButton {
                label: "+ New",
                tip: trf("Create Note  {shortcut}", &[("shortcut", &shortcut_label(CMD, Key::N))]),
                kind: Act::CreateNote,
                destructive: false,
            }],
            _ => vec![],
        }
    }

    fn actions_for(&self, item: &Item) -> Vec<Action> {
        let mut a = vec![];
        match &item.kind {
            Kind::App { path, .. } => {
                a.push(Action::new("Open Application", Act::Run));
                a.push(Action::new("Show in Finder", Act::Reveal(PathBuf::from(path))).key(CMD, Key::Enter));
                a.push(Action::new("Copy Path", Act::CopyText(path.clone())).key(CMD_SHIFT, Key::C));
                if let Some(r) = running::running_apps()
                    .into_iter()
                    .find(|r| r.path.as_deref().is_some_and(|p| p.trim_end_matches('/') == path.trim_end_matches('/')))
                {
                    a.push(Action::new("Quit Application", Act::QuitApp(r.pid)).key(CMD_SHIFT, Key::Q));
                    a.push(Action::new("Hide Application", Act::HideApp(r.pid)).key(CMD_SHIFT, Key::H));
                    a.push(Action::new("Force Quit Application", Act::ForceQuitApp(r.pid)).danger());
                }
            }
            Kind::Quicklink { id, .. } => {
                a.push(Action::new("Open Quicklink", Act::Run));
                if let Some(q) = self.quicklinks.iter().find(|q| &q.id == id) {
                    a.push(Action::new("Copy Link", Act::CopyText(q.link.clone())).key(CMD_SHIFT, Key::C));
                }
                a.push(Action::new("Edit Quicklink", Act::EditQuicklink(id.clone())).key(CMD, Key::E));
                a.push(Action::new("Duplicate Quicklink", Act::DuplicateQuicklink(id.clone())).key(CMD, Key::D));
                a.push(Action::new("Create Quicklink", Act::CreateQuicklink).key(CMD, Key::N));
                a.push(Action::new("Delete Quicklink", Act::DeleteQuicklink(id.clone())).key(CTRL, Key::X).danger());
            }
            Kind::Snippet(id) => {
                a.push(Action::new("Paste Snippet to Active App", Act::Run));
                if let Some(s) = self.snippets.iter().find(|s| &s.id == id) {
                    a.push(Action::new("Copy Snippet to Clipboard", Act::CopyText(s.text.clone())).key(CMD, Key::Enter));
                }
                a.push(Action::new("Edit Snippet", Act::EditSnippet(id.clone())).key(CMD, Key::E));
                a.push(Action::new("Duplicate Snippet", Act::DuplicateSnippet(id.clone())).key(CMD, Key::D));
                a.push(Action::new("Create Snippet", Act::CreateSnippet).key(CMD, Key::N));
                a.push(Action::new("Delete Snippet", Act::DeleteSnippet(id.clone())).key(CTRL, Key::X).danger());
            }
            Kind::Clip(id) => {
                a.push(Action::new("Paste to Active App", Act::ClipPaste(id.clone())));
                a.push(Action::new("Copy to Clipboard", Act::ClipCopy(id.clone())).key(CMD, Key::Enter));
                let pinned = self.shared.clipboard.lock().unwrap().iter().any(|e| &e.id == id && e.pinned);
                a.push(Action::new(if pinned { "Unpin Entry" } else { "Pin Entry" }, Act::ClipPin(id.clone())).key(CMD, Key::Period));
                a.push(Action::new("Save as Snippet", Act::ClipSaveSnippet(id.clone())).key(CMD_SHIFT, Key::S));
                a.push(Action::new("Delete Entry", Act::ClipDelete(id.clone())).key(CTRL, Key::X).danger());
                a.push(Action::new("Delete All Entries", Act::ClipDeleteAll).key(CTRL_SHIFT, Key::X).danger());
            }
            Kind::File(p) => {
                a.push(Action::new("Open", Act::Run));
                a.push(Action::new("Show in Finder", Act::Reveal(p.clone())).key(CMD, Key::Enter));
                a.push(Action::new("Quick Look", Act::QuickLook(p.clone())).key(CMD, Key::Y));
                a.push(Action::new("Convert…", Act::Convert(vec![p.clone()])).key(CMD, Key::T));
                a.push(Action::new("Format Wheel", Act::OpenWheel(vec![p.clone()])).key(Modifiers::ALT, Key::Enter));
                if ferrocast_core::convert::family_of(p) == ferrocast_core::convert::Family::Video {
                    a.push(Action::new("Trim Video…", Act::TrimVideo(p.clone())).key(CMD_SHIFT, Key::V));
                    a.push(Action::new("Crop Video…", Act::CropVideo(p.clone())).key(CMD_SHIFT, Key::J));
                    a.push(Action::new("Change Speed…", Act::SpeedVideo(p.clone())).key(CMD_SHIFT, Key::L));
                    a.push(Action::new("Video Snapshots…", Act::SnapshotVideo(p.clone())).key(CMD_SHIFT, Key::S));
                    a.push(Action::new("Redact Video…", Act::RedactVideo(p.clone())));
                }
                if ferrocast_core::convert::family_of(p) == ferrocast_core::convert::Family::Image {
                    a.push(Action::new("Annotate…", Act::AnnotateCanvas(p.clone())).key(CMD_SHIFT, Key::A));
                    a.push(Action::new("Edit Photo…", Act::EditPhoto(p.clone())).key(CMD_SHIFT, Key::E));
                    a.push(Action::new("Crop…", Act::CropImage(p.clone())).key(CMD_SHIFT, Key::K));
                    a.push(Action::new("Redact Areas…", Act::RedactImage(p.clone())).key(CMD_SHIFT, Key::R));
                    a.push(Action::new("Add Background…", Act::FrameImage(p.clone())).key(CMD_SHIFT, Key::B));
                }
                if ferrocast_core::convert::ext(p) == "pdf" {
                    a.push(Action::new("Organize Pages…", Act::OrganizePdf(p.clone())).key(CMD_SHIFT, Key::O));
                }
                a.push(Action::new("Copy File", Act::CopyFiles(vec![p.clone()])).key(CMD_SHIFT, Key::Period));
                a.push(Action::new("Compress to ZIP", Act::ZipFiles(vec![p.clone()])));
                a.push(Action::new("Copy Path", Act::CopyText(p.to_string_lossy().to_string())).key(CMD_SHIFT, Key::C));
                a.push(Action::new("Move to…", Act::MoveFiles(vec![p.clone()])).key(CMD_SHIFT, Key::M));
                a.push(
                    Action::new(
                        if std::path::Path::new("/Applications/iTerm.app").exists() { "Open in iTerm" } else { "Open in Terminal" },
                        Act::OpenInTerminal(p.clone()),
                    )
                    .key(CMD_SHIFT, Key::T),
                );
                a.push(Action::new("AirDrop", Act::AirDrop(vec![p.clone()])));
                if matches!(self.view(), View::Shelf) {
                    a.push(Action::new("Remove from Shelf", Act::RemoveFromShelf(p.clone())).key(CMD, Key::Backspace));
                    a.push(Action::new("Clear Shelf", Act::ClearShelf).danger());
                } else {
                    a.push(Action::new("Float on Shelf", Act::AddToShelf(vec![p.clone()])).key(CMD_SHIFT, Key::F));
                }
                a.push(Action::new("Move to Trash", Act::Trash(p.clone())).key(CTRL, Key::X).danger());
            }
            Kind::Calc(r) => {
                a.push(Action::new("Copy Answer", Act::CopyText(r.raw.clone())));
                if matches!(self.view(), View::CalcHistory) {
                    if let Some(i) = item.id.strip_prefix("calc:").and_then(|n| n.parse().ok()) {
                        a.push(Action::new("Delete Entry", Act::DeleteCalcEntry(i)).key(CTRL, Key::X).danger());
                    }
                    a.push(Action::new("Clear History", Act::ClearCalcHistory).danger());
                }
                a.push(Action::new("Paste Answer to Active App", Act::PasteText(r.raw.clone())).key(CMD, Key::Enter));
                a.push(
                    Action::new("Copy Question and Answer", Act::CopyText(format!("{} = {}", r.question, r.display)))
                        .key(Modifiers { alt: true, ..CMD_SHIFT }, Key::C),
                );
            }
            Kind::Notion(n) => {
                a.push(Action::new("Open", Act::Run));
                a.push(Action::new("Copy Link", Act::CopyText(n.url.clone())).key(CMD_SHIFT, Key::C));
                a.push(Action::new("Set as Capture Target", Act::NotionTarget(n.clone())));
                a.push(Action::new("Add Text Here…", Act::NotionAddHere(n.clone())));
            }
            Kind::NotionLoadMore => a.push(Action::new("Load More", Act::Run)),
            Kind::NotionConnect | Kind::ObsidianSetup => a.push(Action::new("Open Settings → Integrations", Act::Run)),
            Kind::ObsidianNote(rel) => {
                let entry = self.obsidian_entry(rel);
                let title = entry.as_ref().map(|e| e.title.as_str()).unwrap_or_default();
                let root = self.obsidian_vault().map(|v| v.path).unwrap_or_default();
                let abs = root.join(rel);
                a.push(Action::new("Open in Obsidian", Act::Run));
                a.push(Action::new("Reveal in Finder", Act::Reveal(abs.clone())).key(CMD, Key::Enter));
                a.push(Action::new("Copy Wikilink", Act::CopyText(wikilink(rel, title))).key(CMD_SHIFT, Key::C));
                a.push(
                    Action::new("Copy Path", Act::CopyText(abs.to_string_lossy().into_owned()))
                        .key(Modifiers { alt: true, ..CMD_SHIFT }, Key::C),
                );
                // ⌘Y is the preview toggle of this view
                a.push(Action::new("Quick Look", Act::QuickLook(abs)).key(CMD_SHIFT, Key::Y));
                a.push(Action::new("Search in Obsidian", Act::ObsidianSearch(self.query.trim().to_string())));
            }
            Kind::Note(id) => {
                let pinned = self.notes.iter().any(|n| &n.id == id && n.pinned_at.is_some());
                a.push(Action::new("Open Note", Act::Run));
                a.push(Action::new(if pinned { "Unpin Note" } else { "Pin Note" }, Act::NotePin(id.clone())).key(CMD, Key::Period));
                a.push(Action::new("Create Note", Act::CreateNote).key(CMD, Key::N));
                // goes to Recently Deleted, so no confirmation
                a.push(Action::new("Delete Note", Act::NoteDelete(id.clone())).key(CTRL, Key::X).danger());
            }
            Kind::Emoji(e) => {
                a.push(Action::new("Paste Emoji to Active App", Act::Run));
                a.push(Action::new("Copy Emoji to Clipboard", Act::CopyText(e.to_string())).key(CMD, Key::Enter));
            }
            Kind::ColorFormat(v) => {
                a.push(Action::new("Copy Color", Act::CopyText(v.clone())));
                a.push(Action::new("Paste Color to Active App", Act::PasteText(v.clone())).key(CMD, Key::Enter));
            }
            Kind::Script(i) => {
                a.push(Action::new("Run Script Command", Act::Run));
                if let Some(s) = self.scripts.get(*i) {
                    a.push(Action::new("Edit Script", Act::EditScript(s.path.clone())).key(CMD, Key::E));
                    a.push(Action::new("Show in Finder", Act::Reveal(s.path.clone())).key(CMD_SHIFT, Key::F));
                }
            }
            Kind::Builtin(Builtin::ClipboardHistory) | Kind::Builtin(Builtin::SearchSnippets) => {
                a.push(Action::new("Open Command", Act::Run));
            }
            Kind::System(c) => {
                let mut act = Action::new("Run Command", Act::Run);
                act.destructive = c.is_destructive();
                a.push(act);
            }
            Kind::Window(_) => a.push(Action::new("Apply to Focused Window", Act::Run)),
            Kind::Tiling(_) => a.push(Action::new("Run Command", Act::Run)),
            Kind::Url(_) => a.push(Action::new("Open in Browser", Act::Run)),
            Kind::OpenWindow(_) => a.push(Action::new("Switch to Window", Act::Run)),
            Kind::Timer(..) => a.push(Action::new("Start Timer", Act::Run)),
            Kind::Editor(_) => a.push(Action::new("Open Editor", Act::Run)),
            Kind::ConvertTo(files, t) => {
                a.push(Action::new(
                    match t.needs {
                        Some(_) => "Copy install command",
                        None => "Convert",
                    },
                    Act::Run,
                ));
                a.push(Action::new("Reveal original in Finder", Act::Reveal(files[0].clone())).key(CMD, Key::Enter));
            }
            Kind::RunningTimer(i) => a.push(Action::new("Cancel Timer", Act::CancelTimer(*i)).danger()),
            Kind::Recording(p) => {
                a.push(Action::new("Open in Editor", Act::Run));
                a.push(Action::new("Show in Finder", Act::Reveal(p.clone())).key(CMD, Key::Enter));
                a.push(Action::new("Copy Path", Act::CopyText(p.to_string_lossy().to_string())).key(CMD_SHIFT, Key::C));
                a.push(Action::new("Move to Trash", Act::Trash(p.clone())).key(CMD, Key::Backspace).danger());
            }
            Kind::Process(pr) => {
                a.push(Action::new("Kill Process", Act::Run));
                a.push(Action::new("Force Kill Process", Act::KillProcess(pr.pid, true)).key(CMD, Key::Enter).danger());
                a.push(Action::new("Copy PID", Act::CopyText(pr.pid.to_string())).key(CMD_SHIFT, Key::C));
                a.push(Action::new("Copy Path", Act::CopyText(pr.path.clone())).key(CMD, Key::P));
            }
            Kind::ExtItem(id) => a = self.ext_actions(id),
            Kind::ExtCommand { .. } => a.push(Action::new("Run Extension Command", Act::Run)),
            _ => a.push(Action::new("Open", Act::Run)),
        }
        if matches!(self.view(), View::Root) && item.is_command() {
            let fav = self.settings.is_favorite(&item.id);
            a.push(Action::new(if fav { "Remove from Favorites" } else { "Add to Favorites" }, Act::ToggleFavorite).key(CMD_SHIFT, Key::F));
            if fav {
                a.push(Action::new("Move Favorite Up", Act::MoveFavorite(-1)).key(CMD, Key::ArrowUp));
                a.push(Action::new("Move Favorite Down", Act::MoveFavorite(1)).key(CMD, Key::ArrowDown));
            }
            a.push(Action::new("Set Alias…", Act::SetAlias));
            a.push(Action::new("Set Hotkey…", Act::SetHotkey).key(CMD_SHIFT, Key::Comma));
            a.push(Action::new("Reset Ranking", Act::ResetRanking));
            a.push(Action::new("Disable Command", Act::DisableCommand).key(CMD_SHIFT, Key::D).danger());
        }
        a
    }

    fn primary_label(&self, item: &Item) -> String {
        self.actions_for_cheap(item)
    }

    fn actions_for_cheap(&self, item: &Item) -> String {
        if let Kind::ExtItem(id) = &item.kind {
            return self.ext_primary(id);
        }
        match &item.kind {
            Kind::App { .. } => "Open Application",
            Kind::Quicklink { .. } => "Open Quicklink",
            Kind::Snippet(_) => "Paste Snippet",
            Kind::Clip(_) => "Paste to Active App",
            Kind::File(_) => "Open",
            Kind::Calc(_) => "Copy Answer",
            Kind::Color(_) => "Copy Color as…",
            Kind::ColorFormat(_) => "Copy Color",
            Kind::Emoji(_) => "Paste Emoji",
            Kind::Note(_) => "Open Note",
            Kind::Notion(_) => "Open",
            Kind::NotionLoadMore => "Load More",
            Kind::NotionConnect | Kind::ObsidianSetup => "Open Settings",
            Kind::ObsidianNote(_) => "Open in Obsidian",
            Kind::Script(_) => "Run Script Command",
            Kind::Url(_) => "Open in Browser",
            Kind::OpenWindow(_) => "Switch to Window",
            Kind::Process(_) => "Kill Process",
            Kind::ConvertTo(..) => "Convert",
            Kind::ImageTool(..) | Kind::FileTool(..) => "Apply",
            Kind::Editor(_) => "Open Editor",
            Kind::Timer(..) => "Start Timer",
            Kind::RunningTimer(_) => "Cancel Timer",
            Kind::Recording(_) => "Open in Editor",
            Kind::WebSearch(_) => "Search",
            Kind::Window(_) => "Apply to Window",
            _ => "Run Command",
        }
        .to_string()
    }

    // ------------------------------------------------------------ execution

    /// End the translating effect, exactly once (a no-op when none runs). `success_web`: the page
    /// text was replaced by the marked script, which already removed the effect.
    fn end_effect(&mut self, success_web: bool) {
        if let Some(e) = self.translating_effect.take() {
            let why = match (&e.kind, success_web) {
                (EffectKind::Web(_), true) => "replaced",
                (EffectKind::Web(_), false) => "unmarked",
                _ => "hidden",
            };
            self.finish_effect(e.kind, why);
        }
    }

    /// Undo `kind` and log the one line of its life. `why`: replaced | unmarked | hidden | timeout.
    fn finish_effect(&mut self, kind: EffectKind, why: &str) {
        match &kind {
            EffectKind::Overlay(_) => (self.fx.glow_hide)(),
            EffectKind::Web(b) if why != "replaced" => (self.fx.web_script)(*b, &ferrocast_core::translate::web_unmark_js()),
            _ => {}
        }
        instance::log(&self.store.root, &format!("translate effect {} -> ended: {why}", effect_label(&kind)));
    }

    /// Safety net: end an effect whose translation never arrived.
    pub(crate) fn effect_tick(&mut self, now: Instant) {
        if self.translating_effect.is_some() && now.saturating_duration_since(self.translate_started) > EFFECT_TIMEOUT {
            if let Some(e) = self.translating_effect.take() {
                self.finish_effect(e.kind, "timeout");
            }
        }
    }

    /// The Show delivery: the translation on the clipboard and in the launcher.
    fn show_translation(&mut self, lang: String, text: String) {
        // on the clipboard too, so the translation is never lost
        clipboard::write_text(&text);
        self.shared.mark_clipboard_write();
        self.show();
        self.reset_to_root();
        self.push(View::Translation { lang, text });
    }

    fn paste_text(&mut self, text: &str) {
        clipboard::write_text(text);
        self.shared.mark_clipboard_write();
        self.paste_after_hide();
    }

    fn paste_after_hide(&mut self) {
        self.hide();
        self.reset_to_root();
        let pid = self.prev_pid;
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(160));
            if let Some(p) = pid {
                running::activate(p);
            }
            std::thread::sleep(Duration::from_millis(90));
            keyboard::paste();
        });
    }

    fn copy_text(&mut self, text: &str) {
        notion_io::write_clipboard(text);
        self.shared.mark_clipboard_write();
        self.toast("Copied to Clipboard");
        self.hide();
        self.reset_to_root();
    }

    fn open_target(&mut self, target: &str, with: Option<&str>) {
        if let Err(e) = apps::open(target, with) {
            self.toast(trf("Could not open: {e}", &[("e", &e.to_string())]));
        }
        self.hide();
        self.reset_to_root();
    }

    fn run_item(&mut self, item: &Item) {
        match &item.kind {
            Kind::App { path, .. } => self.open_target(&path.clone(), None),
            Kind::System(c) => {
                if c.is_destructive() {
                    self.push(View::Confirm { msg: format!("{}?", c.title()), act: Act::RunSystem(*c) });
                } else {
                    self.run_system(*c);
                }
            }
            Kind::Window(c) => {
                let (c, gap, pid) = (*c, self.settings.window_gap, self.prev_pid);
                self.hide();
                self.reset_to_root();
                match pid {
                    Some(pid) => {
                        if let Err(e) = windows::apply(c, pid, gap) {
                            services::notify("Window Management", &e.to_string());
                        }
                    }
                    None => services::notify("Window Management", "No target window"),
                }
            }
            Kind::Tiling(a) => {
                let (a, pid) = (*a, self.prev_pid);
                self.hide();
                self.reset_to_root();
                self.run_tiling(a, pid);
            }
            Kind::Builtin(b) => self.run_builtin(*b),
            Kind::Notion(n) => self.notion_open_url(&n.url.clone()),
            Kind::NotionLoadMore => self.notion_load_more(),
            Kind::NotionConnect | Kind::ObsidianSetup => self.notion_open_settings_tab(),
            Kind::ObsidianNote(rel) => self.obsidian_open_note(&rel.clone()),
            Kind::Prefilled(b, q) => {
                self.run_builtin(*b);
                if let View::Ai { prompt, .. } = self.stack.last_mut().unwrap() {
                    *prompt = q.clone();
                } else {
                    self.query = q.clone();
                }
            }
            Kind::Quicklink { id, arg } => {
                let Some(ql) = self.quicklinks.iter().find(|q| &q.id == id).cloned() else { return };
                let defs = placeholders::arguments(&ql.link);
                match (defs.len(), arg) {
                    (0, _) => self.open_quicklink(&ql, HashMap::new()),
                    (1, Some(a)) if !a.is_empty() => {
                        let mut m = HashMap::new();
                        m.insert(defs[0].name.clone(), a.clone());
                        self.open_quicklink(&ql, m);
                    }
                    _ => self.push_args(ArgTarget::Quicklink(ql.id.clone()), ql.name.clone(), defs, arg.clone()),
                }
            }
            Kind::Snippet(id) => {
                let Some(s) = self.snippets.iter().find(|s| &s.id == id).cloned() else { return };
                let defs = placeholders::arguments(&s.text);
                if defs.is_empty() {
                    self.paste_snippet(&s, HashMap::new());
                } else {
                    self.push_args(ArgTarget::Snippet(s.id.clone()), s.name.clone(), defs, None);
                }
            }
            Kind::Script(i) => {
                let Some(s) = self.scripts.get(*i).cloned() else { return };
                if s.arguments.is_empty() {
                    self.run_script_cmd(*i, vec![]);
                } else {
                    let defs = s
                        .arguments
                        .iter()
                        .map(|a| ArgDef {
                            name: if a.placeholder.is_empty() { "Value".into() } else { a.placeholder.clone() },
                            options: a.options.clone(),
                            optional: a.optional,
                            password: a.kind == "password",
                        })
                        .collect();
                    self.push(View::Args {
                        target: ArgTarget::Script(*i),
                        title: s.title.clone(),
                        defs,
                        values: vec![String::new(); s.arguments.len()],
                    });
                    if let View::Args { values, defs, .. } = self.stack.last_mut().unwrap() {
                        for (v, d) in values.iter_mut().zip(defs.iter()) {
                            if let Some((_, val)) = d.options.first() {
                                *v = val.clone();
                            }
                        }
                    }
                }
            }
            Kind::Calc(r) => {
                let r = r.clone();
                self.remember_calc(&r);
                self.copy_text(&r.raw)
            }
            Kind::Color(c) => {
                self.push(View::Colors(*c));
            }
            Kind::ColorFormat(v) => {
                let v = v.clone();
                self.copy_text(&v)
            }
            Kind::Url(u) => self.open_target(&u.clone(), None),
            Kind::File(p) => self.open_target(p.to_string_lossy().as_ref(), None),
            Kind::WebSearch(q) => {
                let url = self.settings.fallback_search_url.clone();
                let mut ctx = placeholders::Context::default();
                for d in placeholders::arguments(&url) {
                    ctx.arguments.insert(d.name, q.clone());
                }
                let u = placeholders::expand(&url, Target::Quicklink, &ctx).text;
                self.open_target(&u, None);
            }
            Kind::Clip(id) => self.perform(Act::ClipPaste(id.clone()), true),
            Kind::Emoji(e) => {
                let e = e.to_string();
                self.remember_emoji(&e);
                self.paste_text(&e)
            }
            Kind::Note(id) => {
                let id = id.clone();
                self.open_notes_window(|c, d| c.open_and_show(d, &id));
            }
            Kind::OpenWindow(w) => {
                let w = w.clone();
                self.hide();
                self.reset_to_root();
                running::activate(w.pid);
                windows::raise_window(w.pid, &w.title);
            }
            Kind::Process(pr) => {
                let pid = pr.pid;
                self.kill_process(pid, false);
            }
            Kind::Timer(secs, label) => {
                let (secs, label) = (*secs, label.clone());
                self.start_timer(secs, label);
            }
            Kind::Editor(Act::Convert(_)) => self.toast("Not available yet"),
            Kind::Editor(act) => {
                let act = act.clone();
                self.perform(act, false);
            }
            Kind::FileTool(files, tool) => {
                let (files, tool) = (files.clone(), tool.clone());
                self.toast(format!("{}…", tr(&tool.label())));
                services::file_tool_files(files, tool, self.tools.ffmpeg, self.bus.clone());
                self.hide();
                self.reset_to_root();
            }
            Kind::ImageTool(files, ferrocast_core::convert::ImageOp::Collage) if files.len() >= 2 => {
                let f = files.clone();
                self.perform(Act::CollageImages(f), false);
            }
            Kind::ImageTool(files, ferrocast_core::convert::ImageOp::Annotate) if files.len() == 1 => {
                let f = files[0].clone();
                self.perform(Act::AnnotateCanvas(f), false);
            }
            Kind::ImageTool(files, op) => {
                let (files, op) = (files.clone(), op.clone());
                self.toast(format!("{}…", op.label()));
                services::image_op_files(files, op, self.bus.clone());
                self.hide();
                self.reset_to_root();
            }
            Kind::ConvertTo(files, t) => {
                if let Some(n) = t.needs {
                    // "ffmpeg (brew install ffmpeg)" → copy the command in parentheses
                    let cmd = n.split_once('(').map(|(_, c)| c.trim_end_matches(')').to_string()).unwrap_or_else(|| n.to_string());
                    self.copy_text(&cmd);
                    self.toast(trf("Install command copied: {cmd}", &[("cmd", &cmd)]));
                    return;
                }
                let (files, t) = (files.clone(), t.clone());
                self.toast(format!(
                    "Converting {} to {}…",
                    if files.len() == 1 {
                        files[0].file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
                    } else {
                        format!("{} files", files.len())
                    },
                    t.label
                ));
                services::convert_files(files, t, self.tools, self.bus.clone());
                self.hide();
                self.reset_to_root();
            }
            Kind::RunningTimer(i) => {
                let i = *i;
                self.cancel_timer(i);
            }
            Kind::Recording(p) => {
                let p = p.clone();
                self.open_editor(p);
            }
            Kind::ExtCommand { ext, cmd } => {
                let (e, c) = (ext.clone(), cmd.clone());
                self.ext_launch(&e, &c);
            }
            Kind::ExtItem(id) => {
                if let Some(Act::ExtAction(a)) = self.ext_actions(id).into_iter().next().map(|a| a.kind) {
                    self.ext_trigger(a);
                }
            }
        }
    }

    /// Keep the last 100 distinct calculations, newest first.
    fn remember_calc(&mut self, r: &ferrocast_core::calc::CalcResult) {
        if matches!(self.view(), View::CalcHistory) {
            return;
        }
        self.calc_history.retain(|e| e.question != r.question);
        self.calc_history.insert(
            0,
            ferrocast_core::models::CalcEntry {
                question: r.question.clone(),
                display: r.display.clone(),
                raw: r.raw.clone(),
                value: r.value,
                unit: r.unit.clone(),
                at: now_secs(),
            },
        );
        self.calc_history.truncate(100);
        let v = self.calc_history.clone();
        self.persist("calc-history.json", &v);
    }

    /// Up to 16 recently used emoji, newest first.
    fn remember_emoji(&mut self, e: &str) {
        self.recent_emoji.retain(|x| x != e);
        self.recent_emoji.insert(0, e.to_string());
        self.recent_emoji.truncate(16);
        let v = self.recent_emoji.clone();
        self.persist("recent-emoji.json", &v);
    }

    /// Manual "Check for Updates": Homebrew installs ask brew, every other install asks the GitHub feed.
    pub(crate) fn check_updates(&mut self) {
        let exe = instance::running_exe().unwrap_or_default();
        self.check_updates_via(exe, services::update_check);
    }

    /// `check_updates` with the feed check injected so tests never touch the network.
    pub(crate) fn check_updates_via(&mut self, exe: PathBuf, feed: impl FnOnce(Bus, bool, PathBuf)) {
        if ferrocast_core::update::channel_of(&exe) != ferrocast_core::update::Channel::Brew {
            self.toast("Checking for updates…");
            feed(self.bus.clone(), true, exe);
            return;
        }
        self.toast("Checking Homebrew…");
        let bus = self.bus.clone();
        std::thread::spawn(move || {
            let Some(brew) = services::brew_path() else {
                bus.send(Msg::Toast("Homebrew not found; update from the repository".into()));
                return;
            };
            let installed = std::process::Command::new(brew)
                .env("HOMEBREW_NO_AUTO_UPDATE", "1")
                .args(["list", "--versions", "ferrocast"])
                .output()
                .map(|o| o.status.success() && !o.stdout.is_empty())
                .unwrap_or(false);
            if !installed {
                bus.send(Msg::Toast("Not installed with Homebrew; update from the repository".into()));
                return;
            }
            let msg = match std::process::Command::new(brew)
                .env("HOMEBREW_NO_AUTO_UPDATE", "1")
                .args(["outdated", "--verbose", "ferrocast"])
                .output()
            {
                Ok(o) if o.status.success() => {
                    let out = String::from_utf8_lossy(&o.stdout);
                    match out.lines().find(|l| l.starts_with("ferrocast")) {
                        Some(l) => format!("Update available: {} · run brew upgrade ferrocast", l.trim_start_matches("ferrocast").trim()),
                        None => trf("Ferrocast {version} is up to date", &[("version", env!("CARGO_PKG_VERSION"))]),
                    }
                }
                Ok(o) => format!("brew: {}", String::from_utf8_lossy(&o.stderr).lines().next().unwrap_or("unknown error")),
                Err(e) => format!("Could not run brew: {e}"),
            };
            bus.send(Msg::Toast(msg));
        });
    }

    /// Decide what a finished feed check means. `channel` and `writable` are injected so tests need
    /// neither the real executable path nor the filesystem.
    pub(crate) fn handle_update_checked(
        &mut self,
        manual: bool,
        result: Result<Option<ferrocast_core::update::Release>, String>,
        channel: ferrocast_core::update::Channel,
        writable: bool,
    ) {
        use ferrocast_core::update::{self, Decision};
        let latest = match result {
            Ok(l) => l,
            Err(e) => {
                if manual {
                    self.toast(e);
                } else {
                    instance::log(&self.store.root, &format!("update check failed: {e}"));
                }
                return;
            }
        };
        self.settings.update_last_check = Some(now_secs());
        self.save_settings();
        let current = update::Version::parse(env!("CARGO_PKG_VERSION")).expect("CARGO_PKG_VERSION is semver");
        let decision = update::decide(&current, latest, self.settings.update_skipped.as_deref());
        self.update_available = match &decision {
            Decision::UpToDate => None,
            Decision::Skipped(r) | Decision::Available(r) => Some(r.clone()),
        };
        let release = match decision {
            Decision::UpToDate => {
                if manual {
                    self.toast(trf("Ferrocast {version} is up to date", &[("version", env!("CARGO_PKG_VERSION"))]));
                }
                return;
            }
            Decision::Skipped(r) if manual => r,
            Decision::Skipped(_) => return,
            Decision::Available(r) => r,
        };
        match self.settings.update_install {
            UpdateInstall::AskFirst => self.push(View::Update { release }),
            UpdateInstall::Automatic => self.start_update(release, channel, writable),
        }
    }

    /// Carry out the plan for the install channel: download and swap, `brew upgrade`, or a refusal toast.
    pub(crate) fn start_update(
        &mut self,
        release: ferrocast_core::update::Release,
        channel: ferrocast_core::update::Channel,
        writable: bool,
    ) {
        use ferrocast_core::update::Plan;
        let ver = release.tag.trim_start_matches('v').to_string();
        match ferrocast_core::update::plan_for(channel, writable) {
            Plan::Refuse(reason) => self.toast(reason),
            Plan::Download => {
                self.toast(trf("Updating to {version}…", &[("version", &ver)]));
                services::update_install(release, self.store.root.clone(), self.bus.clone());
            }
            Plan::BrewUpgrade => {
                self.toast(trf("Updating to {version} with Homebrew…", &[("version", &ver)]));
                services::brew_upgrade(self.bus.clone());
            }
        }
    }

    pub(crate) fn skip_update(&mut self, release: &ferrocast_core::update::Release) {
        self.settings.update_skipped = Some(release.tag.trim_start_matches('v').to_string());
        self.save_settings();
    }

    /// Start a countdown; a thread sleeps and then notifies, unless the timer was cancelled.
    fn start_timer(&mut self, secs: u64, label: String) {
        use std::sync::atomic::{AtomicBool, Ordering};
        let cancelled = std::sync::Arc::new(AtomicBool::new(false));
        let flag = cancelled.clone();
        let what = if label.is_empty() { format!("{} timer", ferrocast_core::timer::describe(secs)) } else { label.clone() };
        let bus = self.bus.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(secs));
            if flag.load(Ordering::Relaxed) {
                return;
            }
            services::notify("Timer done", &what);
            let _ = std::process::Command::new("/usr/bin/afplay").arg("/System/Library/Sounds/Glass.aiff").spawn();
            bus.send(Msg::Wake);
        });
        self.timers.push(RunningTimer { ends_at: Instant::now() + Duration::from_secs(secs), secs, label, cancelled });
        self.toast(trf("Timer started · {time}", &[("time", &ferrocast_core::timer::clock(secs))]));
        self.hide();
        self.reset_to_root();
    }

    fn cancel_timer(&mut self, i: usize) {
        if i < self.timers.len() {
            self.timers[i].cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
            self.timers.remove(i);
            self.toast("Timer cancelled");
        }
        self.sec_dirty = true;
    }

    /// Drop timers that already fired.
    fn sweep_timers(&mut self) {
        let now = Instant::now();
        let before = self.timers.len();
        self.timers.retain(|t| t.ends_at > now);
        if self.timers.len() != before {
            self.sec_dirty = true;
        }
    }

    /// End a process from the Kill Process list and refresh it in place.
    fn kill_process(&mut self, pid: i32, force: bool) {
        let name = self.proc_cache.iter().find(|p| p.pid == pid).map(|p| p.name.clone()).unwrap_or_else(|| pid.to_string());
        if ferrocast_macos::process::kill(pid, force) {
            self.toast(trf(if force { "Force killed {name}" } else { "Killed {name}" }, &[("name", &name)]));
        } else {
            self.toast(trf("Could not kill {name}", &[("name", &name)]));
        }
        self.proc_cache.retain(|p| p.pid != pid);
        self.sec_dirty = true;
    }

    /// Capture commands are listed only where capture works (S1, S3).
    fn capture_hidden(&self, b: Builtin) -> bool {
        crate::record_ctl::is_gated_command(&b.id()) && self.capture.availability != crate::capture_ctl::Availability::Available
    }

    /// Says `msg` where the user is looking: a toast in the launcher, else a notification.
    fn capture_say(&mut self, msg: String) {
        if self.visible {
            self.toast(msg);
        } else {
            services::notify("Ferrocast", &msg);
        }
    }

    /// True when capture may run; otherwise shows the S3 sentence once per session.
    fn capture_gate(&mut self) -> bool {
        match self.capture.gate() {
            crate::capture_ctl::Gate::Go => true,
            crate::capture_ctl::Gate::Say(m) => {
                self.capture_say(m);
                false
            }
            crate::capture_ctl::Gate::Quiet => false,
        }
    }

    /// Opens the pin panel for `file` at its captured size (S9) and closes the launcher.
    fn pin_image(&mut self, file: &std::path::Path) {
        let Some(mtm) = objc2::MainThreadMarker::new() else { return };
        let scale = objc2_app_kit::NSScreen::mainScreen(mtm).map(|s| s.backingScaleFactor());
        let Some((size, scale)) = services::image_size(file).and_then(|s| crate::capture_ctl::pin_args(s, scale)) else {
            self.toast("Could not read the image to pin");
            return;
        };
        match ferrocast_macos::pin::open(mtm, file, size, scale) {
            Ok(_) => {
                self.hide();
                self.reset_to_root();
            }
            Err(e) => self.toast(trf("Pin failed: {e}", &[("e", &e.to_string())])),
        }
    }

    /// Claims the single selection slot for `flow`; says so and returns false when one is pending.
    fn claim_selection(&mut self, flow: crate::capture_ctl::Flow) -> bool {
        if self.selection.begin(flow).is_ok() {
            return true;
        }
        self.capture_say(crate::capture_ctl::SELECTION_BUSY.into());
        false
    }

    /// First step of every select-then-act flow: the helper lists displays and windows on a WORKER
    /// thread (up to 10 s), then `Msg::CaptureListed` brings the result back to the main loop.
    fn list_for(&mut self, flow: crate::capture_ctl::Flow) {
        let exe = self.capture.exe.clone();
        let root = self.store.root.clone();
        let bus = self.bus.clone();
        std::thread::spawn(move || {
            let listed = crate::capture_ctl::list_shareable(&exe, move |m| instance::log(&root, m));
            bus.send(Msg::CaptureListed(flow, listed));
        });
    }

    fn run_capture(&mut self, kind: crate::capture_ctl::CaptureKind) {
        let flow = crate::capture_ctl::Flow::Capture(kind);
        if !self.capture_gate() || !self.claim_selection(flow) {
            return;
        }
        self.hide();
        self.list_for(flow);
    }

    /// Capture Scrolling (S10): pick an area, then scroll it and stitch the strips off-thread.
    fn run_capture_scrolling(&mut self) {
        use crate::capture_ctl::{scroll_error_message, CaptureKind, Flow};
        if !self.capture_gate() {
            return;
        }
        // posting scroll events needs Accessibility: ask now, before the user draws anything
        if let Err(e) = ferrocast_macos::scroll::ensure_trusted(true) {
            self.capture_say(scroll_error_message(e));
            return;
        }
        let flow = Flow::Capture(CaptureKind::Scrolling);
        if !self.claim_selection(flow) {
            return;
        }
        self.hide();
        self.list_for(flow);
    }

    /// The helper listed what exists: start the overlay (non-blocking) or, for the flows without
    /// one, go straight on. Runs in the main loop, never inside the overlay.
    fn capture_listed(
        &mut self,
        flow: crate::capture_ctl::Flow,
        listed: Result<ferrocast_core::capture::protocol::Shareable, crate::capture_ctl::Outcome>,
    ) {
        use crate::capture_ctl::{select_mode, selector_windows};
        if !self.selection.listed(flow) {
            return; // cancelled meanwhile
        }
        let shareable = match listed {
            Ok(s) => s,
            Err(o) => {
                self.selection.cancel();
                return self.flow_failed(flow, o);
            }
        };
        let Some(mode) = select_mode(flow.capture_kind()) else {
            return self.capture_selected(flow, None, shareable);
        };
        let Some(mtm) = objc2::MainThreadMarker::new() else {
            self.selection.cancel();
            return;
        };
        let windows = selector_windows(&shareable.windows);
        let bus = self.bus.clone();
        let mut shareable = Some(Box::new(shareable));
        let done = Box::new(move |sel| {
            if let Some(s) = shareable.take() {
                bus.send(Msg::CaptureSelected(flow, Some(sel), s));
            }
        });
        if let Err(e) = ferrocast_macos::selector::start(mtm, windows, mode, done) {
            self.selection.cancel();
            instance::log(&self.store.root, &format!("selection overlay did not start: {e}"));
        }
    }

    /// A flow that failed before or instead of a selection: say what the flow's kind says.
    fn flow_failed(&mut self, flow: crate::capture_ctl::Flow, o: crate::capture_ctl::Outcome) {
        use crate::capture_ctl::{Flow, Outcome};
        match (flow, o) {
            (_, Outcome::Cancelled | Outcome::Saved(_)) => {}
            (Flow::Capture(_), o) => self.finish_capture(o),
            (Flow::Record(_), Outcome::PermissionDenied) => {
                self.capture_say(crate::capture_ctl::PERMISSION_TOAST.into());
                crate::capture_ctl::open_screen_recording_pane();
            }
            (Flow::Record(_), Outcome::Failed(e)) => self.capture_say(trf("Recording failed: {e}", &[("e", &e.to_string())])),
        }
    }

    /// The user chose (or `None`: the flow has no overlay): carry on with the flow. Esc plans
    /// nothing, so no file and no toast.
    fn capture_selected(
        &mut self,
        flow: crate::capture_ctl::Flow,
        selection: Option<ferrocast_macos::selector::Selection>,
        shareable: ferrocast_core::capture::protocol::Shareable,
    ) {
        use crate::capture_ctl::{capture_plan, scroll_area, scroll_capture, shoot, CaptureKind, Flow, Plan};
        if !self.selection.finish(flow) {
            return;
        }
        let pointer = match objc2::MainThreadMarker::new() {
            Some(mtm) => crate::capture_ctl::pointer_top_left(mtm),
            None => ferrocast_core::capture::region::Point { x: 0.0, y: 0.0 },
        };
        let exe = self.capture.exe.clone();
        let root = self.store.root.clone();
        let log = move |m: &str| instance::log(&root, m);
        let dir = self.settings.capture.screenshots_dir(&dirs::home_dir().unwrap_or_default());
        let bus = self.bus.clone();
        match flow {
            Flow::Capture(CaptureKind::Scrolling) => {
                let Some(area) = scroll_area(selection, &shareable) else { return };
                std::thread::spawn(move || {
                    let (outcome, note) = scroll_capture(&exe, &area, &dir, log);
                    bus.send(Msg::CaptureScroll(outcome, note));
                });
            }
            Flow::Capture(kind) => {
                let Plan::Shoot(target) = capture_plan(kind, selection, pointer, &shareable) else { return };
                let cursor = self.settings.capture.include_cursor;
                std::thread::spawn(move || bus.send(Msg::Capture(shoot(&exe, target, &dir, cursor, log))));
            }
            Flow::Record(kind) => {
                let Plan::Shoot(target) = capture_plan(kind.capture_kind(), selection, pointer, &shareable) else { return };
                let display = crate::record_ctl::display_frame_for(&target, &shareable);
                self.rt.gen += 1;
                self.rt.pending = Some(crate::record_ctl::Pending { target, display });
                let effect = self.record.begin(self.settings.capture.countdown);
                self.perform_record(effect);
            }
        }
    }

    fn recordings_dir(&self) -> PathBuf {
        self.settings.capture.recordings_dir(&dirs::home_dir().unwrap_or_default())
    }

    fn reload_recordings(&mut self) {
        self.rt.recordings = crate::record_ctl::scan(&self.recordings_dir());
        self.invalidate_sections();
    }

    /// Open Last Recording: the newest project in the studio editor (the launcher is shown for it).
    fn open_last_recording(&mut self) {
        if !self.capture_gate() {
            return;
        }
        match crate::record_ctl::newest(&self.recordings_dir()) {
            Some(p) => {
                if !self.visible {
                    self.show();
                }
                self.reset_to_root();
                self.open_editor(p);
            }
            None => self.capture_say("No recordings yet".into()),
        }
    }

    /// Open the studio editor on a project folder: a render worker loads it (applying the auto zooms
    /// in memory when it has none and `capture.auto_zoom` is on) and renders the preview frames.
    pub(crate) fn open_editor(&mut self, dir: PathBuf) {
        self.editor_flush();
        let exe = self.capture.exe.clone();
        let root = self.store.root.clone();
        let log = move |m: &str| instance::log(&root, m);
        let c = &self.settings.capture;
        let auto = c.auto_zoom.then(|| ferrocast_core::capture::zoom::ZoomOpts { scale: c.zoom_scale, ..Default::default() });
        let bus = self.bus.clone();
        let tx = crate::editor_ctl::spawn_worker(
            dir.clone(),
            auto,
            crate::editor_ctl::helper_renderer(exe, log),
            Box::new(move |m| bus.send(Msg::Editor(Box::new(m)))),
        );
        self.editor = Some(crate::editor_ctl::Editor::new(dir.clone(), Some(tx)));
        self.push(View::Editor { project: dir });
    }

    /// The editor session ends with its view: dropping it stops the worker and the helper.
    fn editor_gc(&mut self) {
        if !self.stack.iter().any(|v| matches!(v, View::Editor { .. })) {
            self.editor_flush();
        }
        if self.editor.is_some() && !self.stack.iter().any(|v| matches!(v, View::Editor { .. })) {
            self.editor = None;
        }
    }

    fn handle_editor(&mut self, m: crate::editor_ctl::EditorMsg) {
        use crate::editor_ctl::EditorMsg;
        let Some(ed) = self.editor.as_mut() else { return };
        match m {
            EditorMsg::Opened { dir, result } if dir == ed.dir => ed.opened(result.map(|o| *o)),
            EditorMsg::Frame { dir, seq, result } if dir == ed.dir => {
                if let Some((img, t)) = ed.frame_back(seq, result) {
                    match ed.tex.as_mut() {
                        Some(t) => t.set(img, egui::TextureOptions::LINEAR),
                        None => ed.tex = Some(self.ctx.load_texture("editor-preview", img, egui::TextureOptions::LINEAR)),
                    }
                    ed.shown_t = Some(t);
                }
            }
            EditorMsg::Export { dir, msg } if dir == ed.dir => {
                if let crate::editor_export::Outcome::Finished(path) = ed.export.apply(msg) {
                    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    self.toast(trf("Exported {name}", &[("name", &name)]));
                    if path.exists() {
                        let _ = ferrocast_macos::apps::reveal(&path);
                    }
                }
            }
            EditorMsg::Transcribe { dir, msg } if dir == ed.dir => {
                let now = ed.now();
                ed.transcribe_msg(msg, now);
            }
            EditorMsg::Face { dir, msg } if dir == ed.dir => {
                let now = ed.now();
                ed.face_msg(msg, now);
            }
            _ => {}
        }
    }

    /// The Transcribe button was pressed: start the worker (the helper path, log and bus belong to the app).
    pub(crate) fn editor_transcribe_do(&mut self) {
        let exe = self.capture.exe.clone();
        let root = self.store.root.clone();
        let bus = self.bus.clone();
        let Some(ed) = self.editor.as_mut() else { return };
        if !std::mem::take(&mut ed.st.tr.start) {
            return;
        }
        if !ferrocast_macos::capture::available(&exe) {
            ed.st.tr.error = Some(crate::editor_panels::TranscribeFailure::Message("Transcription is not available in this build.".into()));
            return;
        }
        // The helper never prompts (it answers `permission-speech`), so the app raises the system prompt
        // the first time. `request_access` also covers the microphone, as the Notes dictation does.
        // Never in tests. The placeholder run keeps the tab on "Starting…" until the prompt is answered.
        if !cfg!(test)
            && !ed.st.tr.asked
            && ferrocast_macos::notes_dictation::status() == ferrocast_macos::notes_dictation::Status::NotDetermined
        {
            ed.st.tr.asked = true;
            ed.st.tr.run = Some(crate::editor_panels::TranscribeRun::new());
            let dir = ed.dir.clone();
            ferrocast_macos::notes_dictation::request_access(Box::new(move |_| {
                let msg = crate::editor_panels::TranscribeMsg::PermissionAsked;
                bus.send(Msg::Editor(Box::new(crate::editor_ctl::EditorMsg::Transcribe { dir: dir.clone(), msg })));
            }));
            return;
        }
        let log = move |m: &str| instance::log(&root, m);
        let notify = Box::new(move |m| bus.send(Msg::Editor(Box::new(m))));
        if let Err(e) = ed.start_transcribe(ferrocast_macos::capture::helper_path(&exe), log, notify) {
            ed.st.tr.error = Some(crate::editor_panels::TranscribeFailure::Message(e));
        }
    }

    /// The Analyze face button was pressed: start the worker (the helper path, log and bus belong to the app).
    pub(crate) fn editor_face_do(&mut self) {
        let exe = self.capture.exe.clone();
        let root = self.store.root.clone();
        let bus = self.bus.clone();
        let Some(ed) = self.editor.as_mut() else { return };
        if !std::mem::take(&mut ed.st.face.start) {
            return;
        }
        if !ferrocast_macos::capture::available(&exe) {
            ed.st.face.error = Some("Face tracking is not available in this build.".into());
            return;
        }
        let log = move |m: &str| instance::log(&root, m);
        let notify = Box::new(move |m| bus.send(Msg::Editor(Box::new(m))));
        if let Err(e) = ed.start_face(ferrocast_macos::capture::helper_path(&exe), log, notify) {
            ed.st.face.error = Some(e);
        }
    }

    /// What the Export sheet asked for: start the export (the helper, log and bus belong to the app)
    /// or open the save dialog on a thread.
    fn editor_export_do(&mut self, action: crate::editor_export::SheetAction) {
        use crate::editor_export::{choose_destination, ExportMsg, SheetAction};
        let exe = self.capture.exe.clone();
        let root = self.store.root.clone();
        let bus = self.bus.clone();
        let Some(ed) = self.editor.as_mut() else { return };
        match action {
            SheetAction::None => {}
            SheetAction::Start => {
                if !ferrocast_macos::capture::available(&exe) {
                    ed.export.message = Some("Screen capture is not available in this build.".into());
                    return;
                }
                let dir = ed.dir.clone();
                let log = move |m: &str| instance::log(&root, m);
                let notify = Box::new(move |m| bus.send(Msg::Editor(Box::new(m))));
                if let Err(e) = ed.start_export(ferrocast_macos::capture::helper_path(&exe), log, notify) {
                    ed.export.message = Some(e);
                    let _ = dir;
                }
            }
            SheetAction::Choose => {
                let Some(default) = ed.export.dest.clone() else { return };
                let dir = ed.dir.clone();
                std::thread::spawn(move || {
                    if let Some(p) = choose_destination(&default) {
                        bus.send(Msg::Editor(Box::new(crate::editor_ctl::EditorMsg::Export { dir, msg: ExportMsg::Dest(p) })));
                    }
                });
            }
        }
    }

    /// Record Screen / Area / Window: list what exists off the main thread, pick the target with
    /// the non-blocking overlay, then run the countdown (`capture_selected` -> `perform_record`).
    fn run_record(&mut self, kind: crate::record_ctl::RecordKind) {
        if !self.capture_gate() {
            return;
        }
        if matches!(self.record.phase(), crate::record_ctl::Phase::Countdown(_)) {
            // the command again while the numerals run cancels the start
            let effect = self.record.request_stop();
            return self.perform_record(effect);
        }
        if !self.record.is_idle() {
            self.capture_say("A recording is already running. Stop it from the menu bar.".into());
            return;
        }
        let flow = crate::capture_ctl::Flow::Record(kind);
        if !self.claim_selection(flow) {
            return;
        }
        self.hide();
        self.list_for(flow);
    }

    /// Carries out what the recording state machine asked for, then mirrors it in the HUD and tray.
    pub(crate) fn perform_record(&mut self, effect: crate::record_ctl::Effect) {
        use crate::record_ctl::{Cmd, Effect, Phase, RecordMsg};
        use ferrocast_macos::hud;
        let mtm = objc2::MainThreadMarker::new();
        match effect {
            Effect::None => {}
            Effect::ShowCountdown(n) => {
                if let Some(mtm) = mtm {
                    hud::countdown_show(mtm, self.rt.pending.as_ref().and_then(|p| p.display), n);
                }
                let (bus, gen) = (self.bus.clone(), self.rt.gen);
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_secs(1));
                    bus.send(Msg::Record(RecordMsg::CountdownTick(gen)));
                });
            }
            Effect::CancelCountdown => {
                if let Some(mtm) = mtm {
                    hud::countdown_hide(mtm);
                }
                self.rt.pending = None;
            }
            Effect::StartRecording => self.start_recording_now(),
            Effect::SendPause => self.send_record_cmd(Cmd::Pause),
            Effect::SendResume => self.send_record_cmd(Cmd::Resume),
            Effect::SendStop => self.send_record_cmd(Cmd::Stop),
            Effect::LowDisk(text) => self.capture_say(text),
            Effect::Ended { error } => {
                if let Some(mtm) = mtm {
                    hud::hud_hide(mtm);
                    hud::countdown_hide(mtm);
                }
                self.rt.cmds = None;
                if let Some(e) = error {
                    instance::log(&self.store.root, &format!("recording ended: {e}"));
                }
            }
        }
        if self.record.tray_title().is_some() {
            hud::hud_update(self.record.elapsed(), self.record.phase() == Phase::Paused);
        }
        self.sync_recording_tray();
    }

    fn send_record_cmd(&mut self, cmd: crate::record_ctl::Cmd) {
        if let Some(tx) = &self.rt.cmds {
            let _ = tx.send(cmd);
        }
    }

    /// The countdown is over: create the project folder, show the HUD, then start the helper on its
    /// own thread. The exclusion ids are read after the panels are on screen and before `record.start`.
    fn start_recording_now(&mut self) {
        use crate::record_ctl::{discard_unfinished, project_name, RecordMsg, Session};
        use ferrocast_core::capture::project::{project_dir_name, Project};
        let (Some(mtm), Some(pending)) = (objc2::MainThreadMarker::new(), self.rt.pending.take()) else {
            let effect = self.record.on_stopped();
            return self.perform_record(effect);
        };
        let dir = self.recordings_dir();
        let now = chrono::Local::now();
        let name = project_name(now.naive_local());
        let project = dir.join(project_dir_name(&name, |f| dir.join(f).exists()));
        let created = now.with_timezone(&chrono::Utc).format("%Y-%m-%dT%H:%M:%SZ").to_string();
        if let Err(e) = Project::create(&project, &name, &created) {
            discard_unfinished(&project);
            let effect = self.record.on_stopped();
            self.perform_record(effect);
            return self.capture_say(trf("Recording failed: cannot create the project folder: {e}", &[("e", &e.to_string())]));
        }
        let (pause_bus, stop_bus) = (self.bus.clone(), self.bus.clone());
        ferrocast_macos::hud::hud_show(
            mtm,
            pending.display,
            Box::new(move || pause_bus.send(Msg::Record(RecordMsg::TogglePause))),
            Box::new(move || stop_bus.send(Msg::Record(RecordMsg::Stop))),
        );
        let excludes = ferrocast_macos::hud::excluded_window_ids();
        let (tx, rx) = std::sync::mpsc::channel();
        self.rt.cmds = Some(tx);
        self.rt.project = Some(project.clone());
        let session = Session {
            exe: self.capture.exe.clone(),
            project,
            target: pending.target,
            fps: self.settings.capture.fps,
            // the editor draws its own cursor from events.json: the video stays cursor-free
            cursor: false,
            excludes,
            options: crate::record_ctl::record_options(&self.settings.capture),
            gen: self.rt.gen,
        };
        let root = self.store.root.clone();
        let bus = self.bus.clone();
        std::thread::spawn(move || crate::record_ctl::run_session(session, rx, bus, move |m| instance::log(&root, m)));
    }

    /// Everything the session thread, the HUD and the countdown timer report.
    pub(crate) fn handle_record(&mut self, m: crate::record_ctl::RecordMsg) {
        use crate::record_ctl::{discard_unfinished, saved_toast, warn_message, RecordMsg};
        use ferrocast_core::capture::protocol::RecordEvent;
        let current = |g: u64, rt: &crate::record_ctl::Runtime| g == rt.gen;
        match m {
            RecordMsg::CountdownTick(g) if current(g, &self.rt) => {
                let effect = self.record.tick_countdown();
                self.perform_record(effect);
            }
            RecordMsg::TogglePause => {
                let effect = self.record.toggle_pause();
                self.perform_record(effect);
            }
            RecordMsg::Stop => {
                let effect = self.record.request_stop();
                self.perform_record(effect);
            }
            RecordMsg::Started(g, result) if current(g, &self.rt) => {
                if let Some(mtm) = objc2::MainThreadMarker::new() {
                    // the numerals were excluded from the recording; now they can go
                    ferrocast_macos::hud::countdown_hide(mtm);
                }
                if let Err(failure) = result {
                    if let Some(p) = self.rt.project.take() {
                        discard_unfinished(&p);
                    }
                    let effect = self.record.on_stopped();
                    self.perform_record(effect);
                    if let Some(text) = failure.toast() {
                        self.capture_say(text);
                    }
                    if let Some(pane) = failure.pane() {
                        crate::capture_ctl::open_pane(pane);
                    }
                }
            }
            RecordMsg::Event(g, ev) if current(g, &self.rt) => {
                if let RecordEvent::Warn { code, message, .. } = &ev {
                    if let Some(text) = warn_message(code, message.as_deref()) {
                        self.capture_say(text);
                    }
                }
                let effect = self.record.on_event(&ev);
                self.perform_record(effect);
            }
            RecordMsg::Devices(result) => {
                self.rt.devices = match result {
                    Ok(d) => crate::record_ctl::DevicesState::Ready(d),
                    Err(e) => crate::record_ctl::DevicesState::Failed(e),
                };
            }
            RecordMsg::Finished(g, result) => {
                if current(g, &self.rt) {
                    // a helper that vanished sends no `stopped` event: end the state machine here
                    let effect = self.record.on_stopped();
                    self.perform_record(effect);
                }
                match result {
                    Ok(saved) => {
                        let name = saved.project.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                        let low = self.record.stopped_for_low_disk();
                        let text = saved_toast(&name, saved.duration, saved.interrupted, saved.message.as_deref(), low);
                        // the editor comes in P3: until then the finished project is shown in Finder
                        if !cfg!(test) {
                            let _ = apps::reveal(&saved.project);
                        }
                        self.capture_say(text);
                    }
                    Err(e) => {
                        if let Some(p) = self.rt.project.take() {
                            discard_unfinished(&p);
                        }
                        self.capture_say(trf("Recording failed: {e}", &[("e", &e.to_string())]));
                    }
                }
            }
            _ => {}
        }
    }

    /// Runs one button of the Capture window on the saved file.
    pub(crate) fn capture_result_run(&mut self, b: crate::capture_ctl::ResultButton) {
        use crate::capture_ctl::{button_enabled, ResultButton};
        let Some(View::CaptureResult { file, .. }) = self.stack.last() else { return };
        let file = file.clone();
        if !button_enabled(b, file.exists()) {
            self.toast("The capture file no longer exists");
            return;
        }
        match b {
            ResultButton::Copy => match std::fs::read(&file) {
                Ok(png) => {
                    clipboard::write_png(&png);
                    self.shared.mark_clipboard_write();
                    self.toast("Screenshot copied");
                    self.hide();
                    self.reset_to_root();
                }
                Err(e) => self.toast(trf("Copy failed: {e}", &[("e", &e.to_string())])),
            },
            // the picture is already in the Capture folder: Save shows it in Finder
            ResultButton::Save => {
                let _ = apps::reveal(&file);
                let name = file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                self.toast(trf("Saved: {name}", &[("name", &name)]));
            }
            ResultButton::Annotate => self.perform(Act::AnnotateCanvas(file), false),
            ResultButton::Background => self.perform(Act::FrameImage(file), false),
            ResultButton::CopyText => self.perform(Act::CaptureCopyText(file), false),
            ResultButton::Pin => self.perform(Act::PinImage(file), false),
        }
    }

    fn finish_capture(&mut self, outcome: crate::capture_ctl::Outcome) {
        use crate::capture_ctl::{after_action, open_screen_recording_pane, outcome_toast, AfterAction, Outcome};
        if let Some(m) = outcome_toast(&outcome) {
            self.capture_say(m);
        }
        match outcome {
            Outcome::PermissionDenied => open_screen_recording_pane(),
            Outcome::Saved(path) => match after_action(self.settings.capture.after) {
                AfterAction::Show => match services::image_size(&path) {
                    Some(size) => {
                        self.show();
                        self.push(View::CaptureResult { file: path, size });
                    }
                    None => self.capture_say("Capture failed: could not read the saved image".into()),
                },
                AfterAction::CopyImage => match std::fs::read(&path) {
                    Ok(png) => {
                        clipboard::write_png(&png);
                        self.capture_say("Screenshot copied".into());
                    }
                    Err(e) => self.capture_say(trf("Capture failed: {e}", &[("e", &e.to_string())])),
                },
                AfterAction::Nothing => {}
            },
            Outcome::Cancelled | Outcome::Failed(_) => {}
        }
    }

    fn run_builtin(&mut self, b: Builtin) {
        match b {
            Builtin::AskAi => {
                self.ai_chat = None;
                self.push(View::Ai {
                    prompt: String::new(),
                    asked: String::new(),
                    answer: String::new(),
                    history: vec![],
                    busy: false,
                    run: 0,
                })
            }
            Builtin::Welcome => self.push(View::Welcome),
            Builtin::KeyboardShortcuts => self.push(View::Shortcuts),
            Builtin::About => self.push(View::About),
            Builtin::ExportData => {
                let dest = dirs::download_dir().or_else(dirs::home_dir).unwrap_or_default();
                let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
                match services::export_backup(&self.store.root, &dest, &stamp) {
                    Ok(p) => {
                        let _ = apps::reveal(&p);
                        self.toast("Backup saved to Downloads (clipboard history excluded)");
                    }
                    Err(e) => {
                        instance::log(&self.store.root, &format!("export failed: {e}"));
                        self.toast(trf("Backup failed: {e}", &[("e", &e.to_string())]));
                    }
                }
            }
            Builtin::ImportSnippets => {
                let bus = self.bus.clone();
                self.hide();
                std::thread::spawn(move || {
                    if let Some(p) = services::choose_json_file() {
                        bus.send(Msg::ImportFile(p));
                    }
                });
            }
            Builtin::RevealLog => {
                let p = instance::log_path(&self.store.root);
                if !p.exists() {
                    let _ = std::fs::write(&p, "");
                }
                let _ = apps::reveal(&p);
                self.hide();
            }
            Builtin::ClipboardHistory => self.push(View::Clipboard),
            Builtin::SearchSnippets => self.push(View::Snippets),
            Builtin::CreateSnippet => self.new_snippet(String::new()),
            Builtin::SearchQuicklinks => self.push(View::Quicklinks),
            Builtin::CreateQuicklink => {
                self.push(View::QuicklinkForm { id: None, name: String::new(), link: String::new(), open_with: String::new() })
            }
            Builtin::SearchEmoji => self.push(View::Emoji),
            Builtin::PickColor => {
                // Hide first so the launcher is not what gets sampled.
                self.hide();
                self.reset_to_root();
                (self.fx.color_sampler)(&self.bus);
            }
            Builtin::SearchFiles => self.push(View::Files),
            Builtin::SwitchWindows => self.push(View::Windows),
            Builtin::KillProcess => self.push(View::Processes),
            Builtin::Timers => self.push(View::Timers),
            Builtin::CheckUpdates => self.check_updates(),
            Builtin::CalcHistory => self.push(View::CalcHistory),
            Builtin::Shelf => {
                let v = self.shelf.clone();
                ferrocast_macos::shelf::show(&v);
                self.hide();
                self.reset_to_root();
            }
            Builtin::ConvertFinderSelection => {
                let bus = self.bus.clone();
                self.hide();
                std::thread::spawn(move || {
                    bus.send(Msg::FinderSelection(ferrocast_macos::apps::finder_selection().map_err(|e| e.to_string())));
                });
            }
            Builtin::ConvertFiles => {
                let bus = self.bus.clone();
                self.hide();
                std::thread::spawn(move || {
                    let files = services::choose_files();
                    if !files.is_empty() {
                        bus.send(Msg::ConvertFiles(files));
                    }
                });
            }
            Builtin::SearchNotes => self.push(View::Notes),
            Builtin::SearchNotion => {
                self.push(View::NotionSearch);
                self.notion_view_open();
            }
            Builtin::AddToNotion => self.notion_add_open(None),
            Builtin::AddClipboardToNotion => self.notion_add_clipboard(),
            Builtin::Notes => self.open_notes_window(|c, d| c.toggle(d)),
            Builtin::CreateNote => self.open_notes_window(|c, d| c.create_note(d)),
            Builtin::CreateDictatedNote => self.open_notes_window(|c, d| c.create_dictated_note(d)),
            Builtin::NewObsidianNote => {
                if self.obsidian_compose_vault().is_some() {
                    self.push(View::ObsidianNew { title: String::new(), text: String::new() });
                }
            }
            Builtin::AppendToDailyNote => {
                if self.obsidian_compose_vault().is_some() {
                    self.push(View::ObsidianDaily { text: String::new() });
                }
            }
            Builtin::AddClipboardToDailyNote => {
                if self.obsidian_compose_vault().is_some() {
                    match notion_io::read_clipboard().filter(|t| !t.trim().is_empty()) {
                        Some(text) => self.obsidian_send(services::ObsidianJob::Daily { text }),
                        None => self.toast("The clipboard has no text."),
                    }
                }
            }
            Builtin::SearchObsidian => {
                self.push(View::ObsidianSearch);
                self.obsidian_view_open();
            }
            Builtin::CreateScript => self.push(View::CreateScript { title: String::new(), mode: 0 }),
            Builtin::OpenScriptsFolder => {
                let d = self.store.scripts_dir();
                self.open_target(&d.to_string_lossy(), None);
            }
            Builtin::ReloadScripts => {
                self.reload_scripts();
                self.toast(trn("{n} script command loaded", "{n} script commands loaded", self.scripts.len() as u64));
            }
            Builtin::Settings => self.push(View::Settings),
            Builtin::Quit => self.quit(),
            Builtin::CaptureArea => self.run_capture(crate::capture_ctl::CaptureKind::Area),
            Builtin::CaptureWindow => self.run_capture(crate::capture_ctl::CaptureKind::Window),
            Builtin::CaptureScreen => self.run_capture(crate::capture_ctl::CaptureKind::Screen),
            Builtin::CaptureScrolling => self.run_capture_scrolling(),
            Builtin::RecordScreen => self.run_record(crate::record_ctl::RecordKind::Screen),
            Builtin::RecordArea => self.run_record(crate::record_ctl::RecordKind::Area),
            Builtin::RecordWindow => self.run_record(crate::record_ctl::RecordKind::Window),
            Builtin::OpenLastRecording => self.open_last_recording(),
            Builtin::Recordings => {
                if self.capture_gate() {
                    self.reload_recordings();
                    self.push(View::Recordings);
                }
            }
        }
    }

    fn new_snippet(&mut self, text: String) {
        self.push(View::SnippetForm { id: None, name: String::new(), text, keyword: String::new() });
    }

    /// The Notes window is its own floating panel: the launcher gets out of its way first.
    fn open_notes_window(&mut self, f: impl FnOnce(&mut crate::notes_ctl::NotesCtl, &mut crate::notes_ctl::Data) -> crate::notes_ctl::Out) {
        if self.visible {
            self.hide();
        }
        self.reset_to_root();
        self.notes_do(f);
    }

    fn run_system(&mut self, c: system::SystemCommand) {
        self.hide();
        self.reset_to_root();
        let excl = self.settings.quit_all_excluded.clone();
        let bus = self.bus.clone();
        std::thread::spawn(move || match c.run(&excl) {
            Ok(msg) => {
                if matches!(
                    c,
                    system::SystemCommand::ToggleMute
                        | system::SystemCommand::VolumeUp
                        | system::SystemCommand::VolumeDown
                        | system::SystemCommand::Volume(_)
                        | system::SystemCommand::EmptyTrash
                        | system::SystemCommand::ToggleHiddenFiles
                        | system::SystemCommand::ToggleWifi
                        | system::SystemCommand::KeepAwake
                ) {
                    services::notify("Ferrocast", &msg);
                }
            }
            Err(e) => bus.send(Msg::Toast(format!("{}: {e}", c.title()))),
        });
    }

    fn push_args(&mut self, target: ArgTarget, title: String, defs: Vec<placeholders::ArgumentDef>, first: Option<String>) {
        let ad: Vec<ArgDef> = defs
            .iter()
            .map(|d| ArgDef {
                name: d.name.clone(),
                options: d.options.iter().map(|o| (o.clone(), o.clone())).collect(),
                optional: d.default.is_some(),
                password: false,
            })
            .collect();
        let mut values: Vec<String> = defs.iter().map(|d| d.default.clone().unwrap_or_default()).collect();
        for (v, d) in values.iter_mut().zip(ad.iter()) {
            if v.is_empty() {
                if let Some((_, o)) = d.options.first() {
                    *v = o.clone();
                }
            }
        }
        if let (Some(f), Some(v)) = (first, values.first_mut()) {
            *v = f;
        }
        self.push(View::Args { target, title, defs: ad, values });
    }

    fn arg_context(&self, defs: &[ArgDef], values: &[String]) -> placeholders::Context<'static> {
        let mut ctx = placeholders::Context { clipboard: services::clipboard_texts(&self.shared), ..Default::default() };
        for (i, (d, v)) in defs.iter().zip(values).enumerate() {
            ctx.arguments.insert(d.name.clone(), v.clone());
            if i == 0 {
                ctx.arguments.entry("argument".into()).or_insert(v.clone());
            }
        }
        ctx
    }

    fn open_quicklink(&mut self, q: &Quicklink, args: HashMap<String, String>) {
        let snaps = self.snippets.clone();
        let lookup = |n: &str| snaps.iter().find(|s| s.name == n).map(|s| s.text.clone());
        let ctx = placeholders::Context {
            clipboard: services::clipboard_texts(&self.shared),
            arguments: args,
            snippets: Some(&lookup),
            ..Default::default()
        };
        let is_path = q.link.starts_with('/') || q.link.starts_with('~');
        let target = placeholders::expand(&q.link, if is_path { Target::Snippet } else { Target::Quicklink }, &ctx).text;
        let with = q.open_with.clone();
        self.open_target(&target, with.as_deref());
    }

    fn paste_snippet(&mut self, s: &Snippet, args: HashMap<String, String>) {
        let snaps = self.snippets.clone();
        let lookup = |n: &str| snaps.iter().find(|s| s.name == n).map(|s| s.text.clone());
        let ctx = placeholders::Context {
            clipboard: services::clipboard_texts(&self.shared),
            arguments: args,
            snippets: Some(&lookup),
            ..Default::default()
        };
        let ex = placeholders::expand(&s.text, Target::Snippet, &ctx);
        clipboard::write_text(&ex.text);
        self.shared.mark_clipboard_write();
        let back = ex.cursor.map(|c| ex.text.chars().count().saturating_sub(c));
        self.hide();
        self.reset_to_root();
        let pid = self.prev_pid;
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(160));
            if let Some(p) = pid {
                running::activate(p);
            }
            std::thread::sleep(Duration::from_millis(90));
            keyboard::paste();
            if let Some(n) = back {
                std::thread::sleep(Duration::from_millis(120));
                keyboard::tap_key(keyboard::keycode::LEFT, n);
            }
        });
    }

    fn run_script_cmd(&mut self, idx: usize, args: Vec<String>) {
        let Some(s) = self.scripts.get(idx).cloned() else { return };
        if s.needs_confirmation {
            self.push(View::Confirm {
                msg: trf("Run script “{name}”? Scripts have your user account's permissions.", &[("name", &s.title)]),
                act: Act::RunScript { index: idx, args },
            });
            return;
        }
        self.execute_script_cmd(idx, args);
    }

    fn prune_clip_images(&self) {
        let list = self.shared.clipboard.lock().unwrap().clone();
        ferrocast_core::storage::prune_images(&self.store.images_dir(), &list);
    }

    fn execute_script_cmd(&mut self, idx: usize, args: Vec<String>) {
        let Some(s) = self.scripts.get(idx).cloned() else { return };
        let args =
            s.arguments.iter().zip(args).map(|(d, a)| if d.percent_encoded { placeholders::percent_encode(&a) } else { a }).collect();
        let run = self.next_run;
        self.next_run += 1;
        match s.mode {
            Mode::FullOutput | Mode::Inline => {
                self.runs.insert(run, RunKind::Output);
                // drop any Confirm/Args views so Esc on the output doesn't return to them
                while matches!(self.view(), View::Confirm { .. } | View::Args { .. }) {
                    self.stack.pop();
                }
                self.push(View::Output { run, title: s.title.clone(), text: String::new(), done: false });
            }
            Mode::Compact => {
                self.runs.insert(run, RunKind::Compact(s.title.clone()));
                self.hide();
                self.reset_to_root();
            }
            Mode::Silent => {
                self.runs.insert(run, RunKind::Silent(s.title.clone()));
                self.hide();
                self.reset_to_root();
            }
        }
        services::run_script(&s, args, run, self.bus.clone());
    }

    fn submit_args(&mut self) {
        let View::Args { target, defs, values, .. } = self.view().clone() else { return };
        if defs.iter().zip(&values).any(|(d, v)| !d.optional && v.trim().is_empty()) {
            self.toast("Fill in all required arguments");
            return;
        }
        match target {
            ArgTarget::Quicklink(id) => {
                if let Some(q) = self.quicklinks.iter().find(|q| q.id == id).cloned() {
                    let ctx = self.arg_context(&defs, &values);
                    self.open_quicklink(&q, ctx.arguments);
                }
            }
            ArgTarget::Snippet(id) => {
                if let Some(s) = self.snippets.iter().find(|s| s.id == id).cloned() {
                    let ctx = self.arg_context(&defs, &values);
                    self.paste_snippet(&s, ctx.arguments);
                }
            }
            ArgTarget::Script(i) => self.run_script_cmd(i, values),
        }
    }

    fn perform(&mut self, act: Act, confirmed: bool) {
        let destructive = matches!(
            act,
            Act::ClipDeleteAll
                | Act::Trash(_)
                | Act::DeleteSnippet(_)
                | Act::DeleteQuicklink(_)
                | Act::DisableCommand
                | Act::ForceQuitApp(_)
        );
        if destructive && !confirmed {
            let msg = match &act {
                Act::ClipDeleteAll => "Delete all clipboard history?",
                Act::Trash(_) => "Move this file to the Trash?",
                Act::DeleteSnippet(_) => "Delete this snippet?",
                Act::DeleteQuicklink(_) => "Delete this quicklink?",
                Act::ForceQuitApp(_) => "Force quit this application? Unsaved changes may be lost.",
                _ => "Disable this command? (re-enable in Settings)",
            };
            self.push(View::Confirm { msg: msg.into(), act });
            return;
        }
        let item = self.act_item.clone();
        self.panel = None;
        match act {
            Act::Run => {}
            Act::CopyText(t) => self.copy_text(&t),
            Act::NotionTarget(n) => self.notion_set_target(n),
            Act::ObsidianSearch(q) => self.obsidian_search_in_app(&q),
            Act::NotionAddHere(n) => {
                let kind = if n.kind == ferrocast_core::notion::ItemKind::Page { "page" } else { "data_source" };
                self.notion_add_open(Some(ferrocast_core::models::NotionTarget { id: n.id, kind: kind.into(), title: n.title }));
            }
            Act::CaptureCopyText(file) => {
                let bus = self.bus.clone();
                std::thread::spawn(move || {
                    let (text, toast) = crate::capture_ctl::recognize(&file);
                    bus.send(Msg::OcrDone(text, toast));
                });
                self.hide();
                self.reset_to_root();
            }
            Act::PinImage(file) => self.pin_image(&file),
            Act::ExtAction(id) => self.ext_trigger(id),
            Act::PasteText(t) => self.paste_text(&t),
            Act::Reveal(p) => {
                let _ = apps::reveal(&p);
                self.hide();
                self.reset_to_root();
            }
            Act::QuitApp(pid) => {
                running::quit(pid);
                self.toast("Quit requested");
            }
            Act::ForceQuitApp(pid) => {
                running::force_quit(pid);
                self.toast("Force quit");
            }
            Act::KillProcess(pid, force) => self.kill_process(pid, force),
            Act::CancelTimer(i) => self.cancel_timer(i),
            Act::AddToShelf(files) => {
                let n = files.len();
                for f in files {
                    if !self.shelf.contains(&f) {
                        self.shelf.insert(0, f);
                    }
                }
                let v = self.shelf.clone();
                self.persist("shelf.json", &v);
                // Float: the always-on-top panel pinned to the screen edge
                ferrocast_macos::shelf::show(&v);
                self.toast(trn("{n} item on the shelf", "{n} items on the shelf", n as u64));
            }
            Act::RemoveFromShelf(p) => {
                ferrocast_macos::shelf::remove(&p);
                self.shelf.retain(|f| *f != p);
                let v = self.shelf.clone();
                self.persist("shelf.json", &v);
                self.invalidate_sections();
            }
            Act::ClearShelf => {
                ferrocast_macos::shelf::clear();
                self.shelf.clear();
                self.persist("shelf.json", &Vec::<PathBuf>::new());
                self.invalidate_sections();
                self.toast("Shelf cleared");
            }
            Act::ClearCalcHistory => {
                self.calc_history.clear();
                let v = self.calc_history.clone();
                self.persist("calc-history.json", &v);
                self.toast("History cleared");
            }
            Act::DeleteCalcEntry(i) => {
                if i < self.calc_history.len() {
                    self.calc_history.remove(i);
                    let v = self.calc_history.clone();
                    self.persist("calc-history.json", &v);
                }
            }
            Act::CopyFiles(files) => {
                ferrocast_macos::clipboard::write_files(&files);
                self.shared.mark_clipboard_write();
                self.toast(if files.len() == 1 {
                    tr("File copied, paste it in Finder").into_owned()
                } else {
                    trf("{n} files copied", &[("n", &files.len().to_string())])
                });
            }
            Act::ZipFiles(files) => {
                // each item gets its own ZIP next to it; a selection that vanished starts nothing
                if files.is_empty() || files.iter().any(|p| !p.exists()) {
                    self.toast("Can't zip: a selected file no longer exists");
                } else {
                    self.toast(if files.len() == 1 {
                        "Compressing to ZIP…".to_string()
                    } else {
                        trf("Compressing {n} items to ZIP…", &[("n", &files.len().to_string())])
                    });
                    services::convert_files(files, zip_target(), self.tools, self.bus.clone());
                }
            }
            Act::MoveFiles(files) => {
                let bus = self.bus.clone();
                self.hide();
                std::thread::spawn(move || {
                    let Some(dest) = apps::choose_folder("Move to") else { return };
                    match apps::move_into(&files, &dest) {
                        Ok(new) => {
                            let pairs = files.into_iter().zip(new.iter().cloned()).collect();
                            bus.send(Msg::Moved(pairs));
                            let _ = apps::reveal(&new[0]);
                        }
                        Err(e) => bus.send(Msg::Toast(trf("Move failed: {e}", &[("e", &e.to_string())]))),
                    }
                });
            }
            Act::OpenInTerminal(p) => match apps::open_in_terminal(&p) {
                Ok(_) => {
                    self.hide();
                    self.reset_to_root();
                }
                Err(e) => self.toast(trf("Could not open a terminal: {e}", &[("e", &e.to_string())])),
            },
            Act::AirDrop(files) => {
                // the picker attaches to the frontmost app; hide first so it is not hidden with us
                self.hide();
                if let Err(e) = apps::airdrop(&files) {
                    self.toast(trf("AirDrop: {e}", &[("e", &e.to_string())]));
                }
            }
            Act::Convert(files) => {
                self.show();
                self.push(View::Convert(files));
            }
            Act::EditMetadata(file) => {
                let Some(kind) = meta_kind(&file) else {
                    self.toast("Edit metadata: this file type has no editable tags");
                    return;
                };
                let (bus, f) = (self.bus.clone(), file.clone());
                match kind {
                    MetaKind::Media => {
                        let Some(ff) = self.tools.ffmpeg else {
                            self.toast("ffmpeg is not installed (brew install ffmpeg)");
                            return;
                        };
                        std::thread::spawn(move || bus.send(Msg::MetadataLoaded(f.clone(), crate::media_tools::inspect_metadata(ff, &f))));
                    }
                    MetaKind::Pdf => {
                        std::thread::spawn(move || {
                            let r = ferrocast_macos::pdf::metadata(&f).map(|p| ferrocast_core::metaform::pdf_fields(&p));
                            bus.send(Msg::MetadataLoaded(f.clone(), r));
                        });
                    }
                }
                self.show();
                self.push(View::EditMetadata { file, kind, original: None, working: vec![], remove_all: false, error: None });
            }
            Act::SpeedVideo(file) => match self.tools.ffmpeg {
                None => self.toast("ffmpeg is not installed (brew install ffmpeg)"),
                Some(ff) => {
                    let (bus, f) = (self.bus.clone(), file.clone());
                    std::thread::spawn(move || {
                        let d = crate::media_tools::video_info(ff, &f).map(|(d, _)| d);
                        bus.send(Msg::VideoDuration(f, d));
                    });
                    self.show();
                    self.push(View::VideoSpeed { file, duration: None, factor: 2.0 });
                }
            },
            Act::RedactVideo(file) => match self.tools.ffmpeg {
                None => self.toast("ffmpeg is not installed (brew install ffmpeg)"),
                Some(ff) => {
                    self.ctx.data_mut(|d| {
                        d.remove::<egui::TextureHandle>(Id::new("crop-tex"));
                        d.remove::<(ferrocast_core::croprect::Handle, ferrocast_core::croprect::Rect, Pos2)>(Id::new("crop-drag"));
                    });
                    let (bus, f) = (self.bus.clone(), file.clone());
                    std::thread::spawn(move || {
                        let ready = crate::media_tools::video_dims(ff, &f).and_then(|dims| {
                            let (duration, _) = crate::media_tools::video_info(ff, &f)?;
                            if !duration.is_finite() || duration <= 0.0 {
                                return None;
                            }
                            let at = ferrocast_core::videoplan::frame_time(duration, duration * 0.1);
                            crate::media_tools::preview_frame(ff, &f, at).map(|png| (dims, duration, png))
                        });
                        bus.send(Msg::VideoRedactReady(f, ready));
                    });
                    self.show();
                    self.ref_frame_reset();
                    self.push(View::VideoRedact { file, size: None, aspect: None, duration: 0.0, areas: vec![], sel: 0, ref_t: 0.0 });
                }
            },
            Act::CropVideo(file) => match self.tools.ffmpeg {
                None => self.toast("ffmpeg is not installed (brew install ffmpeg)"),
                Some(ff) => {
                    self.ctx.data_mut(|d| d.remove::<egui::TextureHandle>(Id::new("crop-tex")));
                    let (bus, f) = (self.bus.clone(), file.clone());
                    std::thread::spawn(move || {
                        let ready = crate::media_tools::video_dims(ff, &f).and_then(|dims| {
                            let d = crate::media_tools::video_info(ff, &f).map(|(d, _)| d).unwrap_or(0.0);
                            let at = ferrocast_core::videoplan::frame_time(d, d * 0.1);
                            crate::media_tools::preview_frame(ff, &f, at).map(|png| (dims, d, png))
                        });
                        bus.send(Msg::VideoCropReady(f, ready));
                    });
                    self.show();
                    self.ref_frame_reset();
                    self.push(View::VideoCrop {
                        file,
                        size: None,
                        rect: ferrocast_core::croprect::full(2, 2),
                        aspect: None,
                        duration: 0.0,
                        ref_t: 0.0,
                    });
                }
            },
            action @ (Act::TrimVideo(_) | Act::SnapshotVideo(_) | Act::SplitVideo(_)) => {
                let (Act::TrimVideo(file) | Act::SnapshotVideo(file) | Act::SplitVideo(file)) = action.clone() else { unreachable!() };
                match self.tools.ffmpeg {
                    None => self.toast("ffmpeg is not installed (brew install ffmpeg)"),
                    Some(ff) => {
                        self.ctx.data_mut(|d| {
                            d.remove::<egui::TextureHandle>(Id::new("vt-tex"));
                            d.remove::<f64>(Id::new("vt-shown"));
                            d.remove::<f64>(Id::new("vt-asked"));
                            d.insert_temp(Id::new("vt-busy"), false);
                        });
                        let (bus, f) = (self.bus.clone(), file.clone());
                        std::thread::spawn(move || bus.send(Msg::VideoInfo(f.clone(), crate::media_tools::video_info(ff, &f))));
                        self.show();
                        self.push(match action {
                            Act::SnapshotVideo(_) => View::VideoSnapshot { file, trim: None },
                            Act::SplitVideo(_) => View::VideoSplit { file, trim: None, points: vec![], parts: 2 },
                            _ => View::VideoTrim { file, trim: None },
                        });
                    }
                }
            }
            Act::TrimAudio(file) => match self.tools.ffmpeg {
                None => self.toast("ffmpeg is not installed (brew install ffmpeg)"),
                Some(ff) => {
                    let (bus, f) = (self.bus.clone(), file.clone());
                    std::thread::spawn(move || {
                        let res = crate::media_tools::audio_samples(ff, &f, crate::media_tools::AUDIO_CAP);
                        bus.send(Msg::AudioLoaded(f, res));
                    });
                    self.show();
                    self.push(View::AudioTrim { file, trim: None, audio: None });
                }
            },
            Act::BleepAudio(file) => match self.tools.ffmpeg {
                None => self.toast("ffmpeg is not installed (brew install ffmpeg)"),
                Some(ff) => {
                    let (bus, f) = (self.bus.clone(), file.clone());
                    std::thread::spawn(move || {
                        let res = crate::media_tools::audio_samples(ff, &f, crate::media_tools::AUDIO_CAP);
                        bus.send(Msg::BleepAudioLoaded(f, res));
                    });
                    self.show();
                    self.push(View::BleepAudio { file, duration: 0.0, peaks: None, ranges: vec![], sel: 0 });
                }
            },
            Act::NormalizeAudio(file) => match self.tools.ffmpeg {
                None => self.toast("ffmpeg is not installed (brew install ffmpeg)"),
                Some(ff) => {
                    let (bus, f) = (self.bus.clone(), file.clone());
                    std::thread::spawn(move || bus.send(Msg::LoudnessMeasured(f.clone(), crate::media_tools::loudness(ff, &f))));
                    self.show();
                    self.push(View::NormalizeAudio { file, loudness: None, target: -16.0, range: 11.0, peak: -1.5 });
                }
            },
            Act::AudioChannels(file) => match self.tools.ffmpeg {
                None => self.toast("ffmpeg is not installed (brew install ffmpeg)"),
                Some(ff) => {
                    let (bus, f) = (self.bus.clone(), file.clone());
                    std::thread::spawn(move || bus.send(Msg::ChannelsProbed(f.clone(), crate::media_tools::audio_channel_count(ff, &f))));
                    self.show();
                    self.push(View::ChannelsAudio { file, in_channels: None, stereo: false, left_db: 0.0, right_db: 0.0 });
                }
            },
            Act::VisualizeAudio(file) => match self.tools.ffmpeg {
                None => self.toast("ffmpeg is not installed (brew install ffmpeg)"),
                Some(_) => {
                    self.show();
                    self.push(View::VisualizeAudio {
                        file,
                        orient: ferrocast_core::media::Canvas::Landscape,
                        style: crate::media_tools::VisStyle::Waveform,
                        image: None,
                        picking: false,
                    });
                }
            },
            Act::CollageImages(files) => {
                if files.len() < 2 {
                    self.toast("A collage needs at least two images");
                } else {
                    self.ctx.data_mut(|d| {
                        d.remove::<egui::TextureHandle>(Id::new("collage-tex"));
                        d.remove::<std::sync::Arc<Vec<egui::TextureHandle>>>(Id::new("collage-strip"));
                        d.remove::<std::sync::Arc<Vec<image::RgbaImage>>>(Id::new("collage-base"));
                        d.remove::<ferrocast_core::collage::CollageOpts>(Id::new("collage-last"));
                    });
                    let (bus, fs) = (self.bus.clone(), files.clone());
                    std::thread::spawn(move || {
                        let thumbs: Vec<(usize, usize, Vec<u8>)> = fs
                            .iter()
                            .filter_map(|f| {
                                let png = services::display_png(f, 300)?;
                                let img = image::load_from_memory(&png).ok()?.to_rgba8();
                                Some((img.width() as usize, img.height() as usize, img.into_raw()))
                            })
                            .collect();
                        bus.send(Msg::CollageLoaded(fs, thumbs));
                    });
                    self.show();
                    self.push(View::CollageImage { files, opts: Default::default(), color: 0 });
                }
            }
            Act::FrameImage(file) => match services::image_size(&file) {
                Some(size) => {
                    self.ctx.data_mut(|d| {
                        d.remove::<egui::TextureHandle>(Id::new("frame-tex"));
                        d.remove::<std::sync::Arc<image::RgbaImage>>(Id::new("frame-base"));
                        d.remove::<std::sync::Arc<(String, image::RgbaImage)>>(Id::new("frame-bg"));
                        d.remove::<(ferrocast_core::frame::FrameOpts, usize)>(Id::new("frame-last"));
                    });
                    let (bus, f) = (self.bus.clone(), file.clone());
                    std::thread::spawn(move || bus.send(Msg::FrameLoaded(f.clone(), services::display_png(&f, 600))));
                    self.show();
                    let pad = (size.0.max(size.1) / 12).max(8);
                    self.push(View::FrameImage {
                        file,
                        size,
                        opts: ferrocast_core::frame::FrameOpts { pad, radius: pad / 2, shadow: pad / 3, ..Default::default() },
                        color: 4,
                        picking: false,
                    });
                }
                None => self.toast("Background: could not read the image"),
            },
            Act::RedactImage(file) => match services::image_size(&file) {
                Some(size) => {
                    self.ctx.data_mut(|d| {
                        d.remove::<egui::TextureHandle>(Id::new("redact-tex"));
                        d.remove::<(ferrocast_core::croprect::Handle, ferrocast_core::croprect::Rect, Pos2, usize)>(Id::new("redact-drag"));
                    });
                    let (bus, f) = (self.bus.clone(), file.clone());
                    std::thread::spawn(move || bus.send(Msg::RedactLoaded(f.clone(), services::display_png(&f, 1000))));
                    self.show();
                    self.push(View::RedactImage {
                        file,
                        size,
                        areas: vec![],
                        sel: None,
                        blur: false,
                        radius: REDACT_BLUR_DEFAULT,
                        color: 5,
                    });
                }
                None => self.toast("Redact: could not read the image"),
            },
            Act::CompressImages(files) => {
                if files.is_empty() {
                    self.toast("Compress: no images selected");
                } else {
                    // only std::fs::metadata here; the pixel size is probed on a worker thread
                    let orig_bytes = std::fs::metadata(&files[0]).map(|m| m.len()).unwrap_or(0);
                    let (bus, f) = (self.bus.clone(), files[0].clone());
                    std::thread::spawn(move || bus.send(Msg::CompressProbed(f.clone(), services::image_size(&f))));
                    self.show();
                    self.push(View::CompressImage {
                        files,
                        orig_bytes,
                        size: None,
                        mode: 0,
                        kb: 500,
                        percent: 40,
                        preset: 1,
                        max_edge: None,
                        resize: false,
                        media: None,
                    });
                }
            }
            Act::CompressMedia(files) => {
                use ferrocast_core::convert::{family_of, Family};
                let Some(first) = files.first().cloned() else {
                    self.toast("Compress: no files selected");
                    return;
                };
                let Some(ff) = self.tools.ffmpeg else {
                    self.toast("ffmpeg is not installed (brew install ffmpeg)");
                    return;
                };
                let orig_bytes = std::fs::metadata(&first).map(|m| m.len()).unwrap_or(0);
                let (bus, f) = (self.bus.clone(), first);
                // ffprobe runs on a worker; the window says "reading" until it answers
                std::thread::spawn(move || bus.send(Msg::MediaProbed(f.clone(), crate::media_tools::media_info(ff, &f))));
                let video = files.iter().all(|f| family_of(f) == Family::Video);
                self.show();
                self.push(View::CompressImage {
                    files,
                    orig_bytes,
                    size: None,
                    mode: 0,
                    kb: 2048,
                    percent: 40,
                    preset: 1,
                    max_edge: None,
                    resize: false,
                    media: Some(MediaWin { video, loaded: false, info: None }),
                });
            }
            Act::CropImage(file) => match services::image_size(&file) {
                Some(size) => {
                    self.ctx.data_mut(|d| {
                        d.remove::<egui::TextureHandle>(Id::new("crop-tex"));
                        d.remove::<(ferrocast_core::croprect::Handle, ferrocast_core::croprect::Rect, Pos2)>(Id::new("crop-drag"));
                    });
                    let (bus, f) = (self.bus.clone(), file.clone());
                    std::thread::spawn(move || bus.send(Msg::CropLoaded(f.clone(), services::display_png(&f, 1000))));
                    self.show();
                    self.push(View::CropImage { file, size, rect: ferrocast_core::croprect::full(size.0, size.1), aspect: None });
                }
                None => self.toast("Crop: could not read the image"),
            },
            Act::EditPhoto(file) => match services::image_size(&file) {
                Some(size) => {
                    self.ctx.data_mut(|d| {
                        d.remove::<egui::TextureHandle>(Id::new("edit-tex"));
                        d.remove::<std::sync::Arc<(Vec<u8>, usize, usize)>>(Id::new("edit-base"));
                        d.remove::<ferrocast_core::photo::PhotoOpts>(Id::new("edit-last"));
                    });
                    let (bus, f) = (self.bus.clone(), file.clone());
                    std::thread::spawn(move || bus.send(Msg::EditLoaded(f.clone(), services::display_png(&f, 800))));
                    self.show();
                    self.push(View::EditPhoto { file, size, opts: Default::default() });
                }
                None => self.toast("Edit Photo: could not read the image"),
            },
            Act::AnnotateCanvas(file) => match services::image_size(&file) {
                Some(size) => {
                    self.ctx.data_mut(|d| {
                        d.remove::<egui::TextureHandle>(Id::new("annot-tex"));
                        d.remove::<std::sync::Arc<(Vec<u8>, usize, usize)>>(Id::new("annot-base"));
                        d.remove::<ferrocast_core::annotate::Doc>(Id::new("annot-last"));
                        d.remove::<Option<((f32, f32), String)>>(Id::new("annot-text"));
                        d.remove::<bool>(Id::new("annot-typing"));
                    });
                    let (bus, f) = (self.bus.clone(), file.clone());
                    std::thread::spawn(move || bus.send(Msg::AnnotateLoaded(f.clone(), services::display_png(&f, 1600))));
                    self.show();
                    self.push(View::Annotate {
                        file,
                        size,
                        doc: Default::default(),
                        tool: ferrocast_core::annotate::Tool::Arrow,
                        color: 0,
                        sel: None,
                    });
                }
                None => self.toast("Annotate: could not read the image"),
            },
            Act::ReorderFiles(purpose, files) => {
                let files = ferrocast_core::reorder::finder_order(&files);
                if files.len() < 2 {
                    self.toast("Choose order: pick at least two files");
                } else {
                    self.push(View::Reorder { purpose, files, cursor: 0 });
                }
            }
            Act::OrganizePdf(file) => match ferrocast_macos::pdf::page_count(&file) {
                Ok(n) if n > 0 => {
                    for i in 0..n {
                        self.ctx.data_mut(|d| d.remove::<egui::TextureHandle>(Id::new(("pdfthumb", i))));
                    }
                    let bus = self.bus.clone();
                    let f = file.clone();
                    std::thread::spawn(move || {
                        for i in 0..n {
                            bus.send(Msg::PdfThumb(f.clone(), i, ferrocast_macos::pdf::thumbnail(&f, i, 240).ok()));
                        }
                    });
                    self.show();
                    self.push(View::PdfOrganize { file, n, plan: ferrocast_core::pdfplan::PdfPlan::new(n) });
                }
                Ok(_) => self.toast("This PDF has no pages"),
                Err(e) => self.toast(trf("Organize PDF: {e}", &[("e", &e.to_string())])),
            },
            Act::OpenWheel(files) => {
                let kind = ferrocast_core::wheel::WheelKind::Convert;
                let segs = self.wheel_segments(kind, &files);
                if segs.is_empty() {
                    self.toast("No format options for this file");
                } else {
                    self.show();
                    // no pointer involved: the centre is off screen, arrows and Enter choose
                    self.push(View::Wheel { files, center: Pos2::new(-9999.0, -9999.0), segs, kind, sel: Some(0) });
                }
            }
            Act::QuickLook(p) => {
                // qlmanage shows the system Quick Look panel; it exits when the panel closes.
                let _ = std::process::Command::new("/usr/bin/qlmanage")
                    .arg("-p")
                    .arg(&p)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
                self.hide();
            }
            Act::HideApp(pid) => {
                running::hide(pid);
                self.toast("Hidden");
            }
            Act::ToggleFavorite => {
                if let Some(i) = item {
                    self.settings.toggle_favorite(&i.id);
                    self.save_settings();
                }
            }
            Act::MoveFavorite(d) => {
                if let Some(i) = item {
                    self.settings.move_favorite(&i.id, d);
                    self.save_settings();
                }
            }
            Act::SetAlias => {
                if let Some(i) = item {
                    let cur = self.settings.alias(&i.id).unwrap_or("").to_string();
                    self.push(View::Alias { id: i.id, name: i.title, value: cur });
                }
            }
            Act::SetHotkey => {
                if let Some(i) = item {
                    self.push(View::Hotkey { id: i.id, name: i.title, err: String::new() });
                }
            }
            Act::ResetRanking => {
                if let Some(i) = item {
                    self.frecency.reset(&i.id);
                    self.save_frecency();
                    self.toast("Ranking reset");
                }
            }
            Act::DisableCommand => {
                if let Some(i) = item {
                    self.settings.command_mut(&i.id).disabled = true;
                    self.save_settings();
                    self.toast("Command disabled — re-enable in Settings");
                }
            }
            Act::EditSnippet(id) => {
                if let Some(s) = self.snippets.iter().find(|s| s.id == id).cloned() {
                    self.push(View::SnippetForm { id: Some(s.id), name: s.name, text: s.text, keyword: s.keyword });
                }
            }
            Act::DuplicateSnippet(id) => {
                if let Some(mut s) = self.snippets.iter().find(|s| s.id == id).cloned() {
                    s.id = uuid::Uuid::new_v4().to_string();
                    s.name = format!("{} copy", s.name);
                    s.keyword.clear();
                    self.snippets.push(s);
                    self.save_snippets();
                }
            }
            Act::DeleteSnippet(id) => {
                self.snippets.retain(|s| s.id != id);
                self.save_snippets();
                self.pop_if_confirm();
            }
            Act::CreateSnippet => self.new_snippet(String::new()),
            Act::EditQuicklink(id) => {
                if let Some(q) = self.quicklinks.iter().find(|q| q.id == id).cloned() {
                    self.push(View::QuicklinkForm {
                        id: Some(q.id),
                        name: q.name,
                        link: q.link,
                        open_with: q.open_with.unwrap_or_default(),
                    });
                }
            }
            Act::DuplicateQuicklink(id) => {
                if let Some(mut q) = self.quicklinks.iter().find(|q| q.id == id).cloned() {
                    q.id = uuid::Uuid::new_v4().to_string();
                    q.name = format!("{} copy", q.name);
                    self.quicklinks.push(q);
                    self.save_quicklinks();
                }
            }
            Act::DeleteQuicklink(id) => {
                self.quicklinks.retain(|q| q.id != id);
                self.save_quicklinks();
                self.pop_if_confirm();
            }
            Act::CreateQuicklink => {
                self.push(View::QuicklinkForm { id: None, name: String::new(), link: String::new(), open_with: String::new() })
            }
            Act::ClipCopy(ref id) | Act::ClipPaste(ref id) => {
                let paste = matches!(act, Act::ClipPaste(_));
                let entry = self.shared.clipboard.lock().unwrap().iter().find(|e| &e.id == id).cloned();
                if let Some(e) = entry {
                    match e.kind {
                        ClipKind::Image => {
                            if let Ok(b) = std::fs::read(self.store.images_dir().join(&e.content)) {
                                clipboard::write_png(&b);
                            }
                        }
                        ClipKind::File => clipboard::write_files(&e.content.lines().map(PathBuf::from).collect::<Vec<_>>()),
                        _ => clipboard::write_text(&e.content),
                    }
                    self.shared.mark_clipboard_write();
                    if paste {
                        self.paste_after_hide();
                    } else {
                        self.toast("Copied to Clipboard");
                        self.hide();
                        self.reset_to_root();
                    }
                }
            }
            Act::ClipPin(id) => {
                if let Some(e) = self.shared.clipboard.lock().unwrap().iter_mut().find(|e| e.id == id) {
                    e.pinned = !e.pinned;
                }
                self.shared.save_clipboard();
            }
            Act::ClipDelete(id) => {
                self.shared.clipboard.lock().unwrap().retain(|e| e.id != id);
                self.shared.save_clipboard();
                self.prune_clip_images();
            }
            Act::ClipDeleteAll => {
                self.shared.clipboard.lock().unwrap().retain(|e| e.pinned);
                self.shared.save_clipboard();
                self.prune_clip_images();
                self.toast("Clipboard history cleared");
            }
            Act::ClipSaveSnippet(id) => {
                let t = self.shared.clipboard.lock().unwrap().iter().find(|e| e.id == id).map(|e| e.content.clone());
                if let Some(t) = t {
                    self.new_snippet(t);
                }
            }
            Act::NoteDelete(id) => self.notes_do(|c, d| c.delete_id(d, &id)),
            Act::NotePin(id) => self.notes_do(|c, d| c.toggle_pin_id(d, &id)),
            Act::CreateNote => self.open_notes_window(|c, d| c.create_note(d)),
            Act::Trash(p) => {
                match apps::trash(&p) {
                    Ok(()) => self.toast("Moved to Trash"),
                    Err(e) => self.toast(trf("Trash failed: {e}", &[("e", &e.to_string())])),
                }
                self.files.1.retain(|f| f != &p);
                if self.stack.iter().any(|v| matches!(v, View::Recordings)) {
                    self.reload_recordings();
                }
            }
            Act::EditScript(p) => {
                let _ = std::process::Command::new("/usr/bin/open").arg("-t").arg(&p).status();
                self.hide();
                self.reset_to_root();
            }
            Act::RunSystem(c) => self.run_system(c),
            Act::RunScript { index, args } => self.execute_script_cmd(index, args),
        }
    }

    fn pop_if_confirm(&mut self) {
        if matches!(self.view(), View::Confirm { .. }) {
            self.pop();
        }
    }
}

fn file_item(p: &std::path::Path) -> Item {
    Item {
        id: format!("file:{}", p.display()),
        title: p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        subtitle: p
            .parent()
            .map(|d| d.to_string_lossy().replace(&dirs::home_dir().unwrap_or_default().to_string_lossy().to_string(), "~"))
            .unwrap_or_default(),
        icon: Icon::App(p.to_string_lossy().to_string()),
        accessory: tr("File").into_owned(),
        keywords: vec![],
        kind: Kind::File(p.to_path_buf()),
    }
}

/// `SmileysAndEmotion` -> `Smileys & Emotion`.
fn humanize_group(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push(' ');
        }
        out.push(c);
    }
    out.replace(" And ", " & ")
}

fn looks_like_url(q: &str) -> Option<String> {
    let q = q.trim();
    if q.contains(char::is_whitespace) || q.len() < 4 {
        return None;
    }
    if q.starts_with("http://") || q.starts_with("https://") {
        return Some(q.to_string());
    }
    let host = q.split('/').next()?;
    let tld = host.rsplit('.').next()?;
    if host.contains('.') && tld.len() >= 2 && tld.chars().all(|c| c.is_ascii_alphabetic()) && !host.starts_with('.') {
        return Some(format!("https://{q}"));
    }
    None
}

/// How a tool is reached today.
#[derive(Clone, Debug)]
pub(crate) enum ToolReach {
    /// A dedicated window (or editor view).
    Act(Act),
    /// A one-step image preset.
    Image(ferrocast_core::convert::ImageOp),
    /// A one-step file preset.
    File(ferrocast_core::convert::FileTool),
    /// No direct entry: open the Convert view for these files.
    Open,
}

/// Pure map from a tool to its existing window or preset for these files.
pub(crate) fn tool_reach(tool: ferrocast_core::tools::ToolId, files: &[PathBuf]) -> ToolReach {
    use ferrocast_core::convert::{family_of, Family, FileTool, ImageOp};
    use ferrocast_core::tools::ToolId::*;
    let one = (files.len() == 1).then(|| files[0].clone());
    let images = !files.is_empty() && files.iter().all(|f| family_of(f) == Family::Image);
    match (tool, one) {
        (AnnotateImage, Some(p)) => ToolReach::Act(Act::AnnotateCanvas(p)),
        (EditImage, Some(p)) => ToolReach::Act(Act::EditPhoto(p)),
        (CropImage, Some(p)) => ToolReach::Act(Act::CropImage(p)),
        (RedactImage, Some(p)) => ToolReach::Act(Act::RedactImage(p)),
        (FrameImage, Some(p)) => ToolReach::Act(Act::FrameImage(p)),
        (TrimVideo, Some(p)) => ToolReach::Act(Act::TrimVideo(p)),
        (CropVideo, Some(p)) => ToolReach::Act(Act::CropVideo(p)),
        (ChangeVideoSpeed, Some(p)) => ToolReach::Act(Act::SpeedVideo(p)),
        (VideoSnapshots, Some(p)) => ToolReach::Act(Act::SnapshotVideo(p)),
        (SplitVideo, Some(p)) => ToolReach::Act(Act::SplitVideo(p)),
        (RedactVideo, Some(p)) => ToolReach::Act(Act::RedactVideo(p)),
        (OrganizePdf, Some(p)) => ToolReach::Act(Act::OrganizePdf(p)),
        (CreateCollage, _) if files.len() >= 2 => ToolReach::Act(Act::CollageImages(files.to_vec())),
        (CropImage, None) if images => ToolReach::Image(ImageOp::Crop(1, 1)),
        (Compress, _) if images => ToolReach::Act(Act::CompressImages(files.to_vec())),
        (Compress, _)
            if !files.is_empty() && [Family::Audio, Family::Video].iter().any(|fam| files.iter().all(|f| family_of(f) == *fam)) =>
        {
            ToolReach::Act(Act::CompressMedia(files.to_vec()))
        }
        (Compress, _) if !files.is_empty() && files.iter().all(|f| family_of(f) == Family::Pdf) => {
            ToolReach::File(FileTool::CompressPdf(ferrocast_core::compress::Preset::Balanced))
        }
        (RemoveMetadata, Some(p)) if meta_kind(&p).is_some() => ToolReach::Act(Act::EditMetadata(p)),
        (RemoveMetadata, _) if images => ToolReach::Image(ImageOp::StripMetadata),
        (CreatePdf, _) if images => ToolReach::Image(ImageOp::CombinePdf),
        (MuteVideo, _) => ToolReach::File(FileTool::Mute),
        (JoinVideos, _) => ToolReach::File(FileTool::Join),
        (MergePdf, _) => ToolReach::File(FileTool::Merge),
        (SplitPdf, _) => ToolReach::File(FileTool::SplitPages),
        (SplitVideo, _) => ToolReach::File(FileTool::SplitEvery(60)),
        (NormalizeAudio, Some(p)) if family_of(&p) == Family::Audio => ToolReach::Act(Act::NormalizeAudio(p)),
        (AudioChannels, Some(p)) if family_of(&p) == Family::Audio => ToolReach::Act(Act::AudioChannels(p)),
        (NormalizeAudio, _) => ToolReach::File(FileTool::Normalize(-16.0, 11.0, -1.5)),
        (AudioChannels, _) => ToolReach::File(FileTool::Channels(false, 1.0, 1.0)),
        (AudioToVideo, Some(p)) if family_of(&p) == Family::Audio => ToolReach::Act(Act::VisualizeAudio(p)),
        (AudioToVideo, _) => ToolReach::File(FileTool::Visualize(ferrocast_core::media::Canvas::Landscape)),
        (TrimAudio, Some(p)) if family_of(&p) == Family::Audio => ToolReach::Act(Act::TrimAudio(p)),
        (TrimAudio, _) => ToolReach::File(FileTool::TrimSilence),
        (RedactAudio, Some(p)) if family_of(&p) == Family::Audio => ToolReach::Act(Act::BleepAudio(p)),
        (ExtractArchive, _) if !files.is_empty() => ToolReach::File(FileTool::Extract),
        _ => ToolReach::Open,
    }
}

/// Short label per tool.
pub(crate) fn tool_label(t: ferrocast_core::tools::ToolId) -> &'static str {
    use ferrocast_core::tools::ToolId::*;
    match t {
        Compress => "Compress",
        RemoveMetadata => "Metadata",
        EditImage => "Edit Photo",
        AnnotateImage => "Annotate",
        FrameImage => "Background",
        CropImage => "Crop",
        RedactImage => "Redact",
        CreateCollage => "Collage",
        TrimVideo => "Trim",
        CropVideo => "Crop Video",
        ChangeVideoSpeed => "Speed",
        JoinVideos => "Join",
        VideoSnapshots => "Snapshots",
        SplitVideo => "Split",
        RedactVideo => "Redact Video",
        NormalizeAudio => "Normalize",
        AudioToVideo => "Visualizer",
        TrimAudio => "Trim Audio",
        AudioChannels => "Channels",
        RedactAudio => "Bleep",
        MergePdf => "Merge PDF",
        OrganizePdf => "Organize",
        MuteVideo => "Mute",
        SplitPdf => "Split PDF",
        CreatePdf => "Create PDF",
        ExtractArchive => "Extract",
    }
}

/// One SF symbol per tool.
pub(crate) fn tool_symbol(t: ferrocast_core::tools::ToolId) -> &'static str {
    use ferrocast_core::tools::ToolId::*;
    match t {
        Compress => "sf:arrow.down.right.and.arrow.up.left",
        RemoveMetadata => "sf:tag",
        EditImage => "sf:slider.horizontal.3",
        AnnotateImage => "sf:pencil.tip.crop.circle",
        FrameImage => "sf:square.dashed",
        CropImage | CropVideo => "sf:crop",
        RedactImage | RedactVideo => "sf:eye.slash.fill",
        CreateCollage => "sf:square.grid.2x2",
        TrimVideo | TrimAudio => "sf:scissors",
        ChangeVideoSpeed => "sf:speedometer",
        JoinVideos | MergePdf => "sf:rectangle.stack",
        VideoSnapshots => "sf:camera",
        SplitVideo | SplitPdf => "sf:square.split.2x1",
        NormalizeAudio | AudioChannels => "sf:speaker.wave.2",
        AudioToVideo => "sf:waveform",
        RedactAudio => "sf:speaker.slash.fill",
        OrganizePdf => "sf:doc.on.doc",
        MuteVideo => "sf:speaker.slash",
        CreatePdf => "sf:doc.richtext",
        ExtractArchive => "sf:archivebox",
    }
}

/// Wheel segment for a tool over these files.
pub(crate) fn tool_segment(t: ferrocast_core::tools::ToolId, files: &[PathBuf]) -> WheelSeg {
    tool_segment_with(t, files, ferrocast_core::tools::implemented(t) == ferrocast_core::tools::Impl::Missing)
}

/// Same, with the "missing" status given (so the "Not available yet" mark can be tested).
pub(crate) fn tool_segment_with(t: ferrocast_core::tools::ToolId, files: &[PathBuf], missing: bool) -> WheelSeg {
    let pick = match tool_reach(t, files) {
        ToolReach::Act(a) => WheelPick::Act(a),
        ToolReach::Image(op) => WheelPick::Image(op),
        ToolReach::File(f) => WheelPick::File(f),
        ToolReach::Open => WheelPick::Act(Act::Convert(files.to_vec())),
    };
    WheelSeg { label: tool_label(t).into(), symbol: tool_symbol(t), needs: missing.then_some("Not available yet"), pick }
}

/// One diagnostic line for the log when a segment is picked: the wheel, the segment and the view
/// that ended up on top. Never a path.
pub(crate) fn wheel_pick_log_line(kind: ferrocast_core::wheel::WheelKind, label: &str, view_title: &str, visible: bool) -> String {
    format!("wheel pick {kind:?} {label:?} -> view {view_title:?} launcher_visible={visible}")
}

/// A file drop that arrives within this long of a wheel gesture ending is the end of that same drag.
pub(crate) const WHEEL_DROP_GRACE: Duration = Duration::from_millis(1500);

/// One log line per finished translation: language, engine, outcome and the character COUNT.
/// Never any part of the source or translated text.
pub(crate) fn translate_log_line(lang: &str, engine: &str, result: &Result<String, String>, ms: u64) -> String {
    match result {
        Ok(t) => format!("translate {lang} via {engine} ok chars={} ms={ms}", t.chars().count()),
        Err(e) => format!("translate {lang} failed: {}", e.lines().next().unwrap_or("").trim()),
    }
}

/// The Notes appearance that goes with a theme setting.
pub(crate) fn notes_appearance(theme: Theme) -> ferrocast_macos::notes_theme::Appearance {
    use ferrocast_macos::notes_theme::Appearance;
    match theme {
        Theme::System => Appearance::System,
        Theme::Dark => Appearance::Dark,
        Theme::Light => Appearance::Light,
    }
}

/// Side effects of delivering a translation, behind function pointers so tests never spawn a
/// process or touch Accessibility.
pub(crate) struct Effects {
    /// pid of the app in front now.
    pub(crate) frontmost: fn() -> Option<i32>,
    pub(crate) ax_replace: fn(i32, &str, &str, &Bus),
    /// (browser, language, text, the script to run)
    pub(crate) web_replace: fn(ferrocast_core::translate::Browser, &str, &str, &str, &Bus),
    pub(crate) paste: fn(&mut App, &str),
    /// Probe (off the UI thread) what the effect of translation `run` is drawn with.
    pub(crate) effect_probe: fn(Option<i32>, Option<[f64; 2]>, u64, &Bus),
    /// Show the overlay (main thread only).
    pub(crate) glow_show: fn(&ferrocast_macos::glow::Target),
    /// Hide the overlay (main thread only).
    pub(crate) glow_hide: fn(),
    /// Create the Notes toggle item in the menu bar (main thread only); dropping it removes it.
    pub(crate) notes_tray: fn(&Bus) -> Option<Box<dyn std::any::Any>>,
    /// Start the window tiling worker (it drives the Accessibility API: never in tests).
    pub(crate) tiling_spawn: fn(&Bus) -> Option<ferrocast_macos::tiling::Handle>,
    /// Draw the highlight border and the quick-action bar (main thread only).
    pub(crate) tiling_overlay: fn(&ferrocast_macos::tiling::Overlay),
    /// Make the Notes windows follow the theme setting (main thread only).
    pub(crate) notes_appearance: fn(ferrocast_macos::notes_theme::Appearance),
    /// Run a script in a browser page, ignoring the result (clean-up).
    pub(crate) web_script: fn(ferrocast_core::translate::Browser, &str),
    /// Start the translation request.
    pub(crate) apple_available: fn() -> bool,
    pub(crate) translate_start: fn(&Settings, String, String, Option<i32>, ferrocast_core::models::TranslateOutput, u64, &Bus),
    /// Show the screen colour picker; the result arrives as `Msg::ColorPicked` (main thread only).
    pub(crate) color_sampler: fn(&Bus),
}

impl Effects {
    pub(crate) fn real() -> Effects {
        Effects {
            frontmost: || running::frontmost().map(|f| f.pid),
            ax_replace: |pid, lang, text, bus| services::ax_replace(pid, lang.to_string(), text.to_string(), bus.clone()),
            web_replace: |b, lang, text, js, bus| services::web_replace(b, lang.to_string(), text.to_string(), js.to_string(), bus.clone()),
            paste: |app, text| app.paste_text(text),
            effect_probe: |pid, drop, run, bus| services::effect_start(pid, drop, run, bus.clone()),
            glow_show: |t| {
                if let Some(mtm) = objc2::MainThreadMarker::new() {
                    ferrocast_macos::glow::show(mtm, t.clone(), ferrocast_macos::system::reduce_motion());
                }
            },
            glow_hide: || {
                if let Some(mtm) = objc2::MainThreadMarker::new() {
                    ferrocast_macos::glow::hide(mtm);
                }
            },
            notes_tray: |bus| services::make_notes_tray(bus.clone()).map(|t| Box::new(t) as Box<dyn std::any::Any>),
            tiling_spawn: |bus| {
                let b = bus.clone();
                let clicks = bus.clone();
                ferrocast_macos::tiling_overlay::set_on_action(Box::new(move |q| clicks.send(Msg::TilingQuick(q))));
                Some(ferrocast_macos::tiling::spawn(Box::new(move |e| b.send(Msg::Tiling(e)))))
            },
            tiling_overlay: |o| {
                if let Some(mtm) = objc2::MainThreadMarker::new() {
                    ferrocast_macos::tiling_overlay::show(mtm, o);
                }
            },
            notes_appearance: |a| ferrocast_macos::notes_theme::set_appearance(a),
            web_script: |b, js| services::web_script(b, js.to_string()),
            apple_available: ferrocast_macos::apple_translate::available,
            translate_start: |s, text, lang, pid, out, run, bus| services::translate_text(s, text, lang, pid, out, run, bus.clone()),
            color_sampler: |bus| {
                let bus = bus.clone();
                ferrocast_macos::color_sampler::sample(move |c| {
                    bus.send(Msg::ColorPicked(c.map(|[r, g, b, a]| ferrocast_core::color::Rgba::from_unit(r, g, b, a))))
                });
            },
        }
    }
}

/// Longest a translating effect may run without its translation arriving.
pub(crate) const EFFECT_TIMEOUT: Duration = Duration::from_secs(60);

/// A translating effect that is on screen or in a page.
pub(crate) struct ActiveEffect {
    pub(crate) run: u64,
    pub(crate) kind: EffectKind,
}

/// Where the pill goes when nothing better is known: the wheel center, or the pointer when the wheel
/// was parked off screen (or never placed). `None` when neither is known.
pub(crate) fn effect_drop_point(center: Pos2) -> Option<[f64; 2]> {
    if center.x > -5000.0 && center != Pos2::ZERO {
        return Some([center.x as f64, center.y as f64]);
    }
    mouse_top_left().map(|(m, _)| [m.x as f64, m.y as f64])
}

/// The effect part of its log line: never any text.
pub(crate) fn effect_label(kind: &EffectKind) -> String {
    use ferrocast_macos::glow::Target;
    match kind {
        EffectKind::None => "none".into(),
        EffectKind::Web(b) => format!("web {b:?}"),
        EffectKind::Overlay(Target::Rects(r)) => format!("overlay rects={}", r.len()),
        EffectKind::Overlay(Target::Point(_)) => "overlay point".into(),
    }
}

/// The log line for a delivery decision: language and route, never the text.
pub(crate) fn translate_deliver_line(lang: &str, d: ferrocast_core::translate::Delivery, suffix: &str) -> String {
    use ferrocast_core::translate::Delivery;
    let route = match d {
        Delivery::SetSelection => "set-selection".to_string(),
        Delivery::WebReplace(b) => format!("web-replace {b:?}"),
        Delivery::Paste => "paste".into(),
        Delivery::Show => "show".into(),
        Delivery::Copy => "copy".into(),
    };
    format!("translate deliver {lang} -> {route}{suffix}")
}

/// One diagnostic line for the log when a wheel opens or switches: the kind, how many segments and
/// the first three labels. Never a path.
pub(crate) fn wheel_log_line(event: &str, kind: ferrocast_core::wheel::WheelKind, segs: &[WheelSeg], files: usize) -> String {
    let first = segs.iter().take(3).map(|s| s.label.as_str()).collect::<Vec<_>>().join(",");
    format!("wheel {event} {kind:?} segs={} files={files} first={first}", segs.len())
}

/// SF Symbol for a conversion output, so a ring of formats is told apart at a glance.
pub(crate) fn target_symbol(ext: &str) -> &'static str {
    match ext {
        "jpg" | "png" | "webp" | "heic" | "tiff" | "avif" | "bmp" => "sf:photo",
        "gif" => "sf:livephoto",
        "pdf" => "sf:doc.richtext",
        "mp3" | "m4a" | "wav" | "flac" | "ogg" | "opus" | "aiff" | "wma" => "sf:waveform",
        "mp4" | "mov" | "mkv" | "webm" | "avi" | "wmv" => "sf:film",
        "docx" | "pages" | "epub" => "sf:doc.text",
        "md" | "txt" | "html" => "sf:text.alignleft",
        "pptx" => "sf:rectangle.on.rectangle",
        "xlsx" | "ods" | "csv" | "tsv" => "sf:tablecells",
        "srt" | "vtt" => "sf:captions.bubble",
        "zip" | "tar" | "gz" => "sf:archivebox",
        _ => "sf:doc",
    }
}

/// Wheel geometry, in points from its centre.
pub(crate) const WHEEL_INNER: f32 = ferrocast_core::wheel::Geometry::DEFAULT.r_in;
pub(crate) const WHEEL_OUTER: f32 = ferrocast_core::wheel::Geometry::DEFAULT.r_out;

/// Segment under an offset from the wheel centre.
pub(crate) fn wheel_hit(d: Vec2, n: usize) -> Option<usize> {
    ferrocast_core::wheel::segment_at(d.x, d.y, WHEEL_INNER, WHEEL_OUTER, n)
}

/// Pointer position and its screen's visible frame, in top-left screen points.
pub(crate) fn mouse_top_left() -> Option<(Pos2, Rect)> {
    use objc2_app_kit::{NSEvent, NSScreen};
    let mtm = objc2::MainThreadMarker::new()?;
    let screens = NSScreen::screens(mtm);
    let primary_h = screens.iter().next()?.frame().size.height;
    let m = NSEvent::mouseLocation();
    let scr = screens.iter().find(|s| {
        let f = s.frame();
        m.x >= f.origin.x && m.x < f.origin.x + f.size.width && m.y >= f.origin.y && m.y < f.origin.y + f.size.height
    })?;
    let v = scr.visibleFrame();
    let top = primary_h - (v.origin.y + v.size.height);
    let r = Rect::from_min_size(Pos2::new(v.origin.x as f32, top as f32), Vec2::new(v.size.width as f32, v.size.height as f32));
    Some((Pos2::new(m.x as f32, (primary_h - m.y) as f32), r))
}

/// Visible frame of the screen holding the key window, in top-left screen points.
#[cfg_attr(test, allow(dead_code))]
fn key_window_screen() -> Option<Rect> {
    use objc2_app_kit::{NSApplication, NSScreen};
    let mtm = objc2::MainThreadMarker::new()?;
    let scr = NSApplication::sharedApplication(mtm).keyWindow()?.screen()?;
    let primary_h = NSScreen::screens(mtm).iter().next()?.frame().size.height;
    let v = scr.visibleFrame();
    let top = primary_h - (v.origin.y + v.size.height);
    Some(Rect::from_min_size(Pos2::new(v.origin.x as f32, top as f32), Vec2::new(v.size.width as f32, v.size.height as f32)))
}

fn launcher_origin() -> Option<Pos2> {
    use objc2_app_kit::{NSEvent, NSScreen};
    let mtm = objc2::MainThreadMarker::new()?;
    let screens = NSScreen::screens(mtm);
    let primary_h = screens.iter().next()?.frame().size.height;
    let m = NSEvent::mouseLocation();
    let scr = screens.iter().find(|s| {
        let f = s.frame();
        m.x >= f.origin.x && m.x < f.origin.x + f.size.width && m.y >= f.origin.y && m.y < f.origin.y + f.size.height
    })?;
    let v = scr.visibleFrame();
    let x = v.origin.x + (v.size.width - ui::WIDTH as f64) / 2.0;
    let top = primary_h - (v.origin.y + v.size.height);
    let y = top + v.size.height * 0.22;
    Some(Pos2::new(x as f32, y as f32))
}

#[path = "draw.rs"]
mod draw;
#[path = "ext_glue.rs"]
mod ext_glue;
#[path = "intro.rs"]
mod intro;
#[path = "sync_ctl.rs"]
mod sync_ctl;
#[path = "theme_ctl.rs"]
mod theme_ctl;
pub use sync_ctl::{SyncCtl, SyncDone};
#[path = "tour.rs"]
mod tour;

/// Running Dock apps as (bundle id or name, display name), for the tiling exclusion list.
pub(crate) fn running_tileable_apps() -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> =
        running::running_apps().into_iter().map(|a| (a.bundle_id.unwrap_or_else(|| a.name.clone()), a.name)).collect();
    v.sort_by_key(|a| a.1.to_lowercase());
    v
}
