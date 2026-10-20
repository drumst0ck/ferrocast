//! Multi-step conversion routes: documents through an HTML pivot, archives through a scratch
//! folder, GIF to video, and PDF to Pages. Every external process runs under a hard deadline.

use ferrocast_core::convert::{ext, output_path, Target, Tools};
use ferrocast_core::{clipping, epub, epub_write, markdown, pages, subs, xlsx};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static SEQ: AtomicU64 = AtomicU64::new(0);

/// Run a command with a deadline; stdout is discarded, stderr is returned on failure.
pub(crate) fn run_limited(cmd: &mut Command, secs: u64) -> Result<(), String> {
    run_io(cmd, secs, Stdio::null(), false).map(|_| ())
}

/// Largest listing `run_listing` accepts; a longer one is refused rather than truncated, because a
/// truncated list could hide an unsafe entry.
const LISTING_CAP: u64 = 64 << 20;

/// Run a command under the same deadline and capped stderr, and return its stdout (at most
/// `LISTING_CAP` bytes; more is an error).
pub(crate) fn run_listing(cmd: &mut Command, secs: u64) -> Result<Vec<u8>, String> {
    run_io(cmd, secs, Stdio::piped(), true)
}

fn run_io(cmd: &mut Command, secs: u64, stdout: Stdio, capture: bool) -> Result<Vec<u8>, String> {
    cmd.stdout(stdout).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    // drained concurrently (capped): a chatty tool must not fill the pipe and stall until the deadline
    let stderr = child.stderr.take();
    let reader = std::thread::spawn(move || {
        use std::io::Read;
        let mut buf = Vec::new();
        if let Some(r) = stderr {
            let _ = r.take(1 << 20).read_to_end(&mut buf);
        }
        buf
    });
    let out_reader = capture.then(|| child.stdout.take()).flatten().map(|mut r| {
        std::thread::spawn(move || {
            use std::io::Read;
            let mut buf = Vec::new();
            let _ = (&mut r).take(LISTING_CAP + 1).read_to_end(&mut buf);
            // keep draining so the child is never blocked on a full pipe
            let _ = std::io::copy(&mut r, &mut std::io::sink());
            buf
        })
    });
    let deadline = Instant::now() + Duration::from_secs(secs);
    loop {
        match child.try_wait() {
            Ok(Some(st)) => {
                let err = reader.join().unwrap_or_default();
                let out = out_reader.map(|h| h.join().unwrap_or_default()).unwrap_or_default();
                return if !st.success() {
                    Err(String::from_utf8_lossy(&err).lines().last().unwrap_or("command failed").trim().to_string())
                } else if out.len() as u64 > LISTING_CAP {
                    Err("archive listing is too large".into())
                } else {
                    Ok(out)
                };
            }
            Ok(None) if Instant::now() > deadline => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                if let Some(h) = out_reader {
                    let _ = h.join();
                }
                return Err(format!("timed out after {secs} s"));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(40)),
            Err(e) => return Err(e.to_string()),
        }
    }
}

