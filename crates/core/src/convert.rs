//! File conversion planning: which targets a file can become and which local tool does it.
//! Pure functions; the app runs the planned commands. Tools that ship with macOS come first;
//! ffmpeg and LibreOffice are used only when installed. Nothing leaves the Mac.

use std::path::{Path, PathBuf};

/// Broad family of a file, from its extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Image,
    Audio,
    Video,
    Document,
    Spreadsheet,
    Slides,
    Pdf,
    Subtitle,
    Archive,
    Folder,
    Other,
}

pub fn family_of(path: &Path) -> Family {
    if path.is_dir() {
        return Family::Folder;
    }
    match ext(path).as_str() {
        "jpg" | "jpeg" | "png" | "svg" | "webp" | "heic" | "heif" | "tif" | "tiff" | "avif" | "bmp" | "gif" | "jxl" | "jp2" | "psd"
        | "dng" | "cr2" | "cr3" | "nef" | "arw" | "raf" | "orf" | "rw2" | "icns" | "ico" => Family::Image,
        "mp3" | "m4a" | "wav" | "flac" | "ogg" | "opus" | "aiff" | "aif" | "wma" | "caf" | "aac" | "m4b" => Family::Audio,
        "mp4" | "mov" | "mkv" | "webm" | "avi" | "wmv" | "m4v" | "mpg" | "mpeg" | "ts" | "3gp" => Family::Video,
        "docx" | "doc" | "pages" | "textclipping" | "md" | "markdown" | "txt" | "rtf" | "rtfd" | "odt" | "html" | "htm" | "epub" => {
            Family::Document
        }
        "xlsx" | "xls" | "ods" | "csv" | "tsv" | "numbers" => Family::Spreadsheet,
        "ppt" | "pptx" | "key" | "odp" => Family::Slides,
        "pdf" => Family::Pdf,
        "srt" | "vtt" => Family::Subtitle,
        "zip" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "7z" | "rar" => Family::Archive,
        _ => Family::Other,
    }
}

/// SRT/VTT cues as plain text: no numbering, timings, header or markup. One line per cue line.
pub fn subtitle_text(src: &str) -> String {
    let mut out: Vec<String> = vec![];
    for raw in src.replace("\r\n", "\n").lines() {
        let l = raw.trim().trim_start_matches('\u{feff}');
        if l.is_empty() || l == "WEBVTT" || l.starts_with("NOTE") || l.contains("-->") || l.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        // drop <i>…</i>, <c.color>, {\an8} style markup
        let (mut clean, mut depth) = (String::new(), 0);
        for c in l.chars() {
            match c {
                '<' | '{' => depth += 1,
                '>' | '}' if depth > 0 => depth -= 1,
                _ if depth == 0 => clean.push(c),
                _ => {}
            }
        }
        let clean = clean.trim();
        if !clean.is_empty() {
            out.push(clean.to_string());
        }
    }
    out.join("\n")
}

pub fn ext(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).unwrap_or_default()
}

/// Which optional tools exist on this Mac. Built-ins (sips, afconvert, avconvert, textutil,
/// ditto, zip, tar) are assumed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tools {
    pub ffmpeg: Option<&'static str>,
    pub soffice: Option<&'static str>,
    /// Google's WebP encoder; many ffmpeg builds lack libwebp.
    pub cwebp: Option<&'static str>,
    /// RAR extractor (RAR is input only).
    pub unrar: Option<&'static str>,
}

impl Tools {
    /// Probe the usual install locations.
    pub fn detect() -> Tools {
        fn first(paths: &[&'static str]) -> Option<&'static str> {
            paths.iter().copied().find(|p| Path::new(p).exists())
        }
        Tools {
            ffmpeg: first(&["/opt/homebrew/bin/ffmpeg", "/usr/local/bin/ffmpeg"]),
            soffice: first(&["/opt/homebrew/bin/soffice", "/Applications/LibreOffice.app/Contents/MacOS/soffice"]),
            cwebp: first(&["/opt/homebrew/bin/cwebp", "/usr/local/bin/cwebp"]),
            unrar: first(&["/opt/homebrew/bin/unrar", "/usr/local/bin/unrar", "/Applications/rar/unrar"]),
        }
    }
}

/// How a conversion is carried out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Engine {
    Sips,
    Afconvert,
    Avconvert,
    Textutil,
    Archive,
    Ffmpeg,
    Soffice,
    Cwebp,
    /// PDFKit through osascript: page images at 300 DPI and the text layer.
    Pdfkit,
    /// Subtitle text without timings.
    SubtitleText,
    /// Slides → PDF (LibreOffice) → page images.
    SlideImages,
    /// RAR → extract → ZIP.
    Unrar,
    /// Multi-step routes (text pivot, archive repack, GIF → video, PDF → sheet or Pages).
    Route,
}

impl Engine {
    pub fn label(&self) -> &'static str {
        match self {
            Engine::Sips | Engine::Afconvert | Engine::Avconvert | Engine::Textutil | Engine::Archive => "built in",
            Engine::Ffmpeg => "ffmpeg",
            Engine::Soffice => "LibreOffice",
            Engine::Cwebp => "cwebp",
            Engine::Pdfkit | Engine::SubtitleText => "built in",
            Engine::SlideImages => "LibreOffice",
            Engine::Unrar => "unrar",
            Engine::Route => "built in",
        }
    }
}

/// One offered target for a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// Output extension, e.g. `png`.
    pub ext: &'static str,
    pub label: &'static str,
    pub engine: Engine,
    /// `None` when the engine is available; otherwise what to install.
    pub needs: Option<&'static str>,
}

fn t(ext: &'static str, label: &'static str, engine: Engine, tools: &Tools) -> Target {
    let needs = match engine {
        Engine::Ffmpeg if tools.ffmpeg.is_none() => Some("ffmpeg (brew install ffmpeg)"),
        Engine::Soffice if tools.soffice.is_none() => Some("LibreOffice (brew install --cask libreoffice)"),
        Engine::Cwebp if tools.cwebp.is_none() => Some("webp (brew install webp)"),
        Engine::SlideImages if tools.soffice.is_none() => Some("LibreOffice (brew install --cask libreoffice)"),
        Engine::Unrar if tools.unrar.is_none() => Some("unrar (brew install --cask rar)"),
        _ => None,
    };
    Target { ext, label, engine, needs }
}

