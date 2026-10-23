//! Pin (S9): a borderless always-on-top panel showing a captured image at its size in points,
//! draggable by its background, zoomable 25–400 % (scroll, pinch, <kbd>+</kbd>/<kbd>-</kbd>,
//! <kbd>0</kbd> resets) and closed by <kbd>Esc</kbd>. Several pins can coexist.
//!
//! Everything AppKit runs on the main thread and is tracked in a main-thread registry. The zoom
//! maths is pure ([`PinState`], [`clamp_zoom`], [`points_size`], [`anchored_frame`], [`next_zoom`],
//! [`prev_zoom`], [`scroll_factor`]) and unit-tested; the AppKit code only forwards events to it.
//! Frames are in Cocoa (bottom-left) global points; the maths is axis-independent.

pub const MIN_ZOOM: f64 = 0.25;
pub const MAX_ZOOM: f64 = 4.0;
/// Keyboard zoom ladder (1.0 is "captured size").
const ZOOM_STEPS: [f64; 9] = [0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0, 4.0];

/// Axis-aligned frame in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Frame {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    fn contains(&self, p: (f64, f64)) -> bool {
        p.0 >= self.x && p.0 <= self.x + self.w && p.1 >= self.y && p.1 <= self.y + self.h
    }

    fn center(&self) -> (f64, f64) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
}

/// Zoom limited to 25–400 %; NaN falls back to 100 %.
pub fn clamp_zoom(z: f64) -> f64 {
    if z.is_nan() {
        return 1.0;
    }
    z.clamp(MIN_ZOOM, MAX_ZOOM)
}

/// Captured size in points: pixels divided by the display scale (a bad scale counts as 1).
pub fn points_size(pixels: (u32, u32), scale: f64) -> (f64, f64) {
    let s = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
    (f64::from(pixels.0) / s, f64::from(pixels.1) / s)
}

/// Frame after a zoom change from `old_zoom` to `new_zoom` that keeps the image point under
/// `anchor` (global points) under it. `frame` is the current frame; `base` the size at 100 %.
pub fn anchored_frame(frame: Frame, anchor: (f64, f64), base: (f64, f64), new_zoom: f64) -> Frame {
    let z = clamp_zoom(new_zoom);
    let (w, h) = (base.0 * z, base.1 * z);
    let fx = if frame.w > 0.0 { (anchor.0 - frame.x) / frame.w } else { 0.5 };
    let fy = if frame.h > 0.0 { (anchor.1 - frame.y) / frame.h } else { 0.5 };
    Frame::new(anchor.0 - fx * w, anchor.1 - fy * h, w, h)
}

/// Next larger step of the keyboard ladder (stays at 400 %).
pub fn next_zoom(z: f64) -> f64 {
    ZOOM_STEPS.iter().copied().find(|s| *s > z + 1e-6).unwrap_or(MAX_ZOOM)
}

/// Next smaller step of the keyboard ladder (stays at 25 %).
pub fn prev_zoom(z: f64) -> f64 {
    ZOOM_STEPS.iter().rev().copied().find(|s| *s < z - 1e-6).unwrap_or(MIN_ZOOM)
}

/// Multiplicative zoom factor for a scroll-wheel delta; trackpads (`precise`) report finer deltas.
pub fn scroll_factor(dy: f64, precise: bool) -> f64 {
    let k = if precise { 0.01 } else { 0.05 };
    (dy * k).exp()
}

/// One pin's zoom state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PinState {
    base: (f64, f64),
    zoom: f64,
}

impl PinState {
    pub fn new(base: (f64, f64)) -> Self {
        Self { base, zoom: 1.0 }
    }

    pub fn zoom(&self) -> f64 {
        self.zoom
    }

    pub fn size(&self) -> (f64, f64) {
        (self.base.0 * self.zoom, self.base.1 * self.zoom)
    }

    /// Set the zoom (clamped) and return the new frame anchored at `anchor`.
    pub fn set_zoom(&mut self, frame: Frame, anchor: (f64, f64), zoom: f64) -> Frame {
        self.zoom = clamp_zoom(zoom);
        anchored_frame(frame, anchor, self.base, self.zoom)
    }

