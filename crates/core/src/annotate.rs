//! Annotation model and rasterizer (Tangerine annotateImage): arrows, lines, boxes, ellipses,
//! highlights, freehand, numbered labels and magnifiers on an RGBA buffer. Pure: the canvas view
//! edits a `Doc` and the writer calls `render`. Free text, callouts and image stamps are not here.

pub type P = (f32, f32);

/// A picture to stamp onto the image: straight RGBA8, shared cheaply between clones.
/// Equality is identity (the same loaded picture), so comparing drawings stays cheap.
#[derive(Clone)]
pub struct Img(pub std::sync::Arc<ImgData>);

pub struct ImgData {
    pub w: usize,
    pub h: usize,
    pub rgba: Vec<u8>,
}

impl PartialEq for Img {
    fn eq(&self, o: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &o.0)
    }
}

impl std::fmt::Debug for Img {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Img({}x{})", self.0.w, self.0.h)
    }
}

impl Img {
    /// `None` when the buffer does not match the size or the picture is empty.
    pub fn new(w: usize, h: usize, rgba: Vec<u8>) -> Option<Img> {
        (w > 0 && h > 0 && rgba.len() == w * h * 4).then(|| Img(std::sync::Arc::new(ImgData { w, h, rgba })))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    pub color: [u8; 3],
    pub width: f32,
    /// Fill boxes and ellipses instead of outlining them.
    pub fill: bool,
}

impl Default for Style {
    fn default() -> Self {
        Style { color: [255, 59, 48], width: 4.0, fill: false }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    Line(P, P),
    Arrow(P, P),
    Rect(P, P),
    Ellipse(P, P),
    /// Translucent marker box.
    Highlight(P, P),
    Freehand(Vec<P>),
    /// Numbered circle.
    Number(P, u32),
    /// Circle that shows its centre zoomed.
    Magnifier {
        center: P,
        radius: f32,
        zoom: f32,
    },
    /// Free text with its top-left corner at the point; the size follows the stroke width.
    Text(P, String),
    /// Filled text box at `at` with a pointer line to `tip`.
    Callout {
        at: P,
        tip: P,
        text: String,
    },
    /// Another picture placed with its top-left corner at `at`, drawn `size` pixels wide and tall.
    Stamp {
        at: P,
        size: P,
        img: Img,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub shape: Shape,
    pub style: Style,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Doc {
    pub items: Vec<Item>,
}

/// Tool chosen in the canvas strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Select,
    Arrow,
    Line,
    Box,
    Ellipse,
    Highlight,
    Pen,
    Number,
    Magnifier,
    /// Click to place; the canvas asks for the words.
    Text,
    /// Drag from the target to where the box goes; the canvas asks for the words.
    Callout,
    /// Click to place; the canvas asks for the picture.
    Stamp,
}

/// Palette offered by the canvas.
pub const COLORS: [[u8; 3]; 6] = [[255, 59, 48], [255, 149, 0], [255, 214, 10], [52, 199, 89], [10, 132, 255], [20, 20, 20]];

/// The shape a drag through `pts` (image pixels) makes with `tool`, or `None` when it is too small
/// to mean anything (a stray click) or the tool draws nothing (`Select`).
pub fn build(tool: Tool, pts: &[P], next_number: u32) -> Option<Shape> {
    let (a, b) = (*pts.first()?, *pts.last()?);
    let far = seg_dist(b, a, a) >= 3.0;
    match tool {
        Tool::Select | Tool::Text | Tool::Callout | Tool::Stamp => None,
        Tool::Number => Some(Shape::Number(a, next_number)),
        Tool::Arrow => far.then_some(Shape::Arrow(a, b)),
        Tool::Line => far.then_some(Shape::Line(a, b)),
        Tool::Box => far.then_some(Shape::Rect(a, b)),
        Tool::Ellipse => far.then_some(Shape::Ellipse(a, b)),
        Tool::Highlight => far.then_some(Shape::Highlight(a, b)),
        Tool::Pen => (pts.len() >= 2 && far || pts.len() >= 8).then(|| Shape::Freehand(pts.to_vec())),
        Tool::Magnifier => far.then(|| Shape::Magnifier { center: a, radius: seg_dist(b, a, a).max(8.0), zoom: 2.5 }),
    }
}

/// Pixel height of text drawn with this stroke width.
pub fn text_size(width: f32) -> f32 {
    (width * 5.0).max(12.0)
}

fn font() -> Option<&'static ab_glyph::FontVec> {
    static FONT: std::sync::OnceLock<Option<ab_glyph::FontVec>> = std::sync::OnceLock::new();
    FONT.get_or_init(|| {
        ["/System/Library/Fonts/Supplemental/Arial.ttf", "/System/Library/Fonts/Supplemental/Arial Unicode.ttf"]
            .iter()
            .find_map(|p| std::fs::read(p).ok())
            .and_then(|b| ab_glyph::FontVec::try_from_vec(b).ok())
    })
    .as_ref()
}

/// True when text can be drawn on this Mac (the system font loaded).
pub fn text_available() -> bool {
    font().is_some()
}

/// Width and height in pixels of `text` at `size`, line by line.
pub fn text_extent(text: &str, size: f32) -> (f32, f32) {
    use ab_glyph::{Font, ScaleFont};
    let lines: Vec<&str> = text.split('\n').collect();
    let line_h = size * 1.2;
    let width = match font() {
        Some(f) => {
            let sf = f.as_scaled(ab_glyph::PxScale::from(size));
            lines.iter().map(|l| l.chars().map(|c| sf.h_advance(sf.glyph_id(c))).sum::<f32>()).fold(0.0, f32::max)
        }
        None => lines.iter().map(|l| l.chars().count() as f32 * size * 0.55).fold(0.0, f32::max),
    };
    (width, line_h * lines.len() as f32)
}

fn draw_text(buf: &mut [u8], w: usize, h: usize, at: P, text: &str, size: f32, c: [u8; 3]) {
    use ab_glyph::{Font, ScaleFont};
    let Some(f) = font() else { return };
    let scale = ab_glyph::PxScale::from(size);
    let sf = f.as_scaled(scale);
    for (li, line) in text.split('\n').enumerate() {
        let mut x = at.0;
        let base = at.1 + li as f32 * size * 1.2 + sf.ascent();
        for ch in line.chars() {
            let id = sf.glyph_id(ch);
            let glyph = id.with_scale_and_position(scale, ab_glyph::point(x, base));
            if let Some(o) = f.outline_glyph(glyph) {
                let b = o.px_bounds();
                o.draw(|gx, gy, cov| {
                    let (px, py) = (b.min.x as i64 + gx as i64, b.min.y as i64 + gy as i64);
                    if px >= 0 && py >= 0 && (px as usize) < w && (py as usize) < h {
                        blend(buf, w, px as usize, py as usize, c, cov);
                    }
                });
            }
            x += sf.h_advance(id);
        }
    }
}

fn seg_dist(p: P, a: P, b: P) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 { 0.0 } else { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0) };
    ((p.0 - (a.0 + t * dx)).powi(2) + (p.1 - (a.1 + t * dy)).powi(2)).sqrt()
}

