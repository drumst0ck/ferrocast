//! Which tools Tangerine-style wheels offer per file format (S11 `et()` mapping) and the six file
//! actions. Pure data, so every rule is testable without a window.

/// The 26 advanced file tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolId {
    Compress,
    RemoveMetadata,
    EditImage,
    AnnotateImage,
    FrameImage,
    CropImage,
    RedactImage,
    CreateCollage,
    TrimVideo,
    CropVideo,
    ChangeVideoSpeed,
    JoinVideos,
    VideoSnapshots,
    SplitVideo,
    RedactVideo,
    NormalizeAudio,
    AudioToVideo,
    TrimAudio,
    AudioChannels,
    RedactAudio,
    MergePdf,
    OrganizePdf,
    MuteVideo,
    SplitPdf,
    CreatePdf,
    ExtractArchive,
}

impl ToolId {
    pub const ALL: [ToolId; 26] = [
        ToolId::Compress,
        ToolId::RemoveMetadata,
        ToolId::EditImage,
        ToolId::AnnotateImage,
        ToolId::FrameImage,
        ToolId::CropImage,
        ToolId::RedactImage,
        ToolId::CreateCollage,
        ToolId::TrimVideo,
        ToolId::CropVideo,
        ToolId::ChangeVideoSpeed,
        ToolId::JoinVideos,
        ToolId::VideoSnapshots,
        ToolId::SplitVideo,
        ToolId::RedactVideo,
        ToolId::NormalizeAudio,
        ToolId::AudioToVideo,
        ToolId::TrimAudio,
        ToolId::AudioChannels,
        ToolId::RedactAudio,
        ToolId::MergePdf,
        ToolId::OrganizePdf,
        ToolId::MuteVideo,
        ToolId::SplitPdf,
        ToolId::CreatePdf,
        ToolId::ExtractArchive,
    ];
}

/// The six file actions, in the order the actions wheel shows them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionId {
    AirDrop,
    Float,
    Copy,
    Move,
    CopyPath,
    OpenTerminal,
}

pub const ACTIONS: [ActionId; 6] =
    [ActionId::AirDrop, ActionId::Float, ActionId::Copy, ActionId::Move, ActionId::CopyPath, ActionId::OpenTerminal];

/// How a tool is reachable in Ferrocast today. An honest table: website and UI read it, so a tool
/// is never advertised as a window when it is only a preset or a typed command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Impl {
    /// A dedicated editor window.
    Window,
    /// One-step presets in the Convert view or the wheel.
    Preset,
    /// Only a typed command.
    Typed,
    /// Not available at all.
    Missing,
}

/// Update this table whenever a window lands or is removed (S11 gaps G5-G12 in the parity plan).
pub fn implemented(t: ToolId) -> Impl {
    use ToolId::*;
    match t {
        EditImage | AnnotateImage | FrameImage | CropImage | RedactImage | CreateCollage | TrimVideo | CropVideo | ChangeVideoSpeed
        | VideoSnapshots | RedactVideo | OrganizePdf | SplitVideo | TrimAudio => Impl::Window,
        Compress | RemoveMetadata | JoinVideos | MuteVideo | AudioChannels | NormalizeAudio | MergePdf | SplitPdf | CreatePdf
        | ExtractArchive => Impl::Preset,
        AudioToVideo | RedactAudio => Impl::Typed,
    }
}

