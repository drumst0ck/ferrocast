//! macOS `.textClipping` reader (text lives in the resource fork).

use std::path::Path;

const MAC_ROMAN_HIGH: [char; 128] = [
    'Ä', 'Å', 'Ç', 'É', 'Ñ', 'Ö', 'Ü', 'á', 'à', 'â', 'ä', 'ã', 'å', 'ç', 'é', 'è', //
    'ê', 'ë', 'í', 'ì', 'î', 'ï', 'ñ', 'ó', 'ò', 'ô', 'ö', 'õ', 'ú', 'ù', 'û', 'ü', //
    '†', '°', '¢', '£', '§', '•', '¶', 'ß', '®', '©', '™', '´', '¨', '≠', 'Æ', 'Ø', //
    '∞', '±', '≤', '≥', '¥', 'µ', '∂', '∑', '∏', 'π', '∫', 'ª', 'º', 'Ω', 'æ', 'ø', //
    '¿', '¡', '¬', '√', 'ƒ', '≈', '∆', '«', '»', '…', '\u{a0}', 'À', 'Ã', 'Õ', 'Œ', 'œ', //
    '–', '—', '“', '”', '‘', '’', '÷', '◊', 'ÿ', 'Ÿ', '⁄', '€', '‹', '›', 'ﬁ', 'ﬂ', //
    '‡', '·', '‚', '„', '‰', 'Â', 'Ê', 'Á', 'Ë', 'È', 'Í', 'Î', 'Ï', 'Ì', 'Ó', 'Ô', //
    '\u{f8ff}', 'Ò', 'Ú', 'Û', 'Ù', 'ı', 'ˆ', '˜', '¯', '˘', '˙', '˚', '¸', '˝', '˛', 'ˇ',
];

fn decode_mac_roman(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| match b {
            b'\r' => '\n',
            0..=0x7f => b as char,
            _ => MAC_ROMAN_HIGH[(b - 0x80) as usize],
        })
        .collect()
}

fn decode_utf16_be(bytes: &[u8]) -> String {
    let mut units: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
    if units.first() == Some(&0xFEFF) {
        units.remove(0);
    }
    String::from_utf16_lossy(&units).replace('\r', "\n")
}

fn be32(b: &[u8], at: usize) -> Option<usize> {
    let s = b.get(at..at.checked_add(4)?)?;
    Some(u32::from_be_bytes([s[0], s[1], s[2], s[3]]) as usize)
}

fn be16(b: &[u8], at: usize) -> Option<usize> {
    let s = b.get(at..at.checked_add(2)?)?;
    Some(u16::from_be_bytes([s[0], s[1]]) as usize)
}

/// Find the data of the first resource of `kind` in a classic resource fork.
fn find_resource<'a>(b: &'a [u8], kind: &[u8; 4]) -> Option<&'a [u8]> {
    let data_off = be32(b, 0)?;
    let map_off = be32(b, 4)?;
    let type_list = map_off.checked_add(be16(b, map_off.checked_add(24)?)?)?;
    let count = be16(b, type_list)?.wrapping_add(1) & 0xFFFF;
    for i in 0..count {
        let e = type_list.checked_add(2 + i * 8)?;
        if b.get(e..e.checked_add(4)?)? != kind {
            continue;
        }
        let refs = type_list.checked_add(be16(b, e + 6)?)?;
        // first reference entry: id(2) name(2) attrs+offset(4) handle(4)
        let off = be32(b, refs.checked_add(4)?)? & 0x00FF_FFFF;
        let at = data_off.checked_add(off)?;
        let len = be32(b, at)?;
        return b.get(at.checked_add(4)?..at.checked_add(4)?.checked_add(len)?);
    }
    None
}

/// Extract text from a resource fork: `utxt` (UTF-16 BE), else `TEXT` (MacRoman).
pub fn parse_rsrc(bytes: &[u8]) -> Option<String> {
    if let Some(d) = find_resource(bytes, b"utxt") {
        return Some(decode_utf16_be(d));
    }
    find_resource(bytes, b"TEXT").map(decode_mac_roman)
}