fn scratch() -> Result<PathBuf, String> {
    let d = std::env::temp_dir().join(format!("ferrocast-route-{}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::SeqCst)));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    Ok(d)
}

fn read_lossy(p: &Path) -> Result<String, String> {
    Ok(String::from_utf8_lossy(&std::fs::read(p).map_err(|e| e.to_string())?).to_string())
}

fn soffice(tools: &Tools, args: &[&str], input: &Path, outdir: &Path) -> Result<(), String> {
    let so = tools.soffice.ok_or("LibreOffice is not installed (brew install --cask libreoffice)")?;
    let profile = format!("-env:UserInstallation=file://{}/profile", outdir.display());
    let mut c = Command::new(so);
    c.arg(&profile).arg("--headless");
    c.args(args).arg("--outdir").arg(outdir).arg(input);
    run_limited(&mut c, 120)
}

/// Where a result goes: next to the original, never over an existing file.
fn place(original: &Path, produced: &Path, ext: &str) -> Result<PathBuf, String> {
    let out = output_path(original, ext, |p| p.exists());
    std::fs::rename(produced, &out).or_else(|_| std::fs::copy(produced, &out).map(|_| ())).map_err(|e| e.to_string())?;
    Ok(out)
}

/// The document as HTML, whatever it was.
fn load_html(f: &Path, e: &str, tools: &Tools, sc: &Path) -> Result<String, String> {
    match e {
        "md" => Ok(markdown::markdown_to_html(&read_lossy(f)?)),
        "txt" => Ok(markdown::markdown_to_html(&markdown::plain_to_markdown(&read_lossy(f)?))),
        "textclipping" => Ok(markdown::markdown_to_html(&markdown::plain_to_markdown(&clipping::read_text_clipping(f)?))),
        "html" | "htm" => read_lossy(f),
        "epub" => epub::epub_to_html(f),
        "pages" => {
            soffice(tools, &["--convert-to", "html"], f, sc)?;
            let stem = f.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            read_lossy(&sc.join(format!("{stem}.html")))
        }
        _ => {
            // rtf, rtfd, docx, doc, odt: textutil knows them
            let out = sc.join("in.html");
            run_limited(
                Command::new("/usr/bin/textutil").args(["-convert", "html", "-encoding", "UTF-8", "-output"]).arg(&out).arg(f),
                60,
            )?;
            read_lossy(&out)
        }
    }
}

fn html_to_text(html: &str, sc: &Path) -> Result<String, String> {
    let (src, out) = (sc.join("p.html"), sc.join("p.txt"));
    std::fs::write(&src, html).map_err(|e| e.to_string())?;
    run_limited(Command::new("/usr/bin/textutil").args(["-convert", "txt", "-encoding", "UTF-8", "-output"]).arg(&out).arg(&src), 60)?;
    read_lossy(&out)
}

/// PDF text layer through PDFKit.
fn pdf_text(f: &Path) -> Result<String, String> {
    let js = "function run(a){ObjC.import('PDFKit');var d=$.PDFDocument.alloc.initWithURL($.NSURL.fileURLWithPath(a[0]));return d?ObjC.unwrap(d.string)||'':''}";
    let o = Command::new("/usr/bin/osascript").args(["-l", "JavaScript", "-e", js]).arg(f).output().map_err(|e| e.to_string())?;
    let t = String::from_utf8_lossy(&o.stdout).trim_end().to_string();
    if t.trim().is_empty() {
        Err("no text layer in this PDF (it may be a scan)".into())
    } else {
        Ok(t)
    }
}

/// Inner HTML of <body>, or the whole fragment.
fn body_of(html: &str) -> String {
    epub::body_inner(html)
}

fn paragraphs(text: &str) -> Vec<String> {
    text.replace("\r\n", "\n").split('\n').map(|l| l.trim_end().to_string()).collect()
}

pub(crate) fn route_convert(f: &Path, target: &Target, tools: &Tools) -> Result<Vec<PathBuf>, String> {
    let e = ext(f);
    let out_ext = target.ext;
    let sc = scratch()?;
    let res = (|| -> Result<Vec<PathBuf>, String> {
        let video = ["mp4", "mov", "mkv", "webm", "avi", "wmv"];
        if e == "gif" && video.contains(&out_ext) {
            return gif_to_video(f, out_ext, tools);
        }
        if ["zip", "tar", "gz", "tgz", "rar"].contains(&e.as_str()) {
            return archive(f, &e, out_ext, tools, &sc);
        }
        if e == "pdf" {
            return pdf_to(f, out_ext, &sc);
        }
        if e == "srt" || e == "vtt" {
            let src = read_lossy(f)?;
            let text = if e == "srt" { subs::srt_to_vtt(&src) } else { subs::vtt_to_srt(&src) };
            let out = output_path(f, out_ext, |p| p.exists());
            std::fs::write(&out, text).map_err(|er| er.to_string())?;
            return Ok(vec![out]);
        }
        document(f, &e, out_ext, tools, &sc)
    })();
    let _ = std::fs::remove_dir_all(&sc);
    res
}

fn gif_to_video(f: &Path, out_ext: &str, tools: &Tools) -> Result<Vec<PathBuf>, String> {
    let ff = tools.ffmpeg.ok_or("ffmpeg is not installed (brew install ffmpeg)")?;
    let out = output_path(f, out_ext, |p| p.exists());
    let codec: &[&str] = match out_ext {
        "webm" => &["-c:v", "libvpx-vp9", "-b:v", "0", "-crf", "33"],
        "avi" => &["-c:v", "mpeg4", "-q:v", "4"],
        "wmv" => &["-c:v", "wmv2"],
        _ => &["-c:v", "libx264", "-movflags", "+faststart"],
    };
    let mut c = Command::new(ff);
    c.args(["-y", "-loglevel", "error", "-i"])
        .arg(f)
        .args(["-pix_fmt", "yuv420p", "-vf", "scale=trunc(iw/2)*2:trunc(ih/2)*2"])
        .args(codec)
        .arg(&out);
    run_limited(&mut c, 300)?;
    Ok(vec![out])
}

fn archive(f: &Path, e: &str, out_ext: &str, tools: &Tools, sc: &Path) -> Result<Vec<PathBuf>, String> {
    let ex = sc.join("x");
    std::fs::create_dir_all(&ex).map_err(|er| er.to_string())?;
    let extracted = if e == "zip" {
        run_limited(Command::new("/usr/bin/ditto").args(["-x", "-k"]).arg(f).arg(&ex), 300)
    } else {
        let r = run_limited(Command::new("/usr/bin/bsdtar").arg("-xf").arg(f).arg("-C").arg(&ex), 300);
        match (r, e) {
            (Err(_), "rar") if tools.unrar.is_some() => run_limited(
                Command::new(tools.unrar.unwrap_or_default()).args(["x", "-y", "-idq"]).arg(f).arg(format!("{}/", ex.display())),
                300,
            ),
            (Err(_), "gz") => {
                // a bare .gz holds one file, not a tar
                let stem = f.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "file".into());
                let data = std::process::Command::new("/usr/bin/gunzip").arg("-c").arg(f).output().map_err(|er| er.to_string())?;
                if !data.status.success() {
                    return Err("could not read the GZIP file".into());
                }
                std::fs::write(ex.join(stem), data.stdout).map_err(|er| er.to_string())?;
                Ok(())
            }
            (r, _) => r,
        }
    };
    extracted?;
    let tmp_out = sc.join("packed");
    match out_ext {
        "zip" => run_limited(Command::new("/usr/bin/ditto").args(["-c", "-k", "--sequesterRsrc"]).arg(&ex).arg(&tmp_out), 300)?,
        "tar" => run_limited(Command::new("/usr/bin/bsdtar").arg("-cf").arg(&tmp_out).arg("-C").arg(&ex).arg("."), 300)?,
        "gz" => run_limited(Command::new("/usr/bin/bsdtar").arg("-czf").arg(&tmp_out).arg("-C").arg(&ex).arg("."), 300)?,
        other => return Err(format!("no archive format {other}")),
    }
    let ext_name = if out_ext == "gz" { "tar.gz" } else { out_ext };
    Ok(vec![place(f, &tmp_out, ext_name)?])
}

fn pdf_to(f: &Path, out_ext: &str, _sc: &Path) -> Result<Vec<PathBuf>, String> {
    match out_ext {
        "pages" => {
            let text = pdf_text(f)?;
            let out = output_path(f, "pages", |p| p.exists());
            pages::write_pages(&out, &paragraphs(&text)).map_err(|e| e.to_string())?;
            Ok(vec![out])
        }
        "xlsx" => {
            let rows = xlsx::rows_from_text(&pdf_text(f)?);
            let out = output_path(f, "xlsx", |p| p.exists());
            std::fs::write(&out, xlsx::xlsx_package(&rows)).map_err(|e| e.to_string())?;
            Ok(vec![out])
        }
        "epub" => {
            let text = pdf_text(f)?;
            let body = markdown::markdown_to_html(&markdown::plain_to_markdown(&text));
            let title = f.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let out = output_path(f, "epub", |p| p.exists());
            std::fs::write(&out, epub_write::epub_package(&title, &epub_write::xhtmlize(&body_of(&body)))).map_err(|e| e.to_string())?;
            Ok(vec![out])
        }
        other => Err(format!("PDF to {other} is not available")),
    }
}

fn document(f: &Path, e: &str, out_ext: &str, tools: &Tools, sc: &Path) -> Result<Vec<PathBuf>, String> {
    // plain text and Markdown convert to each other without going through HTML
    let source_text = |f: &Path| -> Result<String, String> {
        if e == "textclipping" {
            clipping::read_text_clipping(f)
        } else {
            read_lossy(f)
        }
    };
    if matches!(e, "txt" | "textclipping") && out_ext == "md" {
        let out = output_path(f, "md", |p| p.exists());
        std::fs::write(&out, markdown::plain_to_markdown(&source_text(f)?)).map_err(|er| er.to_string())?;
        return Ok(vec![out]);
    }
    if e == "textclipping" && out_ext == "txt" {
        let out = output_path(f, "txt", |p| p.exists());
        std::fs::write(&out, source_text(f)?).map_err(|er| er.to_string())?;
        return Ok(vec![out]);
    }
    if e == "md" && out_ext == "txt" {
        let out = output_path(f, "txt", |p| p.exists());
        std::fs::write(&out, markdown::markdown_to_plain(&read_lossy(f)?)).map_err(|er| er.to_string())?;
        return Ok(vec![out]);
    }
    let html = load_html(f, e, tools, sc)?;
    let pivot = sc.join("pivot.html");
    std::fs::write(&pivot, &html).map_err(|er| er.to_string())?;
    match out_ext {
        "html" => {
            let out = output_path(f, "html", |p| p.exists());
            std::fs::write(&out, html).map_err(|er| er.to_string())?;
            Ok(vec![out])
        }
        "md" => {
            let out = output_path(f, "md", |p| p.exists());
            std::fs::write(&out, markdown::html_to_markdown(&html)).map_err(|er| er.to_string())?;
            Ok(vec![out])
        }
        "txt" => {
            let out = output_path(f, "txt", |p| p.exists());
            std::fs::write(&out, html_to_text(&html, sc)?).map_err(|er| er.to_string())?;
            Ok(vec![out])
        }
        "docx" => {
            let made = sc.join("pivot.docx");
            run_limited(Command::new("/usr/bin/textutil").args(["-convert", "docx", "-output"]).arg(&made).arg(&pivot), 60)?;
            Ok(vec![place(f, &made, "docx")?])
        }
        "epub" => {
            let title = f.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let out = output_path(f, "epub", |p| p.exists());
            std::fs::write(&out, epub_write::epub_package(&title, &epub_write::xhtmlize(&body_of(&html)))).map_err(|er| er.to_string())?;
            Ok(vec![out])
        }
        "pages" => {
            let text = html_to_text(&html, sc)?;
            let out = output_path(f, "pages", |p| p.exists());
            pages::write_pages(&out, &paragraphs(&text)).map_err(|er| er.to_string())?;
            Ok(vec![out])
        }
        "pdf" => {
            soffice(tools, &["--convert-to", "pdf"], &pivot, sc)?;
            Ok(vec![place(f, &sc.join("pivot.pdf"), "pdf")?])
        }
        "png" | "jpg" => {
            soffice(tools, &["--convert-to", "pdf"], &pivot, sc)?;
            // name the PDF after the original so the page images carry its name
            let name = f.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "document".into());
            let named = sc.join(format!("{name}.pdf"));
            std::fs::rename(sc.join("pivot.pdf"), &named).map_err(|er| er.to_string())?;
            let pages_out = crate::services::pdfkit_convert(&named, out_ext)?;
            let mut outs = vec![];
            for p in pages_out {
                let fname = p.file_name().ok_or("bad path")?.to_string_lossy().to_string();
                let dest = f.parent().unwrap_or(Path::new(".")).join(&fname);
                let dest = if dest.exists() { output_path(&dest, out_ext, |q| q.exists()) } else { dest };
                std::fs::rename(&p, &dest).or_else(|_| std::fs::copy(&p, &dest).map(|_| ())).map_err(|er| er.to_string())?;
                outs.push(dest);
            }
            Ok(outs)
        }
        other => Err(format!("{e} to {other} is not available yet")),
    }
}

