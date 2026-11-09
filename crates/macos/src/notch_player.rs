//! The notch player (S3), "Vinyl / ambient" (design/notch-player/03-vinyl-ambient.html): a black pill
//! that extends the notch (or hangs from the top centre of a Mac without one) and opens on hover into a
//! 430 x 216 card: the cover as a spinning vinyl disc inside a progress ring, a blurred halo of the
//! cover with the tinted light that bleeds out under the card, a big title, the source app, previous /
//! play-pause / next, and the time row with a seekable bar.
//!
//! Thin AppKit drawing. What to show comes from `ferrocast_core::nowplaying::Model`; where things go,
//! every spring, the disc's speed, the stagger and the Reduce Motion rules are the pure functions in
//! `ferrocast_core::nowplaying_view`, and the disc, halo and tint come from `nowplaying_art` (computed
//! once per cover off the main thread). Everything is drawn in one `drawRect` so the layers keep a
//! single z-order; only the two arrow buttons and the play button are real views, for clicks and
//! accessibility, with the play glyph drawn here so it can morph.
//!
//! One panel, borderless and non-activating, above the menu bar on every Space. Only the card is
//! live: the strip below it, where the tinted light bleeds, ignores the pointer's hover. A repeating
//! timer runs only while something moves (springs at the display's rate, the disc and the equalizer
//! at 30 Hz). Main thread only.

use std::cell::RefCell;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use block2::RcBlock;
use ferrocast_core::nowplaying::{self as np, Command, Model, PlayState};
use ferrocast_core::nowplaying_art::ArtLook;
use ferrocast_core::nowplaying_input::{self as input, Scrub};
use ferrocast_core::nowplaying_view::{self as view, Motion, Notch, CARD_H, CARD_W};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{define_class, msg_send, sel, AnyThread, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAffineTransformNSAppKitAdditions, NSBackingStoreType, NSBezierPath, NSBitmapImageRep, NSButton, NSColor, NSColorSpace,
    NSCompositingOperation, NSCursor, NSDeviceRGBColorSpace, NSEvent, NSFont, NSFontAttributeName, NSFontWeightHeavy, NSFontWeightMedium,
    NSForegroundColorAttributeName, NSGradient, NSGradientDrawingOptions, NSGraphicsContext, NSImage, NSImageInterpolation,
    NSImageSymbolConfiguration, NSKernAttributeName, NSMainMenuWindowLevel, NSPanel, NSScreen, NSStringDrawing, NSTrackingArea,
    NSTrackingAreaOptions, NSView, NSWindowCollectionBehavior, NSWindowSharingType, NSWindowStyleMask, NSWorkspace,
};
use objc2_foundation::{
    NSAffineTransform, NSArray, NSDictionary, NSFileManager, NSNumber, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
    NSTimer,
};

use crate::notes_theme::{Rgba, DARK};

/// What the player tells the app.
pub struct Callbacks {
    /// The pointer entered (true) or left (false) the card.
    pub on_hover: Box<dyn Fn(bool)>,
    pub on_command: Box<dyn Fn(Command)>,
    /// The progress bar was released at this many seconds.
    pub on_seek: Box<dyn Fn(f64)>,
    /// The title or artist was clicked: open the app that plays.
    pub on_open: Box<dyn Fn()>,
}

struct Draw {
    model: Model,
    motion: Motion,
    last_tick: Instant,
    started: Instant,
    notch: Option<Notch>,
    screen: [f64; 4],
    reduce_motion: bool,
    /// Display refresh rate: the timer follows it (60 to 120 Hz).
    fps: f64,
    /// Size of the card now (the panel is this wide and [`view::BLEED_PAD`] taller).
    card: (f64, f64),
    /// What the cover gave: the disc, the halo and the tint.
    look: Arc<ArtLook>,
    disc_img: Option<Retained<NSImage>>,
    halo_img: Option<Retained<NSImage>>,
    art_key: Option<Arc<Vec<u8>>>,
    /// Icon of the app that plays, and its name (fitted to the strip).
    source_icon: Option<Retained<NSImage>>,
    source_id: String,
    /// `app_name(source_id)`, looked up once per source, not on every redraw.
    source_name: String,
    chip_text: String,
    /// The title wrapped to at most two lines, and the artist fitted to one.
    title_lines: Vec<String>,
    artist_text: String,
    /// A press on the progress bar, until it is released.
    scrub: Option<Scrub>,
    /// The pointer is over the title/artist (a link), the progress bar's hit area, or a button.
    hover_link: bool,
    hover_bar: bool,
    hover_btn: Option<usize>,
    /// The pointer is inside the card (not in the light below it).
    pointer_in: bool,
    /// The press started on the title/artist.
    press_link: bool,
}

struct Views {
    panel: Retained<NpPanel>,
    root: Retained<NpView>,
    prev: Retained<NSButton>,
    play: Retained<NSButton>,
    next: Retained<NSButton>,
    _target: Retained<NpTarget>,
    play_symbol: Option<PlayState>,
}

thread_local! {
    static DRAW: RefCell<Option<Draw>> = const { RefCell::new(None) };
    static VIEWS: RefCell<Option<Views>> = const { RefCell::new(None) };
    static CALLBACKS: RefCell<Option<Rc<Callbacks>>> = const { RefCell::new(None) };
    static TIMER: RefCell<Option<(Retained<NSTimer>, f64)>> = const { RefCell::new(None) };
}

fn callbacks() -> Option<Rc<Callbacks>> {
    CALLBACKS.with(|c| c.borrow().clone())
}

define_class!(
    /// Borderless panel that never takes focus and may sit over the menu bar.
    #[unsafe(super(NSPanel))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastNotchPlayerPanel"]
    struct NpPanel;

    impl NpPanel {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key(&self) -> bool {
            false
        }

        #[unsafe(method(canBecomeMainWindow))]
        fn can_become_main(&self) -> bool {
            false
        }

        // AppKit would push a window that overlaps the menu bar down below it
        #[unsafe(method(constrainFrameRect:toScreen:))]
        fn constrain(&self, frame: NSRect, _screen: Option<&NSScreen>) -> NSRect {
            frame
        }
    }
);