fn bbox(a: P, b: P) -> (f32, f32, f32, f32) {
    (a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1))
}

fn number_radius(width: f32) -> f32 {
    (width * 4.0).max(11.0)
}

impl Doc {
    pub fn add(&mut self, shape: Shape, style: Style) {
        self.items.push(Item { shape, style });
    }

    /// Remove the newest item; false when there is nothing to undo.
    pub fn undo(&mut self) -> bool {
        self.items.pop().is_some()
    }

    pub fn remove(&mut self, i: usize) -> bool {
        if i < self.items.len() {
            self.items.remove(i);
            true
        } else {
            false
        }
    }

    /// Next number for a numbered label: one more than the highest in use.
    pub fn next_number(&self) -> u32 {
        self.items.iter().filter_map(|i| if let Shape::Number(_, n) = i.shape { Some(n) } else { None }).max().unwrap_or(0) + 1
    }

    /// Topmost item within `tol` pixels of `p`.
    pub fn hit(&self, p: P, tol: f32) -> Option<usize> {
        self.items.iter().enumerate().rev().find(|(_, it)| it.hits(p, tol)).map(|(i, _)| i)
    }

    pub fn move_item(&mut self, i: usize, dx: f32, dy: f32) {
        let Some(it) = self.items.get_mut(i) else { return };
        let mv = |p: &mut P| {
            p.0 += dx;
            p.1 += dy;
        };
        match &mut it.shape {
            Shape::Line(a, b) | Shape::Arrow(a, b) | Shape::Rect(a, b) | Shape::Ellipse(a, b) | Shape::Highlight(a, b) => {
                mv(a);
                mv(b);
            }
            Shape::Freehand(pts) => pts.iter_mut().for_each(mv),
            Shape::Number(c, _) | Shape::Magnifier { center: c, .. } | Shape::Text(c, _) => mv(c),
            Shape::Callout { at, tip, .. } => {
                mv(at);
                mv(tip);
            }
            Shape::Stamp { at, .. } => mv(at),
        }
    }

    /// The same drawing scaled by `k` (points, line widths, magnifier radius): used to preview on a
    /// downscaled copy of the picture.
    pub fn scaled(&self, k: f32) -> Doc {
        let sp = |p: P| (p.0 * k, p.1 * k);
        let items = self
            .items
            .iter()
            .map(|it| {
                let shape = match &it.shape {
                    Shape::Line(a, b) => Shape::Line(sp(*a), sp(*b)),
                    Shape::Arrow(a, b) => Shape::Arrow(sp(*a), sp(*b)),
                    Shape::Rect(a, b) => Shape::Rect(sp(*a), sp(*b)),
                    Shape::Ellipse(a, b) => Shape::Ellipse(sp(*a), sp(*b)),
                    Shape::Highlight(a, b) => Shape::Highlight(sp(*a), sp(*b)),
                    Shape::Freehand(pts) => Shape::Freehand(pts.iter().map(|p| sp(*p)).collect()),
                    Shape::Number(c, n) => Shape::Number(sp(*c), *n),
                    Shape::Text(c, t) => Shape::Text(sp(*c), t.clone()),
                    Shape::Callout { at, tip, text } => Shape::Callout { at: sp(*at), tip: sp(*tip), text: text.clone() },
                    Shape::Stamp { at, size, img } => Shape::Stamp { at: sp(*at), size: sp(*size), img: img.clone() },
                    Shape::Magnifier { center, radius, zoom } => Shape::Magnifier { center: sp(*center), radius: radius * k, zoom: *zoom },
                };
                Item { shape, style: Style { width: it.style.width * k, ..it.style } }
            })
            .collect();
        Doc { items }
    }

    /// Draw every item, in order, onto `buf` (RGBA8, `w × h`).
    pub fn render(&self, buf: &mut [u8], w: usize, h: usize) {
        assert_eq!(buf.len(), w * h * 4, "buffer does not match the size");
        for it in &self.items {
            it.render(buf, w, h);
        }
    }
}

impl Shape {
    /// Bounding box (x0, y0, x1, y1) of the shape, without stroke width.
    pub fn bounds(&self) -> (f32, f32, f32, f32) {
        self.bounds_with(Style::default().width)
    }

