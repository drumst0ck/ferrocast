//! Frame probe behind `FERROCAST_PROFILE=1`: slow frames and frames drawn while the launcher
//! is hidden. Nothing here runs, allocates or locks unless the variable is set.

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// A frame slower than this is reported (the icon budget per frame is 4 ms, `ui::IconCache`).
pub const SLOW_FRAME: Duration = Duration::from_millis(4);
/// Length of one reporting window for the visible/hidden frame counters.
pub const WINDOW: Duration = Duration::from_secs(10);

/// Pure state machine: feed it every drawn frame, it returns the lines to print.
pub struct Probe {
    window_start: Instant,
    visible_frames: u32,
    hidden_frames: u32,
}

impl Probe {
    pub fn new(now: Instant) -> Self {
        Probe { window_start: now, visible_frames: 0, hidden_frames: 0 }
    }

    /// Record one frame. A window with no frame at all prints nothing, so a missing
    /// `frames` line over a window means 0 frames were drawn.
    pub fn record(&mut self, now: Instant, visible: bool, took: Duration) -> Vec<String> {
        let mut out = vec![];
        if took > SLOW_FRAME {
            out.push(format!("slow frame {took:?} visible={visible}"));
        }
        if visible {
            self.visible_frames += 1;
        } else {
            self.hidden_frames += 1;
        }
        if now.duration_since(self.window_start) >= WINDOW {
            out.push(format!(
                "frames in {:?}: visible={} hidden={}",
                now.duration_since(self.window_start),
                self.visible_frames,
                self.hidden_frames
            ));
            self.window_start = now;
            self.visible_frames = 0;
            self.hidden_frames = 0;
        }
        out
    }
}

fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("FERROCAST_PROFILE").is_some())
}

/// Time `draw` and report it. Without `FERROCAST_PROFILE` this is a plain call.
pub fn frame<R>(visible: bool, draw: impl FnOnce() -> R) -> R {
    if !enabled() {
        return draw();
    }
    static PROBE: OnceLock<Mutex<Probe>> = OnceLock::new();
    let t = Instant::now();
    let r = draw();
    let took = t.elapsed();
    let probe = PROBE.get_or_init(|| Mutex::new(Probe::new(t)));
    if let Ok(mut p) = probe.lock() {
        for line in p.record(Instant::now(), visible, took) {
            eprintln!("{line}");
        }
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slow_frames_are_reported_with_visibility_and_fast_ones_are_silent() {
        let t0 = Instant::now();
        let mut p = Probe::new(t0);
        assert!(p.record(t0, true, Duration::from_millis(4)).is_empty(), "exactly the budget is not slow");
        let v = p.record(t0, false, Duration::from_millis(9));
        assert_eq!(v.len(), 1);
        assert!(v[0].starts_with("slow frame 9") && v[0].ends_with("visible=false"), "{v:?}");
    }

    #[test]
    fn counters_report_once_per_window_and_reset() {
        let t0 = Instant::now();
        let mut p = Probe::new(t0);
        let ms = Duration::from_millis(1);
        for i in 0..3 {
            assert!(p.record(t0 + Duration::from_secs(i), true, ms).is_empty());
        }
        assert!(p.record(t0 + Duration::from_secs(4), false, ms).is_empty());
        let out = p.record(t0 + WINDOW, false, ms);
        assert_eq!(out.len(), 1);
        assert!(out[0].ends_with("visible=3 hidden=2"), "{out:?}");
        // next window starts from zero
        let out = p.record(t0 + WINDOW * 2, true, ms);
        assert!(out[0].ends_with("visible=1 hidden=0"), "{out:?}");
    }
}
