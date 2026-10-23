//! Per-frame composition parameters for the editor preview and export: everything the
//! Swift renderer needs to draw one frame, evaluated here so Swift only draws.
//!
//! All positions are normalised to the recorded video (`0..=1`, top-left origin): the
//! display, or `screen.capture_rect` for Record Area / Window (see [`CaptureSpace`]; a
//! cursor or click outside that rectangle is hidden, the camera clamps to its edge). The
//! camera is the viewport rectangle of [`zoom::viewport_at`]; the cursor is the smoothed
//! path; the click ring is anchored at the RAW click position (the smoothed cursor can
//! lag a click, see `cursor::smooth`); the blur comes from finite-difference velocities
//! of the same functions. Pure and deterministic.
use super::cursor::{self, CursorOpts, Sample};
use super::curves::clamp;
use super::events::{CaptureSpace, Events};
use super::project::Project;
use super::zoom::{self, Viewport};
use serde::{Deserialize, Serialize};

/// Viewport rectangle `[x, y, w, h]` as fractions of the display.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Camera {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Camera {
    pub const IDENTITY: Camera = Camera { x: 0.0, y: 0.0, w: 1.0, h: 1.0 };
}

/// Cursor sprite at one instant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CursorParams {
    pub x: f64,
    pub y: f64,
    /// `arrow`, `ibeam`, `hand`, `crosshair`, `grab`, `resize-h`, `resize-v` (unknown = arrow).
    pub kind: String,
    /// `project.cursor.size`, a multiplier of the sprite's base size.
    pub size: f64,
    /// 0..1 (> 0 here: a fully hidden cursor is `None`).
    pub opacity: f64,
}

/// Click ring at the raw click position.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RingParams {
    pub x: f64,
    pub y: f64,
    /// Fraction of the display width (it is magnified by the camera like the content).
    pub radius: f64,
    pub alpha: f64,
}

/// Directional blur: `samples` taps spread over `shutter` of the frame along `dir`
/// (unit vector, y down), `length` as a fraction of the drawn picture width.
/// `scene`: the camera is the faster mover, so the whole picture is blurred; otherwise only
/// the cursor sprite is (a fast cursor must not smear a static screen).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlurParams {
    pub samples: u32,
    pub shutter: f64,
    pub dir: [f64; 2],
    pub length: f64,
    #[serde(default)]
    pub scene: bool,
}

/// The optional `camera`, `cursor`, `ring` and `blur` fields of `render.frame`.
/// `FrameParams::default()` is the no-op: the frame is rendered exactly as before P3.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FrameParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<Camera>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<CursorParams>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ring: Option<RingParams>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blur: Option<BlurParams>,
}

impl FrameParams {
    pub fn is_noop(&self) -> bool {
        *self == FrameParams::default()
    }
}

/// Parameters for source time `t` (seconds). Builds a [`Composer`] each call; callers that
/// render many frames of one take should keep the composer.
pub fn frame_params(project: &Project, events: &Events, t: f64) -> FrameParams {
    Composer::new(project, events).at(t)
}

/// Half-width (seconds) of the central difference used for velocities.
const DT: f64 = 1.0 / 120.0;
/// The `motion_blur` setting that means "the plan's default blur length".
const BLUR_REFERENCE: f64 = 0.5;

/// Per-take state (smoothed cursor path, idle spans) shared by all frames.
pub struct Composer<'a> {
    project: &'a Project,
    opts: CursorOpts,
    smooth: Vec<Sample>,
    idle: Vec<(f64, f64)>,
    clicks: Vec<super::events::Click>,
    /// `(t, kind)` of the cursor log, sorted.
    kinds: Vec<(f64, String)>,
    /// Display size in points (the unit of `events.json` coordinates).
    disp: [f64; 2],
    /// Display points -> normalised position in `screen.mov` (display or capture rect).
    space: CaptureSpace,
    /// Recorded pixels across the video (blur and ring are authored in these).
    px: [f64; 2],
}