    /// Bounding box, using `width` (the stroke width) to size text.
    pub fn bounds_with(&self, width: f32) -> (f32, f32, f32, f32) {
        match self {
            Shape::Line(a, b) | Shape::Arrow(a, b) | Shape::Rect(a, b) | Shape::Ellipse(a, b) | Shape::Highlight(a, b) => bbox(*a, *b),
            Shape::Freehand(pts) => {
                pts.iter().fold((f32::MAX, f32::MAX, f32::MIN, f32::MIN), |m, p| (m.0.min(p.0), m.1.min(p.1), m.2.max(p.0), m.3.max(p.1)))
            }
            Shape::Number(c, _) => (c.0, c.1, c.0, c.1),
            Shape::Text(at, t) => {
                let (tw, th) = text_extent(t, text_size(width));
                (at.0, at.1, at.0 + tw, at.1 + th)
            }
            Shape::Stamp { at, size, .. } => (at.0, at.1, at.0 + size.0, at.1 + size.1),
            Shape::Callout { at, tip, text } => {
                let (tw, th) = text_extent(text, text_size(width));
                let pad = text_size(width) * 0.5;
                (at.0.min(tip.0), at.1.min(tip.1), (at.0 + tw + 2.0 * pad).max(tip.0), (at.1 + th + 2.0 * pad).max(tip.1))
            }
            Shape::Magnifier { center, radius, .. } => (center.0 - radius, center.1 - radius, center.0 + radius, center.1 + radius),
        }
    }
}

/// Radius of a numbered label drawn with this stroke width.
pub fn label_radius(width: f32) -> f32 {
    number_radius(width)
}

impl Item {
    fn hits(&self, p: P, tol: f32) -> bool {
        let t = tol + self.style.width / 2.0;
        match &self.shape {
            Shape::Line(a, b) | Shape::Arrow(a, b) => seg_dist(p, *a, *b) <= t,
            Shape::Rect(a, b) | Shape::Highlight(a, b) => {
                let (x0, y0, x1, y1) = bbox(*a, *b);
                let inside = p.0 >= x0 - t && p.0 <= x1 + t && p.1 >= y0 - t && p.1 <= y1 + t;
                let filled = self.style.fill || matches!(self.shape, Shape::Highlight(..));
                inside && (filled || !(p.0 > x0 + t && p.0 < x1 - t && p.1 > y0 + t && p.1 < y1 - t))
            }
            Shape::Ellipse(a, b) => {
                let (x0, y0, x1, y1) = bbox(*a, *b);
                let (rx, ry) = (((x1 - x0) / 2.0).max(0.5), ((y1 - y0) / 2.0).max(0.5));
                let d = (((p.0 - (x0 + x1) / 2.0) / rx).powi(2) + ((p.1 - (y0 + y1) / 2.0) / ry).powi(2)).sqrt();
                let slack = t / rx.min(ry);
                if self.style.fill {
                    d <= 1.0 + slack
                } else {
                    (d - 1.0).abs() <= slack
                }
            }
            Shape::Freehand(pts) => {
                pts.windows(2).any(|s| seg_dist(p, s[0], s[1]) <= t) || pts.len() == 1 && seg_dist(p, pts[0], pts[0]) <= t
            }
            Shape::Number(c, _) => seg_dist(p, *c, *c) <= number_radius(self.style.width) + tol,
            Shape::Text(at, t) => {
                let (tw, th) = text_extent(t, text_size(self.style.width));
                p.0 >= at.0 - tol && p.0 <= at.0 + tw + tol && p.1 >= at.1 - tol && p.1 <= at.1 + th + tol
            }
            Shape::Stamp { at, size, .. } => {
                p.0 >= at.0 - tol && p.0 <= at.0 + size.0 + tol && p.1 >= at.1 - tol && p.1 <= at.1 + size.1 + tol
            }
            Shape::Callout { at, tip, text } => {
                let (x0, y0, x1, y1) = self.shape.bounds_with(self.style.width);
                let _ = (at, text);
                (p.0 >= x0 - tol && p.0 <= x1 + tol && p.1 >= y0 - tol && p.1 <= y1 + tol)
                    && (seg_dist(p, *tip, *tip) <= tol || {
                        let size = text_size(self.style.width);
                        let (tw, th) = text_extent(text, size);
                        p.0 >= at.0 - tol && p.0 <= at.0 + tw + size + tol && p.1 >= at.1 - tol && p.1 <= at.1 + th + size + tol
                    })
            }
            Shape::Magnifier { center, radius, .. } => seg_dist(p, *center, *center) <= radius + tol,
        }
    }

