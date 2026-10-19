//! Runs every one of Tangerine's 275 (input → output) pairs on a real generated file and checks
//! the output. Slow (LibreOffice, ffmpeg): `cargo test -p ferrocast -- --ignored real_matrix`.

use crate::route::run_limited;
use crate::services::run_conversion;
use ferrocast_core::convert::{targets, Tools};
use ferrocast_core::{clipping_fixture_rsrc, epub_write, matrix, pages, xlsx};
use std::path::{Path, PathBuf};
use std::process::Command;

fn sh(cmd: &mut Command) -> bool {
    run_limited(cmd, 180).is_ok()
}

fn so(tools: &Tools, dir: &Path, args: &[&str], input: &Path) -> bool {
    let Some(soffice) = tools.soffice else { return false };
    let mut c = Command::new(soffice);
    c.arg(format!("-env:UserInstallation=file://{}/profile", dir.display()))
        .arg("--headless")
        .args(args)
        .arg("--outdir")
        .arg(dir)
        .arg(input);
    sh(&mut c)
}

/// A real sample file for an input format, or `None` when this Mac cannot make one.
fn fixture(i: &str, root: &Path, tools: &Tools, base: &Path) -> Option<PathBuf> {
    let d = root.join(i);
    std::fs::create_dir_all(&d).ok()?;
    let f = d.join(format!("sample.{i}"));
    let ff = tools.ffmpeg;
    let ok = match i {
        "png" => std::fs::copy(base, &f).is_ok(),
        "jpg" | "tiff" | "bmp" | "heic" | "avif" => {
            sh(Command::new("/usr/bin/sips").args(["-s", "format", if i == "jpg" { "jpeg" } else { i }]).arg(base).arg("--out").arg(&f))
        }
        "webp" => tools.cwebp.is_some_and(|cw| sh(Command::new(cw).args(["-quiet", "-q", "80"]).arg(base).arg("-o").arg(&f))),
        "svg" => std::fs::write(
            &f,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="60"><rect width="100" height="60" fill="red"/></svg>"#,
        )
        .is_ok(),
        "mp3" | "m4a" | "wav" | "flac" | "ogg" | "opus" | "aiff" | "wma" => {
            let codec = match i {
                "mp3" => "libmp3lame",
                "m4a" => "aac",
                "wav" => "pcm_s16le",
                "flac" => "flac",
                "ogg" | "opus" => "libopus",
                "aiff" => "pcm_s16be",
                _ => "wmav2",
            };
            ff.is_some_and(|ff| {
                sh(Command::new(ff)
                    .args(["-y", "-loglevel", "error", "-f", "lavfi", "-i", "sine=frequency=440:duration=1", "-c:a", codec])
                    .arg(&f))
            })
        }
        "mp4" | "mov" | "mkv" | "webm" | "avi" | "wmv" | "gif" => {
            // real videos carry sound: video → MP3 needs an audio track
            let codec: &[&str] = match i {
                "mp4" | "mov" | "mkv" => &["-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac"],
                "webm" => &["-c:v", "libvpx-vp9", "-c:a", "libopus"],
                "avi" => &["-c:v", "mpeg4", "-c:a", "libmp3lame"],
                "wmv" => &["-c:v", "wmv2", "-c:a", "wmav2"],
                _ => &[],
            };
            let audio: &[&str] = if i == "gif" { &[] } else { &["-f", "lavfi", "-i", "sine=frequency=440:duration=1"] };
            ff.is_some_and(|ff| {
                sh(Command::new(ff)
                    .args(["-y", "-loglevel", "error", "-f", "lavfi", "-i", "testsrc=size=160x120:rate=10"])
                    .args(audio)
                    .args(["-t", "1"])
                    .args(codec)
                    .arg(&f))
            })
        }
        "txt" => std::fs::write(&f, "Hello Ferrocast\n\nSecond paragraph é ñ\n").is_ok(),
        "md" => std::fs::write(&f, "# Title\n\nHello **bold** world.\n\n- one\n- two\n").is_ok(),
        "html" => std::fs::write(&f, "<html><body><h1>Title</h1><p>Hello <b>bold</b> world.</p></body></html>").is_ok(),
        "rtf" | "rtfd" | "docx" => {
            let src = d.join("src.txt");
            std::fs::write(&src, "Hello Ferrocast\n\nSecond paragraph\n").ok()?;
            let r = sh(Command::new("/usr/bin/textutil").args(["-convert", i, "-output"]).arg(&f).arg(&src));
            let _ = std::fs::remove_file(&src);
            r
        }
        "pages" => pages::write_pages(&f, &["Hello Ferrocast".into(), String::new(), "Second paragraph".into()]).is_ok(),
        "textClipping" => {
            std::fs::write(&f, b"").ok()?;
            std::fs::write(f.join("..namedfork/rsrc"), clipping_fixture_rsrc("Hello clipping")).is_ok()
        }
        "epub" => std::fs::write(&f, epub_write::epub_package("Book", "<p>Hello epub</p><p>Second</p>")).is_ok(),
        "csv" => std::fs::write(&f, "a,b\n1,2\n").is_ok(),
        "tsv" => std::fs::write(&f, "a\tb\n1\t2\n").is_ok(),
        "xlsx" => std::fs::write(&f, xlsx::xlsx_package(&[vec!["a".into(), "b".into()], vec!["1".into(), "2".into()]])).is_ok(),
        "ods" | "xls" => {
            let src = d.join("src.csv");
            std::fs::write(&src, "a,b\n1,2\n").ok()?;
            let r = so(tools, &d, &["--convert-to", i], &src) && std::fs::rename(d.join(format!("src.{i}")), &f).is_ok();
            let _ = std::fs::remove_file(&src);
            r
        }
        "srt" => std::fs::write(&f, "1\n00:00:01,000 --> 00:00:02,000\nHello\n\n2\n00:00:03,000 --> 00:00:04,000\nWorld\n").is_ok(),
        "vtt" => std::fs::write(&f, "WEBVTT\n\n00:01.000 --> 00:02.000\nHello\n\n00:03.000 --> 00:04.000\nWorld\n").is_ok(),
        "pdf" => {
            let src = d.join("src.txt");
            std::fs::write(&src, "Hello Ferrocast\nSecond line\n").ok()?;
            let r = so(tools, &d, &["--convert-to", "pdf"], &src) && std::fs::rename(d.join("src.pdf"), &f).is_ok();
            let _ = std::fs::remove_file(&src);
            r
        }
        "pptx" | "ppt" => {
            let src = d.join("src.txt");
            std::fs::write(&src, "Slide one\n").ok()?;
            let ok_pdf = so(tools, &d, &["--convert-to", "pdf"], &src);
            let pdf = d.join("src.pdf");
            let ok = ok_pdf && so(tools, &d, &["--infilter=impress_pdf_import", "--convert-to", "pptx"], &pdf);
            let pptx = d.join("src.pptx");
            let r = if i == "pptx" {
                ok && std::fs::rename(&pptx, &f).is_ok()
            } else {
                ok && so(tools, &d, &["--convert-to", "ppt"], &pptx) && std::fs::rename(d.join("src.ppt"), &f).is_ok()
            };
            let _ = std::fs::remove_file(&src);
            let _ = std::fs::remove_file(&pdf);
            let _ = std::fs::remove_file(&pptx);
            r
        }
        "zip" | "tar" | "gz" => {
            let inner = d.join("payload");
            std::fs::create_dir_all(&inner).ok()?;
            std::fs::write(inner.join("in.txt"), "inside").ok()?;
            match i {
                "zip" => sh(Command::new("/usr/bin/ditto").args(["-c", "-k", "--keepParent"]).arg(&inner).arg(&f)),
                "tar" => sh(Command::new("/usr/bin/bsdtar").arg("-cf").arg(&f).arg("-C").arg(&d).arg("payload")),
                _ => sh(Command::new("/usr/bin/bsdtar").arg("-czf").arg(&f).arg("-C").arg(&d).arg("payload")),
            }
        }
        // no RAR writer runs here (the bundled rar is x86-only): a hand-built RAR5 holding in.txt
        "rar" => std::fs::write(&f, decode_hex(RAR_HEX)).is_ok(),
        _ => false,
    };
    (ok && f.exists()).then_some(f)
}