define_class!(
    /// Draws the card and reports the pointer.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastNotchPlayerView"]
    struct NpView;

    impl NpView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(mouseEntered:))]
        fn mouse_entered(&self, event: &NSEvent) {
            let p = self.local(event);
            pointer_moved(self, p);
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _event: &NSEvent) {
            pointer_left(self);
        }

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            let p = self.local(event);
            pointer_moved(self, p);
        }

        #[unsafe(method(cursorUpdate:))]
        fn cursor_update(&self, event: &NSEvent) {
            let p = self.local(event);
            pointer_moved(self, p);
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let p = self.local(event);
            pointer_down(self, p);
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            let p = self.local(event);
            pointer_dragged(self, p);
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            let p = self.local(event);
            pointer_up(self, p);
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            draw(self.bounds());
        }
    }
);

define_class!(
    /// Receives the three buttons' clicks.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastNotchPlayerTarget"]
    struct NpTarget;

    impl NpTarget {
        #[unsafe(method(prevClicked:))]
        fn prev_clicked(&self, _sender: &AnyObject) {
            send(Command::Previous);
        }

        #[unsafe(method(playClicked:))]
        fn play_clicked(&self, _sender: &AnyObject) {
            send(Command::PlayPause);
        }

        #[unsafe(method(nextClicked:))]
        fn next_clicked(&self, _sender: &AnyObject) {
            send(Command::Next);
        }
    }
);

impl NpView {
    /// Where `event` is in the view's own top-left coordinates (the view is flipped).
    fn local(&self, event: &NSEvent) -> (f64, f64) {
        let p = self.convertPoint_fromView(event.locationInWindow(), None);
        (p.x, p.y)
    }
}

unsafe impl NSObjectProtocol for NpTarget {}

fn send(c: Command) {
    if let Some(cb) = callbacks() {
        (cb.on_command)(c);
    }
}

/// Register what the player reports (replaces the previous callbacks).
pub fn set_callbacks(c: Callbacks) {
    CALLBACKS.with(|slot| *slot.borrow_mut() = Some(Rc::new(c)));
}

// ---------------------------------------------------------------- the pointer

/// True while the expanded content is (nearly) fully there: only then are text, bar and buttons live.
fn live(d: &Draw) -> bool {
    d.motion.is_open() && d.motion.pw.x > 0.85 && d.motion.items[1].x > 0.5
}

/// Content-frame x offset: the 430-wide content is centred on the card.
fn ox(d: &Draw) -> f64 {
    (d.card.0 - CARD_W) / 2.0
}

fn duration_of(d: &Draw) -> f64 {
    d.model.snapshot.as_ref().map_or(0.0, |s| s.duration)
}

fn layout_of(d: &Draw) -> view::Expanded {
    view::expanded_layout(d.title_lines.len())
}

/// The pointer left the panel.
fn pointer_left(root: &NpView) {
    let was_in = DRAW.with(|d| {
        let mut d = d.try_borrow_mut().ok()?;
        let d = d.as_mut()?;
        (d.hover_link, d.hover_bar, d.hover_btn) = (false, false, None);
        Some(std::mem::replace(&mut d.pointer_in, false))
    });
    root.setNeedsDisplay(true);
    if was_in == Some(true) {
        if let Some(cb) = callbacks() {
            (cb.on_hover)(false);
        }
    }
}

/// The pointer moved: hover in and out of the card, the highlight, the knob and the cursor.
fn pointer_moved(root: &NpView, (x, y): (f64, f64)) {
    let r = DRAW.with(|d| {
        let mut d = d.try_borrow_mut().ok()?;
        let d = d.as_mut()?;
        let inside = y <= d.card.1 && x >= 0.0 && x <= d.card.0;
        let hover_change = (inside != d.pointer_in).then_some(inside);
        d.pointer_in = inside;
        let (cx, e) = (x - ox(d), layout_of(d));
        let on = live(d) && inside;
        let link = on && input::contains(input::link_rect(&e), [cx, y]);
        let bar = on && input::seekable(duration_of(d)) && input::contains(input::bar_hit_rect(e.bar), [cx, y]);
        let btn = if on { [e.prev, e.play, e.next].iter().position(|r| input::contains(*r, [cx, y])) } else { None };
        let changed = (link, bar, btn) != (d.hover_link, d.hover_bar, d.hover_btn);
        (d.hover_link, d.hover_bar, d.hover_btn) = (link, bar, btn);
        Some((hover_change, changed, link || bar || btn.is_some() || d.scrub.is_some()))
    });
    let Some((hover_change, changed, hand)) = r else { return };
    if hand {
        NSCursor::pointingHandCursor().set();
    } else {
        NSCursor::arrowCursor().set();
    }
    if changed {
        root.setNeedsDisplay(true);
    }
    if let (Some(h), Some(cb)) = (hover_change, callbacks()) {
        (cb.on_hover)(h);
    }
}

fn pointer_down(root: &NpView, (x, y): (f64, f64)) {
    DRAW.with(|d| {
        let Ok(mut d) = d.try_borrow_mut() else { return };
        let Some(d) = d.as_mut() else { return };
        if !live(d) {
            return;
        }
        let (cx, e) = (x - ox(d), layout_of(d));
        d.scrub = Scrub::begin(e.bar, cx, y, duration_of(d));
        d.press_link = d.scrub.is_none() && input::contains(input::link_rect(&e), [cx, y]);
    });
    root.setNeedsDisplay(true);
}

fn pointer_dragged(root: &NpView, (x, _y): (f64, f64)) {
    let moved = DRAW.with(|d| {
        let mut d = d.try_borrow_mut().ok()?;
        let d = d.as_mut()?;
        let cx = x - ox(d);
        d.scrub.as_mut()?.drag(cx);
        Some(())
    });
    if moved.is_some() {
        root.setNeedsDisplay(true);
    }
}

/// The button went up: a scrub ends with exactly one seek, a press on the text opens the app.
fn pointer_up(root: &NpView, (x, y): (f64, f64)) {
    enum Act {
        Seek(f64),
        Open,
    }
    let act = DRAW.with(|d| {
        let mut d = d.try_borrow_mut().ok()?;
        let d = d.as_mut()?;
        let cx = x - ox(d);
        let was_link = std::mem::take(&mut d.press_link);
        if let Some(s) = d.scrub.take() {
            return Some(Act::Seek(s.end(cx)));
        }
        let e = layout_of(d);
        (was_link && input::contains(input::link_rect(&e), [cx, y])).then_some(Act::Open)
    });
    root.setNeedsDisplay(true);
    // after the borrow ends: the callbacks may call back into the player
    if let (Some(act), Some(cb)) = (act, callbacks()) {
        match act {
            Act::Seek(secs) => (cb.on_seek)(secs),
            Act::Open => (cb.on_open)(),
        }
    }
}

