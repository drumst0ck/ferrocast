//! Cursor maths for the editor (pure, deterministic): path smoothing, idle
//! hide, click ring envelope and motion-blur parameters.
//!
//! * `smooth` resamples the 60 Hz log to `opts.fps`, then runs two cascaded
//!   first-order low-pass stages (an exactly critically damped follower,
//!   discretised in closed form for piecewise-linear input). Every update is a
//!   convex combination of earlier values, so the path never overshoots and a
//!   stationary target is approached monotonically.
//! * Positions are in screen pixels, times in seconds.

use super::curves::{clamp, ease_out_cubic, lerp};
use super::events::{Click, CursorSample};

/// One point of a cursor path.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    pub t: f64,
    pub x: f64,
    pub y: f64,
}

impl From<&CursorSample> for Sample {
    fn from(c: &CursorSample) -> Self {
        Sample { t: c.0, x: c.1, y: c.2 }
    }
}

/// Convert an `events.json` cursor log.
pub fn samples_from(log: &[CursorSample]) -> Vec<Sample> {
    log.iter().map(Sample::from).collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct CursorOpts {
    /// 0..1, mapped to a total time constant of 0..120 ms.
    pub strength: f64,
    /// Output frame rate the path is resampled to.
    pub fps: f64,
    /// Seconds without movement before the cursor fades out.
    pub hide_idle_after: f64,
    pub fade_out: f64,
    pub fade_in: f64,
    /// Movement (px) from the span anchor that still counts as standing still.
    pub idle_epsilon: f64,
    pub ring_secs: f64,
    pub ring_r0: f64,
    pub ring_r1: f64,
    /// Blur is off below this speed (px/s).
    pub blur_min_speed: f64,
    /// Speed (px/s) at which the sample count reaches its maximum.
    pub blur_full_speed: f64,
    pub blur_max_samples: u32,
    /// Longest blur streak in px.
    pub blur_max_length: f64,
}

impl Default for CursorOpts {
    fn default() -> Self {
        Self {
            strength: 0.5,
            fps: 60.0,
            hide_idle_after: 3.0,
            fade_out: 0.3,
            fade_in: 0.15,
            idle_epsilon: 1.0,
            ring_secs: 0.45,
            ring_r0: 8.0,
            ring_r1: 36.0,
            blur_min_speed: 200.0,
            blur_full_speed: 3000.0,
            blur_max_samples: 8,
            blur_max_length: 64.0,
        }
    }
}

/// Click ring state at one instant.
#[derive(Debug, Clone, PartialEq)]
pub struct Ring {
    pub x: f64,
    pub y: f64,
    pub radius: f64,
    pub alpha: f64,
}

/// Motion blur request for one output frame: `samples` sub-frames spread over
/// `shutter` (fraction of the frame, 0.5 = 180°) along `dir`, `length` px long.
#[derive(Debug, Clone, PartialEq)]
pub struct BlurParams {
    pub samples: u32,
    pub shutter: f64,
    pub dir: [f64; 2],
    pub length: f64,
}

impl BlurParams {
    pub const NONE: BlurParams = BlurParams { samples: 0, shutter: 0.0, dir: [0.0, 0.0], length: 0.0 };
}

/// Smoothed path, resampled to `opts.fps`. First and last samples are exact.
pub fn smooth(samples: &[Sample], opts: &CursorOpts) -> Vec<Sample> {
    if samples.len() < 2 {
        return samples.to_vec();
    }
    let grid = resample(samples, opts.fps);
    let tau = clamp(opts.strength, 0.0, 1.0) * 0.120 / 2.0; // per stage
    let first = &grid[0];
    let mut out = Vec::with_capacity(grid.len());
    out.push(first.clone());
    let (mut a, mut b) = ([first.x, first.y], [first.x, first.y]);
    for w in grid.windows(2) {
        let h = w[1].t - w[0].t;
        let (u0, u1) = ([w[0].x, w[0].y], [w[1].x, w[1].y]);
        let a_new = [stage(a[0], u0[0], u1[0], h, tau), stage(a[1], u0[1], u1[1], h, tau)];
        b = [stage(b[0], a[0], a_new[0], h, tau), stage(b[1], a[1], a_new[1], h, tau)];
        a = a_new;
        out.push(Sample { t: w[1].t, x: b[0], y: b[1] });
    }
    // The follower lags; land exactly on the final recorded position.
    let (last, raw) = (out.last_mut().expect("non-empty"), grid.last().expect("non-empty"));
    last.x = raw.x;
    last.y = raw.y;
    out
}

/// One first-order low-pass step over `h` seconds for an input moving
/// linearly from `u0` to `u1` (exact solution; coefficients are all >= 0).
fn stage(y: f64, u0: f64, u1: f64, h: f64, tau: f64) -> f64 {
    if tau <= 0.0 || h <= 0.0 {
        return u1;
    }
    let e = (-h / tau).exp();
    let s = (u1 - u0) / h;
    u1 - s * tau + (y - u0 + s * tau) * e
}

fn resample(samples: &[Sample], fps: f64) -> Vec<Sample> {
    let fps = if fps.is_finite() && fps > 0.0 { fps } else { 60.0 };
    let (t0, t1) = (samples[0].t, samples[samples.len() - 1].t);
    let mut out = vec![];
    let mut k = 0u64;
    loop {
        let t = t0 + k as f64 / fps;
        if t >= t1 - 1e-9 {
            break;
        }
        let (x, y) = cursor_at(samples, t).expect("non-empty");
        out.push(Sample { t, x, y });
        k += 1;
    }
    let l = &samples[samples.len() - 1];
    out.push(l.clone());
    out
}

/// Position at time `t` (clamped to the path ends); `None` for an empty path.
pub fn cursor_at(path: &[Sample], t: f64) -> Option<(f64, f64)> {
    let (first, last) = (path.first()?, path.last()?);
    if t <= first.t {
        return Some((first.x, first.y));
    }
    if t >= last.t {
        return Some((last.x, last.y));
    }
    let i = path.partition_point(|s| s.t <= t); // 1..len
    let (a, b) = (&path[i - 1], &path[i]);
    let span = b.t - a.t;
    let f = if span > 0.0 { (t - a.t) / span } else { 1.0 };
    Some((lerp(a.x, b.x, f), lerp(a.y, b.y, f)))
}

/// `(start, end)` spans lasting at least `idle_secs` where the cursor stayed
/// within 1 px of where it stopped. `end` is the last still sample.
pub fn idle_spans(samples: &[Sample], idle_secs: f64) -> Vec<(f64, f64)> {
    let eps = CursorOpts::default().idle_epsilon;
    let mut spans = vec![];
    let mut i = 0;
    while i < samples.len() {
        let a = &samples[i];
        let mut j = i;
        while j + 1 < samples.len() && (samples[j + 1].x - a.x).hypot(samples[j + 1].y - a.y) <= eps {
            j += 1;
        }
        if j > i && samples[j].t - a.t >= idle_secs {
            spans.push((a.t, samples[j].t));
            i = j + 1;
        } else {
            i += 1;
        }
    }
    spans
}

/// Cursor opacity 0..1 at `t`: fades out `hide_idle_after` seconds into an
/// idle span, stays hidden until the span ends, then fades back in.
pub fn visible_at(spans: &[(f64, f64)], t: f64, opts: &CursorOpts) -> f64 {
    for &(s, e) in spans {
        let h = s + opts.hide_idle_after;
        if h >= e || t < h {
            continue;
        }
        if t < e {
            return 1.0 - unit((t - h) / opts.fade_out);
        }
        if t < e + opts.fade_in {
            return unit((t - e) / opts.fade_in);
        }
    }
    1.0
}

fn unit(x: f64) -> f64 {
    super::curves::smoothstep(if x.is_finite() { x } else { 1.0 })
}

/// Ring for the latest press at or before `t`, `None` when none is active.
pub fn click_ring(t: f64, clicks: &[Click], opts: &CursorOpts) -> Option<Ring> {
    let c = clicks.iter().rev().find(|c| c.t <= t && c.is_down())?;
    let p = (t - c.t) / opts.ring_secs;
    if !(0.0..1.0).contains(&p) {
        return None;
    }
    Some(Ring { x: c.x, y: c.y, radius: lerp(opts.ring_r0, opts.ring_r1, ease_out_cubic(p)), alpha: 1.0 - p })
}

/// Blur from cursor and camera screen-space velocity (px/s): the faster of
/// the two sets direction and length; zero below `blur_min_speed`.
pub fn blur_params(cursor_v: [f64; 2], camera_v: [f64; 2], opts: &CursorOpts) -> BlurParams {
    if !cursor_v.iter().chain(&camera_v).all(|v| v.is_finite()) {
        return BlurParams::NONE;
    }
    let (sc, sk) = (cursor_v[0].hypot(cursor_v[1]), camera_v[0].hypot(camera_v[1]));
    let (v, speed) = if sk > sc { (camera_v, sk) } else { (cursor_v, sc) };
    if speed < opts.blur_min_speed || speed <= 0.0 {
        return BlurParams::NONE;
    }
    let span = (opts.blur_full_speed - opts.blur_min_speed).max(1e-9);
    let f = clamp((speed - opts.blur_min_speed) / span, 0.0, 1.0);
    let samples = (lerp(2.0, opts.blur_max_samples as f64, f).round() as u32).clamp(1, opts.blur_max_samples.max(1));
    const SHUTTER: f64 = 0.5; // 180 degrees
    let fps = if opts.fps > 0.0 { opts.fps } else { 60.0 };
    BlurParams { samples, shutter: SHUTTER, dir: [v[0] / speed, v[1] / speed], length: (speed * SHUTTER / fps).min(opts.blur_max_length) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(n: usize, hz: f64, v: f64) -> Vec<Sample> {
        (0..n).map(|i| Sample { t: i as f64 / hz, x: i as f64 / hz * v, y: 100.0 }).collect()
    }

    fn opts(strength: f64) -> CursorOpts {
        CursorOpts { strength, ..CursorOpts::default() }
    }

    #[test]
    fn defaults_follow_the_plan() {
        let o = CursorOpts::default();
        assert_eq!(o.strength, 0.5);
        assert_eq!(o.hide_idle_after, 3.0);
        assert_eq!(o.blur_min_speed, 200.0);
        assert_eq!(o.blur_max_samples, 8);
        assert_eq!(o.fps, 60.0);
    }

    #[test]
    fn smoothing_is_deterministic() {
        let s: Vec<Sample> =
            (0..120).map(|i| Sample { t: i as f64 / 60.0, x: ((i * 37) % 91) as f64, y: ((i * 11) % 53) as f64 }).collect();
        assert_eq!(smooth(&s, &CursorOpts::default()), smooth(&s, &CursorOpts::default()));
    }

    #[test]
    fn empty_and_single_inputs() {
        assert!(smooth(&[], &CursorOpts::default()).is_empty());
        assert_eq!(cursor_at(&[], 1.0), None);
        let one = [Sample { t: 2.0, x: 5.0, y: 7.0 }];
        assert_eq!(smooth(&one, &CursorOpts::default()), one.to_vec());
        assert_eq!(cursor_at(&one, 0.0), Some((5.0, 7.0)));
        assert_eq!(cursor_at(&one, 9.0), Some((5.0, 7.0)));
        assert!(idle_spans(&[], 1.0).is_empty());
        assert!(idle_spans(&one, 1.0).is_empty());
        assert_eq!(visible_at(&[], 3.0, &CursorOpts::default()), 1.0);
    }

    #[test]
    fn passes_through_first_and_last_samples() {
        let s = line(60, 60.0, 600.0);
        let p = smooth(&s, &CursorOpts::default());
        assert_eq!((p[0].t, p[0].x, p[0].y), (s[0].t, s[0].x, s[0].y));
        let (a, b) = (p.last().unwrap(), s.last().unwrap());
        assert_eq!((a.t, a.x, a.y), (b.t, b.x, b.y));
    }

    #[test]
    fn straight_path_never_overshoots_and_stays_monotone() {
        for strength in [0.1, 0.5, 1.0] {
            let s = line(90, 60.0, 900.0);
            let end = s.last().unwrap().x;
            let p = smooth(&s, &opts(strength));
            for w in p.windows(2) {
                assert!(w[1].x >= w[0].x - 1e-9, "monotone");
            }
            assert!(p.iter().all(|q| q.x <= end + 1e-9 && q.x >= -1e-9 && (q.y - 100.0).abs() < 1e-9));
        }
    }

    #[test]
    fn smoothed_lags_the_raw_path_and_strength_zero_is_identity() {
        let s = line(60, 60.0, 600.0);
        let p = smooth(&s, &opts(0.5));
        assert!(p[30].x < s[30].x);
        let id = smooth(&s, &opts(0.0));
        for (a, b) in id.iter().zip(&s) {
            assert!((a.x - b.x).abs() < 1e-9);
        }
    }

    #[test]
    fn converges_monotonically_to_a_stationary_target() {
        let mut s: Vec<Sample> = (0..30).map(|i| Sample { t: i as f64 / 60.0, x: i as f64 * 10.0, y: 0.0 }).collect();
        for i in 30..240 {
            s.push(Sample { t: i as f64 / 60.0, x: 290.0, y: 0.0 });
        }
        let p = smooth(&s, &opts(1.0));
        let mut prev = f64::MAX;
        for q in p.iter().skip(30).take(150) {
            let gap = 290.0 - q.x;
            assert!(gap >= -1e-9 && gap <= prev + 1e-9);
            prev = gap;
        }
        assert!(290.0 - p[150].x < 0.5);
    }

    #[test]
    fn frame_rate_independent_on_a_straight_path() {
        let a = smooth(&line(121, 60.0, 500.0), &opts(0.7));
        let b = smooth(&line(61, 30.0, 500.0), &opts(0.7));
        for t in [0.5, 1.0, 1.5] {
            let (xa, xb) = (cursor_at(&a, t).unwrap().0, cursor_at(&b, t).unwrap().0);
            assert!((xa - xb).abs() < 1.5, "{xa} vs {xb}");
        }
    }

    #[test]
    fn cursor_at_interpolates_and_clamps() {
        let p = vec![Sample { t: 1.0, x: 0.0, y: 0.0 }, Sample { t: 2.0, x: 10.0, y: 20.0 }, Sample { t: 4.0, x: 30.0, y: 20.0 }];
        assert_eq!(cursor_at(&p, 0.0), Some((0.0, 0.0)));
        assert_eq!(cursor_at(&p, 1.5), Some((5.0, 10.0)));
        assert_eq!(cursor_at(&p, 2.0), Some((10.0, 20.0)));
        assert_eq!(cursor_at(&p, 3.0), Some((20.0, 20.0)));
        assert_eq!(cursor_at(&p, 9.0), Some((30.0, 20.0)));
    }

    fn still_then_move() -> Vec<Sample> {
        // moves until 1.0 s, still until 6.0 s, moves again.
        let mut v = vec![];
        for i in 0..=60 {
            v.push(Sample { t: i as f64 / 60.0, x: i as f64 * 5.0, y: 0.0 });
        }
        for i in 61..=360 {
            v.push(Sample { t: i as f64 / 60.0, x: 300.0, y: 0.0 });
        }
        for i in 361..=400 {
            v.push(Sample { t: i as f64 / 60.0, x: 300.0 + (i - 360) as f64 * 5.0, y: 0.0 });
        }
        v
    }

    #[test]
    fn idle_span_boundaries() {
        let s = still_then_move();
        let spans = idle_spans(&s, 3.0);
        assert_eq!(spans.len(), 1);
        assert!((spans[0].0 - 1.0).abs() < 1e-9, "{:?}", spans);
        assert!((spans[0].1 - 6.0).abs() < 1e-9, "{:?}", spans);
        assert!(idle_spans(&s, 6.0).is_empty());
        assert!(idle_spans(&line(600, 60.0, 100.0), 1.0).is_empty());
    }

    #[test]
    fn idle_span_at_the_end_of_the_log() {
        let mut s = line(30, 60.0, 600.0);
        let last = s.last().unwrap().clone();
        for i in 30..400 {
            s.push(Sample { t: i as f64 / 60.0, ..last.clone() });
        }
        let spans = idle_spans(&s, 3.0);
        assert_eq!(spans.len(), 1);
        assert!((spans[0].1 - s.last().unwrap().t).abs() < 1e-9);
    }

    #[test]
    fn visibility_fades_out_stays_hidden_and_fades_in() {
        let o = CursorOpts::default();
        let spans = vec![(1.0, 6.0)];
        assert_eq!(visible_at(&spans, 0.5, &o), 1.0);
        assert_eq!(visible_at(&spans, 3.9, &o), 1.0); // 1.0 + 3.0 = hide starts at 4.0
        let mid = visible_at(&spans, 4.0 + o.fade_out / 2.0, &o);
        assert!(mid > 0.0 && mid < 1.0);
        assert_eq!(visible_at(&spans, 5.0, &o), 0.0);
        assert_eq!(visible_at(&spans, 6.0, &o), 0.0);
        let back = visible_at(&spans, 6.0 + o.fade_in / 2.0, &o);
        assert!(back > 0.0 && back < 1.0);
        assert_eq!(visible_at(&spans, 6.0 + o.fade_in, &o), 1.0);
        assert_eq!(visible_at(&spans, 20.0, &o), 1.0);
    }

    #[test]
    fn click_ring_envelope() {
        let o = CursorOpts::default();
        let clicks = vec![
            Click { t: 1.0, x: 10.0, y: 20.0, button: "left".into(), down: true, up: false },
            Click { t: 1.1, x: 10.0, y: 20.0, button: "left".into(), down: false, up: true },
        ];
        assert!(click_ring(0.9, &clicks, &o).is_none());
        let a = click_ring(1.0, &clicks, &o).unwrap();
        assert_eq!((a.x, a.y), (10.0, 20.0));
        assert!((a.radius - o.ring_r0).abs() < 1e-9 && (a.alpha - 1.0).abs() < 1e-9);
        let b = click_ring(1.0 + o.ring_secs / 2.0, &clicks, &o).unwrap();
        assert!(b.radius > a.radius && b.alpha < a.alpha && b.alpha > 0.0);
        assert!(click_ring(1.0 + o.ring_secs + 1e-6, &clicks, &o).is_none());
        // a release alone starts no ring
        assert!(click_ring(1.12, &clicks[1..], &o).is_none());
    }

    #[test]
    fn blur_is_zero_at_rest_and_below_threshold() {
        let o = CursorOpts::default();
        assert_eq!(blur_params([0.0, 0.0], [0.0, 0.0], &o), BlurParams::NONE);
        assert_eq!(blur_params([150.0, 0.0], [0.0, 100.0], &o), BlurParams::NONE);
        assert_eq!(blur_params([f64::NAN, 0.0], [0.0, 0.0], &o), BlurParams::NONE);
    }

    #[test]
    fn blur_grows_then_clamps_at_high_speed() {
        let o = CursorOpts::default();
        let slow = blur_params([400.0, 0.0], [0.0, 0.0], &o);
        let fast = blur_params([2000.0, 0.0], [0.0, 0.0], &o);
        let huge = blur_params([1.0e6, 0.0], [0.0, 0.0], &o);
        assert!(slow.samples >= 2 && slow.samples < fast.samples);
        assert_eq!(huge.samples, o.blur_max_samples);
        assert!(slow.length < fast.length);
        assert_eq!(huge.length, o.blur_max_length);
        assert_eq!(huge.shutter, 0.5);
        assert_eq!(slow.dir, [1.0, 0.0]);
        assert!((huge.dir[0].powi(2) + huge.dir[1].powi(2) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn blur_uses_the_faster_of_cursor_and_camera() {
        let o = CursorOpts::default();
        let p = blur_params([300.0, 0.0], [0.0, -1500.0], &o);
        assert_eq!(p.dir, [0.0, -1.0]);
        assert_eq!(p, blur_params([0.0, 0.0], [0.0, -1500.0], &o));
    }
}
