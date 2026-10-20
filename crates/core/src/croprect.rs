//! Geometry of the crop editor (Tangerine cropImage): a rectangle inside a picture that can be
//! moved, resized by its edges and corners, locked to an aspect ratio or typed in exact pixels.
//! Pure, so every rule is testable without a window.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Handle {
    Move,
    N,
    S,
    E,
    W,
    NE,
    NW,
    SE,
    SW,
}

/// Smallest side the editor allows, in pixels.
pub const MIN: u32 = 8;

/// The whole picture.
pub fn full(iw: u32, ih: u32) -> Rect {
    Rect { x: 0, y: 0, w: iw, h: ih }
}

/// Largest centred rectangle of aspect `rw:rh` inside `iw × ih`.
pub fn fit_aspect(iw: u32, ih: u32, rw: u32, rh: u32) -> Rect {
    if rw == 0 || rh == 0 || iw == 0 || ih == 0 {
        return full(iw, ih);
    }
    let (cw, ch) = crate::convert::crop_size(iw, ih, rw, rh);
    Rect { x: (iw - cw) / 2, y: (ih - ch) / 2, w: cw, h: ch }
}

/// Keep `r` inside the picture and at least `MIN` wide and tall.
pub fn clamp(r: Rect, iw: u32, ih: u32) -> Rect {
    let w = r.w.clamp(MIN.min(iw), iw.max(1));
    let h = r.h.clamp(MIN.min(ih), ih.max(1));
    Rect { x: r.x.min(iw - w), y: r.y.min(ih - h), w, h }
}

/// Drag `handle` by (`dx`, `dy`) pixels. With `aspect` set the ratio is kept and the corner/edge
/// opposite the handle stays where it is.
pub fn drag(r: Rect, handle: Handle, dx: i64, dy: i64, iw: u32, ih: u32, aspect: Option<(u32, u32)>) -> Rect {
    let (mut x0, mut y0, mut x1, mut y1) = (r.x as i64, r.y as i64, (r.x + r.w) as i64, (r.y + r.h) as i64);
    let (iw_i, ih_i) = (iw as i64, ih as i64);
    let min = MIN.min(iw).min(ih) as i64;
    if handle == Handle::Move {
        let nx = (x0 + dx).clamp(0, iw_i - r.w as i64);
        let ny = (y0 + dy).clamp(0, ih_i - r.h as i64);
        return Rect { x: nx as u32, y: ny as u32, w: r.w, h: r.h };
    }
    let (west, east) = (matches!(handle, Handle::W | Handle::NW | Handle::SW), matches!(handle, Handle::E | Handle::NE | Handle::SE));
    let (north, south) = (matches!(handle, Handle::N | Handle::NE | Handle::NW), matches!(handle, Handle::S | Handle::SE | Handle::SW));
    if west {
        x0 = (x0 + dx).clamp(0, x1 - min);
    }
    if east {
        x1 = (x1 + dx).clamp(x0 + min, iw_i);
    }
    if north {
        y0 = (y0 + dy).clamp(0, y1 - min);
    }
    if south {
        y1 = (y1 + dy).clamp(y0 + min, ih_i);
    }
    if let Some((rw, rh)) = aspect.filter(|(a, b)| *a > 0 && *b > 0) {
        let ratio = rw as f64 / rh as f64;
        let (mut w, mut h) = ((x1 - x0) as f64, (y1 - y0) as f64);
        // edges drive one dimension; corners follow whichever side moved more
        let width_drives = match handle {
            Handle::E | Handle::W => true,
            Handle::N | Handle::S => false,
            _ => (dx.abs() as f64) >= (dy.abs() as f64) * ratio,
        };
        if width_drives {
            h = w / ratio;
        } else {
            w = h * ratio;
        }
        // room available on each side of the fixed edge
        let max_w = if west { x1 } else { iw_i - x0 } as f64;
        let max_h = if north { y1 } else { ih_i - y0 } as f64;
        let k = (max_w / w).min(max_h / h).min(1.0);
        let (w, h) = ((w * k).floor().max(min as f64), (h * k).floor().max(min as f64));
        let (w, h) = (w as i64, h as i64);
        // the middle of an edge handle keeps the rectangle centred on that axis
        if west {
            x0 = x1 - w;
        } else {
            x1 = x0 + w;
        }
        if north {
            y0 = y1 - h;
        } else if south {
            y1 = y0 + h;
        } else {
            let cy = (y0 + y1) / 2;
            y0 = (cy - h / 2).clamp(0, ih_i - h);
            y1 = y0 + h;
        }
        if !west && !east {
            let cx = (x0 + x1) / 2;
            x0 = (cx - w / 2).clamp(0, iw_i - w);
            x1 = x0 + w;
        }
    }
    clamp(Rect { x: x0 as u32, y: y0 as u32, w: (x1 - x0) as u32, h: (y1 - y0) as u32 }, iw, ih)
}