// ---------------------------------------------------------------- small helpers

fn rect(r: [f64; 4]) -> NSRect {
    NSRect::new(NSPoint::new(r[0], r[1]), NSSize::new(r[2], r[3]))
}

fn ns(c: Rgba, alpha: f64) -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(c.0 as f64 / 255.0, c.1 as f64 / 255.0, c.2 as f64 / 255.0, alpha * c.3 as f64 / 255.0)
}

fn rgb(c: [u8; 3], alpha: f64) -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(c[0] as f64 / 255.0, c[1] as f64 / 255.0, c[2] as f64 / 255.0, alpha)
}

fn white(alpha: f64) -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(1.0, 1.0, 1.0, alpha)
}

fn symbol(name: &str, pt: f64) -> Option<Retained<NSImage>> {
    let img = NSImage::imageWithSystemSymbolName_accessibilityDescription(&NSString::from_str(name), None)?;
    let cfg = NSImageSymbolConfiguration::configurationWithPointSize_weight(pt, 0.4);
    img.imageWithSymbolConfiguration(&cfg).or(Some(img))
}

/// The display the player lives on: the one with the menu bar, and its notch if it has one.
fn screen_info(mtm: MainThreadMarker) -> Option<([f64; 4], Option<Notch>)> {
    let screens = NSScreen::screens(mtm);
    let s = screens.iter().next()?;
    let f = s.frame();
    let top = s.safeAreaInsets().top;
    let (l, r) = (s.auxiliaryTopLeftArea().size.width, s.auxiliaryTopRightArea().size.width);
    Some(([f.origin.x, f.origin.y, f.size.width, f.size.height], view::notch_from_screen(f.size.width, top, l, r)))
}

fn tip(button: &NSButton, key: &str) {
    button.setToolTip(Some(&NSString::from_str(&ferrocast_core::i18n::tr(key))));
}

fn icon_button(mtm: MainThreadMarker, target: &AnyObject, action: objc2::runtime::Sel, name: Option<&str>) -> Retained<NSButton> {
    let img = name.and_then(|n| symbol(n, 17.0)).unwrap_or_default();
    let b = unsafe { NSButton::buttonWithImage_target_action(&img, Some(target), Some(action), mtm) };
    b.setBordered(false);
    b.setContentTintColor(Some(&ns(DARK.text, 1.0)));
    b.setRefusesFirstResponder(true);
    b
}

/// An image from premultiplied RGBA (main thread).
fn rgba_image(px: &[u8], w: usize, h: usize) -> Option<Retained<NSImage>> {
    unsafe {
        let rep = NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            w as isize,
            h as isize,
            8,
            4,
            true,
            false,
            NSDeviceRGBColorSpace,
            0,
            0,
        )?;
        let (dst, bpr) = (rep.bitmapData(), rep.bytesPerRow() as usize);
        if dst.is_null() || bpr < w * 4 || px.len() < w * h * 4 {
            return None;
        }
        for row in 0..h {
            std::ptr::copy_nonoverlapping(px.as_ptr().add(row * w * 4), dst.add(row * bpr), w * 4);
        }
        let img = NSImage::initWithSize(NSImage::alloc(), NSSize::new(w as f64, h as f64));
        img.addRepresentation(&rep);
        Some(img)
    }
}

fn build(mtm: MainThreadMarker) -> Views {
    let style = NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel;
    let panel: Retained<NpPanel> = unsafe {
        msg_send![
            super(mtm.alloc::<NpPanel>().set_ivars(())),
            initWithContentRect: rect([0.0, 0.0, 10.0, 10.0]),
            styleMask: style,
            backing: NSBackingStoreType::Buffered,
            defer: false
        ]
    };
    unsafe { panel.setReleasedWhenClosed(false) };
    // setFloatingPanel resets the level to NSFloatingWindowLevel (3), which sat BELOW a revealed
    // auto-hidden menu bar: set the level after it, above the main menu.
    panel.setFloatingPanel(true);
    panel.setLevel(NSMainMenuWindowLevel + 2);
    panel.setHidesOnDeactivate(false);
    panel.setOpaque(false);
    panel.setHasShadow(false);
    panel.setBackgroundColor(Some(&NSColor::clearColor()));
    panel.setExcludedFromWindowsMenu(true);
    // Ask macOS to leave it out of captures of the display. Some capture tools ignore this. Debug
    // builds (the demo example) can opt out with the variable to take pictures of it.
    #[cfg(debug_assertions)]
    let capturable = std::env::var_os("FERROCAST_NOTCH_CAPTURABLE").is_some();
    #[cfg(not(debug_assertions))]
    let capturable = false;
    if !capturable {
        panel.setSharingType(NSWindowSharingType::None);
    }
    panel.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces | NSWindowCollectionBehavior::Stationary | NSWindowCollectionBehavior::IgnoresCycle,
    );

    let root: Retained<NpView> =
        unsafe { msg_send![super(mtm.alloc::<NpView>().set_ivars(())), initWithFrame: rect([0.0, 0.0, 10.0, 10.0])] };
    let opts = NSTrackingAreaOptions::MouseEnteredAndExited
        | NSTrackingAreaOptions::MouseMoved
        | NSTrackingAreaOptions::CursorUpdate
        | NSTrackingAreaOptions::ActiveAlways
        | NSTrackingAreaOptions::InVisibleRect;
    let owner: &AnyObject = &root;
    let area = unsafe { NSTrackingArea::initWithRect_options_owner_userInfo(mtm.alloc(), NSRect::ZERO, opts, Some(owner), None) };
    root.addTrackingArea(&area);

    let target: Retained<NpTarget> = unsafe { msg_send![super(mtm.alloc::<NpTarget>().set_ivars(())), init] };
    let t: &AnyObject = &target;
    let prev = icon_button(mtm, t, sel!(prevClicked:), Some("backward.end.fill"));
    // the play button is invisible: the round white button and its morphing glyph are drawn under it
    let play = icon_button(mtm, t, sel!(playClicked:), None);
    let next = icon_button(mtm, t, sel!(nextClicked:), Some("forward.end.fill"));
    tip(&prev, "Previous");
    tip(&play, "Play");
    tip(&next, "Next");
    for v in [&prev as &NSView, &play, &next] {
        root.addSubview(v);
    }
    panel.setContentView(Some(&root));
    Views { panel, root, prev, play, next, _target: target, play_symbol: None }
}

