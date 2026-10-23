//! Auto zoom from the click log, and viewport maths. Everything here is pure:
//! the same events and options always give the same zooms, bit for bit.
//!
//! Coordinates in `Viewport` are normalised to the recorded video (`0..=1`, top-left
//! origin): the display, or `screen.capture_rect` for area/window takes. `events.json`
//! points are converted with `events::CaptureSpace`.
use super::curves::{clamp, critical_spring, lerp, smoothstep};
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

// ---------------------------------------------------------------------------------------------
// Loupes (P6 T38)

use super::project::Project;
pub use super::project::{Loupe, LOUPE_EASE_MAX, LOUPE_RADIUS_MAX, LOUPE_RADIUS_MIN, LOUPE_SCALE_MAX, LOUPE_SCALE_MIN, MIN_LOUPE};
use super::timeline::Timeline;

/// Length of a loupe added at the playhead, seconds.
pub const NEW_LOUPE_LEN: f64 = 3.0;

/// What the renderer draws at one instant: focus centre (`0..=1` of the screen content), radius
/// (fraction of the output width), magnification and fade (`0..=1`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoupeState {
    pub cx: f64,
    pub cy: f64,
    pub radius: f64,
    pub scale: f64,
    pub alpha: f64,
}

/// The loupe active at source time `t`, following the cursor of an unfocused loupe through
/// `cursor` (normalised to the content; the centre of the content when unknown). Fades with the
/// same smoothstep ease as the rest of the effects: `alpha` is 0 at `start`, 1 after `ease_in`,
/// and back to 0 at `end` over `ease_out`. The magnification itself stays constant.
/// Cuts are not consulted: the renderer only asks for kept source times.
pub fn loupe_at_with_cursor(p: &Project, t: f64, cursor: Option<[f64; 2]>) -> Option<LoupeState> {
    let l = p.loupes.iter().find(|l| t >= l.start && t < l.end)?;
    let ramp = |x: f64, dur: f64| if dur > 0.0 { smoothstep(x / dur) } else { 1.0 };
    let alpha = ramp(t - l.start, l.ease_in) * ramp(l.end - t, l.ease_out);
    let [cx, cy] = match (l.x, l.y) {
        (Some(x), Some(y)) => [x, y],
        _ => cursor.unwrap_or([0.5, 0.5]),
    };
    Some(LoupeState { cx: clamp(cx, 0.0, 1.0), cy: clamp(cy, 0.0, 1.0), radius: l.radius, scale: l.scale, alpha })
}

/// [`loupe_at_with_cursor`] without a cursor.
pub fn loupe_at(p: &Project, t: f64) -> Option<LoupeState> {
    loupe_at_with_cursor(p, t, None)
}

/// A loupe placed on the output timeline.
#[derive(Debug, Clone, PartialEq)]
pub struct LoupeSpan {
    pub id: String,
    /// The kept part of the loupe, in source seconds.
    pub src_start: f64,
    pub src_end: f64,
    pub out_start: f64,
    pub out_end: f64,
}

/// Every loupe mapped through trims and speeds (in list order). The parts inside a cut are
/// removed, so a loupe lying wholly in a cut is dropped; a loupe over a speed change keeps its
/// source range and gets the output time those pieces really take.
pub fn loupe_output_spans(p: &Project) -> Vec<LoupeSpan> {
    let tl = Timeline::new(p);
    let mut out = vec![];
    for l in &p.loupes {
        let mut kept: Option<(f64, f64, f64, f64)> = None;
        for c in tl.pieces() {
            let (a, b) = (l.start.max(c.src_start), l.end.min(c.src_end));
            if b <= a {
                continue;
            }
            let (oa, ob) = (tl.to_output(a), c.out_start + (b - c.src_start) / c.factor);
            kept = Some(kept.map_or((a, b, oa, ob), |(s, _, os, _)| (s, b, os, ob)));
        }
        if let Some((src_start, src_end, out_start, out_end)) = kept.filter(|k| k.3 > k.2) {
            out.push(LoupeSpan { id: l.id.clone(), src_start, src_end, out_start, out_end });
        }
    }
    out
}

pub enum LoupeField {
    Start(f64),
    End(f64),
    Scale(f64),
    Radius(f64),
    /// `None` follows the cursor.
    Focus(Option<[f64; 2]>),
    EaseIn(f64),
    EaseOut(f64),
}

fn finite(v: f64) -> Result<f64, String> {
    if v.is_finite() {
        Ok(v)
    } else {
        Err("That is not a number.".into())
    }
}

