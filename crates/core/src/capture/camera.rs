//! Camera overlay layout (S17): shape, corner, size, margin, mirror and visibility, plus the
//! pure geometry the renderer draws. All sizes are fractions of the OUTPUT width so the layout
//! is resolution independent; [`rect`] turns them into whole output pixels.

use super::lut::BUNDLED_LOOK_NAMES;
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// P6 T39: `overlay` (default, the bubble above) or `split` (screen and camera side by side).
    /// Omitted from the JSON at its default so old layouts serialise unchanged.
    #[serde(skip_serializing_if = "CameraPlacement::is_overlay")]
    pub layout: CameraPlacement,
    /// Used when `layout` is `split`; omitted at its default.
    #[serde(skip_serializing_if = "SplitLayout::is_default")]
    pub split: SplitLayout,
    /// P6 T40: colour look applied to the camera image; omitted when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub look: Option<Look>,
    /// P6 T41 (BETA, model only): remove the camera background (person cut-out). The renderer
    /// treats it as best effort; omitted when false.
    #[serde(skip_serializing_if = "is_false")]
    pub cutout: bool,
    /// P6 T42: reframe the camera on the face using `camera_track.json`; omitted when false.
    #[serde(skip_serializing_if = "is_false")]
    pub follow_face: bool,
    /// Zoom of the face-following crop, `1..=3` (renderer default when absent).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub follow_zoom: Option<f64>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// Smallest and largest face-following zoom.
pub const FOLLOW_ZOOM_MIN: f64 = 1.0;
pub const FOLLOW_ZOOM_MAX: f64 = 3.0;

/// A colour look: either a bundled one by `name` or a `.cube` `file` inside the project folder
/// (exactly one of the two), blended with the original picture by `intensity`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Look {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Plain file name of a `.cube` inside the project folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// `0..=1`: 0 leaves the picture alone, 1 is the full look.
    #[serde(default = "full")]
    pub intensity: f64,
}

fn full() -> f64 {
    1.0
}

impl Look {
    fn validate(&self) -> Vec<String> {
        let mut out = vec![];
        match (&self.name, &self.file) {
            (Some(n), None) if !BUNDLED_LOOK_NAMES.contains(&n.as_str()) => {
                out.push(format!("camera.look.name `{n}` is not a bundled look ({})", BUNDLED_LOOK_NAMES.join(", ")));
            }
            (Some(_), None) => {}
            (None, Some(f)) => {
                let plain = !f.is_empty() && f != "." && f != ".." && !f.contains(['/', '\\', '\0']);
                if !plain || !f.to_ascii_lowercase().ends_with(".cube") {
                    out.push(format!("camera.look.file `{f}` must be a plain .cube file name inside the project"));
                }
            }
            _ => out.push("camera.look needs exactly one of name or file".into()),
        }
        if !(self.intensity.is_finite() && (0.0..=1.0).contains(&self.intensity)) {
            out.push(format!("camera.look.intensity {} outside 0..=1", self.intensity));
        }
        out
    }
}

impl Default for CameraLayout {
    fn default() -> Self {
        Self {
            shape: CameraShape::Circle,
            corner: Corner::BottomRight,
            size: 0.20,
            margin: 0.02,
            mirror: true,
            visible: true,
            layout: CameraPlacement::Overlay,
            split: SplitLayout::default(),
            look: None,
            cutout: false,
            follow_face: false,
            follow_zoom: None,
        }
    }
}

/// How the camera shares the canvas with the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CameraPlacement {
    /// A bubble over the screen (S17).
    #[default]
    Overlay,
    /// Screen and camera side by side, nothing overlaps.
    Split,
}

impl CameraPlacement {
    fn is_overlay(&self) -> bool {
        *self == Self::Overlay
    }
}

/// Which side of the canvas the camera takes in a split.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitSide {
    Left,
    #[default]
    Right,
    Top,
    Bottom,
}

/// Smallest and largest camera share of the canvas along the split axis.
pub const SPLIT_RATIO_MIN: f64 = 0.25;
pub const SPLIT_RATIO_MAX: f64 = 0.5;
/// Largest gap between the two parts (fraction of the output width).
pub const SPLIT_GAP_MAX: f64 = 0.10;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SplitLayout {
    /// The side the camera takes.
    pub side: SplitSide,
    /// Camera share of the canvas along the split axis, `0.25..=0.5`.
    pub ratio: f64,
    /// Gap between screen and camera, fraction of the output width, `0..=0.1`.
    pub gap: f64,
}

