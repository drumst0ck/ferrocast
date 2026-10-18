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
        "jpg" | "jpeg" | "png" | "webp" | "heic" | "heif" | "tif" | "tiff" | "avif" | "bmp" | "gif" | "jxl" | "jp2" | "psd" | "dng"
        | "cr2" | "cr3" | "nef" | "arw" | "raf" | "orf" | "rw2" | "icns" | "ico" => Family::Image,
        "mp3" | "m4a" | "wav" | "flac" | "ogg" | "opus" | "aiff" | "aif" | "wma" | "caf" | "aac" | "m4b" => Family::Audio,
        "mp4" | "mov" | "mkv" | "webm" | "avi" | "wmv" | "m4v" | "mpg" | "mpeg" | "ts" | "3gp" => Family::Video,
        "docx" | "doc" | "pages" | "md" | "markdown" | "txt" | "rtf" | "rtfd" | "odt" | "html" | "htm" | "epub" => Family::Document,
        "xlsx" | "xls" | "ods" | "csv" | "tsv" | "numbers" => Family::Spreadsheet,
        "ppt" | "pptx" | "key" | "odp" => Family::Slides,
        "pdf" => Family::Pdf,
        "srt" | "vtt" => Family::Subtitle,
        "zip" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "7z" | "rar" => Family::Archive,
        _ => Family::Other,
    }
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
}

impl Engine {
    pub fn label(&self) -> &'static str {
        match self {
            Engine::Sips | Engine::Afconvert | Engine::Avconvert | Engine::Textutil | Engine::Archive => "built in",
            Engine::Ffmpeg => "ffmpeg",
            Engine::Soffice => "LibreOffice",
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
            t("webp", "WebP", Engine::Ffmpeg, tools),
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
        ],
        Family::Document => match e.as_str() {
            "md" | "markdown" | "txt" => vec![
                t("pdf", "PDF", Engine::Soffice, tools),
                t("docx", "Word (DOCX)", Engine::Textutil, tools),
                t("html", "HTML", Engine::Textutil, tools),
                t("rtf", "RTF", Engine::Textutil, tools),
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
                t("md", "Markdown", Engine::Soffice, tools),
            ],
        },
        Family::Spreadsheet => vec![
            t("pdf", "PDF", Engine::Soffice, tools),
            t("xlsx", "Excel (XLSX)", Engine::Soffice, tools),
            t("csv", "CSV", Engine::Soffice, tools),
            t("ods", "OpenDocument", Engine::Soffice, tools),
        ],
        Family::Slides => vec![t("pdf", "PDF", Engine::Soffice, tools), t("pptx", "PowerPoint (PPTX)", Engine::Soffice, tools)],
        Family::Pdf => vec![
            t("png", "PNG (first page)", Engine::Sips, tools),
            t("jpg", "JPEG (first page)", Engine::Sips, tools),
            t("docx", "Word (DOCX)", Engine::Soffice, tools),
            t("txt", "Plain text", Engine::Soffice, tools),
        ],
        Family::Subtitle => vec![t("vtt", "WebVTT", Engine::Ffmpeg, tools), t("srt", "SubRip (SRT)", Engine::Ffmpeg, tools)],
        Family::Archive | Family::Folder => vec![t("zip", "ZIP", Engine::Archive, tools), t("tar.gz", "tar.gz", Engine::Archive, tools)],
        Family::Other => vec![t("zip", "ZIP", Engine::Archive, tools)],
    };
    v.retain(|x| x.ext != e && !(e == "jpeg" && x.ext == "jpg") && !(e == "tif" && x.ext == "tiff"));
    v
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
        (Engine::Archive, ext) => return Err(format!("no archive format {ext}")),
        (Engine::Ffmpeg, ext) => {
            let ff = tools.ffmpeg.ok_or("ffmpeg is not installed")?.to_string();
            let mut v = vec![ff, "-y".into(), "-loglevel".into(), "error".into(), "-i".into(), i];
            match ext {
                "gif" => v.extend(["-vf".into(), "fps=12,scale=640:-1:flags=lanczos".into()]),
                "mp3" => v.extend(["-vn".into(), "-codec:a".into(), "libmp3lame".into(), "-q:a".into(), "2".into()]),
                "webp" => v.extend(["-quality".into(), "85".into()]),
                _ => {}
            }
            v.push(o);
            v
        }
        (Engine::Soffice, ext) => {
            let so = tools.soffice.ok_or("LibreOffice is not installed")?.to_string();
            let outdir = out.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| ".".into());
            let filter = match ext {
                "md" => "md:Markdown",
                other => other,
            };
            // soffice names the file itself (<stem>.<ext>); the caller renames to `out` afterwards.
            vec![so, "--headless".into(), "--convert-to".into(), filter.into(), "--outdir".into(), outdir, i]
        }
    };
    Ok(cmd)
}

/// soffice writes `<stem>.<ext>` into outdir regardless of the name we wanted.
pub fn soffice_written_path(input: &Path, out: &Path, ext: &str) -> PathBuf {
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("converted");
    out.parent().map(Path::to_path_buf).unwrap_or_default().join(format!("{stem}.{}", ext.split(':').next().unwrap_or(ext)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none() -> Tools {
        Tools::default()
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
        assert!(webp.needs.unwrap().contains("ffmpeg"));
        assert!(v.iter().find(|t| t.ext == "png").unwrap().needs.is_none());
        let with = Tools { ffmpeg: Some("/opt/homebrew/bin/ffmpeg"), soffice: None };
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
        let tools = Tools { ffmpeg: Some("/ff"), soffice: Some("/so") };
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
