//! Minimal writer for Apple Pages (`.pages`) documents.
//!
//! Pages.app scripting is unreliable, so this builds the legacy iWork '09
//! package directly: a ZIP holding `index.xml` (`sl:document` with a body
//! `sf:text-storage`). Plain text paragraphs only. No external dependencies.

use std::io;
use std::path::Path;

/// CRC-32 (IEEE, reflected, polynomial 0xEDB88320).
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// Build a ZIP archive of stored (uncompressed) entries.
pub fn zip_stored(entries: &[(&str, &[u8])]) -> Vec<u8> {
    // Fixed DOS date 2024-01-01 00:00:00.
    const TIME: u16 = 0;
    const DATE: u16 = ((2024 - 1980) << 9) | (1 << 5) | 1;
    let mut out: Vec<u8> = Vec::new();
    let mut central: Vec<u8> = Vec::new();
    for (name, data) in entries {
        let offset = out.len() as u32;
        let crc = crc32(data);
        let size = data.len() as u32;
        let name_b = name.as_bytes();
        // Local file header.
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0x0800u16.to_le_bytes()); // flags: UTF-8 names
        out.extend_from_slice(&0u16.to_le_bytes()); // method: stored
        out.extend_from_slice(&TIME.to_le_bytes());
        out.extend_from_slice(&DATE.to_le_bytes());
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&(name_b.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // extra length
        out.extend_from_slice(name_b);
        out.extend_from_slice(data);
        // Central directory header.
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes()); // version made by
        central.extend_from_slice(&20u16.to_le_bytes()); // version needed
        central.extend_from_slice(&0x0800u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&TIME.to_le_bytes());
        central.extend_from_slice(&DATE.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&(name_b.len() as u16).to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes()); // extra
        central.extend_from_slice(&0u16.to_le_bytes()); // comment
        central.extend_from_slice(&0u16.to_le_bytes()); // disk
        central.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
        central.extend_from_slice(&0u32.to_le_bytes()); // external attrs
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name_b);
    }
    let cd_offset = out.len() as u32;
    let cd_size = central.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

/// Escape text for XML content/attributes; characters illegal in XML 1.0 are dropped.
pub fn xml_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\t' | '\n' | '\r' => out.push(' '),
            c if (c as u32) < 0x20 || c == '\u{7f}' || c == '\u{fffe}' || c == '\u{ffff}' => {}
            c => out.push(c),
        }
    }
    out
}

fn index_xml(paragraphs: &[String]) -> String {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <sl:document xmlns:sl=\"http://developer.apple.com/namespaces/sl\" \
         xmlns:sf=\"http://developer.apple.com/namespaces/sf\" \
         xmlns:sfa=\"http://developer.apple.com/namespaces/sfa\" sl:version=\"2.0\">\n\
         <sf:text-storage sf:kind=\"body\"><sf:text-body><sf:page-start sf:page-index=\"0\"/>",
    );
    for p in paragraphs {
        xml.push_str("<sf:p>");
        xml.push_str(&xml_escape(p));
        xml.push_str("</sf:p>");
    }
    xml.push_str("</sf:text-body></sf:text-storage>\n</sl:document>\n");
    xml
}

/// Bytes of a `.pages` file whose body is the given paragraphs
/// (an empty string is an empty paragraph).
pub fn pages_package(paragraphs: &[String]) -> Vec<u8> {
    let xml = index_xml(paragraphs);
    zip_stored(&[("index.xml", xml.as_bytes())])
}

/// Write a `.pages` file at `path`.
pub fn write_pages(path: &Path, paragraphs: &[String]) -> io::Result<()> {
    std::fs::write(path, pages_package(paragraphs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    const SOFFICE: &str = "/opt/homebrew/bin/soffice";

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ferrocast-pages-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Run a command with a hard deadline; None when it timed out (killed).
    fn run_deadline(cmd: &mut Command, secs: u64) -> Option<std::process::Output> {
        let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().ok()?;
        let deadline = Instant::now() + Duration::from_secs(secs);
        loop {
            if child.try_wait().ok()?.is_some() {
                return child.wait_with_output().ok();
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    #[test]
    fn crc32_check_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn zip_round_trips_through_unzip() {
        let dir = scratch("zip");
        let file = dir.join("a.zip");
        let payload = "héllo 日本\nline2".as_bytes();
        std::fs::write(&file, zip_stored(&[("a.txt", payload), ("d/b.txt", b"two")])).unwrap();
        let t = run_deadline(Command::new("/usr/bin/unzip").arg("-t").arg(&file), 30).unwrap();
        assert!(t.status.success(), "{}", String::from_utf8_lossy(&t.stdout));
        let p = run_deadline(Command::new("/usr/bin/unzip").arg("-p").arg(&file).arg("a.txt"), 30).unwrap();
        assert_eq!(p.stdout, payload);
        let p = run_deadline(Command::new("/usr/bin/unzip").arg("-p").arg(&file).arg("d/b.txt"), 30).unwrap();
        assert_eq!(p.stdout, b"two");
    }

    #[test]
    fn xml_escaping() {
        assert_eq!(xml_escape("a&b<c>d\"e'f"), "a&amp;b&lt;c&gt;d&quot;e&apos;f");
        assert_eq!(xml_escape("a\u{0}b\u{1b}c\u{8}d"), "abcd");
        assert_eq!(xml_escape("é日本"), "é日本");
    }

    #[test]
    fn index_has_one_p_per_paragraph() {
        let xml = index_xml(&["a".into(), String::new(), "b&c".into()]);
        assert!(xml.contains("<sf:p>a</sf:p><sf:p></sf:p><sf:p>b&amp;c</sf:p>"));
    }

    #[test]
    fn libreoffice_reads_package() {
        if !Path::new(SOFFICE).exists() {
            eprintln!("soffice missing; skipping");
            return;
        }
        let dir = scratch("lo");
        let out = dir.join("out");
        let file = dir.join("doc.pages");
        let paras: Vec<String> = ["First é ñ", "", "日本語 & <tags> \"q\"", "Last line"].iter().map(|s| s.to_string()).collect();
        write_pages(&file, &paras).unwrap();

        let t = run_deadline(Command::new("/usr/bin/unzip").arg("-t").arg(&file), 30).unwrap();
        assert!(t.status.success());

        let profile = format!("-env:UserInstallation=file://{}", dir.join("profile").display());
        let r = run_deadline(
            Command::new(SOFFICE)
                .arg(profile)
                .args(["--headless", "--convert-to", "txt:Text (encoded):UTF8", "--outdir"])
                .arg(&out)
                .arg(&file),
            90,
        )
        .expect("soffice timed out");
        assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
        let text = std::fs::read_to_string(out.join("doc.txt")).unwrap();
        let text = text.trim_start_matches('\u{feff}');
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines, ["First é ñ", "", "日本語 & <tags> \"q\"", "Last line"]);

        // Informational only.
        if let Some(o) = run_deadline(Command::new("/usr/bin/mdls").args(["-name", "kMDItemContentType"]).arg(&file), 30) {
            eprintln!("mdls: {}", String::from_utf8_lossy(&o.stdout).trim());
        }
        let ql = dir.join("ql");
        std::fs::create_dir_all(&ql).unwrap();
        if run_deadline(Command::new("/usr/bin/qlmanage").args(["-t", "-s", "256", "-o"]).arg(&ql).arg(&file), 30).is_some() {
            let n = std::fs::read_dir(&ql).map(|d| d.count()).unwrap_or(0);
            eprintln!("qlmanage thumbnails: {n}");
        }
    }
}
