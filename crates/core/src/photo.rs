//! Photo adjustments (Tangerine editImage): light, colour and detail controls, applied to an RGBA
//! buffer. Pure math so every control is testable without files. Each value is -100..=100
//! (grain, denoise, dehaze and clarity: 0..=100); 0 leaves the picture unchanged.

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PhotoOpts {
    pub brightness: i32,
    pub contrast: i32,
    pub saturation: i32,
    /// Positive warms (more red, less blue), negative cools.
    pub warmth: i32,
    /// Local contrast: unsharp mask with a wide radius.
    pub clarity: i32,
    /// Fine sharpening: unsharp mask with a small radius.
    pub sharpen: i32,
    pub grain: i32,
    pub denoise: i32,
    pub dehaze: i32,
}

impl PhotoOpts {
    pub fn is_identity(&self) -> bool {
        *self == PhotoOpts::default()
    }
}

/// `edit brightness 20 contrast 10 saturation -30 warmth 15 clarity 40 sharpen 30 grain 20 denoise 30 dehaze 50`
pub fn parse(q: &str) -> Option<PhotoOpts> {
    let q = q.trim().to_lowercase();
    let rest = q.strip_prefix("edit ")?;
    let mut o = PhotoOpts::default();
    let mut it = rest.split_whitespace();
    let mut any = false;
    while let Some(k) = it.next() {
        let v: i32 = it.next()?.parse().ok()?;
        let signed = matches!(k, "brightness" | "contrast" | "saturation" | "warmth");
        let range = if signed { -100..=100 } else { 0..=100 };
        if !range.contains(&v) {
            return None;
        }
        match k {
            "brightness" => o.brightness = v,
            "contrast" => o.contrast = v,
            "saturation" => o.saturation = v,
            "warmth" => o.warmth = v,
            "clarity" => o.clarity = v,
            "sharpen" => o.sharpen = v,
            "grain" => o.grain = v,
            "denoise" => o.denoise = v,
            "dehaze" => o.dehaze = v,
            _ => return None,
        }
        any = true;
    }
    any.then_some(o)
}

fn clamp8(v: f32) -> u8 {
    v.round().clamp(0.0, 255.0) as u8
}

/// Separable box blur of the RGB channels (alpha is kept), `r` pixels each side.
fn box_blur(src: &[u8], w: usize, h: usize, r: usize) -> Vec<u8> {
    if r == 0 || w == 0 || h == 0 {
        return src.to_vec();
    }
    let pass = |input: &[u8], horizontal: bool| -> Vec<u8> {
        let mut out = input.to_vec();
        let (len, lines) = if horizontal { (w, h) } else { (h, w) };
        let idx = |line: usize, i: usize| if horizontal { (line * w + i) * 4 } else { (i * w + line) * 4 };
        for line in 0..lines {
            for c in 0..3 {
                let mut sum = 0u32;
                let mut count = 0u32;
                for i in 0..=r.min(len - 1) {
                    sum += input[idx(line, i) + c] as u32;
                    count += 1;
                }
                for i in 0..len {
                    out[idx(line, i) + c] = (sum / count) as u8;
                    let add = i + r + 1;
                    if add < len {
                        sum += input[idx(line, add) + c] as u32;
                        count += 1;
                    }
                    if i >= r {
                        sum -= input[idx(line, i - r) + c] as u32;
                        count -= 1;
                    }
                }
            }
        }
        out
    };
    pass(&pass(src, true), false)
}

/// Unsharp mask: `amount` times the difference to a blurred copy.
fn unsharp(buf: &mut [u8], w: usize, h: usize, radius: usize, amount: f32) {
    let blurred = box_blur(buf, w, h, radius);
    for (p, b) in buf.chunks_exact_mut(4).zip(blurred.chunks_exact(4)) {
        for (v, bl) in p.iter_mut().zip(b).take(3) {
            *v = clamp8(*v as f32 + (*v as f32 - *bl as f32) * amount);
        }
    }
}