    fn render(&self, buf: &mut [u8], w: usize, h: usize) {
        let (c, wd) = (self.style.color, self.style.width.max(1.0));
        match &self.shape {
            Shape::Line(a, b) => segment(buf, w, h, *a, *b, wd, c, 1.0),
            Shape::Arrow(a, b) => {
                segment(buf, w, h, *a, *b, wd, c, 1.0);
                let ang = (a.1 - b.1).atan2(a.0 - b.0);
                let head = (wd * 5.0).max(12.0).min(seg_dist(*a, *b, *b).max(1.0));
                for s in [-0.5f32, 0.5] {
                    let tip = (b.0 + head * (ang + s).cos(), b.1 + head * (ang + s).sin());
                    segment(buf, w, h, *b, tip, wd, c, 1.0);
                }
            }
            Shape::Rect(a, b) => {
                let (x0, y0, x1, y1) = bbox(*a, *b);
                if self.style.fill {
                    fill_rect(buf, w, h, (x0, y0, x1, y1), c, 1.0);
                } else {
                    for (p, q) in [((x0, y0), (x1, y0)), ((x1, y0), (x1, y1)), ((x1, y1), (x0, y1)), ((x0, y1), (x0, y0))] {
                        segment(buf, w, h, p, q, wd, c, 1.0);
                    }
                }
            }
            Shape::Highlight(a, b) => fill_rect(buf, w, h, bbox(*a, *b), c, 0.4),
            Shape::Ellipse(a, b) => {
                let (x0, y0, x1, y1) = bbox(*a, *b);
                let (cx, cy, rx, ry) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0, ((x1 - x0) / 2.0).max(0.5), ((y1 - y0) / 2.0).max(0.5));
                for_pixels(w, h, (x0 - wd, y0 - wd, x1 + wd, y1 + wd), |x, y| {
                    let d = (((x - cx) / rx).powi(2) + ((y - cy) / ry).powi(2)).sqrt();
                    let off = (d - 1.0) * rx.min(ry);
                    let cov = if self.style.fill { (0.5 - off).clamp(0.0, 1.0) } else { (wd / 2.0 + 0.5 - off.abs()).clamp(0.0, 1.0) };
                    (cov > 0.0).then_some(cov)
                })
                .into_iter()
                .for_each(|(x, y, cov)| blend(buf, w, x, y, c, cov));
            }
            Shape::Freehand(pts) => match pts.as_slice() {
                [] => {}
                [p] => segment(buf, w, h, *p, *p, wd, c, 1.0),
                _ => pts.windows(2).for_each(|s| segment(buf, w, h, s[0], s[1], wd, c, 1.0)),
            },
            Shape::Number(center, n) => {
                let r = number_radius(self.style.width);
                for_pixels(w, h, (center.0 - r - 1.0, center.1 - r - 1.0, center.0 + r + 1.0, center.1 + r + 1.0), |x, y| {
                    let cov = (r + 0.5 - ((x - center.0).powi(2) + (y - center.1).powi(2)).sqrt()).clamp(0.0, 1.0);
                    (cov > 0.0).then_some(cov)
                })
                .into_iter()
                .for_each(|(x, y, cov)| blend(buf, w, x, y, c, cov));
                digits(buf, w, h, *center, *n, r);
            }
            Shape::Text(at, t) => draw_text(buf, w, h, *at, t, text_size(self.style.width), c),
            Shape::Stamp { at, size, img } => stamp(buf, w, h, *at, *size, img),
            Shape::Callout { at, tip, text } => {
                let size = text_size(self.style.width);
                let (tw, th) = text_extent(text, size);
                let pad = size * 0.5;
                let (x0, y0, x1, y1) = (at.0, at.1, at.0 + tw + 2.0 * pad, at.1 + th + 2.0 * pad);
                // pointer from the box centre to the tip, drawn under the box so it starts at its edge
                segment(buf, w, h, ((x0 + x1) / 2.0, (y0 + y1) / 2.0), *tip, wd.min(size / 3.0).max(2.0), c, 1.0);
                fill_rect(buf, w, h, (x0, y0, x1, y1), c, 1.0);
                let lum = 0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32;
                let ink = if lum > 150.0 { [20, 20, 20] } else { [255, 255, 255] };
                draw_text(buf, w, h, (x0 + pad, y0 + pad), text, size, ink);
            }
            Shape::Magnifier { center, radius, zoom } => {
                let src = buf.to_vec();
                let z = zoom.max(1.0);
                let ring = wd.min(*radius);
                for_pixels(
                    w,
                    h,
                    (center.0 - radius - 1.0, center.1 - radius - 1.0, center.0 + radius + 1.0, center.1 + radius + 1.0),
                    |x, y| {
                        let d = ((x - center.0).powi(2) + (y - center.1).powi(2)).sqrt();
                        (d <= radius + 0.5).then_some(d)
                    },
                )
                .into_iter()
                .for_each(|(x, y, d)| {
                    let (sx, sy) = (center.0 + (x as f32 + 0.5 - center.0) / z, center.1 + (y as f32 + 0.5 - center.1) / z);
                    let (sx, sy) =
                        ((sx.floor() as i64).clamp(0, w as i64 - 1) as usize, (sy.floor() as i64).clamp(0, h as i64 - 1) as usize);
                    let from = &src[(sy * w + sx) * 4..(sy * w + sx) * 4 + 4];
                    let i = (y * w + x) * 4;
                    let edge = (radius + 0.5 - d).clamp(0.0, 1.0);
                    if d >= radius - ring {
                        blend(buf, w, x, y, c, edge);
                    } else {
                        buf[i..i + 4].copy_from_slice(from);
                    }
                });
            }
        }
    }
}

/// Pixels of `rect` (clamped to the image) for which `f(center_x, center_y)` returns a value.
fn for_pixels<T>(w: usize, h: usize, rect: (f32, f32, f32, f32), mut f: impl FnMut(f32, f32) -> Option<T>) -> Vec<(usize, usize, T)> {
    let (x0, y0) = (rect.0.floor().max(0.0) as usize, rect.1.floor().max(0.0) as usize);
    let (x1, y1) = ((rect.2.ceil().max(0.0) as usize).min(w), (rect.3.ceil().max(0.0) as usize).min(h));
    let mut out = vec![];
    for y in y0..y1 {
        for x in x0..x1 {
            if let Some(v) = f(x as f32 + 0.5, y as f32 + 0.5) {
                out.push((x, y, v));
            }
        }
    }
    out
}