// ------------------------------------------------------------------ extractArchive

/// True when an archive entry name could land outside the destination folder: absolute, a `..`
/// component (with `/` or `\\` as separator), a Windows drive prefix, or a NUL byte.
pub(crate) fn unsafe_entry(name: &str) -> bool {
    let b = name.as_bytes();
    if b.contains(&0) || name.starts_with('/') || name.starts_with('\\') {
        return true;
    }
    if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b.len() == 2 || b[2] == b'/' || b[2] == b'\\') {
        return true;
    }
    name.split(['/', '\\']).any(|c| c == "..")
}

/// Refuse the whole archive when any listed entry is unsafe; the listing is one name per line.
fn guard_listing(listing: &[u8]) -> Result<(), String> {
    if listing.contains(&0) {
        return Err("unsafe path in archive: entry with a NUL byte".into());
    }
    let text = String::from_utf8_lossy(listing);
    match text.lines().find(|l| unsafe_entry(l)) {
        Some(bad) => Err(format!("unsafe path in archive: {bad}")),
        None => Ok(()),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Zip,
    Tar,
    Tgz,
    Gz,
    Rar,
}

fn archive_kind(f: &Path) -> Result<Kind, String> {
    let name = f.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        return Ok(Kind::Tgz);
    }
    match ext(f).as_str() {
        "zip" => Ok(Kind::Zip),
        "tar" => Ok(Kind::Tar),
        "gz" => Ok(Kind::Gz),
        "rar" => Ok(Kind::Rar),
        other => Err(format!("Extract does not support .{other} archives")),
    }
}

