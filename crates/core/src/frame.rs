//! "Add a background" (Tangerine frameImage): put a picture on a colour or on a blurred copy of
//! itself, with padding, rounded corners and a drop shadow. Pure options and geometry.

#[derive(Debug, Clone, PartialEq)]
pub enum Backdrop {
    Color([u8; 3]),
    /// Another picture, scaled to cover the canvas (path as typed).
    Image(String),
    /// The picture itself, scaled to cover the canvas and blurred with this radius.
    Blur(f32),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FrameOpts {
    /// Space around the picture, in pixels.
    pub pad: u32,
    pub radius: u32,
    /// Shadow blur radius; 0 is no shadow.
    pub shadow: u32,
    pub backdrop: Backdrop,
}

impl Default for FrameOpts {
    fn default() -> Self {
        FrameOpts { pad: 64, radius: 0, shadow: 0, backdrop: Backdrop::Color([255, 255, 255]) }
    }
}

/// Canvas size for a `w × h` picture.
pub fn canvas_size(w: u32, h: u32, o: &FrameOpts) -> (u32, u32) {
    (w + 2 * o.pad, h + 2 * o.pad)
}

/// Where the shadow is drawn: the picture's rectangle moved down by a third of the blur.
pub fn shadow_offset(o: &FrameOpts) -> (i32, i32) {
    (0, (o.shadow / 3) as i32)
}

/// `frame pad 80 radius 24 shadow 30 bg 222222` or `frame blur` / `frame blur 40`.
/// Words may come in any order; `frame` alone uses the defaults.
pub fn parse(q: &str) -> Option<FrameOpts> {
    // `frame image /Users/Me/bg.jpg pad 40`: the path keeps its case; it ends at the next keyword
    let raw = q.trim();
    // ASCII-only lowercasing keeps byte offsets identical to `raw` (Unicode lowercasing can change them)
    if raw.to_ascii_lowercase().starts_with("frame") {
        if let Some(pos) = raw.to_ascii_lowercase().find(" image ") {
            let after = &raw[pos + " image ".len()..];
            let low = after.to_ascii_lowercase();
            let end = [" pad ", " radius ", " shadow "].iter().filter_map(|k| low.find(k)).min().unwrap_or(after.len());
            let path = after[..end].trim();
            if path.is_empty() {
                return None;
            }
            let rest = format!("{}{}", &raw[..pos], &after[end..]);
            let mut o = parse(&rest)?;
            o.backdrop = Backdrop::Image(path.to_string());
            return Some(o);
        }
    }
    let q = q.trim().to_lowercase();
    let rest = q.strip_prefix("frame")?;
    if !(rest.is_empty() || rest.starts_with(' ')) {
        return None;
    }
    let mut o = FrameOpts::default();
    let mut it = rest.split_whitespace().peekable();
    while let Some(w) = it.next() {
        match w {
            "pad" => o.pad = it.next()?.parse().ok().filter(|v| *v <= 2000)?,
            "radius" => o.radius = it.next()?.parse().ok().filter(|v| *v <= 2000)?,
            "shadow" => o.shadow = it.next()?.parse().ok().filter(|v| *v <= 200)?,
            "bg" => o.backdrop = Backdrop::Color(crate::collage::hex(it.next()?)?),
            "blur" => {
                let r = match it.peek().and_then(|v| v.parse::<f32>().ok()) {
                    Some(v) => {
                        it.next();
                        v
                    }
                    None => 30.0,
                };
                if !(1.0..=200.0).contains(&r) {
                    return None;
                }
                o.backdrop = Backdrop::Blur(r);
            }
            _ => return None,
        }
    }
    Some(o)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_adds_padding_on_every_side() {
        assert_eq!(canvas_size(100, 50, &FrameOpts { pad: 10, ..FrameOpts::default() }), (120, 70));
        assert_eq!(canvas_size(100, 50, &FrameOpts { pad: 0, ..FrameOpts::default() }), (100, 50));
        assert_eq!(shadow_offset(&FrameOpts { shadow: 30, ..FrameOpts::default() }), (0, 10));
    }

    #[test]
    fn typed_options() {
        let o = parse("frame pad 80 radius 24 shadow 30 bg 222222").unwrap();
        assert_eq!((o.pad, o.radius, o.shadow, o.backdrop), (80, 24, 30, Backdrop::Color([0x22, 0x22, 0x22])));
        assert_eq!(parse("frame").unwrap(), FrameOpts::default());
        assert_eq!(parse("frame blur").unwrap().backdrop, Backdrop::Blur(30.0));
        assert_eq!(parse("frame blur 40 pad 10").unwrap().backdrop, Backdrop::Blur(40.0));
        assert_eq!(parse("frame blur 0"), None);
        assert_eq!(parse("frame pad"), None);
        assert_eq!(parse("frame shadow 999"), None);
        assert_eq!(parse("frame bg nothex"), None);
        assert_eq!(parse("frame wat"), None);
        assert_eq!(parse("framed"), None);
        let o = parse("Frame image /Users/Me/My BG.JPG pad 40 shadow 10").unwrap();
        assert_eq!((o.pad, o.shadow, o.backdrop), (40, 10, Backdrop::Image("/Users/Me/My BG.JPG".into())));
        assert_eq!(parse("frame image /a.png").unwrap().backdrop, Backdrop::Image("/a.png".into()));
        assert_eq!(parse("frame image "), None);
        assert_eq!(parse("frame image /a.png radius x"), None);
        // non-ASCII input must not panic, whatever its lowercase length
        for q in ["frameİİ image x", "frame image /İİ/a.png pad 5", "İİİİİİ frame image x", "frame image \u{212a}\u{212a}"] {
            let _ = parse(q);
        }
        assert_eq!(parse("frame image /İ/a.png pad 5").unwrap().backdrop, Backdrop::Image("/İ/a.png".into()));
    }
}
