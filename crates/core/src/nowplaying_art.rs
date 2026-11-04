//! What the notch player derives from a cover, as plain pixels: the vinyl disc (the cover
//! aspect-filled into a circle with grooves and a tinted centre label), the blurred ambient halo and
//! the tint (dominant colour). Computed once per track, off the main thread; the AppKit layer only
//! draws the results. All functions work on RGBA, 8 bits, row-major, top row first.

/// Side of the square the cover is aspect-filled into (the disc image).
pub const DISC_PX: usize = 256;
/// Side of the small image the halo is blurred from; scaling it up gives the soft look.
pub const HALO_PX: usize = 48;
/// Ferrocast's accent: the tint when a cover has no colour of its own.
pub const ACCENT: [u8; 3] = [0xF0, 0x53, 0x5F];

/// Everything derived from one cover.
#[derive(Clone, Debug, PartialEq)]
pub struct ArtLook {
    /// `DISC_PX` squared, premultiplied alpha (transparent outside the circle).
    pub disc: Vec<u8>,
    /// `HALO_PX` squared, premultiplied: blurred, more saturated, and fading to nothing at the edge.
    pub halo: Vec<u8>,
    pub tint: [u8; 3],
}

fn hsv(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let d = mx - mn;
    let h = if d == 0.0 {
        0.0
    } else if mx == r {
        ((g - b) / d).rem_euclid(6.0)
    } else if mx == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h * 60.0, if mx == 0.0 { 0.0 } else { d / mx }, mx)
}

fn rgb(h: f64, s: f64, v: f64) -> [f64; 3] {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h.rem_euclid(360.0) / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [r + m, g + m, b + m]
}

/// The dominant colour of a cover, for the tint: the hue family that carries the most colour
/// (saturation times brightness), averaged and lifted to a bright, saturated version of itself so it
/// reads on black. Grey, black and white covers (no colour) give [`ACCENT`].
pub fn dominant_tint(rgba: &[u8]) -> [u8; 3] {
    const BINS: usize = 12;
    let mut weight = [0.0f64; BINS];
    let mut sum = [[0.0f64; 3]; BINS];
    for p in rgba.chunks_exact(4) {
        if p[3] < 128 {
            continue;
        }
        let (r, g, b) = (p[0] as f64 / 255.0, p[1] as f64 / 255.0, p[2] as f64 / 255.0);
        let (h, s, v) = hsv(r, g, b);
        if s < 0.25 || v < 0.25 {
            continue;
        }
        let bin = ((h / 360.0 * BINS as f64) as usize).min(BINS - 1);
        let w = s * v;
        weight[bin] += w;
        for (acc, c) in sum[bin].iter_mut().zip([r, g, b]) {
            *acc += c * w;
        }
    }
    let (best, total) = weight.iter().enumerate().fold((0, 0.0), |a, (i, w)| if *w > a.1 { (i, *w) } else { a });
    if total < 1.0 {
        return ACCENT;
    }
    let [r, g, b] = sum[best].map(|c| c / total);
    let (h, s, v) = hsv(r, g, b);
    let [r, g, b] = rgb(h, s.max(0.55), v.max(0.85));
    [r, g, b].map(|c| (c * 255.0).round().clamp(0.0, 255.0) as u8)
}

