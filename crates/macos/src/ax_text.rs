//! Accessibility helpers for translating in place: can the focused element of an app take text,
//! and replace its selected text.
//!
//! Every call here can block on the target app (up to the 1 s messaging timeout), so NOTHING in
//! this module may run on the UI thread. Call it from a background thread only.

use accessibility_sys::*;
use core_foundation::array::{CFArrayGetCount, CFArrayGetValueAtIndex, CFArrayRef};
use core_foundation::base::{CFRange, CFRelease, CFTypeRef, TCFType};
use core_foundation::string::CFString;
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use std::ffi::c_void;

/// Longest wait for an unresponsive app, in seconds.
const TIMEOUT: f32 = 1.0;

struct Owned(CFTypeRef);
impl Drop for Owned {
    fn drop(&mut self) {
        // never CFRelease(NULL): it aborts the process
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) }
        }
    }
}

/// The focused element of app `pid`, owned. `None` when AX is not trusted or unreadable.
fn focused_element(pid: i32) -> Option<Owned> {
    unsafe {
        if !AXIsProcessTrusted() {
            return None;
        }
        let app = Owned(AXUIElementCreateApplication(pid) as CFTypeRef);
        AXUIElementSetMessagingTimeout(app.0 as AXUIElementRef, TIMEOUT);
        let name = CFString::new("AXFocusedUIElement");
        let mut v: CFTypeRef = std::ptr::null();
        let err = AXUIElementCopyAttributeValue(app.0 as AXUIElementRef, name.as_concrete_TypeRef(), &mut v);
        // not `then_some(Owned(v))`: that builds (and drops) the value even when the read failed
        if err == kAXErrorSuccess && !v.is_null() {
            Some(Owned(v))
        } else {
            None
        }
    }
}

/// Whether the focused element of `pid` accepts selected text. `None` when undeterminable.
pub fn focused_accepts_text(pid: i32) -> Option<bool> {
    let el = focused_element(pid)?;
    let name = CFString::new("AXSelectedText");
    let mut settable: u8 = 0;
    let err = unsafe { AXUIElementIsAttributeSettable(el.0 as AXUIElementRef, name.as_concrete_TypeRef(), &mut settable) };
    (err == kAXErrorSuccess).then_some(settable != 0)
}

/// Replace the selected text of the focused element of `pid`; true only on success.
pub fn set_selected_text(pid: i32, text: &str) -> bool {
    let Some(el) = focused_element(pid) else { return false };
    let name = CFString::new("AXSelectedText");
    let value = CFString::new(text);
    let err = unsafe { AXUIElementSetAttributeValue(el.0 as AXUIElementRef, name.as_concrete_TypeRef(), value.as_CFTypeRef()) };
    err == kAXErrorSuccess
}

/// Smallest rectangle (x, y, w, h) containing every rect. `None` for an empty slice.
pub fn rect_union(rects: &[[f64; 4]]) -> Option<[f64; 4]> {
    let first = rects.first()?;
    let (mut x0, mut y0, mut x1, mut y1) = (first[0], first[1], first[0] + first[2], first[1] + first[3]);
    for r in &rects[1..] {
        x0 = x0.min(r[0]);
        y0 = y0.min(r[1]);
        x1 = x1.max(r[0] + r[2]);
        y1 = y1.max(r[1] + r[3]);
    }
    Some([x0, y0, x1 - x0, y1 - y0])
}

/// Most selection ranges turned into rectangles.
const MAX_RANGES: usize = 8;
/// Largest plausible side of a selection rectangle, in points.
const MAX_SIDE: f64 = 10000.0;

/// Screen rectangles (x, y, w, h) of the selected text of the focused element of app `pid`, in
/// the global top-left coordinates Accessibility uses (the same space as `mouse_top_left`).
/// One rectangle per selected range (at most 8). `None` when AX is not trusted, the element
/// does not expose bounds, or a rectangle is empty or absurd.
///
/// Can block on the target app (1 s messaging timeout): background thread only, never the UI thread.
pub fn selection_bounds(pid: i32) -> Option<Vec<[f64; 4]>> {
    let el = focused_element(pid)?;
    let element = el.0 as AXUIElementRef;
    let mut ranges = selected_ranges(element);
    ranges.truncate(MAX_RANGES);
    if ranges.is_empty() {
        return None;
    }
    let mut out = Vec::with_capacity(ranges.len());
    for range in ranges {
        let r = bounds_for_range(element, range)?;
        if !valid_rect(&r) {
            return None;
        }
        out.push(r);
    }
    Some(out)
}

fn valid_rect(r: &[f64; 4]) -> bool {
    r.iter().all(|v| v.is_finite()) && r[2] > 0.0 && r[3] > 0.0 && r[2] <= MAX_SIDE && r[3] <= MAX_SIDE
}

fn range_from_value(v: CFTypeRef) -> Option<CFRange> {
    let mut range = CFRange { location: 0, length: 0 };
    let ok = unsafe { AXValueGetValue(v as AXValueRef, kAXValueTypeCFRange, &mut range as *mut _ as *mut c_void) };
    (ok && range.location >= 0 && range.length > 0).then_some(range)
}