/// Targets for a file, best first. The current format is excluded. Targets that need a
/// missing optional tool are still listed (with `needs` set) so the user learns what to install.
pub fn targets(path: &Path, tools: &Tools) -> Vec<Target> {
    let e = ext(path);
    let mut v = match family_of(path) {
        Family::Image => vec![
            t("png", "PNG", Engine::Sips, tools),
            t("jpg", "JPEG", Engine::Sips, tools),
            t("webp", "WebP", Engine::Cwebp, tools),
            t("heic", "HEIC", Engine::Sips, tools),
            t("tiff", "TIFF", Engine::Sips, tools),
            t("avif", "AVIF", Engine::Sips, tools),
            t("bmp", "BMP", Engine::Sips, tools),
            t("gif", "GIF", Engine::Sips, tools),
            t("pdf", "PDF", Engine::Sips, tools),
        ],
        Family::Audio => vec![
            t("m4a", "M4A (AAC)", Engine::Afconvert, tools),
            t("wav", "WAV", Engine::Afconvert, tools),
            t("aiff", "AIFF", Engine::Afconvert, tools),
            t("caf", "CAF", Engine::Afconvert, tools),
            t("mp3", "MP3", Engine::Ffmpeg, tools),
            t("flac", "FLAC", Engine::Ffmpeg, tools),
            t("ogg", "OGG Vorbis", Engine::Ffmpeg, tools),
            t("opus", "Opus", Engine::Ffmpeg, tools),
            t("wma", "WMA", Engine::Ffmpeg, tools),
        ],
        Family::Video => vec![
            t("mp4", "MP4 (H.264)", Engine::Avconvert, tools),
            t("mov", "MOV", Engine::Avconvert, tools),
            t("m4a", "M4A (audio only)", Engine::Avconvert, tools),
            t("webm", "WebM", Engine::Ffmpeg, tools),
            t("mkv", "MKV", Engine::Ffmpeg, tools),
            t("gif", "GIF", Engine::Ffmpeg, tools),
            t("mp3", "MP3 (audio only)", Engine::Ffmpeg, tools),
            t("avi", "AVI", Engine::Ffmpeg, tools),
            t("wmv", "WMV", Engine::Ffmpeg, tools),
        ],
        Family::Document => match e.as_str() {
            "md" | "markdown" | "txt" => vec![
                t("pdf", "PDF", Engine::Soffice, tools),
                t("docx", "Word (DOCX)", Engine::Textutil, tools),
                t("html", "HTML", Engine::Textutil, tools),
                t("rtf", "RTF", Engine::Textutil, tools),
                t("epub", "EPUB", Engine::Soffice, tools),
            ],
            "epub" | "pages" => vec![
                t("pdf", "PDF", Engine::Soffice, tools),
                t("docx", "Word (DOCX)", Engine::Soffice, tools),
                t("txt", "Plain text", Engine::Soffice, tools),
            ],
            _ => vec![
                t("pdf", "PDF", Engine::Soffice, tools),
                t("docx", "Word (DOCX)", Engine::Textutil, tools),
                t("txt", "Plain text", Engine::Textutil, tools),
                t("html", "HTML", Engine::Textutil, tools),
                t("rtf", "RTF", Engine::Textutil, tools),
                t("odt", "OpenDocument", Engine::Textutil, tools),
                t("epub", "EPUB", Engine::Soffice, tools),
            ],
        },
        Family::Spreadsheet => vec![
            t("pdf", "PDF", Engine::Soffice, tools),
            t("xlsx", "Excel (XLSX)", Engine::Soffice, tools),
            t("csv", "CSV", Engine::Soffice, tools),
            t("ods", "OpenDocument", Engine::Soffice, tools),
            t("tsv", "TSV", Engine::Soffice, tools),
        ],
        Family::Slides => vec![
            t("pdf", "PDF", Engine::Soffice, tools),
            t("pptx", "PowerPoint (PPTX)", Engine::Soffice, tools),
            t("png", "PNG (every slide)", Engine::SlideImages, tools),
            t("jpg", "JPEG (every slide)", Engine::SlideImages, tools),
        ],
        Family::Pdf => vec![
            t("png", "PNG (all pages, 300 DPI)", Engine::Pdfkit, tools),
            t("jpg", "JPEG (all pages, 300 DPI)", Engine::Pdfkit, tools),
            t("txt", "Plain text", Engine::Pdfkit, tools),
            t("md", "Markdown (text)", Engine::Pdfkit, tools),
            t("docx", "Word (DOCX)", Engine::Soffice, tools),
            t("epub", "EPUB", Engine::Soffice, tools),
            t("pptx", "PowerPoint (PPTX)", Engine::Soffice, tools),
        ],
        Family::Subtitle => vec![
            t("vtt", "WebVTT", Engine::Ffmpeg, tools),
            t("srt", "SubRip (SRT)", Engine::Ffmpeg, tools),
            t("txt", "Plain text", Engine::SubtitleText, tools),
        ],
        Family::Archive if e == "rar" => vec![t("zip", "ZIP", Engine::Unrar, tools)],
        Family::Archive | Family::Folder => vec![
            t("zip", "ZIP", Engine::Archive, tools),
            t("tar", "TAR", Engine::Archive, tools),
            t("tar.gz", "tar.gz", Engine::Archive, tools),
        ],
        Family::Other => vec![t("zip", "ZIP", Engine::Archive, tools)],
    };
    v.retain(|x| x.ext != e && !(e == "jpeg" && x.ext == "jpg") && !(e == "tif" && x.ext == "tiff"));
    // Formats Tangerine knows: exactly its outputs, in its order. A target the single-command
    // engines already do is reused; everything else goes through a multi-step route.
    let key = match e.as_str() {
        "jpeg" => "jpg",
        "tif" => "tiff",
        other => other,
    };
    if crate::matrix::inputs().iter().any(|i| i.eq_ignore_ascii_case(key)) {
        let always_route = matches!(key, "md" | "epub" | "textclipping" | "zip" | "tar" | "gz" | "rar" | "pages");
        return crate::matrix::outputs_for(key)
            .into_iter()
            .map(|o| {
                let want = if o == "gz" { "tar.gz" } else { o };
                match v.iter().find(|x| x.ext == want) {
                    Some(x) if !always_route => adjust_engine(key, x.clone(), tools),
                    _ => Target { ext: o, label: label_for(o), engine: Engine::Route, needs: route_needs(key, o, tools) },
                }
            })
            .collect();
    }
    v
}

/// Some inputs cannot be read by the engine that normally writes the output: `afconvert` does not
/// read ogg/opus/wma and `avconvert` does not read mkv/webm/avi/wmv, so ffmpeg takes over there.
fn adjust_engine(input: &str, mut x: Target, tools: &Tools) -> Target {
    let ffmpeg_only_audio = matches!(input, "ogg" | "opus" | "wma");
    let ffmpeg_only_video = matches!(input, "mkv" | "webm" | "avi" | "wmv" | "gif");
    if (ffmpeg_only_audio && x.engine == Engine::Afconvert) || (ffmpeg_only_video && x.engine == Engine::Avconvert) {
        x.engine = Engine::Ffmpeg;
        x.needs = if tools.ffmpeg.is_none() { Some("ffmpeg (brew install ffmpeg)") } else { None };
    }
    x
}

/// Menu label for an output extension.
pub fn label_for(ext: &str) -> &'static str {
    match ext {
        "jpg" => "JPEG",
        "png" => "PNG",
        "webp" => "WebP",
        "heic" => "HEIC",
        "tiff" => "TIFF",
        "avif" => "AVIF",
        "bmp" => "BMP",
        "gif" => "GIF",
        "pdf" => "PDF",
        "mp3" => "MP3",
        "m4a" => "M4A",
        "wav" => "WAV",
        "flac" => "FLAC",
        "ogg" => "OGG",
        "opus" => "Opus",
        "aiff" => "AIFF",
        "wma" => "WMA",
        "mp4" => "MP4",
        "mov" => "MOV",
        "mkv" => "MKV",
        "webm" => "WebM",
        "avi" => "AVI",
        "wmv" => "WMV",
        "docx" => "Word (DOCX)",
        "pages" => "Pages",
        "md" => "Markdown",
        "txt" => "Plain text",
        "html" => "HTML",
        "epub" => "EPUB",
        "pptx" => "PowerPoint (PPTX)",
        "xlsx" => "Excel (XLSX)",
        "ods" => "OpenDocument",
        "csv" => "CSV",
        "tsv" => "TSV",
        "srt" => "SubRip (SRT)",
        "vtt" => "WebVTT",
        "zip" => "ZIP",
        "tar" => "TAR",
        "gz" => "GZIP",
        _ => "Other",
    }
}

/// What a routed pair needs beyond the built-in tools, if it is missing.
pub fn route_needs(input: &str, out: &str, tools: &Tools) -> Option<&'static str> {
    let video = matches!(out, "mp4" | "mov" | "mkv" | "webm" | "avi" | "wmv");
    if input == "gif" && video && tools.ffmpeg.is_none() {
        return Some("ffmpeg (brew install ffmpeg)");
    }
    let doc_in = matches!(input, "docx" | "pages" | "md" | "rtf" | "rtfd" | "textclipping" | "txt" | "epub" | "html");
    let office = (doc_in && matches!(out, "pdf" | "png" | "jpg")) || (input == "pages" && !matches!(out, "pages"));
    if office && tools.soffice.is_none() {
        return Some("LibreOffice (brew install --cask libreoffice)");
    }
    None
}

/// `photo.png` beside `photo.jpg`; `photo 2.png` if that exists, and so on.
pub fn output_path(input: &Path, ext: &str, exists: impl Fn(&Path) -> bool) -> PathBuf {
    let dir = input.parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("converted");
    let mut n = 1;
    loop {
        let name = if n == 1 { format!("{stem}.{ext}") } else { format!("{stem} {n}.{ext}") };
        let p = dir.join(name);
        if !exists(&p) {
            return p;
        }
        n += 1;
    }
}

