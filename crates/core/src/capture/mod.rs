//! Screen capture and recording: pure, platform-independent logic.

pub mod settings;

pub use settings::{AfterCapture, CaptureSettings, ScreenshotFormat};

/// Shortest segment the editor keeps (reuses the video trimmer's minimum).
pub const MIN_SEGMENT: f64 = crate::videoplan::MIN_CLIP;
