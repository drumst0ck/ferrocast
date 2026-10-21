//! Timed video redaction with several areas (A11a). Pure: validation and clamping only.
//! Rectangles are in DISPLAYED pixel space (after rotation), like the crop tool (L56).

/// Most areas one run may carry (keeps the filtergraph and the encode bounded).
pub const MAX_AREAS: usize = 64;
/// Allowed overshoot of `end` past the probed duration, in seconds.
pub const END_TOLERANCE: f64 = 0.05;

/// One area covered from `start` to `end` seconds: solid black, or blurred when `blur`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Redaction {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub start: f64,
    pub end: f64,
    pub blur: bool,
}

/// Checks every area against the displayed frame and the probed duration. Messages name the
/// offending area by its 1-based number. Overlapping areas are allowed.
pub fn validate(redactions: &[Redaction], duration: f64, width: u32, height: u32) -> Result<(), String> {
    if redactions.is_empty() {
        return Err("add at least one area to redact".into());
    }
    if redactions.len() > MAX_AREAS {
        return Err(format!("too many areas ({}); the limit is {MAX_AREAS}", redactions.len()));
    }
    if !duration.is_finite() || duration <= 0.0 {
        return Err("could not read the video duration".into());
    }
    for (i, r) in redactions.iter().enumerate() {
        let n = i + 1;
        if r.w < 2 || r.h < 2 {
            return Err(format!("area {n} is too small (minimum 2×2 px)"));
        }
        if u64::from(r.x) + u64::from(r.w) > u64::from(width) || u64::from(r.y) + u64::from(r.h) > u64::from(height) {
            return Err(format!("area {n} is outside the {width}×{height} frame"));
        }
        if !r.start.is_finite() || !r.end.is_finite() || r.start < 0.0 || r.end <= r.start {
            return Err(format!("area {n} has an invalid time range (need 0 ≤ start < end)"));
        }
        if r.end > duration + END_TOLERANCE {
            return Err(format!("area {n} ends after the video ({duration:.2} s)"));
        }
    }
    Ok(())
}

/// The area clipped to the frame, or None when nothing of it (at least 2×2) remains.
pub fn clamp(r: &Redaction, width: u32, height: u32) -> Option<Redaction> {
    if r.x >= width || r.y >= height {
        return None;
    }
    let w = r.w.min(width - r.x);
    let h = r.h.min(height - r.y);
    (w >= 2 && h >= 2).then_some(Redaction { w, h, ..*r })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: u32, y: u32, w: u32, h: u32, start: f64, end: f64) -> Redaction {
        Redaction { x, y, w, h, start, end, blur: false }
    }

    #[test]
    fn area_count_bounds() {
        let one = r(0, 0, 10, 10, 0.0, 1.0);
        assert!(validate(&[], 5.0, 100, 100).is_err());
        assert!(validate(&[one], 5.0, 100, 100).is_ok());
        assert!(validate(&vec![one; 64], 5.0, 100, 100).is_ok());
        assert!(validate(&vec![one; 65], 5.0, 100, 100).unwrap_err().contains("65"));
    }

    #[test]
    fn frame_and_size_checks_name_the_area() {
        let ok = r(0, 0, 10, 10, 0.0, 1.0);
        assert!(validate(&[ok, r(95, 0, 10, 10, 0.0, 1.0)], 5.0, 100, 100).unwrap_err().contains("area 2"));
        assert!(validate(&[r(0, 91, 10, 10, 0.0, 1.0)], 5.0, 100, 100).unwrap_err().contains("area 1"));
        assert!(validate(&[r(90, 90, 10, 10, 0.0, 1.0)], 5.0, 100, 100).is_ok(), "touching the edge is inside");
        assert!(validate(&[ok, ok, r(0, 0, 1, 10, 0.0, 1.0)], 5.0, 100, 100).unwrap_err().contains("area 3"));
        assert!(validate(&[r(u32::MAX, 0, 10, 10, 0.0, 1.0)], 5.0, 100, 100).is_err(), "no overflow");
    }

    #[test]
    fn time_ranges() {
        let bad = |a: f64, b: f64| validate(&[r(0, 0, 10, 10, a, b)], 5.0, 100, 100);
        assert!(bad(2.0, 1.0).unwrap_err().contains("area 1"));
        assert!(bad(1.0, 1.0).is_err());
        assert!(bad(f64::NAN, 1.0).is_err());
        assert!(bad(0.0, f64::NAN).is_err());
        assert!(bad(0.0, f64::INFINITY).is_err());
        assert!(bad(-0.5, 1.0).is_err());
        assert!(bad(0.0, 5.04).is_ok(), "tolerance");
        assert!(bad(0.0, 5.2).unwrap_err().contains("ends after"));
        assert!(validate(&[r(0, 0, 10, 10, 0.0, 1.0)], f64::NAN, 100, 100).is_err());
    }

    #[test]
    fn overlapping_areas_are_allowed() {
        assert!(validate(&[r(10, 10, 50, 50, 0.0, 3.0), r(30, 30, 50, 50, 1.0, 4.0), r(10, 10, 50, 50, 0.0, 3.0)], 5.0, 100, 100).is_ok());
    }

    #[test]
    fn clamp_clips_to_the_frame() {
        assert_eq!(clamp(&r(90, 90, 50, 50, 0.0, 1.0), 100, 100), Some(r(90, 90, 10, 10, 0.0, 1.0)));
        assert_eq!(clamp(&r(0, 0, 10, 10, 0.0, 1.0), 100, 100), Some(r(0, 0, 10, 10, 0.0, 1.0)));
        assert_eq!(clamp(&r(99, 0, 10, 10, 0.0, 1.0), 100, 100), None, "1 px sliver");
        assert_eq!(clamp(&r(100, 0, 10, 10, 0.0, 1.0), 100, 100), None);
    }
}
