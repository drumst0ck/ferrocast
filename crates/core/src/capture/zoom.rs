//! Auto zoom from the click log, and viewport maths. Everything here is pure:
//! the same events and options always give the same zooms, bit for bit.
//!
//! Coordinates in `Viewport` are normalised to the recorded video (`0..=1`, top-left
//! origin): the display, or `screen.capture_rect` for area/window takes. `events.json`
//! points are converted with `events::CaptureSpace`.
use super::curves::{clamp, critical_spring, lerp};
use super::events::{CaptureSpace, Events};
pub use super::project::Zoom;

/// Largest zoom scale the project format accepts (`Project::validate`).
pub const MAX_SCALE: f64 = 6.0;

/// Auto-zoom tunables; the defaults are the numbers in the plan.
#[derive(Debug, Clone, PartialEq)]
pub struct ZoomOpts {
    /// Zoom level, clamped to `1.0..=6.0`.
    pub scale: f64,
    /// A zoom starts this many seconds before its first click.
    pub lead: f64,
    /// A zoom ends this long after its last click or cursor movement.
    pub idle: f64,
    /// Clicks join one cluster while the gap to the previous click is at most this.
    pub merge_gap: f64,
    /// ...and the distance is at most this fraction of the display width.
    pub merge_dist: f64,
    /// Clusters shorter than this are dropped.
    pub min_duration: f64,
    /// Optional cap on a zoom's length (the plan has none).
    pub max_duration: Option<f64>,
    /// Zooms closer than this (or overlapping) merge.
    pub zoom_gap: f64,
    /// Cursor movement above this many pixels per sample counts as activity.
    pub move_px: f64,
    /// A typing burst is at least this many key presses...
    pub typing_keys: usize,
    /// ...each at most this many seconds after the previous.
    pub typing_window: f64,
    pub ease_in: f64,
    pub ease_out: f64,
    /// Fraction of the viewport (per axis) inside which the cursor does not move it.
    pub dead_zone: f64,
}

impl Default for ZoomOpts {
    fn default() -> Self {
        Self {
            scale: 2.0,
            lead: 0.4,
            idle: 1.2,
            merge_gap: 1.5,
            merge_dist: 0.25,
            min_duration: 0.5,
            max_duration: None,
            zoom_gap: 0.8,
            move_px: 8.0,
            typing_keys: 5,
            typing_window: 2.0,
            ease_in: 0.6,
            ease_out: 0.7,
            dead_zone: 0.6,
        }
    }
}

/// Camera: viewport centre (normalised) and zoom factor (`1` = whole display).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub cx: f64,
    pub cy: f64,
    pub scale: f64,
}

impl Viewport {
    /// `[x, y, w, h]` normalised to the display.
    pub fn rect(&self) -> [f64; 4] {
        let w = 1.0 / self.scale;
        [self.cx - w / 2.0, self.cy - w / 2.0, w, w]
    }
}

struct Span {
    start: f64,
    end: f64,
    pts: Vec<[f64; 2]>,
}

fn mean(pts: &[[f64; 2]]) -> [f64; 2] {
    let n = pts.len().max(1) as f64;
    [pts.iter().map(|p| p[0]).sum::<f64>() / n, pts.iter().map(|p| p[1]).sum::<f64>() / n]
}

