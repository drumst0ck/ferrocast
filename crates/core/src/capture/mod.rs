//! Screen capture and recording: pure, platform-independent logic.

pub mod events;
pub mod ocr;
pub mod project;
pub mod protocol;
pub mod region;
pub mod settings;
pub mod stitch;

pub use settings::{AfterCapture, CaptureSettings, ScreenshotFormat};

/// Shortest segment the editor keeps (reuses the video trimmer's minimum).
pub const MIN_SEGMENT: f64 = crate::videoplan::MIN_CLIP;
pub mod camera;
pub mod compose;
pub mod cursor;
pub mod curves;
pub mod export;
pub mod timeline;
pub mod transcript;
pub mod zoom;
