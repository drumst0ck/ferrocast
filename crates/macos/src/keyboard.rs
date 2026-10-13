//! Synthetic keyboard input: paste (⌘V), backspaces, arrow keys, media keys.

use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGKeyCode};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

/// Marker placed in EVENT_SOURCE_USER_DATA so our own event tap ignores
/// events we synthesize.
pub const SYNTHETIC_MARKER: i64 = 0x0F3C_A57;

pub mod keycode {
    pub const V: u16 = 9;
    pub const DELETE: u16 = 51;
    pub const LEFT: u16 = 123;
    pub const RETURN: u16 = 36;
}

fn post_key(code: CGKeyCode, flags: CGEventFlags) {
    let Ok(src) = CGEventSource::new(CGEventSourceStateID::HIDSystemState) else { return };
    for down in [true, false] {
        if let Ok(ev) = CGEvent::new_keyboard_event(src.clone(), code, down) {
            ev.set_flags(flags);
            ev.set_integer_value_field(core_graphics::event::EventField::EVENT_SOURCE_USER_DATA, SYNTHETIC_MARKER);
            ev.post(CGEventTapLocation::HID);
        }
    }
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
    post_key(keycode::V, CGEventFlags::CGEventFlagCommand);
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
