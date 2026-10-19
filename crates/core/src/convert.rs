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
    /// Google's WebP encoder; many ffmpeg builds lack libwebp.
    pub cwebp: Option<&'static str>,
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
}

impl Engine {
    pub fn label(&self) -> &'static str {
        match self {
            Engine::Sips | Engine::Afconvert | Engine::Avconvert | Engine::Textutil | Engine::Archive => "built in",
            Engine::Ffmpeg => "ffmpeg",
            Engine::Soffice => "LibreOffice",
            Engine::Cwebp => "cwebp",
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
                other => other,
            };
            let mut v = vec![so, "--headless".into()];
            // PDFs open in Draw by default, which cannot export to text documents
            if self::ext(input) == "pdf" {
                v.push("--infilter=writer_pdf_import".into());
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
        assert!(webp.needs.unwrap().contains("webp"));
        assert!(v.iter().find(|t| t.ext == "png").unwrap().needs.is_none());
        let with = Tools { ffmpeg: None, soffice: None, cwebp: Some("/opt/homebrew/bin/cwebp") };
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
        let tools = Tools { ffmpeg: Some("/ff"), soffice: Some("/so"), cwebp: Some("/cw") };
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
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// Grid for a collage of `n` pictures: (columns, rows), as square as possible.
pub fn collage_grid(n: usize) -> (usize, usize) {
    if n == 0 {
        return (0, 0);
    }
    let cols = (n as f64).sqrt().ceil() as usize;
    (cols, n.div_ceil(cols))
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
