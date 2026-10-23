//! MP4 export planning (S16): the pure part of `render`.
//!
//! Design (T28): every number is computed here, Swift only draws and encodes.
//! * Output time -> source time goes through [`Timeline`] (the single source of the mapping).
//!   The helper receives the kept [`Segment`]s (needed again by the audio mix of T33 and as a
//!   cross-check of the frame count) and, for every output frame, a line of a params file
//!   ([`write_frame_params`]) with the SOURCE time `t` and the camera/cursor/ring/blur
//!   [`FrameParams`] that [`Composer`] evaluated. A file, not the request line: 60 fps x minutes
//!   is megabytes, a file is read line by line in flat memory, cannot deadlock the stdin pipe and
//!   leaves the `render` request a few hundred bytes. The caller deletes the file when done.
//! * Frame `i` is shown at output time `i / fps`; the frame count is `round(duration * fps)`
//!   (at least 1), so the video lasts the timeline duration within half a frame.

use super::compose::{CameraFx, Composer, FrameParams};
use super::events::Events;
use super::project::{AudioTrack, Project};
use super::timeline::{self, Timeline};
use serde::{Deserialize, Serialize};
use std::io::{self, Write};

/// Longest side of an export (4K UHD).
pub const MAX_LONG_EDGE: u32 = 3840;
/// Pixel budget: 3840 x 2160, so portrait and square outputs are bounded too.
pub const MAX_PIXELS: u64 = 3840 * 2160;
pub const MAX_FPS: u32 = 60;
pub const MIN_WIDTH: u32 = 16;
/// GIF limits (S16): width and frame rate. A larger request is clamped by [`export_plan`] and
/// rejected (`invalid`) by the helper.
pub const GIF_MAX_WIDTH: u32 = 960;
pub const GIF_MAX_FPS: u32 = 15;
/// Memory budget of one GIF, in frames x pixels. `CGImageDestinationAddImage` keeps every frame
/// until `finalize` (no streaming API). Measured on this Mac (T29, 960 x 540, real helper, peak
/// resident set): 38 frames 257 MB, 150 frames (10 s at 15 fps) 897 MB, 300 frames 1.3 GB,
/// 450 frames 2.2 GB, about 4.7 MB per frame. 300 frames of 960 x 540 (20 s at 15 fps, ~1.3 GB) is the
/// budget; smaller or lower-fps clips get proportionally more frames, taller ones fewer.
pub const GIF_MAX_PIXEL_FRAMES: u64 = 300 * 960 * 540;

/// Per-frame delays in centiseconds (the only unit GIF has) for `frames` frames at `fps`.
/// Frame `i` ends at `round((i + 1) * 100 / fps)` centiseconds and lasts the difference to the
/// previous end, so the rounding error is spread over the frames and the TOTAL is
/// `round(frames * 100 / fps)`: within half a centisecond of the exact length, never a
/// frame-delay off (rounding each delay alone would drift by up to `frames / 2` cs).
pub fn gif_delays(frames: u32, fps: u32) -> Vec<u32> {
    let end = |i: u32| (u64::from(i) * 100 + u64::from(fps) / 2) / u64::from(fps.max(1));
    (0..frames).map(|i| (end(i + 1) - end(i)) as u32).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Codec {
    H264,
    Hevc,
}

impl Codec {
    pub fn parse(s: &str) -> Option<Codec> {
        match s {
            "h264" => Some(Codec::H264),
            "hevc" => Some(Codec::Hevc),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    Standard,
    High,
}

/// Container of an export. GIF has its own limits (S16), see [`GIF_MAX_WIDTH`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Mp4,
    Gif,
}

/// What the user chose in the export sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportOptions {
    pub format: Format,
    /// Ignored for GIF.
    pub codec: Codec,
    /// Output width in pixels (even); the height follows the project's aspect.
    pub width: u32,
    pub fps: u32,
    pub quality: Quality,
}

/// One kept source span and the factor it plays at.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Segment {
    pub src_start: f64,
    pub src_end: f64,
    pub speed: f64,
}

/// Which recorded audio file an [`AudioInput`] mixes (wire names `mic` / `system`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioKind {
    Mic,
    System,
}