    pub fn zoom_by(&mut self, frame: Frame, anchor: (f64, f64), factor: f64) -> Frame {
        self.set_zoom(frame, anchor, self.zoom * factor)
    }

    pub fn step_in(&mut self, frame: Frame, anchor: (f64, f64)) -> Frame {
        self.set_zoom(frame, anchor, next_zoom(self.zoom))
    }

    pub fn step_out(&mut self, frame: Frame, anchor: (f64, f64)) -> Frame {
        self.set_zoom(frame, anchor, prev_zoom(self.zoom))
    }

    pub fn reset(&mut self, frame: Frame, anchor: (f64, f64)) -> Frame {
        self.set_zoom(frame, anchor, 1.0)
    }
}

/// Anchor for keyboard zoom: the pointer when it is over the pin, else the pin's centre.
pub fn key_anchor(frame: Frame, pointer: (f64, f64)) -> (f64, f64) {
    if frame.contains(pointer) {
        pointer
    } else {
        frame.center()
    }
}

// ------------------------------------------------------------------ AppKit

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::Path;

use objc2::rc::Retained;
use objc2::{define_class, msg_send, AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSBackingStoreType, NSEvent, NSFloatingWindowLevel, NSImage, NSPanel, NSScreen, NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

/// Handle of an open pin.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PinId(u64);

const KEY_ESC: u16 = 53;
const KEYS_PLUS: [u16; 2] = [24, 69];
const KEYS_MINUS: [u16; 2] = [27, 78];
const KEYS_ZERO: [u16; 2] = [29, 82];

struct Entry {
    panel: Retained<PinPanel>,
    state: PinState,
}

thread_local! {
    static PINS: RefCell<HashMap<u64, Entry>> = RefCell::new(HashMap::new());
    static NEXT_ID: Cell<u64> = const { Cell::new(1) };
}

struct ViewIvars {
    id: u64,
    image: Retained<NSImage>,
}

define_class!(
    /// Draws the image scaled to the view; leaves mouse-down alone so the panel drags by background.
    #[unsafe(super(objc2_app_kit::NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastPinView"]
    #[ivars = ViewIvars]
    struct PinView;

    impl PinView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(acceptsFirstResponder))]
        fn accepts_first_responder(&self) -> bool {
            true
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _e: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            self.ivars().image.drawInRect(self.bounds());
        }

        #[unsafe(method(scrollWheel:))]
        fn scroll_wheel(&self, e: &NSEvent) {
            let f = scroll_factor(e.scrollingDeltaY(), e.hasPreciseScrollingDeltas());
            update(self.ivars().id, |s, fr, p| s.zoom_by(fr, p, f));
        }

        #[unsafe(method(magnifyWithEvent:))]
        fn magnify(&self, e: &NSEvent) {
            let f = 1.0 + e.magnification();
            update(self.ivars().id, |s, fr, p| s.zoom_by(fr, p, f));
        }

        #[unsafe(method(keyDown:))]
        fn key_down(&self, e: &NSEvent) {
            let id = self.ivars().id;
            let code = e.keyCode();
            if code == KEY_ESC {
                close(PinId(id));
            } else if KEYS_PLUS.contains(&code) {
                update(id, PinState::step_in);
            } else if KEYS_MINUS.contains(&code) {
                update(id, PinState::step_out);
            } else if KEYS_ZERO.contains(&code) {
                update(id, PinState::reset);
            }
        }
    }
);

define_class!(
    /// Borderless panel that can take focus (for Esc and the zoom keys).
    #[unsafe(super(NSPanel))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastPinPanel"]
    struct PinPanel;

    impl PinPanel {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key(&self) -> bool {
            true
        }

        #[unsafe(method(canBecomeMainWindow))]
        fn can_become_main(&self) -> bool {
            false
        }
    }
);

fn to_frame(r: NSRect) -> Frame {
    Frame::new(r.origin.x, r.origin.y, r.size.width, r.size.height)
}

fn to_ns(f: Frame) -> NSRect {
    NSRect::new(NSPoint::new(f.x, f.y), NSSize::new(f.w, f.h))
}