/// Zoom segments for the clicks (and typing bursts) in `events`.
pub fn auto_zooms(events: &Events, opts: &ZoomOpts) -> Vec<Zoom> {
    let [_, _, dw, dh] = events.screen.display_frame;
    if !(dw > 0.0 && dh > 0.0) {
        return vec![];
    }
    let scale = clamp(opts.scale, 1.0, MAX_SCALE);
    let px = events.screen.scale.max(f64::MIN_POSITIVE);
    // Focus points are normalised to the recorded video (the capture rect for area/window takes).
    let space = CaptureSpace::new(&events.screen);
    let norm = |x: f64, y: f64| space.point_clamped(x, y);

    // A click outside the recorded rectangle is not in the video: nothing to zoom to.
    let mut clicks: Vec<_> = events.clicks.iter().filter(|c| c.is_down() && c.t.is_finite() && space.contains(c.x, c.y)).collect();
    clicks.sort_by(|a, b| a.t.total_cmp(&b.t));
    let mut cursor: Vec<_> = events.cursor.iter().filter(|c| c.t().is_finite()).collect();
    cursor.sort_by(|a, b| a.t().total_cmp(&b.t()));

    // 1. click clusters
    let mut clusters: Vec<Vec<&super::events::Click>> = vec![];
    for c in clicks {
        let joins = clusters
            .last()
            .and_then(|g| g.last())
            .is_some_and(|p| c.t - p.t <= opts.merge_gap && ((c.x - p.x).powi(2) + (c.y - p.y).powi(2)).sqrt() <= opts.merge_dist * dw);
        if joins {
            clusters.last_mut().unwrap().push(c);
        } else {
            clusters.push(vec![c]);
        }
    }
    let mut spans: Vec<Span> = vec![];
    for g in &clusters {
        let first = g[0].t;
        let mut last = g[g.len() - 1].t;
        // cursor movement after the last click keeps the zoom open
        for w in cursor.windows(2) {
            let (a, b) = (w[0], w[1]);
            if b.t() <= last {
                continue;
            }
            let moved = ((b.1 - a.1).powi(2) + (b.2 - a.2).powi(2)).sqrt() * px;
            if moved > opts.move_px && b.t() - last <= opts.idle {
                last = b.t();
            }
        }
        spans.push(Span { start: (first - opts.lead).max(0.0), end: last + opts.idle, pts: g.iter().map(|c| norm(c.x, c.y)).collect() });
    }

    // 2. typing bursts, centred on the last known cursor position
    let mut keys: Vec<f64> = events.keys.iter().filter(|k| k.down && k.t.is_finite()).map(|k| k.t).collect();
    keys.sort_by(f64::total_cmp);
    let mut i = 0;
    while i < keys.len() {
        let mut j = i;
        while j + 1 < keys.len() && keys[j + 1] - keys[j] <= opts.typing_window {
            j += 1;
        }
        if j + 1 - i >= opts.typing_keys.max(1) {
            let at = keys[j];
            let pos = cursor.iter().rev().find(|c| c.t() <= at).or(cursor.first()).map_or([0.5, 0.5], |c| norm(c.1, c.2));
            spans.push(Span { start: (keys[i] - opts.lead).max(0.0), end: at + opts.idle, pts: vec![pos] });
        }
        i = j + 1;
    }

    // 3. drop short, merge close, cap long
    spans.retain(|s| s.end - s.start >= opts.min_duration);
    spans.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.end.total_cmp(&b.end)));
    let mut merged: Vec<Span> = vec![];
    for s in spans {
        match merged.last_mut() {
            Some(m) if s.start - m.end < opts.zoom_gap - 1e-9 => {
                m.end = m.end.max(s.end);
                m.pts.extend(s.pts);
            }
            _ => merged.push(s),
        }
    }
    let half = 0.5 / scale;
    merged
        .into_iter()
        .enumerate()
        .map(|(i, s)| {
            let f = mean(&s.pts);
            Zoom {
                id: format!("z{}", i + 1),
                start: s.start,
                end: opts.max_duration.map_or(s.end, |m| s.end.min(s.start + m)),
                mode: "auto".into(),
                scale,
                focus: Some([clamp(f[0], half, 1.0 - half), clamp(f[1], half, 1.0 - half)]),
                ease_in: opts.ease_in,
                ease_out: opts.ease_out,
            }
        })
        .collect()
}

/// Camera at output-independent source time `t`, with the default dead zone.
/// `cursor` is the (smoothed) cursor, normalised to the display, if known.
///
/// Zoom-in is a critically damped spring from `start` (response `ease_in`);
/// zoom-out starts at `end` (response `ease_out`) and the camera is back to the
/// full display after `end + ease_out`. While zoomed, the centre stays on the
/// focus until the cursor leaves the inner 60 % of the viewport, then moves
/// just enough to keep it on that box's edge. The viewport never leaves the display.
pub fn viewport_at(zoom: &Zoom, t: f64, cursor: Option<[f64; 2]>) -> Viewport {
    viewport_with(zoom, t, cursor, ZoomOpts::default().dead_zone)
}