// ---------------------------------------------------------------- drawing

fn fill_round(r: [f64; 4], radius: f64, color: &NSColor) {
    color.setFill();
    NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect(r), radius, radius).fill();
}

fn fill_oval(r: [f64; 4], color: &NSColor) {
    color.setFill();
    NSBezierPath::bezierPathWithOvalInRect(rect(r)).fill();
}

fn text_attrs(font: &NSFont, color: &NSColor, kern: f64) -> Retained<NSDictionary<NSString, AnyObject>> {
    let (k1, k2, k3) = unsafe { (NSFontAttributeName, NSForegroundColorAttributeName, NSKernAttributeName) };
    let kern = NSNumber::new_f64(kern);
    NSDictionary::from_slices(&[k1, k2, k3], &[font as &AnyObject, color as &AnyObject, &*kern as &AnyObject])
}

fn text_width(s: &str, attrs: &NSDictionary<NSString, AnyObject>) -> f64 {
    unsafe { NSString::from_str(s).sizeWithAttributes(Some(attrs)).width }
}

fn draw_text(s: &str, attrs: &NSDictionary<NSString, AnyObject>, r: [f64; 4]) {
    unsafe { NSString::from_str(s).drawInRect_withAttributes(rect(r), Some(attrs)) };
}

fn title_font() -> Retained<NSFont> {
    NSFont::systemFontOfSize_weight(28.0, unsafe { NSFontWeightHeavy })
}

fn title_attrs(color: &NSColor) -> Retained<NSDictionary<NSString, AnyObject>> {
    // the artboard's letter spacing: -0.035 em
    text_attrs(&title_font(), color, -0.035 * 28.0)
}

fn artist_attrs() -> Retained<NSDictionary<NSString, AnyObject>> {
    text_attrs(&NSFont::systemFontOfSize(14.0), &ns(Rgba(0xd6, 0xd6, 0xd6, 255), 1.0), 0.0)
}

fn chip_attrs() -> Retained<NSDictionary<NSString, AnyObject>> {
    text_attrs(&NSFont::systemFontOfSize(12.0), &ns(Rgba(0xd4, 0xd4, 0xd8, 255), 1.0), 0.0)
}

fn draw(bounds: NSRect) {
    DRAW.with(|d| {
        // never panic inside drawRect: a borrow held by a tick means "skip this frame"
        if let Ok(d) = d.try_borrow() {
            if let Some(d) = d.as_ref() {
                draw_with(d, bounds);
            }
        }
    });
}

/// Draw `img` into `r`, `fraction` opaque, smoothly scaled.
fn draw_image(img: &NSImage, r: [f64; 4], fraction: f64) {
    unsafe {
        if let Some(ctx) = NSGraphicsContext::currentContext() {
            ctx.setImageInterpolation(NSImageInterpolation::High);
        }
        img.drawInRect_fromRect_operation_fraction_respectFlipped_hints(
            rect(r),
            NSRect::ZERO,
            NSCompositingOperation::SourceOver,
            fraction,
            true,
            None,
        );
    }
}