/// Apply a zoom change to pin `id`, anchored under the mouse when it is over the pin, else at
/// the pin's centre.
fn update(id: u64, change: impl FnOnce(&mut PinState, Frame, (f64, f64)) -> Frame) {
    let target = PINS.with(|p| p.borrow().get(&id).map(|e| (e.panel.clone(), e.state)));
    let Some((panel, mut state)) = target else { return };
    let frame = to_frame(panel.frame());
    let m = NSEvent::mouseLocation();
    let anchor = key_anchor(frame, (m.x, m.y));
    let next = change(&mut state, frame, anchor);
    panel.setFrame_display(to_ns(next), true);
    PINS.with(|p| {
        if let Some(e) = p.borrow_mut().get_mut(&id) {
            e.state = state;
        }
    });
}

/// Open a pin of the PNG at `png_path`, `pixel_size` pixels captured on a display of `scale`
/// (shown at pixel size / scale points, centred on the main screen). Main thread only.
pub fn open(mtm: MainThreadMarker, png_path: &Path, pixel_size: (u32, u32), scale: f64) -> Result<PinId, String> {
    if pixel_size.0 == 0 || pixel_size.1 == 0 {
        return Err("Pin: the image has no pixels".to_string());
    }
    if !png_path.is_file() {
        return Err(format!("Pin: {} is not a file", png_path.display()));
    }
    let path = png_path.to_str().ok_or("Pin: the path is not valid UTF-8")?;
    let image =
        NSImage::initWithContentsOfFile(NSImage::alloc(), &NSString::from_str(path)).ok_or("Pin: the file is not a readable image")?;
    let base = points_size(pixel_size, scale);
    image.setSize(NSSize::new(base.0, base.1));

    let area =
        NSScreen::mainScreen(mtm).map(|s| s.visibleFrame()).unwrap_or(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1440.0, 900.0)));
    let frame =
        Frame::new(area.origin.x + (area.size.width - base.0) / 2.0, area.origin.y + (area.size.height - base.1) / 2.0, base.0, base.1);

    let id = NEXT_ID.with(|n| {
        let v = n.get();
        n.set(v + 1);
        v
    });
    let style = NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel;
    let panel: Retained<PinPanel> = unsafe {
        msg_send![
            super(mtm.alloc::<PinPanel>().set_ivars(())),
            initWithContentRect: to_ns(frame),
            styleMask: style,
            backing: NSBackingStoreType::Buffered,
            defer: false
        ]
    };
    unsafe { panel.setReleasedWhenClosed(false) };
    panel.setLevel(NSFloatingWindowLevel);
    panel.setFloatingPanel(true);
    panel.setHidesOnDeactivate(false);
    panel.setHasShadow(true);
    panel.setMovableByWindowBackground(true);
    panel.setExcludedFromWindowsMenu(true);
    panel.setCollectionBehavior(NSWindowCollectionBehavior::CanJoinAllSpaces | NSWindowCollectionBehavior::FullScreenAuxiliary);
    let bounds = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(base.0, base.1));
    let view: Retained<PinView> =
        unsafe { msg_send![super(mtm.alloc::<PinView>().set_ivars(ViewIvars { id, image })), initWithFrame: bounds] };
    panel.setContentView(Some(&view));
    panel.makeKeyAndOrderFront(None);
    panel.makeFirstResponder(Some(&view));
    PINS.with(|p| p.borrow_mut().insert(id, Entry { panel, state: PinState::new(base) }));
    Ok(PinId(id))
}

/// Close one pin; unknown or already closed ids are ignored.
pub fn close(id: PinId) {
    let entry = PINS.with(|p| p.borrow_mut().remove(&id.0));
    if let Some(e) = entry {
        e.panel.orderOut(None);
        e.panel.close();
    }
}

/// Close every pin.
pub fn close_all() {
    let all: Vec<Entry> = PINS.with(|p| p.borrow_mut().drain().map(|(_, e)| e).collect());
    for e in all {
        e.panel.orderOut(None);
        e.panel.close();
    }
}