/// The command to run: program and arguments. `out` must already be chosen with `output_path`.
pub fn plan(input: &Path, target: &Target, out: &Path, tools: &Tools) -> Result<Vec<String>, String> {
    let i = input.to_string_lossy().to_string();
    let o = out.to_string_lossy().to_string();
    let cmd = match (&target.engine, target.ext) {
        (Engine::Pdfkit | Engine::SubtitleText | Engine::SlideImages | Engine::Unrar | Engine::Route, _) => {
            return Err("handled natively".into())
        }
        (Engine::Sips, ext) => {
            let fmt = match ext {
                "jpg" => "jpeg",
                other => other,
            };
            vec!["/usr/bin/sips".into(), "-s".into(), "format".into(), fmt.into(), i, "--out".into(), o]
        }
        (Engine::Afconvert, ext) => {
            let (f, d): (&str, &str) = match ext {
                "m4a" => ("m4af", "aac"),
                "wav" => ("WAVE", "LEI16"),
                "aiff" => ("AIFF", "BEI16"),
                "caf" => ("caff", "LEI16"),
                _ => return Err(format!("afconvert cannot write {ext}")),
            };
            vec!["/usr/bin/afconvert".into(), "-f".into(), f.into(), "-d".into(), d.into(), i, o]
        }
        (Engine::Avconvert, ext) => {
            let preset = match ext {
                "mp4" | "mov" => "PresetHighestQuality",
                "m4a" => "PresetAppleM4A",
                _ => return Err(format!("avconvert cannot write {ext}")),
            };
            vec![
                "/usr/bin/avconvert".into(),
                "--source".into(),
                i,
                "--output".into(),
                o,
                "--preset".into(),
                preset.into(),
                "--replace".into(),
            ]
        }
        (Engine::Textutil, ext) => vec!["/usr/bin/textutil".into(), "-convert".into(), ext.into(), i, "-output".into(), o],
        (Engine::Archive, "zip") => {
            vec!["/usr/bin/ditto".into(), "-c".into(), "-k".into(), "--sequesterRsrc".into(), "--keepParent".into(), i, o]
        }
        (Engine::Archive, "tar.gz") => {
            let parent = input.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| ".".into());
            let name = input.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            vec!["/usr/bin/tar".into(), "-czf".into(), o, "-C".into(), parent, name]
        }
        (Engine::Archive, "tar") => {
            let parent = input.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| ".".into());
            let name = input.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            vec!["/usr/bin/tar".into(), "-cf".into(), o, "-C".into(), parent, name]
        }
        (Engine::Archive, ext) => return Err(format!("no archive format {ext}")),
        (Engine::Cwebp, _) => {
            let cw = tools.cwebp.ok_or("cwebp is not installed")?.to_string();
            vec![cw, "-quiet".into(), "-q".into(), "85".into(), i, "-o".into(), o]
        }
        (Engine::Ffmpeg, ext) => {
            let ff = tools.ffmpeg.ok_or("ffmpeg is not installed")?.to_string();
            let mut v = vec![ff, "-y".into(), "-loglevel".into(), "error".into(), "-i".into(), i];
            // Explicit encoders: Homebrew's ffmpeg has no libvorbis or libwebp, so defaults fail.
            let args: &[&str] = match ext {
                "gif" => &["-vf", "fps=12,scale=640:-1:flags=lanczos"],
                "mp3" => &["-vn", "-c:a", "libmp3lame", "-q:a", "2"],
                "ogg" | "opus" => &["-vn", "-c:a", "libopus", "-b:a", "128k"],
                "flac" => &["-vn", "-c:a", "flac"],
                "webm" => &["-c:v", "libvpx-vp9", "-b:v", "0", "-crf", "33", "-c:a", "libopus"],
                "mkv" => &["-c:v", "libx264", "-crf", "20", "-c:a", "aac"],
                "avi" => &["-c:v", "mpeg4", "-q:v", "4", "-c:a", "libmp3lame"],
                "wmv" => &["-c:v", "wmv2", "-c:a", "wmav2"],
                "mp4" | "mov" => &["-c:v", "libx264", "-pix_fmt", "yuv420p", "-vf", "scale=trunc(iw/2)*2:trunc(ih/2)*2", "-c:a", "aac"],
                "m4a" => &["-vn", "-c:a", "aac", "-b:a", "192k"],
                "wav" => &["-vn", "-c:a", "pcm_s16le"],
                "aiff" => &["-vn", "-c:a", "pcm_s16be"],
                "wma" => &["-vn", "-c:a", "wmav2"],
                _ => &[],
            };
            v.extend(args.iter().map(|a| a.to_string()));
            v.push(o);
            v
        }
        (Engine::Soffice, ext) => {
            let so = tools.soffice.ok_or("LibreOffice is not installed")?.to_string();
            let outdir = out.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| ".".into());
            let filter = match ext {
                "txt" => "txt:Text (encoded):UTF8",
                // comma separated, UTF-8, quote all text cells: independent of the last-used settings
                "csv" => "csv:Text - txt - csv (StarCalc):44,34,76,1",
                // tab-separated, UTF-8; soffice names the file .csv (see soffice_written_path)
                "tsv" => "csv:Text - txt - csv (StarCalc):9,34,76,1",
                other => other,
            };
            let mut v = vec![so, "--headless".into()];
            // PDFs open in Draw by default, which cannot export to text documents
            if self::ext(input) == "pdf" {
                // PPTX comes from the slide importer, text documents from the Writer one
                v.push(if ext == "pptx" { "--infilter=impress_pdf_import" } else { "--infilter=writer_pdf_import" }.into());
            }
            // soffice names the file itself (<stem>.<ext>); the caller renames to `out` afterwards.
            v.extend(["--convert-to".into(), filter.into(), "--outdir".into(), outdir, i]);
            v
        }
    };
    Ok(cmd)
}