/// The folder name for an archive: its file name without the archive extension(s).
fn folder_stem(f: &Path) -> String {
    let name = f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let lower = name.to_ascii_lowercase();
    let cut = [".tar.gz", ".tgz", ".zip", ".tar", ".gz", ".rar"].iter().find(|e| lower.ends_with(*e)).map(|e| e.len()).unwrap_or(0);
    let stem = name[..name.len() - cut].trim().to_string();
    if stem.is_empty() || stem == "." || stem == ".." {
        "archive".into()
    } else {
        stem
    }
}

/// Create a fresh sibling folder; `create_dir` is atomic and fails if the name exists (as a folder
/// or a file), so an existing item is never merged into or replaced.
fn make_folder(f: &Path) -> Result<PathBuf, String> {
    let parent = f.parent().unwrap_or(Path::new("."));
    let stem = folder_stem(f);
    for n in 1..=10_000u32 {
        let dir = parent.join(if n == 1 { stem.clone() } else { format!("{stem} {n}") });
        match std::fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("could not create {}: {e}", dir.display())),
        }
    }
    Err("no free folder name".into())
}

const EXTRACT_SECS: u64 = 120;

/// Unpack `f` into a new sibling folder and return it. Entries are listed and checked first, so an
/// archive with an unsafe path is refused before anything is created. On any failure the folder
/// made here is removed. The archive itself is only ever read.
pub(crate) fn extract_archive(f: &Path, tools: &Tools) -> Result<PathBuf, String> {
    let kind = archive_kind(f)?;
    let unrar = if kind == Kind::Rar { Some(tools.unrar.ok_or("needs unrar (brew install --cask rar)")?) } else { None };
    // 1. guard, before any folder exists
    match kind {
        Kind::Zip => guard_listing(&run_listing(Command::new("/usr/bin/unzip").arg("-Z1").arg(f), EXTRACT_SECS)?)?,
        Kind::Tar => guard_listing(&run_listing(Command::new("/usr/bin/tar").arg("-tf").arg(f), EXTRACT_SECS)?)?,
        Kind::Tgz => guard_listing(&run_listing(Command::new("/usr/bin/tar").arg("-tzf").arg(f), EXTRACT_SECS)?)?,
        Kind::Rar => guard_listing(&run_listing(Command::new(unrar.unwrap_or_default()).arg("lb").arg(f), EXTRACT_SECS)?)?,
        Kind::Gz => {}
    }
    // 2. extract into a folder that did not exist
    let dir = make_folder(f)?;
    let res = match kind {
        Kind::Zip => run_limited(Command::new("/usr/bin/ditto").args(["-x", "-k"]).arg(f).arg(&dir), EXTRACT_SECS),
        Kind::Tar => run_limited(Command::new("/usr/bin/tar").arg("-xf").arg(f).arg("-C").arg(&dir), EXTRACT_SECS),
        Kind::Tgz => run_limited(Command::new("/usr/bin/tar").arg("-xzf").arg(f).arg("-C").arg(&dir), EXTRACT_SECS),
        Kind::Rar => run_limited(
            Command::new(unrar.unwrap_or_default()).args(["x", "-y", "-idq"]).arg(f).arg(format!("{}/", dir.display())),
            EXTRACT_SECS,
        ),
        Kind::Gz => gunzip_into(f, &dir),
    };
    match res {
        Ok(()) => Ok(dir),
        Err(e) => {
            let _ = std::fs::remove_dir_all(&dir);
            Err(format!("could not extract {}: {e}", f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()))
        }
    }
}

/// A bare .gz holds one file: `gunzip -c` into `dir/<name without .gz>`.
fn gunzip_into(f: &Path, dir: &Path) -> Result<(), String> {
    let name = folder_stem(f);
    let out = std::fs::File::create(dir.join(name)).map_err(|e| e.to_string())?;
    run_io(Command::new("/usr/bin/gunzip").arg("-c").arg(f), EXTRACT_SECS, Stdio::from(out), false).map(|_| ())
}