const RAR_HEX: &str =
    "526172211a070100c51a333203010000db8737921302020a040a00386e3591000006696e2e747874696e736964652072617219b23a3503050000";

fn decode_hex(s: &str) -> Vec<u8> {
    (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap_or(0)).collect()
}

fn magic_ok(path: &Path, ext: &str) -> Result<(), String> {
    magic_only(path, ext)
}

fn magic_only(path: &Path, ext: &str) -> Result<(), String> {
    let b = std::fs::read(path).map_err(|e| e.to_string())?;
    if b.is_empty() {
        return Err("empty file".into());
    }
    let has = |off: usize, m: &[u8]| b.len() >= off + m.len() && &b[off..off + m.len()] == m;
    let ok = match ext {
        "png" => has(0, b"\x89PNG"),
        "jpg" => has(0, &[0xFF, 0xD8]),
        "webp" => has(0, b"RIFF") && has(8, b"WEBP"),
        "heic" | "avif" | "m4a" | "mp4" | "mov" => {
            has(4, b"ftyp") || has(4, b"moov") || has(4, b"wide") || has(4, b"mdat") || has(4, b"free")
        }
        "tiff" => has(0, b"II*\0") || has(0, b"MM\0*"),
        "bmp" => has(0, b"BM"),
        "gif" => has(0, b"GIF8"),
        "pdf" => has(0, b"%PDF"),
        "mp3" => has(0, b"ID3") || (b[0] == 0xFF && b.get(1).is_some_and(|x| x & 0xE0 == 0xE0)),
        "wav" | "avi" => has(0, b"RIFF"),
        "flac" => has(0, b"fLaC"),
        "ogg" | "opus" => has(0, b"OggS"),
        "aiff" => has(0, b"FORM"),
        "wma" | "wmv" => has(0, &[0x30, 0x26, 0xB2, 0x75]),
        "mkv" | "webm" => has(0, &[0x1A, 0x45, 0xDF, 0xA3]),
        "docx" | "pptx" | "xlsx" | "ods" | "epub" | "pages" | "zip" => has(0, b"PK"),
        "tar" | "gz" => Command::new("/usr/bin/bsdtar").arg("-tf").arg(path).output().map(|o| o.status.success()).unwrap_or(false),
        // text formats: non-empty is the check, plus valid UTF-8
        _ => std::str::from_utf8(&b).is_ok(),
    };
    if ok {
        Ok(())
    } else {
        Err(format!("wrong content for .{ext} (starts {:02x?})", &b[..b.len().min(6)]))
    }
}

