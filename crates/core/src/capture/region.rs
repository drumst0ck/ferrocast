//! Selection geometry for Capture Area/Window (pure, no platform calls).
//!
//! Two spaces are used, both in points:
//! * top-left space: origin at the primary display's top-left, y grows down
//!   (what `CGDisplay`/ScreenCaptureKit report);
//! * Cocoa space: origin at the primary display's bottom-left, y grows up
//!   (what `NSScreen`/`NSWindow` use).
//!
//! Display frames are given in top-left space, so a display above the primary
//! has a negative `y` and one to the left a negative `x`. Retina is only a
//! per-display `scale` applied when converting points to pixels.

use chrono::{Datelike, NaiveDateTime, Timelike};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    pub fn is_empty(&self) -> bool {
        self.w <= 0.0 || self.h <= 0.0
    }

    /// Half-open containment: left/top edges inside, right/bottom outside.
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x < self.x + self.w && p.y >= self.y && p.y < self.y + self.h
    }

    /// Overlap of two rects; `None` when they only touch or are apart.
    pub fn intersection(&self, o: &Rect) -> Option<Rect> {
        let x0 = self.x.max(o.x);
        let y0 = self.y.max(o.y);
        let x1 = (self.x + self.w).min(o.x + o.w);
        let y1 = (self.y + self.h).min(o.y + o.h);
        (x1 > x0 && y1 > y0).then(|| Rect::new(x0, y0, x1 - x0, y1 - y0))
    }
}

/// A display: frame in top-left space (points) and backing scale (1.0, 2.0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Display {
    pub frame: Rect,
    pub scale: f64,
}

/// A window frame in top-left space (points).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Window {
    pub id: u32,
    pub frame: Rect,
}

/// Rect spanned by two corners, whatever the drag direction.
pub fn normalize(a: Point, b: Point) -> Rect {
    Rect::new(a.x.min(b.x), a.y.min(b.y), (a.x - b.x).abs(), (a.y - b.y).abs())
}

/// Index of the display holding `p`.
pub fn display_at(displays: &[Display], p: Point) -> Option<usize> {
    displays.iter().position(|d| d.frame.contains(p))
}

/// `rect` limited to the display; `None` when nothing is left.
pub fn clamp_to_display(rect: Rect, display: &Display) -> Option<Rect> {
    rect.intersection(&display.frame)
}

/// Resolve a drag: normalise, then clip to the display under the pointer
/// (`end`). Returns the display index and the clipped rect, `None` when the
/// pointer is off every display or the clipped rect is empty.
pub fn select_area(displays: &[Display], start: Point, end: Point) -> Option<(usize, Rect)> {
    let i = display_at(displays, end)?;
    let r = clamp_to_display(normalize(start, end), &displays[i])?;
    Some((i, r))
}

/// Topmost window under `p`; `windows` is ordered front to back.
pub fn snap_window(windows: &[Window], p: Point) -> Option<&Window> {
    windows.iter().find(|w| w.frame.contains(p))
}

/// Flip a rect between top-left and Cocoa space (the operation is its own
/// inverse). `primary_height` is the primary display's height in points.
pub fn flip_rect(r: Rect, primary_height: f64) -> Rect {
    Rect::new(r.x, primary_height - (r.y + r.h), r.w, r.h)
}

/// Flip a point between top-left and Cocoa space.
pub fn flip_point(p: Point, primary_height: f64) -> Point {
    Point { x: p.x, y: primary_height - p.y }
}

/// Native pixel size of a rect on a display with the given scale.
pub fn pixel_size(r: Rect, scale: f64) -> (u32, u32) {
    ((r.w * scale).round() as u32, (r.h * scale).round() as u32)
}

/// `W × H` label in pixels.
pub fn size_label(r: Rect, scale: f64) -> String {
    let (w, h) = pixel_size(r, scale);
    format!("{w} × {h}")
}

