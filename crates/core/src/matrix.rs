//! Tangerine's conversion matrix, replicated from its site (see odd/tasks/file-tools.md S10):
//! 46 input formats and 275 (input → output) options.

const IMAGE_IN: [&str; 8] = ["jpg", "png", "webp", "heic", "tiff", "svg", "avif", "bmp"];
const IMAGE_OUT: [&str; 8] = ["jpg", "png", "webp", "heic", "tiff", "avif", "bmp", "pdf"];
const AUDIO: [&str; 8] = ["mp3", "m4a", "wav", "flac", "ogg", "opus", "aiff", "wma"];
const VIDEO: [&str; 6] = ["mp4", "mov", "mkv", "webm", "avi", "wmv"];
const SUBS: [&str; 2] = ["srt", "vtt"];
const DOCS: [&str; 6] = ["docx", "pages", "md", "rtf", "rtfd", "textClipping"];
const SHEETS: [&str; 5] = ["xlsx", "xls", "ods", "csv", "tsv"];
const DOC_OUT: [&str; 9] = ["docx", "pages", "pdf", "txt", "md", "html", "epub", "png", "jpg"];
/// Formats Tangerine reads but never writes.
const INPUT_ONLY: [&str; 6] = ["rar", "ppt", "xls", "rtf", "rtfd", "textClipping"];

/// Every input format, in the order the site lists them.
pub fn inputs() -> Vec<&'static str> {
    let mut v: Vec<&str> = vec![];
    for g in [
        &IMAGE_IN[..],
        &AUDIO,
        &VIDEO,
        &["gif"],
        &DOCS,
        &SHEETS,
        &SUBS,
        &["pdf", "ppt", "pptx", "epub", "txt", "html", "zip", "tar", "gz", "rar"],
    ] {
        for e in g {
            if !v.contains(e) {
                v.push(e);
            }
        }
    }
    v
}

/// Output formats offered for an input extension (lowercase-insensitive except `textClipping`).
pub fn outputs_for(ext: &str) -> Vec<&'static str> {
    let e = if ext.eq_ignore_ascii_case("textclipping") { "textClipping" } else { ext };
    let e = e.to_lowercase().replace("jpeg", "jpg").replace("tif", "tiff").replace("tifff", "tiff");
    let e = if e == "textclipping" { "textClipping".to_string() } else { e };
    let mut t: Vec<&str> = if IMAGE_IN.contains(&e.as_str()) {
        IMAGE_OUT.to_vec()
    } else if AUDIO.contains(&e.as_str()) {
        AUDIO.to_vec()
    } else if VIDEO.contains(&e.as_str()) {
        VIDEO.iter().copied().chain(["gif", "mp3"]).collect()
    } else if e == "gif" {
        VIDEO.to_vec()
    } else if e == "pdf" {
        vec!["docx", "pages", "pptx", "xlsx", "jpg", "png", "epub", "txt", "md"]
    } else if e == "ppt" || e == "pptx" {
        vec!["pdf", "jpg", "png"]
    } else if SHEETS.contains(&e.as_str()) {
        SHEETS.iter().copied().chain(["pdf"]).collect()
    } else if DOCS.contains(&e.as_str()) {
        DOC_OUT.to_vec()
    } else if e == "epub" {
        vec!["pdf", "docx", "pages", "txt", "md", "html"]
    } else if e == "txt" {
        vec!["docx", "pages", "pdf", "md", "html", "epub", "jpg", "png"]
    } else if SUBS.contains(&e.as_str()) {
        SUBS.iter().copied().chain(["txt"]).collect()
    } else if ["zip", "tar", "gz", "rar"].contains(&e.as_str()) {
        vec!["zip", "tar", "gz"]
    } else {
        vec![]
    };
    t.retain(|o| *o != e && !INPUT_ONLY.contains(o));
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_tangerines_275_options_over_46_inputs() {
        let ins = inputs();
        assert_eq!(ins.len(), 46);
        let total: usize = ins.iter().map(|i| outputs_for(i).len()).sum();
        assert_eq!(total, 275);
    }

    #[test]
    fn spot_checks_from_the_site() {
        assert_eq!(outputs_for("mp4"), vec!["mov", "mkv", "webm", "avi", "wmv", "gif", "mp3"]);
        assert_eq!(outputs_for("gif"), vec!["mp4", "mov", "mkv", "webm", "avi", "wmv"]);
        assert_eq!(outputs_for("pdf"), vec!["docx", "pages", "pptx", "xlsx", "jpg", "png", "epub", "txt", "md"]);
        assert_eq!(outputs_for("xls"), vec!["xlsx", "ods", "csv", "tsv", "pdf"]);
        assert_eq!(outputs_for("gz"), vec!["zip", "tar"]);
        assert_eq!(outputs_for("rar"), vec!["zip", "tar", "gz"]);
        assert_eq!(outputs_for("srt"), vec!["vtt", "txt"]);
        assert!(outputs_for("html").is_empty());
        assert!(outputs_for("exe").is_empty());
        assert_eq!(outputs_for("JPEG"), outputs_for("jpg"), "case and jpeg alias");
        assert_eq!(outputs_for("textclipping").len(), 9);
        // outputs never include the input itself or input-only formats
        for i in inputs() {
            let o = outputs_for(i);
            assert!(!o.contains(&i) && !o.iter().any(|x| INPUT_ONLY.contains(x)), "{i}: {o:?}");
        }
    }
}
