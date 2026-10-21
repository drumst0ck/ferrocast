//! Minimal pure .xlsx writer (inline strings) and text-to-table helper.

use crate::pages::{xml_escape, zip_stored};

/// Spreadsheet column name for a zero-based index: 0 → "A", 26 → "AA".
pub fn col_name(i: usize) -> String {
    let mut n = i + 1;
    let mut out = Vec::new();
    while n > 0 {
        let r = (n - 1) % 26;
        out.push(b'A' + r as u8);
        n = (n - 1) / 26;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

/// Plain decimal/integer with optional leading `-`, no leading zeros
/// (except "0" and "0.x").
fn is_number(s: &str) -> bool {
    let body = s.strip_prefix('-').unwrap_or(s);
    let (int, frac) = match body.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (body, None),
    };
    if int.is_empty() || !int.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    if int.len() > 1 && int.starts_with('0') {
        return false;
    }
    if let Some(f) = frac {
        if f.is_empty() || !f.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
    }
    // Excel keeps ~15 significant digits; longer ones stay text.
    int.len() + frac.map_or(0, |f| f.len()) <= 15
}

fn sheet_xml(rows: &[Vec<String>]) -> String {
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData>",
    );
    for (r, row) in rows.iter().enumerate() {
        s.push_str(&format!("<row r=\"{}\">", r + 1));
        for (c, cell) in row.iter().enumerate() {
            let cref = format!("{}{}", col_name(c), r + 1);
            if cell.is_empty() {
                continue;
            }
            if is_number(cell) {
                s.push_str(&format!("<c r=\"{cref}\"><v>{cell}</v></c>"));
            } else {
                s.push_str(&format!("<c r=\"{cref}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>", xml_escape(cell)));
            }
        }
        s.push_str("</row>");
    }
    s.push_str("</sheetData></worksheet>");
    s
}

/// Build a minimal valid .xlsx with one sheet "Sheet1".
pub fn xlsx_package(rows: &[Vec<String>]) -> Vec<u8> {
    let content_types = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/><Override PartName=\"/xl/worksheets/sheet1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/></Types>";
    let rels = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>";
    let workbook = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><sheets><sheet name=\"Sheet1\" sheetId=\"1\" r:id=\"rId1\"/></sheets></workbook>";
    let wb_rels = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet1.xml\"/></Relationships>";
    let sheet = sheet_xml(rows);
    zip_stored(&[
        ("[Content_Types].xml", content_types.as_bytes()),
        ("_rels/.rels", rels.as_bytes()),
        ("xl/workbook.xml", workbook.as_bytes()),
        ("xl/_rels/workbook.xml.rels", wb_rels.as_bytes()),
        ("xl/worksheets/sheet1.xml", sheet.as_bytes()),
    ])
}

/// Split extracted text into a table: one row per non-empty line, columns
/// split on tabs or runs of 2+ spaces.
pub fn rows_from_text(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut cells: Vec<String> = Vec::new();
        let mut cur = String::new();
        let mut spaces = 0usize;
        for ch in line.chars() {
            if ch == '\t' {
                cells.push(cur.trim().to_string());
                cur.clear();
                spaces = 0;
            } else if ch == ' ' {
                spaces += 1;
                cur.push(ch);
            } else {
                if spaces >= 2 {
                    cells.push(cur.trim().to_string());
                    cur.clear();
                }
                spaces = 0;
                cur.push(ch);
            }
        }
        cells.push(cur.trim().to_string());
        // Leading separators can produce empty cells; keep them only inside the row.
        while cells.first().is_some_and(|c| c.is_empty()) && cells.len() > 1 {
            cells.remove(0);
        }
        while cells.last().is_some_and(|c| c.is_empty()) && cells.len() > 1 {
            cells.pop();
        }
        rows.push(cells);
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    #[test]
    fn col_names() {
        assert_eq!(col_name(0), "A");
        assert_eq!(col_name(25), "Z");
        assert_eq!(col_name(26), "AA");
        assert_eq!(col_name(27), "AB");
        assert_eq!(col_name(701), "ZZ");
        assert_eq!(col_name(702), "AAA");
    }

    #[test]
    fn numbers_vs_strings_and_escaping() {
        let rows = vec![vec![
            "1.5".to_string(),
            "-3".to_string(),
            "007".to_string(),
            "0.25".to_string(),
            "a<b&\"c\"".to_string(),
            "1.".to_string(),
        ]];
        let zip = xlsx_package(&rows);
        let text = String::from_utf8_lossy(&zip).into_owned();
        assert!(text.contains("<c r=\"A1\"><v>1.5</v></c>"));
        assert!(text.contains("<c r=\"B1\"><v>-3</v></c>"));
        assert!(text.contains("<c r=\"C1\" t=\"inlineStr\"><is><t xml:space=\"preserve\">007</t>"));
        assert!(text.contains("<c r=\"D1\"><v>0.25</v></c>"));
        assert!(text.contains("a&lt;b&amp;&quot;c&quot;"));
        assert!(text.contains("<c r=\"F1\" t=\"inlineStr\">"));
        assert!(!is_number("") && !is_number("-") && !is_number(".5") && !is_number("1e5"));
        assert!(is_number("0") && is_number("-0.5"));
    }

    #[test]
    fn rows_from_text_samples() {
        let t = "Name\tQty  Price\n\n  Apple   3    1.5  \nlone line here\n";
        assert_eq!(rows_from_text(t), vec![vec!["Name", "Qty", "Price"], vec!["Apple", "3", "1.5"], vec!["lone line here"],]);
        assert!(rows_from_text("  \n\n").is_empty());
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
    fn libreoffice_reads_package() {
        let soffice = "/opt/homebrew/bin/soffice";
        if !Path::new(soffice).exists() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("fc-xlsx-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut row = vec![String::new(); 28];
        row[0] = "café".to_string();
        row[1] = "a,b".to_string();
        row[2] = "42.5".to_string();
        row[27] = "far".to_string();
        let file = dir.join("t.xlsx");
        std::fs::write(&file, xlsx_package(&[row, vec!["x".into(), "<&>".into()]])).unwrap();
        if Path::new("/usr/bin/unzip").exists() {
            assert_eq!(run_deadline(Command::new("/usr/bin/unzip").arg("-t").arg(&file), 30), Some(true));
        }
        let out = dir.join("out");
        let ok = run_deadline(
            Command::new(soffice)
                .arg(format!("-env:UserInstallation=file://{}", dir.join("profile").display()))
                .args(["--headless", "--convert-to", "csv", "--outdir"])
                .arg(&out)
                .arg(&file),
            90,
        );
        assert_eq!(ok, Some(true));
        let csv = std::fs::read_to_string(out.join("t.csv")).unwrap();
        assert!(csv.contains("café"), "{csv}");
        assert!(csv.contains("\"a,b\""), "{csv}");
        assert!(csv.contains("42.5") || csv.contains("\"42,5\""), "{csv}"); // locale decimal separator
        assert!(csv.lines().next().unwrap().ends_with(",far"), "{csv}");
        assert!(csv.contains("x,<&>"), "{csv}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
