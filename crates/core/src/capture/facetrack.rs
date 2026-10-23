//! Face-tracking maths (P6 T42): the `camera_track.json` sample format and the PURE reframing
//! of the camera image on the face. Nothing here touches Vision or the filesystem except
//! [`FaceTrack::load`]; the same track and options always give the same crop, bit for bit,
//! whatever order the frames are asked in (so exports are reproducible).

use super::curves::{clamp, SETTLE};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

/// File name of the track inside a project folder.
pub const TRACK_FILE: &str = "camera_track.json";
/// Frames per second assumed when the file has no usable `fps`.
pub const DEFAULT_FPS: f64 = 30.0;
/// Smallest and largest reframing zoom.
pub const ZOOM_MIN: f64 = 1.0;
pub const ZOOM_MAX: f64 = 3.0;
/// Fixed simulation step of the smoother (seconds). Part of the output contract: results are
/// the filter state on this grid, linearly interpolated.
pub const STEP: f64 = 1.0 / 120.0;
/// The smoother is simulated for at most this long; later times hold the last position.
pub const MAX_SECONDS: f64 = 4.0 * 3600.0;

/// One detected face at camera time `t` (seconds). `x, y` is the top-left corner of the box,
/// `w, h` its size, all normalised to the camera frame with a TOP-LEFT origin (Vision's
/// bottom-left rects must be flipped by the writer).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FaceSample {
    pub t: f64,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub confidence: f64,
}

/// `camera_track.json`: `{"fps": 30, "samples": [{"t","x","y","w","h","confidence"}, ...]}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FaceTrack {
    pub fps: f64,
    /// Sorted by `t`. Frames without a face are simply absent (gaps are fine).
    pub samples: Vec<FaceSample>,
}

impl FaceTrack {
    /// Decode a track, tolerating bad data: unusable samples (missing or non-finite numbers,
    /// empty boxes) are dropped, boxes are clamped into the frame, a missing confidence counts
    /// as 1, a bad `fps` becomes 30 and samples are sorted by time. Only text that is not a JSON
    /// object (or whose `samples` is not a list) is an error.
    pub fn parse(json: &str) -> Result<Self, String> {
        let v: Value = serde_json::from_str(json).map_err(|e| format!("camera_track.json is not valid: {e}"))?;
        let obj = v.as_object().ok_or("camera_track.json must be an object")?;
        let fps = obj.get("fps").and_then(Value::as_f64).filter(|f| f.is_finite() && *f > 0.0).unwrap_or(DEFAULT_FPS);
        let list = match obj.get("samples") {
            None | Some(Value::Null) => &[][..],
            Some(Value::Array(a)) => a.as_slice(),
            Some(_) => return Err("camera_track.json: samples must be a list".into()),
        };
        let mut samples: Vec<FaceSample> = list.iter().filter_map(sample_from).collect();
        samples.sort_by(|a, b| a.t.total_cmp(&b.t));
        Ok(Self { fps, samples })
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        Self::parse(&text)
    }
}

fn sample_from(v: &Value) -> Option<FaceSample> {
    let n = |k: &str| v.get(k).and_then(Value::as_f64).filter(|x| x.is_finite());
    let (t, x, y, w, h) = (n("t")?, n("x")?, n("y")?, n("w")?, n("h")?);
    let (w, h) = (w.min(1.0), h.min(1.0));
    if t < 0.0 || w <= 0.0 || h <= 0.0 {
        return None;
    }
    Some(FaceSample {
        t,
        x: clamp(x, 0.0, 1.0 - w),
        y: clamp(y, 0.0, 1.0 - h),
        w,
        h,
        confidence: n("confidence").map_or(1.0, |c| clamp(c, 0.0, 1.0)),
    })
}

