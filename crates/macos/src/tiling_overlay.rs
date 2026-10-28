//! What automatic tiling draws on top of the windows: the highlight border around the focused
//! tiled window and the quick-action bar above a window.
//!
//! Both are borderless, non-activating panels. The border ignores the mouse; the bar takes clicks
//! but never becomes key, so using it does not steal focus from the window it acts on. Every
//! function here runs on the main thread (`MainThreadMarker`); the worker only describes what to
//! draw ([`crate::tiling::Overlay`]) and the app calls [`show`] when that changes. The geometry
//! and the rules for when each is shown are pure and tested in `ferrocast_core::tiling`.

use std::cell::RefCell;
use std::rc::Rc;

use ferrocast_core::tiling::QuickAction;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{define_class, msg_send, sel, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSBackingStoreType, NSBezelStyle, NSButton, NSColor, NSFloatingWindowLevel, NSImage, NSPanel, NSScreen, NSStatusWindowLevel, NSView,
    NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_foundation::{NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};

use crate::tiling::Overlay;

define_class!(
    /// Borderless panel that never takes focus.
    #[unsafe(super(NSPanel))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastTilingPanel"]
    struct TilingPanel;

    impl TilingPanel {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key(&self) -> bool {
            false
        }

        #[unsafe(method(canBecomeMainWindow))]
        fn can_become_main(&self) -> bool {
            false
        }
    }
);

define_class!(
    /// Receives the bar's button clicks; the button's tag is the index into `QuickAction::ALL`.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastQuickBarTarget"]
    struct BarTarget;

    impl BarTarget {
        #[unsafe(method(clicked:))]
        fn clicked(&self, sender: &NSButton) {
            let i = sender.tag();
            let cb = CALLBACK.with(|c| c.borrow().clone());
            if let (Some(cb), Some(a)) = (cb, usize::try_from(i).ok().and_then(|i| QuickAction::ALL.get(i))) {
                cb(*a);
            }
        }
    }

    unsafe impl NSObjectProtocol for BarTarget {}
);

/// Colours of the bar's own background, for a dark or a light system appearance (r, g, b, a).
#[derive(Debug, PartialEq)]
struct BarColors {
    background: [f64; 4],
    border: [f64; 4],
    /// Colour of the button symbols.
    tint: [f64; 3],
}

fn bar_colors(dark: bool) -> BarColors {
    if dark {
        BarColors { background: [0.14, 0.14, 0.16, 0.98], border: [1.0, 1.0, 1.0, 0.18], tint: [0.95, 0.95, 0.97] }
    } else {
        BarColors { background: [0.97, 0.97, 0.98, 0.98], border: [0.0, 0.0, 0.0, 0.16], tint: [0.12, 0.12, 0.14] }
    }
}

fn is_dark(mtm: MainThreadMarker) -> bool {
    use objc2_app_kit::NSApplication;
    NSApplication::sharedApplication(mtm).effectiveAppearance().name().to_string().contains("Dark")
}

struct BarParts {
    panel: Retained<TilingPanel>,
    view: Retained<NSView>,
    buttons: Vec<Retained<NSButton>>,
    /// Kept alive: buttons hold their target weakly.
    _target: Retained<BarTarget>,
}

type OnAction = Rc<dyn Fn(QuickAction)>;

thread_local! {
    static BORDER: RefCell<Option<Retained<TilingPanel>>> = const { RefCell::new(None) };
    static BAR: RefCell<Option<BarParts>> = const { RefCell::new(None) };
    static CALLBACK: RefCell<Option<OnAction>> = const { RefCell::new(None) };
}

/// Register what a click on a bar button does (it only has to forward a message). Main thread.
pub fn set_on_action(cb: Box<dyn Fn(QuickAction)>) {
    CALLBACK.with(|c| *c.borrow_mut() = Some(Rc::from(cb)));
}

fn primary_height(mtm: MainThreadMarker) -> f64 {
    NSScreen::screens(mtm).iter().next().map(|s| s.frame().size.height).unwrap_or(0.0)
}

/// Cocoa (bottom-left) rect of a global top-left `[x, y, w, h]`.
fn cocoa(r: [f64; 4], primary_h: f64) -> NSRect {
    NSRect::new(NSPoint::new(r[0], primary_h - r[1] - r[3]), NSSize::new(r[2], r[3]))
}

fn new_panel(mtm: MainThreadMarker, level: isize, ignores_mouse: bool, shadow: bool) -> Retained<TilingPanel> {
    let style = NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel;
    let p: Retained<TilingPanel> = unsafe {
        msg_send![
            super(mtm.alloc::<TilingPanel>().set_ivars(())),
            initWithContentRect: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(10.0, 10.0)),
            styleMask: style,
            backing: NSBackingStoreType::Buffered,
            defer: false
        ]
    };
    unsafe { p.setReleasedWhenClosed(false) };
    p.setLevel(level);
    p.setFloatingPanel(true);
    p.setHidesOnDeactivate(false);
    p.setOpaque(false);
    p.setHasShadow(shadow);
    p.setBackgroundColor(Some(&NSColor::clearColor()));
    p.setIgnoresMouseEvents(ignores_mouse);
    p.setExcludedFromWindowsMenu(true);
    p.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::IgnoresCycle,
    );
    p
}

