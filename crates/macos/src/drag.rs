//! Global, permission-free sampling of the mouse, modifier keys and the system drag
//! pasteboard, for the drop wheel. Safe to call from a background thread.

use objc2::rc::Retained;
use objc2_app_kit::{NSEvent, NSPasteboard, NSPasteboardNameDrag, NSPasteboardTypeFileURL};
use std::path::PathBuf;

/// Is the left mouse button held anywhere on screen?
pub fn left_down() -> bool {
    NSEvent::pressedMouseButtons() & 1 == 1
}

/// Raw `NSEventModifierFlags` bits currently held (Shift = 1<<17, Control = 1<<18…).
pub fn modifier_bits() -> usize {
    NSEvent::modifierFlags_class().bits()
}

fn pb() -> Retained<NSPasteboard> {
    NSPasteboard::pasteboardWithName(unsafe { NSPasteboardNameDrag })
}

/// changeCount of the drag pasteboard; it changes when a new drag starts.
pub fn drag_count() -> i64 {
    pb().changeCount() as i64
}

/// Files on the drag pasteboard (the ones being dragged, during a drag).
pub fn drag_files() -> Vec<PathBuf> {
    unsafe {
        pb().pasteboardItems()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|it| it.stringForType(NSPasteboardTypeFileURL))
                    .filter_map(|s| crate::clipboard::file_url_to_path(&s.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn samples_without_a_drag() {
        // Only that sampling works: whether the physical left button is down is the user's, not the
        // test's (asserting `!left_down()` failed whenever the mouse was in use while the suite ran).
        let _ = super::left_down();
        let _ = super::modifier_bits();
        let c = super::drag_count();
        assert_eq!(super::drag_count(), c, "stable while nothing is dragged");
        let _ = super::drag_files();
    }
}
