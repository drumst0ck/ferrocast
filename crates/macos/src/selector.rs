//! Selection overlay for Capture Area / Capture Window (S5): a dimmed borderless panel on every
//! display, a crosshair, a `W × H` pixel label, a hole for the selected area, <kbd>Space</kbd> to
//! toggle window mode and <kbd>Esc</kbd> to cancel.
//!
//! `select` runs on the main thread and blocks (pumping AppKit events itself) until the user
//! finishes. The behaviour lives in [`SelectorState`] and [`layout_for`], both pure and unit-tested;
//! the AppKit code only forwards events to them and draws what they say. All geometry is in
//! top-left global points (see `ferrocast_core::capture::region`).

use std::cell::{Cell, RefCell};

use ferrocast_core::capture::region::{flip_point, flip_rect, select_area, size_label, snap_window, Display, Point, Rect, Window};
use objc2::rc::Retained;
use objc2::{define_class, msg_send, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSBackingStoreType, NSBezierPath, NSColor, NSCursor, NSEvent, NSEventMask, NSFont, NSPanel, NSScreen, NSTextAlignment,
    NSTextField, NSView, NSWindingRule, NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_foundation::{NSDate, NSDefaultRunLoopMode, NSPoint, NSRect, NSSize, NSString};

/// Smallest dragged side, in points, that counts as an area (a plain click selects nothing).
const MIN_SIDE: f64 = 2.0;
/// Label box (estimated) used to keep it on the display, and its offset from the pointer.
const LABEL_W: f64 = 96.0;
const LABEL_H: f64 = 22.0;
const LABEL_OFFSET: f64 = 14.0;
const KEY_ESC: u16 = 53;
const KEY_SPACE: u16 = 49;

/// What the user chose.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Selection {
    /// Area in top-left global points, already clipped to one display.
    Area(Rect),
    /// A window id from the `windows` passed to [`select`].
    Window(u32),
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Area,
    Window,
}

/// Pure selection state machine. Feed it pointer and key events; read `outcome` when done.
#[derive(Clone, Debug)]
pub struct SelectorState {
    displays: Vec<Display>,
    windows: Vec<Window>,
    mode: Mode,
    /// Where the button went down in area mode, while it is held.
    drag_start: Option<Point>,
    /// The button is held in window mode.
    pressed: bool,
    pointer: Option<Point>,
    outcome: Option<Selection>,
}