/// `Capture YYYY-MM-DD at HH.MM.SS.png`, with ` 2`, ` 3`… before the
/// extension while `exists(name)` says the name is taken.
pub fn capture_file_name(at: NaiveDateTime, exists: impl Fn(&str) -> bool) -> String {
    let stem =
        format!("Capture {:04}-{:02}-{:02} at {:02}.{:02}.{:02}", at.year(), at.month(), at.day(), at.hour(), at.minute(), at.second());
    let mut name = format!("{stem}.png");
    let mut n = 2;
    while exists(&name) {
        name = format!("{stem} {n}.png");
        n += 1;
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn p(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    /// Primary 1440×900 @2x, external 1920×1080 @1x above it (negative y),
    /// centred horizontally: x from -240.
    fn above() -> Vec<Display> {
        vec![
            Display { frame: Rect::new(0.0, 0.0, 1440.0, 900.0), scale: 2.0 },
            Display { frame: Rect::new(-240.0, -1080.0, 1920.0, 1080.0), scale: 1.0 },
        ]
    }

    /// External @1x to the left of the primary (negative x).
    fn left() -> Vec<Display> {
        vec![
            Display { frame: Rect::new(0.0, 0.0, 1440.0, 900.0), scale: 2.0 },
            Display { frame: Rect::new(-1920.0, 0.0, 1920.0, 1080.0), scale: 1.0 },
        ]
    }

    #[test]
    fn normalize_any_drag_direction() {
        let want = Rect::new(10.0, 20.0, 30.0, 40.0);
        assert_eq!(normalize(p(10.0, 20.0), p(40.0, 60.0)), want);
        assert_eq!(normalize(p(40.0, 60.0), p(10.0, 20.0)), want);
        assert_eq!(normalize(p(40.0, 20.0), p(10.0, 60.0)), want);
        assert!(normalize(p(5.0, 5.0), p(5.0, 5.0)).is_empty());
    }

    #[test]
    fn display_at_handles_negative_origins_and_edges() {
        let d = above();
        assert_eq!(display_at(&d, p(10.0, 10.0)), Some(0));
        assert_eq!(display_at(&d, p(-100.0, -10.0)), Some(1));
        assert_eq!(display_at(&d, p(0.0, 0.0)), Some(0));
        assert_eq!(display_at(&d, p(0.0, -0.5)), Some(1));
        assert_eq!(display_at(&d, p(-300.0, 10.0)), None);
        assert_eq!(display_at(&left(), p(-1.0, 5.0)), Some(1));
    }

    #[test]
    fn clamp_clips_and_rejects() {
        let d = Display { frame: Rect::new(0.0, 0.0, 100.0, 100.0), scale: 1.0 };
        assert_eq!(clamp_to_display(Rect::new(-10.0, 20.0, 50.0, 200.0), &d), Some(Rect::new(0.0, 20.0, 40.0, 80.0)));
        assert_eq!(clamp_to_display(Rect::new(100.0, 0.0, 10.0, 10.0), &d), None);
        assert_eq!(clamp_to_display(Rect::new(10.0, 10.0, 0.0, 10.0), &d), None);
    }

    #[test]
    fn rect_crossing_displays_is_clipped_to_pointer_display() {
        let d = above();
        // Drag from the primary up into the display above; pointer ends above.
        let (i, r) = select_area(&d, p(100.0, 200.0), p(300.0, -300.0)).unwrap();
        assert_eq!(i, 1);
        assert_eq!(r, Rect::new(100.0, -300.0, 200.0, 300.0));
        // Same drag reversed: pointer ends on the primary.
        let (i, r) = select_area(&d, p(300.0, -300.0), p(100.0, 200.0)).unwrap();
        assert_eq!(i, 0);
        assert_eq!(r, Rect::new(100.0, 0.0, 200.0, 200.0));
        // Pointer off every display.
        assert_eq!(select_area(&d, p(10.0, 10.0), p(-500.0, 10.0)), None);
        // Left layout: drag from primary into the left display.
        let (i, r) = select_area(&left(), p(50.0, 50.0), p(-100.0, 150.0)).unwrap();
        assert_eq!(i, 1);
        assert_eq!(r, Rect::new(-100.0, 50.0, 100.0, 100.0));
    }

    #[test]
    fn snap_picks_topmost_window() {
        let w =
            [Window { id: 1, frame: Rect::new(100.0, 100.0, 200.0, 200.0) }, Window { id: 2, frame: Rect::new(0.0, 0.0, 500.0, 500.0) }];
        assert_eq!(snap_window(&w, p(150.0, 150.0)).map(|w| w.id), Some(1));
        assert_eq!(snap_window(&w, p(400.0, 400.0)).map(|w| w.id), Some(2));
        assert_eq!(snap_window(&w, p(600.0, 10.0)), None);
        assert_eq!(snap_window(&[], p(1.0, 1.0)), None);
    }

    #[test]
    fn flips_round_trip_with_display_above_primary() {
        let ph = 900.0;
        // A rect on the display above: top-left y negative, Cocoa y above 900.
        let r = Rect::new(-100.0, -300.0, 200.0, 100.0);
        let c = flip_rect(r, ph);
        assert_eq!(c, Rect::new(-100.0, 1100.0, 200.0, 100.0));
        assert_eq!(flip_rect(c, ph), r);
        // A rect on the primary: top at 100, height 200 → Cocoa bottom at 600.
        assert_eq!(flip_rect(Rect::new(10.0, 100.0, 50.0, 200.0), ph), Rect::new(10.0, 600.0, 50.0, 200.0));
        assert_eq!(flip_point(p(5.0, 0.0), ph), p(5.0, 900.0));
        assert_eq!(flip_point(flip_point(p(-7.0, -50.0), ph), ph), p(-7.0, -50.0));
    }

    #[test]
    fn flip_is_independent_of_secondary_scale_or_origin() {
        // Cocoa y of the external's bottom edge equals primary height for a display above.
        let d = above();
        let ext = flip_rect(d[1].frame, 900.0);
        assert_eq!(ext, Rect::new(-240.0, 900.0, 1920.0, 1080.0));
        let ext_left = flip_rect(left()[1].frame, 900.0);
        assert_eq!(ext_left, Rect::new(-1920.0, -180.0, 1920.0, 1080.0));
    }

    #[test]
    fn pixel_size_uses_display_scale() {
        let r = Rect::new(0.0, 0.0, 300.0, 200.0);
        assert_eq!(pixel_size(r, 2.0), (600, 400));
        assert_eq!(pixel_size(r, 1.0), (300, 200));
        assert_eq!(pixel_size(Rect::new(0.0, 0.0, 10.5, 3.0), 2.0), (21, 6));
        assert_eq!(size_label(r, 2.0), "600 × 400");
        assert_eq!(size_label(r, 1.0), "300 × 200");
    }

    fn at() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2025, 3, 7).unwrap().and_hms_opt(9, 5, 3).unwrap()
    }

    #[test]
    fn file_name_is_zero_padded() {
        assert_eq!(capture_file_name(at(), |_| false), "Capture 2025-03-07 at 09.05.03.png");
    }

    #[test]
    fn file_name_collision_suffixes() {
        let taken = ["Capture 2025-03-07 at 09.05.03.png", "Capture 2025-03-07 at 09.05.03 2.png"];
        assert_eq!(capture_file_name(at(), |n| taken.contains(&n)), "Capture 2025-03-07 at 09.05.03 3.png");
        assert_eq!(capture_file_name(at(), |n| n == taken[0]), "Capture 2025-03-07 at 09.05.03 2.png");
    }
}
