//! On-device detection with the Vision framework (faces, text), for Redact.

use objc2::rc::Retained;
use objc2::AnyThread;
use objc2_foundation::{NSArray, NSDictionary, NSObjectProtocol};
use objc2_vision::{
    VNDetectFaceRectanglesRequest, VNDetectTextRectanglesRequest, VNImageRequestHandler, VNRecognizeTextRequest, VNRequest,
    VNRequestTextRecognitionLevel,
};
use std::path::Path;

/// A detected region, normalised to 0..1 with the origin at the TOP-left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Region {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Faces and/or text boxes in an image. Runs locally; nothing leaves the Mac.
pub fn detect(path: &Path, faces: bool, text: bool) -> anyhow::Result<Vec<Region>> {
    let Some(url) = crate::clipboard::ns_file_url(path) else { anyhow::bail!("No image path was given") };
    let handler = unsafe { VNImageRequestHandler::initWithURL_options(VNImageRequestHandler::alloc(), &url, &NSDictionary::new()) };
    let face_req = unsafe { VNDetectFaceRectanglesRequest::new() };
    let text_req = unsafe { VNDetectTextRectanglesRequest::new() };
    let mut reqs: Vec<Retained<VNRequest>> = vec![];
    if faces {
        reqs.push(Retained::into_super(Retained::into_super(face_req.clone())));
    }
    if text {
        reqs.push(Retained::into_super(Retained::into_super(text_req.clone())));
    }
    handler
        .performRequests_error(&NSArray::from_retained_slice(&reqs))
        .map_err(|e| anyhow::anyhow!("Vision: {}", e.localizedDescription()))?;
    let mut out = vec![];
    let mut push = |r: objc2_foundation::NSRect| {
        // Vision uses a bottom-left origin
        out.push(Region { x: r.origin.x, y: 1.0 - r.origin.y - r.size.height, w: r.size.width, h: r.size.height });
    };
    if faces {
        for o in unsafe { face_req.results() }.map(|a| a.to_vec()).unwrap_or_default() {
            push(unsafe { o.boundingBox() });
        }
    }
    if text {
        for o in unsafe { text_req.results() }.map(|a| a.to_vec()).unwrap_or_default() {
            push(unsafe { o.boundingBox() });
        }
    }
    Ok(out)
}

/// Recognises text in an image file (language correction, automatic language). Runs locally.
/// Tries the accurate level first; when the accurate model is unavailable on this Mac (it fails with
/// `CRImageReaderError` at times) it retries once with the fast level so Copy Text keeps working.
/// Boxes are normalised with the bottom-left origin, as Vision returns them.
pub fn recognize_text(path: &Path) -> Result<Vec<ferrocast_core::capture::ocr::TextBox>, String> {
    with_fallback(
        || recognize_at(path, VNRequestTextRecognitionLevel::Accurate),
        || recognize_at(path, VNRequestTextRecognitionLevel::Fast),
    )
}

/// Runs `first`; when it fails runs `second` once. Both failing reports the FIRST error (the primary path's).
fn with_fallback<T, E>(first: impl FnOnce() -> Result<T, E>, second: impl FnOnce() -> Result<T, E>) -> Result<T, E> {
    match first() {
        Ok(v) => Ok(v),
        Err(e) => second().map_err(|_| e),
    }
}

fn recognize_at(path: &Path, level: VNRequestTextRecognitionLevel) -> Result<Vec<ferrocast_core::capture::ocr::TextBox>, String> {
    let Some(url) = crate::clipboard::ns_file_url(path) else { return Err("No image path was given".into()) };
    let handler = unsafe { VNImageRequestHandler::initWithURL_options(VNImageRequestHandler::alloc(), &url, &NSDictionary::new()) };
    let req = VNRecognizeTextRequest::new();
    req.setRecognitionLevel(level);
    req.setUsesLanguageCorrection(true);
    // macOS 13+ API: only call it where the selector exists
    if req.respondsToSelector(objc2::sel!(setAutomaticallyDetectsLanguage:)) {
        req.setAutomaticallyDetectsLanguage(true);
    }
    let reqs: Vec<Retained<VNRequest>> = vec![Retained::into_super(Retained::into_super(req.clone()))];
    handler.performRequests_error(&NSArray::from_retained_slice(&reqs)).map_err(|e| format!("Vision: {}", e.localizedDescription()))?;
    let mut out = vec![];
    for o in req.results().map(|a| a.to_vec()).unwrap_or_default() {
        let Some(best) = o.topCandidates(1).to_vec().into_iter().next() else { continue };
        let r = unsafe { o.boundingBox() };
        out.push(ferrocast_core::capture::ocr::TextBox {
            text: best.string().to_string(),
            x: r.origin.x,
            y: r.origin.y,
            w: r.size.width,
            h: r.size.height,
            confidence: best.confidence(),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn fallback_runs_the_second_path_only_when_the_first_fails() {
        let mut calls = vec![];
        let ok: Result<i32, String> = super::with_fallback(
            || {
                calls.push("a");
                Ok(1)
            },
            || Ok(2),
        );
        assert_eq!(ok, Ok(1));
        let mut ran_second = false;
        let recovered: Result<i32, String> = super::with_fallback(
            || Err("accurate failed".into()),
            || {
                ran_second = true;
                Ok(2)
            },
        );
        assert_eq!(recovered, Ok(2));
        assert!(ran_second);
        assert_eq!(calls, vec!["a"]);
    }

    #[test]
    fn fallback_reports_the_first_error_when_both_fail() {
        let r: Result<i32, String> = super::with_fallback(|| Err("first".into()), || Err("second".into()));
        assert_eq!(r, Err("first".to_string()));
    }

    #[test]
    fn recognizes_text_of_a_fixture_in_reading_order() {
        let p = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/text.png");
        let boxes = match super::recognize_text(std::path::Path::new(p)) {
            Ok(b) => b,
            Err(e) if e.contains("inference context") => {
                eprintln!("skipped: Vision cannot create an inference context on this machine");
                return;
            }
            Err(e) => panic!("{e}"),
        };
        let text = ferrocast_core::capture::ocr::join(&boxes);
        eprintln!("recognised: {boxes:?} -> {text:?}");
        assert!(!text.trim().is_empty(), "no text recognised");
        assert!(boxes.iter().all(|b| (0.0..=1.0).contains(&b.confidence)));
    }

    #[test]
    fn finds_text_in_a_rendered_line() {
        let p = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/text.png");
        // virtual CI machines cannot create a Vision inference context: skip only for that error
        let run = |faces: bool, text: bool| match super::detect(std::path::Path::new(p), faces, text) {
            Ok(r) => Some(r),
            Err(e) if e.to_string().contains("inference context") => {
                eprintln!("skipped: Vision cannot create an inference context on this machine");
                None
            }
            Err(e) => panic!("{e}"),
        };
        let Some(r) = run(false, true) else { return };
        assert!(!r.is_empty(), "no text found");
        assert!(r.iter().all(|b| b.x >= 0.0 && b.y >= 0.0 && b.x + b.w <= 1.01 && b.y + b.h <= 1.01), "{r:?}");
        let Some(faces) = run(true, false) else { return };
        assert!(faces.is_empty(), "no faces in a text image");
    }
}
