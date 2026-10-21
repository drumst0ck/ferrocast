//! Collage layouts (Tangerine "Make a collage"): grid, row, column and featured, with spacing,
//! final width, corner radius and background. Pure geometry; pixels are composed by the app.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    Grid,
    Row,
    Column,
    /// First picture large, the rest in two rows beside it.
    Featured,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CollageOpts {
    pub layout: Layout,
    /// Final canvas width in pixels; `None` keeps 600 px cells.
    pub width: Option<u32>,
    /// Spacing between pictures and around the edge.
    pub gap: u32,
    /// Corner radius of each picture.
    pub radius: u32,
    /// Background colour; `None` is transparent.
    pub bg: Option<[u8; 3]>,
}

impl Default for CollageOpts {
    fn default() -> Self {
        CollageOpts { layout: Layout::Grid, width: None, gap: 12, radius: 0, bg: Some([255, 255, 255]) }
    }
}

/// A picture's rectangle on the canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// Canvas size and one cell per picture for a given cell edge.
pub fn layout_at(layout: Layout, n: usize, cell: u32, gap: u32) -> ((u32, u32), Vec<Cell>) {
    if n == 0 {
        return ((0, 0), vec![]);
    }
    let sq = |c: u32, r: u32| Cell { x: gap + c * (cell + gap), y: gap + r * (cell + gap), w: cell, h: cell };
    let n32 = n as u32;
    match layout {
        Layout::Grid => {
            let (cols, rows) = crate::convert::collage_grid(n);
            let cells = (0..n).map(|i| sq((i % cols) as u32, (i / cols) as u32)).collect();
            ((cols as u32 * (cell + gap) + gap, rows as u32 * (cell + gap) + gap), cells)
        }
        Layout::Row => ((n32 * (cell + gap) + gap, cell + 2 * gap), (0..n32).map(|i| sq(i, 0)).collect()),
        Layout::Column => ((cell + 2 * gap, n32 * (cell + gap) + gap), (0..n32).map(|i| sq(0, i)).collect()),
        Layout::Featured => {
            let big = 2 * cell + gap;
            let mut cells = vec![Cell { x: gap, y: gap, w: big, h: big }];
            let rest = n32 - 1;
            let cols = rest.div_ceil(2);
            for i in 0..rest {
                cells.push(Cell { x: 2 * gap + big + (i / 2) * (cell + gap), y: gap + (i % 2) * (cell + gap), w: cell, h: cell });
            }
            let width = if rest == 0 { big + 2 * gap } else { 2 * gap + big + cols * (cell + gap) };
            ((width, big + 2 * gap), cells)
        }
    }
}

/// Layout for the options: the cell edge is solved so the canvas is `opts.width` wide.
pub fn layout(n: usize, o: &CollageOpts) -> ((u32, u32), Vec<Cell>) {
    let cell = match o.width {
        None => 600,
        Some(target) => {
            let w0 = layout_at(o.layout, n, 0, o.gap).0 .0 as f64;
            let w1 = layout_at(o.layout, n, 1000, o.gap).0 .0 as f64;
            let k = (w1 - w0) / 1000.0;
            if k <= 0.0 {
                600
            } else {
                (((target as f64 - w0) / k).floor() as i64).clamp(8, 4000) as u32
            }
        }
    };
    layout_at(o.layout, n, cell, o.gap)
}

/// Coverage (0.0–1.0) of pixel (x, y) inside a `w × h` rounded rectangle of radius `r`.
pub fn corner_alpha(x: u32, y: u32, w: u32, h: u32, r: u32) -> f32 {
    let r = r.min(w / 2).min(h / 2) as f32;
    if r <= 0.0 {
        return 1.0;
    }
    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
    let cx = px.clamp(r, w as f32 - r);
    let cy = py.clamp(r, h as f32 - r);
    let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
    (r + 0.5 - d).clamp(0.0, 1.0)
}

/// Typed options: `collage row 1600 gap 16 radius 12 bg 000000` (words in any order; `bg none`
/// is transparent; a bare number is the final width).
pub fn parse(q: &str) -> Option<CollageOpts> {
    let q = q.trim().to_lowercase();
    let rest = q.strip_prefix("collage")?;
    if !(rest.is_empty() || rest.starts_with(' ')) {
        return None;
    }
    let mut o = CollageOpts::default();
    let mut it = rest.split_whitespace();
    while let Some(w) = it.next() {
        match w {
            "grid" => o.layout = Layout::Grid,
            "row" => o.layout = Layout::Row,
            "column" | "col" => o.layout = Layout::Column,
            "featured" => o.layout = Layout::Featured,
            "gap" => o.gap = it.next()?.parse().ok().filter(|g| *g <= 200)?,
            "radius" => o.radius = it.next()?.parse().ok().filter(|r| *r <= 1000)?,
            "bg" => {
                let v = it.next()?;
                o.bg = if v == "none" { None } else { Some(hex(v)?) };
            }
            n => o.width = Some(n.parse().ok().filter(|w| (64..=16_000).contains(w))?),
        }
    }
    Some(o)
}

