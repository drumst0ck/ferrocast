//! "Translating" glow (S3/S4): a click-through overlay panel that draws an Apple-Intelligence-style
//! animated gradient outline (rotating soft colours with a blurred halo) around the text being
//! translated, or a small frosted pill "Translating…" at the drop point.
//!
//! Every function here must run on the main thread (they take a `MainThreadMarker`); nothing
//! blocks. The geometry (`layout_for`, `flip_y`) is pure and unit-tested below; the drawing code
//! uses the very same functions.

use std::cell::{Cell, RefCell};

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject};
use objc2::{define_class, msg_send, MainThreadMarker, MainThreadOnly, Message};
use objc2_app_kit::{
    NSAnimationContext, NSBackingStoreType, NSColor, NSFont, NSPanel, NSPopUpMenuWindowLevel, NSScreen, NSTextAlignment, NSTextField,
    NSView, NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView, NSWindowCollectionBehavior,
    NSWindowStyleMask,
};
use objc2_core_graphics::{CGMutablePath, CGPath};
use objc2_foundation::{NSArray, NSNumber, NSPoint, NSRect, NSSize, NSString};
use objc2_quartz_core::{CABasicAnimation, CAGradientLayer, CALayer, CAMediaTiming, CAMediaTimingFunction, CAShapeLayer};

/// Gap between the selection rectangle and the glow outline, in points.
const PAD: f64 = 6.0;
/// Room around the outline inside the panel, for the halo.
const MARGIN: f64 = 36.0;
const OUTLINE_RADIUS: f64 = 8.0;
const PILL_W: f64 = 150.0;
const PILL_H: f64 = 36.0;
const PILL_RADIUS: f64 = 18.0;
const PILL_OFFSET: f64 = 28.0;
const CORE_WIDTH: f64 = 3.0;
const HALO_WIDTH: f64 = 16.0;
const HALO_BLUR: f64 = 14.0;
/// A tighter second halo, so the neon reads bright close to the tube and soft far away.
const HALO2_WIDTH: f64 = 8.0;
const HALO2_BLUR: f64 = 5.0;
/// The near-white core inside the coloured tube (the bright centre of a neon light).
const HOT_CORE_WIDTH: f64 = 1.2;
const SPIN_SECONDS: f64 = 1.6;
const BREATH_SECONDS: f64 = 1.2;
const FADE_IN: f64 = 0.25;
const FADE_OUT: f64 = 0.2;

/// What the glow surrounds. Coordinates are global top-left points (Accessibility's space).
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    /// Outline around the union of these `[x, y, w, h]` rectangles.
    Rects(Vec<[f64; 4]>),
    /// A "Translating…" pill centered 28 pt below this `[x, y]` point.
    Point([f64; 2]),
}

/// Where the panel goes and what is drawn inside it.
#[derive(Clone, Debug, PartialEq)]
struct PanelLayout {
    /// Panel frame in Cocoa (bottom-left) global coordinates.
    frame: [f64; 4],
    /// Outline rectangle in panel-local coordinates.
    outline: [f64; 4],
    radius: f64,
    pill: bool,
}

/// Cocoa bottom-left y of a rectangle given its top-left y, height and the primary screen height.
fn flip_y(top: f64, h: f64, screen_h: f64) -> f64 {
    screen_h - top - h
}

/// Panel frame and outline for `target`; `None` for an empty rect list.
fn layout_for(target: &Target, screen_h: f64) -> Option<PanelLayout> {
    let (rect, radius, pill) = match target {
        Target::Rects(rects) => {
            let u = crate::ax_text::rect_union(rects)?;
            ([u[0] - PAD, u[1] - PAD, u[2] + 2.0 * PAD, u[3] + 2.0 * PAD], OUTLINE_RADIUS, false)
        }
        Target::Point([x, y]) => ([x - PILL_W / 2.0, y + PILL_OFFSET, PILL_W, PILL_H], PILL_RADIUS, true),
    };
    let w = rect[2] + 2.0 * MARGIN;
    let h = rect[3] + 2.0 * MARGIN;
    Some(PanelLayout {
        frame: [rect[0] - MARGIN, flip_y(rect[1] - MARGIN, h, screen_h), w, h],
        outline: [MARGIN, MARGIN, rect[2], rect[3]],
        radius,
        pill,
    })
}

// ------------------------------------------------------------------ AppKit