/// Rectangle of exactly `w × h` pixels, centred on `r` and kept inside the picture.
pub fn with_size(r: Rect, w: u32, h: u32, iw: u32, ih: u32) -> Rect {
    let (w, h) = (w.clamp(MIN.min(iw), iw), h.clamp(MIN.min(ih), ih));
    let cx = r.x + r.w / 2;
    let cy = r.y + r.h / 2;
    clamp(Rect { x: cx.saturating_sub(w / 2), y: cy.saturating_sub(h / 2), w, h }, iw, ih)
}

/// Which handle sits under (`px`, `py`) for a rectangle drawn at `scale` screen pixels per image
/// pixel, with `grab` screen pixels of slack; inside the rectangle (away from the edges) it moves.
pub fn hit(r: Rect, px: f32, py: f32, scale: f32, grab: f32) -> Option<Handle> {
    let g = grab / scale.max(0.0001);
    let (x0, y0, x1, y1) = (r.x as f32, r.y as f32, (r.x + r.w) as f32, (r.y + r.h) as f32);
    if px < x0 - g || px > x1 + g || py < y0 - g || py > y1 + g {
        return None;
    }
    let (l, rt) = ((px - x0).abs() <= g, (px - x1).abs() <= g);
    let (t, b) = ((py - y0).abs() <= g, (py - y1).abs() <= g);
    Some(match (l, rt, t, b) {
        (true, _, true, _) => Handle::NW,
        (_, true, true, _) => Handle::NE,
        (true, _, _, true) => Handle::SW,
        (_, true, _, true) => Handle::SE,
        (true, ..) => Handle::W,
        (_, true, ..) => Handle::E,
        (_, _, true, _) => Handle::N,
        (_, _, _, true) => Handle::S,
        _ => Handle::Move,
    })
}

