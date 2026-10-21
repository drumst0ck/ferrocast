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
    /// Why egui repainted, for frames drawn while hidden: cause -> count in this window.
    hidden_causes: std::collections::BTreeMap<String, u32>,
}

impl Probe {
    pub fn new(now: Instant) -> Self {
        Probe { window_start: now, visible_frames: 0, hidden_frames: 0, hidden_causes: Default::default() }
    }

    /// Record one frame.
    pub fn record(&mut self, now: Instant, visible: bool, took: Duration, causes: Vec<String>) -> Vec<String> {
        let mut out = vec![];
        if took > SLOW_FRAME {
            out.push(format!("slow frame {took:?} visible={visible}"));
        }
        if visible {
            self.visible_frames += 1;
        } else {
            self.hidden_frames += 1;
            for c in causes {
                *self.hidden_causes.entry(c).or_default() += 1;
            }
        }
        out.extend(self.flush(now, false));
        out
    }

    /// Close the window when it is over (`force` closes it regardless). Called by every frame and by
    /// a ticker thread, so a window in which no frame was drawn is reported too ("visible=0 hidden=0").
    pub fn flush(&mut self, now: Instant, force: bool) -> Vec<String> {
        let mut out = vec![];
        let len = now.duration_since(self.window_start);
        if force || len >= WINDOW {
            out.push(format!("frames in {len:?}: visible={} hidden={}", self.visible_frames, self.hidden_frames));
            for (cause, n) in std::mem::take(&mut self.hidden_causes) {
                out.push(format!("hidden repaint cause x{n}: {cause}"));
            }
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

/// Time `draw` and report it; `causes` (what egui says requested the repaint) is only asked for
/// hidden frames. Without `FERROCAST_PROFILE` this is a plain call.
pub fn frame<R>(visible: bool, causes: impl FnOnce() -> Vec<String>, draw: impl FnOnce() -> R) -> R {
    if !enabled() {
        return draw();
    }
    static PROBE: OnceLock<Mutex<Probe>> = OnceLock::new();
    let t = Instant::now();
    let r = draw();
    let took = t.elapsed();
    let causes = if visible { vec![] } else { causes() };
    let probe = PROBE.get_or_init(|| {
        std::thread::Builder::new()
            .name("perf-probe".into())
            .spawn(|| loop {
                std::thread::sleep(WINDOW);
                if let Some(Ok(mut p)) = PROBE.get().map(|m| m.lock()) {
                    for line in p.flush(Instant::now(), false) {
                        eprintln!("{line}");
                    }
                }
            })
            .ok();
        Mutex::new(Probe::new(t))
    });
    if let Ok(mut p) = probe.lock() {
        for line in p.record(Instant::now(), visible, took, causes) {
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
        assert!(p.record(t0, true, Duration::from_millis(4), vec![]).is_empty(), "exactly the budget is not slow");
        let v = p.record(t0, false, Duration::from_millis(9), vec![]);
        assert_eq!(v.len(), 1);
        assert!(v[0].starts_with("slow frame 9") && v[0].ends_with("visible=false"), "{v:?}");
    }

    #[test]
    fn counters_report_once_per_window_and_reset() {
        let t0 = Instant::now();
        let mut p = Probe::new(t0);
        let ms = Duration::from_millis(1);
        for i in 0..3 {
            assert!(p.record(t0 + Duration::from_secs(i), true, ms, vec![]).is_empty());
        }
        assert!(p.record(t0 + Duration::from_secs(4), false, ms, vec!["a.rs:1 x".into()]).is_empty());
        let out = p.record(t0 + WINDOW, false, ms, vec!["a.rs:1 x".into(), "b.rs:2 y".into()]);
        assert!(out[0].ends_with("visible=3 hidden=2"), "{out:?}");
        assert_eq!(&out[1..], ["hidden repaint cause x2: a.rs:1 x", "hidden repaint cause x1: b.rs:2 y"], "{out:?}");
        // next window starts from zero
        let out = p.record(t0 + WINDOW * 2, true, ms, vec![]);
        assert!(out[0].ends_with("visible=1 hidden=0"), "{out:?}");
    }
}