/// soffice writes `<stem>.<ext>` into outdir regardless of the name we wanted.
pub fn soffice_written_path(input: &Path, out: &Path, ext: &str) -> PathBuf {
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("converted");
    let written = if ext == "tsv" { "csv" } else { ext.split(':').next().unwrap_or(ext) };
    out.parent().map(Path::to_path_buf).unwrap_or_default().join(format!("{stem}.{written}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none() -> Tools {
        Tools::default()
    }

    #[test]
    fn subtitle_text_drops_timings_numbers_and_markup() {
        let srt = "1\r\n00:00:01,000 --> 00:00:02,000\r\nHello <i>there</i>\r\n\r\n2\r\n00:00:03,000 --> 00:00:04,500\r\n{\\an8}Second line\r\n1984\r\n";
        assert_eq!(subtitle_text(srt), "Hello there\nSecond line\n1984".replace("\n1984", ""));
        let vtt = "WEBVTT\n\nNOTE hidden\n\n00:01.000 --> 00:02.000\n<c.yellow>Hi</c>\n";
        assert_eq!(subtitle_text(vtt), "Hi");
    }

    #[test]
    fn families_from_extensions() {
        assert_eq!(family_of(Path::new("a.HEIC")), Family::Image);
        assert_eq!(family_of(Path::new("a.mkv")), Family::Video);
        assert_eq!(family_of(Path::new("notes.md")), Family::Document);
        assert_eq!(family_of(Path::new("x.unknownext")), Family::Other);
    }

    #[test]
    fn image_targets_exclude_current_and_mark_missing_tools() {
        let v = targets(Path::new("photo.jpeg"), &none());
        assert!(v.iter().all(|t| t.ext != "jpg"));
        let webp = v.iter().find(|t| t.ext == "webp").unwrap();
        assert!(webp.needs.unwrap().contains("webp"));
        assert!(v.iter().find(|t| t.ext == "png").unwrap().needs.is_none());
        let with = Tools { ffmpeg: None, soffice: None, cwebp: Some("/opt/homebrew/bin/cwebp"), ..Default::default() };
        assert!(targets(Path::new("photo.jpg"), &with).iter().find(|t| t.ext == "webp").unwrap().needs.is_none());
    }

    #[test]
    fn output_names_never_overwrite() {
        let taken = ["/d/photo.png", "/d/photo 2.png"];
        let p = output_path(Path::new("/d/photo.jpg"), "png", |p| taken.contains(&p.to_str().unwrap()));
        assert_eq!(p, PathBuf::from("/d/photo 3.png"));
        assert_eq!(output_path(Path::new("/d/a.mov"), "mp4", |_| false), PathBuf::from("/d/a.mp4"));
    }

    #[test]
    fn plans_use_the_right_tool() {
        let tools = Tools { ffmpeg: Some("/ff"), soffice: Some("/so"), cwebp: Some("/cw"), ..Default::default() };
        let png = targets(Path::new("/d/p.jpg"), &tools).into_iter().find(|t| t.ext == "png").unwrap();
        assert_eq!(
            plan(Path::new("/d/p.jpg"), &png, Path::new("/d/p.png"), &tools).unwrap()[..4],
            ["/usr/bin/sips", "-s", "format", "png"]
        );
        let mp3 = targets(Path::new("/d/s.wav"), &tools).into_iter().find(|t| t.ext == "mp3").unwrap();
        let c = plan(Path::new("/d/s.wav"), &mp3, Path::new("/d/s.mp3"), &tools).unwrap();
        assert_eq!(c[0], "/ff");
        assert!(c.contains(&"libmp3lame".to_string()));
        let pdf = targets(Path::new("/d/r.docx"), &tools).into_iter().find(|t| t.ext == "pdf").unwrap();
        let c = plan(Path::new("/d/r.docx"), &pdf, Path::new("/d/r.pdf"), &tools).unwrap();
        assert_eq!(c[0], "/so");
        assert_eq!(soffice_written_path(Path::new("/d/r.docx"), Path::new("/d/r 2.pdf"), "pdf"), PathBuf::from("/d/r.pdf"));
        let zip = targets(Path::new("/d/folder"), &tools).into_iter().find(|t| t.ext == "zip").unwrap();
        assert_eq!(plan(Path::new("/d/folder"), &zip, Path::new("/d/folder.zip"), &tools).unwrap()[0], "/usr/bin/ditto");
        let missing = Tools::default();
        let webp = targets(Path::new("/d/p.jpg"), &missing).into_iter().find(|t| t.ext == "webp").unwrap();
        assert!(plan(Path::new("/d/p.jpg"), &webp, Path::new("/d/p.webp"), &missing).is_err());
    }
}

// ---------------------------------------------------------------- image tools

/// One-shot image tools offered next to conversions.
#[derive(Debug, Clone, PartialEq)]
pub enum ImageOp {
    /// Re-encode as JPEG at this quality (1–100).
    Compress(u8),
    /// Fit inside a square of this many pixels, keeping the aspect ratio and format.
    Resize(u32),
    /// Centre crop to this aspect ratio (width, height).
    Crop(u32, u32),
    /// Re-encode without EXIF, GPS or other metadata.
    StripMetadata,
    /// Every selected image as one page of a single PDF.
    CombinePdf,
    /// Rotate clockwise by 90, 180 or 270 degrees.
    Rotate(u16),
    /// Mirror horizontally (true) or vertically (false).
    Flip(bool),
    /// Pad to a square on a solid background (RGB hex, e.g. "FFFFFF").
    Background(&'static str),
    /// Every selected image in one grid picture.
    Collage,
    /// Cover detected faces (true) or text (false) with solid boxes.
    Redact(bool),
    /// One-step photo adjustment.
    Adjust(Adjust),
    /// Copy the image and open the copy in Preview for Markup.
    Annotate,
    /// Re-encode as JPEG, at the highest quality that fits this many KB.
    CompressToKb(u32),
    /// Re-encode as JPEG so the file is this percent smaller than the original.
    CompressByPercent(u8),
    /// Centre crop to exactly this many pixels (clamped to the image).
    CropExact(u32, u32),
    /// Write camera and/or location tags into a JPEG's EXIF.
    SetMeta(MetaEdit),
}

/// Camera and location tags to write. Empty fields are left out.
#[derive(Debug, Clone, PartialEq)]
pub struct MetaEdit {
    pub make: String,
    pub model: String,
    /// Latitude, longitude in decimal degrees.
    pub gps: Option<(f64, f64)>,
}

/// Photo adjustments offered as one-step edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Adjust {
    BlackAndWhite,
    Brighter,
    Darker,
    MoreContrast,
}

impl ImageOp {
    pub fn label(&self) -> String {
        match self {
            ImageOp::Compress(q) => format!("Compress · JPEG {q}%"),
            ImageOp::Resize(px) => format!("Resize · fit {px} px"),
            ImageOp::Crop(w, h) => format!("Crop · {w}:{h}"),
            ImageOp::StripMetadata => "Remove metadata".into(),
            ImageOp::CombinePdf => "Combine into one PDF".into(),
            ImageOp::Rotate(d) => format!("Rotate · {d}°"),
            ImageOp::Flip(true) => "Flip horizontally".into(),
            ImageOp::Flip(false) => "Flip vertically".into(),
            ImageOp::Background(c) => format!("Square on {} background", if *c == "FFFFFF" { "white" } else { "black" }),
            ImageOp::Collage => "Make a collage".into(),
            ImageOp::Redact(true) => "Redact faces".into(),
            ImageOp::Redact(false) => "Redact text".into(),
            ImageOp::Adjust(a) => match a {
                Adjust::BlackAndWhite => "Edit · Black & white",
                Adjust::Brighter => "Edit · Brighter",
                Adjust::Darker => "Edit · Darker",
                Adjust::MoreContrast => "Edit · More contrast",
            }
            .into(),
            ImageOp::Annotate => "Annotate in Preview".into(),
            ImageOp::CompressToKb(kb) => format!("Compress · under {}", format_kb(*kb)),
            ImageOp::CompressByPercent(p) => format!("Compress · {p}% smaller"),
            ImageOp::CropExact(w, h) => format!("Crop · exactly {w}×{h} px"),
            ImageOp::SetMeta(m) => {
                let mut parts = vec![];
                if !m.make.is_empty() {
                    parts.push(format!("make {}", m.make));
                }
                if !m.model.is_empty() {
                    parts.push(format!("model {}", m.model));
                }
                if let Some((la, lo)) = m.gps {
                    parts.push(format!("GPS {la:.4}, {lo:.4}"));
                }
                format!("Set metadata · {}", parts.join(", "))
            }
        }
    }
    /// Short word added to the output file name.
    pub fn suffix(&self) -> String {
        match self {
            ImageOp::Compress(_) => "compressed".into(),
            ImageOp::Resize(px) => format!("{px}px"),
            ImageOp::Crop(w, h) => format!("{w}x{h}"),
            ImageOp::StripMetadata => "clean".into(),
            ImageOp::CombinePdf => "combined".into(),
            ImageOp::Rotate(d) => format!("rotated {d}"),
            ImageOp::Flip(_) => "flipped".into(),
            ImageOp::Background(_) => "square".into(),
            ImageOp::Collage => "collage".into(),
            ImageOp::Redact(_) => "redacted".into(),
            ImageOp::Adjust(_) => "edited".into(),
            ImageOp::Annotate => "annotated".into(),
            ImageOp::CompressToKb(_) | ImageOp::CompressByPercent(_) => "compressed".into(),
            ImageOp::CropExact(w, h) => format!("{w}x{h}"),
            ImageOp::SetMeta(_) => "tagged".into(),
        }
    }
    /// Tools for `n` selected images.
    pub fn offered(n: usize) -> Vec<ImageOp> {
        let mut v = vec![
            ImageOp::Compress(80),
            ImageOp::Compress(60),
            ImageOp::Compress(40),
            ImageOp::Resize(2048),
            ImageOp::Resize(1280),
            ImageOp::Resize(640),
            ImageOp::Crop(1, 1),
            ImageOp::Crop(16, 9),
            ImageOp::Crop(4, 3),
            ImageOp::Crop(9, 16),
            ImageOp::StripMetadata,
            ImageOp::Rotate(90),
            ImageOp::Rotate(180),
            ImageOp::Rotate(270),
            ImageOp::Flip(true),
            ImageOp::Flip(false),
            ImageOp::Background("FFFFFF"),
            ImageOp::Background("000000"),
            ImageOp::Redact(true),
            ImageOp::Redact(false),
            ImageOp::Adjust(Adjust::BlackAndWhite),
            ImageOp::Adjust(Adjust::Brighter),
            ImageOp::Adjust(Adjust::Darker),
            ImageOp::Adjust(Adjust::MoreContrast),
            ImageOp::Annotate,
        ];
        if n > 1 {
            v.insert(0, ImageOp::Collage);
            v.insert(0, ImageOp::CombinePdf);
        }
        v
    }
}

/// `photo compressed.jpg`, `photo compressed 2.jpg`… next to the input.
pub fn output_with_suffix(input: &Path, suffix: &str, ext: &str, exists: impl Fn(&Path) -> bool) -> PathBuf {
    let dir = input.parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("image");
    let mut n = 1;
    loop {
        let name = if n == 1 { format!("{stem} {suffix}.{ext}") } else { format!("{stem} {suffix} {n}.{ext}") };
        let p = dir.join(name);
        if !exists(&p) {
            return p;
        }
        n += 1;
    }
}

fn format_kb(kb: u32) -> String {
    if kb >= 1024 && kb.is_multiple_of(1024) {
        format!("{} MB", kb / 1024)
    } else {
        format!("{kb} KB")
    }
}

/// Image tool typed in the search box: `500kb`, `1.5mb`, `40%` (smaller) or `800x600`.
pub fn parse_image_query(q: &str) -> Option<ImageOp> {
    // `make Canon`, `model EOS R5`, `gps 40.4168,-3.7038`: keep the original case and spaces
    let raw = q.trim();
    let lower = raw.to_lowercase();
    for key in ["make", "model", "gps"] {
        if let Some(rest) = lower.strip_prefix(key).filter(|r| r.starts_with(' ')) {
            let value = raw[key.len()..].trim();
            if rest.trim().is_empty() {
                return None;
            }
            return match key {
                "make" => Some(ImageOp::SetMeta(MetaEdit { make: value.into(), model: String::new(), gps: None })),
                "model" => Some(ImageOp::SetMeta(MetaEdit { make: String::new(), model: value.into(), gps: None })),
                _ => {
                    let (la, lo) = value.split_once(',')?;
                    let (la, lo): (f64, f64) = (la.trim().parse().ok()?, lo.trim().parse().ok()?);
                    ((-90.0..=90.0).contains(&la) && (-180.0..=180.0).contains(&lo))
                        .then(|| ImageOp::SetMeta(MetaEdit { make: String::new(), model: String::new(), gps: Some((la, lo)) }))
                }
            };
        }
    }
    let q = lower.replace(' ', "");
    if let Some(n) = q.strip_suffix('%') {
        let p: u8 = n.parse().ok()?;
        return (1..=95).contains(&p).then_some(ImageOp::CompressByPercent(p));
    }
    for (suffix, mult) in [("kb", 1.0), ("mb", 1024.0)] {
        if let Some(n) = q.strip_suffix(suffix) {
            let v: f64 = n.parse().ok()?;
            let kb = (v * mult).round();
            return (1.0..=1_000_000.0).contains(&kb).then_some(ImageOp::CompressToKb(kb as u32));
        }
    }
    let (w, h) = q.split_once('x').or_else(|| q.split_once('×'))?;
    let (w, h): (u32, u32) = (w.parse().ok()?, h.parse().ok()?);
    ((1..=20_000).contains(&w) && (1..=20_000).contains(&h)).then_some(ImageOp::CropExact(w, h))
}

/// Highest JPEG quality (5..=95) whose encoded size is within `target` bytes. `encode(q)` returns
/// the size at quality `q`. Falls back to 5 when even that is too big.
pub fn fit_quality(target: u64, mut encode: impl FnMut(u8) -> Option<u64>) -> u8 {
    let (mut lo, mut hi, mut best) = (5u8, 95u8, 5u8);
    while lo <= hi {
        let mid = lo + (hi - lo) / 2;
        match encode(mid) {
            Some(sz) if sz <= target => {
                best = mid;
                lo = mid + 1;
            }
            _ => {
                if mid == 0 {
                    break;
                }
                hi = mid - 1;
            }
        }
    }
    best
}

/// Grid for a collage of `n` pictures: (columns, rows), as square as possible.
pub fn collage_grid(n: usize) -> (usize, usize) {
    if n == 0 {
        return (0, 0);
    }
    let cols = (n as f64).sqrt().ceil() as usize;
    (cols, n.div_ceil(cols))
}

/// Pixel rectangle (x, y, w, h) for a normalised top-left box, grown by `pad` of its size
/// on each side and clamped to the image.
pub fn redact_rect(x: f64, y: f64, w: f64, h: f64, iw: u32, ih: u32, pad: f64) -> (u32, u32, u32, u32) {
    let (fw, fh) = (iw as f64, ih as f64);
    // the epsilon keeps float noise (0.55 * 100 = 55.000000000000007) from adding a pixel
    const E: f64 = 1e-9;
    let x0 = ((x - w * pad) * fw + E).floor().clamp(0.0, fw);
    let y0 = ((y - h * pad) * fh + E).floor().clamp(0.0, fh);
    let x1 = ((x + w * (1.0 + pad)) * fw - E).ceil().clamp(0.0, fw);
    let y1 = ((y + h * (1.0 + pad)) * fh - E).ceil().clamp(0.0, fh);
    (x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32)
}

/// Largest centred `rw:rh` rectangle inside `w × h`.
pub fn crop_size(w: u32, h: u32, rw: u32, rh: u32) -> (u32, u32) {
    if rw == 0 || rh == 0 || w == 0 || h == 0 {
        return (w, h);
    }
    // width-limited or height-limited
    let by_w = (w, (w as u64 * rh as u64 / rw as u64) as u32);
    if by_w.1 <= h {
        by_w
    } else {
        ((h as u64 * rw as u64 / rh as u64) as u32, h)
    }
}

#[cfg(test)]
mod image_tool_tests {
    use super::*;

    #[test]
    fn metadata_queries() {
        let m = |make: &str, model: &str, gps| Some(ImageOp::SetMeta(MetaEdit { make: make.into(), model: model.into(), gps }));
        assert_eq!(parse_image_query("make Canon"), m("Canon", "", None));
        assert_eq!(parse_image_query("model EOS R5"), m("", "EOS R5", None));
        assert_eq!(parse_image_query("gps 40.4168, -3.7038"), m("", "", Some((40.4168, -3.7038))));
        for bad in ["make", "make ", "gps 91,0", "gps 1", "gps a,b", "makeup"] {
            assert_eq!(parse_image_query(bad), None, "{bad}");
        }
    }

    #[test]
    fn image_queries() {
        assert_eq!(parse_image_query("500kb"), Some(ImageOp::CompressToKb(500)));
        assert_eq!(parse_image_query("1.5 MB"), Some(ImageOp::CompressToKb(1536)));
        assert_eq!(parse_image_query("40%"), Some(ImageOp::CompressByPercent(40)));
        assert_eq!(parse_image_query("800x600"), Some(ImageOp::CropExact(800, 600)));
        assert_eq!(parse_image_query("800×600"), Some(ImageOp::CropExact(800, 600)));
        for bad in ["", "kb", "0%", "100%", "0x5", "abc", "10", "5mbx"] {
            assert_eq!(parse_image_query(bad), None, "{bad}");
        }
    }

    #[test]
    fn quality_search_finds_the_highest_fit() {
        // size grows linearly with quality: 1000 bytes per point
        let mut calls = 0;
        let q = fit_quality(50_500, |q| {
            calls += 1;
            Some(q as u64 * 1000)
        });
        assert_eq!(q, 50);
        assert!(calls <= 8, "binary search, not a scan: {calls}");
        assert_eq!(fit_quality(10, |q| Some(q as u64 * 1000)), 5, "floor when nothing fits");
        assert_eq!(fit_quality(u64::MAX, |q| Some(q as u64)), 95);
    }

    #[test]
    fn redact_rects() {
        assert_eq!(redact_rect(0.25, 0.5, 0.5, 0.25, 100, 100, 0.0), (25, 50, 50, 25));
        // padding grows the box and clamps at the edges
        assert_eq!(redact_rect(0.0, 0.0, 0.5, 0.5, 100, 100, 0.1), (0, 0, 55, 55));
        assert_eq!(redact_rect(0.9, 0.9, 0.2, 0.2, 100, 100, 0.0), (90, 90, 10, 10));
    }

    #[test]
    fn crop_sizes() {
        assert_eq!(crop_size(4000, 3000, 1, 1), (3000, 3000));
        assert_eq!(crop_size(4000, 3000, 16, 9), (4000, 2250));
        assert_eq!(crop_size(3000, 4000, 16, 9), (3000, 1687));
        assert_eq!(crop_size(1000, 1000, 9, 16), (562, 1000));
    }

    #[test]
    fn names_and_offers() {
        let p = output_with_suffix(Path::new("/d/photo.png"), "compressed", "jpg", |p| p == Path::new("/d/photo compressed.jpg"));
        assert_eq!(p, PathBuf::from("/d/photo compressed 2.jpg"));
        assert!(!ImageOp::offered(1).contains(&ImageOp::CombinePdf));
        assert_eq!(ImageOp::offered(3)[0], ImageOp::CombinePdf);
        assert!(ImageOp::offered(3).contains(&ImageOp::Collage));
        assert!(!ImageOp::offered(1).contains(&ImageOp::Collage));
        assert_eq!(
            (collage_grid(1), collage_grid(2), collage_grid(4), collage_grid(5), collage_grid(7)),
            ((1, 1), (2, 1), (2, 2), (3, 2), (3, 3))
        );
    }
}

/// JPEG without metadata, losslessly: drops APP1–APP15 (EXIF, XMP, IPTC, ICC…) and comments,
/// keeps APP0 (JFIF) and the image data untouched. `None` if the bytes are not a JPEG.
pub fn strip_jpeg(b: &[u8]) -> Option<Vec<u8>> {
    if b.len() < 4 || b[0] != 0xFF || b[1] != 0xD8 {
        return None;
    }
    let mut out = vec![0xFF, 0xD8];
    let mut i = 2;
    while i + 4 <= b.len() {
        if b[i] != 0xFF {
            return None;
        }
        let m = b[i + 1];
        if m == 0xDA {
            // start of scan: the rest is image data, copy as is
            out.extend_from_slice(&b[i..]);
            return Some(out);
        }
        let len = u16::from_be_bytes([b[i + 2], b[i + 3]]) as usize;
        let end = i + 2 + len;
        if end > b.len() {
            return None;
        }
        let drop = (0xE1..=0xEF).contains(&m) || m == 0xFE;
        if !drop {
            out.extend_from_slice(&b[i..end]);
        }
        i = end;
    }
    None
}

// ------------------------------------------------------ trim, split, merge

/// Tools for audio, video and PDF files.
#[derive(Debug, Clone, PartialEq)]
pub enum FileTool {
    /// Keep from `start` to `end` seconds.
    Trim(f64, f64),
    /// Cut into pieces of this many seconds.
    SplitEvery(u32),
    /// Join the selected files, in order, into one.
    Merge,
    /// One PDF per page.
    SplitPages,
    /// Drop the audio track.
    Mute,
    /// Play at this factor (0.25–4), pitch preserved.
    Speed(f64),
    /// One full-resolution PNG at this second.
    Snapshot(f64),
    /// Crop video to w×h at x,y.
    CropVideo(u32, u32, u32, u32),
    /// Cover a region (x,y,w,h) from..to seconds; blur instead of a box.
    RedactVideo(u32, u32, u32, u32, f64, f64, bool),
    /// EBU R128 loudness normalisation (I, LRA, TP).
    Normalize(f64, f64, f64),
    /// Audio → MP4 with a waveform (landscape/portrait/square).
    Visualize(crate::media::Canvas),
    /// Strip silence from both ends of a recording.
    TrimSilence,
    /// Mono (false) or stereo (true) with per-channel gain.
    Channels(bool, f64, f64),
    /// Replace from..to seconds with a 1 kHz bleep.
    Bleep(f64, f64),
    /// Join mixed MP4/MOV/MKV clips into one MP4 (first clip is the canvas).
    Join,
    /// New PDF from these pages in order: (0-based source page, quarter turns clockwise).
    OrganizePdf(Vec<(usize, i32)>),
}

pub fn format_secs(s: f64) -> String {
    let t = s.round() as u64;
    if t >= 3600 {
        format!("{}:{:02}:{:02}", t / 3600, t / 60 % 60, t % 60)
    } else {
        format!("{}:{:02}", t / 60, t % 60)
    }
}

impl FileTool {
    pub fn label(&self) -> String {
        match self {
            FileTool::Trim(a, b) => format!("Trim · {} to {}", format_secs(*a), format_secs(*b)),
            FileTool::SplitEvery(n) if n % 60 == 0 => format!("Split · every {} min", n / 60),
            FileTool::SplitEvery(n) => format!("Split · every {n} s"),
            FileTool::Merge => "Merge into one file".into(),
            FileTool::SplitPages => "Split into one PDF per page".into(),
            FileTool::Mute => "Remove audio".into(),
            FileTool::Speed(f) => format!("Speed · {f}×"),
            FileTool::Snapshot(t) => format!("Save frame at {}", format_secs(*t)),
            FileTool::CropVideo(_, _, w, h) => format!("Crop video · {w}×{h}"),
            FileTool::RedactVideo(_, _, _, _, a, b, blur) => {
                format!("Redact video · {} to {}{}", format_secs(*a), format_secs(*b), if *blur { " (blur)" } else { "" })
            }
            FileTool::Normalize(i, ..) => format!("Normalize volume · {i} LUFS"),
            FileTool::Visualize(c) => format!(
                "Audio visualizer · {}",
                match c {
                    crate::media::Canvas::Landscape => "landscape",
                    crate::media::Canvas::Portrait => "portrait",
                    crate::media::Canvas::Square => "square",
                }
            ),
            FileTool::TrimSilence => "Trim silence at both ends".into(),
            FileTool::Channels(true, ..) => "Convert to stereo".into(),
            FileTool::Channels(false, ..) => "Convert to mono".into(),
            FileTool::Bleep(a, b) => format!("Bleep · {} to {}", format_secs(*a), format_secs(*b)),
            FileTool::Join => "Join videos into one MP4".into(),
            FileTool::OrganizePdf(p) => format!("Organize PDF · {} page{}", p.len(), if p.len() == 1 { "" } else { "s" }),
        }
    }
    pub fn suffix(&self) -> String {
        match self {
            FileTool::Trim(..) => "trimmed".into(),
            FileTool::SplitEvery(_) | FileTool::SplitPages => "part".into(),
            FileTool::Merge => "merged".into(),
            FileTool::Mute => "muted".into(),
            FileTool::Speed(f) => format!("speed {f}x"),
            FileTool::Snapshot(_) => "frame".into(),
            FileTool::CropVideo(..) => "cropped".into(),
            FileTool::RedactVideo(..) => "redacted".into(),
            FileTool::Normalize(..) => "normalized".into(),
            FileTool::Visualize(_) => "visualizer".into(),
            FileTool::TrimSilence => "trimmed".into(),
            FileTool::Channels(true, ..) => "stereo".into(),
            FileTool::Channels(false, ..) => "mono".into(),
            FileTool::Bleep(..) => "bleeped".into(),
            FileTool::Join => "joined".into(),
            FileTool::OrganizePdf(_) => "organized".into(),
        }
    }
    /// Tools offered for `n` selected files of one family.
    pub fn offered(family: Family, n: usize) -> Vec<FileTool> {
        match family {
            Family::Video if n > 1 => vec![FileTool::Join, FileTool::Merge],
            Family::Audio | Family::Pdf if n > 1 => vec![FileTool::Merge],
            Family::Pdf => vec![FileTool::SplitPages],
            Family::Video => vec![
                FileTool::Mute,
                FileTool::Speed(2.0),
                FileTool::Speed(0.5),
                FileTool::Snapshot(0.0),
                FileTool::SplitEvery(60),
                FileTool::SplitEvery(300),
            ],
            Family::Audio => vec![
                FileTool::Normalize(-16.0, 11.0, -1.5),
                FileTool::TrimSilence,
                FileTool::Channels(false, 1.0, 1.0),
                FileTool::Channels(true, 1.0, 1.0),
                FileTool::Visualize(crate::media::Canvas::Landscape),
                FileTool::Visualize(crate::media::Canvas::Square),
                FileTool::SplitEvery(60),
                FileTool::SplitEvery(300),
            ],
            _ => vec![],
        }
    }
}

/// `90`, `1:30`, `1:02:03` → seconds.
pub fn parse_time(s: &str) -> Option<f64> {
    let parts: Vec<&str> = s.trim().split(':').collect();
    if parts.is_empty() || parts.len() > 3 {
        return None;
    }
    let mut total = 0.0;
    for p in &parts {
        let v: f64 = p.trim().parse().ok().filter(|v: &f64| *v >= 0.0)?;
        total = total * 60.0 + v;
    }
    Some(total)
}

/// Typed tool: `trim 0:10-0:30`, `split 30s`, `speed 1.5x`, `frame 1:23`, `crop 640x360`,
/// `crop 640x360 at 100,50`, `redact 10,10 200x100 0:05-0:09`, `redact … blur`, `bleep 0:05-0:07`,
/// `normalize -14`, `mono`, `stereo`, `mute`, `visualizer portrait`.
pub fn parse_tool_query(q: &str) -> Option<FileTool> {
    let q = q.trim().to_lowercase();
    let range = |s: &str| -> Option<(f64, f64)> {
        let (a, b) = s.split_once('-')?;
        let (a, b) = (parse_time(a)?, parse_time(b)?);
        (b > a).then_some((a, b))
    };
    if let Some(rest) = q.strip_prefix("trim ") {
        let (a, b) = range(rest)?;
        return Some(FileTool::Trim(a, b));
    }
    if let Some(rest) = q.strip_prefix("pages ") {
        // "3,1,2r,1": 1-based pages in the wanted order; r = 90° right, l = 90° left, rr = 180°
        let mut sel = vec![];
        for part in rest.split(',') {
            let part = part.trim();
            let digits = part.trim_end_matches(['r', 'l']);
            let turns = part[digits.len()..].chars().fold(0i32, |t, c| if c == 'r' { t + 1 } else { t - 1 });
            let n: usize = digits.parse().ok().filter(|n| *n >= 1)?;
            sel.push((n - 1, turns.rem_euclid(4)));
        }
        return (!sel.is_empty()).then_some(FileTool::OrganizePdf(sel));
    }
    if q == "mute" || q == "remove audio" {
        return Some(FileTool::Mute);
    }
    if q == "mono" {
        return Some(FileTool::Channels(false, 1.0, 1.0));
    }
    if q == "stereo" {
        return Some(FileTool::Channels(true, 1.0, 1.0));
    }
    if q == "silence" || q == "trim silence" {
        return Some(FileTool::TrimSilence);
    }
    if let Some(rest) = q.strip_prefix("speed ") {
        let f: f64 = rest.trim().trim_end_matches('x').trim().parse().ok()?;
        return (0.25..=4.0).contains(&f).then_some(FileTool::Speed(f));
    }
    if let Some(rest) = q.strip_prefix("frame ").or_else(|| q.strip_prefix("snapshot ")) {
        return Some(FileTool::Snapshot(parse_time(rest)?));
    }
    if let Some(rest) = q.strip_prefix("bleep ") {
        let (a, b) = range(rest)?;
        return Some(FileTool::Bleep(a, b));
    }
    if let Some(rest) = q.strip_prefix("normalize") {
        let i: f64 = rest.trim().parse().unwrap_or(-16.0);
        return (-40.0..=0.0).contains(&i).then_some(FileTool::Normalize(i, 11.0, -1.5));
    }
    if let Some(rest) = q.strip_prefix("visualizer") {
        let c = match rest.trim() {
            "portrait" => crate::media::Canvas::Portrait,
            "square" => crate::media::Canvas::Square,
            _ => crate::media::Canvas::Landscape,
        };
        return Some(FileTool::Visualize(c));
    }
    if let Some(rest) = q.strip_prefix("crop ") {
        // "640x360" or "640x360 at 100,50"
        let (size, at) = rest.split_once(" at ").map(|(s, a)| (s, Some(a))).unwrap_or((rest, None));
        let (w, h) = size.trim().split_once('x')?;
        let (w, h): (u32, u32) = (w.trim().parse().ok()?, h.trim().parse().ok()?);
        let (x, y) = match at {
            Some(a) => {
                let (x, y) = a.split_once(',')?;
                (x.trim().parse().ok()?, y.trim().parse().ok()?)
            }
            None => (0, 0),
        };
        return (w > 0 && h > 0).then_some(FileTool::CropVideo(x, y, w, h));
    }
    if let Some(rest) = q.strip_prefix("redact ") {
        // "x,y wxh a-b [blur]"
        let blur = rest.ends_with(" blur");
        let rest = rest.trim_end_matches(" blur");
        let mut it = rest.split_whitespace();
        let (x, y) = it.next()?.split_once(',')?;
        let (w, h) = it.next()?.split_once('x')?;
        let (a, b) = range(it.next()?)?;
        return Some(FileTool::RedactVideo(x.parse().ok()?, y.parse().ok()?, w.parse().ok()?, h.parse().ok()?, a, b, blur));
    }
    let rest = q.strip_prefix("split ")?.replace(' ', "");
    let (n, mult) = match rest.chars().last()? {
        's' => (&rest[..rest.len() - 1], 1),
        'm' => (&rest[..rest.len() - 1], 60),
        _ => (rest.as_str(), 1),
    };
    let v: u32 = n.parse().ok()?;
    (1..=86_400).contains(&v).then_some(FileTool::SplitEvery(v * mult))
}

struct ExifEntry {
    tag: u16,
    typ: u16,
    count: u32,
    data: Vec<u8>,
}

fn ascii(tag: u16, s: &str) -> ExifEntry {
    let mut data = s.as_bytes().to_vec();
    data.push(0);
    ExifEntry { tag, typ: 2, count: data.len() as u32, data }
}

fn rationals(tag: u16, vals: &[(u32, u32)]) -> ExifEntry {
    let data = vals.iter().flat_map(|(n, d)| n.to_le_bytes().into_iter().chain(d.to_le_bytes())).collect();
    ExifEntry { tag, typ: 5, count: vals.len() as u32, data }
}

/// Bytes an IFD with these entries takes, including its out-of-line data.
fn ifd_size(entries: &[ExifEntry]) -> usize {
    2 + entries.len() * 12 + 4 + entries.iter().filter(|e| e.data.len() > 4).map(|e| e.data.len() + e.data.len() % 2).sum::<usize>()
}

/// Little-endian IFD starting at TIFF offset `base`.
fn write_ifd(mut entries: Vec<ExifEntry>, base: u32) -> Vec<u8> {
    entries.sort_by_key(|e| e.tag);
    let mut out = (entries.len() as u16).to_le_bytes().to_vec();
    let mut extra = vec![];
    let data_start = base + 2 + entries.len() as u32 * 12 + 4;
    for e in &entries {
        out.extend(e.tag.to_le_bytes());
        out.extend(e.typ.to_le_bytes());
        out.extend(e.count.to_le_bytes());
        if e.data.len() <= 4 {
            let mut v = e.data.clone();
            v.resize(4, 0);
            out.extend(v);
        } else {
            out.extend((data_start + extra.len() as u32).to_le_bytes());
            extra.extend(&e.data);
            if e.data.len() % 2 == 1 {
                extra.push(0);
            }
        }
    }
    out.extend(0u32.to_le_bytes());
    out.extend(extra);
    out
}

/// Degrees, minutes, seconds as EXIF rationals (seconds in 1/10000).
fn dms(v: f64) -> [(u32, u32); 3] {
    let v = v.abs();
    let d = v.floor();
    let m = ((v - d) * 60.0).floor();
    let s = ((v - d) * 60.0 - m) * 60.0;
    [(d as u32, 1), (m as u32, 1), ((s * 10000.0).round() as u32, 10000)]
}

/// An APP1 EXIF segment (with marker) holding the requested tags.
pub fn build_exif_segment(m: &MetaEdit) -> Vec<u8> {
    let mut ifd0 = vec![];
    if !m.make.is_empty() {
        ifd0.push(ascii(0x010F, &m.make));
    }
    if !m.model.is_empty() {
        ifd0.push(ascii(0x0110, &m.model));
    }
    let mut gps = vec![];
    if let Some((la, lo)) = m.gps {
        gps.push(ExifEntry { tag: 0, typ: 1, count: 4, data: vec![2, 3, 0, 0] });
        gps.push(ascii(1, if la >= 0.0 { "N" } else { "S" }));
        gps.push(rationals(2, &dms(la)));
        gps.push(ascii(3, if lo >= 0.0 { "E" } else { "W" }));
        gps.push(rationals(4, &dms(lo)));
        ifd0.push(ExifEntry { tag: 0x8825, typ: 4, count: 1, data: vec![0; 4] });
    }
    let gps_base = (8 + ifd_size(&ifd0)) as u32;
    if let Some(p) = ifd0.iter_mut().find(|e| e.tag == 0x8825) {
        p.data = gps_base.to_le_bytes().to_vec();
    }
    let mut tiff = vec![b'I', b'I', 42, 0, 8, 0, 0, 0];
    tiff.extend(write_ifd(ifd0, 8));
    if !gps.is_empty() {
        tiff.extend(write_ifd(gps, gps_base));
    }
    let mut seg = vec![0xFF, 0xE1];
    seg.extend(((tiff.len() + 8) as u16).to_be_bytes());
    seg.extend(b"Exif\0\0");
    seg.extend(tiff);
    seg
}

/// JPEG with its EXIF replaced by `m`'s tags (other metadata segments are dropped).
/// `None` if the bytes are not a JPEG.
pub fn set_jpeg_meta(b: &[u8], m: &MetaEdit) -> Option<Vec<u8>> {
    let clean = strip_jpeg(b)?;
    let mut out = clean[..2].to_vec();
    // EXIF goes right after SOI; a JFIF APP0 may follow it
    out.extend(build_exif_segment(m));
    out.extend(&clean[2..]);
    Some(out)
}

/// PNG without text or EXIF chunks (eXIf, tEXt, iTXt, zTXt, tIME). `None` if not a PNG.
pub fn strip_png(b: &[u8]) -> Option<Vec<u8>> {
    const SIG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    if b.len() < 8 || b[..8] != SIG {
        return None;
    }
    let mut out = SIG.to_vec();
    let mut i = 8;
    while i + 12 <= b.len() {
        let len = u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]) as usize;
        let end = i + 12 + len;
        if end > b.len() {
            return None;
        }
        let kind = &b[i + 4..i + 8];
        if !matches!(kind, b"eXIf" | b"tEXt" | b"iTXt" | b"zTXt" | b"tIME") {
            out.extend_from_slice(&b[i..end]);
        }
        i = end;
    }
    Some(out)
}