/// Read the text of a `.textClipping` file.
pub fn read_text_clipping(path: &Path) -> Result<String, String> {
    let fork = path.join("..namedfork/rsrc");
    if let Ok(bytes) = std::fs::read(&fork) {
        if !bytes.is_empty() {
            if let Some(t) = parse_rsrc(&bytes) {
                return Ok(t);
            }
        }
    }
    match std::fs::read(path) {
        Ok(data) if !data.is_empty() => match String::from_utf8(data) {
            Ok(t) => Ok(t),
            Err(_) => Err("textClipping has no readable text".to_string()),
        },
        Ok(_) => Err("textClipping has no text (empty file, no resource fork)".to_string()),
        Err(e) => Err(format!("cannot read textClipping: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a resource fork holding the given `(type, data)` resources.
    fn build_rsrc(resources: &[(&[u8; 4], &[u8])]) -> Vec<u8> {
        let mut data = Vec::new();
        let mut offsets = Vec::new();
        for (_, d) in resources {
            offsets.push(data.len());
            data.extend((d.len() as u32).to_be_bytes());
            data.extend(*d);
        }
        let n = resources.len();
        let mut map = vec![0u8; 24];
        let type_list_off = 28u16;
        map.extend(type_list_off.to_be_bytes());
        map.extend(0u16.to_be_bytes()); // name list offset
        map.extend(((n as u16).wrapping_sub(1)).to_be_bytes());
        let refs_start = 2 + n * 8;
        for (i, (kind, _)) in resources.iter().enumerate() {
            map.extend(*kind);
            map.extend(0u16.to_be_bytes()); // one resource of this type
            map.extend(((refs_start + i * 12) as u16).to_be_bytes());
        }
        for (i, _) in resources.iter().enumerate() {
            map.extend(128u16.to_be_bytes());
            map.extend(0xFFFFu16.to_be_bytes());
            map.extend((offsets[i] as u32).to_be_bytes());
            map.extend(0u32.to_be_bytes());
        }
        let mut out = Vec::new();
        out.extend(256u32.to_be_bytes());
        out.extend((256 + data.len() as u32).to_be_bytes());
        out.extend((data.len() as u32).to_be_bytes());
        out.extend((map.len() as u32).to_be_bytes());
        out.resize(256, 0);
        out.extend(data);
        out.extend(map);
        out
    }

    fn utf16be(s: &str, bom: bool) -> Vec<u8> {
        let mut v: Vec<u16> = Vec::new();
        if bom {
            v.push(0xFEFF);
        }
        v.extend(s.encode_utf16());
        v.iter().flat_map(|u| u.to_be_bytes()).collect()
    }

    #[test]
    fn parses_utxt_with_bom() {
        let fork = build_rsrc(&[(b"TEXT", b"old"), (b"utxt", &utf16be("Hola ñandú ✓", true))]);
        assert_eq!(parse_rsrc(&fork).as_deref(), Some("Hola ñandú ✓"));
    }

    #[test]
    fn falls_back_to_mac_roman_text() {
        let fork = build_rsrc(&[(b"TEXT", b"caf\x8e\rok \xd2q\xd3")]);
        assert_eq!(parse_rsrc(&fork).as_deref(), Some("café\nok “q”"));
    }

    #[test]
    fn rejects_garbage_and_missing_types() {
        assert_eq!(parse_rsrc(&[]), None);
        assert_eq!(parse_rsrc(&[0xff; 64]), None);
        let fork = build_rsrc(&[(b"icns", b"zz")]);
        assert_eq!(parse_rsrc(&fork), None);
    }

    #[test]
    fn reads_resource_fork_from_file() {
        let dir = std::env::temp_dir().join(format!("fc-clip-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.textClipping");
        std::fs::write(&f, b"").unwrap();
        let fork = build_rsrc(&[(b"utxt", &utf16be("from fork", false))]);
        if std::fs::write(f.join("..namedfork/rsrc"), &fork).is_ok() {
            assert_eq!(read_text_clipping(&f).unwrap(), "from fork");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn data_fork_utf8_fallback_and_empty() {
        let dir = std::env::temp_dir().join(format!("fc-clip2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("b.textClipping");
        std::fs::write(&f, "plain text").unwrap();
        assert_eq!(read_text_clipping(&f).unwrap(), "plain text");
        let e = dir.join("c.textClipping");
        std::fs::write(&e, b"").unwrap();
        assert!(read_text_clipping(&e).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
