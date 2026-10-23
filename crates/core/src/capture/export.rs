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

use super::compose::{Composer, FrameParams};
use super::events::Events;
use super::project::Project;
use super::timeline::{self, Timeline};
use serde::{Deserialize, Serialize};
use std::io::{self, Write};

/// Longest side of an export (4K UHD).
pub const MAX_LONG_EDGE: u32 = 3840;
/// Pixel budget: 3840 x 2160, so portrait and square outputs are bounded too.
pub const MAX_PIXELS: u64 = 3840 * 2160;
pub const MAX_FPS: u32 = 60;
pub const MIN_WIDTH: u32 = 16;

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

/// Container of an export. GIF arrives with T29.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Mp4,
}

/// What the user chose in the export sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportOptions {
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

#[derive(Debug, Clone, PartialEq)]
pub struct ExportPlan {
    pub segments: Vec<Segment>,
    /// `timeline::output_duration`.
    pub duration: f64,
    pub frames: u32,
    /// Output pixels, both even.
    pub size: [u32; 2],
    pub fps: u32,
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
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NothingToExport => write!(f, "There is nothing to export: the edit keeps no video."),
            Self::InvalidTimeline(m) => write!(f, "The edit is not valid: {m}"),
            Self::BadSize(m) => write!(f, "{m}"),
            Self::BadFps(n) => write!(f, "{n} fps is not supported: use 1 to {MAX_FPS}."),
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
    let w = opts.width;
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
    let frames = (duration * f64::from(opts.fps)).round().max(1.0) as u32;
    Ok(ExportPlan { segments, duration, frames, size: [w, h], fps: opts.fps, codec: opts.codec, quality: opts.quality })
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
    let tl = Timeline::new(project);
    let composer = Composer::new(project, events);
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
        ExportOptions { codec: Codec::H264, width, fps, quality: Quality::Standard }
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
}