#[cfg(test)]
mod strip_tests {
    use super::*;

    #[test]
    fn jpeg_drops_app1_and_comments_keeps_rest() {
        // SOI, APP0 JFIF, APP1 "Exif", COM, DQT, SOS + data, EOI
        let mut j = vec![0xFF, 0xD8];
        j.extend([0xFF, 0xE0, 0x00, 0x06, b'J', b'F', b'I', b'F']);
        j.extend([0xFF, 0xE1, 0x00, 0x08, b'E', b'x', b'i', b'f', 0, 0]);
        j.extend([0xFF, 0xFE, 0x00, 0x05, b'h', b'i', b'!']);
        j.extend([0xFF, 0xDB, 0x00, 0x04, 1, 2]);
        j.extend([0xFF, 0xDA, 0x00, 0x02, 9, 9, 9, 0xFF, 0xD9]);
        let s = strip_jpeg(&j).unwrap();
        assert!(!s.windows(4).any(|w| w == b"Exif"));
        assert!(!s.windows(3).any(|w| w == b"hi!"));
        assert!(s.windows(4).any(|w| w == b"JFIF"));
        assert!(s.ends_with(&[0xFF, 0xDA, 0x00, 0x02, 9, 9, 9, 0xFF, 0xD9]));
        assert_eq!(strip_jpeg(b"not a jpeg"), None);
    }