/// The non-empty selected ranges: `AXSelectedTextRanges` when present, else `AXSelectedTextRange`.
fn selected_ranges(element: AXUIElementRef) -> Vec<CFRange> {
    let mut out = Vec::new();
    if let Some(arr) = copy_attribute(element, "AXSelectedTextRanges") {
        unsafe {
            let n = CFArrayGetCount(arr.0 as CFArrayRef);
            for i in 0..n {
                let v = CFArrayGetValueAtIndex(arr.0 as CFArrayRef, i);
                if !v.is_null() {
                    out.extend(range_from_value(v as CFTypeRef));
                }
            }
        }
    }
    if out.is_empty() {
        if let Some(v) = copy_attribute(element, "AXSelectedTextRange") {
            out.extend(range_from_value(v.0));
        }
    }
    out
}

fn copy_attribute(element: AXUIElementRef, attr: &str) -> Option<Owned> {
    let name = CFString::new(attr);
    let mut v: CFTypeRef = std::ptr::null();
    let err = unsafe { AXUIElementCopyAttributeValue(element, name.as_concrete_TypeRef(), &mut v) };
    if err == kAXErrorSuccess && !v.is_null() {
        Some(Owned(v))
    } else {
        None
    }
}

fn bounds_for_range(element: AXUIElementRef, mut range: CFRange) -> Option<[f64; 4]> {
    unsafe {
        let param = AXValueCreate(kAXValueTypeCFRange, &mut range as *mut _ as *const c_void);
        if param.is_null() {
            return None;
        }
        let param = Owned(param as CFTypeRef);
        let name = CFString::new("AXBoundsForRange");
        let mut v: CFTypeRef = std::ptr::null();
        let err = AXUIElementCopyParameterizedAttributeValue(element, name.as_concrete_TypeRef(), param.0, &mut v);
        if err != kAXErrorSuccess || v.is_null() {
            return None;
        }
        let v = Owned(v);
        let mut rect = CGRect::new(&CGPoint::new(0.0, 0.0), &CGSize::new(0.0, 0.0));
        if !AXValueGetValue(v.0 as AXValueRef, kAXValueTypeCGRect, &mut rect as *mut _ as *mut c_void) {
            return None;
        }
        Some([rect.origin.x, rect.origin.y, rect.size.width, rect.size.height])
    }
}

/// Bundle id of the frontmost app (reuses `running::frontmost`).
pub fn frontmost_bundle_id() -> Option<String> {
    crate::running::frontmost().and_then(|a| a.bundle_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn rect_union_empty_is_none() {
        assert_eq!(rect_union(&[]), None);
    }

    #[test]
    fn rect_union_single_is_identity() {
        assert_eq!(rect_union(&[[10.0, 20.0, 30.0, 40.0]]), Some([10.0, 20.0, 30.0, 40.0]));
    }

    #[test]
    fn rect_union_covers_all_rects() {
        let u = rect_union(&[[10.0, 10.0, 100.0, 20.0], [0.0, 40.0, 50.0, 20.0], [200.0, 5.0, 10.0, 10.0]]).unwrap();
        assert_eq!(u, [0.0, 5.0, 210.0, 55.0]);
    }

    #[test]
    fn dropping_a_null_owned_is_safe() {
        // AXUIElementCopyAttributeValue leaves the out pointer NULL on failure; the helper must never
        // CFRelease(NULL) (that aborts the whole app)
        drop(Owned(std::ptr::null()));
    }

    #[test]
    fn missing_pid_is_none_false_and_does_not_block() {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            // pids that AXUIElementCreateApplication may refuse (it can return NULL) must not crash
            for pid in [0, -1, i32::MIN] {
                assert_eq!(focused_accepts_text(pid), None, "pid {pid}");
                assert!(!set_selected_text(pid, "x"), "pid {pid}");
            }
            let r = (focused_accepts_text(i32::MAX), set_selected_text(i32::MAX, "x"));
            let _ = tx.send(r);
        });
        let (accepts, set) = rx.recv_timeout(Duration::from_secs(5)).expect("AX helpers blocked");
        assert_eq!(accepts, None);
        assert!(!set);
        let _ = frontmost_bundle_id();
    }

    #[test]
    fn selection_bounds_of_a_missing_pid_is_none_and_does_not_block() {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let r: Vec<_> = [0, -1, i32::MAX].iter().map(|&p| selection_bounds(p)).collect();
            let _ = tx.send(r);
        });
        let r = rx.recv_timeout(Duration::from_secs(10)).expect("selection_bounds blocked");
        assert!(r.iter().all(|x| x.is_none()));
    }

    #[test]
    fn rect_validity() {
        assert!(valid_rect(&[0.0, 0.0, 10.0, 10.0]));
        assert!(!valid_rect(&[0.0, 0.0, 0.0, 10.0]));
        assert!(!valid_rect(&[0.0, 0.0, 10.0, -1.0]));
        assert!(!valid_rect(&[0.0, 0.0, 10001.0, 10.0]));
        assert!(!valid_rect(&[f64::NAN, 0.0, 10.0, 10.0]));
    }
}