/// One run of an audio file placed on the output timeline (`timeline::AudioSegment` on the wire).
/// Seconds; `src_*` is file time, `out_start` the position in the video, the run lasts
/// `(src_end - src_start) / speed` (a rate change: the pitch follows the speed).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AudioRun {
    pub src_start: f64,
    pub src_end: f64,
    pub out_start: f64,
    pub speed: f64,
}

/// One audio file of the MP4 mix: the plain file name inside the project, the linear gain
/// (`Audio::linear_gain`) and its runs. Muted or zero-gain tracks are never sent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioInput {
    pub track: AudioKind,
    pub file: String,
    pub gain: f64,
    pub segments: Vec<AudioRun>,
}

/// The audio inputs of `project` for an MP4 export: mic then system, skipping tracks without a
/// source, without runs, or with a zero (muted) gain.
pub fn audio_inputs(project: &Project) -> Vec<AudioInput> {
    [(AudioKind::Mic, AudioTrack::Mic), (AudioKind::System, AudioTrack::System)]
        .into_iter()
        .filter_map(|(kind, track)| {
            let src = match track {
                AudioTrack::Mic => project.sources.mic.as_ref(),
                AudioTrack::System => project.sources.system.as_ref(),
            }?;
            let gain = project.audio.linear_gain(track);
            let segments: Vec<AudioRun> = timeline::audio_segments(project, track)
                .into_iter()
                .map(|s| AudioRun { src_start: s.src_start, src_end: s.src_end, out_start: s.out_start, speed: s.speed })
                .collect();
            (gain.is_finite() && gain > 0.0 && !segments.is_empty()).then(|| AudioInput {
                track: kind,
                file: src.file.clone(),
                gain,
                segments,
            })
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExportPlan {
    pub segments: Vec<Segment>,
    /// Audio files to mix into an MP4 (empty for a GIF or a project without audio).
    pub audio: Vec<AudioInput>,
    /// `timeline::output_duration`.
    pub duration: f64,
    pub frames: u32,
    /// Output pixels, both even.
    pub size: [u32; 2],
    pub fps: u32,
    pub format: Format,
    pub codec: Codec,
    pub quality: Quality,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlanError {
    /// The edit keeps nothing (zero output duration).
    NothingToExport,
    /// The timeline is inconsistent (first problem of `timeline::validate`).
    InvalidTimeline(String),
    BadSize(String),
    BadFps(u32),
    /// A GIF would exceed [`GIF_MAX_PIXEL_FRAMES`] (ImageIO keeps every frame in memory).
    GifTooLong {
        frames: u32,
        max_seconds: f64,
    },
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NothingToExport => write!(f, "There is nothing to export: the edit keeps no video."),
            Self::InvalidTimeline(m) => write!(f, "The edit is not valid: {m}"),
            Self::BadSize(m) => write!(f, "{m}"),
            Self::BadFps(n) => write!(f, "{n} fps is not supported: use 1 to {MAX_FPS}."),
            Self::GifTooLong { max_seconds, .. } => {
                write!(f, "This edit is too long for a GIF: the limit is {max_seconds:.0} seconds. Trim it or export an MP4.")
            }
        }
    }
}

impl std::error::Error for PlanError {}

/// Round up to the next even integer (encoders want even sides).
fn even_up(v: f64) -> u32 {
    let n = v.max(0.0) as u32;
    n + n % 2
}

/// Width / height of `spec` ("16:9", "4:3", "9:16", "1:1"); anything else follows the source.
pub fn aspect_ratio(spec: &str, source: [u32; 2]) -> f64 {
    let parts: Vec<f64> = spec.split([':', '/']).filter_map(|p| p.trim().parse().ok()).collect();
    match parts[..] {
        [w, h] if w > 0.0 && h > 0.0 && spec.split([':', '/']).count() == 2 => w / h,
        _ => f64::from(source[0]) / f64::from(source[1].max(1)),
    }
}

/// Pixel size of the recorded video: `events.screen.size`, else the display in points x scale.
pub fn source_pixels(events: &Events) -> [u32; 2] {
    let s = &events.screen;
    let [_, _, dw, dh] = s.display_frame;
    let pts = |pixels: u32, points: f64| if pixels > 0 { pixels } else { (points * s.scale).round().max(0.0) as u32 };
    [pts(s.size[0], dw), pts(s.size[1], dh)]
}

/// Validate `opts` against the limits (3840 long edge, 3840 x 2160 pixels, 60 fps) and compute
/// the plan. `source` is the pixel size of the recorded video (see [`source_pixels`]).
pub fn export_plan(project: &Project, source: [u32; 2], opts: &ExportOptions) -> Result<ExportPlan, PlanError> {
    if opts.fps == 0 || opts.fps > MAX_FPS {
        return Err(PlanError::BadFps(opts.fps));
    }
    let gif = opts.format == Format::Gif;
    let (fps, w) = if gif { (opts.fps.min(GIF_MAX_FPS), opts.width.min(GIF_MAX_WIDTH)) } else { (opts.fps, opts.width) };
    if w < MIN_WIDTH || !w.is_multiple_of(2) || w > MAX_LONG_EDGE {
        return Err(PlanError::BadSize(format!("The width {w} is not valid: use an even number from {MIN_WIDTH} to {MAX_LONG_EDGE}.")));
    }
    let h = even_up((f64::from(w) / aspect_ratio(&project.style.aspect, source)).round().max(2.0));
    if h > MAX_LONG_EDGE || u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err(PlanError::BadSize(format!("{w}×{h} is larger than 3840×2160: choose a smaller resolution.")));
    }
    let tl = Timeline::new(project);
    let duration = tl.output_duration();
    if !(duration.is_finite() && duration > 0.0) {
        return Err(PlanError::NothingToExport);
    }
    if let Some(problem) = timeline::validate(project).into_iter().next() {
        return Err(PlanError::InvalidTimeline(problem));
    }
    let segments = tl.pieces().iter().map(|p| Segment { src_start: p.src_start, src_end: p.src_end, speed: p.factor }).collect();
    let frames = (duration * f64::from(fps)).round().max(1.0) as u32;
    if gif {
        let max_frames = (GIF_MAX_PIXEL_FRAMES / (u64::from(w) * u64::from(h))) as u32;
        if frames > max_frames {
            return Err(PlanError::GifTooLong { frames, max_seconds: (f64::from(max_frames) / f64::from(fps)).floor() });
        }
    }
    let audio = if gif { vec![] } else { audio_inputs(project) };
    Ok(ExportPlan { segments, audio, duration, frames, size: [w, h], fps, format: opts.format, codec: opts.codec, quality: opts.quality })
}

/// Output width for a resolution tier: `long_edge` is the longer side of the result
/// (1920 = "1080p", 3840 = "4K"), `None` keeps the source size, both capped to 3840 and even.
pub fn width_for(long_edge: Option<u32>, aspect: f64, source: [u32; 2]) -> u32 {
    let w = match long_edge {
        None => f64::from(source[0].min(MAX_LONG_EDGE)),
        Some(le) if aspect >= 1.0 => f64::from(le.min(MAX_LONG_EDGE)),
        Some(le) => (f64::from(le.min(MAX_LONG_EDGE)) * aspect).round(),
    };
    even_up(w)
}

/// One line of the params file: the source time of an output frame plus its draw parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrameLine {
    pub t: f64,
    #[serde(flatten)]
    pub params: FrameParams,
}