/// Tools offered for a file with this extension; `multi` is true for a selection of several files
/// of that format. The lists follow the tool-by-format table in the spec (S11). That table names
/// only mp3, m4a, wav and flac for audio, so other audio formats get none and audio has no
/// multi-selection list.
pub fn tools_for(ext: &str, multi: bool) -> Vec<ToolId> {
    use ToolId::*;
    let e = ext.trim().trim_start_matches('.').to_ascii_lowercase();
    let e = match e.as_str() {
        "jpeg" => "jpg",
        "tif" => "tiff",
        other => other,
    };
    match (e, multi) {
        ("jpg" | "png" | "webp" | "heic" | "tiff", false) => {
            vec![Compress, RemoveMetadata, EditImage, AnnotateImage, FrameImage, CropImage, RedactImage]
        }
        ("jpg" | "png" | "webp" | "heic" | "tiff", true) => vec![Compress, RemoveMetadata, CropImage, CreatePdf, CreateCollage],
        ("svg", true) => vec![CreatePdf, CreateCollage],
        ("mp3" | "m4a" | "wav" | "flac", false) => {
            vec![Compress, RemoveMetadata, NormalizeAudio, AudioToVideo, TrimAudio, AudioChannels, RedactAudio]
        }
        ("mp4" | "mov" | "mkv", false) => {
            vec![Compress, RemoveMetadata, MuteVideo, TrimVideo, CropVideo, ChangeVideoSpeed, VideoSnapshots, SplitVideo, RedactVideo]
        }
        ("mp4" | "mov" | "mkv", true) => vec![Compress, RemoveMetadata, MuteVideo, TrimVideo, CropVideo, ChangeVideoSpeed, JoinVideos],
        ("gif", false) => vec![RemoveMetadata],
        ("pdf", false) => vec![Compress, RemoveMetadata, SplitPdf, OrganizePdf],
        ("pdf", true) => vec![Compress, RemoveMetadata, SplitPdf, MergePdf],
        ("zip" | "tar" | "gz" | "rar", _) => vec![ExtractArchive],
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn images_get_seven_tools_alone_and_five_together() {
        for e in ["jpg", "png", "webp", "heic", "tiff"] {
            assert_eq!(
                tools_for(e, false),
                vec![
                    ToolId::Compress,
                    ToolId::RemoveMetadata,
                    ToolId::EditImage,
                    ToolId::AnnotateImage,
                    ToolId::FrameImage,
                    ToolId::CropImage,
                    ToolId::RedactImage
                ],
                "{e}"
            );
            assert_eq!(
                tools_for(e, true),
                vec![ToolId::Compress, ToolId::RemoveMetadata, ToolId::CropImage, ToolId::CreatePdf, ToolId::CreateCollage],
                "{e} multi"
            );
        }
    }

    #[test]
    fn svg_only_has_multi_selection_tools() {
        assert!(tools_for("svg", false).is_empty());
        assert_eq!(tools_for("svg", true), vec![ToolId::CreatePdf, ToolId::CreateCollage]);
    }

    #[test]
    fn audio_video_gif_pdf_and_archives() {
        for e in ["mp3", "m4a", "wav", "flac"] {
            assert_eq!(tools_for(e, false).len(), 7, "{e}");
            assert!(tools_for(e, false).contains(&ToolId::TrimAudio));
        }
        for e in ["mp4", "mov", "mkv"] {
            assert_eq!(tools_for(e, false).len(), 9, "{e}");
            assert_eq!(tools_for(e, true).len(), 7, "{e} multi");
            assert!(tools_for(e, true).contains(&ToolId::JoinVideos));
            assert!(!tools_for(e, true).contains(&ToolId::SplitVideo));
        }
        assert_eq!(tools_for("gif", false), vec![ToolId::RemoveMetadata]);
        assert_eq!(tools_for("pdf", false), vec![ToolId::Compress, ToolId::RemoveMetadata, ToolId::SplitPdf, ToolId::OrganizePdf]);
        assert_eq!(tools_for("pdf", true), vec![ToolId::Compress, ToolId::RemoveMetadata, ToolId::SplitPdf, ToolId::MergePdf]);
        for e in ["zip", "tar", "gz", "rar"] {
            assert_eq!(tools_for(e, false), vec![ToolId::ExtractArchive], "{e}");
        }
        assert!(tools_for("txt", false).is_empty());
        assert!(tools_for("", true).is_empty());
    }

    #[test]
    fn aliases_and_case_are_normalised() {
        assert_eq!(tools_for("JPEG", false), tools_for("jpg", false));
        assert_eq!(tools_for("tif", false), tools_for("tiff", false));
        assert_eq!(tools_for("MP4", true), tools_for("mp4", true));
    }

    #[test]
    fn all_26_tools_are_reachable_and_have_a_status() {
        let mut seen = std::collections::HashSet::new();
        for e in ["jpg", "svg", "mp3", "mp4", "gif", "pdf", "zip"] {
            for m in [false, true] {
                seen.extend(tools_for(e, m));
            }
        }
        assert_eq!(ToolId::ALL.len(), 26);
        for t in ToolId::ALL {
            assert!(seen.contains(&t), "{t:?} is in no list");
            let _ = implemented(t);
        }
    }

    #[test]
    fn six_actions_in_tangerine_order() {
        assert_eq!(
            ACTIONS,
            [ActionId::AirDrop, ActionId::Float, ActionId::Copy, ActionId::Move, ActionId::CopyPath, ActionId::OpenTerminal]
        );
    }
}