/// `x,y,w,h` of the rectangle for the writer.
pub fn tuple(r: Rect) -> (u32, u32, u32, u32) {
    (r.x, r.y, r.w, r.h)
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: Rect = Rect { x: 100, y: 100, w: 200, h: 100 };

    #[test]
    fn move_stays_inside_the_picture() {
        assert_eq!(drag(R, Handle::Move, 30, -20, 1000, 800, None), Rect { x: 130, y: 80, ..R });
        assert_eq!(drag(R, Handle::Move, -500, -500, 1000, 800, None), Rect { x: 0, y: 0, ..R });
        assert_eq!(drag(R, Handle::Move, 5000, 5000, 1000, 800, None), Rect { x: 800, y: 700, ..R });
    }

    #[test]
    fn edges_and_corners_resize_from_the_right_side() {
        assert_eq!(drag(R, Handle::E, 50, 99, 1000, 800, None), Rect { w: 250, ..R }, "east only changes width");
        assert_eq!(drag(R, Handle::W, 40, 0, 1000, 800, None), Rect { x: 140, w: 160, ..R });
        assert_eq!(drag(R, Handle::N, 0, -30, 1000, 800, None), Rect { y: 70, h: 130, ..R });
        assert_eq!(drag(R, Handle::SE, 10, 20, 1000, 800, None), Rect { w: 210, h: 120, ..R });
        assert_eq!(drag(R, Handle::NW, -10, -10, 1000, 800, None), Rect { x: 90, y: 90, w: 210, h: 110 });
        // never beyond the picture, never smaller than MIN, never inverted
        assert_eq!(drag(R, Handle::E, 9999, 0, 1000, 800, None).w, 900);
        assert_eq!(drag(R, Handle::E, -9999, 0, 1000, 800, None).w, MIN);
        assert_eq!(drag(R, Handle::W, 9999, 0, 1000, 800, None), Rect { x: 292, w: MIN, ..R });
    }

    #[test]
    fn aspect_lock_keeps_the_ratio_and_the_fixed_corner() {
        let sq = Rect { x: 100, y: 100, w: 100, h: 100 };
        let r = drag(sq, Handle::SE, 60, 10, 1000, 800, Some((1, 1)));
        assert_eq!((r.x, r.y), (100, 100), "north-west corner fixed");
        assert_eq!(r.w, r.h, "still square");
        assert!(r.w >= 150, "the larger move wins: {r:?}");
        let r = drag(sq, Handle::NW, -40, -10, 1000, 800, Some((1, 1)));
        assert_eq!((r.x + r.w, r.y + r.h), (200, 200), "south-east corner fixed");
        assert_eq!(r.w, r.h);
        let wide = drag(Rect { x: 0, y: 0, w: 160, h: 90 }, Handle::E, 40, 0, 1000, 800, Some((16, 9)));
        assert_eq!((wide.w, wide.h), (200, 112), "height follows width at 16:9 (floored)");
        let tall = drag(Rect { x: 0, y: 0, w: 160, h: 90 }, Handle::S, 0, 90, 1000, 800, Some((16, 9)));
        assert_eq!((tall.w, tall.h), (320, 180), "width follows height");
        // limited by the picture: grows only as far as it fits
        let capped = drag(Rect { x: 0, y: 0, w: 100, h: 100 }, Handle::SE, 5000, 5000, 400, 300, Some((1, 1)));
        assert_eq!((capped.w, capped.h), (300, 300));
    }

    #[test]
    fn presets_and_exact_sizes() {
        assert_eq!(fit_aspect(1000, 600, 1, 1), Rect { x: 200, y: 0, w: 600, h: 600 });
        assert_eq!(fit_aspect(1000, 600, 16, 9), Rect { x: 0, y: 0, w: 1000, h: 562 }.pipe_centered(600));
        assert_eq!(fit_aspect(10, 10, 0, 5), full(10, 10), "bad ratio gives the whole picture");
        let r = with_size(Rect { x: 400, y: 300, w: 200, h: 100 }, 300, 300, 1000, 800);
        assert_eq!(r, Rect { x: 350, y: 200, w: 300, h: 300 }, "centred on the old rectangle (centre 500,350)");
        let r = with_size(Rect { x: 0, y: 0, w: 50, h: 50 }, 400, 400, 300, 200);
        assert_eq!((r.w, r.h), (300, 200), "clamped to the picture");
        assert_eq!(with_size(R, 1, 1, 1000, 800).w, MIN);
        assert_eq!(
            clamp(Rect { x: 990, y: 790, w: 500, h: 500 }, 1000, 800),
            Rect { x: 500, y: 300, w: 500, h: 500 },
            "pushed back inside"
        );
    }

    #[test]
    fn hit_picks_handles_and_ignores_far_points() {
        let s = 0.5; // each image pixel is half a screen pixel: 10 screen px of slack = 20 image px
        assert_eq!(hit(R, 100.0, 100.0, s, 10.0), Some(Handle::NW));
        assert_eq!(hit(R, 300.0, 200.0, s, 10.0), Some(Handle::SE));
        assert_eq!(hit(R, 300.0, 150.0, s, 10.0), Some(Handle::E));
        assert_eq!(hit(R, 200.0, 100.0, s, 10.0), Some(Handle::N));
        assert_eq!(hit(R, 200.0, 150.0, s, 10.0), Some(Handle::Move));
        assert_eq!(hit(R, 600.0, 600.0, s, 10.0), None);
        assert_eq!(tuple(R), (100, 100, 200, 100));
    }

    impl Rect {
        /// Test helper: the same rectangle centred vertically in a picture `ih` tall.
        fn pipe_centered(self, ih: u32) -> Rect {
            Rect { y: (ih - self.h) / 2, ..self }
        }
    }
}