/// Write `plan.frames` lines (NDJSON, in output order) for the helper.
pub fn write_frame_params(project: &Project, events: &Events, plan: &ExportPlan, out: &mut impl Write) -> io::Result<()> {
    write_frame_params_fx(project, events, plan, CameraFx::default(), out)
}

/// Pixel size of a `render.frame` canvas for the requested `width`: the height follows the
/// project's aspect, rounded like the helper does (`max(1, round(w / aspect))`).
pub fn preview_canvas(project: &Project, source: [u32; 2], width: u32) -> [u32; 2] {
    let h = (f64::from(width) / aspect_ratio(&project.style.aspect, source)).round().max(1.0);
    [width, h as u32]
}

/// [`write_frame_params`] with the P6 camera effects (look, split, face following) of the project
/// resolved once in `fx` (`CameraFx::prepare` for `plan.size`).
pub fn write_frame_params_fx(project: &Project, events: &Events, plan: &ExportPlan, fx: CameraFx, out: &mut impl Write) -> io::Result<()> {
    let tl = Timeline::new(project);
    let composer = Composer::new(project, events).with_camera_fx(fx);
    for i in 0..plan.frames {
        let t = tl.to_source(f64::from(i) / f64::from(plan.fps));
        let line = FrameLine { t, params: composer.at(t) };
        serde_json::to_writer(&mut *out, &line)?;
        out.write_all(b"\n")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::events::ScreenInfo;
    use super::super::project::{Speed, Trim, Zoom};
    use super::*;

    const SRC: [u32; 2] = [2000, 1000];

    fn project(duration: f64) -> Project {
        let mut p = Project::new("t", "2026-01-01T00:00:00Z");
        p.duration = duration;
        p
    }

    fn opts(width: u32, fps: u32) -> ExportOptions {
        ExportOptions { format: Format::Mp4, codec: Codec::H264, width, fps, quality: Quality::Standard }
    }

    #[test]
    fn plan_of_an_untouched_project() {
        let plan = export_plan(&project(10.0), SRC, &opts(1920, 30)).unwrap();
        assert_eq!(plan.frames, 300);
        assert_eq!(plan.size, [1920, 1080]);
        assert_eq!(plan.duration, 10.0);
        assert_eq!(plan.segments, vec![Segment { src_start: 0.0, src_end: 10.0, speed: 1.0 }]);
        assert_eq!((plan.fps, plan.codec, plan.quality), (30, Codec::H264, Quality::Standard));
    }

    #[test]
    fn plan_follows_trims_and_speed_like_the_timeline() {
        let mut p = project(10.0);
        p.trims = vec![Trim { start: 1.0, end: 3.0 }, Trim { start: 5.0, end: 9.0 }];
        p.speeds = vec![Speed { start: 6.0, end: 8.0, factor: 2.0 }];
        let plan = export_plan(&p, SRC, &opts(1280, 60)).unwrap();
        // 2 s + 1 s (6..7 at 1x) + 1 s (6..8 at 2x) + 1 s (8..9) = 5 s
        assert_eq!(plan.duration, Timeline::new(&p).output_duration());
        assert_eq!(plan.duration, 5.0);
        assert_eq!(plan.frames, 300);
        assert_eq!(
            plan.segments,
            vec![
                Segment { src_start: 1.0, src_end: 3.0, speed: 1.0 },
                Segment { src_start: 5.0, src_end: 6.0, speed: 1.0 },
                Segment { src_start: 6.0, src_end: 8.0, speed: 2.0 },
                Segment { src_start: 8.0, src_end: 9.0, speed: 1.0 },
            ]
        );
    }

    #[test]
    fn frame_count_is_within_one_frame_of_the_duration() {
        for (dur, fps) in [(1.01, 30), (10.0 / 3.0, 60), (0.02, 30), (73.2, 24), (0.4, 60)] {
            let plan = export_plan(&project(dur), SRC, &opts(640, fps)).unwrap();
            let video = f64::from(plan.frames) / f64::from(fps);
            assert!((video - dur).abs() <= 1.0 / f64::from(fps), "{dur} @ {fps}: {} frames", plan.frames);
            assert!(plan.frames >= 1);
        }
    }

    #[test]
    fn audio_inputs_follow_sources_gain_and_timeline() {
        use super::super::project::{Source, TrackMix};
        let mut p = project(10.0);
        assert!(export_plan(&p, SRC, &opts(640, 30)).unwrap().audio.is_empty(), "no audio sources: nothing sent");
        p.sources.mic = Some(Source { file: "mic.m4a".into(), offset: 0.5 });
        p.sources.system = Some(Source { file: "system.m4a".into(), offset: 0.0 });
        p.speeds = vec![Speed { start: 4.0, end: 6.0, factor: 2.0 }];
        let plan = export_plan(&p, SRC, &opts(640, 30)).unwrap();
        let names: Vec<_> = plan.audio.iter().map(|a| (a.track, a.file.as_str())).collect();
        assert_eq!(names, vec![(AudioKind::Mic, "mic.m4a"), (AudioKind::System, "system.m4a")]);
        assert_eq!(plan.audio[0].gain, 1.0);
        assert_eq!(plan.audio[1].gain, 0.6);
        // mic: file time = screen time - 0.5; three pieces (0..4, 4..6 at 2x, 6..10)
        let m = &plan.audio[0].segments;
        assert_eq!(m.len(), 3);
        assert!((m[0].src_start - 0.0).abs() < 1e-12 && (m[0].out_start - 0.5).abs() < 1e-12);
        assert_eq!(m[1].speed, 2.0);
        // muted or zero-gain tracks are not sent; a GIF has no audio
        p.audio.mic = Some(TrackMix { gain_db: 0.0, mute: true });
        let plan = export_plan(&p, SRC, &opts(640, 30)).unwrap();
        assert_eq!(plan.audio.iter().map(|a| a.track).collect::<Vec<_>>(), vec![AudioKind::System]);
        p.audio.mute = true;
        assert!(export_plan(&p, SRC, &opts(640, 30)).unwrap().audio.is_empty());
        p.audio.mute = false;
        let gif = ExportOptions { format: Format::Gif, ..opts(640, 15) };
        assert!(export_plan(&p, SRC, &gif).unwrap().audio.is_empty());
    }

    #[test]
    fn size_follows_the_aspect_with_even_sides() {
        let mut p = project(1.0);
        p.style.aspect = "9:16".into();
        assert_eq!(export_plan(&p, SRC, &opts(1080, 30)).unwrap().size, [1080, 1920]);
        p.style.aspect = "1:1".into();
        assert_eq!(export_plan(&p, SRC, &opts(960, 30)).unwrap().size, [960, 960]);
        p.style.aspect = "4:3".into();
        assert_eq!(export_plan(&p, SRC, &opts(960, 30)).unwrap().size, [960, 720]);
        // `source` (or anything unknown) follows the recorded video; 1920 x 1241 is rounded to an even side
        p.style.aspect = "source".into();
        assert_eq!(export_plan(&p, [3456, 2234], &opts(1920, 30)).unwrap().size, [1920, 1241 + 1]);
        assert_eq!(aspect_ratio("16:9", SRC), 16.0 / 9.0);
        assert_eq!(aspect_ratio("nonsense", SRC), 2.0);
        assert_eq!(aspect_ratio("0:9", SRC), 2.0);
    }

    #[test]
    fn limits_are_enforced() {
        let p = project(1.0);
        assert!(export_plan(&p, SRC, &opts(3840, 60)).is_ok(), "4K @ 60 is allowed");
        assert!(matches!(export_plan(&p, SRC, &opts(3842, 60)), Err(PlanError::BadSize(_))));
        assert!(matches!(export_plan(&p, SRC, &opts(1921, 30)), Err(PlanError::BadSize(_))), "odd width");
        assert!(matches!(export_plan(&p, SRC, &opts(8, 30)), Err(PlanError::BadSize(_))), "too small");
        assert_eq!(export_plan(&p, SRC, &opts(1920, 0)), Err(PlanError::BadFps(0)));
        assert_eq!(export_plan(&p, SRC, &opts(1920, 61)), Err(PlanError::BadFps(61)));
        let mut portrait = project(1.0);
        portrait.style.aspect = "9:16".into();
        assert!(matches!(export_plan(&portrait, SRC, &opts(3840, 30)), Err(PlanError::BadSize(_))), "3840 x 6827 is over budget");
        assert!(export_plan(&portrait, SRC, &opts(2160, 30)).is_ok(), "2160 x 3840 fits");
    }

    #[test]
    fn nothing_to_export_and_invalid_timelines_are_refused() {
        assert_eq!(export_plan(&project(0.0), SRC, &opts(1920, 30)), Err(PlanError::NothingToExport));
        let mut p = project(10.0);
        p.trims = vec![Trim { start: 5.0, end: 20.0 }];
        assert!(matches!(export_plan(&p, SRC, &opts(1920, 30)), Err(PlanError::InvalidTimeline(_))));
        assert!(PlanError::NothingToExport.to_string().contains("nothing to export"));
    }

    #[test]
    fn width_tiers() {
        assert_eq!(width_for(Some(1920), 16.0 / 9.0, SRC), 1920);
        assert_eq!(width_for(Some(3840), 16.0 / 9.0, SRC), 3840);
        // portrait: the long edge is the height
        assert_eq!(width_for(Some(1920), 9.0 / 16.0, SRC), 1080);
        assert_eq!(width_for(Some(3840), 9.0 / 16.0, SRC), 2160);
        assert_eq!(width_for(Some(1920), 1.0, SRC), 1920);
        // source: capped to 4K and even
        assert_eq!(width_for(None, 16.0 / 9.0, [3456, 2234]), 3456);
        assert_eq!(width_for(None, 16.0 / 9.0, [5120, 2880]), 3840);
        assert_eq!(width_for(None, 16.0 / 9.0, [1001, 600]), 1002);
    }

    #[test]
    fn source_pixels_prefers_the_video_size() {
        let mut e =
            Events::new(ScreenInfo { size: [3456, 2234], scale: 2.0, display_frame: [0.0, 0.0, 1728.0, 1117.0], capture_rect: None });
        assert_eq!(source_pixels(&e), [3456, 2234]);
        e.screen.size = [0, 0];
        assert_eq!(source_pixels(&e), [3456, 2234], "falls back to points x scale");
    }

    #[test]
    fn params_file_has_one_line_per_frame_with_source_times() {
        let mut p = project(4.0);
        p.trims = vec![Trim { start: 1.0, end: 3.0 }];
        p.speeds = vec![Speed { start: 2.0, end: 3.0, factor: 2.0 }];
        p.zooms = vec![Zoom {
            id: "z".into(),
            start: 1.2,
            end: 2.5,
            mode: "auto".into(),
            scale: 2.0,
            focus: Some([0.5, 0.5]),
            ease_in: 0.2,
            ease_out: 0.2,
        }];
        let e = Events::new(ScreenInfo { size: [2000, 1000], scale: 2.0, display_frame: [0.0, 0.0, 1000.0, 500.0], capture_rect: None });
        let plan = export_plan(&p, SRC, &opts(640, 30)).unwrap();
        let mut buf = vec![];
        write_frame_params(&p, &e, &plan, &mut buf).unwrap();
        let text = String::from_utf8(buf).unwrap();
        let lines: Vec<FrameLine> = text.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(lines.len() as u32, plan.frames);
        let tl = Timeline::new(&p);
        for (i, l) in lines.iter().enumerate() {
            // serde_json prints shortest decimals, which may differ from the f64 by one ulp
            assert!((l.t - tl.to_source(i as f64 / 30.0)).abs() < 1e-12, "frame {i}");
        }
        assert_eq!(lines[0].t, 1.0);
        // in the middle of the zoom the camera is set, before it it is not
        assert!(lines[0].params.camera.is_none());
        assert!(lines.iter().any(|l| l.params.camera.is_some()));
        // deterministic
        let mut again = vec![];
        write_frame_params(&p, &e, &plan, &mut again).unwrap();
        assert_eq!(text.as_bytes(), &again[..]);
    }

    #[test]
    fn frame_line_flattens_params_like_render_frame() {
        let l = FrameLine { t: 1.5, params: FrameParams::default() };
        assert_eq!(serde_json::to_string(&l).unwrap(), r#"{"t":1.5}"#);
        let back: FrameLine = serde_json::from_str(r#"{"t":2.0,"camera":{"x":0.25,"y":0.25,"w":0.5,"h":0.5}}"#).unwrap();
        assert_eq!(back.params.camera.map(|c| c.w), Some(0.5));
    }
    fn gif_opts(width: u32, fps: u32) -> ExportOptions {
        ExportOptions { format: Format::Gif, ..opts(width, fps) }
    }

    #[test]
    fn gif_plan_clamps_width_and_fps_but_mp4_is_unchanged() {
        let p = project(2.0);
        let plan = export_plan(&p, SRC, &gif_opts(1920, 30)).unwrap();
        assert_eq!((plan.size, plan.fps, plan.frames, plan.format), ([960, 540], 15, 30, Format::Gif));
        let plan = export_plan(&p, SRC, &gif_opts(3840, 60)).unwrap();
        assert_eq!((plan.size[0], plan.fps), (960, 15));
        let plan = export_plan(&p, SRC, &gif_opts(480, 10)).unwrap();
        assert_eq!((plan.size, plan.fps, plan.frames), ([480, 270], 10, 20), "within the limits nothing changes");
        let plan = export_plan(&p, SRC, &gif_opts(960, 15)).unwrap();
        assert_eq!((plan.size[0], plan.fps), (960, 15), "the limits themselves are allowed");
        // MP4 keeps its own limits
        let mp4 = export_plan(&p, SRC, &opts(3840, 60)).unwrap();
        assert_eq!((mp4.size[0], mp4.fps, mp4.format), (3840, 60, Format::Mp4));
        // still validated like any export
        assert!(matches!(export_plan(&p, SRC, &gif_opts(8, 15)), Err(PlanError::BadSize(_))));
        assert!(matches!(export_plan(&p, SRC, &gif_opts(481, 15)), Err(PlanError::BadSize(_))), "odd width");
        assert_eq!(export_plan(&p, SRC, &gif_opts(960, 0)), Err(PlanError::BadFps(0)));
        assert_eq!(export_plan(&p, SRC, &gif_opts(960, 61)), Err(PlanError::BadFps(61)));
        // portrait: the width is clamped, the height follows (960 x 1707 -> even)
        let mut portrait = project(1.0);
        portrait.style.aspect = "9:16".into();
        assert_eq!(export_plan(&portrait, SRC, &gif_opts(2160, 30)).unwrap().size, [960, 1708]);
    }

    #[test]
    fn gif_length_is_capped_by_a_pixel_budget_with_a_clear_message() {
        // 960 x 540 at 15 fps: 300 frames = 20 s
        assert!(export_plan(&project(20.0), SRC, &gif_opts(960, 15)).is_ok());
        let err = export_plan(&project(21.0), SRC, &gif_opts(960, 15)).unwrap_err();
        assert!(matches!(err, PlanError::GifTooLong { frames: 315, .. }), "{err:?}");
        let text = err.to_string();
        assert!(text.contains("too long for a GIF") && text.contains("20 seconds") && text.contains("MP4"), "{text}");
        // a lower fps or a smaller picture allows a longer clip, taller pictures a shorter one, MP4 has no cap
        assert!(export_plan(&project(21.0), SRC, &gif_opts(960, 10)).is_ok());
        assert!(export_plan(&project(60.0), SRC, &gif_opts(480, 15)).is_ok(), "480 x 270: 1200 frames");
        assert!(export_plan(&project(81.0), SRC, &gif_opts(480, 15)).is_err());
        let mut portrait = project(7.0);
        portrait.style.aspect = "9:16".into();
        assert!(matches!(export_plan(&portrait, SRC, &gif_opts(960, 15)), Err(PlanError::GifTooLong { .. })), "960 x 1708: 94 frames");
        portrait.duration = 6.0;
        assert!(export_plan(&portrait, SRC, &gif_opts(960, 15)).is_ok());
        assert!(export_plan(&project(600.0), SRC, &opts(960, 15)).is_ok());
    }

    #[test]
    fn gif_delays_spread_the_rounding_so_the_total_stays_exact() {
        // 15 fps = 6.67 cs per frame: 7, 6, 7 repeating, never 7 x n
        assert_eq!(gif_delays(6, 15), vec![7, 6, 7, 7, 6, 7]);
        assert_eq!(gif_delays(15, 15).iter().sum::<u32>(), 100, "15 frames at 15 fps last exactly 1 s");
        assert_eq!(gif_delays(10, 10), vec![10; 10]);
        assert!(gif_delays(0, 15).is_empty());
        for fps in 1..=GIF_MAX_FPS {
            for frames in [1u32, 2, 7, 15, 100, 149, 150, 450] {
                let d = gif_delays(frames, fps);
                assert_eq!(d.len() as u32, frames);
                let total: u32 = d.iter().sum();
                let exact = f64::from(frames) * 100.0 / f64::from(fps);
                assert!((f64::from(total) - exact).abs() <= 0.5, "{frames} @ {fps}: {total} vs {exact}");
                let nominal = 100.0 / f64::from(fps);
                assert!(d.iter().all(|&c| c >= 2 && (f64::from(c) - nominal).abs() < 1.0), "{d:?}");
            }
        }
    }

    #[test]
    fn gif_total_duration_is_within_one_frame_delay_of_output_duration() {
        let mut p = project(10.0);
        p.trims = vec![Trim { start: 0.5, end: 7.3 }];
        p.speeds = vec![Speed { start: 2.0, end: 4.0, factor: 1.5 }];
        let exact = Timeline::new(&p).output_duration();
        for fps in [5, 10, 12, 15] {
            let plan = export_plan(&p, SRC, &gif_opts(480, fps)).unwrap();
            let total = f64::from(gif_delays(plan.frames, plan.fps).iter().sum::<u32>()) / 100.0;
            assert!((total - exact).abs() <= 1.0 / f64::from(fps), "{fps} fps: {total} vs {exact}");
        }
    }
}
