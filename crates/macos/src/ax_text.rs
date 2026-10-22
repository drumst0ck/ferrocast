//! Accessibility helpers for translating in place: can the focused element of an app take text,
//! and replace its selected text.
//!
//! Every call here can block on the target app (up to the 1 s messaging timeout), so NOTHING in
//! this module may run on the UI thread. Call it from a background thread only.

use accessibility_sys::*;
use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::string::CFString;

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
}