fn draw_with(d: &Draw, bounds: NSRect) {
    let rm = d.reduce_motion;
    let m = &d.motion;
    let (w, h) = d.card;
    let coll = view::collapsed_size(d.notch);
    let shape = m.shape(coll);
    let o = (bounds.size.width - CARD_W) / 2.0;
    let tm = d.started.elapsed().as_secs_f64();
    let ex = m.pw.x;
    let tint = d.look.tint;
    let pulse = m.pulse.x;
    let snap = d.model.snapshot.as_ref();

    // the tinted light that bleeds out under the card, drawn first so the card sits on top of it
    let (br, ba) = ferrocast_core::nowplaying_art::bleed_params(w, h, ex, pulse);
    if ba > 0.01 {
        let c = [br[0] + br[2] / 2.0, br[1] + br[3] / 2.0];
        NSGraphicsContext::saveGraphicsState_class();
        let t = NSAffineTransform::transform();
        t.translateXBy_yBy(c[0], c[1]);
        t.scaleXBy_yBy(1.0, br[3] / br[2]);
        t.concat();
        if let Some(g) = NSGradient::initWithStartingColor_endingColor(NSGradient::alloc(), &rgb(tint, ba), &rgb(tint, 0.0)) {
            g.drawFromCenter_radius_toCenter_radius_options(
                NSPoint::new(0.0, 0.0),
                0.0,
                NSPoint::new(0.0, 0.0),
                br[2] / 2.0,
                NSGradientDrawingOptions::empty(),
            );
        }
        NSGraphicsContext::restoreGraphicsState_class();
    }

    // the card: black, its top corners pushed above the view and clipped so only the bottom corners
    // are round and the top edge is flush with the screen
    let r = shape.radius.min(h / 2.0);
    let card = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect([0.0, -r, w, h + r]), r, r);
    NSColor::colorWithSRGBRed_green_blue_alpha(5.0 / 255.0, 5.0 / 255.0, 5.0 / 255.0, 1.0).setFill();
    card.fill();
    NSGraphicsContext::saveGraphicsState_class();
    card.addClip();

    // the halo (a blurred copy of the cover) under a left-to-right shade that keeps the text readable
    let (ha, hx, hy) = ferrocast_core::nowplaying_art::halo_params(ex, pulse, tm, rm);
    if ha > 0.01 {
        if let Some(img) = &d.halo_img {
            draw_image(img, [o - 120.0 + hx, -70.0 + hy, 400.0, 400.0], ha);
        }
        let k = ha.min(0.7) / 0.7;
        let black = |a: f64| NSColor::colorWithSRGBRed_green_blue_alpha(0.0, 0.0, 0.0, a * k);
        let colors = NSArray::from_retained_slice(&[black(0.12), black(0.62), black(0.72)]);
        let locations = [0.0f64, 0.44, 1.0];
        let shade = unsafe {
            NSGradient::initWithColors_atLocations_colorSpace(
                NSGradient::alloc(),
                &colors,
                locations.as_ptr(),
                &NSColorSpace::sRGBColorSpace(),
            )
        };
        if let Some(g) = shade {
            g.drawInRect_angle(rect([o, 0.0, CARD_W, CARD_H]), 0.0);
            // while the card is wider than the artboard's 430 pt (the spring's overshoot) the
            // shade's end colours carry on to the edges
            if o > 0.0 {
                black(0.12).setFill();
                NSBezierPath::fillRect(rect([0.0, 0.0, o, CARD_H]));
                black(0.72).setFill();
                NSBezierPath::fillRect(rect([o + CARD_W, 0.0, o + 1.0, CARD_H]));
            }
        }
    }

    // the disc, its shadow, the gloss and the ring
    let parts = view::parts_at(ex, coll);
    let (disc, ring) = (parts.disc, parts.ring);
    let (cx, cy) = (o + disc[0] + disc[2] / 2.0, disc[1] + disc[3] / 2.0);
    let dr = view::disc_radius(&parts);
    if ex > 0.05 && dr > 0.0 {
        // the cover's drop shadow (0 14 22 black .55): a soft radial falloff under the disc
        if let Some(g) = NSGradient::initWithStartingColor_endingColor(
            NSGradient::alloc(),
            &NSColor::colorWithSRGBRed_green_blue_alpha(0.0, 0.0, 0.0, 0.55 * ex.clamp(0.0, 1.0)),
            &NSColor::clearColor(),
        ) {
            g.drawFromCenter_radius_toCenter_radius_options(
                NSPoint::new(cx, cy + 14.0),
                dr * 0.8,
                NSPoint::new(cx, cy + 14.0),
                dr * 1.28,
                NSGradientDrawingOptions::empty(),
            );
        }
    }
    if let Some(img) = &d.disc_img {
        NSGraphicsContext::saveGraphicsState_class();
        let t = NSAffineTransform::transform();
        t.translateXBy_yBy(cx, cy);
        // a positive angle turns clockwise in this flipped view, like the artboard's CSS rotate
        t.rotateByDegrees(m.angle);
        t.translateXBy_yBy(-cx, -cy);
        t.concat();
        draw_image(img, [o + disc[0], disc[1], disc[2], disc[3]], 1.0);
        NSGraphicsContext::restoreGraphicsState_class();
        // gloss: fixed highlights on the glass, they do not turn with the record
        if dr > 8.0 {
            for i in 0..90 {
                let (a0, a1) = (i as f64 * 4.0, i as f64 * 4.0 + 4.0);
                let g = view::gloss_alpha(a0 + 2.0);
                if g <= 0.004 {
                    continue;
                }
                let p = NSBezierPath::bezierPath();
                p.moveToPoint(NSPoint::new(cx, cy));
                p.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle(NSPoint::new(cx, cy), dr, a0 - 90.0, a1 - 90.0);
                p.closePath();
                white(g).setFill();
                p.fill();
            }
        }
    }
    // the progress ring: a faint track and the arc from the top, clockwise, in the tint
    if let Some(s) = snap {
        let elapsed = (np::mono_secs() - d.model.sampled_at).max(0.0);
        let pos = np::position_at(s, elapsed);
        let frac = np::fraction(pos, s.duration);
        let (track_w, arc_w) = view::ring_stroke(ring[2]);
        let (rcx, rcy, rr) = (o + ring[0] + ring[2] / 2.0, ring[1] + ring[3] / 2.0, ring[2] / 2.0 - arc_w / 2.0);
        if rr > 1.0 {
            let t = NSBezierPath::bezierPath();
            t.appendBezierPathWithOvalInRect(rect([rcx - rr, rcy - rr, rr * 2.0, rr * 2.0]));
            t.setLineWidth(track_w);
            white(0.16).setStroke();
            t.stroke();
            let end = view::ring_end_degrees(frac);
            if end > 0.5 {
                let a = NSBezierPath::bezierPath();
                // y grows downward here, so increasing angles run clockwise on screen
                a.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle(NSPoint::new(rcx, rcy), rr, -90.0, -90.0 + end);
                a.setLineWidth(arc_w);
                a.setLineCapStyle(objc2_app_kit::NSLineCapStyle::Round);
                rgb(tint, 1.0).setStroke();
                a.stroke();
            }
        }
    }

    // the equalizer: four white bars, flat when paused
    let e = parts.eq;
    let (n, gap) = (4usize, 2.5);
    let bw = ((e[2] - gap * (n as f64 - 1.0)) / n as f64).max(1.0);
    for i in 0..n {
        let hh = (e[3] * view::eq_scale(tm, i, m.am.x, rm)).max(2.0);
        fill_round([o + e[0] + i as f64 * (bw + gap), e[1] + e[3] - hh, bw, hh], 2.0_f64.min(bw / 2.0), &white(1.0));
    }

    let Some(snap) = snap else {
        NSGraphicsContext::restoreGraphicsState_class();
        return;
    };
    let x = view::expanded_layout(d.title_lines.len());
    let ctl_origin = [184.0, 166.0];

    // item 0: the source chip
    let (a, s, dy) = m.item(0, rm);
    if a > 0.01 {
        let origin = [18.0, 16.0];
        if let Some(icon) = &d.source_icon {
            let r = view::xf_rect(x.chip_icon, origin, s, dy);
            draw_image(icon, [o + r[0], r[1], r[2], r[3]], a);
        }
        let r = view::xf_rect([x.chip[0], x.chip[1] + 1.0, x.chip[2], x.chip[3]], origin, s, dy);
        let attrs = text_attrs(&NSFont::systemFontOfSize(12.0), &ns(Rgba(0xd4, 0xd4, 0xd8, 255), a), 0.0);
        draw_text(&d.chip_text, &attrs, [o + r[0], r[1], x.chip[2] + 40.0, r[3]]);
    }

    // item 1: the title and the artist; the title underlines in the tint while the pointer is on it
    let (a, s, dy) = m.item(1, rm);
    if a > 0.01 {
        let origin = [192.0, x.title[1] + (x.title[3] + 7.0 + x.artist[3]) / 2.0];
        let tattrs = title_attrs(&ns(DARK.text, a));
        for (i, line) in d.title_lines.iter().enumerate() {
            let r = view::xf_rect([x.title[0], x.title[1] + i as f64 * 28.84, x.title[2], 28.84], origin, s, dy);
            draw_text(line, &tattrs, [o + r[0], r[1], r[2] + 20.0, r[3] + 6.0]);
            if d.hover_link {
                let lw = text_width(line, &tattrs).min(x.title[2]);
                let u = [o + r[0], r[1] + r[3] - 3.0, lw * s, 2.0];
                fill_round(u, 1.0, &rgb(tint, a));
            }
        }
        let ar = view::xf_rect(x.artist, origin, s, dy);
        let aattrs = text_attrs(&NSFont::systemFontOfSize(14.0), &ns(Rgba(0xd6, 0xd6, 0xd6, 255), a), 0.0);
        draw_text(&d.artist_text, &aattrs, [o + ar[0], ar[1], ar[2], ar[3] + 4.0]);
    }

    // item 2: previous / play-pause (a round white button whose glyph morphs) / next
    let (a, s, dy) = m.item(2, rm);
    if a > 0.01 {
        for (i, r) in [x.prev, x.play, x.next].into_iter().enumerate() {
            let r = view::xf_rect(r, ctl_origin, s, dy);
            let r = [o + r[0], r[1], r[2], r[3]];
            if i == 1 {
                fill_oval(r, &white(a));
                draw_morph(r, view::pp_points(m.pm.x), a);
            } else if d.hover_btn == Some(i) {
                fill_oval(r, &white(0.12 * a));
            }
        }
        if d.hover_btn == Some(1) {
            // the artboard's hover: the play button grows a little
            let r = view::xf_rect(x.play, ctl_origin, s * 1.06, dy - 1.5);
            fill_oval([o + r[0], r[1], r[2], r[3]], &white(0.0));
        }
    }

    // item 3: the time row, with the seek bar between the times
    let (a, s, dy) = m.item(3, rm);
    if a > 0.01 {
        let origin = [192.0, 201.5];
        let sc = |r: [f64; 4]| {
            let r = view::xf_rect(r, origin, s, dy);
            [o + r[0], r[1], r[2], r[3]]
        };
        let elapsed = (np::mono_secs() - d.model.sampled_at).max(0.0);
        let pos = d.scrub.map_or_else(|| np::position_at(snap, elapsed), |s| s.position());
        let active = d.scrub.is_some() || d.hover_bar;
        let b = sc(x.bar);
        let bar = if active { [b[0], b[1] - 1.0, b[2], b[3] + 2.0] } else { b };
        fill_round(bar, bar[3] / 2.0, &white(0.16 * a));
        let frac = np::fraction(pos, snap.duration);
        if frac > 0.0 {
            fill_round([bar[0], bar[1], (bar[2] * frac).max(bar[3]), bar[3]], bar[3] / 2.0, &rgb(tint, a));
        }
        let mono = NSFont::monospacedDigitSystemFontOfSize_weight(11.0, unsafe { NSFontWeightMedium });
        let attrs = text_attrs(&mono, &ns(Rgba(0xcf, 0xcf, 0xd4, 255), a), 0.0);
        draw_text(&np::format_clock(pos), &attrs, sc(x.elapsed));
        if snap.duration > 0.0 {
            let right = format!("\u{2212}{}", np::format_clock((snap.duration - pos).max(0.0)));
            let rr = sc(x.remaining);
            let wd = text_width(&right, &attrs);
            draw_text(&right, &attrs, [rr[0] + rr[2] - wd, rr[1], wd + 2.0, rr[3]]);
        }
        if active && snap.duration > 0.0 {
            // the knob, and while dragging a bubble with the time above it
            let (kd, kx) = (12.0, bar[0] + bar[2] * frac);
            fill_oval([kx - kd / 2.0, bar[1] + bar[3] / 2.0 - kd / 2.0, kd, kd], &white(a));
            if d.scrub.is_some() {
                let battrs = text_attrs(&NSFont::monospacedDigitSystemFontOfSize_weight(11.0, 0.3), &ns(DARK.text, a), 0.0);
                let label = np::format_clock(pos);
                let tw = text_width(&label, &battrs);
                let (bw2, bh) = (tw + 14.0, 20.0);
                let bx = (kx - bw2 / 2.0).clamp(o + 192.0, o + 192.0 + 214.0 - bw2);
                let by = bar[1] - bh - 8.0;
                fill_round([bx, by, bw2, bh], bh / 2.0, &white(0.2 * a));
                draw_text(&label, &battrs, [bx + 7.0, by + 3.5, tw + 2.0, 14.0]);
            }
        }
    }

    NSGraphicsContext::restoreGraphicsState_class();
    // a hairline inside the card's edge
    let inner = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect([0.5, -r, w - 1.0, h + r - 0.5]), r, r);
    inner.setLineWidth(1.0);
    white(0.07).setStroke();
    inner.stroke();
}