/// `RRGGBB` (optionally `#`-prefixed).
pub fn hex(v: &str) -> Option<[u8; 3]> {
    let v = v.trim_start_matches('#');
    if v.len() != 6 || !v.is_ascii() {
        return None;
    }
    let p = |i: usize| u8::from_str_radix(&v[i..i + 2], 16).ok();
    Some([p(0)?, p(2)?, p(4)?])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inside(c: &Cell, size: (u32, u32)) -> bool {
        c.x + c.w <= size.0 && c.y + c.h <= size.1
    }

    fn disjoint(cells: &[Cell]) -> bool {
        cells
            .iter()
            .enumerate()
            .all(|(i, a)| cells.iter().skip(i + 1).all(|b| a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y))
    }

    #[test]
    fn every_layout_fits_the_canvas_without_overlap() {
        for l in [Layout::Grid, Layout::Row, Layout::Column, Layout::Featured] {
            for n in 1..=9 {
                let (size, cells) = layout_at(l, n, 100, 10);
                assert_eq!(cells.len(), n, "{l:?} {n}");
                assert!(cells.iter().all(|c| inside(c, size)), "{l:?} {n} {size:?} {cells:?}");
                assert!(disjoint(&cells), "{l:?} {n} overlap");
            }
        }
    }

    #[test]
    fn exact_shapes() {
        assert_eq!(layout_at(Layout::Row, 3, 100, 10).0, (340, 120));
        assert_eq!(layout_at(Layout::Column, 3, 100, 10).0, (120, 340));
        assert_eq!(layout_at(Layout::Grid, 4, 100, 10).0, (230, 230));
        let (size, cells) = layout_at(Layout::Featured, 5, 100, 10);
        assert_eq!(cells[0], Cell { x: 10, y: 10, w: 210, h: 210 });
        assert_eq!(size, (10 + 210 + 10 + 2 * 110, 230));
        assert_eq!(layout_at(Layout::Featured, 1, 100, 10).0, (230, 230));
        assert_eq!(layout_at(Layout::Grid, 0, 100, 10), ((0, 0), vec![]));
    }

    #[test]
    fn width_option_hits_the_target() {
        for l in [Layout::Grid, Layout::Row, Layout::Column, Layout::Featured] {
            let o = CollageOpts { layout: l, width: Some(1600), gap: 16, ..CollageOpts::default() };
            let ((w, _), _) = layout(5, &o);
            assert!((1600 - 8..=1600).contains(&w), "{l:?} gave {w}");
        }
        assert_eq!(layout(4, &CollageOpts::default()).0 .0, 2 * 612 + 12);
    }

    #[test]
    fn rounded_corners() {
        assert_eq!(corner_alpha(50, 50, 100, 100, 20), 1.0);
        assert_eq!(corner_alpha(0, 0, 100, 100, 20), 0.0);
        assert_eq!(corner_alpha(0, 0, 100, 100, 0), 1.0);
        assert!(corner_alpha(3, 3, 100, 100, 20) < 1.0);
        // radius larger than half the side is clamped: a circle, centre opaque
        assert_eq!(corner_alpha(50, 50, 100, 100, 999), 1.0);
    }

    #[test]
    fn typed_options() {
        let o = parse("collage row 1600 gap 16 radius 12 bg 000000").unwrap();
        assert_eq!((o.layout, o.width, o.gap, o.radius, o.bg), (Layout::Row, Some(1600), 16, 12, Some([0, 0, 0])));
        assert_eq!(parse("collage").unwrap(), CollageOpts::default());
        assert_eq!(parse("collage featured bg none").unwrap().bg, None);
        assert_eq!(parse("collage bg #ff8800").unwrap().bg, Some([255, 136, 0]));
        assert_eq!(parse("collage gap"), None);
        assert_eq!(parse("collage bg zzzzzz"), None);
        assert_eq!(parse("collage 10"), None);
        assert_eq!(parse("collagex"), None);
        assert_eq!(parse("grid"), None);
    }
}