/// Reframing options.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReframeOpts {
    /// Width / height of the crop (1 = square, the overlay bubble).
    pub crop_aspect: f64,
    /// `1..=3`: 1 is the largest crop of that aspect, 3 is three times tighter.
    pub zoom: f64,
    /// Seconds a critically damped spring needs to settle on a new face position; 0 = none.
    pub smoothing: f64,
    /// Samples below this confidence are ignored (the last good position is held).
    pub min_confidence: f64,
    /// Fastest the crop centre may travel, in frame widths (heights) per second.
    pub max_speed: f64,
    /// The crop centre sits this fraction of the crop height BELOW the face centre, leaving
    /// headroom above the head.
    pub headroom: f64,
}

impl Default for ReframeOpts {
    fn default() -> Self {
        Self { crop_aspect: 1.0, zoom: 1.0, smoothing: 0.4, min_confidence: 0.5, max_speed: 1.0, headroom: 0.1 }
    }
}

impl ReframeOpts {
    /// Replace non-finite values by the defaults and clamp the rest into range.
    fn sane(&self) -> Self {
        let d = Self::default();
        let f = |v: f64, d: f64| if v.is_finite() { v } else { d };
        Self {
            crop_aspect: if self.crop_aspect.is_finite() && self.crop_aspect > 0.0 { self.crop_aspect } else { d.crop_aspect },
            zoom: clamp(f(self.zoom, d.zoom), ZOOM_MIN, ZOOM_MAX),
            smoothing: f(self.smoothing, d.smoothing).max(0.0),
            min_confidence: clamp(f(self.min_confidence, d.min_confidence), 0.0, 1.0),
            max_speed: f(self.max_speed, d.max_speed).max(1e-6),
            headroom: clamp(f(self.headroom, d.headroom), -0.5, 0.5),
        }
    }
}

/// A crop of the camera image in source pixels, top-left origin, always inside the frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CropRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// The whole smoothed path of a track, computed once; [`Reframer::at`] is then O(1) and a pure
/// function of `(track, source size, options, t)`.
#[derive(Debug, Clone)]
pub struct Reframer {
    /// Crop centre per [`STEP`], normalised to the frame.
    path: Vec<[f64; 2]>,
    /// Crop size normalised to the frame.
    size: [f64; 2],
    source: [f64; 2],
}

impl Reframer {
    pub fn new(track: &FaceTrack, source_w: f64, source_h: f64, opts: &ReframeOpts) -> Self {
        let o = opts.sane();
        if !(source_w.is_finite() && source_h.is_finite() && source_w > 0.0 && source_h > 0.0) {
            return Self { path: vec![[0.5, 0.5]], size: [0.0, 0.0], source: [0.0, 0.0] };
        }
        // largest crop of the requested aspect inside the frame, then divided by the zoom
        let (cw, ch) =
            if source_w / source_h >= o.crop_aspect { (source_h * o.crop_aspect, source_h) } else { (source_w, source_w / o.crop_aspect) };
        let size = [cw / o.zoom / source_w, ch / o.zoom / source_h];
        let range = |k: usize| (size[k] / 2.0, 1.0 - size[k] / 2.0);
        let fit = |c: [f64; 2]| [clamp(c[0], range(0).0, range(0).1), clamp(c[1], range(1).0, range(1).1)];

        let good: Vec<&FaceSample> = track.samples.iter().filter(|s| s.confidence >= o.min_confidence).collect();
        let target_of = |s: &FaceSample| fit([s.x + s.w / 2.0, s.y + s.h / 2.0 + o.headroom * size[1]]);
        let last_t = good.last().map_or(0.0, |s| s.t);
        let seconds = (last_t + 6.0 * o.smoothing + 1.0 + 1.0 / o.max_speed).min(MAX_SECONDS);
        let steps = (seconds / STEP).ceil() as usize;

        let mut path = Vec::with_capacity(steps + 1);
        let mut next = 0; // good samples with t <= current time
        let mut target = good.first().map_or([0.5, 0.5], |s| target_of(s));
        let mut pos = fit(target);
        let mut vel = [0.0f64; 2];
        path.push(pos);
        let omega = if o.smoothing > 0.0 { SETTLE / o.smoothing } else { 0.0 };
        let decay = (-omega * STEP).exp();
        let limit = o.max_speed * STEP;
        for k in 1..=steps {
            let t = k as f64 * STEP;
            while next < good.len() && good[next].t <= t {
                target = target_of(good[next]);
                next += 1;
            }
            for a in 0..2 {
                let mut to = target[a];
                let mut v = 0.0;
                if omega > 0.0 {
                    let d = pos[a] - target[a];
                    let j = vel[a] + omega * d;
                    to = target[a] + (d + j * STEP) * decay;
                    v = (vel[a] - omega * j * STEP) * decay;
                }
                let step = clamp(to - pos[a], -limit, limit);
                vel[a] = if step == to - pos[a] { v } else { step / STEP };
                pos[a] = clamp(pos[a] + step, range(a).0, range(a).1);
            }
            path.push(pos);
        }
        Self { path, size, source: [source_w, source_h] }
    }

