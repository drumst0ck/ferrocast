//! Global keystroke listener for snippet auto-expansion (CGEventTap, listen-only).
//!
//! Requires Accessibility / Input Monitoring permission. The tap keeps a small
//! buffer of recently typed characters; when it ends with a registered keyword
//! the `on_match` callback runs on a worker thread.

use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop};
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType, CallbackResult,
    EventField,
};
use foreign_types_shared::ForeignType;
use std::sync::{Arc, Mutex, RwLock};

extern "C" {
    fn CGEventKeyboardGetUnicodeString(event: *const std::ffi::c_void, max_len: usize, actual_len: *mut usize, buf: *mut u16);
}

fn event_chars(ev: &CGEvent) -> String {
    let mut buf = [0u16; 8];
    let mut len = 0usize;
    unsafe { CGEventKeyboardGetUnicodeString(ev.as_ptr() as *const _, buf.len(), &mut len, buf.as_mut_ptr()) };
    String::from_utf16_lossy(&buf[..len.min(buf.len())])
}

/// Pure matching logic, testable without the OS.
#[derive(Default)]
pub struct Matcher {
    buffer: String,
}

impl Matcher {
    pub fn reset(&mut self) {
        self.buffer.clear();
    }

    pub fn backspace(&mut self) {
        self.buffer.pop();
    }

    /// Feed typed text; returns the matched keyword, if any.
    pub fn push(&mut self, s: &str, keywords: &[String]) -> Option<String> {
        self.buffer.push_str(s);
        let n = self.buffer.chars().count();
        if n > 64 {
            self.buffer = self.buffer.chars().skip(n - 64).collect();
        }
        let hit = keywords.iter().filter(|k| !k.is_empty() && self.buffer.ends_with(k.as_str())).max_by_key(|k| k.len()).cloned();
        if hit.is_some() {
            self.buffer.clear();
        }
        hit
    }
}

pub type Keywords = Arc<RwLock<Vec<String>>>;

/// Start the listener thread. `enabled` is consulted per keystroke so the
/// app can pause expansion (e.g. while its own window is focused).
pub fn start(
    keywords: Keywords,
    enabled: Arc<dyn Fn() -> bool + Send + Sync>,
    on_match: Arc<dyn Fn(String) + Send + Sync>,
) -> anyhow::Result<()> {
    std::thread::Builder::new().name("snippet-tap".into()).spawn(move || {
        let matcher = Mutex::new(Matcher::default());
        let tap = CGEventTap::new(
            CGEventTapLocation::Session,
            CGEventTapPlacement::HeadInsertEventTap,
            CGEventTapOptions::ListenOnly,
            vec![CGEventType::KeyDown, CGEventType::LeftMouseDown, CGEventType::RightMouseDown],
            move |_proxy, ty, ev| {
                let mut m = matcher.lock().unwrap();
                match ty {
                    CGEventType::KeyDown => {
                        if ev.get_integer_value_field(EventField::EVENT_SOURCE_USER_DATA) == crate::keyboard::SYNTHETIC_MARKER {
                            return CallbackResult::Keep;
                        }
                        let flags = ev.get_flags();
                        if flags.intersects(CGEventFlags::CGEventFlagCommand | CGEventFlags::CGEventFlagControl) {
                            m.reset();
                            return CallbackResult::Keep;
                        }
                        let code = ev.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) as u16;
                        match code {
                            51 => m.backspace(),
                            36 | 48 | 53 | 76 | 115..=126 => m.reset(), // return, tab, esc, enter, nav keys
                            _ => {
                                if !enabled() {
                                    m.reset();
                                    return CallbackResult::Keep;
                                }
                                let s = event_chars(ev);
                                let kws = keywords.read().unwrap();
                                if let Some(k) = m.push(&s, &kws) {
                                    let cb = on_match.clone();
                                    std::thread::spawn(move || cb(k));
                                }
                            }
                        }
                    }
                    _ => m.reset(),
                }
                CallbackResult::Keep
            },
        );
        let tap = match tap {
            Ok(t) => t,
            Err(()) => {
                eprintln!("ferrocast: could not create event tap (grant Accessibility / Input Monitoring)");
                return;
            }
        };
        unsafe {
            let src = tap.mach_port().create_runloop_source(0).expect("runloop source");
            CFRunLoop::get_current().add_source(&src, kCFRunLoopCommonModes);
            tap.enable();
            CFRunLoop::run_current();
        }
    })?;
    Ok(())
}

/// Replace the just-typed keyword with `text`: delete it, paste through the
/// clipboard (restoring the previous content), then move the cursor.
pub fn inject(keyword_len: usize, text: &str, cursor_from_end: Option<usize>) {
    use crate::{clipboard, keyboard};
    let saved = clipboard::read();
    keyboard::tap_key(keyboard::keycode::DELETE, keyword_len);
    std::thread::sleep(std::time::Duration::from_millis(30));
    clipboard::write_text(text);
    std::thread::sleep(std::time::Duration::from_millis(30));
    keyboard::paste();
    std::thread::sleep(std::time::Duration::from_millis(150));
    if let Some(n) = cursor_from_end {
        keyboard::tap_key(keyboard::keycode::LEFT, n);
    }
    if let Some(s) = saved {
        clipboard::write(&s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_keyword_suffix() {
        let kws = vec!["!sig".to_string(), ";;log".to_string()];
        let mut m = Matcher::default();
        assert_eq!(m.push("h", &kws), None);
        assert_eq!(m.push("i!si", &kws), None);
        assert_eq!(m.push("g", &kws), Some("!sig".into()));
        // buffer cleared after match
        assert_eq!(m.push("g", &kws), None);
    }

    #[test]
    fn backspace_edits_buffer() {
        let kws = vec!["abc".to_string()];
        let mut m = Matcher::default();
        m.push("abx", &kws);
        m.backspace();
        assert_eq!(m.push("c", &kws), Some("abc".into()));
    }
}