fn captured(cmd: &mut Command) -> String {
    cmd.output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default()
}

/// Reads the output back: text inside documents/sheets, pixel size of images, duration of media.
fn content_ok(input: &str, path: &Path, ext: &str, tools: &Tools) -> Result<(), String> {
    let marker = match input {
        "pptx" | "ppt" => "Slide",
        "csv" | "tsv" | "xlsx" | "xls" | "ods" => "2",
        "svg" => "",
        _ => "Hello",
    };
    let text = match ext {
        "txt" | "md" | "html" | "csv" | "tsv" | "srt" | "vtt" => Some(std::fs::read_to_string(path).unwrap_or_default()),
        "docx" => Some(captured(Command::new("/usr/bin/unzip").args(["-p"]).arg(path).arg("word/document.xml"))),
        "xlsx" => Some(captured(Command::new("/usr/bin/unzip").args(["-p"]).arg(path).arg("xl/worksheets/sheet1.xml"))),
        "ods" => Some(captured(Command::new("/usr/bin/unzip").args(["-p"]).arg(path).arg("content.xml"))),
        "epub" => Some(captured(Command::new("/usr/bin/unzip").args(["-p"]).arg(path).arg("*.xhtml"))),
        "pages" => Some(captured(Command::new("/usr/bin/unzip").args(["-p"]).arg(path).arg("index.xml"))),
        _ => None,
    };
    let pdf_in = matches!(input, "pdf" | "pptx" | "ppt");
    if let Some(t) = text {
        let skip = marker.is_empty() || (pdf_in && ext == "docx");
        if !skip && !t.contains(marker) {
            return Err(format!(".{ext} does not contain {marker:?}"));
        }
    }
    match ext {
        "png" | "jpg" | "webp" | "heic" | "tiff" | "avif" | "bmp" => {
            let out = captured(Command::new("/usr/bin/sips").args(["-g", "pixelWidth"]).arg(path));
            let w: u32 = out.lines().find_map(|l| l.trim().strip_prefix("pixelWidth:")).and_then(|v| v.trim().parse().ok()).unwrap_or(0);
            if w == 0 && ext != "webp" {
                return Err("image has no readable width".into());
            }
        }
        "mp3" | "m4a" | "wav" | "flac" | "ogg" | "opus" | "aiff" | "wma" | "mp4" | "mov" | "mkv" | "webm" | "avi" | "wmv" | "gif" => {
            let probe = tools.ffmpeg.map(|f| std::path::Path::new(f).with_file_name("ffprobe"));
            if let Some(pr) = probe {
                let d = captured(
                    Command::new(pr).args(["-v", "error", "-show_entries", "format=duration", "-of", "default=nw=1:nk=1"]).arg(path),
                );
                if d.trim().parse::<f64>().map(|d| d <= 0.0).unwrap_or(true) {
                    return Err(format!(".{ext} has no readable duration"));
                }
            }
        }
        _ => {}
    }
    Ok(())
}

