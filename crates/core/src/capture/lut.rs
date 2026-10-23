//! 3D colour look-up tables (P6 T40): a strict `.cube` parser, the six bundled looks (generated
//! in code from small colour-math functions, no files) and the helpers the renderer needs
//! (`to_rgba_f32` for `CIColorCube`, intensity blending, trilinear sampling for tests).
//!
//! Data is RGB, red index fastest, as in the `.cube` format and as `CIColorCube` expects.

/// Largest and smallest `LUT_3D_SIZE` accepted.
pub const SIZE_MIN: usize = 2;
pub const SIZE_MAX: usize = 64;
/// Edge length of the bundled looks.
pub const BUNDLED_SIZE: usize = 17;
/// Names of the bundled looks, in display order.
pub const BUNDLED_LOOK_NAMES: [&str; 6] = ["warm", "cool", "cinematic", "faded", "high-contrast", "mono"];

#[derive(Debug, Clone, PartialEq)]
pub struct Lut3D {
    /// Edge length: the table has `size^3` entries.
    pub size: usize,
    /// `size^3 * 3` floats (R, G, B per entry), red fastest, every value in `0..=1`.
    pub data: Vec<f32>,
    pub title: Option<String>,
    /// Input domain of the table (`0..1` unless the file says otherwise); [`Lut3D::sample`]
    /// honours it, `CIColorCube` consumers must normalise inputs themselves.
    pub domain_min: [f32; 3],
    pub domain_max: [f32; 3],
}

fn unit(v: f32) -> f32 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(0.0, 1.0)
    }
}

impl Lut3D {
    /// The table that leaves every colour unchanged.
    pub fn identity(size: usize) -> Self {
        Self::build(size, |c| c)
    }

    /// A table with `f(input rgb in 0..1)` at every lattice point (outputs clamped to `0..=1`).
    pub fn build(size: usize, f: impl Fn([f32; 3]) -> [f32; 3]) -> Self {
        let size = size.clamp(SIZE_MIN, SIZE_MAX);
        let m = (size - 1) as f32;
        let mut data = Vec::with_capacity(size * size * size * 3);
        for b in 0..size {
            for g in 0..size {
                for r in 0..size {
                    data.extend(f([r as f32 / m, g as f32 / m, b as f32 / m]).map(unit));
                }
            }
        }
        Self { size, data, title: None, domain_min: [0.0; 3], domain_max: [1.0; 3] }
    }

    /// `size^3 * 4` floats with alpha 1, the layout of `CIColorCube`'s `inputCubeData`.
    pub fn to_rgba_f32(&self) -> Vec<f32> {
        self.data.chunks_exact(3).flat_map(|c| [c[0], c[1], c[2], 1.0]).collect()
    }

    /// Blend with the identity: 0 = unchanged picture, 1 = this look. Out-of-range values are
    /// clamped and NaN counts as 0.
    pub fn with_intensity(&self, intensity: f32) -> Self {
        let k = unit(intensity);
        let id = Self::identity(self.size);
        let data = self.data.iter().zip(&id.data).map(|(&a, &b)| if k >= 1.0 { a } else { b + (a - b) * k }).collect();
        Self { data, ..self.clone() }
    }

    /// Trilinear lookup of `rgb` (through the input domain, clamped).
    pub fn sample(&self, rgb: [f32; 3]) -> [f32; 3] {
        let n = self.size;
        let m = (n - 1) as f32;
        let mut pos = [0.0f32; 3];
        for c in 0..3 {
            let span = self.domain_max[c] - self.domain_min[c];
            pos[c] = unit((rgb[c] - self.domain_min[c]) / if span > 0.0 { span } else { 1.0 }) * m;
        }
        let i0 = pos.map(|p| (p.floor() as usize).min(n - 2));
        let fr = [pos[0] - i0[0] as f32, pos[1] - i0[1] as f32, pos[2] - i0[2] as f32];
        let at = |r: usize, g: usize, b: usize| {
            let i = ((b * n + g) * n + r) * 3;
            [self.data[i], self.data[i + 1], self.data[i + 2]]
        };
        let mut out = [0.0f32; 3];
        for corner in 0..8usize {
            let (dr, dg, db) = (corner & 1, (corner >> 1) & 1, (corner >> 2) & 1);
            let w = [
                if dr == 1 { fr[0] } else { 1.0 - fr[0] },
                if dg == 1 { fr[1] } else { 1.0 - fr[1] },
                if db == 1 { fr[2] } else { 1.0 - fr[2] },
            ];
            let v = at(i0[0] + dr, i0[1] + dg, i0[2] + db);
            for c in 0..3 {
                out[c] += v[c] * w[0] * w[1] * w[2];
            }
        }
        out
    }