/// [`viewport_at`] with an explicit dead-zone fraction.
pub fn viewport_with(zoom: &Zoom, t: f64, cursor: Option<[f64; 2]>, dead_zone: f64) -> Viewport {
    let full = Viewport { cx: 0.5, cy: 0.5, scale: 1.0 };
    let top = clamp(zoom.scale, 1.0, MAX_SCALE);
    let env = if t <= zoom.start {
        return full;
    } else if t < zoom.end {
        critical_spring(t - zoom.start, zoom.ease_in)
    } else if t < zoom.end + zoom.ease_out {
        critical_spring(zoom.end - zoom.start, zoom.ease_in) * (1.0 - critical_spring(t - zoom.end, zoom.ease_out))
    } else {
        return full;
    };
    let s = lerp(1.0, top, env);
    let half = 0.5 / s;
    let tgt = zoom.focus.unwrap_or([0.5, 0.5]);
    let tgt = [clamp(tgt[0], 0.5 / top, 1.0 - 0.5 / top), clamp(tgt[1], 0.5 / top, 1.0 - 0.5 / top)];
    let mut c = [lerp(0.5, tgt[0], env), lerp(0.5, tgt[1], env)];
    if let Some(p) = cursor {
        let hb = clamp(dead_zone, 0.0, 1.0) * half;
        for k in 0..2 {
            let p = clamp(p[k], 0.0, 1.0);
            if p > c[k] + hb {
                c[k] = p - hb;
            } else if p < c[k] - hb {
                c[k] = p + hb;
            }
        }
    }
    Viewport { cx: clamp(c[0], half, 1.0 - half), cy: clamp(c[1], half, 1.0 - half), scale: s }
}

#[cfg(test)]
mod tests {
    use super::super::events::{Click, CursorSample, Key, ScreenInfo};
    use super::*;

    fn ev() -> Events {
        Events::new(ScreenInfo { size: [2000, 1000], scale: 2.0, display_frame: [0.0, 0.0, 1000.0, 500.0], capture_rect: None })
    }
    fn click(t: f64, x: f64, y: f64) -> Click {
        Click { t, x, y, button: "left".into(), down: true, up: false }
    }
    fn with_clicks(c: &[(f64, f64, f64)]) -> Events {
        let mut e = ev();
        e.clicks = c.iter().map(|&(t, x, y)| click(t, x, y)).collect();
        e
    }

    #[test]
    fn no_clicks_no_zooms() {
        assert!(auto_zooms(&ev(), &ZoomOpts::default()).is_empty());
    }

    #[test]
    fn single_click_lead_and_idle() {
        let z = auto_zooms(&with_clicks(&[(5.0, 500.0, 250.0)]), &ZoomOpts::default());
        assert_eq!(z.len(), 1);
        assert!((z[0].start - 4.6).abs() < 1e-9);
        assert!((z[0].end - 6.2).abs() < 1e-9);
        assert_eq!(z[0].mode, "auto");
        assert_eq!(z[0].focus, Some([0.5, 0.5]));
    }

    #[test]
    fn start_never_negative() {
        let z = auto_zooms(&with_clicks(&[(0.1, 500.0, 250.0)]), &ZoomOpts::default());
        assert_eq!(z[0].start, 0.0);
    }

    #[test]
    fn close_clicks_merge_far_clicks_split() {
        let z = auto_zooms(&with_clicks(&[(5.0, 250.0, 250.0), (6.0, 300.0, 260.0)]), &ZoomOpts::default());
        assert_eq!(z.len(), 1);
        assert!((z[0].end - 7.2).abs() < 1e-9);
        let z = auto_zooms(&with_clicks(&[(5.0, 100.0, 250.0), (20.0, 100.0, 250.0)]), &ZoomOpts::default());
        assert_eq!(z.len(), 2);
        // close in time but far in space: separate clusters, zooms overlap, so they merge
        let z = auto_zooms(&with_clicks(&[(5.0, 100.0, 250.0), (6.0, 900.0, 250.0)]), &ZoomOpts::default());
        assert_eq!(z.len(), 1);
    }

    #[test]
    fn nearby_zooms_merge_under_gap() {
        // first ends 6.2, second starts 7.0 (gap 0.8 is not closer than 0.8), third pair 6.9 (gap .7)
        let z = auto_zooms(&with_clicks(&[(5.0, 100.0, 100.0), (7.4, 100.0, 100.0)]), &ZoomOpts::default());
        assert_eq!(z.len(), 2);
        let z = auto_zooms(&with_clicks(&[(5.0, 100.0, 100.0), (7.3, 100.0, 100.0)]), &ZoomOpts::default());
        assert_eq!(z.len(), 1);
    }

