//! Drag + key "wheel": the state machine that turns raw mouse/keyboard/pasteboard samples
//! into wheel events, and the hit-testing of wheel segments. Pure; the app feeds samples.

/// Which wheel to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WheelKind {
    /// Conversions and image tools.
    Convert,
    /// File actions (copy, move, terminal, AirDrop…).
    Actions,
}

/// One sample of the world, taken every few milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Sample {
    pub left_down: bool,
    /// changeCount of the system drag pasteboard.
    pub drag_count: i64,
    /// The drag pasteboard holds file URLs.
    pub has_files: bool,
    pub convert_key: bool,
    pub actions_key: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WheelEvent {
    /// A key was tapped during a file drag: open this wheel.
    Open(WheelKind),
    /// The mouse was released while a wheel was open.
    Release,
}

/// Remembers the drag in progress between samples.
#[derive(Debug, Default)]
pub struct Tracker {
    down_count: Option<i64>,
    convert_was: bool,
    actions_was: bool,
    open: bool,
}

impl Tracker {
    /// Feed one sample; returns an event when something happened.
    pub fn step(&mut self, s: Sample) -> Option<WheelEvent> {
        let mut ev = None;
        if s.left_down {
            // the drag pasteboard changes when a drag starts after the press
            let base = *self.down_count.get_or_insert(s.drag_count);
            let dragging_files = s.drag_count != base && s.has_files;
            if dragging_files && !self.open {
                if s.convert_key && !self.convert_was {
                    ev = Some(WheelEvent::Open(WheelKind::Convert));
                } else if s.actions_key && !self.actions_was {
                    ev = Some(WheelEvent::Open(WheelKind::Actions));
                }
                if ev.is_some() {
                    self.open = true;
                }
            }
        } else {
            if self.open {
                ev = Some(WheelEvent::Release);
            }
            self.down_count = None;
            self.open = false;
        }
        self.convert_was = s.convert_key;
        self.actions_was = s.actions_key;
        ev
    }
}

/// `NSEventModifierFlags` bit for a key name used in Settings; 0 (never held) when unknown.
pub fn key_mask(name: &str) -> usize {
    match name {
        "Shift" => 1 << 17,
        "Control" => 1 << 18,
        "Option" => 1 << 19,
        "Command" => 1 << 20,
        _ => 0,
    }
}

/// Segment under `p` for a ring of `n` equal segments starting at 12 o'clock, clockwise.
/// `p` is relative to the centre, y down. `None` outside the ring or in the hole.
pub fn segment_at(px: f32, py: f32, inner: f32, outer: f32, n: usize) -> Option<usize> {
    if n == 0 {
        return None;
    }
    let r = (px * px + py * py).sqrt();
    if r < inner || r > outer {
        return None;
    }
    // angle from 12 o'clock, clockwise, in [0, 2π)
    let a = px.atan2(-py).rem_euclid(std::f32::consts::TAU);
    let step = std::f32::consts::TAU / n as f32;
    // segment i is centred on i * step
    Some(((a + step / 2.0) / step) as usize % n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(left: bool, count: i64, files: bool, ck: bool, ak: bool) -> Sample {
        Sample { left_down: left, drag_count: count, has_files: files, convert_key: ck, actions_key: ak }
    }

    #[test]
    fn opens_only_on_a_key_tap_during_a_file_drag() {
        let mut t = Tracker::default();
        assert_eq!(t.step(s(true, 5, true, false, false)), None); // press, no drag yet
        assert_eq!(t.step(s(true, 5, true, true, false)), None); // shift without a drag
        assert_eq!(t.step(s(true, 5, true, false, false)), None);
        assert_eq!(t.step(s(true, 6, true, false, false)), None); // drag started
        assert_eq!(t.step(s(true, 6, true, true, false)), Some(WheelEvent::Open(WheelKind::Convert)));
        assert_eq!(t.step(s(true, 6, true, true, false)), None); // held, not re-opened
        assert_eq!(t.step(s(false, 6, true, false, false)), Some(WheelEvent::Release));
        assert_eq!(t.step(s(false, 6, true, false, false)), None);
    }

    #[test]
    fn ignores_text_drags_and_holds_from_before_the_press() {
        let mut t = Tracker::default();
        t.step(s(true, 1, false, false, false));
        assert_eq!(t.step(s(true, 2, false, true, false)), None, "text drag");
        t.step(s(false, 2, false, false, false));
        // key already held when the drag starts: needs a fresh tap
        t.step(s(true, 2, true, true, false));
        assert_eq!(t.step(s(true, 3, true, true, false)), None);
        assert_eq!(t.step(s(true, 3, true, false, true)), Some(WheelEvent::Open(WheelKind::Actions)));
        // release without a wheel produces nothing
        let mut u = Tracker::default();
        u.step(s(true, 1, true, false, false));
        assert_eq!(u.step(s(false, 2, true, false, false)), None);
    }

    #[test]
    fn key_masks() {
        assert_eq!(key_mask("Shift"), 1 << 17);
        assert_eq!(key_mask("Command"), 1 << 20);
        assert_eq!(key_mask("Off"), 0);
    }

    #[test]
    fn segments() {
        // 4 segments: top, right, bottom, left
        assert_eq!(segment_at(0.0, -100.0, 40.0, 160.0, 4), Some(0));
        assert_eq!(segment_at(100.0, 0.0, 40.0, 160.0, 4), Some(1));
        assert_eq!(segment_at(0.0, 100.0, 40.0, 160.0, 4), Some(2));
        assert_eq!(segment_at(-100.0, 0.0, 40.0, 160.0, 4), Some(3));
        assert_eq!(segment_at(10.0, 10.0, 40.0, 160.0, 4), None, "hole");
        assert_eq!(segment_at(200.0, 0.0, 40.0, 160.0, 4), None, "outside");
        // boundary just left of 12 o'clock still belongs to segment 0
        assert_eq!(segment_at(-10.0, -100.0, 40.0, 160.0, 4), Some(0));
    }
}
