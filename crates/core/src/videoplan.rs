//! State of the video trim viewer (Tangerine trimVideo): a cursor over the timeline, a start and
//! an end marker, stepping by frame or by second. Pure, so every rule is testable without a window.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Trim {
    pub duration: f64,
    pub fps: f64,
    pub cursor: f64,
    pub start: f64,
    pub end: f64,
}

/// Smallest clip the viewer allows, in seconds.
pub const MIN_CLIP: f64 = 0.04;

impl Trim {
    /// A viewer over `duration` seconds at `fps` frames per second; the whole clip is selected.
    /// `None` when the numbers cannot describe a video.
    pub fn new(duration: f64, fps: f64) -> Option<Trim> {
        (duration.is_finite() && duration > MIN_CLIP && fps.is_finite() && fps > 0.0 && fps <= 1000.0).then_some(Trim {
            duration,
            fps,
            cursor: 0.0,
            start: 0.0,
            end: duration,
        })
    }

    pub fn frame(&self) -> f64 {
        1.0 / self.fps
    }

    /// Move the cursor by `frames` frames (negative goes back), staying on the timeline.
    pub fn step_frames(&mut self, frames: i64) {
        self.seek(self.cursor + frames as f64 * self.frame());
    }

    pub fn step_secs(&mut self, secs: f64) {
        self.seek(self.cursor + secs);
    }

    pub fn seek(&mut self, t: f64) {
        // the last frame starts one frame before the end
        let last = (self.duration - self.frame()).max(0.0);
        self.cursor = if t.is_finite() { t.clamp(0.0, last) } else { self.cursor };
    }

    /// Cursor snapped to the frame grid, so markers always land on a frame.
    fn snapped(&self) -> f64 {
        ((self.cursor * self.fps).round() / self.fps).clamp(0.0, self.duration)
    }

    /// Put the start marker at the cursor; the end moves out of the way when needed.
    pub fn mark_start(&mut self) {
        let t = self.snapped().min(self.duration - MIN_CLIP);
        self.start = t;
        if self.end < t + MIN_CLIP {
            self.end = (t + MIN_CLIP).min(self.duration);
        }
    }

    /// Put the end marker at the cursor (the end of that frame is not included).
    pub fn mark_end(&mut self) {
        let t = self.snapped().max(MIN_CLIP);
        self.end = t;
        if self.start > t - MIN_CLIP {
            self.start = (t - MIN_CLIP).max(0.0);
        }
    }

    /// Exact start (typed seconds): kept inside the clip and at least `MIN_CLIP` before the end.
    pub fn set_start(&mut self, s: f64) {
        if s.is_finite() {
            self.start = s.clamp(0.0, (self.end - MIN_CLIP).max(0.0));
        }
    }

    /// Exact end (typed seconds): kept inside the clip and at least `MIN_CLIP` after the start.
    pub fn set_end(&mut self, e: f64) {
        if e.is_finite() {
            self.end = e.clamp((self.start + MIN_CLIP).min(self.duration), self.duration);
        }
    }

    /// Set both markers at once, clamped to the clip. A range shorter than `MIN_CLIP` is refused.
    /// True when the markers actually moved.
    pub fn set_range(&mut self, a: f64, b: f64) -> bool {
        if !a.is_finite() || !b.is_finite() {
            return false;
        }
        let (a, b) = (a.clamp(0.0, self.duration), b.clamp(0.0, self.duration));
        if b - a < MIN_CLIP || ((a - self.start).abs() < 1e-6 && (b - self.end).abs() < 1e-6) {
            return false;
        }
        (self.start, self.end) = (a, b);
        true
    }

    pub fn is_whole(&self) -> bool {
        self.start <= 0.0 && (self.duration - self.end).abs() < 1e-6
    }

    /// Length of the selected clip in seconds.
    pub fn clip(&self) -> f64 {
        (self.end - self.start).max(0.0)
    }

    /// `m:ss.ff` with the frame number inside the second.
    pub fn label(&self, t: f64) -> String {
        let total = t.max(0.0);
        let secs = total.floor() as u64;
        let ff = ((total - total.floor()) * self.fps).round() as u64;
        let (ff, secs) = if ff as f64 >= self.fps.round() { (0, secs + 1) } else { (ff, secs) };
        format!("{}:{:02}.{:02}", secs / 60, secs % 60, ff)
    }
}

/// Playback speeds the speed window offers (the tool accepts 0.25..=4).
pub const SPEED_MIN: f64 = 0.25;
pub const SPEED_MAX: f64 = 4.0;

/// Length of a clip of `duration` seconds played at `factor`x; `None` for a factor outside the
/// supported range or a duration that is not a number.
pub fn sped_duration(duration: f64, factor: f64) -> Option<f64> {
    (duration.is_finite() && duration > 0.0 && factor.is_finite() && (SPEED_MIN..=SPEED_MAX).contains(&factor)).then(|| duration / factor)
}