impl<'a> Composer<'a> {
    pub fn new(project: &'a Project, events: &'a Events) -> Self {
        let c = &project.cursor;
        let opts = CursorOpts { strength: c.smoothing, hide_idle_after: c.hide_idle_after, ..CursorOpts::default() };
        let mut log: Vec<&super::events::CursorSample> =
            events.cursor.iter().filter(|s| s.0.is_finite() && s.1.is_finite() && s.2.is_finite()).collect();
        log.sort_by(|a, b| a.0.total_cmp(&b.0));
        let raw: Vec<Sample> = log.iter().map(|s| Sample::from(*s)).collect();
        let kinds = log.iter().map(|s| (s.0, s.3.clone())).collect();
        let smooth = cursor::smooth(&raw, &opts);
        let idle = cursor::idle_spans(&raw, opts.hide_idle_after);
        let mut clicks = events.clicks.clone();
        clicks.retain(|k| k.t.is_finite() && k.x.is_finite() && k.y.is_finite());
        clicks.sort_by(|a, b| a.t.total_cmp(&b.t));
        let [_, _, dw, dh] = events.screen.display_frame;
        let sc = events.screen.scale.max(f64::MIN_POSITIVE);
        let space = CaptureSpace::new(&events.screen);
        let [vw, vh] = space.size();
        let px = |pixels: u32, points: f64| if pixels > 0 { pixels as f64 } else { points * sc };
        Self {
            project,
            opts,
            smooth,
            idle,
            clicks,
            kinds,
            disp: [dw, dh],
            space,
            px: [px(events.screen.size[0], vw), px(events.screen.size[1], vh)],
        }
    }

    fn usable(&self) -> bool {
        self.disp.iter().all(|d| *d > 0.0 && d.is_finite()) && self.px.iter().all(|p| *p > 0.0)
    }

    /// Smoothed cursor, normalised to the video and clamped to its edge (camera follow, blur).
    fn smoothed(&self, t: f64) -> Option<[f64; 2]> {
        cursor::cursor_at(&self.smooth, t).map(|(x, y)| self.space.point_clamped(x, y))
    }

    /// Active zoom (the latest started wins when they overlap) and the viewport it gives.
    fn viewport(&self, t: f64) -> Viewport {
        let full = Viewport { cx: 0.5, cy: 0.5, scale: 1.0 };
        let z = self.project.zooms.iter().filter(|z| t > z.start && t < z.end + z.ease_out).max_by(|a, b| a.start.total_cmp(&b.start));
        z.map_or(full, |z| zoom::viewport_at(z, t, self.smoothed(t)))
    }

    /// Cursor and camera-centre velocity in recorded px/s, scaled by the zoom (on-screen speed).
    fn velocities(&self, t: f64) -> ([f64; 2], [f64; 2]) {
        let (a, b) = (t - DT, t + DT);
        let (va, vb) = (self.viewport(a), self.viewport(b));
        let vt = self.viewport(t);
        let cur = |t: f64| self.smoothed(t).unwrap_or([0.0; 2]);
        let (ca, cb) = (cur(a), cur(b));
        let span = 2.0 * DT;
        let cursor_v = [(cb[0] - ca[0]) * self.px[0] / span * vt.scale, (cb[1] - ca[1]) * self.px[1] / span * vt.scale];
        let camera_v = [(vb.cx - va.cx) * self.px[0] / span * vt.scale, (vb.cy - va.cy) * self.px[1] / span * vt.scale];
        (cursor_v, camera_v)
    }

