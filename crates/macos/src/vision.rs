//! On-device detection with the Vision framework (faces, text), for Redact.

use objc2::rc::Retained;
use objc2::AnyThread;
use objc2_foundation::{NSArray, NSDictionary};
use objc2_vision::{VNDetectFaceRectanglesRequest, VNDetectTextRectanglesRequest, VNImageRequestHandler, VNRequest};
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

#[cfg(test)]
mod tests {
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
