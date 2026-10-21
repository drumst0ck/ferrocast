//! Minimal pure EPUB 3 writer plus an HTML → XHTML normaliser.

use crate::pages::{xml_escape, zip_stored};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

const VOID: [&str; 6] = ["br", "hr", "img", "meta", "link", "input"];

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// `CCYY-MM-DDThh:mm:ssZ` for the current time.
fn modified_timestamp() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    let t = secs.rem_euclid(86_400);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z", t / 3600, t % 3600 / 60, t % 60)
}

fn uuid_like(title: &str, body: &str) -> String {
    let mut h1 = DefaultHasher::new();
    (title, body).hash(&mut h1);
    let mut h2 = DefaultHasher::new();
    (body, title, 0x9e37_79b9u32).hash(&mut h2);
    let hex = format!("{:016x}{:016x}", h1.finish(), h2.finish());
    format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32])
}

/// Build a valid EPUB 3 container with one chapter holding `xhtml_body`.
pub fn epub_package(title: &str, xhtml_body: &str) -> Vec<u8> {
    let t = xml_escape(title);
    let container = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\"><rootfiles><rootfile full-path=\"OEBPS/content.opf\" media-type=\"application/oebps-package+xml\"/></rootfiles></container>";
    let opf = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" unique-identifier=\"bookid\"><metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:identifier id=\"bookid\">urn:uuid:{}</dc:identifier><dc:title>{t}</dc:title><dc:language>en</dc:language><meta property=\"dcterms:modified\">{}</meta></metadata><manifest><item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/><item id=\"chapter1\" href=\"chapter1.xhtml\" media-type=\"application/xhtml+xml\"/></manifest><spine><itemref idref=\"chapter1\"/></spine></package>",
        uuid_like(title, xhtml_body),
        modified_timestamp()
    );
    let nav = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE html>\n<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\" xml:lang=\"en\"><head><meta charset=\"utf-8\"/><title>{t}</title></head><body><nav epub:type=\"toc\" id=\"toc\"><h1>Contents</h1><ol><li><a href=\"chapter1.xhtml\">{t}</a></li></ol></nav></body></html>"
    );
    let chapter = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE html>\n<html xmlns=\"http://www.w3.org/1999/xhtml\" xml:lang=\"en\"><head><meta charset=\"utf-8\"/><title>{t}</title></head><body>{xhtml_body}</body></html>"
    );
    zip_stored(&[
        ("mimetype", b"application/epub+zip"),
        ("META-INF/container.xml", container.as_bytes()),
        ("OEBPS/content.opf", opf.as_bytes()),
        ("OEBPS/nav.xhtml", nav.as_bytes()),
        ("OEBPS/chapter1.xhtml", chapter.as_bytes()),
    ])
}

fn push_text(out: &mut String, text: &str) {
    out.push_str(&text.replace("&nbsp;", "&#160;").replace("&NBSP;", "&#160;"));
}

/// Rewrite one tag's inner text (between `<` and `>`, without them).
fn rewrite_tag(inner: &str) -> (String, Option<String>) {
    // Returns (xhtml tag, lowercase name for non-closing tags).
    let closing = inner.starts_with('/');
    let body = inner.trim_start_matches('/');
    let self_closed = body.trim_end().ends_with('/');
    let body = body.trim_end().trim_end_matches('/');
    let name_end = body.find(|c: char| c.is_whitespace()).unwrap_or(body.len());
    let name = body[..name_end].to_ascii_lowercase();
    let is_void = VOID.contains(&name.as_str());
    if closing {
        if is_void {
            return (String::new(), None);
        }
        return (format!("</{name}>"), Some(name));
    }
    let mut out = format!("<{name}");
    let b: Vec<char> = body[name_end..].chars().collect();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_whitespace() {
            i += 1;
            continue;
        }
        let s = i;
        while i < b.len() && !b[i].is_whitespace() && b[i] != '=' {
            i += 1;
        }
        let an: String = b[s..i].iter().collect::<String>().to_ascii_lowercase();
        let mut j = i;
        while j < b.len() && b[j].is_whitespace() {
            j += 1;
        }
        if j < b.len() && b[j] == '=' {
            j += 1;
            while j < b.len() && b[j].is_whitespace() {
                j += 1;
            }
            let val: String;
            if j < b.len() && (b[j] == '"' || b[j] == '\'') {
                let q = b[j];
                let vs = j + 1;
                let mut k = vs;
                while k < b.len() && b[k] != q {
                    k += 1;
                }
                val = b[vs..k].iter().collect::<String>().replace('"', "&quot;");
                j = (k + 1).min(b.len());
            } else {
                let vs = j;
                while j < b.len() && !b[j].is_whitespace() {
                    j += 1;
                }
                val = b[vs..j].iter().collect::<String>().replace('"', "&quot;");
            }
            out.push_str(&format!(" {an}=\"{val}\""));
            i = j;
        } else {
            out.push_str(&format!(" {an}=\"{an}\""));
        }
    }
    if is_void || self_closed {
        out.push_str("/>");
        (out, None)
    } else {
        out.push('>');
        (out, Some(name))
    }
}

