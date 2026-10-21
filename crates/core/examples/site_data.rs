//! Writes the website data files from the app's own tables.
//!
//! `cargo run -p ferrocast-core --example site_data [OUT_DIR]` writes `conversions.json` and
//! `tools.json` (default: `web/src/data` in the workspace). The integration test
//! `tests/site_data.rs` includes this file and fails when the committed JSON drifts.
//!
//! Family choice: `matrix.rs` lists GIF as its own input whose outputs are all video formats and
//! `tools_for("gif")` is video-side, and the spec lists GIF under Video, so GIF is grouped with
//! Video here (`convert::family_of` would say Image; that is only used for routing).
//! `needs` is read from `convert::targets` on a synthetic path with an all-absent `Tools`,
//! because `route_needs` alone only covers routed pairs (it misses cwebp, unrar and ffmpeg
//! for audio/video) while `targets` covers every pair.

use ferrocast_core::convert::{label_for, targets, Tools};
use ferrocast_core::matrix::{inputs, outputs_for};
use ferrocast_core::tools::{implemented, tools_for, ActionId, Impl, ToolId, ACTIONS};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// (id, name, input extensions). Every matrix input must be in exactly one family.
const FAMILIES: [(&str, &str, &[&str]); 9] = [
    ("images", "Images", &["jpg", "png", "webp", "heic", "tiff", "svg", "avif", "bmp"]),
    ("audio", "Audio", &["mp3", "m4a", "wav", "flac", "ogg", "opus", "aiff", "wma"]),
    ("video", "Video", &["mp4", "mov", "mkv", "webm", "avi", "wmv", "gif"]),
    ("pdf", "PDF", &["pdf"]),
    ("documents", "Documents", &["docx", "pages", "md", "rtf", "rtfd", "textClipping", "epub", "txt", "html"]),
    ("slides", "Slides", &["ppt", "pptx"]),
    ("spreadsheets", "Spreadsheets", &["xlsx", "xls", "ods", "csv", "tsv"]),
    ("subtitles", "Subtitles", &["srt", "vtt"]),
    ("archives", "Archives", &["zip", "tar", "gz", "rar"]),
];

#[derive(Serialize)]
struct Output {
    ext: String,
    label: String,
    needs: Option<String>,
}
#[derive(Serialize)]
struct Input {
    ext: String,
    label: String,
    outputs: Vec<Output>,
}
#[derive(Serialize)]
struct Family {
    id: String,
    name: String,
    inputs: Vec<Input>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Conversions {
    generated_from: String,
    inputs: usize,
    conversions: usize,
    families: Vec<Family>,
}

/// Tool name from an install hint such as `"webp (brew install webp)"`.
fn needs_name(hint: &str) -> String {
    let head = hint.split(" (").next().unwrap_or(hint);
    if head == "webp" { "cwebp" } else { head }.to_string()
}

fn conversions() -> Conversions {
    let none = Tools::default();
    let all = inputs();
    let mut total_in = 0;
    let mut total_out = 0;
    let mut families = vec![];
    for (id, name, exts) in FAMILIES {
        let mut ins = vec![];
        for ext in exts {
            let outs = outputs_for(ext);
            let tg = targets(Path::new(&format!("/nonexistent/x.{ext}")), &none);
            assert_eq!(tg.len(), outs.len(), "targets/outputs_for differ for {ext}");
            let outputs = outs
                .iter()
                .zip(&tg)
                .map(|(o, t)| {
                    assert_eq!(t.ext, *o, "target order differs for {ext}");
                    Output { ext: o.to_string(), label: label_for(o).to_string(), needs: t.needs.map(needs_name) }
                })
                .collect::<Vec<_>>();
            total_in += 1;
            total_out += outputs.len();
            ins.push(Input { ext: ext.to_string(), label: label_for(ext).to_string(), outputs });
        }
        families.push(Family { id: id.into(), name: name.into(), inputs: ins });
    }
    for i in &all {
        assert_eq!(FAMILIES.iter().filter(|f| f.2.contains(i)).count(), 1, "{i} must be in exactly one family");
    }
    assert!(total_in == 46 && all.len() == 46, "expected 46 inputs, got {total_in}");
    assert!(total_out == 275, "expected 275 conversions, got {total_out}");
    Conversions { generated_from: "crates/core/src/matrix.rs".into(), inputs: total_in, conversions: total_out, families }
}

/// lowerCamel id of a tool (the variant name).
pub fn tool_key(t: ToolId) -> &'static str {
    use ToolId::*;
    match t {
        Compress => "compress",
        RemoveMetadata => "removeMetadata",
        EditImage => "editImage",
        AnnotateImage => "annotateImage",
        FrameImage => "frameImage",
        CropImage => "cropImage",
        RedactImage => "redactImage",
        CreateCollage => "createCollage",
        TrimVideo => "trimVideo",
        CropVideo => "cropVideo",
        ChangeVideoSpeed => "changeVideoSpeed",
        JoinVideos => "joinVideos",
        VideoSnapshots => "videoSnapshots",
        SplitVideo => "splitVideo",
        RedactVideo => "redactVideo",
        NormalizeAudio => "normalizeAudio",
        AudioToVideo => "audioToVideo",
        TrimAudio => "trimAudio",
        AudioChannels => "audioChannels",
        RedactAudio => "redactAudio",
        MergePdf => "mergePdf",
        OrganizePdf => "organizePdf",
        MuteVideo => "muteVideo",
        SplitPdf => "splitPdf",
        CreatePdf => "createPdf",
        ExtractArchive => "extractArchive",
    }
}