define_class!(
    /// Borderless panel that never takes focus.
    #[unsafe(super(NSPanel))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastGlowPanel"]
    struct GlowPanel;

    impl GlowPanel {
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

thread_local! {
    static PANEL: RefCell<Option<Retained<GlowPanel>>> = const { RefCell::new(None) };
    /// Bumped by every show/hide so a stale fade-out completion never hides a newer glow.
    static GENERATION: Cell<u64> = const { Cell::new(0) };
}

fn rect(r: [f64; 4]) -> NSRect {
    NSRect::new(NSPoint::new(r[0], r[1]), NSSize::new(r[2], r[3]))
}

fn primary_screen_height(mtm: MainThreadMarker) -> f64 {
    NSScreen::screens(mtm).iter().next().map(|s| s.frame().size.height).unwrap_or(0.0)
}

fn panel(mtm: MainThreadMarker) -> Retained<GlowPanel> {
    PANEL.with(|slot| {
        if let Some(p) = slot.borrow().as_ref() {
            return p.clone();
        }
        let style = NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel;
        let p: Retained<GlowPanel> = unsafe {
            msg_send![
                super(mtm.alloc::<GlowPanel>().set_ivars(())),
                initWithContentRect: rect([0.0, 0.0, 10.0, 10.0]),
                styleMask: style,
                backing: NSBackingStoreType::Buffered,
                defer: false
            ]
        };
        unsafe { p.setReleasedWhenClosed(false) };
        p.setLevel(NSPopUpMenuWindowLevel);
        p.setFloatingPanel(true);
        p.setHidesOnDeactivate(false);
        p.setOpaque(false);
        p.setHasShadow(false);
        p.setBackgroundColor(Some(&NSColor::clearColor()));
        p.setIgnoresMouseEvents(true);
        p.setExcludedFromWindowsMenu(true);
        p.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        *slot.borrow_mut() = Some(p.clone());
        p
    })
}

fn srgb(r: f64, g: f64, b: f64) -> Retained<NSColor> {
    NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, 1.0)
}

/// Soft Apple-Intelligence palette, closing back on the first colour.
fn palette() -> Vec<Retained<NSColor>> {
    let c = |hex: u32| srgb(((hex >> 16) & 255) as f64 / 255.0, ((hex >> 8) & 255) as f64 / 255.0, (hex & 255) as f64 / 255.0);
    // Ferrocast neon reds around the website accent #f0535f, far enough apart (crimson, orange-red,
    // hot pink-red, a near-white hot spot and a dark red) that the turning gradient visibly moves
    vec![c(0xFF1A3D), c(0xFF6A2A), c(0xFF2D6F), c(0xFFD0D8), c(0xF0535F), c(0xB30028), c(0xFF1A3D)]
}

fn rounded_path(r: [f64; 4], radius: f64) -> objc2_core_foundation::CFRetained<CGPath> {
    let path = CGMutablePath::new();
    let rc =
        objc2_core_foundation::CGRect::new(objc2_core_foundation::CGPoint::new(r[0], r[1]), objc2_core_foundation::CGSize::new(r[2], r[3]));
    unsafe { CGMutablePath::add_rounded_rect(Some(&path), std::ptr::null(), rc, radius, radius) };
    unsafe { std::mem::transmute::<objc2_core_foundation::CFRetained<CGMutablePath>, objc2_core_foundation::CFRetained<CGPath>>(path) }
}

fn number(v: f64) -> Retained<NSNumber> {
    NSNumber::new_f64(v)
}

fn linear_forever(anim: &CABasicAnimation, seconds: f64) {
    anim.setDuration(seconds);
    anim.setRepeatCount(f32::INFINITY);
    anim.setTimingFunction(Some(&CAMediaTimingFunction::functionWithName(&NSString::from_str("linear"))));
}

/// One glowing ring: a conic gradient, masked by a stroked rounded rect. `blur` > 0 makes the
/// soft halo (Core Image Gaussian blur on a wrapper layer, so the blur applies after the mask).
fn ring(bounds: [f64; 4], outline: [f64; 4], radius: f64, width: f64, blur: f64, reduce_motion: bool) -> Retained<CALayer> {
    let inner = CALayer::new();
    inner.setFrame(rect(bounds));

    let (cx, cy) = (outline[0] + outline[2] / 2.0, outline[1] + outline[3] / 2.0);
    let side = (outline[2].hypot(outline[3]) + 2.0 * width).max(2.0 * width + 8.0);
    let gradient = CAGradientLayer::new();
    gradient.setFrame(rect([cx - side / 2.0, cy - side / 2.0, side, side]));
    gradient.setType(&NSString::from_str("conic"));
    gradient.setStartPoint(objc2_core_foundation::CGPoint::new(0.5, 0.5));
    gradient.setEndPoint(objc2_core_foundation::CGPoint::new(0.5, 0.0));
    let colors: Vec<_> = palette().iter().map(|c| c.CGColor()).collect();
    let objs: Vec<&AnyObject> = colors.iter().map(|c| unsafe { &*(&**c as *const _ as *const AnyObject) }).collect();
    unsafe { gradient.setColors(Some(&NSArray::from_slice(&objs))) };
    if !reduce_motion {
        let spin = CABasicAnimation::animationWithKeyPath(Some(&NSString::from_str("transform.rotation.z")));
        unsafe {
            spin.setFromValue(Some(&number(0.0)));
            spin.setToValue(Some(&number(-std::f64::consts::TAU)));
        }
        linear_forever(&spin, SPIN_SECONDS);
        gradient.addAnimation_forKey(&spin, Some(&NSString::from_str("spin")));
    }
    inner.addSublayer(&gradient);

    let mask = CAShapeLayer::new();
    mask.setFrame(rect(bounds));
    mask.setPath(Some(&rounded_path(outline, radius)));
    mask.setFillColor(None);
    mask.setStrokeColor(Some(&NSColor::whiteColor().CGColor()));
    mask.setLineWidth(width);
    unsafe { inner.setMask(Some(&mask)) };

    let outer = CALayer::new();
    outer.setFrame(rect(bounds));
    outer.addSublayer(&inner);
    if blur > 0.0 {
        if let Some(cls) = AnyClass::get(c"CIFilter") {
            unsafe {
                let filter: Option<Retained<AnyObject>> = msg_send![cls, filterWithName: &*NSString::from_str("CIGaussianBlur")];
                if let Some(f) = filter {
                    let _: () = msg_send![&*f, setValue: &*number(blur), forKey: &*NSString::from_str("inputRadius")];
                    let _: () = msg_send![&*outer, setFilters: &*NSArray::from_slice(&[&*f])];
                }
            }
        }
        if !reduce_motion {
            let breath = CABasicAnimation::animationWithKeyPath(Some(&NSString::from_str("opacity")));
            unsafe {
                breath.setFromValue(Some(&number(0.6)));
                breath.setToValue(Some(&number(1.0)));
            }
            breath.setDuration(BREATH_SECONDS);
            breath.setAutoreverses(true);
            breath.setRepeatCount(f32::INFINITY);
            breath.setTimingFunction(Some(&CAMediaTimingFunction::functionWithName(&NSString::from_str("easeInEaseOut"))));
            outer.addAnimation_forKey(&breath, Some(&NSString::from_str("breath")));
        }
    }
    outer
}

/// The thin near-white line in the middle of the tube that makes the ring read as neon.
fn hot_core(bounds: [f64; 4], outline: [f64; 4], radius: f64) -> Retained<CAShapeLayer> {
    let core = CAShapeLayer::new();
    core.setFrame(rect(bounds));
    core.setPath(Some(&rounded_path(outline, radius)));
    core.setFillColor(None);
    core.setStrokeColor(Some(&NSColor::colorWithSRGBRed_green_blue_alpha(1.0, 0.9, 0.92, 0.85).CGColor()));
    core.setLineWidth(HOT_CORE_WIDTH);
    core
}

fn set_alpha(panel: &GlowPanel, alpha: f64, seconds: f64, done: Option<RcBlock<dyn Fn()>>) {
    let p = panel.retain();
    let body = RcBlock::new(move |ctx: std::ptr::NonNull<NSAnimationContext>| {
        unsafe { ctx.as_ref() }.setDuration(seconds);
        unsafe {
            let anim: Retained<GlowPanel> = msg_send![&*p, animator];
            anim.setAlphaValue(alpha);
        }
    });
    match done {
        Some(d) => NSAnimationContext::runAnimationGroup_completionHandler(&body, Some(&d)),
        None => NSAnimationContext::runAnimationGroup_completionHandler(&body, None),
    }
}

/// Show the glow around `target`, replacing any glow already on screen. Main thread only.
pub fn show(mtm: MainThreadMarker, target: Target, reduce_motion: bool) {
    let Some(layout) = layout_for(&target, primary_screen_height(mtm)) else { return };
    let gen = GENERATION.with(|g| {
        g.set(g.get() + 1);
        g.get()
    });
    let _ = gen;
    let panel = panel(mtm);
    let size = [layout.frame[2], layout.frame[3]];
    let bounds = [0.0, 0.0, size[0], size[1]];

    let content = NSView::initWithFrame(mtm.alloc::<NSView>(), rect(bounds));
    content.setWantsLayer(true);
    if layout.pill {
        let fx = NSVisualEffectView::initWithFrame(mtm.alloc::<NSVisualEffectView>(), rect(layout.outline));
        fx.setMaterial(NSVisualEffectMaterial::HUDWindow);
        fx.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
        fx.setState(NSVisualEffectState::Active);
        fx.setWantsLayer(true);
        if let Some(l) = fx.layer() {
            l.setCornerRadius(layout.radius);
            l.setMasksToBounds(true);
        }
        let label = NSTextField::labelWithString(&NSString::from_str("Translating\u{2026}"), mtm);
        label.setFont(Some(&NSFont::systemFontOfSize(13.0)));
        label.setAlignment(NSTextAlignment::Center);
        label.sizeToFit();
        let lh = label.frame().size.height;
        label.setFrame(rect([0.0, (PILL_H - lh) / 2.0, PILL_W, lh]));
        fx.addSubview(&label);
        content.addSubview(&fx);
    }
    if let Some(root) = content.layer() {
        // halo first (under), crisp ring on top
        root.addSublayer(&ring(bounds, layout.outline, layout.radius, HALO_WIDTH, HALO_BLUR, reduce_motion));
        root.addSublayer(&ring(bounds, layout.outline, layout.radius, HALO2_WIDTH, HALO2_BLUR, reduce_motion));
        root.addSublayer(&ring(bounds, layout.outline, layout.radius, CORE_WIDTH, 0.0, reduce_motion));
        root.addSublayer(&hot_core(bounds, layout.outline, layout.radius));
    }
    panel.setContentView(Some(&content));
    panel.setFrame_display(rect(layout.frame), true);
    panel.setAlphaValue(0.0);
    panel.orderFrontRegardless();
    set_alpha(&panel, 1.0, FADE_IN, None);
}

/// Fade the glow out (0.2 s) and remove it. No-op when nothing is shown. Main thread only.
pub fn hide(mtm: MainThreadMarker) {
    let panel = PANEL.with(|slot| slot.borrow().clone());
    let Some(panel) = panel else { return };
    if !panel.isVisible() {
        return;
    }
    let gen = GENERATION.with(|g| {
        g.set(g.get() + 1);
        g.get()
    });
    let p = panel.clone();
    let done: RcBlock<dyn Fn()> = RcBlock::new(move || {
        if GENERATION.with(|g| g.get()) == gen {
            p.orderOut(None);
            p.setContentView(None);
        }
    });
    let _ = mtm;
    set_alpha(&panel, 0.0, FADE_OUT, Some(done));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flip_converts_top_left_to_bottom_left() {
        assert_eq!(flip_y(0.0, 100.0, 1000.0), 900.0);
        assert_eq!(flip_y(200.0, 60.0, 1000.0), 740.0);
        assert_eq!(flip_y(-50.0, 10.0, 1000.0), 1040.0);
    }

    #[test]
    fn rects_layout_pads_union_and_flips() {
        let l = layout_for(&Target::Rects(vec![[200.0, 200.0, 400.0, 60.0]]), 1000.0).unwrap();
        assert!(!l.pill);
        assert_eq!(l.radius, 8.0);
        // outline = union padded by 6, inside a panel margin of 24
        assert_eq!(l.outline, [MARGIN, MARGIN, 412.0, 72.0]);
        assert_eq!(l.frame, [194.0 - MARGIN, 1000.0 - 194.0 - 72.0 - MARGIN, 412.0 + 2.0 * MARGIN, 72.0 + 2.0 * MARGIN]);
    }

    #[test]
    fn rects_layout_uses_union_of_several() {
        let l = layout_for(&Target::Rects(vec![[100.0, 100.0, 50.0, 20.0], [100.0, 130.0, 80.0, 20.0]]), 800.0).unwrap();
        assert_eq!(l.outline[2], 80.0 + 2.0 * PAD);
        assert_eq!(l.outline[3], 50.0 + 2.0 * PAD);
    }

    #[test]
    fn empty_rects_have_no_layout() {
        assert_eq!(layout_for(&Target::Rects(vec![]), 800.0), None);
    }

    #[test]
    fn point_layout_is_a_pill_centered_28pt_below() {
        let l = layout_for(&Target::Point([800.0, 400.0]), 1000.0).unwrap();
        assert!(l.pill);
        assert_eq!(l.radius, 18.0);
        assert_eq!(l.outline, [MARGIN, MARGIN, 150.0, 36.0]);
        // pill top-left = (725, 428) -> bottom-left y = 1000 - 428 - 36 = 536
        assert_eq!(l.frame, [725.0 - MARGIN, 536.0 - MARGIN, 150.0 + 2.0 * MARGIN, 36.0 + 2.0 * MARGIN]);
    }
}