    /// The table as `.cube` text (parses back with [`parse_cube`]).
    pub fn to_cube(&self) -> String {
        let mut s = String::new();
        if let Some(t) = &self.title {
            s += &format!("TITLE \"{t}\"\n");
        }
        s += &format!("LUT_3D_SIZE {}\n", self.size);
        for c in self.data.chunks_exact(3) {
            s += &format!("{} {} {}\n", c[0], c[1], c[2]);
        }
        s
    }
}

fn numbers<const N: usize>(rest: &str, what: &str) -> Result<[f32; N], String> {
    let parts: Vec<&str> = rest.split_whitespace().collect();
    if parts.len() != N {
        return Err(format!("{what} needs three numbers, found {}", parts.len()));
    }
    let mut out = [0.0f32; N];
    for (o, p) in out.iter_mut().zip(parts) {
        let v: f32 = p.parse().map_err(|_| format!("{what}: `{p}` is not a number"))?;
        if !v.is_finite() {
            return Err(format!("{what}: `{p}` is not finite"));
        }
        *o = v;
    }
    Ok(out)
}

/// Parse a 3D `.cube` LUT. Understands `TITLE`, `DOMAIN_MIN`/`DOMAIN_MAX`, `LUT_3D_SIZE`
/// (`2..=64`), `#` comments, blank lines and CRLF; needs exactly `size^3` rows of three finite
/// numbers (clamped to `0..=1`). 1D LUTs, unknown keywords and malformed rows are errors; the
/// function never panics.
pub fn parse_cube(text: &str) -> Result<Lut3D, String> {
    let (mut size, mut title) = (None::<usize>, None);
    let (mut dmin, mut dmax) = ([0.0f32; 3], [1.0f32; 3]);
    let mut data: Vec<f32> = vec![];
    let mut rows = 0usize;
    for (n, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let at = |e: String| format!("line {}: {e}", n + 1);
        let (word, rest) = line.split_once(char::is_whitespace).map_or((line, ""), |(w, r)| (w, r.trim()));
        if word.chars().next().is_some_and(char::is_alphabetic) {
            match word {
                "TITLE" => title = Some(rest.trim_matches('"').to_string()),
                "DOMAIN_MIN" => dmin = numbers::<3>(rest, "DOMAIN_MIN").map_err(at)?,
                "DOMAIN_MAX" => dmax = numbers::<3>(rest, "DOMAIN_MAX").map_err(at)?,
                "LUT_1D_SIZE" | "LUT_1D_INPUT_RANGE" => return Err(at("1D LUTs are not supported, use a 3D .cube".into())),
                "LUT_3D_SIZE" => {
                    if size.is_some() {
                        return Err(at("LUT_3D_SIZE given twice".into()));
                    }
                    let s: usize = rest.parse().map_err(|_| at(format!("LUT_3D_SIZE must be an integer, found `{rest}`")))?;
                    if !(SIZE_MIN..=SIZE_MAX).contains(&s) {
                        return Err(at(format!("LUT_3D_SIZE {s} outside {SIZE_MIN}..={SIZE_MAX}")));
                    }
                    size = Some(s);
                    data.reserve(s * s * s * 3);
                }
                other => return Err(at(format!("unknown keyword {other}"))),
            }
            continue;
        }
        let Some(s) = size else { return Err(at("data row before LUT_3D_SIZE".into())) };
        if rows == s * s * s {
            return Err(at(format!("more than {} rows", s * s * s)));
        }
        data.extend(numbers::<3>(line, "row").map_err(at)?.map(unit));
        rows += 1;
    }
    let size = size.ok_or("missing LUT_3D_SIZE")?;
    if rows != size * size * size {
        return Err(format!("expected {} rows for LUT_3D_SIZE {size}, found {rows}", size * size * size));
    }
    if (0..3).any(|c| dmin[c] >= dmax[c]) {
        return Err("DOMAIN_MIN must be below DOMAIN_MAX on every channel".into());
    }
    Ok(Lut3D { size, data, title, domain_min: dmin, domain_max: dmax })
}