    /// The crop at camera time `t` (negative and non-finite times count as 0; past the end of
    /// the track the last position is held).
    pub fn at(&self, t: f64) -> CropRect {
        let t = if t.is_finite() { t.max(0.0) } else { 0.0 };
        let u = t / STEP;
        let i = (u.floor() as usize).min(self.path.len() - 1);
        let j = (i + 1).min(self.path.len() - 1);
        let f = if i == j { 0.0 } else { u - i as f64 };
        let c = [self.path[i][0] + (self.path[j][0] - self.path[i][0]) * f, self.path[i][1] + (self.path[j][1] - self.path[i][1]) * f];
        let [sw, sh] = self.source;
        let (w, h) = (self.size[0] * sw, self.size[1] * sh);
        CropRect { x: clamp(c[0] * sw - w / 2.0, 0.0, (sw - w).max(0.0)), y: clamp(c[1] * sh - h / 2.0, 0.0, (sh - h).max(0.0)), w, h }
    }
}

/// The crop of the camera image (`source_w` x `source_h` pixels) that follows the face at camera
/// time `t`. Builds the whole path each call: use a [`Reframer`] for many frames.
///
/// The centre sits on the face (plus headroom), is clamped so the crop stays inside the frame,
/// and chases the face with a critically damped spring ([`ReframeOpts::smoothing`]) that never
/// overshoots and never moves faster than [`ReframeOpts::max_speed`]. Low-confidence samples and
/// gaps hold the last good position; before the first good sample the crop sits on it already.
pub fn reframe(track: &FaceTrack, t: f64, source_w: f64, source_h: f64, opts: &ReframeOpts) -> CropRect {
    Reframer::new(track, source_w, source_h, opts).at(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: f64 = 1920.0;
    const H: f64 = 1080.0;

    fn s(t: f64, x: f64, y: f64, w: f64, h: f64, confidence: f64) -> FaceSample {
        FaceSample { t, x, y, w, h, confidence }
    }
    /// A face of 0.1 x 0.2 whose centre is at `(cx, cy)`.
    fn face(t: f64, cx: f64, cy: f64, confidence: f64) -> FaceSample {
        s(t, cx - 0.05, cy - 0.1, 0.1, 0.2, confidence)
    }
    fn track(samples: Vec<FaceSample>) -> FaceTrack {
        FaceTrack { fps: 30.0, samples }
    }
    fn opts() -> ReframeOpts {
        ReframeOpts { smoothing: 0.0, max_speed: 100.0, headroom: 0.0, ..ReframeOpts::default() }
    }
    fn centre(c: CropRect) -> (f64, f64) {
        (c.x + c.w / 2.0, c.y + c.h / 2.0)
    }

    #[test]
    fn parse_is_tolerant_of_gaps_and_junk() {
        let t = FaceTrack::parse(
            r#"{"fps":30,"extra":1,"samples":[
                {"t":1.0,"x":0.4,"y":0.3,"w":0.1,"h":0.2,"confidence":0.9},
                {"t":0.0,"x":0.4,"y":0.3,"w":0.1,"h":0.2},
                {"t":0.5,"x":null,"y":0.3,"w":0.1,"h":0.2,"confidence":1},
                {"t":2.5,"x":-0.2,"y":0.9,"w":0.5,"h":0.5,"confidence":7},
                {"t":3.0,"x":0.1,"y":0.1,"w":0,"h":0.2,"confidence":1},
                {"t":4.0,"x":0.1,"y":0.1,"w":0.1,"h":0.1,"confidence":0.8}]}"#,
        )
        .unwrap();
        let ts: Vec<f64> = t.samples.iter().map(|s| s.t).collect();
        assert_eq!(ts, [0.0, 1.0, 2.5, 4.0], "null, zero-size and unordered samples are handled");
        assert_eq!(t.samples[0].confidence, 1.0, "a missing confidence counts as sure");
        let c = &t.samples[2];
        assert_eq!((c.x, c.confidence), (0.0, 1.0), "values are clamped into the frame");
        assert!(c.y + c.h <= 1.0 && c.x + c.w <= 1.0);
        assert_eq!(FaceTrack::parse(r#"{"samples":[]}"#).unwrap(), FaceTrack { fps: 30.0, samples: vec![] });
        assert_eq!(FaceTrack::parse(r#"{"fps":-3,"samples":[]}"#).unwrap().fps, 30.0);
        assert!(FaceTrack::parse("not json").is_err());
        assert!(FaceTrack::parse(r#"{"samples":5}"#).is_err());
        assert!(FaceTrack::parse("").is_err());
    }

    #[test]
    fn json_shape_round_trips() {
        let t = track(vec![s(0.0, 0.25, 0.125, 0.5, 0.25, 0.75)]);
        let j = serde_json::to_string(&t).unwrap();
        assert_eq!(j, r#"{"fps":30.0,"samples":[{"t":0.0,"x":0.25,"y":0.125,"w":0.5,"h":0.25,"confidence":0.75}]}"#);
        assert_eq!(FaceTrack::parse(&j).unwrap(), t);
    }

    #[test]
    fn load_reads_a_file_and_reports_a_missing_one() {
        let dir = std::env::temp_dir().join(format!("ferrocast-facetrack-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("camera_track.json");
        std::fs::write(&f, r#"{"fps":24,"samples":[{"t":0,"x":0.1,"y":0.1,"w":0.2,"h":0.2,"confidence":1}]}"#).unwrap();
        assert_eq!(FaceTrack::load(&f).unwrap().fps, 24.0);
        assert!(FaceTrack::load(&dir.join("nope.json")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn centres_the_face_with_headroom_and_zoom() {
        let t = track(vec![face(0.0, 0.5, 0.45, 1.0)]);
        // zoom 1, square crop of a 16:9 frame: the full height, centred on the face horizontally
        let c = reframe(&t, 0.0, W, H, &opts());
        assert_eq!((c.w, c.h, c.x, c.y), (1080.0, 1080.0, 420.0, 0.0));
        // zoom 2: a 540 square on the face; headroom 0.1 puts the crop centre 10 % of the crop below the face centre
        let o = ReframeOpts { zoom: 2.0, headroom: 0.1, ..opts() };
        let c = reframe(&t, 0.0, W, H, &o);
        assert_eq!((c.w, c.h), (540.0, 540.0));
        assert!((centre(c).0 - 960.0).abs() < 1e-9 && (centre(c).1 - (0.45 * H + 54.0)).abs() < 1e-9, "{c:?}");
        // 16:9 crop of a portrait frame stays inside it
        let c = reframe(&t, 0.0, 1080.0, 1920.0, &ReframeOpts { crop_aspect: 16.0 / 9.0, ..opts() });
        assert_eq!((c.w, c.h), (1080.0, 607.5));
        // zoom is clamped to 1..=3
        assert_eq!(reframe(&t, 0.0, W, H, &ReframeOpts { zoom: 9.0, ..opts() }).w, 360.0);
        assert_eq!(reframe(&t, 0.0, W, H, &ReframeOpts { zoom: 0.2, ..opts() }).w, 1080.0);
    }

    #[test]
    fn the_crop_never_leaves_the_frame() {
        let mut seed = 11u64;
        let mut rnd = || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((seed >> 11) as f64) / ((1u64 << 53) as f64)
        };
        for (w, h) in [(W, H), (1080.0, 1920.0), (640.0, 640.0), (333.0, 777.0)] {
            let samples = (0..120).map(|i| face(i as f64 / 30.0, rnd() * 1.4 - 0.2, rnd() * 1.4 - 0.2, rnd())).collect();
            let t = track(samples);
            for zoom in [1.0, 1.7, 3.0] {
                for aspect in [1.0, 16.0 / 9.0, 9.0 / 16.0] {
                    let o =
                        ReframeOpts { zoom, crop_aspect: aspect, smoothing: 0.3, max_speed: 1.0, headroom: 0.15, ..ReframeOpts::default() };
                    let r = Reframer::new(&t, w, h, &o);
                    for k in 0..200 {
                        let c = r.at(k as f64 * 0.03 - 0.5);
                        let tag = format!("{w}x{h} z{zoom} a{aspect} {c:?}");
                        assert!(c.x >= 0.0 && c.y >= 0.0 && c.x + c.w <= w + 1e-9 && c.y + c.h <= h + 1e-9, "{tag}");
                        assert!((c.w / c.h - aspect).abs() < 1e-9, "{tag}");
                    }
                }
            }
        }
    }

    #[test]
    fn results_do_not_depend_on_call_order_or_the_reframer_instance() {
        let t = track((0..90).map(|i| face(i as f64 / 30.0, 0.3 + 0.4 * ((i as f64) / 15.0).sin().abs(), 0.5, 1.0)).collect());
        let o = ReframeOpts { zoom: 2.0, smoothing: 0.4, max_speed: 0.8, ..ReframeOpts::default() };
        let times = [2.9, 0.0, 1.234, 0.31, 2.0, 1.234, 5.0, -1.0];
        let forward: Vec<CropRect> = times.iter().map(|&x| reframe(&t, x, W, H, &o)).collect();
        let r = Reframer::new(&t, W, H, &o);
        let mut order: Vec<usize> = (0..times.len()).rev().collect();
        order.rotate_left(3);
        for i in order {
            assert_eq!(r.at(times[i]), forward[i], "t = {}", times[i]);
        }
        assert_eq!(forward[2], forward[5]);
        assert_eq!(forward[7], forward[1], "negative time is time 0");
        assert_eq!(reframe(&t, f64::NAN, W, H, &o), forward[1]);
    }

    #[test]
    fn holds_the_last_good_position_through_gaps_and_low_confidence() {
        let t = track(vec![
            face(0.0, 0.3, 0.5, 1.0),
            face(1.0, 0.3, 0.5, 0.9),
            face(1.5, 0.8, 0.5, 0.2),
            face(2.0, 0.85, 0.5, 0.0),
            face(8.0, 0.8, 0.5, 0.2),
        ]);
        let o = ReframeOpts { zoom: 2.0, ..opts() };
        let r = Reframer::new(&t, W, H, &o);
        let at_one = r.at(1.0);
        for x in [1.2, 1.6, 2.0, 5.0, 7.9, 12.0] {
            assert_eq!(r.at(x), at_one, "t = {x}");
        }
        // a good sample after the gap moves it again
        let t2 = track(vec![face(0.0, 0.3, 0.5, 1.0), face(6.0, 0.7, 0.5, 1.0)]);
        let r = Reframer::new(&t2, W, H, &o);
        assert_eq!(r.at(5.9), r.at(0.0));
        assert!(r.at(6.0).x > r.at(0.0).x + 100.0);
    }

    #[test]
    fn before_the_first_good_sample_and_without_any_it_does_not_swoop() {
        let o = ReframeOpts { zoom: 2.0, smoothing: 0.5, ..opts() };
        let t = track(vec![face(3.0, 0.7, 0.4, 1.0)]);
        let r = Reframer::new(&t, W, H, &o);
        assert_eq!(r.at(0.0), r.at(3.0), "starts on the first good face instead of travelling to it");
        let none = Reframer::new(&track(vec![]), W, H, &o);
        let c = none.at(1.0);
        assert_eq!(centre(c), (960.0, 540.0));
        let low = Reframer::new(&track(vec![face(0.0, 0.9, 0.9, 0.1)]), W, H, &o);
        assert_eq!(centre(low.at(0.0)), (960.0, 540.0), "only low-confidence faces: centred");
    }

    #[test]
    fn never_moves_faster_than_the_configured_speed() {
        let t = track(vec![face(0.0, 0.2, 0.5, 1.0), face(1.0, 0.8, 0.5, 1.0)]);
        for (smoothing, max_speed) in [(0.0, 0.5), (0.3, 0.5), (0.3, 0.2), (0.05, 2.0)] {
            let o = ReframeOpts { zoom: 3.0, smoothing, max_speed, ..opts() };
            let r = Reframer::new(&t, W, H, &o);
            let dt = 0.01;
            let mut prev = r.at(0.0);
            let mut travelled = 0.0;
            for k in 1..=1000 {
                let c = r.at(k as f64 * dt);
                let step = (c.x - prev.x).abs() / W;
                assert!(step <= max_speed * dt * 1.01 + 1e-9, "smoothing {smoothing} speed {max_speed}: {step} at {k}");
                travelled += step;
                prev = c;
            }
            assert!(travelled > 0.0, "it does follow");
        }
    }

    #[test]
    fn smoothing_is_a_critically_damped_approach_without_overshoot() {
        let t = track(vec![face(0.0, 0.3, 0.5, 1.0), face(1.0, 0.7, 0.5, 1.0)]);
        let o = ReframeOpts { zoom: 3.0, smoothing: 0.5, max_speed: 100.0, ..opts() };
        let r = Reframer::new(&t, W, H, &o);
        let (from, to) = (r.at(0.9).x, 0.7 * W - 180.0);
        assert!((from - (0.3 * W - 180.0)).abs() < 1e-6);
        let mut prev = from;
        for k in 0..=300 {
            let x = r.at(1.0 + k as f64 * 0.01).x;
            assert!(x >= prev - 1e-9 && x <= to + 1e-6, "monotone, no overshoot: {x}");
            prev = x;
        }
        assert!((to - r.at(1.5).x) / (to - from) < 0.005, "settled after the smoothing time");
        assert!(r.at(1.01).x - from < 0.05 * (to - from), "eases in from rest");
        // no smoothing: jumps straight to the new face on the next grid step
        let o0 = ReframeOpts { smoothing: 0.0, ..o };
        assert!((Reframer::new(&t, W, H, &o0).at(1.05).x - to).abs() < 1e-6);
    }

    #[test]
    fn degenerate_sizes_give_an_empty_crop() {
        let t = track(vec![face(0.0, 0.5, 0.5, 1.0)]);
        for (w, h) in [(0.0, H), (W, 0.0), (f64::NAN, H), (-5.0, 5.0)] {
            let c = reframe(&t, 0.0, w, h, &ReframeOpts::default());
            assert_eq!((c.w, c.h), (0.0, 0.0), "{w}x{h}");
        }
        // junk options fall back to the defaults instead of producing NaN
        let o = ReframeOpts {
            crop_aspect: f64::NAN,
            zoom: f64::NAN,
            smoothing: f64::NAN,
            max_speed: f64::NAN,
            min_confidence: f64::NAN,
            headroom: f64::NAN,
        };
        let c = reframe(&t, 0.5, W, H, &o);
        assert!([c.x, c.y, c.w, c.h].iter().all(|v| v.is_finite()) && c.w > 0.0);
    }
}
