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
    fn frame_rates_from_ffprobe() {
        assert_eq!(parse_rate("25"), Some(25.0));
        assert!((parse_rate("30000/1001").unwrap() - 29.97).abs() < 0.01);
        assert_eq!(parse_rate(" 24/1 "), Some(24.0));
        for bad in ["0/0", "x", "", "-5", "1/0", "99999"] {
            assert_eq!(parse_rate(bad), None, "{bad}");
        }
    }
}