/// The play/pause glyph: two shapes of four points each, on the artboard's 24-point grid, in a
/// button of rect `r`, black on the white disc.
fn draw_morph(r: [f64; 4], p: [f64; 16], alpha: f64) {
    let k = 22.0 / 24.0; // the artboard draws the glyph 22 pt in the 52 pt button
    let (gx, gy) = (r[0] + r[2] / 2.0 - 12.0 * k, r[1] + r[3] / 2.0 - 12.0 * k);
    let path = NSBezierPath::bezierPath();
    for q in 0..2 {
        let pt = |i: usize| NSPoint::new(gx + p[q * 8 + i * 2] * k, gy + p[q * 8 + i * 2 + 1] * k);
        path.moveToPoint(pt(0));
        for i in 1..4 {
            path.lineToPoint(pt(i));
        }
        path.closePath();
    }
    path.setLineJoinStyle(objc2_app_kit::NSLineJoinStyle::Round);
    path.setLineWidth(1.6);
    ns(Rgba(0, 0, 0, 255), alpha).setFill();
    ns(Rgba(0, 0, 0, 255), alpha).setStroke();
    path.fill();
    path.stroke();
}

// ---------------------------------------------------------------- app, name and icon of the source

/// Path of the app with this bundle id, if macOS knows one.
fn app_path(bundle_id: &str) -> Option<String> {
    if bundle_id.is_empty() {
        return None;
    }
    let url = NSWorkspace::sharedWorkspace().URLForApplicationWithBundleIdentifier(&NSString::from_str(bundle_id))?;
    url.path().map(|p| p.to_string())
}

/// Name of the app that plays, as Finder shows it.
fn app_name(bundle_id: &str) -> Option<String> {
    let path = app_path(bundle_id)?;
    Some(NSFileManager::defaultManager().displayNameAtPath(&NSString::from_str(&path)).to_string())
}

/// The source's name: the one kept from the last draw when the source is the same, else `lookup()`.
fn cached_source_name(old: Option<(&str, &str)>, want_id: &str, lookup: impl FnOnce() -> String) -> String {
    match old {
        Some((id, name)) if id == want_id => name.to_string(),
        _ => lookup(),
    }
}

