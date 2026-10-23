//! Synthetic scrolling for Capture Scrolling (S10): posts pixel scroll-wheel events over a point
//! and reads the Esc key. Posting events needs Accessibility; nothing here needs Screen Recording.

use objc2_core_foundation::CGPoint;
use objc2_core_graphics::{CGEvent, CGEventSource, CGEventSourceStateID, CGEventTapLocation, CGScrollEventUnit};
use std::fmt;

/// Virtual key code of Esc.
const KEY_ESCAPE: u16 = 53;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollError {
    /// Accessibility is off for Ferrocast, so events cannot be posted.
    NotTrusted,
    /// Core Graphics refused to create the event.
    EventFailed,
}

impl fmt::Display for ScrollError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotTrusted => write!(f, "Accessibility is off for Ferrocast"),
            Self::EventFailed => write!(f, "could not create the scroll event"),
        }
    }
}

impl std::error::Error for ScrollError {}

/// Is Accessibility granted? With `prompt` the system dialog is shown when it is not.
pub fn ensure_trusted(prompt: bool) -> Result<(), ScrollError> {
    if crate::windows::accessibility_trusted(prompt) {
        Ok(())
    } else {
        Err(ScrollError::NotTrusted)
    }
}

/// Post one pixel scroll over the global top-left point `(x, y)`. A negative `dy` moves the
/// content up (reveals what is below).
pub fn post_scroll(x: f64, y: f64, dy: i32) -> Result<(), ScrollError> {
    ensure_trusted(false)?;
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState);
    let ev = CGEvent::new_scroll_wheel_event2(source.as_deref(), CGScrollEventUnit::Pixel, 1, dy, 0, 0).ok_or(ScrollError::EventFailed)?;
    CGEvent::set_location(Some(&ev), CGPoint::new(x, y));
    CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&ev));
    Ok(())
}

/// Is Esc held down right now (polled; needs no permission)?
pub fn escape_pressed() -> bool {
    CGEventSource::key_state(CGEventSourceStateID::HIDSystemState, KEY_ESCAPE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_have_readable_text() {
        assert_eq!(ScrollError::NotTrusted.to_string(), "Accessibility is off for Ferrocast");
        assert_eq!(ScrollError::EventFailed.to_string(), "could not create the scroll event");
    }
}
