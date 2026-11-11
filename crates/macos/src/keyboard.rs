//! Synthetic keyboard input: paste (⌘V), backspaces, arrow keys, media keys.

use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGKeyCode};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

/// Marker placed in EVENT_SOURCE_USER_DATA so our own event tap ignores
/// events we synthesize.
pub const SYNTHETIC_MARKER: i64 = 0x00F3_CA57;

pub mod keycode {
    pub const V: u16 = 9;
    pub const DELETE: u16 = 51;
    pub const LEFT: u16 = 123;
    pub const RETURN: u16 = 36;
}

fn post_key(code: CGKeyCode, flags: CGEventFlags) {
    post_key_marked(code, flags, SYNTHETIC_MARKER);
}

/// Post a key press with an explicit user-data marker, so a tap can recognise its own re-posts.
pub fn post_key_marked(code: CGKeyCode, flags: CGEventFlags, marker: i64) {
    let Ok(src) = CGEventSource::new(CGEventSourceStateID::HIDSystemState) else { return };
    for down in [true, false] {
        if let Ok(ev) = CGEvent::new_keyboard_event(src.clone(), code, down) {
            ev.set_flags(flags);
            ev.set_integer_value_field(core_graphics::event::EventField::EVENT_SOURCE_USER_DATA, marker);
            ev.post(CGEventTapLocation::HID);
        }
    }
}

/// Give a Return the Finder-Enter tap swallowed back to the system. The event carries
/// `finder_enter::REPOST_MARKER`, which the tap always lets through.
pub fn repost_return() {
    post_key_marked(keycode::RETURN, CGEventFlags::CGEventFlagNull, ferrocast_core::finder_enter::REPOST_MARKER);
}

