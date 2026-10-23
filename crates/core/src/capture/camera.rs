//! Camera overlay layout (S17): shape, corner, size, margin, mirror and visibility, plus the
//! pure geometry the renderer draws. All sizes are fractions of the OUTPUT width so the layout
//! is resolution independent; [`rect`] turns them into whole output pixels.

use serde::{Deserialize, Serialize};

/// Smallest and largest overlay side as a fraction of the output width (S17: 10-40 %).
pub const SIZE_MIN: f64 = 0.10;
pub const SIZE_MAX: f64 = 0.40;
/// Largest margin (fraction of the output width).
pub const MARGIN_MAX: f64 = 0.25;
/// Corner radius of the `rounded` shape, as a fraction of the side.
pub const ROUNDED_RADIUS: f64 = 0.2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CameraShape {
    Circle,
    Rounded,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CameraLayout {
    pub shape: CameraShape,
    pub corner: Corner,
    /// Side of the (square) overlay, fraction of the output width, `0.10..=0.40`.
    pub size: f64,
    /// Gap to the canvas edges, fraction of the output width, `0..=0.25`.
    pub margin: f64,
    /// Flip the camera image horizontally (selfie view).
    pub mirror: bool,
    pub visible: bool,
}

impl Default for CameraLayout {
    fn default() -> Self {
        Self { shape: CameraShape::Circle, corner: Corner::BottomRight, size: 0.20, margin: 0.02, mirror: true, visible: true }
    }
}

/// Output-pixel rectangle, top-left origin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl CameraLayout {
    /// Problems with the layout (empty = valid).
    pub fn validate(&self) -> Vec<String> {
        let mut out = vec![];
        if !(self.size.is_finite() && (SIZE_MIN..=SIZE_MAX).contains(&self.size)) {
            out.push(format!("camera.size {} outside {SIZE_MIN}..={SIZE_MAX}", self.size));
        }
        if !(self.margin.is_finite() && (0.0..=MARGIN_MAX).contains(&self.margin)) {
            out.push(format!("camera.margin {} outside 0..={MARGIN_MAX}", self.margin));
        }
        out
    }
}

/// Overlay rectangle in whole output pixels. The side is `size x canvas_w` (clamped to the
/// shorter canvas edge) and the margin is shrunk when needed, so the rect never leaves the canvas.
pub fn rect(canvas_w: f64, canvas_h: f64, l: &CameraLayout) -> Rect {
    let side = (l.size * canvas_w).round().min(canvas_w.min(canvas_h)).max(1.0);
    let margin = (l.margin * canvas_w).round().max(0.0);
    let (mx, my) = (margin.min((canvas_w - side).max(0.0)), margin.min((canvas_h - side).max(0.0)));
    let x = match l.corner {
        Corner::TopLeft | Corner::BottomLeft => mx,
        Corner::TopRight | Corner::BottomRight => canvas_w - side - mx,
    };
    let y = match l.corner {
        Corner::TopLeft | Corner::TopRight => my,
        Corner::BottomLeft | Corner::BottomRight => canvas_h - side - my,
    };
    Rect { x, y, w: side, h: side }
}

/// Corner radius in output pixels for a square of `side` pixels.
pub fn corner_radius(shape: CameraShape, side: f64) -> f64 {
    match shape {
        CameraShape::Circle => side / 2.0,
        CameraShape::Rounded => side * ROUNDED_RADIUS,
        CameraShape::Square => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(corner: Corner, size: f64, margin: f64) -> CameraLayout {
        CameraLayout { corner, size, margin, ..CameraLayout::default() }
    }

    #[test]
    fn defaults_are_valid_and_match_the_spec_range() {
        let d = CameraLayout::default();
        assert!(d.validate().is_empty());
        assert_eq!(
            (d.shape, d.corner, d.size, d.margin, d.mirror, d.visible),
            (CameraShape::Circle, Corner::BottomRight, 0.20, 0.02, true, true)
        );
    }

    #[test]
    fn size_and_margin_ranges() {
        for ok in [0.10, 0.25, 0.40] {
            assert!(l(Corner::TopLeft, ok, 0.0).validate().is_empty(), "{ok}");
        }
        for bad in [0.09, 0.41, f64::NAN, f64::INFINITY, -0.2] {
            assert_eq!(l(Corner::TopLeft, bad, 0.0).validate().len(), 1, "{bad}");
        }
        assert_eq!(l(Corner::TopLeft, 0.2, 0.26).validate().len(), 1);
        assert_eq!(l(Corner::TopLeft, 0.2, -0.01).validate().len(), 1);
        assert_eq!(l(Corner::TopLeft, 0.2, f64::NAN).validate().len(), 1);
    }

    #[test]
    fn four_corners_at_1920x1080() {
        // side = 0.2 * 1920 = 384, margin = 0.02 * 1920 = 38.4 -> 38
        let r = |c| rect(1920.0, 1080.0, &l(c, 0.2, 0.02));
        assert_eq!(r(Corner::TopLeft), Rect { x: 38.0, y: 38.0, w: 384.0, h: 384.0 });
        assert_eq!(r(Corner::TopRight), Rect { x: 1498.0, y: 38.0, w: 384.0, h: 384.0 });
        assert_eq!(r(Corner::BottomLeft), Rect { x: 38.0, y: 658.0, w: 384.0, h: 384.0 });
        assert_eq!(r(Corner::BottomRight), Rect { x: 1498.0, y: 658.0, w: 384.0, h: 384.0 });
    }

    #[test]
    fn never_leaves_the_canvas() {
        for (w, h) in [(1920.0, 1080.0), (1080.0, 1920.0), (200.0, 100.0), (3840.0, 2160.0), (64.0, 64.0)] {
            for c in [Corner::TopLeft, Corner::TopRight, Corner::BottomLeft, Corner::BottomRight] {
                for size in [0.10, 0.4] {
                    for margin in [0.0, 0.02, 0.25] {
                        let r = rect(w, h, &l(c, size, margin));
                        assert!(r.x >= 0.0 && r.y >= 0.0 && r.x + r.w <= w && r.y + r.h <= h, "{w}x{h} {c:?} {size} {margin}: {r:?}");
                        assert_eq!(r.w, r.h);
                        assert!(r.w >= 1.0);
                    }
                }
            }
        }
    }

    #[test]
    fn tall_canvas_clamps_the_side_to_the_short_edge() {
        // 0.4 * 100 = 40 > height 30
        let r = rect(100.0, 30.0, &l(Corner::BottomRight, 0.4, 0.0));
        assert_eq!(r, Rect { x: 70.0, y: 0.0, w: 30.0, h: 30.0 });
    }

    #[test]
    fn radius_per_shape() {
        assert_eq!(corner_radius(CameraShape::Circle, 384.0), 192.0);
        assert_eq!(corner_radius(CameraShape::Rounded, 100.0), 20.0);
        assert_eq!(corner_radius(CameraShape::Square, 100.0), 0.0);
    }

    #[test]
    fn json_shape_and_partial_decode() {
        let s = serde_json::to_string(&CameraLayout::default()).unwrap();
        assert_eq!(s, r#"{"shape":"circle","corner":"bottom_right","size":0.2,"margin":0.02,"mirror":true,"visible":true}"#);
        let p: CameraLayout = serde_json::from_str(r#"{"shape":"rounded","corner":"top_left"}"#).unwrap();
        assert_eq!((p.shape, p.corner, p.size), (CameraShape::Rounded, Corner::TopLeft, 0.20));
        assert!(serde_json::from_str::<CameraLayout>(r#"{"shape":"star"}"#).is_err());
    }
}