fn luma(c: [f32; 3]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

fn smooth(x: f32) -> f32 {
    let x = unit(x);
    x * x * (3.0 - 2.0 * x)
}

fn look_fn(name: &str) -> Option<fn([f32; 3]) -> [f32; 3]> {
    Some(match name {
        "warm" => |c| [c[0] * 1.06 + 0.02, c[1] * 1.01, c[2] * 0.90],
        "cool" => |c| [c[0] * 0.90, c[1] * 0.99 + 0.005, c[2] * 1.05 + 0.02],
        "cinematic" => |c| {
            // gentle S-curve, shadows pushed to teal and highlights to orange
            let t = luma(c) - 0.5;
            let k = |x: f32| x + 0.25 * (smooth(x) - x);
            [k(c[0]) + 0.08 * t, k(c[1]), k(c[2]) - 0.08 * t]
        },
        "faded" => |c| {
            let l = luma(c);
            c.map(|x| 0.07 + 0.86 * (x + (l - x) * 0.15))
        },
        "high-contrast" => |c| c.map(smooth),
        "mono" => |c| [luma(c); 3],
        _ => return None,
    })
}

/// One bundled look by name (`warm`, `cool`, `cinematic`, `faded`, `high-contrast`, `mono`).
pub fn bundled_look(name: &str) -> Option<Lut3D> {
    let mut lut = Lut3D::build(BUNDLED_SIZE, look_fn(name)?);
    lut.title = Some(name.to_string());
    Some(lut)
}

/// The six bundled looks, [`BUNDLED_LOOK_NAMES`] order.
pub fn bundled_looks() -> Vec<(&'static str, Lut3D)> {
    BUNDLED_LOOK_NAMES.iter().filter_map(|n| bundled_look(n).map(|l| (*n, l))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube(size: usize, f: impl Fn([f32; 3]) -> [f32; 3]) -> String {
        let mut s = format!("TITLE \"t\"\nLUT_3D_SIZE {size}\n");
        let m = (size - 1) as f32;
        for b in 0..size {
            for g in 0..size {
                for r in 0..size {
                    let o = f([r as f32 / m, g as f32 / m, b as f32 / m]);
                    s += &format!("{} {} {}\n", o[0], o[1], o[2]);
                }
            }
        }
        s
    }

    fn near(a: [f32; 3], b: [f32; 3], eps: f32) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() <= eps)
    }

    #[test]
    fn parses_an_identity_cube_with_comments_crlf_and_blank_lines() {
        let text = cube(2, |c| c).replace('\n', "\r\n");
        let text = format!("# made by hand\r\n\r\n{text}\r\n# trailing\r\n");
        let l = parse_cube(&text).unwrap();
        assert_eq!((l.size, l.data.len(), l.title.as_deref()), (2, 24, Some("t")));
        assert_eq!(l.data, Lut3D::identity(2).data);
        assert_eq!((l.domain_min, l.domain_max), ([0.0; 3], [1.0; 3]));
        // red varies fastest: the second row is red = 1
        assert_eq!(&l.data[3..6], &[1.0, 0.0, 0.0]);
        assert_eq!(&l.data[6..9], &[0.0, 1.0, 0.0]);
    }

    #[test]
    fn parses_domain_title_and_clamps_values() {
        let mut t = String::from("TITLE no quotes\nDOMAIN_MIN 0 0 0\nDOMAIN_MAX 2 2 2\nLUT_3D_SIZE 2\n");
        for i in 0..8 {
            t += &format!("{} -0.5 1e0\n", 1.5 + i as f32);
        }
        let l = parse_cube(&t).unwrap();
        assert_eq!(l.title.as_deref(), Some("no quotes"));
        assert_eq!(l.domain_max, [2.0; 3]);
        assert!(l.data.chunks(3).all(|c| c == [1.0, 0.0, 1.0]));
        // the domain is honoured when sampling: input 1.0 is the middle of 0..2
        let id = parse_cube(&format!("DOMAIN_MAX 2 2 2\n{}", cube(2, |c| c).replace("TITLE \"t\"\n", ""))).unwrap();
        assert!(near(id.sample([1.0, 1.0, 1.0]), [0.5, 0.5, 0.5], 1e-6));
    }

    #[test]
    fn rejects_malformed_input_with_a_clear_message() {
        let ok = cube(2, |c| c);
        let cases: Vec<(String, &str)> = vec![
            ("".into(), "LUT_3D_SIZE"),
            ("# only a comment\n".into(), "LUT_3D_SIZE"),
            ("LUT_1D_SIZE 4\n0 0 0\n".into(), "1D"),
            ("LUT_3D_SIZE 1\n0 0 0\n".into(), "2..=64"),
            ("LUT_3D_SIZE 65\n".into(), "2..=64"),
            ("LUT_3D_SIZE two\n".into(), "LUT_3D_SIZE"),
            ("LUT_3D_SIZE 2\nLUT_3D_SIZE 2\n".into(), "twice"),
            ("0 0 0\nLUT_3D_SIZE 2\n".into(), "before"),
            (ok.lines().take(5).collect::<Vec<_>>().join("\n"), "expected 8"),
            (format!("{ok}1 1 1\n"), "more than 8"),
            (ok.replacen("1 0 0", "1 0", 1), "three"),
            (ok.replacen("1 0 0", "1 0 0 0", 1), "three"),
            (ok.replacen("1 0 0", "1 0 x", 1), "number"),
            (ok.replacen("1 0 0", "1 0 NaN", 1), "finite"),
            (ok.replacen("1 0 0", "1 0 inf", 1), "finite"),
            (format!("DOMAIN_MIN 1 1 1\nDOMAIN_MAX 0 0 0\n{ok}"), "DOMAIN"),
            (format!("DOMAIN_MIN 0 0\n{ok}"), "DOMAIN_MIN"),
            (format!("BOGUS 1\n{ok}"), "BOGUS"),
            ("LUT_3D_SIZE 2\n\0\u{1}\u{ff}".into(), "number"),
        ];
        for (text, needle) in cases {
            let e = parse_cube(&text).expect_err(&text);
            assert!(e.contains(needle), "`{needle}` not in `{e}` for {text:?}");
        }
    }

    #[test]
    fn never_panics_on_truncated_or_garbled_input() {
        let ok = cube(3, |c| [c[1], c[2], c[0]]);
        for n in 0..ok.len().min(400) {
            if ok.is_char_boundary(n) {
                let _ = parse_cube(&ok[..n]);
            }
        }
        let mut seed = 7u64;
        for _ in 0..300 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let junk: String = (0..40).map(|i| ((seed >> (i % 50)) as u8 % 95 + 32) as char).collect();
            let _ = parse_cube(&junk);
            let _ = parse_cube(&format!("LUT_3D_SIZE 2\n{junk}"));
        }
    }

    #[test]
    fn rgba_helper_and_cube_text_round_trip() {
        let l = bundled_look("warm").unwrap();
        let rgba = l.to_rgba_f32();
        assert_eq!(rgba.len(), l.size.pow(3) * 4);
        assert!(rgba.chunks(4).all(|p| p[3] == 1.0));
        assert_eq!(&rgba[..3], &l.data[..3]);
        let again = parse_cube(&l.to_cube()).unwrap();
        assert_eq!(again.size, l.size);
        assert!(again.data.iter().zip(&l.data).all(|(a, b)| (a - b).abs() < 1e-6));
    }

    #[test]
    fn six_bundled_looks_are_valid_and_distinct() {
        let all = bundled_looks();
        let names: Vec<&str> = all.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, ["warm", "cool", "cinematic", "faded", "high-contrast", "mono"]);
        assert_eq!(names, BUNDLED_LOOK_NAMES);
        let id = Lut3D::identity(17);
        for (n, l) in &all {
            assert_eq!((l.size, l.data.len()), (17, 17 * 17 * 17 * 3), "{n}");
            assert!(l.data.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)), "{n}");
            assert_ne!(l.data, id.data, "{n} must change the picture");
            assert!(parse_cube(&l.to_cube()).is_ok(), "{n}");
            assert_eq!(bundled_look(n).unwrap(), *l);
        }
        assert!(bundled_look("nope").is_none());
    }

    #[test]
    fn bundled_looks_are_monotonic_along_the_grey_ramp() {
        for (n, l) in bundled_looks() {
            let mut prev = [0.0f32; 3];
            for i in 0..=64 {
                let v = i as f32 / 64.0;
                let o = l.sample([v, v, v]);
                for c in 0..3 {
                    assert!(o[c] + 1e-5 >= prev[c], "{n} channel {c} drops at {v}: {} < {}", o[c], prev[c]);
                }
                prev = o;
            }
        }
    }

    #[test]
    fn mono_has_equal_channels_and_keeps_black_and_white() {
        let m = bundled_look("mono").unwrap();
        for c in m.data.chunks(3) {
            assert!(c[0] == c[1] && c[1] == c[2], "{c:?}");
        }
        assert!(near(m.sample([0.0; 3]), [0.0; 3], 1e-6) && near(m.sample([1.0; 3]), [1.0; 3], 1e-4));
        // green is brighter than blue (luma weights)
        assert!(m.sample([0.0, 1.0, 0.0])[0] > m.sample([0.0, 0.0, 1.0])[0]);
    }

    #[test]
    fn warm_is_warmer_and_cool_is_cooler_and_cinematic_splits_tones() {
        let mid = [0.5, 0.5, 0.5];
        let (w, c) = (bundled_look("warm").unwrap().sample(mid), bundled_look("cool").unwrap().sample(mid));
        assert!(w[0] > w[2] && c[2] > c[0], "{w:?} {c:?}");
        let k = bundled_look("cinematic").unwrap();
        let (shadow, high) = (k.sample([0.2; 3]), k.sample([0.8; 3]));
        assert!(shadow[2] > shadow[0], "teal shadows {shadow:?}");
        assert!(high[0] > high[2], "orange highlights {high:?}");
        let f = bundled_look("faded").unwrap();
        assert!(f.sample([0.0; 3])[0] > 0.02 && f.sample([1.0; 3])[0] < 0.98, "faded lifts blacks and lowers whites");
        let hc = bundled_look("high-contrast").unwrap();
        assert!(hc.sample([0.25; 3])[0] < 0.25 && hc.sample([0.75; 3])[0] > 0.75);
    }

    #[test]
    fn identity_leaves_values_unchanged_and_intensity_blends() {
        let id = Lut3D::identity(17);
        let mut seed = 3u64;
        for _ in 0..200 {
            let mut c = [0.0f32; 3];
            for v in &mut c {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                *v = ((seed >> 40) as f32) / ((1u64 << 24) as f32);
            }
            assert!(near(id.sample(c), c, 1e-5), "{c:?}");
        }
        for (n, l) in bundled_looks() {
            assert_eq!(l.with_intensity(0.0).data, id.data, "{n}: intensity 0 is the identity");
            assert_eq!(l.with_intensity(1.0).data, l.data, "{n}");
            let half = l.with_intensity(0.5);
            assert!(half.data.iter().zip(l.data.iter().zip(&id.data)).all(|(h, (a, b))| (h - (a + b) / 2.0).abs() < 1e-6), "{n}");
            assert_eq!(l.with_intensity(f32::NAN).data, id.data, "{n}: nan is off");
            assert_eq!(l.with_intensity(7.0).data, l.data, "{n}: clamped");
        }
    }
}