/// Scaled blit with alpha: each destination pixel samples the nearest source pixel.
fn stamp(buf: &mut [u8], w: usize, h: usize, at: P, size: P, img: &Img) {
    let d = &img.0;
    if !(size.0 >= 1.0 && size.1 >= 1.0 && size.0.is_finite() && size.1.is_finite() && at.0.is_finite() && at.1.is_finite()) {
        return;
    }
    for (x, y, (sx, sy)) in for_pixels(w, h, (at.0, at.1, at.0 + size.0, at.1 + size.1), |px, py| {
        let (u, v) = ((px - at.0) / size.0, (py - at.1) / size.1);
        ((0.0..1.0).contains(&u) && (0.0..1.0).contains(&v)).then_some(((u * d.w as f32) as usize, (v * d.h as f32) as usize))
    }) {
        let i = (sy.min(d.h - 1) * d.w + sx.min(d.w - 1)) * 4;
        let a = d.rgba[i + 3] as f32 / 255.0;
        if a > 0.0 {
            blend(buf, w, x, y, [d.rgba[i], d.rgba[i + 1], d.rgba[i + 2]], a);
        }
    }
}

fn blend(buf: &mut [u8], w: usize, x: usize, y: usize, c: [u8; 3], a: f32) {
    let i = (y * w + x) * 4;
    for k in 0..3 {
        buf[i + k] = (c[k] as f32 * a + buf[i + k] as f32 * (1.0 - a)).round() as u8;
    }
    buf[i + 3] = buf[i + 3].max((a * 255.0).round() as u8);
}

#[allow(clippy::too_many_arguments)]
fn segment(buf: &mut [u8], w: usize, h: usize, a: P, b: P, wd: f32, c: [u8; 3], alpha: f32) {
    let m = wd / 2.0 + 1.0;
    let (x0, y0, x1, y1) = bbox(a, b);
    for (x, y, cov) in for_pixels(w, h, (x0 - m, y0 - m, x1 + m, y1 + m), |px, py| {
        let cov = (wd / 2.0 + 0.5 - seg_dist((px, py), a, b)).clamp(0.0, 1.0);
        (cov > 0.0).then_some(cov)
    }) {
        blend(buf, w, x, y, c, cov * alpha);
    }
}

fn fill_rect(buf: &mut [u8], w: usize, h: usize, r: (f32, f32, f32, f32), c: [u8; 3], alpha: f32) {
    for (x, y, _) in for_pixels(w, h, r, |px, py| (px >= r.0 && px <= r.2 && py >= r.1 && py <= r.3).then_some(())) {
        blend(buf, w, x, y, c, alpha);
    }
}

const FONT: [&str; 10] = [
    "111101101101111",
    "010110010010111",
    "111001111100111",
    "111001111001111",
    "101101111001001",
    "111100111001111",
    "111100111101111",
    "111001001001001",
    "111101111101111",
    "111101111001111",
];

