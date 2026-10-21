//! PDF page organization backed by macOS PDFKit, driven through
//! `/usr/bin/osascript` (JXA). Standard library only.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const OSASCRIPT: &str = "/usr/bin/osascript";
const DEADLINE: Duration = Duration::from_secs(60);

/// One output page: which source page to take and how far to rotate it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageSel {
    /// 0-based index into the source document.
    pub index: usize,
    /// Rotation in 90 degree steps; any integer, normalized modulo 4.
    pub quarter_turns: i32,
}

const COUNT_JS: &str = r#"
ObjC.import('Quartz');
function run(argv) {
  var doc = $.PDFDocument.alloc.initWithURL($.NSURL.fileURLWithPath(argv[0]));
  if (doc.isNil()) { throw new Error('cannot open PDF'); }
  if (doc.isLocked) { throw new Error('PDF is locked'); }
  return String(doc.pageCount);
}
"#;

const ORGANIZE_JS: &str = r#"
ObjC.import('Quartz');
function run(argv) {
  var src = $.PDFDocument.alloc.initWithURL($.NSURL.fileURLWithPath(argv[0]));
  if (src.isNil()) { throw new Error('cannot open PDF'); }
  if (src.isLocked) { throw new Error('PDF is locked'); }
  var sel = JSON.parse(argv[2]);
  var out = $.PDFDocument.alloc.init;
  for (var i = 0; i < sel.length; i++) {
    var page = src.pageAtIndex(sel[i][0]).copy;
    var turns = ((sel[i][1] % 4) + 4) % 4;
    page.rotation = (((page.rotation + turns * 90) % 360) + 360) % 360;
    out.insertPageAtIndex(page, i);
  }
  if (!out.writeToFile(argv[1])) { throw new Error('PDFKit failed to write output'); }
  return 'ok';
}
"#;

/// Runs a JXA script with a hard deadline; stderr and stdout drained concurrently.
fn run_jxa(script: &str, args: &[&str]) -> Result<String, String> {
    let mut child = Command::new(OSASCRIPT)
        .args(["-l", "JavaScript", "-e", script])
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot start osascript: {e}"))?;
    fn drain<R: Read + Send + 'static>(r: Option<R>) -> thread::JoinHandle<String> {
        thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut r) = r {
                let _ = r.read_to_end(&mut buf);
            }
            String::from_utf8_lossy(&buf).into_owned()
        })
    }
    let out = drain(child.stdout.take());
    let err = drain(child.stderr.take());
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if start.elapsed() >= DEADLINE => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = out.join();
                let _ = err.join();
                return Err("osascript timed out".into());
            }
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(e) => return Err(format!("osascript wait failed: {e}")),
        }
    };
    let stdout = out.join().unwrap_or_default();
    let stderr = err.join().unwrap_or_default();
    if status.success() {
        Ok(stdout.trim().to_string())
    } else {
        Err(format!("osascript failed: {}", stderr.trim()))
    }
}

fn path_str(p: &Path) -> Result<&str, String> {
    p.to_str().ok_or_else(|| "path is not valid UTF-8".to_string())
}

/// Number of pages in the PDF at `src`.
pub fn page_count(src: &Path) -> Result<usize, String> {
    let s = path_str(src)?;
    run_jxa(COUNT_JS, &[s])?.parse::<usize>().map_err(|e| format!("unexpected page count: {e}"))
}

static SEQ: AtomicU64 = AtomicU64::new(0);

const THUMB_JS: &str = r#"
ObjC.import('Quartz');
ObjC.import('AppKit');
function run(argv) {
  var doc = $.PDFDocument.alloc.initWithURL($.NSURL.fileURLWithPath(argv[0]));
  if (doc.isNil()) { throw new Error('cannot open PDF'); }
  if (doc.isLocked) { throw new Error('PDF is locked'); }
  var i = parseInt(argv[1]), max = parseInt(argv[2]);
  if (!(i >= 0 && i < doc.pageCount)) { throw new Error('page out of range'); }
  var page = doc.pageAtIndex(i);
  var box = page.boundsForBox($.kPDFDisplayBoxCropBox);
  var k = max / Math.max(box.size.width, box.size.height);
  var img = page.thumbnailOfSizeForBox($.NSMakeSize(Math.max(1, Math.round(box.size.width * k)), Math.max(1, Math.round(box.size.height * k))), $.kPDFDisplayBoxCropBox);
  var rep = $.NSBitmapImageRep.imageRepWithData(img.TIFFRepresentation);
  var png = rep.representationUsingTypeProperties($.NSBitmapImageFileTypePNG, $());
  if (!png.writeToFileAtomically(argv[3], true)) { throw new Error('cannot write thumbnail'); }
  return 'ok';
}
"#;

