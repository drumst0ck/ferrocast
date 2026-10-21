//! Minimal EPUB reader: flattens the spine into a single HTML document.

use std::path::Path;
use std::process::Command;

/// Local name (prefix stripped) and attributes of a tag's inner text
/// (everything between `<` and `>`).
fn parse_tag(inner: &str) -> (String, Vec<(String, String)>) {
    let inner = inner.trim().trim_end_matches('/').trim();
    let name_end = inner.find(|c: char| c.is_whitespace()).unwrap_or(inner.len());
    let full = &inner[..name_end];
    let local = full.rsplit(':').next().unwrap_or(full).to_ascii_lowercase();
    let mut attrs = Vec::new();
    let mut rest = &inner[name_end..];
    loop {
        rest = rest.trim_start();
        let Some(eq) = rest.find('=') else { break };
        let key = rest[..eq].trim();
        let key = key.rsplit(':').next().unwrap_or(key).to_ascii_lowercase();
        let after = rest[eq + 1..].trim_start();
        let Some(q) = after.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            break;
        };
        let body = &after[1..];
        let Some(end) = body.find(q) else { break };
        attrs.push((key, body[..end].to_string()));
        rest = &body[end + 1..];
    }
    (local, attrs)
}

fn attr<'a>(attrs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

/// Iterate `(inner_tag_text, text_after_tag_until_next_lt)` over an XML string.
fn tags(xml: &str) -> Vec<(&str, &str)> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(lt) = rest.find('<') {
        let after = &rest[lt + 1..];
        let Some(gt) = after.find('>') else { break };
        let inner = &after[..gt];
        let tail = &after[gt + 1..];
        let text_end = tail.find('<').unwrap_or(tail.len());
        out.push((inner, &tail[..text_end]));
        rest = tail;
    }
    out
}

/// Extract the OPF `full-path` from META-INF/container.xml.
pub fn parse_container(xml: &str) -> Option<String> {
    for (inner, _) in tags(xml) {
        if inner.starts_with('/') || inner.starts_with('?') || inner.starts_with('!') {
            continue;
        }
        let (name, attrs) = parse_tag(inner);
        if name == "rootfile" {
            if let Some(p) = attr(&attrs, "full-path") {
                if !p.is_empty() {
                    return Some(p.to_string());
                }
            }
        }
    }
    None
}

/// Parse an OPF: returns `(title, spine hrefs in order)`; hrefs are as written
/// in the manifest (not resolved). Spine items missing from the manifest are skipped.
pub fn parse_opf(xml: &str) -> (String, Vec<String>) {
    let mut title = String::new();
    let mut manifest: Vec<(String, String)> = Vec::new();
    let mut idrefs: Vec<String> = Vec::new();
    for (inner, text) in tags(xml) {
        if inner.starts_with('/') || inner.starts_with('?') || inner.starts_with('!') {
            continue;
        }
        let (name, attrs) = parse_tag(inner);
        match name.as_str() {
            "title" if title.is_empty() => title = text.trim().to_string(),
            "item" => {
                if let (Some(id), Some(href)) = (attr(&attrs, "id"), attr(&attrs, "href")) {
                    manifest.push((id.to_string(), href.to_string()));
                }
            }
            "itemref" => {
                if let Some(r) = attr(&attrs, "idref") {
                    idrefs.push(r.to_string());
                }
            }
            _ => {}
        }
    }
    let hrefs = idrefs.iter().filter_map(|r| manifest.iter().find(|(id, _)| id == r).map(|(_, h)| h.clone())).collect();
    (title, hrefs)
}