/// Number of open pins (main thread).
pub fn count() -> usize {
    PINS.with(|p| p.borrow().len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_is_clamped_to_25_and_400_percent() {
        assert_eq!(clamp_zoom(0.01), 0.25);
        assert_eq!(clamp_zoom(9.0), 4.0);
        assert_eq!(clamp_zoom(1.5), 1.5);
        assert_eq!(clamp_zoom(f64::NAN), 1.0);
    }

    #[test]
    fn size_in_points_divides_pixels_by_scale() {
        assert_eq!(points_size((800, 600), 2.0), (400.0, 300.0));
        assert_eq!(points_size((801, 600), 2.0), (400.5, 300.0));
        assert_eq!(points_size((800, 600), 1.0), (800.0, 600.0));
        assert_eq!(points_size((800, 600), 0.0), (800.0, 600.0));
    }

    #[test]
    fn zoom_keeps_the_point_under_the_pointer_fixed() {
        let f = Frame::new(100.0, 200.0, 400.0, 300.0);
        // pointer at the centre: the centre stays put
        let g = anchored_frame(f, (300.0, 350.0), (400.0, 300.0), 2.0);
        assert_eq!(g, Frame::new(-100.0, 50.0, 800.0, 600.0));
        // pointer at the bottom-left corner: that corner stays put
        let g = anchored_frame(f, (100.0, 200.0), (400.0, 300.0), 0.5);
        assert_eq!(g, Frame::new(100.0, 200.0, 200.0, 150.0));
        // pointer a quarter across: it stays a quarter across
        let g = anchored_frame(f, (200.0, 275.0), (400.0, 300.0), 2.0);
        assert_eq!((200.0 - g.x) / g.w, 0.25);
        assert_eq!((275.0 - g.y) / g.h, 0.25);
    }

    #[test]
    fn anchored_frame_clamps_the_zoom() {
        let f = Frame::new(0.0, 0.0, 100.0, 100.0);
        assert_eq!(anchored_frame(f, (0.0, 0.0), (100.0, 100.0), 99.0).w, 400.0);
        assert_eq!(anchored_frame(f, (0.0, 0.0), (100.0, 100.0), 0.0).w, 25.0);
    }

    #[test]
    fn steps_walk_the_ladder_and_stop_at_the_limits() {
        assert_eq!(next_zoom(1.0), 1.25);
        assert_eq!(prev_zoom(1.0), 0.75);
        assert_eq!(next_zoom(1.1), 1.25, "off-ladder values snap to the next step");
        assert_eq!(prev_zoom(1.1), 1.0);
        assert_eq!(next_zoom(4.0), 4.0);
        assert_eq!(prev_zoom(0.25), 0.25);
        assert_eq!(next_zoom(0.25), 0.5);
        assert_eq!(prev_zoom(4.0), 3.0);
    }

    #[test]
    fn scroll_up_zooms_in_and_down_zooms_out() {
        assert!(scroll_factor(3.0, true) > 1.0);
        assert!(scroll_factor(-3.0, true) < 1.0);
        assert_eq!(scroll_factor(0.0, false), 1.0);
        assert!(scroll_factor(1.0, false) > scroll_factor(1.0, true));
    }

    #[test]
    fn state_zooms_resets_and_never_leaves_the_range() {
        let mut s = PinState::new((400.0, 300.0));
        let mut f = Frame::new(0.0, 0.0, 400.0, 300.0);
        let c = (200.0, 150.0);
        f = s.step_in(f, c);
        assert_eq!((s.zoom(), s.size()), (1.25, (500.0, 375.0)));
        assert_eq!(f, Frame::new(-50.0, -37.5, 500.0, 375.0));
        for _ in 0..20 {
            f = s.step_in(f, c);
        }
        assert_eq!(s.zoom(), 4.0);
        for _ in 0..20 {
            f = s.zoom_by(f, c, 0.5);
        }
        assert_eq!(s.zoom(), 0.25);
        f = s.step_out(f, c);
        assert_eq!(s.zoom(), 0.25);
        f = s.reset(f, c);
        assert_eq!((s.zoom(), f.w, f.h), (1.0, 400.0, 300.0));
        assert_eq!((f.x + f.w / 2.0, f.y + f.h / 2.0), c, "reset keeps the anchor");
    }

    #[test]
    fn keyboard_anchor_is_the_pointer_over_the_pin_else_the_centre() {
        let f = Frame::new(0.0, 0.0, 100.0, 50.0);
        assert_eq!(key_anchor(f, (10.0, 20.0)), (10.0, 20.0));
        assert_eq!(key_anchor(f, (500.0, 20.0)), (50.0, 25.0));
    }
}