fn tool_name(t: ToolId) -> &'static str {
    use ToolId::*;
    match t {
        Compress => "Compress",
        RemoveMetadata => "Edit metadata",
        EditImage => "Edit photo",
        AnnotateImage => "Annotate photo",
        FrameImage => "Add a background",
        CropImage => "Crop image",
        RedactImage => "Redact photo",
        CreateCollage => "Make a collage",
        TrimVideo => "Trim video",
        CropVideo => "Crop video",
        ChangeVideoSpeed => "Change video speed",
        JoinVideos => "Join videos",
        VideoSnapshots => "Video snapshots",
        SplitVideo => "Split video",
        RedactVideo => "Redact video",
        NormalizeAudio => "Normalize audio",
        AudioToVideo => "Audio to video",
        TrimAudio => "Trim audio",
        AudioChannels => "Audio channels",
        RedactAudio => "Bleep audio",
        MergePdf => "Merge PDFs",
        OrganizePdf => "Organize PDF pages",
        MuteVideo => "Mute video",
        SplitPdf => "Split PDF",
        CreatePdf => "Create a PDF",
        ExtractArchive => "Extract archive",
    }
}

/// Real list shortcuts, from the `.key(CMD_SHIFT, ..)` rows in `crates/app/src/app.rs` (item actions).
fn tool_shortcut(t: ToolId) -> Option<&'static str> {
    use ToolId::*;
    match t {
        AnnotateImage => Some("⇧⌘A"),
        EditImage => Some("⇧⌘E"),
        CropImage => Some("⇧⌘K"),
        RedactImage => Some("⇧⌘R"),
        FrameImage => Some("⇧⌘B"),
        TrimVideo => Some("⇧⌘V"),
        CropVideo => Some("⇧⌘J"),
        ChangeVideoSpeed => Some("⇧⌘L"),
        VideoSnapshots => Some("⇧⌘S"),
        OrganizePdf => Some("⇧⌘O"),
        _ => None,
    }
}

pub fn status_key(i: Impl) -> &'static str {
    match i {
        Impl::Window => "window",
        Impl::Preset => "preset",
        Impl::Typed => "typed",
        Impl::Missing => "missing",
    }
}

fn action_key(a: ActionId) -> &'static str {
    match a {
        ActionId::AirDrop => "airDrop",
        ActionId::Float => "float",
        ActionId::Copy => "copy",
        ActionId::Move => "move",
        ActionId::CopyPath => "copyPath",
        ActionId::OpenTerminal => "openTerminal",
        ActionId::Zip => "zip",
    }
}

#[derive(Serialize)]
struct ToolRow {
    id: &'static str,
    name: &'static str,
    status: &'static str,
    shortcut: Option<&'static str>,
    formats: Vec<String>,
}
#[derive(Serialize)]
struct FormatRow {
    ext: String,
    single: Vec<&'static str>,
    multi: Vec<&'static str>,
}
#[derive(Serialize)]
struct ToolsFile {
    tools: Vec<ToolRow>,
    actions: Vec<&'static str>,
    #[serde(rename = "byFormat")]
    by_format: Vec<FormatRow>,
}

fn tools_file() -> ToolsFile {
    let exts = inputs();
    let keys = |e: &str, multi: bool| tools_for(e, multi).into_iter().map(tool_key).collect::<Vec<_>>();
    let tools = ToolId::ALL
        .iter()
        .map(|&t| ToolRow {
            id: tool_key(t),
            name: tool_name(t),
            status: status_key(implemented(t)),
            shortcut: tool_shortcut(t),
            formats: exts
                .iter()
                .filter(|e| tools_for(e, false).contains(&t) || tools_for(e, true).contains(&t))
                .map(|e| e.to_string())
                .collect(),
        })
        .collect();
    let by_format = exts
        .iter()
        .map(|e| FormatRow { ext: e.to_string(), single: keys(e, false), multi: keys(e, true) })
        .filter(|r| !r.single.is_empty() || !r.multi.is_empty())
        .collect();
    ToolsFile { tools, actions: ACTIONS.iter().map(|&a| action_key(a)).collect(), by_format }
}

fn pretty<T: Serialize>(v: &T) -> String {
    let mut s = serde_json::to_string_pretty(v).expect("serialize");
    s.push('\n');
    s
}

/// `(conversions.json, tools.json)` contents.
pub fn generate() -> (String, String) {
    (pretty(&conversions()), pretty(&tools_file()))
}

fn main() {
    let dir = match std::env::args().nth(1) {
        Some(p) => PathBuf::from(p),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join("web/src/data"),
    };
    std::fs::create_dir_all(&dir).expect("create output dir");
    let (c, t) = generate();
    std::fs::write(dir.join("conversions.json"), c).expect("write conversions.json");
    std::fs::write(dir.join("tools.json"), t).expect("write tools.json");
    println!("wrote {}", dir.display());
}