/// White digits centred in a circle of radius `r`.
fn digits(buf: &mut [u8], w: usize, h: usize, center: P, n: u32, r: f32) {
    let text = n.to_string();
    let count = text.len() as f32;
    // each digit is 3 cells wide plus a one-cell gap, 5 tall
    let cell = ((r * 1.1) / (count * 4.0 - 1.0).max(5.0 / 2.0 * 2.0)).max(1.0).min(r / 3.0).floor().max(1.0);
    let total_w = (count * 4.0 - 1.0) * cell;
    let (left, top) = (center.0 - total_w / 2.0, center.1 - 2.5 * cell);
    for (di, ch) in text.chars().enumerate() {
        let glyph = FONT[ch.to_digit(10).unwrap_or(0) as usize].as_bytes();
        for (i, bit) in glyph.iter().enumerate() {
            if *bit == b'1' {
                let (gx, gy) = ((i % 3) as f32, (i / 3) as f32);
                fill_rect(
                    buf,
                    w,
                    h,
                    (
                        left + (di as f32 * 4.0 + gx) * cell,
                        top + gy * cell,
                        left + (di as f32 * 4.0 + gx + 1.0) * cell,
                        top + (gy + 1.0) * cell,
                    ),
                    [255, 255, 255],
                    1.0,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canvas(w: usize, h: usize) -> Vec<u8> {
        vec![255; w * h * 4]
    }

    fn px(b: &[u8], w: usize, x: usize, y: usize) -> [u8; 4] {
        let i = (y * w + x) * 4;
        [b[i], b[i + 1], b[i + 2], b[i + 3]]
    }

    const RED: Style = Style { color: [255, 0, 0], width: 4.0, fill: false };

    #[test]
    fn line_and_arrow_color_their_path_only() {
        let mut d = Doc::default();
        d.add(Shape::Arrow((10.0, 50.0), (90.0, 50.0)), RED);
        let mut b = canvas(100, 100);
        d.render(&mut b, 100, 100);
        assert_eq!(px(&b, 100, 50, 50), [255, 0, 0, 255], "shaft");
        assert_eq!(px(&b, 100, 50, 90), [255, 255, 255, 255], "far from it untouched");
        let head = (80..90).any(|x| px(&b, 100, x, 44) == [255, 0, 0, 255] || px(&b, 100, x, 56) == [255, 0, 0, 255]);
        assert!(head, "arrow head has barbs off the shaft");
        let mut l = Doc::default();
        l.add(Shape::Line((10.0, 10.0), (10.0, 10.0)), RED);
        let mut b = canvas(20, 20);
        l.render(&mut b, 20, 20);
        assert_eq!(px(&b, 20, 10, 10), [255, 0, 0, 255], "a zero-length line is a dot");
    }

    #[test]
    fn boxes_outline_or_fill() {
        let mut b = canvas(60, 60);
        let mut d = Doc::default();
        d.add(Shape::Rect((10.0, 10.0), (50.0, 50.0)), RED);
        d.render(&mut b, 60, 60);
        assert_eq!(px(&b, 60, 10, 30), [255, 0, 0, 255], "edge");
        assert_eq!(px(&b, 60, 30, 30), [255, 255, 255, 255], "inside stays");
        let mut f = Doc::default();
        f.add(Shape::Rect((50.0, 50.0), (10.0, 10.0)), Style { fill: true, ..RED });
        let mut b = canvas(60, 60);
        f.render(&mut b, 60, 60);
        assert_eq!(px(&b, 60, 30, 30), [255, 0, 0, 255], "corners in any order, filled");
    }

    #[test]
    fn ellipse_outline_and_fill() {
        let mut b = canvas(100, 60);
        let mut d = Doc::default();
        d.add(Shape::Ellipse((10.0, 10.0), (90.0, 50.0)), RED);
        d.render(&mut b, 100, 60);
        assert_eq!(px(&b, 100, 50, 10), [255, 0, 0, 255], "top of the ring");
        assert_eq!(px(&b, 100, 50, 30), [255, 255, 255, 255], "centre empty");
        assert_eq!(px(&b, 100, 11, 11), [255, 255, 255, 255], "bounding-box corner empty");
        d.items[0].style.fill = true;
        let mut b = canvas(100, 60);
        d.render(&mut b, 100, 60);
        assert_eq!(px(&b, 100, 50, 30), [255, 0, 0, 255]);
    }

    #[test]
    fn highlight_is_translucent() {
        let mut d = Doc::default();
        d.add(Shape::Highlight((0.0, 0.0), (20.0, 20.0)), Style { color: [255, 255, 0], ..Style::default() });
        let mut b = canvas(30, 30);
        b[(10 * 30 + 10) * 4 + 2] = 100; // a darker blue channel shows through
        d.render(&mut b, 30, 30);
        let p = px(&b, 30, 10, 10);
        assert!(p[2] > 40 && p[2] < 100, "yellow over 100 blue keeps some: {p:?}");
        assert_eq!(px(&b, 30, 25, 25), [255, 255, 255, 255]);
    }

    #[test]
    fn numbered_label_draws_white_digits_on_a_disc() {
        for n in [1u32, 8, 12] {
            let mut d = Doc::default();
            d.add(Shape::Number((30.0, 30.0), n), RED);
            let mut b = canvas(60, 60);
            d.render(&mut b, 60, 60);
            assert_eq!(px(&b, 60, 30 - 9, 30), [255, 0, 0, 255], "disc for {n}");
            let reds = (0..60).flat_map(|y| (0..60).map(move |x| (x, y))).filter(|&(x, y)| px(&b, 60, x, y) == [255, 0, 0, 255]).count();
            let white_inside =
                (21..40).flat_map(|y| (21..40).map(move |x| (x, y))).filter(|&(x, y)| px(&b, 60, x, y) == [255, 255, 255, 255]).count();
            assert!(reds > 200, "disc area for {n}: {reds}");
            assert!(white_inside > 4, "digits visible for {n}");
        }
        let mut d = Doc::default();
        d.add(Shape::Number((10.0, 10.0), 1), RED);
        d.add(Shape::Number((30.0, 10.0), 4), RED);
        assert_eq!(d.next_number(), 5);
        assert_eq!(Doc::default().next_number(), 1);
    }

    #[test]
    fn magnifier_shows_the_centre_zoomed() {
        // black left of x=45, white right of it; a 2x lens centred at x=50 pushes that boundary to x=40
        let (w, h) = (80, 80);
        let mut b = vec![255u8; w * h * 4];
        for y in 0..h {
            for x in 0..45 {
                b[(y * w + x) * 4..(y * w + x) * 4 + 3].copy_from_slice(&[0, 0, 0]);
            }
        }
        let mut d = Doc::default();
        d.add(Shape::Magnifier { center: (50.0, 40.0), radius: 20.0, zoom: 2.0 }, Style { width: 2.0, ..Style::default() });
        d.render(&mut b, w, h);
        // source boundary at x=45; inside the lens it lands at 50 + (45-50)*2 = 40
        assert_eq!(px(&b, w, 34, 40)[0], 0, "left of the zoomed boundary is black");
        assert_eq!(px(&b, w, 46, 40)[0], 255, "right of it is white");
        assert_eq!(px(&b, w, 50, 21), [255, 59, 48, 255], "ring in the default colour");
        assert_eq!(px(&b, w, 5, 5), [0, 0, 0, 255], "outside untouched");
    }

    #[test]
    fn freehand_follows_its_points() {
        let mut d = Doc::default();
        d.add(Shape::Freehand(vec![(5.0, 5.0), (5.0, 25.0), (25.0, 25.0)]), RED);
        let mut b = canvas(40, 40);
        d.render(&mut b, 40, 40);
        assert_eq!(px(&b, 40, 5, 15), [255, 0, 0, 255]);
        assert_eq!(px(&b, 40, 15, 25), [255, 0, 0, 255]);
        assert_eq!(px(&b, 40, 20, 10), [255, 255, 255, 255]);
        let mut one = Doc::default();
        one.add(Shape::Freehand(vec![]), RED);
        one.add(Shape::Freehand(vec![(3.0, 3.0)]), RED);
        one.render(&mut canvas(10, 10), 10, 10);
    }

    #[test]
    fn drawing_outside_the_image_is_clipped_not_a_panic() {
        let mut d = Doc::default();
        d.add(Shape::Arrow((-50.0, -50.0), (500.0, 500.0)), RED);
        d.add(Shape::Rect((-10.0, -10.0), (1e6, 1e6)), Style { fill: true, ..RED });
        d.add(Shape::Magnifier { center: (-5.0, 200.0), radius: 30.0, zoom: 3.0 }, RED);
        d.add(Shape::Number((-3.0, 99.0), 7), RED);
        d.add(Shape::Ellipse((f32::MAX / 4.0, 0.0), (0.0, 10.0)), RED);
        let mut b = canvas(30, 30);
        d.render(&mut b, 30, 30);
    }

    #[test]
    fn drags_become_shapes_and_strays_do_not() {
        let drag = [(10.0, 10.0), (60.0, 40.0)];
        assert_eq!(build(Tool::Arrow, &drag, 1), Some(Shape::Arrow((10.0, 10.0), (60.0, 40.0))));
        assert_eq!(build(Tool::Box, &drag, 1), Some(Shape::Rect((10.0, 10.0), (60.0, 40.0))));
        assert_eq!(build(Tool::Number, &[(5.0, 5.0)], 3), Some(Shape::Number((5.0, 5.0), 3)), "a click places a number");
        assert_eq!(build(Tool::Arrow, &[(5.0, 5.0), (6.0, 5.0)], 1), None, "a 1 px drag is a click");
        assert_eq!(build(Tool::Select, &drag, 1), None);
        assert_eq!(build(Tool::Pen, &[(1.0, 1.0)], 1), None);
        assert_eq!(build(Tool::Pen, &[(1.0, 1.0), (30.0, 30.0)], 1), Some(Shape::Freehand(vec![(1.0, 1.0), (30.0, 30.0)])));
        assert_eq!(build(Tool::Arrow, &[], 1), None);
        match build(Tool::Magnifier, &[(50.0, 50.0), (80.0, 50.0)], 1) {
            Some(Shape::Magnifier { center, radius, zoom }) => assert_eq!((center, radius, zoom), ((50.0, 50.0), 30.0, 2.5)),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn text_draws_dark_pixels_inside_its_extent_only() {
        if !text_available() {
            eprintln!("skipped: no system font");
            return;
        }
        let style = Style { color: [0, 0, 0], width: 6.0, fill: false };
        let mut d = Doc::default();
        d.add(Shape::Text((10.0, 10.0), "Hello\nWorld".into()), style);
        let mut b = canvas(200, 120);
        d.render(&mut b, 200, 120);
        let size = text_size(6.0);
        let (tw, th) = text_extent("Hello\nWorld", size);
        assert!(tw > size * 2.0 && th > size * 2.0, "two lines are taller than one: {tw}x{th}");
        let dark = |x0: usize, y0: usize, x1: usize, y1: usize| {
            (y0..y1).flat_map(|y| (x0..x1).map(move |x| (x, y))).filter(|&(x, y)| px(&b, 200, x, y)[0] < 128).count()
        };
        assert!(dark(10, 10, 10 + tw as usize + 2, 10 + th as usize + 2) > 60, "ink inside the extent");
        assert_eq!(dark(0, 0, 200, 8), 0, "nothing above the text");
        assert_eq!(dark(10 + tw as usize + 6, 0, 200, 120), 0, "nothing to the right");
        assert!(dark(10, 10 + (th / 2.0) as usize, 10 + tw as usize + 2, 10 + th as usize + 2) > 20, "second line");
        assert_eq!(d.hit((12.0, 12.0), 2.0), Some(0), "inside the box is a hit");
        assert_eq!(d.hit((190.0, 110.0), 2.0), None);
        assert_eq!(d.items[0].shape.bounds_with(6.0), (10.0, 10.0, 10.0 + tw, 10.0 + th));
        d.move_item(0, 5.0, 5.0);
        assert_eq!(d.items[0].shape, Shape::Text((15.0, 15.0), "Hello\nWorld".into()));
        assert_eq!(d.scaled(0.5).items[0].style.width, 3.0, "text size follows the scale");
        let mut e = Doc::default();
        e.add(Shape::Text((-50.0, 500.0), String::new()), style);
        e.add(Shape::Text((-5.0, -5.0), "clipped é ñ 日本".into()), style);
        e.render(&mut canvas(30, 30), 30, 30);
    }

    #[test]
    fn callout_has_a_box_with_text_and_a_pointer() {
        if !text_available() {
            return;
        }
        let style = Style { color: [0, 0, 0], width: 6.0, fill: false };
        let mut d = Doc::default();
        d.add(Shape::Callout { at: (80.0, 10.0), tip: (20.0, 90.0), text: "Look".into() }, style);
        let mut b = canvas(200, 120);
        d.render(&mut b, 200, 120);
        let size = text_size(6.0);
        let (tw, th) = text_extent("Look", size);
        // the box is filled with the colour and the text is drawn in the contrasting ink
        assert_eq!(px(&b, 200, 82, 12)[..3], [0, 0, 0], "box corner");
        let white = (80..80 + (tw + size) as usize)
            .flat_map(|x| (10..10 + (th + size) as usize).map(move |y| (x, y)))
            .filter(|&(x, y)| px(&b, 200, x, y)[0] > 200)
            .count();
        assert!(white > 30, "light text on the dark box: {white}");
        // the pointer reaches the tip
        assert_eq!(px(&b, 200, 20, 90)[..3], [0, 0, 0], "tip");
        assert_eq!(px(&b, 200, 150, 100), [255, 255, 255, 255], "far away untouched");
        assert_eq!(d.hit((20.0, 90.0), 3.0), Some(0));
        assert_eq!(d.hit((85.0, 15.0), 2.0), Some(0));
        assert_eq!(d.hit((190.0, 110.0), 2.0), None);
        d.move_item(0, 10.0, 0.0);
        assert!(matches!(&d.items[0].shape, Shape::Callout { at, tip, .. } if *at == (90.0, 10.0) && *tip == (30.0, 90.0)));
        assert_eq!(d.scaled(0.5).items[0].style.width, 3.0);
        // a light colour gets dark text
        let mut l = Doc::default();
        l.add(
            Shape::Callout { at: (10.0, 10.0), tip: (10.0, 10.0), text: "Hi".into() },
            Style { color: [255, 214, 10], width: 6.0, fill: false },
        );
        let mut b = canvas(100, 60);
        l.render(&mut b, 100, 60);
        let darkest = (10..60).flat_map(|x| (10..50).map(move |y| (x, y))).map(|(x, y)| px(&b, 100, x, y)[2]).min().unwrap();
        assert!(darkest < 60, "dark ink on a light box");
        l.add(Shape::Callout { at: (-40.0, 500.0), tip: (-9.0, -9.0), text: String::new() }, style);
        l.render(&mut canvas(30, 30), 30, 30);
    }

    #[test]
    fn stamp_scales_blends_and_stays_inside() {
        // 2x2 source: red, green / blue, transparent
        let src = Img::new(2, 2, vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 9, 9, 9, 0]).unwrap();
        let mut d = Doc::default();
        d.add(Shape::Stamp { at: (10.0, 10.0), size: (20.0, 20.0), img: src.clone() }, Style::default());
        let mut b = canvas(40, 40);
        d.render(&mut b, 40, 40);
        assert_eq!(px(&b, 40, 12, 12)[..3], [255, 0, 0], "top-left quadrant");
        assert_eq!(px(&b, 40, 27, 12)[..3], [0, 255, 0], "top-right quadrant");
        assert_eq!(px(&b, 40, 12, 27)[..3], [0, 0, 255], "bottom-left quadrant");
        assert_eq!(px(&b, 40, 27, 27), [255, 255, 255, 255], "transparent source pixel leaves the picture");
        assert_eq!(px(&b, 40, 5, 5), [255, 255, 255, 255], "outside untouched");
        let half = Img::new(1, 1, vec![0, 0, 0, 128]).unwrap();
        let mut e = Doc::default();
        e.add(Shape::Stamp { at: (0.0, 0.0), size: (5.0, 5.0), img: half }, Style::default());
        let mut b = canvas(10, 10);
        e.render(&mut b, 10, 10);
        assert!((120..136).contains(&px(&b, 10, 2, 2)[0]), "50% black over white: {:?}", px(&b, 10, 2, 2));
        assert_eq!(d.hit((15.0, 15.0), 1.0), Some(0));
        assert_eq!(d.hit((35.0, 35.0), 1.0), None);
        assert_eq!(d.items[0].shape.bounds(), (10.0, 10.0, 30.0, 30.0));
        d.move_item(0, 5.0, 0.0);
        assert!(matches!(&d.items[0].shape, Shape::Stamp { at, .. } if *at == (15.0, 10.0)));
        let s = d.scaled(0.5);
        assert!(matches!(&s.items[0].shape, Shape::Stamp { at, size, img } if *at == (7.5, 5.0) && *size == (10.0, 10.0) && *img == src));
        let mut f = Doc::default();
        f.add(Shape::Stamp { at: (-30.0, -30.0), size: (60.0, 60.0), img: src.clone() }, Style::default());
        f.add(Shape::Stamp { at: (5.0, 5.0), size: (0.0, f32::NAN), img: src }, Style::default());
        f.render(&mut canvas(20, 20), 20, 20);
        assert!(Img::new(2, 2, vec![0; 3]).is_none() && Img::new(0, 1, vec![]).is_none());
    }

    #[test]
    fn bounds_cover_each_shape() {
        assert_eq!(Shape::Rect((50.0, 10.0), (10.0, 40.0)).bounds(), (10.0, 10.0, 50.0, 40.0));
        assert_eq!(Shape::Freehand(vec![(5.0, 9.0), (1.0, 20.0), (7.0, 2.0)]).bounds(), (1.0, 2.0, 7.0, 20.0));
        assert_eq!(Shape::Magnifier { center: (20.0, 20.0), radius: 5.0, zoom: 2.0 }.bounds(), (15.0, 15.0, 25.0, 25.0));
        assert_eq!(Shape::Number((3.0, 4.0), 1).bounds(), (3.0, 4.0, 3.0, 4.0));
        assert_eq!(label_radius(1.0), 11.0);
        assert_eq!(label_radius(5.0), 20.0);
    }

    #[test]
    fn scaling_keeps_the_picture_in_proportion() {
        let mut d = Doc::default();
        d.add(Shape::Rect((100.0, 100.0), (300.0, 200.0)), Style { width: 8.0, ..RED });
        d.add(Shape::Magnifier { center: (200.0, 200.0), radius: 40.0, zoom: 2.0 }, RED);
        let s = d.scaled(0.5);
        assert_eq!(s.items[0].shape, Shape::Rect((50.0, 50.0), (150.0, 100.0)));
        assert_eq!(s.items[0].style.width, 4.0);
        assert_eq!(s.items[1].shape, Shape::Magnifier { center: (100.0, 100.0), radius: 20.0, zoom: 2.0 });
        assert_eq!(d.scaled(1.0), d);
    }

    #[test]
    fn hit_move_undo_remove() {
        let mut d = Doc::default();
        d.add(Shape::Rect((10.0, 10.0), (50.0, 50.0)), RED);
        d.add(Shape::Arrow((0.0, 80.0), (100.0, 80.0)), RED);
        assert_eq!(d.hit((10.0, 30.0), 2.0), Some(0), "on the outline");
        assert_eq!(d.hit((30.0, 30.0), 2.0), None, "inside an unfilled box is not a hit");
        assert_eq!(d.hit((50.0, 81.0), 2.0), Some(1));
        assert_eq!(d.hit((90.0, 10.0), 2.0), None);
        d.items[0].style.fill = true;
        assert_eq!(d.hit((30.0, 30.0), 2.0), Some(0), "inside a filled box is");
        d.move_item(0, 5.0, -5.0);
        assert_eq!(d.items[0].shape, Shape::Rect((15.0, 5.0), (55.0, 45.0)));
        d.move_item(9, 1.0, 1.0);
        assert!(d.undo());
        assert_eq!(d.items.len(), 1);
        assert!(!d.remove(3));
        assert!(d.remove(0));
        assert!(!d.undo(), "nothing left to undo");
    }
}