/// Deterministic pseudo-random value in -1.0..1.0 for a pixel (same picture, same grain).
fn noise(x: usize, y: usize) -> f32 {
    let mut n = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263);
    n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
    n ^= n >> 16;
    (n & 0xFFFF) as f32 / 32767.5 - 1.0
}

/// Apply every non-zero control to `buf` (RGBA8, `w × h`). Order: noise reduction and dehaze
/// first, then light and colour, then detail, grain last so it is not sharpened.
pub fn apply(buf: &mut [u8], w: usize, h: usize, o: &PhotoOpts) {
    assert_eq!(buf.len(), w * h * 4, "buffer does not match the size");
    if o.is_identity() || buf.is_empty() {
        return;
    }
    if o.denoise > 0 {
        let blurred = box_blur(buf, w, h, 1 + (o.denoise as usize) / 50);
        let k = o.denoise as f32 / 100.0;
        for (p, b) in buf.chunks_exact_mut(4).zip(blurred.chunks_exact(4)) {
            for c in 0..3 {
                p[c] = clamp8(p[c] as f32 * (1.0 - k) + b[c] as f32 * k);
            }
        }
    }
    if o.dehaze > 0 {
        // haze lifts the darkest channel everywhere: estimate it from the darkest 1% and remove
        // a share of it, then stretch back to full range
        let mut mins: Vec<u8> = buf.chunks_exact(4).map(|p| p[0].min(p[1]).min(p[2])).collect();
        mins.sort_unstable();
        let haze = mins[mins.len() / 100] as f32 * (o.dehaze as f32 / 100.0);
        let scale = 255.0 / (255.0 - haze).max(1.0);
        for p in buf.chunks_exact_mut(4) {
            for v in p.iter_mut().take(3) {
                *v = clamp8((*v as f32 - haze) * scale);
            }
        }
    }
    let (b, ct, sat, warm) =
        (o.brightness as f32 * 2.55, 1.0 + o.contrast as f32 / 100.0, 1.0 + o.saturation as f32 / 100.0, o.warmth as f32 * 0.4);
    if o.brightness != 0 || o.contrast != 0 || o.saturation != 0 || o.warmth != 0 {
        for p in buf.chunks_exact_mut(4) {
            let (mut r, mut g, mut bl) = (p[0] as f32, p[1] as f32, p[2] as f32);
            r = (r - 128.0) * ct + 128.0 + b + warm;
            g = (g - 128.0) * ct + 128.0 + b;
            bl = (bl - 128.0) * ct + 128.0 + b - warm;
            let l = 0.299 * r + 0.587 * g + 0.114 * bl;
            p[0] = clamp8(l + (r - l) * sat);
            p[1] = clamp8(l + (g - l) * sat);
            p[2] = clamp8(l + (bl - l) * sat);
        }
    }
    if o.clarity > 0 {
        unsharp(buf, w, h, (w.min(h) / 40).max(3), o.clarity as f32 / 100.0 * 0.8);
    }
    if o.sharpen > 0 {
        unsharp(buf, w, h, 1, o.sharpen as f32 / 100.0 * 1.5);
    }
    if o.grain > 0 {
        let amp = o.grain as f32 * 0.6;
        for (i, p) in buf.chunks_exact_mut(4).enumerate() {
            let n = noise(i % w, i / w) * amp;
            for c in p.iter_mut().take(3) {
                *c = clamp8(*c as f32 + n);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(w: usize, h: usize, rgb: [u8; 3]) -> Vec<u8> {
        (0..w * h).flat_map(|_| [rgb[0], rgb[1], rgb[2], 255]).collect()
    }

    fn mean(buf: &[u8], c: usize) -> f32 {
        buf.chunks_exact(4).map(|p| p[c] as f32).sum::<f32>() / (buf.len() / 4) as f32
    }

    fn spread(buf: &[u8]) -> f32 {
        let m = mean(buf, 0);
        (buf.chunks_exact(4).map(|p| (p[0] as f32 - m).powi(2)).sum::<f32>() / (buf.len() / 4) as f32).sqrt()
    }

    /// Left half dark, right half light, so edges exist.
    fn edge(w: usize, h: usize) -> Vec<u8> {
        (0..w * h).flat_map(|i| if i % w < w / 2 { [60, 60, 60, 255] } else { [190, 190, 190, 255] }).collect()
    }

    #[test]
    fn identity_changes_nothing() {
        let mut b = edge(8, 8);
        let before = b.clone();
        apply(&mut b, 8, 8, &PhotoOpts::default());
        assert_eq!(b, before);
    }

    #[test]
    fn light_and_colour() {
        let mut b = flat(4, 4, [100, 100, 100]);
        apply(&mut b, 4, 4, &PhotoOpts { brightness: 20, ..Default::default() });
        assert_eq!(&b[..3], &[151, 151, 151], "100 + 20% of 255");
        let mut b = flat(4, 4, [100, 150, 200]);
        apply(&mut b, 4, 4, &PhotoOpts { saturation: -100, ..Default::default() });
        assert_eq!(b[0], b[1]);
        assert_eq!(b[1], b[2]);
        let mut b = flat(4, 4, [100, 100, 100]);
        apply(&mut b, 4, 4, &PhotoOpts { warmth: 50, ..Default::default() });
        assert!(b[0] > 100 && b[2] < 100 && b[1] == 100, "{:?}", &b[..3]);
        let mut b = edge(8, 8);
        apply(&mut b, 8, 8, &PhotoOpts { contrast: 50, ..Default::default() });
        assert!(b[0] < 60 && b[7 * 4] > 190, "dark gets darker, light lighter");
    }

    #[test]
    fn detail_controls() {
        let base = edge(40, 10);
        let mut sharp = base.clone();
        apply(&mut sharp, 40, 10, &PhotoOpts { sharpen: 80, ..Default::default() });
        assert!(spread(&sharp) > spread(&base), "sharpen widens the edge");
        let mut clear = base.clone();
        apply(&mut clear, 40, 10, &PhotoOpts { clarity: 80, ..Default::default() });
        assert_ne!(clear, base);
        let mut soft = base.clone();
        apply(&mut soft, 40, 10, &PhotoOpts { denoise: 100, ..Default::default() });
        assert!(spread(&soft) < spread(&base), "denoise smooths");
    }

    #[test]
    fn grain_is_deterministic_and_bounded() {
        let mut a = flat(16, 16, [128, 128, 128]);
        let mut b = a.clone();
        let o = PhotoOpts { grain: 50, ..Default::default() };
        apply(&mut a, 16, 16, &o);
        apply(&mut b, 16, 16, &o);
        assert_eq!(a, b);
        assert!(spread(&a) > 1.0, "grain adds variation");
        assert!(a.chunks_exact(4).all(|p| (p[0] as i32 - 128).abs() <= 31), "amplitude is 0.6 per point");
        assert!(a.chunks_exact(4).all(|p| p[3] == 255), "alpha untouched");
    }

    #[test]
    fn dehaze_removes_a_lifted_black_point() {
        // a hazy picture: nothing darker than 80
        let mut b: Vec<u8> = (0..100)
            .flat_map(|i| {
                let v = 80 + (i % 10) as u8 * 15;
                [v, v, v, 255]
            })
            .collect();
        let before = b.iter().step_by(4).copied().min().unwrap();
        apply(&mut b, 10, 10, &PhotoOpts { dehaze: 100, ..Default::default() });
        let after = b.iter().step_by(4).copied().min().unwrap();
        assert!(after < before, "black point drops: {before} → {after}");
    }

    #[test]
    fn typed_options() {
        let o = parse("edit brightness 20 contrast -10 grain 30 dehaze 50").unwrap();
        assert_eq!((o.brightness, o.contrast, o.grain, o.dehaze), (20, -10, 30, 50));
        for bad in ["edit", "edit brightness", "edit brightness 101", "edit grain -5", "edit wat 5", "edit brightness x", "brightness 5"] {
            assert_eq!(parse(bad), None, "{bad}");
        }
    }
}
