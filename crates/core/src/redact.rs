//! Image redaction options (Tangerine redactImage): detect faces or text, or cover several typed
//! areas, with a solid colour or a blur. Pure parsing and geometry; pixels are changed by the app.

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Style {
    Solid([u8; 3]),
    /// Gaussian blur radius in pixels.
    Blur(f32),
}

/// Pixel rectangle, top-left origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Area {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    Faces,
    Text,
    Areas(Vec<Area>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RedactSpec {
    pub source: Source,
    pub style: Style,
}

/// Area clamped to an `iw × ih` image; `None` when nothing of it lies inside.
pub fn clamp_area(a: Area, iw: u32, ih: u32) -> Option<Area> {
    if a.x >= iw || a.y >= ih || a.w == 0 || a.h == 0 {
        return None;
    }
    Some(Area { x: a.x, y: a.y, w: a.w.min(iw - a.x), h: a.h.min(ih - a.y) })
}

/// `redact faces`, `redact text blur 25`, `redact 10,10 200x100 300,40 80x80 color ff0000`.
/// Several areas are allowed; `blur` takes an optional radius (default 20); default style is black.
pub fn parse(q: &str) -> Option<RedactSpec> {
    let q = q.trim().to_lowercase();
    let rest = q.strip_prefix("redact")?;
    if !rest.starts_with(' ') {
        return None;
    }
    let mut style = Style::Solid([0, 0, 0]);
    let (mut faces, mut text, mut areas) = (false, false, vec![]);
    let toks: Vec<&str> = rest.split_whitespace().collect();
    let mut i = 0;
    while i < toks.len() {
        match toks[i] {
            "faces" => faces = true,
            "text" => text = true,
            "blur" => {
                let r = match toks.get(i + 1).and_then(|v| v.parse::<f32>().ok()) {
                    Some(v) => {
                        i += 1;
                        v
                    }
                    None => 20.0,
                };
                if !(1.0..=200.0).contains(&r) {
                    return None;
                }
                style = Style::Blur(r);
            }
            "color" | "colour" | "solid" => {
                if let Some(c) = toks.get(i + 1).and_then(|v| crate::collage::hex(v)) {
                    style = Style::Solid(c);
                    i += 1;
                } else if toks[i] != "solid" {
                    return None;
                } else {
                    style = Style::Solid([0, 0, 0]);
                }
            }
            pos => {
                let (x, y) = pos.split_once(',')?;
                let (w, h) = toks.get(i + 1)?.split_once('x')?;
                areas.push(Area {
                    x: x.parse().ok()?,
                    y: y.parse().ok()?,
                    w: w.parse().ok().filter(|v| *v > 0)?,
                    h: h.parse().ok().filter(|v| *v > 0)?,
                });
                i += 1;
            }
        }
        i += 1;
    }
    let source = match (faces, text, areas.is_empty()) {
        (true, false, true) => Source::Faces,
        (false, true, true) => Source::Text,
        (false, false, false) => Source::Areas(areas),
        _ => return None,
    };
    Some(RedactSpec { source, style })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detection_and_styles() {
        assert_eq!(parse("redact faces").unwrap(), RedactSpec { source: Source::Faces, style: Style::Solid([0, 0, 0]) });
        assert_eq!(parse("redact text blur").unwrap().style, Style::Blur(20.0));
        assert_eq!(parse("redact text blur 35").unwrap().style, Style::Blur(35.0));
        assert_eq!(parse("redact faces color ff0000").unwrap().style, Style::Solid([255, 0, 0]));
        assert_eq!(parse("redact faces blur 5 solid").unwrap().style, Style::Solid([0, 0, 0]));
    }

    #[test]
    fn several_areas() {
        let s = parse("redact 10,20 200x100 300,40 80x80 blur 10").unwrap();
        assert_eq!(s.source, Source::Areas(vec![Area { x: 10, y: 20, w: 200, h: 100 }, Area { x: 300, y: 40, w: 80, h: 80 }]));
        assert_eq!(s.style, Style::Blur(10.0));
    }

    #[test]
    fn rejects_bad_input() {
        for bad in [
            "redact",
            "redactx faces",
            "redact faces text",
            "redact faces 1,1 2x2",
            "redact 1,1",
            "redact 1,1 0x5",
            "redact blur 0",
            "redact blur 999",
            "redact color zz",
            "redact wat",
            "faces",
        ] {
            assert_eq!(parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn clamps_to_the_image() {
        let a = Area { x: 90, y: 10, w: 50, h: 5 };
        assert_eq!(clamp_area(a, 100, 100), Some(Area { x: 90, y: 10, w: 10, h: 5 }));
        assert_eq!(clamp_area(Area { x: 100, y: 0, w: 5, h: 5 }, 100, 100), None);
        assert_eq!(clamp_area(Area { x: 0, y: 0, w: 0, h: 5 }, 100, 100), None);
    }
}