/// PNG bytes of page `index` (0-based) scaled so its longest side is `max_px`.
pub fn thumbnail(src: &Path, index: usize, max_px: u32) -> Result<Vec<u8>, String> {
    let s = path_str(src)?;
    let dir = std::env::temp_dir().join(format!("ferrocast-thumb-{}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed)));
    std::fs::create_dir(&dir).map_err(|e| e.to_string())?;
    let out = dir.join("t.png");
    let res = path_str(&out).and_then(|o| run_jxa(THUMB_JS, &[s, &index.to_string(), &max_px.clamp(16, 2048).to_string(), o]));
    let bytes = res.and_then(|_| std::fs::read(&out).map_err(|e| e.to_string()));
    let _ = std::fs::remove_dir_all(&dir);
    bytes
}

/// Builds `dest` from the selected pages of `src`. Never overwrites `dest`.
pub fn organize(src: &Path, pages: &[PageSel], dest: &Path) -> Result<(), String> {
    if pages.is_empty() {
        return Err("no pages selected".into());
    }
    let src_s = path_str(src)?;
    path_str(dest)?;
    if dest.symlink_metadata().is_ok() {
        return Err(format!("destination already exists: {}", dest.display()));
    }
    if let (Ok(a), Ok(b)) = (src.canonicalize(), dest.canonicalize()) {
        if a == b {
            return Err("destination equals source".into());
        }
    }
    if src == dest {
        return Err("destination equals source".into());
    }
    let count = page_count(src)?;
    if let Some(p) = pages.iter().find(|p| p.index >= count) {
        return Err(format!("page index {} out of range (0..{count})", p.index));
    }
    let json =
        format!("[{}]", pages.iter().map(|p| format!("[{},{}]", p.index, p.quarter_turns.rem_euclid(4))).collect::<Vec<_>>().join(","));

    let scratch_dir: PathBuf =
        std::env::temp_dir().join(format!("pdf-organize-{}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed)));
    std::fs::create_dir(&scratch_dir).map_err(|e| format!("cannot create scratch dir: {e}"))?;
    let result = publish(src_s, &json, &scratch_dir, dest);
    let _ = std::fs::remove_dir_all(&scratch_dir);
    result
}

fn publish(src: &str, json: &str, scratch_dir: &Path, dest: &Path) -> Result<(), String> {
    let scratch = scratch_dir.join("out.pdf");
    run_jxa(ORGANIZE_JS, &[src, path_str(&scratch)?, json])?;
    let bytes = std::fs::read(&scratch).map_err(|e| format!("cannot read scratch output: {e}"))?;
    if bytes.is_empty() {
        return Err("PDFKit produced an empty file".into());
    }
    let mut f =
        std::fs::OpenOptions::new().write(true).create_new(true).open(dest).map_err(|e| format!("cannot create destination: {e}"))?;
    if let Err(e) = f.write_all(&bytes).and_then(|_| f.sync_all()) {
        drop(f);
        let _ = std::fs::remove_file(dest);
        return Err(format!("cannot write destination: {e}"));
    }
    Ok(())
}

const META_JS: &str = r#"
ObjC.import('Quartz');
function run(argv) {
  var doc = $.PDFDocument.alloc.initWithURL($.NSURL.fileURLWithPath(argv[0]));
  if (doc.isNil()) { throw new Error('cannot open PDF'); }
  if (doc.isLocked) { throw new Error('PDF is locked'); }
  var attrs = doc.documentAttributes, out = {};
  var keys = ['Title', 'Author', 'Subject', 'Keywords', 'Creator', 'Producer'];
  for (var i = 0; i < keys.length; i++) {
    if (attrs.isNil()) { break; }
    var v = attrs.objectForKey(keys[i]);
    if (v.isNil()) { continue; }
    var u = ObjC.deepUnwrap(v);
    out[keys[i]] = Array.isArray(u) ? u.join(', ') : String(u);
  }
  return Object.keys(out).map(function (k) { return k + '=' + encodeURIComponent(out[k]); }).join('\n');
}
"#;

const SET_META_JS: &str = r#"
ObjC.import('Quartz');
function run(argv) {
  var doc = $.PDFDocument.alloc.initWithURL($.NSURL.fileURLWithPath(argv[0]));
  if (doc.isNil()) { throw new Error('cannot open PDF'); }
  if (doc.isLocked) { throw new Error('PDF is locked'); }
  // each edit arrives as "Key" (remove) or "Key=<percent-encoded value>"
  var edits = argv.slice(3).map(function (e) {
    var at = e.indexOf('=');
    return at < 0 ? [e, null] : [e.slice(0, at), decodeURIComponent(e.slice(at + 1))];
  });
  var d = $.NSMutableDictionary.alloc.init;
  if (argv[2] !== 'all' && !doc.documentAttributes.isNil()) {
    d = $.NSMutableDictionary.dictionaryWithDictionary(doc.documentAttributes);
  }
  for (var i = 0; i < edits.length; i++) {
    var k = edits[i][0], v = edits[i][1];
    if (v === null || v === '') { d.removeObjectForKey(k); }
    else if (k === 'Keywords') {
      d.setObjectForKey($(v.split(',').map(function (x) { return x.trim(); }).filter(function (x) { return x.length > 0; })), k);
    } else { d.setObjectForKey($(v), k); }
  }
  doc.documentAttributes = d;
  if (!doc.writeToFile(argv[1])) { throw new Error('PDFKit failed to write output'); }
  return 'ok';
}
"#;

/// Metadata attributes PDFKit exposes and this tool edits.
pub const META_KEYS: [&str; 6] = ["Title", "Author", "Subject", "Keywords", "Creator", "Producer"];

/// Title, Author, Subject, Keywords, Creator and Producer of the PDF at `src`, those that are set.
pub fn metadata(src: &Path) -> Result<Vec<(String, String)>, String> {
    let json = run_jxa(META_JS, &[path_str(src)?])?;
    let mut out = vec![];
    for line in json.lines() {
        let (k, v) = line.split_once('=').ok_or("unexpected metadata output")?;
        out.push((k.to_string(), pct_decode(v)?));
    }
    Ok(out)
}

/// Escapes everything but ASCII letters and digits, so a value is one safe argument.
fn pct_encode(s: &str) -> String {
    s.bytes().map(|b| if b.is_ascii_alphanumeric() { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

fn pct_decode(s: &str) -> Result<String, String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = s.get(i + 1..i + 3).ok_or("bad escape in metadata")?;
            out.push(u8::from_str_radix(hex, 16).map_err(|_| "bad escape in metadata")?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| "metadata is not UTF-8".to_string())
}

/// Writes a copy of `src` next to it as "<name> edited.pdf" ("... edited 2.pdf" on a collision),
/// with the given attribute edits (`None` removes a key; names are those of [`META_KEYS`]) or,
/// with `remove_all`, with every attribute dropped. Never overwrites; the original is untouched.
pub fn set_metadata(src: &Path, edits: &[(String, Option<String>)], remove_all: bool) -> Result<PathBuf, String> {
    let src_s = path_str(src)?;
    if let Some((k, _)) = edits.iter().find(|(k, _)| !META_KEYS.contains(&k.as_str())) {
        return Err(format!("PDF metadata has no field named {k}"));
    }
    let args: Vec<String> = edits.iter().map(|(k, v)| v.as_ref().map_or(k.clone(), |v| format!("{k}={}", pct_encode(v)))).collect();
    let scratch_dir: PathBuf =
        std::env::temp_dir().join(format!("pdf-meta-{}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed)));
    std::fs::create_dir(&scratch_dir).map_err(|e| format!("cannot create scratch dir: {e}"))?;
    let result = (|| {
        let scratch = scratch_dir.join("out.pdf");
        let mut argv = vec![src_s, path_str(&scratch)?, if remove_all { "all" } else { "keep" }];
        argv.extend(args.iter().map(String::as_str));
        run_jxa(SET_META_JS, &argv)?;
        let bytes = std::fs::read(&scratch).map_err(|e| format!("cannot read scratch output: {e}"))?;
        if bytes.is_empty() {
            return Err("PDFKit produced an empty file".to_string());
        }
        let dir = src.parent().map(Path::to_path_buf).unwrap_or_default();
        let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("document");
        for n in 1u32.. {
            let name = if n == 1 { format!("{stem} edited.pdf") } else { format!("{stem} edited {n}.pdf") };
            let dest = dir.join(name);
            match std::fs::OpenOptions::new().write(true).create_new(true).open(&dest) {
                Ok(mut f) => {
                    if let Err(e) = f.write_all(&bytes).and_then(|_| f.sync_all()) {
                        drop(f);
                        let _ = std::fs::remove_file(&dest);
                        return Err(format!("cannot write destination: {e}"));
                    }
                    return Ok(dest);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(format!("cannot create destination: {e}")),
            }
        }
        unreachable!()
    })();
    let _ = std::fs::remove_dir_all(&scratch_dir);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAKE_JS: &str = r#"
ObjC.import('AppKit'); ObjC.import('Quartz');
function run(argv) {
  var doc = $.PDFDocument.alloc.init;
  var names = ['alpha', 'bravo', 'charlie'];
  for (var i = 0; i < names.length; i++) {
    var tv = $.NSTextView.alloc.initWithFrame($.NSMakeRect(0, 0, 400, 400));
    tv.string = 'page-' + names[i];
    var data = tv.dataWithPDFInsideRect(tv.bounds);
    var one = $.PDFDocument.alloc.initWithData(data);
    doc.insertPageAtIndex(one.pageAtIndex(0).copy, i);
  }
  if (!doc.writeToFile(argv[0])) { throw new Error('write failed'); }
  return 'ok';
}
"#;

    const INSPECT_JS: &str = r#"
ObjC.import('Quartz');
function run(argv) {
  var doc = $.PDFDocument.alloc.initWithURL($.NSURL.fileURLWithPath(argv[0]));
  var r = [];
  for (var i = 0; i < doc.pageCount; i++) {
    var p = doc.pageAtIndex(i);
    r.push(ObjC.unwrap(p.string).trim() + '@' + p.rotation);
  }
  return r.join(',');
}
"#;

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("pdf-test-{}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn fixture(dir: &Path) -> PathBuf {
        let p = dir.join("src.pdf");
        run_jxa(MAKE_JS, &[p.to_str().unwrap()]).unwrap();
        p
    }

    fn inspect(p: &Path) -> String {
        run_jxa(INSPECT_JS, &[p.to_str().unwrap()]).unwrap()
    }

    fn sel(index: usize, quarter_turns: i32) -> PageSel {
        PageSel { index, quarter_turns }
    }

    #[test]
    fn thumbnails_are_pngs_of_the_requested_size() {
        let d = tmp();
        let s = fixture(&d);
        let t = thumbnail(&s, 1, 120).unwrap();
        assert_eq!(&t[..4], b"\x89PNG");
        // the fixture page is square, so the longest side is the requested size
        let w = u32::from_be_bytes([t[16], t[17], t[18], t[19]]);
        let h = u32::from_be_bytes([t[20], t[21], t[22], t[23]]);
        assert!(w.max(h) >= 118 && w.max(h) <= 122, "{w}x{h}");
        assert!(thumbnail(&s, 3, 120).is_err(), "page out of range");
        assert!(thumbnail(&d.join("missing.pdf"), 0, 120).is_err());
        assert!(
            std::fs::read_dir(std::env::temp_dir())
                .unwrap()
                .flatten()
                .all(|e| !e.file_name().to_string_lossy().starts_with(&format!("ferrocast-thumb-{}-", std::process::id()))),
            "scratch removed"
        );
    }

    #[test]
    fn counts_pages() {
        let d = tmp();
        let s = fixture(&d);
        assert_eq!(page_count(&s).unwrap(), 3);
        assert_eq!(inspect(&s), "page-alpha@0,page-bravo@0,page-charlie@0");
        assert!(page_count(&d.join("missing.pdf")).is_err());
    }

    #[test]
    fn reorder_duplicate_rotate_remove() {
        let d = tmp();
        let s = fixture(&d);
        let out = d.join("out.pdf");
        // reorder + duplicate + rotate; bravo (index 1) removed
        organize(&s, &[sel(2, 0), sel(0, 1), sel(0, -1), sel(2, 5)], &out).unwrap();
        assert_eq!(page_count(&out).unwrap(), 4);
        assert_eq!(inspect(&out), "page-charlie@0,page-alpha@90,page-alpha@270,page-charlie@90");
        // source untouched
        assert_eq!(inspect(&s), "page-alpha@0,page-bravo@0,page-charlie@0");
    }

    #[test]
    fn rejects_invalid_without_touching_files() {
        let d = tmp();
        let s = fixture(&d);
        let before = std::fs::read(&s).unwrap();
        let out = d.join("out.pdf");
        assert!(organize(&s, &[], &out).is_err());
        assert!(organize(&s, &[sel(3, 0)], &out).is_err());
        assert!(!out.exists());
        assert!(organize(&s, &[sel(0, 0)], &s).is_err());
        assert!(organize(&d.join("nope.pdf"), &[sel(0, 0)], &out).is_err());
        assert!(!out.exists());
        std::fs::write(&out, b"keep").unwrap();
        assert!(organize(&s, &[sel(0, 0)], &out).is_err());
        assert_eq!(std::fs::read(&out).unwrap(), b"keep");
        assert_eq!(std::fs::read(&s).unwrap(), before);
        // unreadable (not a PDF) source
        let bad = d.join("bad.pdf");
        std::fs::write(&bad, b"not a pdf").unwrap();
        assert!(organize(&bad, &[sel(0, 0)], &d.join("o2.pdf")).is_err());
        assert!(!d.join("o2.pdf").exists());
    }

    #[test]
    fn percent_escaping_round_trips_unicode_and_separators() {
        for v in ["", "plain", "a=b\nc\td", "Título ñ 日本 🎵", "100%"] {
            assert_eq!(pct_decode(&pct_encode(v)).unwrap(), v);
        }
        assert!(pct_encode("a=b").chars().all(|c| c.is_ascii_alphanumeric() || c == '%'));
        assert!(pct_decode("%zz").is_err());
        assert!(pct_decode("%4").is_err());
    }

    fn s(k: &str, v: Option<&str>) -> (String, Option<String>) {
        (k.to_string(), v.map(String::from))
    }

    #[test]
    #[ignore = "requires PDFKit through osascript; creates isolated real PDFs"]
    fn pdf_metadata_round_trip_real_file() {
        let d = tmp();
        let src = fixture(&d);
        let before = std::fs::read(&src).unwrap();
        let pages = page_count(&src).unwrap();

        let a =
            set_metadata(&src, &[s("Title", Some("Título ñ = 日本")), s("Author", Some("Ann")), s("Keywords", Some("uno, dos"))], false)
                .unwrap();
        assert_eq!(a, d.join("src edited.pdf"));
        let m = metadata(&a).unwrap();
        let get = |k: &str| m.iter().find(|(x, _)| x == k).map(|(_, v)| v.clone());
        assert_eq!(get("Title").as_deref(), Some("Título ñ = 日本"));
        assert_eq!(get("Author").as_deref(), Some("Ann"));
        assert_eq!(get("Keywords").as_deref(), Some("uno, dos"));
        assert_eq!(page_count(&a).unwrap(), pages);
        assert_eq!(inspect(&a), inspect(&src), "page text unchanged");

        // second run: ' 2', and removing one key keeps the others
        let b = set_metadata(&a, &[s("Author", None), s("Subject", Some("S"))], false).unwrap();
        assert_eq!(b, d.join("src edited edited.pdf"));
        let mb = metadata(&b).unwrap();
        assert!(mb.iter().all(|(k, _)| k != "Author"));
        assert!(mb.iter().any(|(k, v)| k == "Title" && v == "Título ñ = 日本"));
        assert!(mb.iter().any(|(k, v)| k == "Subject" && v == "S"));
        let c = set_metadata(&src, &[s("Title", Some("again"))], false).unwrap();
        assert_eq!(c, d.join("src edited 2.pdf"));

        // remove_all clears what we set
        let z = set_metadata(&a, &[], true).unwrap();
        let mz = metadata(&z).unwrap();
        assert!(mz.iter().all(|(k, _)| !["Title", "Author", "Subject", "Keywords"].contains(&k.as_str())), "{mz:?}");
        assert_eq!(page_count(&z).unwrap(), pages);

        assert_eq!(std::fs::read(&src).unwrap(), before, "original untouched");
    }

    #[test]
    #[ignore = "requires PDFKit through osascript"]
    fn pdf_metadata_rejects_bad_input_and_leaves_nothing() {
        let d = tmp();
        let src = fixture(&d);
        assert!(set_metadata(&src, &[s("Bogus", Some("x"))], false).unwrap_err().contains("Bogus"));
        let bad = d.join("bad.pdf");
        std::fs::write(&bad, b"not a pdf").unwrap();
        assert!(set_metadata(&bad, &[s("Title", Some("x"))], false).is_err());
        assert!(metadata(&bad).is_err());
        assert!(set_metadata(&d.join("missing.pdf"), &[], true).is_err());
        let names: Vec<_> = std::fs::read_dir(&d).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert!(names.iter().all(|n| !n.contains("edited")), "{names:?}");
    }
}