fn show_border(mtm: MainThreadMarker, b: &crate::tiling::BorderSpec, primary_h: f64) {
    BORDER.with(|slot| {
        let mut slot = slot.borrow_mut();
        let panel = slot.get_or_insert_with(|| {
            let p = new_panel(mtm, NSFloatingWindowLevel, true, false);
            let view = NSView::initWithFrame(mtm.alloc::<NSView>(), NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(10.0, 10.0)));
            view.setWantsLayer(true);
            p.setContentView(Some(&view));
            p
        });
        panel.setFrame_display(cocoa(b.frame, primary_h), true);
        if let Some(layer) = panel.contentView().and_then(|v| v.layer()) {
            let [r, g, bl] = b.color;
            let color = NSColor::colorWithSRGBRed_green_blue_alpha(r as f64 / 255.0, g as f64 / 255.0, bl as f64 / 255.0, 1.0);
            layer.setBorderWidth(b.width);
            layer.setBorderColor(Some(&color.CGColor()));
            layer.setCornerRadius(b.radius);
        }
        panel.orderFrontRegardless();
    });
}

const SYMBOLS: [&str; 5] =
    ["arrow.up.left.and.arrow.down.right", "macwindow.on.rectangle", "display.2", "arrow.left.arrow.right", "nosign"];
const TIPS: [&str; 5] = ["Focus mode", "Float or tile this window", "Move to next display", "Swap with main slot", "Stop tiling this app"];

fn make_bar(mtm: MainThreadMarker) -> BarParts {
    // a subtle shadow from the window itself; the content below is a solid rounded pill
    let panel = new_panel(mtm, NSStatusWindowLevel, false, true);
    let (w, h) = (ferrocast_core::tiling::BAR_W, ferrocast_core::tiling::BAR_H);
    let fx = NSView::initWithFrame(mtm.alloc::<NSView>(), NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(w, h)));
    fx.setWantsLayer(true);
    if let Some(l) = fx.layer() {
        l.setCornerRadius(h / 2.0);
        l.setMasksToBounds(true);
        l.setBorderWidth(0.5);
    }
    let mut buttons = vec![];
    let target: Retained<BarTarget> = unsafe { msg_send![super(mtm.alloc::<BarTarget>().set_ivars(())), init] };
    let t: &AnyObject = &target;
    for (i, (sym, tip)) in SYMBOLS.iter().zip(TIPS).enumerate() {
        let tip = &*ferrocast_core::i18n::tr(tip);
        let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(&NSString::from_str(sym), Some(&NSString::from_str(tip)));
        let button = unsafe {
            match image {
                Some(img) => NSButton::buttonWithImage_target_action(&img, Some(t), Some(sel!(clicked:)), mtm),
                None => NSButton::buttonWithTitle_target_action(&NSString::from_str(tip), Some(t), Some(sel!(clicked:)), mtm),
            }
        };
        button.setBezelStyle(NSBezelStyle::Toolbar);
        button.setBordered(false);
        button.setRefusesFirstResponder(true);
        button.setTag(i as isize);
        button.setToolTip(Some(&NSString::from_str(tip)));
        button.setFrame(NSRect::new(NSPoint::new(10.0 + 36.0 * i as f64, (h - 24.0) / 2.0), NSSize::new(32.0, 24.0)));
        fx.addSubview(&button);
        buttons.push(button);
    }
    panel.setContentView(Some(&fx));
    BarParts { panel, view: fx, buttons, _target: target }
}

/// Paint the pill for the current system appearance (it can change while Ferrocast runs).
fn style_bar(mtm: MainThreadMarker, parts: &BarParts) {
    let c = bar_colors(is_dark(mtm));
    let color = |a: [f64; 4]| NSColor::colorWithSRGBRed_green_blue_alpha(a[0], a[1], a[2], a[3]);
    if let Some(l) = parts.view.layer() {
        l.setBackgroundColor(Some(&color(c.background).CGColor()));
        l.setBorderColor(Some(&color(c.border).CGColor()));
    }
    let tint = color([c.tint[0], c.tint[1], c.tint[2], 1.0]);
    for b in &parts.buttons {
        b.setContentTintColor(Some(&tint));
    }
}

fn show_bar(mtm: MainThreadMarker, frame: [f64; 4], primary_h: f64) {
    BAR.with(|slot| {
        let mut slot = slot.borrow_mut();
        let parts = slot.get_or_insert_with(|| make_bar(mtm));
        style_bar(mtm, parts);
        parts.panel.setFrame_display(cocoa(frame, primary_h), true);
        parts.panel.orderFrontRegardless();
    });
}

/// Make the screen show `o`: the border and the bar, each hidden when `None`. Main thread only.
pub fn show(mtm: MainThreadMarker, o: &Overlay) {
    let primary_h = primary_height(mtm);
    match &o.border {
        Some(b) => show_border(mtm, b, primary_h),
        None => BORDER.with(|s| {
            if let Some(p) = s.borrow().as_ref() {
                p.orderOut(None);
            }
        }),
    }
    match &o.bar {
        Some(b) => show_bar(mtm, b.frame, primary_h),
        None => BAR.with(|s| {
            if let Some(p) = s.borrow().as_ref() {
                p.panel.orderOut(None);
            }
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_has_its_own_nearly_opaque_background_in_both_appearances() {
        let (d, l) = (bar_colors(true), bar_colors(false));
        assert!(d.background[3] >= 0.95 && l.background[3] >= 0.95, "solid, readable over any window");
        assert!(d.background[0] < 0.3 && l.background[0] > 0.8, "dark pill in dark mode, light pill in light mode");
        assert!(d.tint[0] > 0.8 && l.tint[0] < 0.3, "symbols contrast with the pill");
        assert_ne!(d, l);
    }
}