    pub fn at(&self, t: f64) -> FrameParams {
        if !t.is_finite() || !self.usable() {
            return FrameParams::default();
        }
        let c = &self.project.cursor;
        let vp = self.viewport(t);
        let camera = (vp.scale > 1.0 + 1e-9).then(|| {
            let [x, y, w, h] = vp.rect();
            Camera { x: x.clamp(0.0, 1.0 - w), y: y.clamp(0.0, 1.0 - h), w, h }
        });

        let kind = self.kind_at(t);
        let opacity = cursor::visible_at(&self.idle, t, &self.opts);
        let hidden = matches!(c.style.as_str(), "none" | "hidden");
        // A cursor outside the recorded rectangle is not in the video: hidden, not clamped.
        let visible = cursor::cursor_at(&self.smooth, t).filter(|&(x, y)| self.space.contains(x, y));
        let cursor_p = match (visible.and(self.smoothed(t)), kind) {
            (Some([x, y]), Some(kind)) if opacity > 0.0 && !hidden => Some(CursorParams { x, y, kind, size: c.size, opacity }),
            _ => None,
        };

        let ring = (c.click_effect == "ring")
            .then(|| cursor::click_ring(t, &self.clicks, &self.opts))
            .flatten()
            .filter(|r| self.space.contains(r.x, r.y))
            .map(|r| {
                let [x, y] = self.space.point_clamped(r.x, r.y);
                RingParams { x, y, radius: r.radius / self.px[0], alpha: r.alpha }
            });

        let amount = clamp01(c.motion_blur);
        let blur = (amount > 0.0).then(|| {
            let (cv, kv) = self.velocities(t);
            let b = cursor::blur_params(cv, kv, &self.opts);
            (b.samples > 0 && b.length > 0.0).then(|| BlurParams {
                samples: b.samples,
                shutter: b.shutter,
                dir: b.dir,
                length: b.length * amount / BLUR_REFERENCE / self.px[0],
                scene: kv[0].hypot(kv[1]) > cv[0].hypot(cv[1]),
            })
        });
        FrameParams { camera, cursor: cursor_p, ring, blur: blur.flatten() }
    }

    /// Kind of the latest cursor sample at or before `t` (the first sample before the log starts).
    fn kind_at(&self, t: f64) -> Option<String> {
        let i = self.kinds.partition_point(|k| k.0 <= t).saturating_sub(1);
        self.kinds.get(i).map(|k| k.1.clone())
    }
}

