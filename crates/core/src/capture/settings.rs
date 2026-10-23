//! `Settings.capture`: defaults per S2.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Screenshot file format.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum ScreenshotFormat {
    #[default]
    Png,
    Jpg,
}

/// What happens with a finished capture.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum AfterCapture {
    /// Open the Capture window.
    #[default]
    Show,
    /// Only copy to the clipboard.
    Copy,
    /// Only save the file.
    Save,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CaptureSettings {
    /// Screenshot folder; empty = `~/Pictures/Ferrocast Captures`.
    pub folder: String,
    pub screenshot_format: ScreenshotFormat,
    pub after: AfterCapture,
    pub include_cursor: bool,
    /// Recordings folder; empty = `~/Movies/Ferrocast Recordings`.
    pub recordings_folder: String,
    /// 30 or 60.
    pub fps: u32,
    /// Editor background preset id; empty = the built-in default.
    pub default_background: String,
    /// Cursor size multiplier in the editor.
    pub cursor_size: f64,
    pub auto_zoom: bool,
    pub zoom_scale: f64,
    /// Record the camera too (P4).
    pub camera: bool,
    /// Camera device id from the helper's `devices`; empty = the system default.
    pub camera_id: String,
    /// Record the microphone too (P4).
    pub microphone: bool,
    /// Microphone device id from the helper's `devices`; empty = the system default.
    pub mic_id: String,
    /// Record the system audio too (P4).
    pub system_audio: bool,
    /// Seconds before recording starts: 0, 3 or 5.
    pub countdown: u32,
}

impl Default for CaptureSettings {
    fn default() -> Self {
        CaptureSettings {
            folder: String::new(),
            screenshot_format: ScreenshotFormat::Png,
            after: AfterCapture::Show,
            include_cursor: false,
            recordings_folder: String::new(),
            fps: 60,
            default_background: String::new(),
            cursor_size: 1.0,
            auto_zoom: true,
            zoom_scale: 2.0,
            camera: false,
            camera_id: String::new(),
            microphone: false,
            mic_id: String::new(),
            system_audio: false,
            countdown: 3,
        }
    }
}

fn resolve(custom: &str, default_rel: &str, home: &Path) -> PathBuf {
    let c = custom.trim();
    if c.is_empty() {
        home.join(default_rel)
    } else if c == "~" {
        home.to_path_buf()
    } else if let Some(rest) = c.strip_prefix("~/") {
        home.join(rest)
    } else {
        PathBuf::from(c)
    }
}

impl CaptureSettings {
    /// Where screenshots are saved.
    pub fn screenshots_dir(&self, home: &Path) -> PathBuf {
        resolve(&self.folder, "Pictures/Ferrocast Captures", home)
    }

    /// Where recording projects are saved.
    pub fn recordings_dir(&self, home: &Path) -> PathBuf {
        resolve(&self.recordings_folder, "Movies/Ferrocast Recordings", home)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Settings;

    #[test]
    fn defaults_match_s2() {
        let c = CaptureSettings::default();
        assert_eq!(c.screenshot_format, ScreenshotFormat::Png);
        assert_eq!((c.fps, c.countdown), (60, 3));
        assert!(c.auto_zoom);
        assert_eq!(c.zoom_scale, 2.0);
        assert_eq!(c.after, AfterCapture::Show);
        assert!(c.folder.is_empty());
        let home = std::path::Path::new("/Users/me");
        assert_eq!(c.screenshots_dir(home), home.join("Pictures/Ferrocast Captures"));
        assert_eq!(c.recordings_dir(home), home.join("Movies/Ferrocast Recordings"));
    }

    #[test]
    fn custom_folder_wins_and_tilde_expands() {
        let home = std::path::Path::new("/Users/me");
        let c = CaptureSettings { folder: "/tmp/x".into(), recordings_folder: "~/Rec".into(), ..Default::default() };
        assert_eq!(c.screenshots_dir(home), std::path::PathBuf::from("/tmp/x"));
        assert_eq!(c.recordings_dir(home), home.join("Rec"));
    }

    #[test]
    fn old_settings_load_unchanged_with_capture_defaults() {
        let old: Settings = serde_json::from_str(r#"{"launcher_hotkey":"Alt+Space","compact_mode":true}"#).unwrap();
        assert!(old.compact_mode);
        assert_eq!(old.capture, CaptureSettings::default());
        assert_eq!(Settings::default().capture, CaptureSettings::default());
    }

    #[test]
    fn partial_capture_object_fills_the_rest_and_round_trips() {
        let s: Settings = serde_json::from_str(r#"{"capture":{"fps":30,"after":"Copy"}}"#).unwrap();
        assert_eq!((s.capture.fps, s.capture.after), (30, AfterCapture::Copy));
        assert_eq!((s.capture.countdown, s.capture.zoom_scale), (3, 2.0));
        let back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn p4_sources_default_off_and_old_files_load_unchanged() {
        let c = CaptureSettings::default();
        assert!(!c.camera && !c.microphone && !c.system_audio);
        assert!(c.camera_id.is_empty() && c.mic_id.is_empty());
        let s: Settings = serde_json::from_str(r#"{"capture":{"fps":30,"camera_id":"0x1"}}"#).unwrap();
        assert_eq!((s.capture.fps, s.capture.camera_id.as_str(), s.capture.camera), (30, "0x1", false));
        let on: Settings =
            serde_json::from_str(r#"{"capture":{"camera":true,"microphone":true,"system_audio":true,"mic_id":"m"}}"#).unwrap();
        assert!(on.capture.camera && on.capture.microphone && on.capture.system_audio);
        let back: Settings = serde_json::from_str(&serde_json::to_string(&on).unwrap()).unwrap();
        assert_eq!(back, on);
    }
}
