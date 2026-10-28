//! Color detection and conversion for Root Search (`#ff6363`, `rgb(...)`, `hsl(...)`).

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    /// 0.0..=1.0
    pub a: f32,
}

fn parse_component(s: &str, max: f32) -> Option<f32> {
    let s = s.trim();
    if let Some(p) = s.strip_suffix('%') {
        return Some(p.trim().parse::<f32>().ok()? / 100.0 * max);
    }
    s.parse::<f32>().ok()
}

fn parse_alpha(s: &str) -> Option<f32> {
    let s = s.trim();
    if let Some(p) = s.strip_suffix('%') {
        return Some((p.trim().parse::<f32>().ok()? / 100.0).clamp(0.0, 1.0));
    }
    Some(s.parse::<f32>().ok()?.clamp(0.0, 1.0))
}

/// Split `a, b, c / d` or `a b c / d` into components and optional alpha.
fn split_args(inner: &str) -> Option<(Vec<String>, Option<String>)> {
    let (main, alpha) = match inner.split_once('/') {
        Some((m, a)) => (m, Some(a.trim().to_string())),
        None => (inner, None),
    };
    let parts: Vec<String> = main.split(|c: char| c == ',' || c.is_whitespace()).filter(|p| !p.is_empty()).map(String::from).collect();
    match (parts.len(), alpha) {
        (3, a) => Some((parts, a)),
        (4, None) => {
            let mut p = parts;
            let a = p.pop();
            Some((p, a))
        }
        _ => None,
    }
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let h = h.rem_euclid(360.0) / 360.0;
    let hue = |p: f32, q: f32, mut t: f32| {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    let (r, g, b) = if s == 0.0 {
        (l, l, l)
    } else {
        let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
        let p = 2.0 * l - q;
        (hue(p, q, h + 1.0 / 3.0), hue(p, q, h), hue(p, q, h - 1.0 / 3.0))
    };
    let c = |v: f32| (v * 255.0).round().clamp(0.0, 255.0) as u8;
    (c(r), c(g), c(b))
}

impl Rgba {
    /// From 0-1 components (as AppKit gives them): rounded to 0-255, out-of-range and NaN clamped.
    pub fn from_unit(r: f64, g: f64, b: f64, a: f64) -> Rgba {
        let c = |v: f64| if v.is_nan() { 0 } else { (v.clamp(0.0, 1.0) * 255.0).round() as u8 };
        let a = if a.is_nan() { 1.0 } else { a.clamp(0.0, 1.0) as f32 };
        Rgba { r: c(r), g: c(g), b: c(b), a }
    }

    /// Parse a CSS-like color string. Named colors are intentionally not
    /// accepted so they don't hijack normal searches.
    pub fn parse(input: &str) -> Option<Rgba> {
        let s = input.trim().to_lowercase();
        if let Some(hex) = s.strip_prefix('#') {
            if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
                return None;
            }
            let expand = |h: &str| h.chars().flat_map(|c| [c, c]).collect::<String>();
            let full = match hex.len() {
                3 | 4 => expand(hex),
                6 | 8 => hex.to_string(),
                _ => return None,
            };
            let byte = |i: usize| u8::from_str_radix(&full[i..i + 2], 16).ok();
            let a = if full.len() == 8 { byte(6)? as f32 / 255.0 } else { 1.0 };
            return Some(Rgba { r: byte(0)?, g: byte(2)?, b: byte(4)?, a });
        }
        let (name, rest) = s.split_once('(')?;
        let inner = rest.strip_suffix(')')?;
        let (parts, alpha) = split_args(inner)?;
        let a = match alpha {
            Some(a) => parse_alpha(&a)?,
            None => 1.0,
        };
        match name.trim() {
            "rgb" | "rgba" => {
                let c = |i: usize| parse_component(&parts[i], 255.0).map(|v| v.round().clamp(0.0, 255.0) as u8);
                Some(Rgba { r: c(0)?, g: c(1)?, b: c(2)?, a })
            }
            "hsl" | "hsla" => {
                let h = parts[0].trim_end_matches("deg").parse::<f32>().ok()?;
                let sat = parse_component(&parts[1], 1.0)?;
                let l = parse_component(&parts[2], 1.0)?;
                let (r, g, b) = hsl_to_rgb(h, sat.clamp(0.0, 1.0), l.clamp(0.0, 1.0));
                Some(Rgba { r, g, b, a })
            }
            _ => None,
        }
    }

    pub fn to_hsl(&self) -> (f32, f32, f32) {
        let r = self.r as f32 / 255.0;
        let g = self.g as f32 / 255.0;
        let b = self.b as f32 / 255.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;
        if (max - min).abs() < f32::EPSILON {
            return (0.0, 0.0, l * 100.0);
        }
        let d = max - min;
        let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
        let h = if max == r {
            (g - b) / d + if g < b { 6.0 } else { 0.0 }
        } else if max == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        (h * 60.0, s * 100.0, l * 100.0)
    }

    /// All copyable representations: (label, value).
    pub fn formats(&self) -> Vec<(&'static str, String)> {
        let (h, s, l) = self.to_hsl();
        let a = self.a;
        let f = |v: f32| format!("{:.3}", v).trim_end_matches('0').trim_end_matches('.').to_string();
        vec![
            ("HEX", format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)),
            ("HEX with Alpha", format!("#{:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b, (a * 255.0).round() as u8)),
            ("RGB", format!("rgb({}, {}, {})", self.r, self.g, self.b)),
            ("RGBA", format!("rgba({}, {}, {}, {})", self.r, self.g, self.b, f(a))),
            ("RGB (CSS4)", format!("rgb({} {} {} / {}%)", self.r, self.g, self.b, (a * 100.0).round())),
            ("HSL", format!("hsl({}, {}%, {}%)", h.round(), s.round(), l.round())),
            ("HSLA", format!("hsla({}, {}%, {}%, {})", h.round(), s.round(), l.round(), f(a))),
            (
                "NSColor",
                format!(
                    "NSColor(red: {}, green: {}, blue: {}, alpha: {})",
                    f(self.r as f32 / 255.0),
                    f(self.g as f32 / 255.0),
                    f(self.b as f32 / 255.0),
                    f(a)
                ),
            ),
            (
                "UIColor",
                format!(
                    "UIColor(red: {}, green: {}, blue: {}, alpha: {})",
                    f(self.r as f32 / 255.0),
                    f(self.g as f32 / 255.0),
                    f(self.b as f32 / 255.0),
                    f(a)
                ),
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn from_unit_rounds_clamps_and_keeps_alpha() {
        let c = Rgba::from_unit(1.0, 0.5, 0.0, 1.0);
        assert_eq!((c.r, c.g, c.b, c.a), (255, 128, 0, 1.0));
        assert_eq!(c.formats()[0].1, "#FF8000");
        let c = Rgba::from_unit(-0.2, 1.4, f64::NAN, 0.25);
        assert_eq!((c.r, c.g, c.b, c.a), (0, 255, 0, 0.25));
    }

    use super::*;

    #[test]
    fn hex() {
        assert_eq!(Rgba::parse("#ff6363"), Some(Rgba { r: 255, g: 99, b: 99, a: 1.0 }));
        assert_eq!(Rgba::parse("#fff").map(|c| (c.r, c.g, c.b)), Some((255, 255, 255)));
        assert!(Rgba::parse("#ggg").is_none());
        assert!(Rgba::parse("#12345").is_none());
    }

    #[test]
    fn functional() {
        let c = Rgba::parse("rgb(255 107 53)").unwrap();
        assert_eq!((c.r, c.g, c.b), (255, 107, 53));
        let c = Rgba::parse("rgba(255, 99, 99, 0.5)").unwrap();
        assert!((c.a - 0.5).abs() < 1e-6);
        let c = Rgba::parse("hsl(0 100% 50%)").unwrap();
        assert_eq!((c.r, c.g, c.b), (255, 0, 0));
        assert!(Rgba::parse("red").is_none());
    }

    #[test]
    fn roundtrip_hsl() {
        let c = Rgba::parse("#336699").unwrap();
        let (h, s, l) = c.to_hsl();
        assert_eq!((h.round(), s.round(), l.round()), (210.0, 50.0, 40.0));
        assert_eq!(c.formats()[0].1, "#336699");
    }
}