impl SelectorState {
    pub fn new(displays: Vec<Display>, windows: Vec<Window>) -> Self {
        Self { displays, windows, mode: Mode::Area, drag_start: None, pressed: false, pointer: None, outcome: None }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn outcome(&self) -> Option<Selection> {
        self.outcome
    }

    pub fn pointer(&self) -> Option<Point> {
        self.pointer
    }

    /// The rectangle being dragged (unclipped), if any.
    fn drag_rect(&self) -> Option<(usize, Rect)> {
        let (a, b) = (self.drag_start?, self.pointer?);
        select_area(&self.displays, a, b)
    }

    /// Window under the pointer in window mode.
    pub fn hovered_window(&self) -> Option<&Window> {
        if self.mode != Mode::Window {
            return None;
        }
        snap_window(&self.windows, self.pointer?)
    }

    pub fn pointer_down(&mut self, p: Point) {
        if self.outcome.is_some() {
            return;
        }
        self.pointer = Some(p);
        match self.mode {
            Mode::Area => self.drag_start = Some(p),
            Mode::Window => self.pressed = true,
        }
    }

    pub fn pointer_move(&mut self, p: Point) {
        if self.outcome.is_none() {
            self.pointer = Some(p);
        }
    }

    pub fn pointer_up(&mut self, p: Point) {
        if self.outcome.is_some() {
            return;
        }
        self.pointer = Some(p);
        match self.mode {
            Mode::Area => {
                if let Some(start) = self.drag_start.take() {
                    if let Some((_, r)) = select_area(&self.displays, start, p) {
                        if r.w >= MIN_SIDE && r.h >= MIN_SIDE {
                            self.outcome = Some(Selection::Area(r));
                        }
                    }
                }
            }
            Mode::Window => {
                if std::mem::take(&mut self.pressed) {
                    if let Some(w) = snap_window(&self.windows, p) {
                        self.outcome = Some(Selection::Window(w.id));
                    }
                }
            }
        }
    }

    /// Space switches between area and window mode and drops any drag in progress.
    pub fn toggle_mode(&mut self) {
        if self.outcome.is_some() {
            return;
        }
        self.mode = match self.mode {
            Mode::Area => Mode::Window,
            Mode::Window => Mode::Area,
        };
        self.drag_start = None;
        self.pressed = false;
    }

    /// Esc: cancel from any state.
    pub fn cancel(&mut self) {
        if self.outcome.is_none() {
            self.outcome = Some(Selection::Cancelled);
        }
    }
}

/// What one display's overlay draws; all points are local to the display (top-left origin).
#[derive(Clone, Debug, PartialEq)]
pub struct OverlayLayout {
    /// Un-dimmed rectangle (dragged area or hovered window).
    pub hole: Option<Rect>,
    /// Crosshair centre, area mode only.
    pub crosshair: Option<Point>,
    /// `W × H` text and its top-left corner.
    pub label: Option<(String, Point)>,
}

/// Overlay content for display `index`; pure.
pub fn layout_for(state: &SelectorState, index: usize) -> OverlayLayout {
    let display = &state.displays[index];
    let origin = display.frame;
    let local = |r: Rect| Rect::new(r.x - origin.x, r.y - origin.y, r.w, r.h);
    let on_this = state.pointer.is_some_and(|p| origin.contains(p));

    let shown = match state.mode {
        Mode::Area => state.drag_rect().filter(|(i, _)| *i == index).map(|(_, r)| r),
        Mode::Window => state.hovered_window().and_then(|w| w.frame.intersection(&origin)),
    };
    let crosshair =
        (state.mode == Mode::Area && on_this).then(|| state.pointer.map(|p| Point { x: p.x - origin.x, y: p.y - origin.y })).flatten();
    let label = shown.and_then(|r| {
        // anchor the label at the pointer, flipped inside when it would leave the display
        let p = state.pointer.filter(|_| on_this)?;
        let (mut x, mut y) = (p.x - origin.x + LABEL_OFFSET, p.y - origin.y + LABEL_OFFSET);
        if x + LABEL_W > origin.w {
            x = p.x - origin.x - LABEL_OFFSET - LABEL_W;
        }
        if y + LABEL_H > origin.h {
            y = p.y - origin.y - LABEL_OFFSET - LABEL_H;
        }
        Some((size_label(r, display.scale), Point { x: x.max(0.0), y: y.max(0.0) }))
    });
    OverlayLayout { hole: shown.map(local), crosshair, label }
}

// ------------------------------------------------------------------ AppKit

thread_local! {
    static STATE: RefCell<Option<SelectorState>> = const { RefCell::new(None) };
    static PRIMARY_HEIGHT: Cell<f64> = const { Cell::new(0.0) };
    static VIEWS: RefCell<Vec<(Retained<OverlayView>, Retained<NSTextField>)>> = const { RefCell::new(Vec::new()) };
}

fn with_state(f: impl FnOnce(&mut SelectorState)) {
    STATE.with(|s| {
        if let Some(st) = s.borrow_mut().as_mut() {
            f(st);
        }
    });
    refresh();
}

/// Mouse position in top-left global points.
fn mouse_point() -> Point {
    let m = NSEvent::mouseLocation();
    flip_point(Point { x: m.x, y: m.y }, PRIMARY_HEIGHT.with(Cell::get))
}

struct ViewIvars {
    index: usize,
}

define_class!(
    /// Draws the dim layer, the hole and the crosshair for one display.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastSelectorView"]
    #[ivars = ViewIvars]
    struct OverlayView;

    impl OverlayView {
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

        #[unsafe(method(resetCursorRects))]
        fn reset_cursor_rects(&self) {
            self.addCursorRect_cursor(self.bounds(), &NSCursor::crosshairCursor());
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, _e: &NSEvent) {
            let p = mouse_point();
            with_state(|s| s.pointer_down(p));
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, _e: &NSEvent) {
            let p = mouse_point();
            with_state(|s| s.pointer_move(p));
        }

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, _e: &NSEvent) {
            let p = mouse_point();
            with_state(|s| s.pointer_move(p));
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, _e: &NSEvent) {
            let p = mouse_point();
            with_state(|s| s.pointer_up(p));
        }

        #[unsafe(method(keyDown:))]
        fn key_down(&self, e: &NSEvent) {
            match e.keyCode() {
                KEY_ESC => with_state(SelectorState::cancel),
                KEY_SPACE if !e.isARepeat() => {
                    let p = mouse_point();
                    with_state(|s| {
                        s.toggle_mode();
                        s.pointer_move(p);
                    })
                }
                _ => {}
            }
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let index = self.ivars().index;
            let Some(layout) = STATE.with(|s| s.borrow().as_ref().map(|st| layout_for(st, index))) else { return };
            let bounds = self.bounds();
            let ns = |r: Rect| NSRect::new(NSPoint::new(r.x, r.y), NSSize::new(r.w, r.h));
            // dim everywhere except the hole (even-odd fill)
            let dim = NSBezierPath::bezierPathWithRect(bounds);
            if let Some(h) = layout.hole {
                dim.appendBezierPath(&NSBezierPath::bezierPathWithRect(ns(h)));
            }
            dim.setWindingRule(NSWindingRule::EvenOdd);
            NSColor::colorWithSRGBRed_green_blue_alpha(0.0, 0.0, 0.0, 0.45).setFill();
            dim.fill();
            if let Some(h) = layout.hole {
                // A fully transparent pixel of a non-opaque window lets the mouse go through to the app behind,
                // so a click INSIDE the highlighted window (the hole) never reached this view and window mode
                // could not select anything (seen live: window mode never produced a mouseDown). A nearly
                // invisible fill keeps the hole clickable.
                NSColor::colorWithSRGBRed_green_blue_alpha(0.0, 0.0, 0.0, 0.01).setFill();
                NSBezierPath::bezierPathWithRect(ns(h)).fill();
                let edge = NSBezierPath::bezierPathWithRect(ns(h));
                edge.setLineWidth(1.0);
                NSColor::whiteColor().setStroke();
                edge.stroke();
            }
            if let Some(c) = layout.crosshair {
                let lines = NSBezierPath::bezierPath();
                lines.moveToPoint(NSPoint::new(0.0, c.y));
                lines.lineToPoint(NSPoint::new(bounds.size.width, c.y));
                lines.moveToPoint(NSPoint::new(c.x, 0.0));
                lines.lineToPoint(NSPoint::new(c.x, bounds.size.height));
                lines.setLineWidth(1.0);
                NSColor::colorWithSRGBRed_green_blue_alpha(1.0, 1.0, 1.0, 0.6).setStroke();
                lines.stroke();
            }
        }
    }
);