#[test]
#[ignore = "slow: runs 275 real conversions; cargo test -p ferrocast -- --ignored real_matrix --nocapture"]
fn real_matrix_all_275_pairs() {
    let tools = Tools::detect();
    let root = std::env::temp_dir().join(format!("fc-matrix-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let base = root.join("base.png");
    let icon = concat!(env!("CARGO_MANIFEST_DIR"), "/../../resources/icon-1024.png");
    assert!(sh(Command::new("/usr/bin/sips").args(["-Z", "256", icon, "--out"]).arg(&base)));
    let (mut ok, mut failures) = (0usize, vec![]);
    for i in matrix::inputs() {
        let outs = matrix::outputs_for(i);
        let Some(f) = fixture(i, &root, &tools, &base) else {
            for o in outs {
                failures.push(format!("{i}→{o}: NO FIXTURE (cannot create a .{i} here)"));
            }
            continue;
        };
        let offered = targets(&f, &tools);
        for o in outs {
            let Some(t) = offered.iter().find(|t| t.ext == o) else {
                failures.push(format!("{i}→{o}: not offered by targets()"));
                continue;
            };
            let (made, errs) = run_conversion(std::slice::from_ref(&f), t, &tools);
            match made.first() {
                None => failures.push(format!("{i}→{o}: {}", errs.first().cloned().unwrap_or_else(|| "no output".into()))),
                Some(p) => match magic_ok(p, o).and_then(|_| content_ok(i, p, o, &tools)) {
                    Ok(()) => ok += 1,
                    Err(e) => failures.push(format!("{i}→{o}: {e}")),
                },
            }
        }
    }
    eprintln!("MATRIX {ok}/275 pairs verified on real files; {} failing", failures.len());
    for f in &failures {
        eprintln!("  FAIL {f}");
    }
    let _ = std::fs::remove_dir_all(&root);
    assert!(failures.is_empty(), "{} of 275 pairs failed", failures.len());
}

#[test]
#[ignore = "debug probe: FC_PROBE_PDF=/path/x.pdf cargo test -p ferrocast -- --ignored probe_pdf_epub --nocapture"]
fn probe_pdf_epub() {
    let Ok(p) = std::env::var("FC_PROBE_PDF") else { return };
    let tools = Tools::detect();
    let f = PathBuf::from(p);
    let t = targets(&f, &tools).into_iter().find(|t| t.ext == "epub").expect("epub offered");
    let (made, errs) = run_conversion(std::slice::from_ref(&f), &t, &tools);
    eprintln!("PROBE made={made:?} errs={errs:?}");
    if let Some(m) = made.first() {
        let l = captured(Command::new("/usr/bin/unzip").arg("-l").arg(m));
        eprintln!("PROBE list:\n{l}");
        let x = captured(Command::new("/usr/bin/unzip").arg("-p").arg(m).arg("*.xhtml"));
        eprintln!("PROBE xhtml:\n{x}");
    }
}