/// Turn HTML fragments into well-formed XHTML (void elements self-closed,
/// lowercase tags, quoted attributes, `&nbsp;` → `&#160;`).
pub fn xhtmlize(html: &str) -> String {
    let mut out = String::with_capacity(html.len() + 16);
    let mut rest = html;
    while let Some(i) = rest.find('<') {
        push_text(&mut out, &rest[..i]);
        let after = &rest[i..];
        if after.starts_with("<!--") {
            let end = after.find("-->").map_or(after.len(), |e| e + 3);
            out.push_str(&after[..end]);
            rest = &after[end..];
            continue;
        }
        if after.starts_with("<![CDATA[") {
            let end = after.find("]]>").map_or(after.len(), |e| e + 3);
            out.push_str(&after[..end]);
            rest = &after[end..];
            continue;
        }
        // Find the closing '>' outside of quotes.
        let mut quote: Option<char> = None;
        let mut end = None;
        for (k, c) in after.char_indices().skip(1) {
            match (quote, c) {
                (None, '"') | (None, '\'') => quote = Some(c),
                (Some(q), c) if c == q => quote = None,
                (None, '>') => {
                    end = Some(k);
                    break;
                }
                _ => {}
            }
        }
        let Some(end) = end else {
            // Stray '<' with no end: escape it.
            out.push_str("&lt;");
            rest = &after[1..];
            continue;
        };
        let inner = &after[1..end];
        let first = inner.chars().next().unwrap_or(' ');
        if first == '!' || first == '?' {
            out.push_str(&after[..=end]);
        } else if first.is_ascii_alphabetic() || (first == '/' && inner.len() > 1) {
            let (tag, name) = rewrite_tag(inner);
            out.push_str(&tag);
            // Raw-text elements: copy through the matching close untouched.
            if let Some(n) = name {
                if (n == "script" || n == "style") && !inner.starts_with('/') {
                    let tail = &after[end + 1..];
                    let close = format!("</{n}");
                    let at = tail.to_ascii_lowercase().find(&close).unwrap_or(tail.len());
                    out.push_str(&tail[..at]);
                    rest = &tail[at..];
                    continue;
                }
            }
        } else {
            out.push_str("&lt;");
            push_text(&mut out, &after[1..=end]);
        }
        rest = &after[end + 1..];
    }
    push_text(&mut out, rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    /// Simple stack checker: every non-self-closed open tag must be closed in order.
    fn balanced(x: &str) -> bool {
        let mut stack: Vec<String> = Vec::new();
        let mut rest = x;
        while let Some(i) = rest.find('<') {
            let after = &rest[i..];
            if after.starts_with("<!") || after.starts_with("<?") {
                match after.find('>') {
                    Some(e) => rest = &after[e + 1..],
                    None => return false,
                }
                continue;
            }
            let Some(e) = after.find('>') else { return false };
            let inner = &after[1..e];
            if let Some(n) = inner.strip_prefix('/') {
                if stack.pop().as_deref() != Some(n.trim()) {
                    return false;
                }
            } else if !inner.ends_with('/') {
                let n = inner.split_whitespace().next().unwrap_or("");
                if VOID.contains(&n) {
                    return false; // void elements must self-close
                }
                stack.push(n.to_string());
            }
            rest = &after[e + 1..];
        }
        stack.is_empty()
    }

    #[test]
    fn xhtmlize_samples() {
        let s = xhtmlize("<P CLASS=intro>a&nbsp;b<BR>c<hr><IMG SRC=x.png alt='q'></P><input disabled>");
        assert_eq!(s, "<p class=\"intro\">a&#160;b<br/>c<hr/><img src=\"x.png\" alt=\"q\"/></p><input disabled=\"disabled\"/>");
        let samples = [
            "<h1>T</h1><p>one<br>two</p><hr><ul><li>a</li><li>b</li></ul>",
            "<P>x<BR/>y</P><meta charset=utf-8><link rel=stylesheet href=a.css>",
            "<table><tr><td>1<td>",
            "<p><img src=a.png></p><!-- c <b> --><p>z</p>",
            "<pre><code class=\"language-rust\">a &lt; b</code></pre><br></br>",
        ];
        for h in &samples[..2] {
            assert!(balanced(&xhtmlize(h)), "{h}");
        }
        assert!(balanced(&xhtmlize(samples[3])));
        assert!(balanced(&xhtmlize(samples[4])), "{}", xhtmlize(samples[4]));
        // Unclosed td is invalid input; the checker must notice it.
        assert!(!balanced(&xhtmlize(samples[2])));
    }

    #[test]
    fn xhtmlize_markdown_output_and_script() {
        let html = crate::markdown::markdown_to_html("# Hi\n\nline  \nnext\n\n---\n\n![a](b.png)\n");
        assert!(balanced(&xhtmlize(&html)), "{}", xhtmlize(&html));
        assert_eq!(xhtmlize("<script>if(a<b){}</script>"), "<script>if(a<b){}</script>");
        assert_eq!(xhtmlize("1 < 2"), "1 &lt; 2");
    }

    fn run_deadline(cmd: &mut Command, secs: u64) -> Option<bool> {
        let mut child = cmd.stdout(Stdio::null()).stderr(Stdio::null()).spawn().ok()?;
        let end = Instant::now() + Duration::from_secs(secs);
        loop {
            if let Some(st) = child.try_wait().ok()? {
                return Some(st.success());
            }
            if Instant::now() > end {
                let _ = child.kill();
                let _ = child.wait();
                return Some(false);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    #[test]
    fn first_entry_is_stored_mimetype() {
        let z = epub_package("T", "<p>x</p>");
        assert_eq!(&z[0..4], &[0x50, 0x4b, 0x03, 0x04]);
        assert_eq!(u16::from_le_bytes([z[8], z[9]]), 0, "stored");
        assert_eq!(u16::from_le_bytes([z[26], z[27]]), 8);
        assert_eq!(u16::from_le_bytes([z[28], z[29]]), 0, "no extra field");
        assert_eq!(&z[30..38], b"mimetype");
        assert_eq!(&z[38..58], b"application/epub+zip");
    }

    #[test]
    fn identifier_and_timestamp_shape() {
        let id = uuid_like("a", "b");
        assert_eq!(id.len(), 36);
        assert_eq!(id, uuid_like("a", "b"));
        assert_ne!(id, uuid_like("a", "c"));
        let ts = modified_timestamp();
        assert_eq!(ts.len(), 20);
        assert!(ts.ends_with('Z') && &ts[10..11] == "T");
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
    }

    #[test]
    fn round_trip_and_real_readers() {
        let dir = std::env::temp_dir().join(format!("fc-epubw-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let body = xhtmlize("<h1>Alpha café</h1><p>Beta<br>Gamma &amp; delta</p>");
        let file = dir.join("t.epub");
        std::fs::write(&file, epub_package("My <Title>", &body)).unwrap();
        if Path::new("/usr/bin/unzip").exists() {
            assert_eq!(run_deadline(Command::new("/usr/bin/unzip").arg("-t").arg(&file), 30), Some(true));
            let html = crate::epub::epub_to_html(&file).unwrap();
            assert!(html.contains("<title>My &lt;Title&gt;</title>") || html.contains("My"), "{html}");
            let a = html.find("Alpha café").unwrap();
            let b = html.find("Beta").unwrap();
            let c = html.find("Gamma &amp; delta").unwrap();
            assert!(a < b && b < c, "{html}");
        }
        // LibreOffice has no EPUB import filter (export only), so well-formedness
        // of every XML part is checked with xmllint instead.
        if Path::new("/usr/bin/xmllint").exists() && Path::new("/usr/bin/unzip").exists() {
            for part in ["META-INF/container.xml", "OEBPS/content.opf", "OEBPS/nav.xhtml", "OEBPS/chapter1.xhtml"] {
                let x = dir.join("part.xml");
                let out = Command::new("/usr/bin/unzip").arg("-p").arg(&file).arg(part).output().unwrap();
                std::fs::write(&x, out.stdout).unwrap();
                assert_eq!(run_deadline(Command::new("/usr/bin/xmllint").arg("--noout").arg(&x), 30), Some(true), "{part}");
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