/// Icon of the app that plays.
fn app_icon(bundle_id: &str) -> Option<Retained<NSImage>> {
    let path = app_path(bundle_id)?;
    Some(NSWorkspace::sharedWorkspace().iconForFile(&NSString::from_str(&path)))
}

/// Same cover? The reading thread hands out one `Arc` per cover, so a pointer check is enough.
fn same_art(a: &Option<Arc<Vec<u8>>>, b: &Option<Arc<Vec<u8>>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        _ => false,
    }
}

/// Refresh rate of the display the pill is on (60 on a display that does not say).
fn refresh_rate(mtm: MainThreadMarker) -> f64 {
    NSScreen::screens(mtm).iter().next().map_or(60.0, |s| s.maximumFramesPerSecond() as f64)
}

// ---------------------------------------------------------------- layout, timer, render

/// Place the panel and the three buttons for the current state of the springs.
fn layout() {
    let Some((card, screen, state, ctl)) = DRAW.with(|d| {
        let mut d = d.try_borrow_mut().ok()?;
        let d = d.as_mut()?;
        let coll = view::collapsed_size(d.notch);
        let s = d.motion.shape(coll);
        let (sx, sy) = d.motion.squash(d.reduce_motion);
        d.card = ((s.w * sx).max(60.0), (s.h * sy).max(coll.1 * 0.9));
        let e = layout_of(d);
        let items = d.motion.item(2, d.reduce_motion);
        let o = ox(d);
        let rects = [e.prev, e.play, e.next].map(|r| {
            let r = view::xf_rect(r, [184.0, 166.0], items.1, items.2);
            [o + r[0], r[1], r[2], r[3]]
        });
        Some((d.card, d.screen, d.model.snapshot.as_ref().map(|s| s.state), (rects, items.0)))
    }) else {
        return;
    };
    VIEWS.with(|v| {
        let mut v = v.borrow_mut();
        let Some(v) = v.as_mut() else { return };
        v.panel.setFrame_display(rect(view::panel_frame(screen, card)), false);
        let (rects, alpha) = ctl;
        for (b, r) in [&v.prev, &v.play, &v.next].into_iter().zip(rects) {
            b.setFrame(rect(r));
            b.setAlphaValue(alpha);
            b.setHidden(alpha < 0.05);
        }
        if state.is_some() && state != v.play_symbol {
            tip(&v.play, if state == Some(PlayState::Playing) { "Pause" } else { "Play" });
            v.play_symbol = state;
        }
        v.root.setNeedsDisplay(true);
    });
}

fn stop_timer() {
    TIMER.with(|t| {
        if let Some((timer, _)) = t.borrow_mut().take() {
            timer.invalidate();
        }
    });
}

fn sync_timer(interval: Option<f64>) {
    let Some(interval) = interval else {
        stop_timer();
        return;
    };
    let same = TIMER.with(|t| t.borrow().as_ref().is_some_and(|(_, i)| (*i - interval).abs() < 1e-9));
    if same {
        return;
    }
    stop_timer();
    let block = RcBlock::new(|_t: NonNull<NSTimer>| tick());
    let timer = unsafe { NSTimer::scheduledTimerWithTimeInterval_repeats_block(interval, true, &block) };
    TIMER.with(|t| *t.borrow_mut() = Some((timer, interval)));
}

/// One timer step: move the springs and the disc, redraw, and set the next rate (or stop).
fn tick() {
    let next = DRAW.with(|d| {
        let mut d = d.borrow_mut();
        let d = d.as_mut()?;
        let now = Instant::now();
        let dt = now.duration_since(d.last_tick).as_secs_f64();
        d.last_tick = now;
        d.motion.step(dt, d.reduce_motion);
        let playing = d.model.snapshot.as_ref().is_some_and(|s| s.state == PlayState::Playing);
        Some(view::tick_interval(
            !d.motion.at_rest(),
            d.motion.spinning(d.reduce_motion),
            playing,
            d.model.expanded,
            d.reduce_motion,
            d.fps,
        ))
    });
    layout();
    sync_timer(next.flatten());
}

/// Show or update the player for `model`; hides it when `model.visible` is false. Call on every
/// `Out::render` of the controller (it only comes when something changed).
pub fn render(mtm: MainThreadMarker, model: &Model, reduce_motion: bool) {
    if !model.visible {
        hide(mtm);
        return;
    }
    let Some((screen, notch)) = screen_info(mtm) else { return };
    VIEWS.with(|v| {
        let mut v = v.borrow_mut();
        if v.is_none() {
            *v = Some(build(mtm));
        }
    });
    let snap = model.snapshot.clone();
    let playing = snap.as_ref().is_some_and(|s| s.state == PlayState::Playing);
    DRAW.with(|d| {
        let mut d = d.borrow_mut();
        let now = Instant::now();
        let key = |m: &Model| m.snapshot.as_ref().map(|s| (s.bundle_id.clone(), s.title.clone(), s.artist.clone()));
        let old = d.take();
        let new_track = old.as_ref().is_some_and(|o| key(&o.model).is_some() && key(&o.model) != key(model));
        let started = old.as_ref().map_or(now, |o| o.started);
        let mut motion = old.as_ref().map_or_else(|| Motion::with_playing(playing), |o| o.motion.clone());
        motion.set_playing(playing);
        if new_track {
            motion.track_changed(!reduce_motion);
        }
        motion.set_expanded(model.expanded, reduce_motion);

        // what the cover gave (prepared off the main thread): the disc, the halo and the tint
        let want = snap.as_ref().and_then(|s| s.artwork.clone());
        let carried = old.as_ref().filter(|o| same_art(&want, &o.art_key) && o.disc_img.is_some());
        let (look, disc_img, halo_img) = match carried {
            Some(o) => (o.look.clone(), o.disc_img.clone(), o.halo_img.clone()),
            None => {
                let look = crate::nowplaying_art::lookup(want.as_ref());
                let disc = rgba_image(&look.disc, ferrocast_core::nowplaying_art::DISC_PX, ferrocast_core::nowplaying_art::DISC_PX);
                let halo = rgba_image(&look.halo, ferrocast_core::nowplaying_art::HALO_PX, ferrocast_core::nowplaying_art::HALO_PX);
                (look, disc, halo)
            }
        };
        let want_id = snap.as_ref().map(|s| s.bundle_id.clone()).unwrap_or_default();
        let (source_icon, source_id) = match &old {
            Some(o) if o.source_id == want_id => (o.source_icon.clone(), o.source_id.clone()),
            _ => (app_icon(&want_id), want_id.clone()),
        };
        let source_name = cached_source_name(old.as_ref().map(|o| (o.source_id.as_str(), o.source_name.as_str())), &want_id, || {
            app_name(&want_id).unwrap_or_default()
        });

        // text: the title wrapped to two lines, the artist and the source name fitted
        let (title_lines, artist_text, chip_text) = match &snap {
            Some(s) => {
                let tattrs = title_attrs(&ns(DARK.text, 1.0));
                let lines = view::wrap_lines(&s.title, view::TITLE_W, 2, &|t| text_width(t, &tattrs));
                let line = match (s.artist.is_empty(), s.album.is_empty()) {
                    (false, false) => format!("{} \u{2013} {}", s.artist, s.album),
                    (false, true) => s.artist.clone(),
                    _ => s.album.clone(),
                };
                let aattrs = artist_attrs();
                let artist = view::truncate_to_width(&line, view::TITLE_W, &|t| text_width(t, &aattrs));
                let cattrs = chip_attrs();
                let free = ((CARD_W - notch.map_or(200.0, |n| n.width)) / 2.0 - 38.0 - 8.0).max(30.0);
                let chip = view::truncate_to_width(&source_name, free, &|t| text_width(t, &cattrs));
                (lines, artist, chip)
            }
            None => (vec![], String::new(), String::new()),
        };
        let card = old.as_ref().map_or_else(|| view::collapsed_size(notch), |o| o.card);
        let keep = |f: fn(&Draw) -> bool| old.as_ref().is_some_and(f);
        *d = Some(Draw {
            model: model.clone(),
            motion,
            last_tick: now,
            started,
            notch,
            screen,
            reduce_motion,
            fps: refresh_rate(mtm),
            card,
            look,
            disc_img,
            halo_img,
            art_key: want,
            source_icon,
            source_id,
            source_name,
            chip_text,
            title_lines,
            artist_text,
            scrub: old.as_ref().and_then(|o| o.scrub),
            hover_link: keep(|o| o.hover_link),
            hover_bar: keep(|o| o.hover_bar),
            hover_btn: old.as_ref().and_then(|o| o.hover_btn),
            pointer_in: keep(|o| o.pointer_in),
            press_link: keep(|o| o.press_link),
        });
    });
    VIEWS.with(|v| {
        if let Some(v) = v.borrow().as_ref() {
            v.panel.orderFrontRegardless();
        }
    });
    layout();
    // run one step now (the springs start moving, or snap with Reduce Motion), then at their own rate
    tick();
}