    #[test]
    fn file_tool_queries() {
        assert_eq!(parse_time("90"), Some(90.0));
        assert_eq!(parse_time("1:30"), Some(90.0));
        assert_eq!(parse_time("1:02:03"), Some(3723.0));
        assert_eq!(parse_time("a"), None);
        assert_eq!(parse_tool_query("trim 0:10-0:30"), Some(FileTool::Trim(10.0, 30.0)));
        assert_eq!(parse_tool_query("trim 30-10"), None, "end before start");
        assert_eq!(parse_tool_query("split 30s"), Some(FileTool::SplitEvery(30)));
        assert_eq!(parse_tool_query("split 2m"), Some(FileTool::SplitEvery(120)));
        assert_eq!(parse_tool_query("split 0"), None);
        assert_eq!(parse_tool_query("speed 2x"), Some(FileTool::Speed(2.0)));
        assert_eq!(parse_tool_query("speed 9"), None);
        assert_eq!(parse_tool_query("mute"), Some(FileTool::Mute));
        assert_eq!(parse_tool_query("pages 3,1,2r,1ll"), Some(FileTool::OrganizePdf(vec![(2, 0), (0, 0), (1, 1), (0, 2)])));
        assert_eq!(parse_tool_query("pages 0"), None);
        assert_eq!(parse_tool_query("pages 1,x"), None);
        assert_eq!(parse_tool_query("frame 1:30"), Some(FileTool::Snapshot(90.0)));
        assert_eq!(parse_tool_query("crop 640x360 at 10,20"), Some(FileTool::CropVideo(10, 20, 640, 360)));
        assert_eq!(parse_tool_query("redact 10,10 200x100 0:05-0:09 blur"), Some(FileTool::RedactVideo(10, 10, 200, 100, 5.0, 9.0, true)));
        assert_eq!(parse_tool_query("bleep 0:05-0:07"), Some(FileTool::Bleep(5.0, 7.0)));
        assert_eq!(parse_tool_query("normalize -14"), Some(FileTool::Normalize(-14.0, 11.0, -1.5)));
        assert_eq!(parse_tool_query("visualizer portrait"), Some(FileTool::Visualize(crate::media::Canvas::Portrait)));
        assert_eq!(parse_tool_query("mono"), Some(FileTool::Channels(false, 1.0, 1.0)));
        assert!(FileTool::offered(Family::Video, 1).contains(&FileTool::Mute));
        assert_eq!(FileTool::offered(Family::Video, 2)[0], FileTool::Join);
        assert_eq!(parse_tool_query("trim"), None);
        assert_eq!(FileTool::offered(Family::Pdf, 1), vec![FileTool::SplitPages]);
        assert_eq!(FileTool::offered(Family::Audio, 2), vec![FileTool::Merge]);
        assert!(FileTool::offered(Family::Image, 2).is_empty());
        assert_eq!(format_secs(3723.0), "1:02:03");
    }