/// Shortest piece a split may produce, in seconds.
pub const MIN_PIECE: f64 = 0.05;

/// `n` pieces of equal length covering `0..duration`, contiguous, the last one ending exactly at
/// `duration`. Empty when the numbers cannot describe a split (or a piece would be too short).
pub fn equal_parts(duration: f64, n: usize) -> Vec<(f64, f64)> {
    if !duration.is_finite() || duration <= 0.0 || n == 0 || duration / (n as f64) < MIN_PIECE {
        return vec![];
    }
    (0..n)
        .map(|i| {
            let a = duration * i as f64 / n as f64;
            let b = if i + 1 == n { duration } else { duration * (i + 1) as f64 / n as f64 };
            (a, b)
        })
        .collect()
}

/// Contiguous ranges between the given cut points. Points that are not numbers or fall outside
/// `(0, duration)` are ignored, the rest is sorted and deduplicated, and a point that would leave
/// a piece shorter than [`MIN_PIECE`] is dropped, so no content is ever lost. Empty when
/// `duration` is not usable.
pub fn points_to_ranges(duration: f64, points: &[f64]) -> Vec<(f64, f64)> {
    if !duration.is_finite() || duration <= 0.0 {
        return vec![];
    }
    let mut pts: Vec<f64> = points.iter().copied().filter(|p| p.is_finite() && *p > 0.0 && *p < duration).collect();
    pts.sort_by(|a, b| a.total_cmp(b));
    let mut kept: Vec<f64> = vec![];
    let mut last = 0.0;
    for p in pts {
        if p - last >= MIN_PIECE && duration - p >= MIN_PIECE {
            kept.push(p);
            last = p;
        }
    }
    let mut out = Vec::with_capacity(kept.len() + 1);
    let mut start = 0.0;
    for p in kept {
        out.push((start, p));
        start = p;
    }
    out.push((start, duration));
    out
}

/// The ranges a split will produce: the cut points when there are any, otherwise `parts` equal
/// pieces. Empty when the numbers cannot describe a split; a single range means "nothing to cut".
pub fn split_ranges(duration: f64, points: &[f64], parts: usize) -> Vec<(f64, f64)> {
    if points.is_empty() {
        equal_parts(duration, parts)
    } else {
        points_to_ranges(duration, points)
    }
}

impl Trim {
    /// Add a cut point at the cursor, snapped to the frame grid. Returns false (and changes
    /// nothing) when that frame already has a point or the cursor is at either end of the clip.
    /// The list stays sorted.
    pub fn add_cut(&self, points: &mut Vec<f64>) -> bool {
        let t = self.snapped();
        if t <= 0.0 || t >= self.duration || points.iter().any(|p| (p - t).abs() < self.frame() / 2.0) {
            return false;
        }
        points.push(t);
        points.sort_by(|a, b| a.total_cmp(b));
        true
    }
}