/// Remove the player from the screen and stop its timer.
pub fn hide(_mtm: MainThreadMarker) {
    stop_timer();
    VIEWS.with(|v| {
        if let Some(v) = v.borrow().as_ref() {
            v.panel.orderOut(None);
        }
    });
    DRAW.with(|d| *d.borrow_mut() = None);
}

/// Save what the panel shows as a PNG on a dark wall colour (a helper for the demo example: it
/// needs no Screen Recording permission). `false` when nothing is on screen or the write fails.
#[allow(deprecated)] // lockFocus: a debug picture helper, not worth a block-based image
pub fn save_png(path: &str) -> bool {
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep};
    VIEWS.with(|v| {
        let v = v.borrow();
        let Some(v) = v.as_ref() else { return false };
        let b = v.root.bounds();
        if b.size.width < 2.0 || b.size.height < 2.0 {
            return false;
        }
        let Some(rep) = v.root.bitmapImageRepForCachingDisplayInRect(b) else { return false };
        v.root.cacheDisplayInRect_toBitmapImageRep(b, &rep);
        // composite over the wall so the light that bleeds below the card is visible
        let (w, h) = (b.size.width + 40.0, b.size.height + 16.0);
        let img = NSImage::initWithSize(NSImage::alloc(), NSSize::new(w, h));
        img.lockFocus();
        NSColor::colorWithSRGBRed_green_blue_alpha(0.05, 0.043, 0.047, 1.0).setFill();
        NSBezierPath::fillRect(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(w, h)));
        // the card hangs from the top edge of the picture
        let cardless = NSImage::initWithSize(NSImage::alloc(), b.size);
        cardless.addRepresentation(&rep);
        cardless.drawInRect_fromRect_operation_fraction(
            NSRect::new(NSPoint::new(20.0, 0.0), b.size),
            NSRect::ZERO,
            NSCompositingOperation::SourceOver,
            1.0,
        );
        img.unlockFocus();
        let Some(tiff) = img.TIFFRepresentation() else { return false };
        let Some(out) = NSBitmapImageRep::imageRepWithData(&tiff) else { return false };
        let props = objc2_foundation::NSDictionary::new();
        let Some(png) = (unsafe { out.representationUsingType_properties(NSBitmapImageFileType::PNG, &props) }) else { return false };
        std::fs::write(path, png.to_vec()).is_ok()
    })
}

/// Average milliseconds one frame of the card takes to draw (a helper for the demo: the timer has
/// 8.3 ms per frame at 120 Hz). `None` when nothing is on screen.
pub fn draw_cost_ms(frames: usize) -> Option<f64> {
    VIEWS.with(|v| {
        let v = v.borrow();
        let v = v.as_ref()?;
        let b = v.root.bounds();
        let rep = v.root.bitmapImageRepForCachingDisplayInRect(b)?;
        let t = Instant::now();
        for _ in 0..frames {
            v.root.cacheDisplayInRect_toBitmapImageRep(b, &rep);
        }
        Some(t.elapsed().as_secs_f64() * 1000.0 / frames.max(1) as f64)
    })
}

#[cfg(test)]
mod name_cache_tests {
    use super::cached_source_name;
    use std::cell::Cell;

    #[test]
    fn the_source_name_is_looked_up_only_when_the_source_changes() {
        let calls = Cell::new(0);
        let look = |n: &str| {
            let n = n.to_string();
            let calls = &calls;
            move || {
                calls.set(calls.get() + 1);
                n
            }
        };
        let first = cached_source_name(None, "com.a", look("A"));
        assert_eq!((first.as_str(), calls.get()), ("A", 1));
        let again = cached_source_name(Some(("com.a", "A")), "com.a", look("A"));
        assert_eq!((again.as_str(), calls.get()), ("A", 1), "a redraw of the same source does no lookup");
        let other = cached_source_name(Some(("com.a", "A")), "com.b", look("B"));
        assert_eq!((other.as_str(), calls.get()), ("B", 2));
    }
}