/// A loupe at `at`, [`NEW_LOUPE_LEN`] long, cut short by the next loupe or the end. Returns its id.
/// Wrap in `History::edit(.., EditKind::Loupe, ..)` to make it undoable.
pub fn add_loupe(p: &mut Project, at: f64) -> Result<String, String> {
    let at = finite(at)?.clamp(0.0, p.duration.max(0.0));
    if p.loupes.iter().any(|l| l.start <= at && at < l.end) {
        return Err("There is already a loupe at the playhead.".into());
    }
    let next = p.loupes.iter().filter(|l| l.start > at).map(|l| l.start).fold(f64::INFINITY, f64::min);
    let end = (at + NEW_LOUPE_LEN).min(p.duration).min(next);
    if end - at < MIN_LOUPE {
        return Err("There is no room for a loupe at the playhead.".into());
    }
    let id = (1..).map(|n| format!("l{n}")).find(|id| p.loupes.iter().all(|l| l.id != *id)).unwrap_or_default();
    p.loupes.push(Loupe {
        id: id.clone(),
        start: at,
        end,
        scale: 2.0,
        radius: 0.15,
        x: None,
        y: None,
        ease_in: 0.25,
        ease_out: 0.25,
        extra: Default::default(),
    });
    p.loupes.sort_by(|a, b| a.start.total_cmp(&b.start));
    Ok(id)
}

/// Edit one field of loupe `id`, clamped so the project stays valid (neighbours limit the span).
pub fn set_loupe(p: &mut Project, id: &str, field: LoupeField) -> Result<(), String> {
    let dur = p.duration.max(0.0);
    let idx = p.loupes.iter().position(|l| l.id == id).ok_or("That loupe no longer exists.")?;
    let prev_end =
        p.loupes.iter().enumerate().filter(|(i, l)| *i != idx && l.end <= p.loupes[idx].start).map(|(_, l)| l.end).fold(0.0, f64::max);
    let next_start =
        p.loupes.iter().enumerate().filter(|(i, l)| *i != idx && l.start >= p.loupes[idx].end).map(|(_, l)| l.start).fold(dur, f64::min);
    let l = &mut p.loupes[idx];
    match field {
        LoupeField::Start(v) => l.start = finite(v)?.clamp(prev_end, (l.end - MIN_LOUPE).max(prev_end)),
        LoupeField::End(v) => l.end = finite(v)?.clamp(l.start + MIN_LOUPE, next_start.max(l.start + MIN_LOUPE)),
        LoupeField::Scale(v) => l.scale = finite(v)?.clamp(LOUPE_SCALE_MIN, LOUPE_SCALE_MAX),
        LoupeField::Radius(v) => l.radius = finite(v)?.clamp(LOUPE_RADIUS_MIN, LOUPE_RADIUS_MAX),
        LoupeField::Focus(f) => {
            let (x, y) = match f {
                Some([x, y]) => (Some(finite(x)?.clamp(0.0, 1.0)), Some(finite(y)?.clamp(0.0, 1.0))),
                None => (None, None),
            };
            l.x = x;
            l.y = y;
        }
        LoupeField::EaseIn(v) => l.ease_in = finite(v)?.clamp(0.0, LOUPE_EASE_MAX),
        LoupeField::EaseOut(v) => l.ease_out = finite(v)?.clamp(0.0, LOUPE_EASE_MAX),
    }
    Ok(())
}