    #[test]
    fn exif_segment_has_the_tags() {
        let seg = build_exif_segment(&MetaEdit { make: "Canon".into(), model: "EOS R5".into(), gps: Some((40.4168, -3.7038)) });
        assert_eq!(&seg[..2], &[0xFF, 0xE1]);
        assert_eq!(u16::from_be_bytes([seg[2], seg[3]]) as usize, seg.len() - 2, "segment length");
        assert_eq!(&seg[4..10], b"Exif\0\0");
        for needle in [&b"Canon\0"[..], b"EOS R5\0", b"N\0", b"W\0"] {
            assert!(seg.windows(needle.len()).any(|w| w == needle), "{needle:?}");
        }
        // 40° 25' 0.48" → 40/1, 25/1, 4800/10000
        assert_eq!(dms(40.4168), [(40, 1), (25, 1), (4800, 10000)]);
        let empty = build_exif_segment(&MetaEdit { make: String::new(), model: String::new(), gps: None });
        assert_eq!(u16::from_be_bytes([empty[2], empty[3]]) as usize, empty.len() - 2);
    }

    #[test]
    fn png_drops_text_chunks() {
        fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
            let mut c = (data.len() as u32).to_be_bytes().to_vec();
            c.extend_from_slice(kind);
            c.extend_from_slice(data);
            c.extend([0, 0, 0, 0]); // CRC not checked here
            c
        }
        let mut p = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        p.extend(chunk(b"IHDR", &[0; 13]));
        p.extend(chunk(b"tEXt", b"Author\0me"));
        p.extend(chunk(b"eXIf", b"MM\0*"));
        p.extend(chunk(b"IDAT", b"pixels"));
        p.extend(chunk(b"IEND", b""));
        let s = strip_png(&p).unwrap();
        assert!(!s.windows(4).any(|w| w == b"tEXt" || w == b"eXIf"));
        assert!(s.windows(4).any(|w| w == b"IDAT"));
    }
}