    #[test]
    fn cursor_movement_extends_end_but_small_jitter_does_not() {
        let mut e = with_clicks(&[(5.0, 500.0, 250.0)]);
        // moving 10 pt (20 px) per sample until t=8.0
        e.cursor = (0..=60).map(|i| CursorSample(6.0 + i as f64 / 30.0, 500.0 + i as f64 * 5.0, 250.0, "arrow".into())).collect();
        let z = auto_zooms(&e, &ZoomOpts::default());
        assert!((z[0].end - 9.2).abs() < 1e-6, "end {}", z[0].end);
        let mut e = with_clicks(&[(5.0, 500.0, 250.0)]);
        e.cursor = (0..=60).map(|i| CursorSample(6.0 + i as f64 / 30.0, 500.0 + i as f64 * 0.5, 250.0, "arrow".into())).collect();
        assert!((auto_zooms(&e, &ZoomOpts::default())[0].end - 6.2).abs() < 1e-9);
    }

    #[test]
    fn min_and_max_duration() {
        let opts = ZoomOpts { min_duration: 3.0, ..ZoomOpts::default() };
        assert!(auto_zooms(&with_clicks(&[(5.0, 1.0, 1.0)]), &opts).is_empty());
        let opts = ZoomOpts { max_duration: Some(1.0), ..ZoomOpts::default() };
        let z = auto_zooms(&with_clicks(&[(5.0, 1.0, 1.0)]), &opts);
        assert!((z[0].end - z[0].start - 1.0).abs() < 1e-9);
    }

    #[test]
    fn ignores_releases_and_unsorted_input() {
        let mut e = with_clicks(&[(9.0, 500.0, 250.0), (5.0, 500.0, 250.0)]);
        e.clicks.push(Click { t: 30.0, x: 1.0, y: 1.0, button: "left".into(), down: false, up: true });
        let z = auto_zooms(&e, &ZoomOpts::default());
        assert_eq!(z.len(), 2);
        assert!((z[0].start - 4.6).abs() < 1e-9);
    }

    #[test]
    fn typing_burst_makes_zoom_at_last_cursor() {
        let mut e = ev();
        e.cursor = vec![CursorSample(1.0, 400.0, 100.0, "arrow".into())];
        e.keys = (0..5).map(|i| Key { t: 10.0 + i as f64 * 0.3, down: true, mods: vec![], code: None, ch: None }).collect();
        let z = auto_zooms(&e, &ZoomOpts::default());
        assert_eq!(z.len(), 1);
        assert!((z[0].start - 9.6).abs() < 1e-9);
        assert_eq!(z[0].focus, Some([0.4, 0.25]));
        e.keys.truncate(4);
        assert!(auto_zooms(&e, &ZoomOpts::default()).is_empty());
    }

    #[test]
    fn scale_clamped_and_focus_keeps_viewport_inside() {
        let opts = ZoomOpts { scale: 3.0, ..ZoomOpts::default() };
        let z = auto_zooms(&with_clicks(&[(5.0, 0.0, 0.0), (30.0, 1000.0, 500.0)]), &opts);
        assert_eq!(z.len(), 2);
        for z in &z {
            assert!(z.scale <= 3.0);
            let f = z.focus.unwrap();
            assert!(f.iter().all(|v| (1.0 / 6.0 - 1e-12..=5.0 / 6.0 + 1e-12).contains(v)));
        }
        let big = ZoomOpts { scale: 50.0, ..ZoomOpts::default() };
        assert!(auto_zooms(&with_clicks(&[(5.0, 1.0, 1.0)]), &big)[0].scale <= 6.0);
        let tiny = ZoomOpts { scale: 0.2, ..ZoomOpts::default() };
        assert_eq!(auto_zooms(&with_clicks(&[(5.0, 1.0, 1.0)]), &tiny)[0].scale, 1.0);
    }

    #[test]
    fn viewport_always_inside_display_property() {
        let mut seed = 7u64;
        let mut r = move || {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((seed >> 11) as f64) / ((1u64 << 53) as f64)
        };
        for _ in 0..1000 {
            let z = Zoom {
                id: "z".into(),
                start: 1.0,
                end: 1.0 + r() * 4.0,
                mode: "auto".into(),
                scale: 1.0 + r() * 5.0,
                focus: Some([r(), r()]),
                ease_in: 0.1 + r(),
                ease_out: 0.1 + r(),
            };
            let t = r() * 8.0;
            let cur = if r() > 0.3 { Some([r() * 1.4 - 0.2, r() * 1.4 - 0.2]) } else { None };
            let [x, y, w, h] = viewport_at(&z, t, cur).rect();
            let e = 1e-9;
            assert!(x >= -e && y >= -e && x + w <= 1.0 + e && y + h <= 1.0 + e, "{x} {y} {w} {h}");
            assert!(viewport_at(&z, t, cur).scale <= z.scale + 1e-12);
        }
    }