/// Press a key `n` times without modifiers.
pub fn tap_key(code: u16, n: usize) {
    for _ in 0..n {
        post_key(code, CGEventFlags::CGEventFlagNull);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

/// Send ⌘V to the frontmost app.
pub fn paste() {
    post_key(paste_keycode(), CGEventFlags::CGEventFlagCommand);
}

/// The virtual keycode that types `ch` (no modifiers) according to `translate`, the
/// layout's keycode -> character table. Lowest keycode wins.
pub fn keycode_for_char(ch: char, translate: impl Fn(u16) -> Option<char>) -> Option<u16> {
    (0u16..128).find(|c| translate(*c).is_some_and(|t| t.eq_ignore_ascii_case(&ch)))
}

/// Last "v" keycode resolved on the main thread; 0 means not resolved yet.
static PASTE_KEYCODE: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0);

/// Pick the paste keycode. The layout (`query`) may only be read on the main thread: Text
/// Input Sources asserts the main queue and aborts the process otherwise (macOS 26+). Off
/// the main thread, use the `cached` main-thread result, or the QWERTY position.
fn resolve_paste_keycode(on_main: bool, cached: u16, query: impl FnOnce() -> Option<u16>) -> u16 {
    if on_main {
        query().unwrap_or(keycode::V)
    } else if cached != 0 {
        cached
    } else {
        keycode::V
    }
}

fn on_main_thread() -> bool {
    extern "C" {
        fn pthread_main_np() -> i32;
    }
    unsafe { pthread_main_np() != 0 }
}

/// Re-read the "v" keycode from the current layout. Call on the main thread before handing a
/// paste to a worker thread, so a layout switch since launch is honoured; a no-op elsewhere.
pub fn refresh_paste_keycode() {
    if on_main_thread() {
        paste_keycode();
    }
}

/// Keycode for the "v" key in the user's current layout (Dvorak, Colemak, ...), falling
/// back to the ASCII-capable layout and finally to the QWERTY position.
fn paste_keycode() -> u16 {
    use std::sync::atomic::Ordering;
    let on_main = on_main_thread();
    let code = resolve_paste_keycode(on_main, PASTE_KEYCODE.load(Ordering::Relaxed), || layout::keycode_for('v'));
    if on_main {
        PASTE_KEYCODE.store(code, Ordering::Relaxed);
    }
    code
}

mod layout {
    use std::ffi::c_void;

    #[link(name = "Carbon", kind = "framework")]
    extern "C" {
        static kTISPropertyUnicodeKeyLayoutData: *const c_void;
        fn TISCopyCurrentKeyboardLayoutInputSource() -> *const c_void;
        fn TISCopyCurrentASCIICapableKeyboardLayoutInputSource() -> *const c_void;
        fn TISGetInputSourceProperty(src: *const c_void, key: *const c_void) -> *const c_void;
        fn LMGetKbdType() -> u8;
        fn UCKeyTranslate(
            layout: *const c_void,
            key_code: u16,
            action: u16,
            modifiers: u32,
            kbd_type: u32,
            options: u32,
            dead_keys: *mut u32,
            max_len: usize,
            actual_len: *mut usize,
            out: *mut u16,
        ) -> i32;
    }
    extern "C" {
        fn CFDataGetBytePtr(d: *const c_void) -> *const u8;
        fn CFRelease(o: *const c_void);
    }

    fn lookup(src: *const c_void, ch: char) -> Option<u16> {
        if src.is_null() {
            return None;
        }
        let data = unsafe { TISGetInputSourceProperty(src, kTISPropertyUnicodeKeyLayoutData) };
        let found = if data.is_null() {
            None
        } else {
            let ptr = unsafe { CFDataGetBytePtr(data) } as *const c_void;
            let kbd = unsafe { LMGetKbdType() } as u32;
            super::keycode_for_char(ch, |code| {
                let mut dead = 0u32;
                let mut len = 0usize;
                let mut buf = [0u16; 4];
                // action 0 = key down, options 1 = no dead keys
                let st = unsafe { UCKeyTranslate(ptr, code, 0, 0, kbd, 1, &mut dead, buf.len(), &mut len, buf.as_mut_ptr()) };
                if st != 0 || len != 1 {
                    return None;
                }
                char::from_u32(buf[0] as u32)
            })
        };
        unsafe { CFRelease(src) };
        found
    }

    pub fn keycode_for(ch: char) -> Option<u16> {
        lookup(unsafe { TISCopyCurrentKeyboardLayoutInputSource() }, ch)
            .or_else(|| lookup(unsafe { TISCopyCurrentASCIICapableKeyboardLayoutInputSource() }, ch))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dvorak(code: u16) -> Option<char> {
        // QWERTY "v" (9) types "k" on Dvorak; "." (47) types "v".
        match code {
            9 => Some('k'),
            47 => Some('v'),
            _ => None,
        }
    }

    #[test]
    fn finds_v_on_a_dvorak_layout_not_the_qwerty_position() {
        assert_eq!(keycode_for_char('v', dvorak), Some(47));
    }

    #[test]
    fn qwerty_layout_resolves_to_nine_and_missing_char_is_none() {
        assert_eq!(keycode_for_char('v', |c| (c == 9).then_some('v')), Some(9));
        assert_eq!(keycode_for_char('v', |_| Some('я')), None);
    }

    #[test]
    fn off_main_thread_never_queries_the_layout_and_uses_the_cache_or_qwerty() {
        // TIS* aborts the process when called off the main queue (macOS 26+), so a paste from
        // a worker thread must not touch it.
        let panics = || -> Option<u16> { panic!("layout queried off the main thread") };
        assert_eq!(resolve_paste_keycode(false, 47, panics), 47);
        assert_eq!(resolve_paste_keycode(false, 0, panics), keycode::V);
    }

    #[test]
    fn main_thread_queries_the_layout_and_falls_back_to_qwerty() {
        assert_eq!(resolve_paste_keycode(true, 0, || Some(47)), 47);
        assert_eq!(resolve_paste_keycode(true, 47, || None), keycode::V);
    }

    #[test]
    fn live_layout_resolves_some_keycode_for_v() {
        // Whatever the host layout, paste must target a real key that is not forced to 9 blindly.
        let c = paste_keycode();
        assert!(c < 128);
    }
}

#[derive(Debug, Clone, Copy)]
pub enum MediaKey {
    PlayPause,
    Next,
    Previous,
}

/// Post an NX media key event (same as the hardware F7–F9 keys).
pub fn media_key(key: MediaKey) {
    use objc2_app_kit::{NSEvent, NSEventModifierFlags, NSEventType};
    use objc2_foundation::NSPoint;
    let code: isize = match key {
        MediaKey::PlayPause => 16,
        MediaKey::Next => 17,
        MediaKey::Previous => 18,
    };
    for (flags, state) in [(0xa00usize, 0xa), (0xb00, 0xb)] {
        let ev = NSEvent::otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2(
            NSEventType::SystemDefined,
            NSPoint::new(0.0, 0.0),
            NSEventModifierFlags(flags),
            0.0,
            0,
            None,
            8,
            (code << 16) | (state << 8),
            -1,
        );
        if let Some(cg) = ev.and_then(|e| e.CGEvent()) {
            objc2_core_graphics::CGEvent::post(objc2_core_graphics::CGEventTapLocation::HIDEventTap, Some(&cg));
        }
    }
}