/// Frame rate from ffprobe's `r_frame_rate` text such as `30000/1001` or `25`.
pub fn parse_rate(s: &str) -> Option<f64> {
    let s = s.trim();
    let v = match s.split_once('/') {
        Some((n, d)) => {
            let (n, d): (f64, f64) = (n.trim().parse().ok()?, d.trim().parse().ok()?);
            if d == 0.0 {
                return None;
            }
            n / d
        }
        None => s.parse().ok()?,
    };
    (v.is_finite() && v > 0.0 && v <= 1000.0).then_some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t() -> Trim {
        Trim::new(10.0, 25.0).unwrap()
    }

    #[test]
    fn rejects_numbers_that_are_not_a_video() {
        for (d, f) in [(0.0, 25.0), (-1.0, 25.0), (f64::NAN, 25.0), (10.0, 0.0), (10.0, f64::INFINITY), (0.01, 25.0), (10.0, 5000.0)] {
            assert!(Trim::new(d, f).is_none(), "{d} {f}");
        }
        let v = t();
        assert!(v.is_whole() && v.clip() == 10.0 && v.cursor == 0.0);
    }

    #[test]
    fn stepping_stays_on_the_timeline() {
        let mut v = t();
        v.step_frames(-3);
        assert_eq!(v.cursor, 0.0);
        v.step_frames(5);
        assert!((v.cursor - 0.2).abs() < 1e-9, "5 frames at 25 fps");
        v.step_secs(2.0);
        assert!((v.cursor - 2.2).abs() < 1e-9);
        v.step_secs(100.0);
        assert!((v.cursor - 9.96).abs() < 1e-9, "the last frame starts one frame before the end");
        v.seek(f64::NAN);
        assert!((v.cursor - 9.96).abs() < 1e-9, "NaN keeps the cursor");
        v.step_frames(i64::MIN / 2);
        assert_eq!(v.cursor, 0.0);
    }

    #[test]
    fn markers_snap_to_frames_and_never_cross() {
        let mut v = t();
        v.seek(3.0123);
        v.mark_start();
        assert!((v.start - 3.0).abs() < 1e-9, "snapped to the 25 fps grid: {}", v.start);
        v.seek(6.52);
        v.mark_end();
        assert!((v.start - 3.0).abs() < 1e-9 && (v.end - 6.52).abs() < 1e-9, "{v:?}");
        assert!((v.clip() - 3.52).abs() < 1e-9 && !v.is_whole());
        // half a frame rounds to the nearer frame line
        v.seek(6.5);
        v.mark_end();
        assert!((v.end - 6.52).abs() < 1e-9, "{}", v.end);
        // start after the end pushes the end out
        v.seek(8.0);
        v.mark_start();
        assert!(v.end >= v.start + MIN_CLIP && v.end <= 10.0);
        // end before the start pulls the start back
        v.seek(1.0);
        v.mark_end();
        assert!(v.start <= v.end - MIN_CLIP && v.start >= 0.0);
        // markers at the very ends keep a clip
        v.seek(9.96);
        v.mark_start();
        assert!(v.clip() >= MIN_CLIP - 1e-9 && v.end <= v.duration);
        v.seek(0.0);
        v.mark_end();
        assert!(v.start >= 0.0 && v.clip() >= MIN_CLIP - 1e-9);
    }

    #[test]
    fn labels_show_minutes_seconds_and_frame() {
        let v = t();
        assert_eq!(v.label(0.0), "0:00.00");
        assert_eq!(v.label(65.2), "1:05.05");
        assert_eq!(v.label(3.999), "0:04.00", "rounding up to a whole second carries");
        assert_eq!(v.label(-2.0), "0:00.00");
    }

    #[test]
    fn speed_changes_the_length() {
        assert_eq!(sped_duration(10.0, 2.0), Some(5.0));
        assert_eq!(sped_duration(10.0, 0.5), Some(20.0));
        assert_eq!(sped_duration(10.0, 1.0), Some(10.0));
        for (d, f) in [(10.0, 0.1), (10.0, 4.5), (10.0, f64::NAN), (-1.0, 2.0), (f64::INFINITY, 2.0), (0.0, 1.0)] {
            assert_eq!(sped_duration(d, f), None, "{d} {f}");
        }
    }

    #[test]
    fn frame_rates_from_ffprobe() {
        assert_eq!(parse_rate("25"), Some(25.0));
        assert!((parse_rate("30000/1001").unwrap() - 29.97).abs() < 0.01);
        assert_eq!(parse_rate(" 24/1 "), Some(24.0));
        for bad in ["0/0", "x", "", "-5", "1/0", "99999"] {
            assert_eq!(parse_rate(bad), None, "{bad}");
        }
    }

    #[test]
    fn split_ranges_prefers_points_over_parts() {
        assert_eq!(split_ranges(10.0, &[4.0], 5), vec![(0.0, 4.0), (4.0, 10.0)], "points override parts");
        assert_eq!(split_ranges(10.0, &[], 4), equal_parts(10.0, 4), "no points equals equal_parts");
        assert_eq!(split_ranges(10.0, &[], 1).len(), 1);
        assert!(split_ranges(10.0, &[], 0).is_empty());
    }

    #[test]
    fn split_ranges_edge_cases() {
        assert_eq!(split_ranges(10.0, &[3.0, 3.01], 2).len(), 2, "tiny piece dropped");
        assert_eq!(split_ranges(10.0, &[0.01], 2), vec![(0.0, 10.0)], "only a tiny piece: nothing to cut");
        assert!(split_ranges(f64::NAN, &[1.0], 2).is_empty());
        assert!(split_ranges(f64::NAN, &[], 2).is_empty());
        assert!(split_ranges(0.1, &[], 10).is_empty());
    }

    #[test]
    fn add_cut_snaps_dedupes_and_sorts() {
        let mut v = t();
        let mut pts = vec![];
        v.seek(5.0);
        assert!(v.add_cut(&mut pts));
        assert!(!v.add_cut(&mut pts), "same frame twice");
        v.seek(2.0123);
        assert!(v.add_cut(&mut pts));
        assert!((pts[0] - 2.0).abs() < 1e-9 && (pts[1] - 5.0).abs() < 1e-9, "{pts:?}");
        v.seek(0.0);
        assert!(!v.add_cut(&mut pts), "no cut at the very start");
        assert_eq!(pts.len(), 2);
    }

    fn contiguous(r: &[(f64, f64)], duration: f64) -> bool {
        !r.is_empty()
            && r[0].0 == 0.0
            && r.last().map(|x| x.1) == Some(duration)
            && r.windows(2).all(|w| w[0].1 == w[1].0)
            && r.iter().all(|x| x.1 - x.0 >= MIN_PIECE - 1e-9)
    }

    #[test]
    fn equal_parts_cover_the_whole_clip_without_drift() {
        assert_eq!(equal_parts(10.0, 1), vec![(0.0, 10.0)]);
        let four = equal_parts(10.0, 4);
        assert_eq!(four.len(), 4);
        assert!((four[1].0 - 2.5).abs() < 1e-9 && (four[2].1 - 7.5).abs() < 1e-9);
        assert!(contiguous(&four, 10.0));
        for n in 1..=50 {
            let r = equal_parts(7.3, n);
            assert_eq!(r.len(), n, "{n}");
            assert!(contiguous(&r, 7.3), "{n}: {r:?}");
        }
    }

    #[test]
    fn equal_parts_refuses_impossible_requests() {
        assert!(equal_parts(10.0, 0).is_empty());
        assert!(equal_parts(0.0, 3).is_empty());
        assert!(equal_parts(f64::NAN, 3).is_empty());
        assert!(equal_parts(-5.0, 3).is_empty());
        assert!(equal_parts(f64::INFINITY, 3).is_empty());
        assert!(equal_parts(0.1, 10).is_empty(), "pieces of 0.01 s are refused");
    }

    #[test]
    fn points_become_contiguous_ranges() {
        assert_eq!(points_to_ranges(10.0, &[3.0, 7.0]), vec![(0.0, 3.0), (3.0, 7.0), (7.0, 10.0)]);
        assert_eq!(points_to_ranges(10.0, &[7.0, 3.0, 3.0]), vec![(0.0, 3.0), (3.0, 7.0), (7.0, 10.0)], "sorted and deduplicated");
        assert_eq!(points_to_ranges(10.0, &[]), vec![(0.0, 10.0)], "no points keeps the whole clip");
    }

    #[test]
    fn points_outside_the_clip_or_not_numbers_are_ignored() {
        assert_eq!(points_to_ranges(10.0, &[-1.0, 0.0, 10.0, 11.0, f64::NAN, f64::INFINITY]), vec![(0.0, 10.0)]);
        assert!(points_to_ranges(0.0, &[1.0]).is_empty());
        assert!(points_to_ranges(f64::NAN, &[1.0]).is_empty());
    }

    #[test]
    fn a_point_that_would_make_a_tiny_piece_is_dropped_so_no_content_is_lost() {
        assert_eq!(points_to_ranges(10.0, &[3.0, 3.01]), vec![(0.0, 3.0), (3.0, 10.0)]);
        assert_eq!(points_to_ranges(10.0, &[0.01, 5.0]), vec![(0.0, 5.0), (5.0, 10.0)], "tiny first piece");
        assert_eq!(points_to_ranges(10.0, &[9.99]), vec![(0.0, 10.0)], "tiny last piece");
        let r = points_to_ranges(10.0, &[1.0, 1.02, 1.04, 4.0, 4.03, 9.97]);
        assert!(contiguous(&r, 10.0), "{r:?}");
    }

    #[test]
    fn exact_start_and_end_keep_a_minimum_clip_inside_the_timeline() {
        let mut v = t();
        v.set_start(3.0);
        v.set_end(7.5);
        assert_eq!((v.start, v.end), (3.0, 7.5));
        v.set_start(50.0);
        assert!((v.start - (7.5 - MIN_CLIP)).abs() < 1e-9, "start stops one minimum clip before the end");
        v.set_end(-4.0);
        assert!((v.end - (v.start + MIN_CLIP)).abs() < 1e-9, "end stops one minimum clip after the start");
        v.set_start(-2.0);
        assert_eq!(v.start, 0.0);
        v.set_end(99.0);
        assert_eq!(v.end, 10.0);
        let before = v;
        v.set_start(f64::NAN);
        v.set_end(f64::INFINITY);
        assert_eq!(v, before, "numbers that are not numbers change nothing");
    }

    #[test]
    fn set_range_applies_a_valid_range_and_reports_whether_it_changed_anything() {
        let mut v = t();
        assert!(v.set_range(1.0, 4.0));
        assert_eq!((v.start, v.end), (1.0, 4.0));
        assert!(!v.set_range(1.0, 4.0), "the same range changes nothing");
        assert!(!v.set_range(5.0, 5.01), "a range shorter than the minimum clip is refused");
        assert_eq!((v.start, v.end), (1.0, 4.0));
        assert!(v.set_range(-1.0, 99.0), "a range past the ends is clamped");
        assert!(v.is_whole());
    }
}
