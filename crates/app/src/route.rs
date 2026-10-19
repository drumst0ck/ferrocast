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
    cmd.stdout(Stdio::null()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(secs);
    loop {
        match child.try_wait() {
            Ok(Some(st)) => {
                let out = child.wait_with_output().map_err(|e| e.to_string())?;
                return if st.success() {
                    Ok(())
                } else {
                    Err(String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("command failed").trim().to_string())
                };
            }
            Ok(None) if Instant::now() > deadline => {
                let _ = child.kill();
                let _ = child.wait();
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