pub fn delete_loupe(p: &mut Project, id: &str) -> Result<(), String> {
    let n = p.loupes.len();
    p.loupes.retain(|l| l.id != id);
    if p.loupes.len() == n {
        return Err("That loupe no longer exists.".into());
    }
    Ok(())
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

#[cfg(test)]
mod loupe_tests {
    use super::super::project::{Project, Speed, Trim};
    use super::super::timeline::{EditKind, History};
    use super::*;

    fn loupe(id: &str, start: f64, end: f64) -> Loupe {
        Loupe {
            id: id.into(),
            start,
            end,
            scale: 2.0,
            radius: 0.12,
            x: Some(0.3),
            y: Some(0.6),
            ease_in: 0.2,
            ease_out: 0.2,
            extra: Default::default(),
        }
    }

    fn proj() -> Project {
        let mut p = Project::new("t", "2026-01-01T00:00:00Z");
        p.duration = 20.0;
        p
    }

    fn problems(p: &Project) -> Vec<String> {
        p.validate(|_| true).into_iter().filter(|m| m.contains("loupe")).collect()
    }

    #[test]
    fn loupe_json_is_optional_byte_stable_and_keeps_unknown_fields() {
        let mut p = proj();
        assert!(!p.to_json().contains("loupes"));
        let l = Loupe { x: None, y: None, ..loupe("l1", 1.0, 3.0) };
        p.loupes.push(l);
        let s = serde_json::to_string(&p.loupes).unwrap();
        assert_eq!(s, r#"[{"id":"l1","start":1.0,"end":3.0,"scale":2.0,"radius":0.12,"ease_in":0.2,"ease_out":0.2}]"#);
        let l: Loupe = serde_json::from_str(r#"{"id":"a","start":1,"end":2,"future":{"k":[1]}}"#).unwrap();
        assert_eq!((l.scale, l.radius, l.x, l.y), (2.0, 0.15, None, None));
        assert_eq!(serde_json::to_value(&l).unwrap()["future"], serde_json::json!({"k":[1]}));
    }

    #[test]
    fn validation_rules_each_report_one_problem() {
        let with = |f: &dyn Fn(&mut Vec<Loupe>)| {
            let mut p = proj();
            p.loupes = vec![loupe("a", 2.0, 5.0)];
            f(&mut p.loupes);
            problems(&p)
        };
        assert!(with(&|_| {}).is_empty());
        assert_eq!(with(&|l| l[0].scale = 1.19).len(), 1);
        assert_eq!(with(&|l| l[0].scale = 4.01).len(), 1);
        assert_eq!(with(&|l| l[0].radius = 0.04).len(), 1);
        assert_eq!(with(&|l| l[0].radius = 0.31).len(), 1);
        assert_eq!(with(&|l| l[0].x = Some(1.2)).len(), 1);
        assert_eq!(with(&|l| l[0].y = None).len(), 1, "one focus coordinate without the other");
        assert_eq!(with(&|l| l[0].ease_in = -0.1).len(), 1);
        assert_eq!(with(&|l| l[0].ease_out = f64::NAN).len(), 1);
        assert_eq!(with(&|l| l[0].end = 1.0).len(), 1, "end before start");
        assert_eq!(with(&|l| l[0].end = 2.1).len(), 1, "too short");
        assert_eq!(with(&|l| l[0].end = 25.0).len(), 1, "past the duration");
        assert_eq!(with(&|l| l[0].start = -1.0).len(), 1);
        assert_eq!(with(&|l| l.push(loupe("b", 4.0, 7.0))).len(), 1, "overlap");
        assert_eq!(with(&|l| l.push(loupe("a", 6.0, 7.0))).len(), 1, "duplicate id");
        assert!(with(&|l| l.push(loupe("b", 5.0, 7.0))).is_empty(), "touching is fine");
        assert!(with(&|l| l.insert(0, loupe("b", 8.0, 9.0))).is_empty(), "order in the file does not matter");
    }

    #[test]
    fn loupe_at_envelope_and_focus() {
        let mut p = proj();
        p.loupes = vec![loupe("a", 2.0, 6.0)];
        assert_eq!(loupe_at(&p, 1.9), None);
        assert_eq!(loupe_at(&p, 6.0), None);
        let s = loupe_at(&p, 2.0).unwrap();
        assert_eq!((s.alpha, s.cx, s.cy, s.radius, s.scale), (0.0, 0.3, 0.6, 0.12, 2.0));
        let mid = loupe_at(&p, 2.1).unwrap().alpha;
        assert!((mid - 0.5).abs() < 1e-12, "{mid}");
        assert_eq!(loupe_at(&p, 4.0).unwrap().alpha, 1.0);
        assert!((loupe_at(&p, 5.9).unwrap().alpha - 0.5).abs() < 1e-12);
        // follows the cursor when no focus is set, clamped to the content
        p.loupes[0].x = None;
        p.loupes[0].y = None;
        let s = loupe_at_with_cursor(&p, 4.0, Some([0.8, 1.4])).unwrap();
        assert_eq!((s.cx, s.cy), (0.8, 1.0));
        let s = loupe_at(&p, 4.0).unwrap();
        assert_eq!((s.cx, s.cy), (0.5, 0.5));
        // zero easing is a hard edge, not NaN
        p.loupes[0].ease_in = 0.0;
        p.loupes[0].ease_out = 0.0;
        assert_eq!(loupe_at(&p, 2.0).unwrap().alpha, 1.0);
    }

    #[test]
    fn output_spans_follow_cuts_and_speed() {
        let mut p = proj();
        p.trims = vec![Trim { start: 0.0, end: 4.0 }, Trim { start: 8.0, end: 20.0 }];
        p.speeds = vec![Speed { start: 10.0, end: 14.0, factor: 2.0 }];
        p.loupes = vec![loupe("inside_cut", 5.0, 7.0), loupe("straddle", 3.0, 9.0), loupe("fast", 9.0, 12.0), loupe("late", 12.0, 16.0)];
        // `fast` and `late` overlap on purpose: spans are computed per loupe, validation is separate.
        let spans = loupe_output_spans(&p);
        let ids: Vec<&str> = spans.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["straddle", "fast", "late"], "a loupe wholly inside a cut is dropped");
        // kept 0..4 -> out 0..4, kept 8..20: 8..10 -> 4..6, 10..14 at 2x -> 6..8, 14..20 -> 8..14
        let s = &spans[0];
        assert_eq!((s.out_start, s.out_end), (3.0, 5.0));
        assert_eq!((s.src_start, s.src_end), (3.0, 9.0));
        let f = &spans[1];
        assert_eq!((f.out_start, f.out_end), (5.0, 7.0));
        let l = &spans[2];
        assert_eq!((l.out_start, l.out_end), (7.0, 10.0));
    }

    #[test]
    fn edits_are_clamped_validated_and_undoable() {
        let mut p = proj();
        let mut h = History::new(50, 0.5);
        let mut id = String::new();
        assert!(h.edit(&mut p, EditKind::Loupe, 0.0, |p| id = add_loupe(p, 4.0).unwrap()));
        assert_eq!(p.loupes.len(), 1);
        assert_eq!((p.loupes[0].start, p.loupes[0].end), (4.0, 7.0));
        assert!(problems(&p).is_empty());
        // no loupe on top of another one
        assert!(add_loupe(&mut p, 5.0).is_err());
        let before = p.clone();
        assert!(set_loupe(&mut p, "nope", LoupeField::Scale(2.0)).is_err());
        assert!(set_loupe(&mut p, &id, LoupeField::Scale(f64::NAN)).is_err());
        assert_eq!(p, before);
        // continuous edits coalesce into one undo step and clamp to the valid range
        for (i, v) in [3.0, 3.5, 9.0].into_iter().enumerate() {
            assert!(h.edit(&mut p, EditKind::Loupe, 0.1 + i as f64 * 0.1, |p| set_loupe(p, &id, LoupeField::Scale(v)).unwrap()));
        }
        assert_eq!(p.loupes[0].scale, 4.0);
        set_loupe(&mut p, &id, LoupeField::Radius(0.9)).unwrap();
        assert_eq!(p.loupes[0].radius, 0.30);
        set_loupe(&mut p, &id, LoupeField::Focus(Some([2.0, -1.0]))).unwrap();
        assert_eq!((p.loupes[0].x, p.loupes[0].y), (Some(1.0), Some(0.0)));
        set_loupe(&mut p, &id, LoupeField::Focus(None)).unwrap();
        assert_eq!((p.loupes[0].x, p.loupes[0].y), (None, None));
        set_loupe(&mut p, &id, LoupeField::End(100.0)).unwrap();
        assert_eq!(p.loupes[0].end, 20.0);
        set_loupe(&mut p, &id, LoupeField::Start(19.99)).unwrap();
        assert!(p.loupes[0].end - p.loupes[0].start >= MIN_LOUPE - 1e-12);
        assert!(problems(&p).is_empty(), "{:?}", problems(&p));
        // a neighbour limits End
        let mut q = proj();
        q.loupes = vec![loupe("a", 2.0, 4.0), loupe("b", 8.0, 9.0)];
        set_loupe(&mut q, "a", LoupeField::End(15.0)).unwrap();
        assert_eq!(q.loupes[0].end, 8.0);
        set_loupe(&mut q, "b", LoupeField::Start(0.0)).unwrap();
        assert_eq!(q.loupes[1].start, 8.0, "a now ends at 8, so b cannot start earlier");
        // delete + undo/redo restore exact projects
        assert!(h.edit(&mut p, EditKind::Loupe, 5.0, |p| delete_loupe(p, &id).unwrap()));
        assert!(p.loupes.is_empty() && delete_loupe(&mut p, &id).is_err());
        assert!(h.undo(&mut p));
        assert_eq!((p.loupes.len(), p.loupes[0].scale), (1, 4.0));
        assert!(h.undo(&mut p));
        assert!(p.loupes.is_empty(), "the add and the three scale drags were one undo step");
        assert!(h.redo(&mut p));
        assert_eq!(p.loupes.len(), 1);
    }
}