define_class!(
    /// Borderless panel that can take focus, so Esc and Space reach the view.
    #[unsafe(super(NSPanel))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastSelectorPanel"]
    struct SelectorPanel;

    impl SelectorPanel {
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

fn ns_rect(r: Rect) -> NSRect {
    NSRect::new(NSPoint::new(r.x, r.y), NSSize::new(r.w, r.h))
}

/// Push the current state to every view: label text/position and a redraw.
fn refresh() {
    let layouts: Vec<OverlayLayout> = STATE.with(|s| match s.borrow().as_ref() {
        Some(st) => (0..st.displays.len()).map(|i| layout_for(st, i)).collect(),
        None => Vec::new(),
    });
    VIEWS.with(|v| {
        for ((view, label), layout) in v.borrow().iter().zip(layouts) {
            match layout.label {
                Some((text, at)) => {
                    label.setStringValue(&NSString::from_str(&text));
                    label.setFrame(ns_rect(Rect::new(at.x, at.y, LABEL_W, LABEL_H)));
                    label.setHidden(false);
                }
                None => label.setHidden(true),
            }
            view.setNeedsDisplay(true);
        }
    });
}

fn displays_now(mtm: MainThreadMarker) -> (Vec<Display>, Vec<NSRect>, f64) {
    let screens = NSScreen::screens(mtm);
    let primary_h = screens.iter().next().map(|s| s.frame().size.height).unwrap_or(0.0);
    let mut displays = Vec::new();
    let mut cocoa = Vec::new();
    for s in screens.iter() {
        let f = s.frame();
        cocoa.push(f);
        let top_left = flip_rect(Rect::new(f.origin.x, f.origin.y, f.size.width, f.size.height), primary_h);
        displays.push(Display { frame: top_left, scale: s.backingScaleFactor() });
    }
    (displays, cocoa, primary_h)
}

/// Show the overlay on every display and block the calling main-thread run loop until the user
/// selects an area, picks a window or cancels. `windows` are front-to-back frames in top-left
/// points (for window mode). Nothing is written and nothing else is shown on `Cancelled`.
pub fn select(mtm: MainThreadMarker, windows: Vec<Window>) -> Selection {
    let (displays, cocoa, primary_h) = displays_now(mtm);
    if displays.is_empty() {
        return Selection::Cancelled;
    }
    PRIMARY_HEIGHT.with(|p| p.set(primary_h));
    let mut state = SelectorState::new(displays.clone(), windows);
    state.pointer_move(mouse_point());
    STATE.with(|s| *s.borrow_mut() = Some(state));

    let app = NSApplication::sharedApplication(mtm);
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);

    let mut panels = Vec::new();
    for (i, frame) in cocoa.iter().enumerate() {
        let style = NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel;
        let panel: Retained<SelectorPanel> = unsafe {
            msg_send![
                super(mtm.alloc::<SelectorPanel>().set_ivars(())),
                initWithContentRect: *frame,
                styleMask: style,
                backing: NSBackingStoreType::Buffered,
                defer: false
            ]
        };
        unsafe { panel.setReleasedWhenClosed(false) };
        // NSScreenSaverWindowLevel
        panel.setLevel(1000);
        panel.setOpaque(false);
        panel.setHasShadow(false);
        panel.setBackgroundColor(Some(&NSColor::clearColor()));
        panel.setAcceptsMouseMovedEvents(true);
        panel.setHidesOnDeactivate(false);
        panel.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        let bounds = NSRect::new(NSPoint::new(0.0, 0.0), frame.size);
        let view: Retained<OverlayView> =
            unsafe { msg_send![super(mtm.alloc::<OverlayView>().set_ivars(ViewIvars { index: i })), initWithFrame: bounds] };
        let label = NSTextField::labelWithString(&NSString::from_str(""), mtm);
        label.setFont(Some(&NSFont::monospacedDigitSystemFontOfSize_weight(12.0, 0.0)));
        label.setAlignment(NSTextAlignment::Center);
        label.setTextColor(Some(&NSColor::whiteColor()));
        label.setDrawsBackground(true);
        label.setBackgroundColor(Some(&NSColor::colorWithSRGBRed_green_blue_alpha(0.0, 0.0, 0.0, 0.75)));
        label.setHidden(true);
        view.addSubview(&label);
        panel.setContentView(Some(&view));
        panel.makeKeyAndOrderFront(None);
        panel.makeFirstResponder(Some(&view));
        VIEWS.with(|v| v.borrow_mut().push((view, label)));
        panels.push(panel);
    }
    refresh();

    let result = loop {
        if let Some(done) = STATE.with(|s| s.borrow().as_ref().and_then(SelectorState::outcome)) {
            break done;
        }
        let until = NSDate::distantFuture();
        if let Some(ev) =
            unsafe { app.nextEventMatchingMask_untilDate_inMode_dequeue(NSEventMask::Any, Some(&until), NSDefaultRunLoopMode, true) }
        {
            app.sendEvent(&ev);
        }
    };

    for p in &panels {
        p.orderOut(None);
        p.close();
    }
    VIEWS.with(|v| v.borrow_mut().clear());
    STATE.with(|s| *s.borrow_mut() = None);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    fn displays() -> Vec<Display> {
        // primary 1440x900 @2x, a 1920x1080 @1x display to its right
        vec![
            Display { frame: Rect::new(0.0, 0.0, 1440.0, 900.0), scale: 2.0 },
            Display { frame: Rect::new(1440.0, 0.0, 1920.0, 1080.0), scale: 1.0 },
        ]
    }

    fn windows() -> Vec<Window> {
        vec![Window { id: 7, frame: Rect::new(100.0, 100.0, 300.0, 200.0) }, Window { id: 9, frame: Rect::new(0.0, 0.0, 800.0, 600.0) }]
    }

    fn state() -> SelectorState {
        SelectorState::new(displays(), windows())
    }

    #[test]
    fn drag_in_any_direction_selects_the_normalised_area() {
        let mut s = state();
        s.pointer_down(pt(300.0, 400.0));
        s.pointer_move(pt(200.0, 350.0));
        assert_eq!(s.outcome(), None);
        s.pointer_up(pt(100.0, 300.0));
        assert_eq!(s.outcome(), Some(Selection::Area(Rect::new(100.0, 300.0, 200.0, 100.0))));
    }

    #[test]
    fn drag_is_clipped_to_the_display_under_the_pointer() {
        let mut s = state();
        s.pointer_down(pt(1400.0, 100.0));
        s.pointer_up(pt(1500.0, 200.0));
        assert_eq!(s.outcome(), Some(Selection::Area(Rect::new(1440.0, 100.0, 60.0, 100.0))));
    }

    #[test]
    fn a_plain_click_or_tiny_drag_selects_nothing_and_can_retry() {
        let mut s = state();
        s.pointer_down(pt(50.0, 50.0));
        s.pointer_up(pt(50.0, 50.0));
        assert_eq!(s.outcome(), None);
        s.pointer_down(pt(50.0, 50.0));
        s.pointer_up(pt(51.0, 90.0));
        assert_eq!(s.outcome(), None);
        s.pointer_down(pt(50.0, 50.0));
        s.pointer_up(pt(90.0, 90.0));
        assert_eq!(s.outcome(), Some(Selection::Area(Rect::new(50.0, 50.0, 40.0, 40.0))));
    }

    #[test]
    fn release_off_every_display_selects_nothing() {
        let mut s = state();
        s.pointer_down(pt(50.0, 50.0));
        s.pointer_up(pt(-20.0, 50.0));
        assert_eq!(s.outcome(), None);
    }

    #[test]
    fn esc_cancels_idle_and_mid_drag() {
        let mut s = state();
        s.cancel();
        assert_eq!(s.outcome(), Some(Selection::Cancelled));
        let mut s = state();
        s.pointer_down(pt(10.0, 10.0));
        s.pointer_move(pt(200.0, 200.0));
        s.cancel();
        assert_eq!(s.outcome(), Some(Selection::Cancelled));
    }

    #[test]
    fn nothing_changes_after_the_outcome_is_set() {
        let mut s = state();
        s.cancel();
        s.pointer_down(pt(10.0, 10.0));
        s.pointer_up(pt(200.0, 200.0));
        s.toggle_mode();
        assert_eq!(s.outcome(), Some(Selection::Cancelled));
        assert_eq!(s.mode(), Mode::Area);
    }

    #[test]
    fn space_toggles_window_mode_and_drops_the_drag() {
        let mut s = state();
        s.pointer_down(pt(10.0, 10.0));
        s.toggle_mode();
        assert_eq!(s.mode(), Mode::Window);
        s.pointer_up(pt(500.0, 500.0));
        assert_eq!(s.outcome(), None, "the area drag was dropped and 500,500 is not on a clicked window");
        s.toggle_mode();
        assert_eq!(s.mode(), Mode::Area);
    }

    #[test]
    fn window_mode_hover_picks_the_topmost_and_click_selects_it() {
        let mut s = state();
        s.toggle_mode();
        s.pointer_move(pt(150.0, 150.0));
        assert_eq!(s.hovered_window().map(|w| w.id), Some(7));
        s.pointer_move(pt(700.0, 500.0));
        assert_eq!(s.hovered_window().map(|w| w.id), Some(9));
        s.pointer_down(pt(150.0, 150.0));
        s.pointer_up(pt(150.0, 150.0));
        assert_eq!(s.outcome(), Some(Selection::Window(7)));
    }

    #[test]
    fn window_mode_click_on_empty_space_selects_nothing() {
        let mut s = state();
        s.toggle_mode();
        s.pointer_down(pt(1000.0, 800.0));
        s.pointer_up(pt(1000.0, 800.0));
        assert_eq!(s.outcome(), None);
        assert!(s.hovered_window().is_none());
    }

    #[test]
    fn layout_shows_crosshair_and_pixel_label_while_dragging() {
        let mut s = state();
        s.pointer_move(pt(100.0, 100.0));
        let idle = layout_for(&s, 0);
        assert_eq!(idle.crosshair, Some(pt(100.0, 100.0)));
        assert_eq!((idle.hole, idle.label), (None, None));
        assert_eq!(layout_for(&s, 1).crosshair, None, "crosshair only on the pointer's display");

        s.pointer_down(pt(100.0, 100.0));
        s.pointer_move(pt(300.0, 250.0));
        let l = layout_for(&s, 0);
        assert_eq!(l.hole, Some(Rect::new(100.0, 100.0, 200.0, 150.0)));
        // 2x display: pixels are doubled
        assert_eq!(l.label, Some(("400 × 300".to_string(), pt(314.0, 264.0))));
        assert_eq!(layout_for(&s, 1).hole, None);
    }

    #[test]
    fn layout_uses_display_local_coordinates_and_scale_on_the_second_display() {
        let mut s = state();
        s.pointer_down(pt(1500.0, 100.0));
        s.pointer_move(pt(1700.0, 200.0));
        let l = layout_for(&s, 1);
        assert_eq!(l.hole, Some(Rect::new(60.0, 100.0, 200.0, 100.0)));
        assert_eq!(l.label.as_ref().map(|(t, _)| t.as_str()), Some("200 × 100"));
        assert_eq!(layout_for(&s, 0).hole, None);
    }

    #[test]
    fn label_flips_inside_near_the_display_edges() {
        let mut s = state();
        s.pointer_down(pt(1000.0, 600.0));
        s.pointer_move(pt(1430.0, 895.0));
        let (_, at) = layout_for(&s, 0).label.unwrap();
        assert_eq!(at, pt(1430.0 - LABEL_OFFSET - LABEL_W, 895.0 - LABEL_OFFSET - LABEL_H));
    }

    #[test]
    fn window_mode_highlights_the_hovered_window_with_its_pixel_size() {
        let mut s = state();
        s.toggle_mode();
        s.pointer_move(pt(150.0, 150.0));
        let l = layout_for(&s, 0);
        assert_eq!(l.hole, Some(Rect::new(100.0, 100.0, 300.0, 200.0)));
        assert_eq!(l.crosshair, None);
        assert_eq!(l.label.as_ref().map(|(t, _)| t.as_str()), Some("600 × 400"));
        s.pointer_move(pt(1000.0, 800.0));
        assert_eq!(layout_for(&s, 0).hole, None);
    }

    #[test]
    fn pixel_size_matches_core() {
        use ferrocast_core::capture::region::pixel_size;
        assert_eq!(pixel_size(Rect::new(0.0, 0.0, 10.5, 3.0), 2.0), (21, 6));
    }

    /// Manual check, needs a display and a user: `cargo test -p ferrocast-macos -- --ignored selector_manual --nocapture`.
    #[test]
    #[ignore = "interactive: needs a display and a human"]
    fn selector_manual() {
        let mtm = MainThreadMarker::new().expect("run with --test-threads=1 on the main thread");
        println!("{:?}", select(mtm, Vec::new()));
    }
}