impl Default for SplitLayout {
    fn default() -> Self {
        Self { side: SplitSide::Right, ratio: 0.33, gap: 0.01 }
    }
}

impl SplitLayout {
    fn is_default(&self) -> bool {
        *self == Self::default()
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
        if self.layout == CameraPlacement::Split {
            let s = &self.split;
            if !(s.ratio.is_finite() && (SPLIT_RATIO_MIN..=SPLIT_RATIO_MAX).contains(&s.ratio)) {
                out.push(format!("camera.split.ratio {} outside {SPLIT_RATIO_MIN}..={SPLIT_RATIO_MAX}", s.ratio));
            }
            if !(s.gap.is_finite() && (0.0..=SPLIT_GAP_MAX).contains(&s.gap)) {
                out.push(format!("camera.split.gap {} outside 0..={SPLIT_GAP_MAX}", s.gap));
            }
        }
        if let Some(l) = &self.look {
            out.extend(l.validate());
        }
        if self.follow_zoom.is_some_and(|z| !(z.is_finite() && (FOLLOW_ZOOM_MIN..=FOLLOW_ZOOM_MAX).contains(&z))) {
            out.push(format!("camera.follow_zoom {} outside {FOLLOW_ZOOM_MIN}..={FOLLOW_ZOOM_MAX}", self.follow_zoom.unwrap_or_default()));
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

/// Round down to an even number of pixels (encoders want even sizes), never below `min`.
fn even(v: f64, min: f64) -> f64 {
    if v.is_finite() {
        ((v / 2.0).floor() * 2.0).max(min)
    } else {
        min
    }
}

/// `(screen_rect, camera_rect)` for the `split` layout, in whole output pixels, all even and
/// inside the canvas (an odd canvas edge loses one pixel). The camera part is
/// `ratio x` the canvas along the split axis and spans the whole other axis (the renderer
/// aspect-fills the camera into it); the screen is letterboxed, centred, inside what remains
/// after the gap. `screen_aspect` is width / height of the screen content; a non-positive or
/// non-finite value fills the part. Out-of-range ratio and gap are clamped.
pub fn split_rects(canvas_w: f64, canvas_h: f64, l: &CameraLayout, screen_aspect: f64) -> (Rect, Rect) {
    let (w, h) = (even(canvas_w, 4.0), even(canvas_h, 4.0));
    let s = &l.split;
    let horizontal = matches!(s.side, SplitSide::Left | SplitSide::Right);
    let (axis, cross) = if horizontal { (w, h) } else { (h, w) };
    let ratio = if s.ratio.is_finite() { s.ratio.clamp(SPLIT_RATIO_MIN, SPLIT_RATIO_MAX) } else { SplitLayout::default().ratio };
    let cam_len = even(ratio * axis, 2.0).min(axis - 2.0);
    let gap_px = if s.gap.is_finite() { s.gap.clamp(0.0, SPLIT_GAP_MAX) * w } else { 0.0 };
    let gap = even(gap_px.min(axis - cam_len - 2.0), 0.0);
    let screen_len = axis - cam_len - gap;
    // part rectangles along the split axis: (offset, length)
    let (cam_at, screen_at) = match s.side {
        SplitSide::Left | SplitSide::Top => (0.0, cam_len + gap),
        SplitSide::Right | SplitSide::Bottom => (axis - cam_len, 0.0),
    };
    let part =
        |at: f64, len: f64| if horizontal { Rect { x: at, y: 0.0, w: len, h: cross } } else { Rect { x: 0.0, y: at, w: cross, h: len } };
    let (cam, area) = (part(cam_at, cam_len), part(screen_at, screen_len));
    let (mut sw, mut sh) = (area.w, area.h);
    if screen_aspect.is_finite() && screen_aspect > 0.0 {
        sh = (area.w / screen_aspect).min(area.h);
        sw = (sh * screen_aspect).min(area.w);
        sw = even(sw, 2.0).min(area.w);
        sh = even(sh, 2.0).min(area.h);
    }
    let screen = Rect { x: area.x + even((area.w - sw) / 2.0, 0.0), y: area.y + even((area.h - sh) / 2.0, 0.0), w: sw, h: sh };
    (screen, cam)
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

    // ---- P6 T39: split layout

    fn split(side: SplitSide, ratio: f64, gap: f64) -> CameraLayout {
        CameraLayout { layout: CameraPlacement::Split, split: SplitLayout { side, ratio, gap }, ..CameraLayout::default() }
    }

    #[test]
    fn split_fields_are_skipped_at_defaults_and_round_trip() {
        let old = r#"{"shape":"circle","corner":"bottom_right","size":0.2,"margin":0.02,"mirror":true,"visible":true}"#;
        assert_eq!(serde_json::to_string(&CameraLayout::default()).unwrap(), old);
        let l = split(SplitSide::Left, 0.3, 0.01);
        let s = serde_json::to_string(&l).unwrap();
        assert_eq!(
            s,
            r#"{"shape":"circle","corner":"bottom_right","size":0.2,"margin":0.02,"mirror":true,"visible":true,"layout":"split","split":{"side":"left","ratio":0.3,"gap":0.01}}"#
        );
        assert_eq!(serde_json::from_str::<CameraLayout>(&s).unwrap(), l);
        // a partial split object fills the rest from the defaults
        let p: CameraLayout = serde_json::from_str(r#"{"layout":"split","split":{"side":"top"}}"#).unwrap();
        assert_eq!((p.layout, p.split.side, p.split.ratio), (CameraPlacement::Split, SplitSide::Top, SplitLayout::default().ratio));
        assert!(serde_json::from_str::<CameraLayout>(r#"{"layout":"grid"}"#).is_err());
    }

    #[test]
    fn split_validation() {
        assert!(split(SplitSide::Right, 0.25, 0.0).validate().is_empty());
        assert!(split(SplitSide::Right, 0.5, 0.1).validate().is_empty());
        for bad in [0.24, 0.51, f64::NAN] {
            assert_eq!(split(SplitSide::Top, bad, 0.0).validate().len(), 1, "{bad}");
        }
        for bad in [-0.01, 0.11, f64::INFINITY] {
            assert_eq!(split(SplitSide::Top, 0.3, bad).validate().len(), 1, "{bad}");
        }
        // a bad split under the overlay layout is not used, so not reported
        let mut o = split(SplitSide::Top, 9.0, 0.3);
        o.layout = CameraPlacement::Overlay;
        assert!(o.validate().is_empty());
    }

    const R16_9: f64 = 16.0 / 9.0;

    #[test]
    fn split_rects_for_the_four_sides_at_1920x1080() {
        let at = |side| split_rects(1920.0, 1080.0, &split(side, 0.3, 0.01), R16_9);
        let r = |x, y, w, h| Rect { x, y, w, h };
        // camera 576 px, gap 18 px, the screen is letterboxed inside the remaining 1326 x 1080
        assert_eq!(at(SplitSide::Right), (r(0.0, 168.0, 1326.0, 744.0), r(1344.0, 0.0, 576.0, 1080.0)));
        assert_eq!(at(SplitSide::Left), (r(594.0, 168.0, 1326.0, 744.0), r(0.0, 0.0, 576.0, 1080.0)));
        // camera 324 px tall, gap 18: the screen part is 1920 x 738, limited by its height
        assert_eq!(at(SplitSide::Top), (r(304.0, 342.0, 1312.0, 738.0), r(0.0, 0.0, 1920.0, 324.0)));
        assert_eq!(at(SplitSide::Bottom), (r(304.0, 0.0, 1312.0, 738.0), r(0.0, 756.0, 1920.0, 324.0)));
    }

    #[test]
    fn split_rects_are_even_inside_the_canvas_and_disjoint_for_odd_sizes() {
        let sides = [SplitSide::Left, SplitSide::Right, SplitSide::Top, SplitSide::Bottom];
        for (w, h) in [(1921.0, 1081.0), (1081.0, 1921.0), (801.0, 451.0), (333.0, 777.0), (64.0, 64.0), (1920.0, 1080.0)] {
            for side in sides {
                for ratio in [0.25, 0.33, 0.5] {
                    for gap in [0.0, 0.013, 0.1] {
                        for aspect in [R16_9, 1.0, 9.0 / 16.0, 2.4] {
                            let (s, c) = split_rects(w, h, &split(side, ratio, gap), aspect);
                            let tag = format!("{w}x{h} {side:?} {ratio} {gap} {aspect}: {s:?} {c:?}");
                            for r in [s, c] {
                                assert!([r.x, r.y, r.w, r.h].iter().all(|v| v % 2.0 == 0.0 && *v >= 0.0), "{tag}");
                                assert!(r.w >= 2.0 && r.h >= 2.0, "{tag}");
                                assert!(r.x + r.w <= w && r.y + r.h <= h, "{tag}");
                            }
                            let apart = s.x + s.w <= c.x || c.x + c.w <= s.x || s.y + s.h <= c.y || c.y + c.h <= s.y;
                            assert!(apart, "{tag}");
                            // letterboxed, not stretched: within one even step of the screen aspect
                            assert!((s.w / s.h - aspect).abs() <= 2.0 / s.h.min(s.w) + 2.0 * aspect / s.h, "{tag}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn split_rects_tiny_or_degenerate_canvases_stay_valid() {
        let (s, c) = split_rects(10.0, 10.0, &split(SplitSide::Right, 0.5, 0.1), 0.0);
        assert!(s.w >= 2.0 && c.w >= 2.0 && s.x + s.w <= 10.0 && c.x + c.w <= 10.0);
        let (s, c) = split_rects(0.0, 0.0, &split(SplitSide::Top, 0.5, 0.1), f64::NAN);
        assert!(s.w >= 2.0 && c.h >= 2.0, "{s:?} {c:?}");
    }

    // ---- P6 T40 looks, T41 cutout, T42 follow-face fields

    fn look(name: Option<&str>, file: Option<&str>, intensity: f64) -> Look {
        Look { name: name.map(Into::into), file: file.map(Into::into), intensity }
    }

    #[test]
    fn look_cutout_and_follow_fields_are_optional_and_skipped_at_defaults() {
        let d = serde_json::to_value(CameraLayout::default()).unwrap();
        for k in ["look", "cutout", "follow_face", "follow_zoom"] {
            assert!(d.get(k).is_none(), "{k} must be omitted at its default");
        }
        let l = CameraLayout {
            look: Some(look(Some("warm"), None, 0.8)),
            cutout: true,
            follow_face: true,
            follow_zoom: Some(1.5),
            ..CameraLayout::default()
        };
        let s = serde_json::to_string(&l).unwrap();
        assert_eq!(
            s,
            r#"{"shape":"circle","corner":"bottom_right","size":0.2,"margin":0.02,"mirror":true,"visible":true,"look":{"name":"warm","intensity":0.8},"cutout":true,"follow_face":true,"follow_zoom":1.5}"#
        );
        assert_eq!(serde_json::from_str::<CameraLayout>(&s).unwrap(), l);
        // a custom look and a default intensity
        let c: CameraLayout = serde_json::from_str(r#"{"look":{"file":"grade.cube"}}"#).unwrap();
        assert_eq!(c.look, Some(look(None, Some("grade.cube"), 1.0)));
        assert_eq!(serde_json::to_string(&c.look).unwrap(), r#"{"file":"grade.cube","intensity":1.0}"#);
    }

    #[test]
    fn look_validation() {
        let with = |lk: Look| CameraLayout { look: Some(lk), ..CameraLayout::default() }.validate();
        assert!(with(look(Some("mono"), None, 0.0)).is_empty());
        assert!(with(look(Some("high-contrast"), None, 1.0)).is_empty());
        assert!(with(look(None, Some("my grade.CUBE"), 0.5)).is_empty());
        assert_eq!(with(look(Some("sepia"), None, 1.0)).len(), 1, "not a bundled look");
        assert_eq!(with(look(None, None, 1.0)).len(), 1, "neither name nor file");
        assert_eq!(with(look(Some("warm"), Some("a.cube"), 1.0)).len(), 1, "both");
        for bad in ["../a.cube", "a/b.cube", "a\\b.cube", "", "..", "grade.png", "grade"] {
            assert_eq!(with(look(None, Some(bad), 1.0)).len(), 1, "{bad:?}");
        }
        for bad in [-0.01, 1.01, f64::NAN, f64::INFINITY] {
            assert_eq!(with(look(Some("warm"), None, bad)).len(), 1, "{bad}");
        }
    }

    #[test]
    fn follow_zoom_range() {
        let with = |z: Option<f64>| CameraLayout { follow_face: true, follow_zoom: z, ..CameraLayout::default() }.validate();
        assert!(with(None).is_empty() && with(Some(1.0)).is_empty() && with(Some(3.0)).is_empty());
        for bad in [0.99, 3.01, f64::NAN] {
            assert_eq!(with(Some(bad)).len(), 1, "{bad}");
        }
    }
}