/// Box-average `src` (`sw` x `sh`) down to `dw` x `dh`.
pub fn downscale(src: &[u8], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<u8> {
    let mut out = vec![0u8; dw * dh * 4];
    if sw == 0 || sh == 0 || dw == 0 || dh == 0 || src.len() < sw * sh * 4 {
        return out;
    }
    for y in 0..dh {
        let (y0, y1) = (y * sh / dh, ((y + 1) * sh).div_ceil(dh).min(sh).max(y * sh / dh + 1));
        for x in 0..dw {
            let (x0, x1) = (x * sw / dw, ((x + 1) * sw).div_ceil(dw).min(sw).max(x * sw / dw + 1));
            let mut acc = [0u32; 4];
            for yy in y0..y1 {
                for xx in x0..x1 {
                    for (c, a) in acc.iter_mut().enumerate() {
                        *a += src[(yy * sw + xx) * 4 + c] as u32;
                    }
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as u32;
            for c in 0..4 {
                out[(y * dw + x) * 4 + c] = ((acc[c] + n / 2) / n) as u8;
            }
        }
    }
    out
}

/// Blur `px` (`w` x `h`) with `passes` box blurs of `radius` (edges repeat).
pub fn box_blur(px: &mut [u8], w: usize, h: usize, radius: usize, passes: usize) {
    if w == 0 || h == 0 || radius == 0 || px.len() < w * h * 4 {
        return;
    }
    let mut tmp = vec![0u8; w * h * 4];
    for _ in 0..passes {
        blur_lines(px, &mut tmp, w, 4, h, w * 4, radius);
        px.copy_from_slice(&tmp);
        blur_lines(px, &mut tmp, h, w * 4, w, 4, radius);
        px.copy_from_slice(&tmp);
    }
}

/// Blur each of `lines` lines of `len` samples (`stride` bytes apart, lines `line_stride` apart).
fn blur_lines(src: &[u8], dst: &mut [u8], len: usize, stride: usize, lines: usize, line_stride: usize, r: usize) {
    for l in 0..lines {
        for i in 0..len {
            for c in 0..4 {
                let (lo, hi) = (i.saturating_sub(r), (i + r).min(len - 1));
                let mut sum = 0u32;
                for j in lo..=hi {
                    sum += src[l * line_stride + j * stride + c] as u32;
                }
                dst[l * line_stride + i * stride + c] = (sum / (hi - lo + 1) as u32) as u8;
            }
        }
    }
}

/// Scale the colour of every pixel away from its grey by `amount` (1 = unchanged).
pub fn saturate(px: &mut [u8], amount: f64) {
    for p in px.chunks_exact_mut(4) {
        let g = 0.299 * p[0] as f64 + 0.587 * p[1] as f64 + 0.114 * p[2] as f64;
        for c in &mut p[..3] {
            *c = (g + (*c as f64 - g) * amount).round().clamp(0.0, 255.0) as u8;
        }
    }
}

fn mix(over: [f64; 3], a: f64, under: [f64; 3]) -> [f64; 3] {
    [0, 1, 2].map(|i| over[i] * a + under[i] * (1.0 - a))
}

/// The vinyl: `src` (`side` x `side`, the cover already aspect-filled) inside a circle, with the
/// artboard's grooves (a 20 % black ring every 4 pt of a 128 pt disc) and a centre hole 15 % wide in
/// `#050505` with a 2 pt ring of `tint`. Edges are anti-aliased; the result is premultiplied.
pub fn compose_disc(src: &[u8], side: usize, tint: [u8; 3]) -> Vec<u8> {
    let mut out = vec![0u8; side * side * 4];
    if side == 0 || src.len() < side * side * 4 {
        return out;
    }
    let s = side as f64;
    let (c, radius) = (s / 2.0, s / 2.0);
    let unit = s / 128.0; // one artboard point
    let (hole, ring) = (s * 0.075, s * 0.075 + 2.0 * unit);
    for y in 0..side {
        for x in 0..side {
            let (dx, dy) = (x as f64 + 0.5 - c, y as f64 + 0.5 - c);
            let d = (dx * dx + dy * dy).sqrt();
            let cover = (radius - d + 0.5).clamp(0.0, 1.0);
            if cover == 0.0 {
                continue;
            }
            let i = (y * side + x) * 4;
            let mut col = [src[i] as f64, src[i + 1] as f64, src[i + 2] as f64];
            // grooves
            if (d / unit).rem_euclid(4.0) >= 3.0 {
                col = mix([0.0; 3], 0.2, col);
            }
            // centre label: ring of tint, hole of near black, each with a soft 1 px edge
            let ring_a = (ring - d + 0.5).clamp(0.0, 1.0);
            col = mix(tint.map(f64::from), ring_a, col);
            let hole_a = (hole - d + 0.5).clamp(0.0, 1.0);
            col = mix([5.0; 3], hole_a, col);
            for k in 0..3 {
                out[i + k] = (col[k] * cover).round().clamp(0.0, 255.0) as u8;
            }
            out[i + 3] = (cover * 255.0).round() as u8;
        }
    }
    out
}

/// Derive everything from a cover that was aspect-filled into `DISC_PX` squared RGBA.
pub fn look_from_square(rgba: &[u8]) -> ArtLook {
    let tint = dominant_tint(&downscale(rgba, DISC_PX, DISC_PX, HALO_PX, HALO_PX));
    let mut halo = downscale(rgba, DISC_PX, DISC_PX, HALO_PX, HALO_PX);
    box_blur(&mut halo, HALO_PX, HALO_PX, 3, 2);
    saturate(&mut halo, 1.5);
    fade_edges(&mut halo, HALO_PX, HALO_PX);
    ArtLook { disc: compose_disc(rgba, DISC_PX, tint), halo, tint }
}

/// Fade the border of the halo (12 % of its width, smoothstep) so its edge dissolves instead of
/// cutting the card, as the artboard's 34 px blur does; the colour is premultiplied by the result.
pub fn fade_edges(px: &mut [u8], w: usize, h: usize) {
    let band = (w.min(h) as f64 * 0.12).max(1.0);
    for y in 0..h {
        for x in 0..w {
            let d = x.min(y).min(w - 1 - x).min(h - 1 - y) as f64;
            let t = (d / band).clamp(0.0, 1.0);
            let a = t * t * (3.0 - 2.0 * t);
            let i = (y * w + x) * 4;
            for c in 0..3 {
                px[i + c] = (px[i + c] as f64 * a).round() as u8;
            }
            px[i + 3] = (a * 255.0).round() as u8;
        }
    }
}

/// A track without a cover: a dark record with the accent on its label.
pub fn placeholder_look() -> ArtLook {
    let mut px = vec![255u8; DISC_PX * DISC_PX * 4];
    for y in 0..DISC_PX {
        for x in 0..DISC_PX {
            let t = (x + y) as f64 / (2.0 * DISC_PX as f64);
            let v = (0x2a as f64 + (0x4a - 0x2a) as f64 * t) as u8;
            let i = (y * DISC_PX + x) * 4;
            px[i..i + 3].copy_from_slice(&[v, v, v + 4]);
        }
    }
    let mut halo = downscale(&px, DISC_PX, DISC_PX, HALO_PX, HALO_PX);
    box_blur(&mut halo, HALO_PX, HALO_PX, 3, 2);
    fade_edges(&mut halo, HALO_PX, HALO_PX);
    ArtLook { disc: compose_disc(&px, DISC_PX, ACCENT), halo, tint: ACCENT }
}

/// Opacity and drift (points) of the halo: it fades in with the card (`ex`, the width spring) and
/// the new-track swell (`pulse`, 0..1), and drifts slowly (none with Reduce Motion).
pub fn halo_params(ex: f64, pulse: f64, tm: f64, rm: bool) -> (f64, f64, f64) {
    let (e, pu) = (ex.clamp(0.0, 1.0), pulse.clamp(0.0, 1.0));
    let alpha = (e * 0.7 + pu * 0.25).clamp(0.0, 1.0);
    if rm {
        (alpha, 0.0, 0.0)
    } else {
        (alpha, (tm * 0.3).sin() * 14.0, (tm * 0.23).cos() * 10.0)
    }
}

/// The light that bleeds out under a card of `w` x `h`: its rectangle (an ellipse inscribed in it)
/// and opacity.
pub fn bleed_params(w: f64, h: f64, ex: f64, pulse: f64) -> ([f64; 4], f64) {
    let (e, pu) = (ex.clamp(0.0, 1.0), pulse.clamp(0.0, 1.0));
    ([w * 0.08, h * 0.35, w * 0.84, h * 0.85], (0.3 + e * 0.35 + pu * 0.3).clamp(0.0, 1.0))
}

#[cfg(test)]
#[path = "nowplaying_art_tests.rs"]
mod tests;