/// Inner HTML of `<body>`; the whole input when there is no body element.
pub fn body_inner(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let Some(start) = lower.find("<body") else {
        return html.trim().to_string();
    };
    let Some(gt) = lower[start..].find('>') else {
        return html.trim().to_string();
    };
    let from = start + gt + 1;
    let to = lower.rfind("</body").filter(|t| *t >= from).unwrap_or(html.len());
    html[from..to].trim().to_string()
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Some(v) = s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn unzip_entry(epub: &Path, entry: &str) -> Result<String, String> {
    let out = Command::new("/usr/bin/unzip").arg("-p").arg(epub).arg(entry).output().map_err(|e| format!("unzip failed to start: {e}"))?;
    if !out.status.success() {
        return Err(format!("cannot read '{entry}' from EPUB (not a valid zip?)"));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Resolve `href` relative to `base_dir` ("" or "OEBPS/"), dropping fragments and `.`/`..`.
fn resolve(base_dir: &str, href: &str) -> String {
    let href = href.split('#').next().unwrap_or("");
    let href = percent_decode(href);
    let mut parts: Vec<&str> = Vec::new();
    let joined = format!("{base_dir}{href}");
    for seg in joined.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

/// Convert an EPUB file into one HTML document (spine order concatenated).
pub fn epub_to_html(path: &Path) -> Result<String, String> {
    let container = unzip_entry(path, "META-INF/container.xml")
        .map_err(|_| "not an EPUB: META-INF/container.xml missing or file is not a zip".to_string())?;
    let opf_path = parse_container(&container).ok_or_else(|| "EPUB container.xml has no rootfile".to_string())?;
    let opf_path = percent_decode(&opf_path);
    let opf = unzip_entry(path, &opf_path)?;
    let (title, hrefs) = parse_opf(&opf);
    if hrefs.is_empty() {
        return Err("EPUB spine is empty".to_string());
    }
    let base = match opf_path.rfind('/') {
        Some(i) => &opf_path[..=i],
        None => "",
    };
    let mut bodies = Vec::new();
    for href in &hrefs {
        let entry = resolve(base, href);
        if let Ok(html) = unzip_entry(path, &entry) {
            let body = body_inner(&html);
            if !body.is_empty() {
                bodies.push(body);
            }
        }
    }
    if bodies.is_empty() {
        return Err("EPUB spine has no readable chapters".to_string());
    }
    Ok(format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>{}</title></head><body>{}</body></html>",
        title,
        bodies.join("\n\n")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_container_finds_rootfile() {
        let xml = r#"<?xml version="1.0"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
 <rootfiles><rootfile media-type="application/oebps-package+xml" full-path="OEBPS/content.opf"/></rootfiles></container>"#;
        assert_eq!(parse_container(xml).as_deref(), Some("OEBPS/content.opf"));
        let single = "<rootfile full-path='a b/x.opf' media-type='x'></rootfile>";
        assert_eq!(parse_container(single).as_deref(), Some("a b/x.opf"));
        assert_eq!(parse_container("<container/>"), None);
    }

    #[test]
    fn parse_opf_spine_order_and_prefixes() {
        let xml = r#"<opf:package xmlns:opf="http://www.idpf.org/2007/opf" xmlns:dc="http://purl.org/dc/elements/1.1/">
<opf:metadata><dc:title>My Book</dc:title></opf:metadata>
<opf:manifest>
 <opf:item href="ch2.xhtml" id="b" media-type="application/xhtml+xml"/>
 <opf:item id='a' href='ch%201.xhtml' media-type='application/xhtml+xml'/>
 <opf:item id="css" href="s.css"/>
</opf:manifest>
<opf:spine><opf:itemref idref="a"/><opf:itemref idref="missing"/><opf:itemref idref="b"/></opf:spine>
</opf:package>"#;
        let (title, spine) = parse_opf(xml);
        assert_eq!(title, "My Book");
        assert_eq!(spine, vec!["ch%201.xhtml", "ch2.xhtml"]);
    }

    #[test]
    fn body_inner_variants() {
        assert_eq!(body_inner("<html><head></head><BODY class=\"x\"><p>Hi</p></BODY></html>"), "<p>Hi</p>");
        assert_eq!(body_inner("<p>no body</p>"), "<p>no body</p>");
    }

    #[test]
    fn resolve_and_decode() {
        assert_eq!(resolve("OEBPS/", "ch%201.xhtml#x"), "OEBPS/ch 1.xhtml");
        assert_eq!(resolve("OEBPS/", "../t/a.xhtml"), "t/a.xhtml");
        assert_eq!(resolve("", "a.xhtml"), "a.xhtml");
        assert_eq!(percent_decode("100%"), "100%");
    }

    #[test]
    fn rejects_non_zip() {
        let dir = std::env::temp_dir().join(format!("fc-epub-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("bad.epub");
        std::fs::write(&f, b"not a zip").unwrap();
        let err = epub_to_html(&f).unwrap_err();
        assert!(err.contains("not an EPUB"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn builds_real_epub_and_reads_in_order() {
        if !Path::new("/usr/bin/zip").exists() || !Path::new("/usr/bin/unzip").exists() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("fc-epub-ok-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("META-INF")).unwrap();
        std::fs::create_dir_all(dir.join("OEBPS")).unwrap();
        std::fs::write(dir.join("mimetype"), "application/epub+zip").unwrap();
        std::fs::write(
            dir.join("META-INF/container.xml"),
            r#"<container><rootfiles><rootfile full-path="OEBPS/content.opf"/></rootfiles></container>"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("OEBPS/content.opf"),
            r#"<package><metadata><dc:title>Tiny</dc:title></metadata>
<manifest><item id="c1" href="one.xhtml"/><item id="c2" href="two.xhtml"/></manifest>
<spine><itemref idref="c2"/><itemref idref="c1"/></spine></package>"#,
        )
        .unwrap();
        std::fs::write(dir.join("OEBPS/one.xhtml"), "<html><body><h1>One</h1></body></html>").unwrap();
        std::fs::write(dir.join("OEBPS/two.xhtml"), "<html><body><h1>Two</h1></body></html>").unwrap();
        let epub = dir.join("t.epub");
        let ok = Command::new("/usr/bin/zip")
            .current_dir(&dir)
            .args(["-q", "-X", "t.epub", "mimetype", "META-INF/container.xml"])
            .arg("OEBPS/content.opf")
            .arg("OEBPS/one.xhtml")
            .arg("OEBPS/two.xhtml")
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            return;
        }
        let html = epub_to_html(&epub).unwrap();
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.contains("<title>Tiny</title>"));
        let two = html.find("<h1>Two</h1>").unwrap();
        let one = html.find("<h1>One</h1>").unwrap();
        assert!(two < one, "spine order not respected: {html}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