    fn zoom() -> Zoom {
        Zoom { id: "z".into(), start: 1.0, end: 5.0, mode: "auto".into(), scale: 2.0, focus: Some([0.5, 0.5]), ease_in: 0.6, ease_out: 0.7 }
    }

    #[test]
    fn envelope_in_hold_out() {
        let z = zoom();
        assert_eq!(viewport_at(&z, 0.5, None).scale, 1.0);
        assert_eq!(viewport_at(&z, 1.0, None).scale, 1.0);
        assert!(viewport_at(&z, 1.3, None).scale > 1.0);
        assert!(viewport_at(&z, 3.0, None).scale > 1.999);
        assert!(viewport_at(&z, 5.3, None).scale < 2.0);
        assert!(viewport_at(&z, 5.3, None).scale > 1.0);
        assert_eq!(viewport_at(&z, 6.0, None).scale, 1.0);
        let v = viewport_at(&z, 0.0, None);
        assert_eq!((v.cx, v.cy), (0.5, 0.5));
    }

    #[test]
    fn dead_zone_follow() {
        let z = zoom();
        // viewport 0.5 wide, dead zone 60 % = 0.3 wide: ±0.15 around centre
        let v = viewport_at(&z, 3.0, Some([0.6, 0.5]));
        assert!((v.cx - 0.5).abs() < 1e-3, "inside dead zone must not move: {}", v.cx);
        let v = viewport_at(&z, 3.0, Some([0.8, 0.5]));
        assert!((v.cx - 0.65).abs() < 1e-3, "cx {}", v.cx);
        let v = viewport_at(&z, 3.0, Some([5.0, 0.5]));
        assert!(v.cx <= 0.75 + 1e-9);
    }

    #[test]
    fn golden_json() {
        let e = with_clicks(&[(5.0, 250.0, 250.0), (6.0, 300.0, 260.0), (20.0, 900.0, 100.0)]);
        let a = serde_json::to_string(&auto_zooms(&e, &ZoomOpts::default())).unwrap();
        let b = serde_json::to_string(&auto_zooms(&e, &ZoomOpts::default())).unwrap();
        assert_eq!(a, b);
        assert_eq!(
            a,
            r#"[{"id":"z1","start":4.6,"end":7.2,"mode":"auto","scale":2.0,"focus":[0.275,0.51],"ease_in":0.6,"ease_out":0.7},{"id":"z2","start":19.6,"end":21.2,"mode":"auto","scale":2.0,"focus":[0.75,0.25],"ease_in":0.6,"ease_out":0.7}]"#
        );
    }

    /// The lower-right quarter `[500, 250, 500, 250]` of the 1000 x 500 pt display was recorded.
    fn area_with_clicks(c: &[(f64, f64, f64)]) -> Events {
        let mut e = with_clicks(c);
        e.screen.size = [1000, 500];
        e.screen.capture_rect = Some([500.0, 250.0, 500.0, 250.0]);
        e
    }

    #[test]
    fn area_zoom_focus_is_in_the_recorded_rect() {
        // (700, 350) is (0.4, 0.4) of the rect; a display-normalised focus would be (0.7, 0.7)
        let z = auto_zooms(&area_with_clicks(&[(5.0, 700.0, 350.0)]), &ZoomOpts::default());
        assert_eq!(z.len(), 1);
        let f = z[0].focus.unwrap();
        assert!((f[0] - 0.4).abs() < 1e-12 && (f[1] - 0.4).abs() < 1e-12, "{f:?}");
    }

    #[test]
    fn area_clicks_outside_the_rect_make_no_zoom() {
        assert!(auto_zooms(&area_with_clicks(&[(5.0, 100.0, 100.0)]), &ZoomOpts::default()).is_empty());
    }

    #[test]
    fn a_rect_equal_to_the_display_changes_nothing() {
        let plain = with_clicks(&[(5.0, 300.0, 100.0), (9.0, 800.0, 400.0)]);
        let mut full = plain.clone();
        full.screen.capture_rect = Some([0.0, 0.0, 1000.0, 500.0]);
        assert_eq!(auto_zooms(&plain, &ZoomOpts::default()), auto_zooms(&full, &ZoomOpts::default()));
    }
}