fn clamp01(x: f64) -> f64 {
    clamp(x, 0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::super::events::{Click, CursorSample, ScreenInfo};
    use super::super::project::Zoom;
    use super::*;

    /// 1000 x 500 pt display at 2x (2000 x 1000 px).
    fn events() -> Events {
        Events::new(ScreenInfo { size: [2000, 1000], scale: 2.0, display_frame: [0.0, 0.0, 1000.0, 500.0], capture_rect: None })
    }

    fn project() -> Project {
        Project::new("t", "2026-01-01T00:00:00Z")
    }

    fn zoom(start: f64, end: f64, scale: f64, focus: Option<[f64; 2]>) -> Zoom {
        Zoom { id: "z1".into(), start, end, mode: "auto".into(), scale, focus, ease_in: 0.4, ease_out: 0.4 }
    }

    fn at(t: f64, x: f64, y: f64) -> CursorSample {
        CursorSample(t, x, y, "arrow".into())
    }

    fn click(t: f64, x: f64, y: f64) -> Click {
        Click { t, x, y, button: "left".into(), down: true, up: false }
    }

    /// Cursor log at 60 Hz along `f(t)` for `secs`.
    fn path(secs: f64, f: impl Fn(f64) -> (f64, f64)) -> Vec<CursorSample> {
        (0..=(secs * 60.0) as usize)
            .map(|i| {
                let t = i as f64 / 60.0;
                let (x, y) = f(t);
                at(t, x, y)
            })
            .collect()
    }

    #[test]
    fn same_inputs_give_identical_params() {
        let mut p = project();
        p.zooms = vec![zoom(1.0, 3.0, 2.0, Some([0.5, 0.5]))];
        let mut e = events();
        e.cursor = path(4.0, |t| (100.0 + t * 150.0, 200.0 + 30.0 * (t * 3.0).sin()));
        e.clicks = vec![click(1.5, 300.0, 250.0)];
        for t in [0.0, 0.7, 1.55, 2.0, 3.2, 9.0] {
            assert_eq!(frame_params(&p, &e, t), frame_params(&p, &e, t));
        }
    }

    #[test]
    fn no_zoom_is_the_identity_camera() {
        let mut e = events();
        e.cursor = path(2.0, |t| (100.0 + t * 100.0, 200.0));
        let f = frame_params(&project(), &e, 1.0);
        assert_eq!(f.camera, None, "identity camera is omitted");
        // zoomed out before the zoom, and again after its ease-out
        let mut p = project();
        p.zooms = vec![zoom(1.0, 2.0, 2.0, None)];
        assert_eq!(frame_params(&p, &e, 0.5).camera, None);
        assert_eq!(frame_params(&p, &e, 5.0).camera, None);
    }

    #[test]
    fn zoom_gives_a_viewport_inside_the_display() {
        let mut p = project();
        p.zooms = vec![zoom(1.0, 3.0, 2.0, Some([0.5, 0.5]))];
        let mut e = events();
        e.cursor = path(4.0, |_| (500.0, 250.0));
        let c = frame_params(&p, &e, 2.0).camera.expect("zoomed");
        assert!((c.w - 0.5).abs() < 1e-3 && (c.h - 0.5).abs() < 1e-3, "{c:?}");
        assert!((c.x - 0.25).abs() < 1e-3 && (c.y - 0.25).abs() < 1e-3, "{c:?}");
    }

    #[test]
    fn the_viewport_never_leaves_the_display() {
        let mut p = project();
        p.zooms = vec![zoom(0.5, 6.0, 6.0, Some([0.99, 0.01])), zoom(7.0, 9.0, 3.0, None)];
        let mut e = events();
        // the cursor sweeps the corners and beyond the display
        e.cursor = path(10.0, |t| if (t as u64).is_multiple_of(2) { (-50.0 + t * 400.0, 520.0) } else { (1100.0, -30.0 + t * 30.0) });
        let mut t = -1.0;
        while t < 11.0 {
            if let Some(c) = frame_params(&p, &e, t).camera {
                assert!(c.x >= 0.0 && c.y >= 0.0 && c.x + c.w <= 1.0 && c.y + c.h <= 1.0, "t={t} {c:?}");
                assert!(c.w > 0.0 && c.h > 0.0);
            }
            t += 0.05;
        }
    }

    #[test]
    fn the_cursor_is_hidden_while_idle_and_returns_on_movement() {
        let mut e = events();
        // moves for 1 s, rests 1..10 s (collapsed like the Swift log), then moves again
        e.cursor = path(1.0, |t| (100.0 + t * 100.0, 200.0));
        e.cursor.push(at(10.0, 200.0, 200.0));
        e.cursor.push(at(10.5, 400.0, 200.0));
        let p = project(); // hide_idle_after = 3 s
        let c = frame_params(&p, &e, 2.0).cursor.expect("visible 1 s into the rest");
        assert_eq!((c.opacity, c.kind.as_str()), (1.0, "arrow"));
        assert_eq!(c.size, 1.4);
        assert!((c.x - 0.2).abs() < 1e-3 && (c.y - 0.4).abs() < 1e-3, "{c:?}");
        let fading = frame_params(&p, &e, 4.15).cursor.expect("fading out");
        assert!(fading.opacity > 0.0 && fading.opacity < 1.0, "{fading:?}");
        assert_eq!(frame_params(&p, &e, 7.0).cursor, None, "hidden in the middle of the idle span");
        assert!(frame_params(&p, &e, 10.5).cursor.is_some());
    }

    #[test]
    fn the_cursor_kind_follows_the_log() {
        let mut e = events();
        e.cursor =
            vec![at(0.0, 100.0, 100.0), CursorSample(1.0, 110.0, 100.0, "ibeam".into()), CursorSample(2.0, 120.0, 100.0, "hand".into())];
        let p = project();
        assert_eq!(frame_params(&p, &e, 0.5).cursor.unwrap().kind, "arrow");
        assert_eq!(frame_params(&p, &e, 1.5).cursor.unwrap().kind, "ibeam");
        assert_eq!(frame_params(&p, &e, 2.5).cursor.unwrap().kind, "hand");
    }

    #[test]
    fn the_ring_lives_only_inside_its_envelope_at_the_raw_click() {
        let mut e = events();
        e.cursor = path(3.0, |t| (100.0 + t * 100.0, 200.0));
        e.clicks = vec![click(1.0, 200.0, 250.0)];
        let p = project();
        assert_eq!(frame_params(&p, &e, 0.99).ring, None);
        let r = frame_params(&p, &e, 1.2).ring.expect("inside the envelope");
        assert!((r.x - 0.2).abs() < 1e-9 && (r.y - 0.5).abs() < 1e-9, "raw click, normalised: {r:?}");
        assert!(r.alpha > 0.0 && r.alpha < 1.0 && r.radius > 0.0, "{r:?}");
        assert!(r.radius < 36.0 / 2000.0 + 1e-9, "fraction of the display width: {r:?}");
        assert_eq!(frame_params(&p, &e, 1.0 + CursorOpts::default().ring_secs + 0.01).ring, None);
        // the drawn cursor is the smoothed one, not the click position
        let c = frame_params(&p, &e, 1.2).cursor.unwrap();
        assert!((c.y - 0.4).abs() < 1e-3 && (r.y - c.y).abs() > 0.05);
    }

    #[test]
    fn effects_can_be_switched_off_in_the_project() {
        let mut e = events();
        e.cursor = path(2.0, |t| (100.0 + t * 1500.0, 200.0));
        e.clicks = vec![click(0.5, 300.0, 200.0)];
        let mut p = project();
        assert!(frame_params(&p, &e, 0.6).ring.is_some() && frame_params(&p, &e, 0.6).blur.is_some());
        p.cursor.click_effect = "none".into();
        p.cursor.motion_blur = 0.0;
        let f = frame_params(&p, &e, 0.6);
        assert!(f.ring.is_none() && f.blur.is_none(), "{f:?}");
        p.cursor.style = "none".into();
        assert!(frame_params(&p, &e, 0.6).cursor.is_none());
    }

    #[test]
    fn blur_is_zero_at_rest_and_follows_the_cursor_speed() {
        let mut e = events();
        // still until 1 s, then 1500 pt/s to the right (3000 px/s), then still
        e.cursor = vec![at(0.0, 100.0, 200.0), at(1.0, 100.0, 200.0), at(1.2, 400.0, 200.0), at(3.0, 400.0, 200.0)];
        let p = project();
        assert_eq!(frame_params(&p, &e, 0.5).blur, None);
        assert_eq!(frame_params(&p, &e, 2.5).blur, None);
        let b = frame_params(&p, &e, 1.1).blur.expect("moving");
        assert!(b.samples >= 2 && b.length > 0.0 && b.shutter > 0.0, "{b:?}");
        assert!(b.dir[0] > 0.9 && b.dir[1].abs() < 0.1, "to the right: {b:?}");
        assert!(b.length <= 64.0 / 2000.0 + 1e-9, "capped at blur_max_length px: {b:?}");
        assert!(!b.scene, "a fast cursor over a still camera blurs the cursor only: {b:?}");
    }

    #[test]
    fn a_panning_camera_blurs_even_with_a_still_cursor() {
        let mut p = project();
        p.zooms = vec![zoom(0.0, 5.0, 4.0, Some([0.1, 0.5]))];
        let mut e = events();
        // zoom in on the left; the cursor then jumps right, the camera follows
        e.cursor = vec![at(0.0, 100.0, 250.0), at(2.0, 100.0, 250.0), at(2.2, 900.0, 250.0), at(5.0, 900.0, 250.0)];
        let mut peak: f64 = 0.0;
        let mut scene = false;
        let mut t = 2.0;
        while t < 4.0 {
            if let Some(b) = frame_params(&p, &e, t).blur {
                peak = peak.max(b.length);
                scene |= b.scene;
            }
            t += 0.02;
        }
        assert!(peak > 0.0, "camera pan produced no blur");
        assert!(scene, "the camera dominates at some point: the whole picture blurs");
        assert_eq!(frame_params(&p, &e, 4.9).blur, None, "settled");
    }

    #[test]
    fn a_project_without_p3_inputs_is_the_noop() {
        let f = frame_params(&project(), &events(), 1.0);
        assert!(f.is_noop(), "{f:?}");
        assert_eq!(serde_json::to_string(&f).unwrap(), "{}");
        assert!(frame_params(&project(), &events(), f64::NAN).is_noop());
    }

    #[test]
    fn params_round_trip_through_json() {
        let mut p = project();
        p.zooms = vec![zoom(0.5, 3.0, 2.0, Some([0.4, 0.6]))];
        let mut e = events();
        e.cursor = path(3.0, |t| (100.0 + t * 200.0, 200.0));
        e.clicks = vec![click(1.0, 300.0, 200.0)];
        let f = frame_params(&p, &e, 1.1);
        assert!(f.camera.is_some() && f.cursor.is_some() && f.ring.is_some());
        let s = serde_json::to_string(&f).unwrap();
        // serde_json's default float parsing can differ in the last bit: compare within 1e-12.
        let back = serde_json::from_str::<FrameParams>(&s).unwrap();
        let (a, b) = (back.ring.unwrap(), f.ring.clone().unwrap());
        assert!((a.radius - b.radius).abs() < 1e-12 && a.alpha == b.alpha);
        assert_eq!(
            (back.cursor.as_ref().map(|c| c.kind.clone()), back.blur.map(|b| b.samples)),
            (f.cursor.map(|c| c.kind), f.blur.map(|b| b.samples))
        );
        assert!(back.camera.is_some());
    }

    /// Same 1000 x 500 pt display, but only its lower-right quarter `[500, 250, 500, 250]`
    /// was recorded: `screen.mov` is 1000 x 500 px (2x of 500 x 250 pt).
    fn area() -> Events {
        Events::new(ScreenInfo {
            size: [1000, 500],
            scale: 2.0,
            display_frame: [0.0, 0.0, 1000.0, 500.0],
            capture_rect: Some([500.0, 250.0, 500.0, 250.0]),
        })
    }

    fn approx(a: &serde_json::Value, b: &serde_json::Value) -> bool {
        use serde_json::Value::*;
        match (a, b) {
            (Number(x), Number(y)) => (x.as_f64().unwrap() - y.as_f64().unwrap()).abs() < 1e-9,
            (Array(x), Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(a, b)| approx(a, b)),
            (Object(x), Object(y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| approx(v, w))),
            _ => a == b,
        }
    }

    #[test]
    fn an_area_recording_maps_the_cursor_into_the_rect() {
        let mut e = area();
        e.cursor = path(1.0, |_| (750.0, 375.0)); // the middle of the rect
        let c = frame_params(&project(), &e, 0.5).cursor.expect("inside the rect");
        assert!((c.x - 0.5).abs() < 1e-9 && (c.y - 0.5).abs() < 1e-9, "{c:?}");
        e.cursor = path(1.0, |_| (625.0, 312.5)); // a quarter in
        let c = frame_params(&project(), &e, 0.5).cursor.unwrap();
        assert!((c.x - 0.25).abs() < 1e-9 && (c.y - 0.25).abs() < 1e-9, "{c:?}");
    }

    #[test]
    fn an_area_recording_maps_ring_and_camera_and_keeps_the_blur_in_video_pixels() {
        let mut p = project();
        p.zooms = vec![zoom(0.0, 3.0, 2.0, Some([0.5, 0.5]))];
        let mut e = area();
        e.cursor = path(3.0, |_| (750.0, 375.0));
        e.clicks = vec![click(1.0, 625.0, 312.5)];
        let f = frame_params(&p, &e, 1.2);
        let r = f.ring.expect("ring");
        assert!((r.x - 0.25).abs() < 1e-9 && (r.y - 0.25).abs() < 1e-9, "{r:?}");
        assert!(r.radius < 36.0 / 1000.0 + 1e-9, "fraction of the VIDEO width (1000 px): {r:?}");
        // the cursor sits at the rect centre, i.e. the zoom focus: the camera does not drift
        let c = f.camera.expect("zoomed");
        assert!((c.x - 0.25).abs() < 1e-3 && (c.y - 0.25).abs() < 1e-3 && (c.w - 0.5).abs() < 1e-3, "{c:?}");
        // blur velocity: 400 pt/s right = 800 px/s of the 1000 px wide video, same as the equivalent display take
        let mut a = area();
        a.cursor = vec![at(0.0, 600.0, 300.0), at(1.0, 600.0, 300.0), at(1.2, 680.0, 300.0), at(3.0, 680.0, 300.0)];
        let mut d = events();
        d.screen.size = [1000, 500]; // a 500 x 250 pt display, so the rect-relative motion is identical
        d.screen.display_frame = [0.0, 0.0, 500.0, 250.0];
        d.cursor = vec![at(0.0, 100.0, 50.0), at(1.0, 100.0, 50.0), at(1.2, 180.0, 50.0), at(3.0, 180.0, 50.0)];
        d.clicks = vec![];
        for t in [0.5, 1.05, 1.1, 1.15, 2.0] {
            // the smoothing runs on absolute points, so the last bit may differ: compare within 1e-9
            let (x, y) = (
                serde_json::to_value(frame_params(&project(), &a, t)).unwrap(),
                serde_json::to_value(frame_params(&project(), &d, t)).unwrap(),
            );
            assert!(approx(&x, &y), "t={t}: {x} vs {y}");
        }
        assert!(frame_params(&project(), &a, 1.1).blur.is_some());
    }

    #[test]
    fn outside_the_rect_the_cursor_and_ring_are_hidden_and_the_camera_clamps() {
        let mut p = project();
        p.zooms = vec![zoom(0.0, 5.0, 2.0, None)];
        let mut e = area();
        // 0..1 s inside, then the pointer leaves to the upper left (outside the rect), clicks there
        e.cursor = vec![at(0.0, 900.0, 400.0), at(1.0, 900.0, 400.0), at(1.1, 100.0, 100.0), at(4.0, 100.0, 100.0)];
        e.clicks = vec![click(2.0, 100.0, 100.0)];
        assert!(frame_params(&p, &e, 0.5).cursor.is_some());
        let out = frame_params(&p, &e, 2.1);
        assert_eq!(out.cursor, None, "a cursor outside the recorded area is not drawn");
        assert_eq!(out.ring, None, "nor is a click ring outside it");
        let c = out.camera.expect("the zoom goes on");
        assert!(c.x >= 0.0 && c.y >= 0.0 && c.x + c.w <= 1.0 + 1e-12 && c.y + c.h <= 1.0 + 1e-12, "{c:?}");
        assert!(c.x < 0.01 && c.y < 0.01, "following a cursor clamped to the top-left edge: {c:?}");
    }

    #[test]
    fn a_rect_equal_to_the_display_is_the_identity() {
        let mut p = project();
        p.zooms = vec![zoom(1.0, 3.0, 2.0, None)];
        let mut e = events();
        e.cursor = path(4.0, |t| (100.0 + t * 150.0, 200.0 + 30.0 * (t * 3.0).sin()));
        e.clicks = vec![click(1.5, 300.0, 250.0)];
        let mut w = e.clone();
        w.screen.capture_rect = Some([0.0, 0.0, 1000.0, 500.0]);
        for t in [0.0, 0.7, 1.55, 2.0, 3.2, 9.0] {
            assert_eq!(frame_params(&p, &e, t), frame_params(&p, &w, t), "t={t}");
        }
    }
}
