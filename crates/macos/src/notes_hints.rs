//! Footer hint bar of the Notes window (S11): a 30 pt strip at the very bottom, below the
//! Format Bar, that lists the commands that can be pressed ("Actions ⌘K"), in the style of the
//! launcher's footer. Left: the typing hint. Right: clickable hint groups (label + key cap).
//! Which parts fit is decided by the pure [`visible_hints`], so it is unit-tested without AppKit.
//! All AppKit calls happen on the main thread.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{define_class, msg_send, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSBezierPath, NSEvent, NSFont, NSFontAttributeName, NSFontWeightMedium, NSForegroundColorAttributeName,
    NSStringDrawing, NSTextField, NSTrackingArea, NSTrackingAreaOptions, NSView,
};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};

use crate::notes_popups::{token_fill, TokenFill};
use crate::notes_theme::{self as theme, Token};

// ---------------------------------------------------------------- pure logic

/// Height of the bar in points (hairline included).
pub const HEIGHT: f64 = 30.0;
/// Minimum space between the window edges and the content.
pub const PAD: f64 = 12.0;
/// Space between two hint groups (hover backgrounds included).
pub const GROUP_GAP: f64 = 8.0;
/// Minimum space between the typing hint and the first hint group.
pub const TYPING_GAP: f64 = 16.0;

/// The clickable hints, in priority order (the first one is kept longest).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hint {
    Actions,
    Browse,
    Pin,
    New,
}

impl Hint {
    /// Priority order: `Actions` is the rightmost group and the last one dropped.
    pub const ALL: [Hint; 4] = [Hint::Actions, Hint::Browse, Hint::Pin, Hint::New];

    pub fn label(self) -> &'static str {
        match self {
            Hint::Actions => "Actions",
            Hint::Browse => "Browse",
            Hint::Pin => "Pin",
            Hint::New => "New",
        }
    }

    pub fn keys(self) -> &'static str {
        match self {
            Hint::Actions => "⌘K",
            Hint::Browse => "⌘P",
            Hint::Pin => "⇧⌘P",
            Hint::New => "⌘N",
        }
    }
}

/// The typing hint shown on the left.
pub const TYPING_HINT: &str = "Type / for formats · : for emoji";

/// Measured widths: the typing hint and every group, in [`Hint::ALL`] order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sizes {
    pub typing: f64,
    pub groups: [f64; 4],
}

/// What fits: the typing hint or not, and how many groups (the first `groups` of [`Hint::ALL`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VisibleHints {
    pub typing: bool,
    pub groups: usize,
}

/// Width taken by the first `n` groups with the gaps between them.
pub fn groups_width(sizes: &Sizes, n: usize) -> f64 {
    let n = n.min(4);
    if n == 0 {
        return 0.0;
    }
    sizes.groups[..n].iter().sum::<f64>() + GROUP_GAP * (n as f64 - 1.0)
}

/// Decide what fits in a bar `width` points wide with at least [`PAD`] on both sides. The
/// typing hint goes first, then whole groups from the lowest priority up; never half a group.
/// `Actions` always stays, even when it alone does not fit.
pub fn visible_hints(width: f64, sizes: &Sizes) -> VisibleHints {
    let avail = width - 2.0 * PAD;
    if sizes.typing + TYPING_GAP + groups_width(sizes, 4) <= avail {
        return VisibleHints { typing: true, groups: 4 };
    }
    let groups = (1..=4).rev().find(|&n| groups_width(sizes, n) <= avail).unwrap_or(1);
    VisibleHints { typing: false, groups }
}

// ---------------------------------------------------------------- AppKit

const HOVER_H: f64 = 22.0;
const HOVER_RADIUS: f64 = 6.0;
const HOVER_PAD: f64 = 4.0;
const LABEL_GAP: f64 = 8.0;
const CAP_H: f64 = 18.0;
const CAP_PAD: f64 = 8.0;
const CAP_RADIUS: f64 = 6.0;
const FONT_SIZE: f64 = 11.0;

type HintCb = Rc<dyn Fn(Hint)>;

struct GroupIvars {
    hint: Hint,
    hover: Cell<bool>,
    cb: Rc<RefCell<Option<HintCb>>>,
    label_w: f64,
    keys_w: f64,
}

fn label_font() -> Retained<NSFont> {
    NSFont::systemFontOfSize(FONT_SIZE)
}

fn keys_font() -> Retained<NSFont> {
    NSFont::systemFontOfSize_weight(FONT_SIZE, unsafe { NSFontWeightMedium })
}

fn attrs(font: &NSFont, token: Token) -> Retained<NSDictionary<NSString, AnyObject>> {
    let color = theme::color(token);
    let (k1, k2) = unsafe { (NSFontAttributeName, NSForegroundColorAttributeName) };
    NSDictionary::from_slices(&[k1, k2], &[font as &AnyObject, &*color as &AnyObject])
}

fn text_size(text: &str, font: &NSFont) -> NSSize {
    let a = attrs(font, Token::Sub);
    unsafe { NSString::from_str(text).sizeWithAttributes(Some(&a)) }
}

/// Width of a hint group: hover padding, label, gap, key cap, hover padding.
fn group_width(label_w: f64, keys_w: f64) -> f64 {
    HOVER_PAD + label_w + LABEL_GAP + keys_w + 2.0 * CAP_PAD + HOVER_PAD
}

define_class!(
    /// One clickable hint: `Sub` label, `Field` key cap with `SubStrong` keys, `Field` hover background.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastNotesHintGroup"]
    #[ivars = GroupIvars]
    struct HintGroup;

    impl HintGroup {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let iv = self.ivars();
            let b = self.bounds();
            if iv.hover.get() {
                theme::color(Token::Field).setFill();
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(b, HOVER_RADIUS, HOVER_RADIUS).fill();
            }
            let lf = label_font();
            let label = NSString::from_str(iv.hint.label());
            let lh = unsafe { label.sizeWithAttributes(Some(&attrs(&lf, Token::Sub))) }.height;
            unsafe {
                label.drawAtPoint_withAttributes(
                    NSPoint::new(HOVER_PAD, ((b.size.height - lh) / 2.0).round()),
                    Some(&attrs(&lf, Token::Sub)),
                );
            }
            let cap_x = HOVER_PAD + iv.label_w + LABEL_GAP;
            let cap_w = iv.keys_w + 2.0 * CAP_PAD;
            let cap = NSRect::new(NSPoint::new(cap_x, (b.size.height - CAP_H) / 2.0), NSSize::new(cap_w, CAP_H));
            theme::color(Token::Field).setFill();
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(cap, CAP_RADIUS, CAP_RADIUS).fill();
            let kf = keys_font();
            let keys = NSString::from_str(iv.hint.keys());
            let ka = attrs(&kf, Token::SubStrong);
            let kh = unsafe { keys.sizeWithAttributes(Some(&ka)) }.height;
            unsafe {
                keys.drawAtPoint_withAttributes(NSPoint::new(cap_x + CAP_PAD, ((b.size.height - kh) / 2.0).round()), Some(&ka));
            }
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        // The panel is movable by its background: a click on a hint must not start a drag.
        #[unsafe(method(mouseDownCanMoveWindow))]
        fn mouse_down_can_move_window(&self) -> bool {
            false
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, _e: &NSEvent) {}

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, e: &NSEvent) {
            let p = self.convertPoint_fromView(e.locationInWindow(), None);
            let b = self.bounds();
            if p.x < 0.0 || p.y < 0.0 || p.x > b.size.width || p.y > b.size.height {
                return;
            }
            let cb = self.ivars().cb.borrow().clone();
            if let Some(cb) = cb {
                cb(self.ivars().hint);
            }
        }

        #[unsafe(method(mouseEntered:))]
        fn mouse_entered(&self, _e: &NSEvent) {
            self.set_hover(true);
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _e: &NSEvent) {
            self.set_hover(false);
        }
    }
);

impl HintGroup {
    fn set_hover(&self, on: bool) {
        self.ivars().hover.set(on);
        unsafe {
            let _: () = msg_send![self, setNeedsDisplay: true];
        }
    }
}

/// The footer view. Create it once, mount [`HintBar::view`] at the bottom of the content view
/// and call [`HintBar::layout`] whenever the width changes.
pub struct HintBar {
    root: Retained<TokenFill>,
    typing: Retained<NSTextField>,
    groups: Vec<Retained<HintGroup>>,
    sizes: Sizes,
    last: Cell<VisibleHints>,
    cb: Rc<RefCell<Option<HintCb>>>,
}

impl HintBar {
    /// Build the strip. Panics when not on the main thread.
    pub fn new() -> HintBar {
        let mtm = MainThreadMarker::new().expect("HintBar must be created on the main thread");
        let width = 560.0;
        let root = token_fill(mtm, NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, HEIGHT)), Token::Bg);
        root.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewMaxYMargin);
        // 1 pt `Sep` hairline on top, the same recipe as the Format Bar.
        let top = token_fill(mtm, NSRect::new(NSPoint::new(0.0, HEIGHT - 1.0), NSSize::new(width, 1.0)), Token::Sep);
        top.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewMinYMargin);
        root.addSubview(&top);

        let typing = NSTextField::labelWithString(&NSString::from_str(TYPING_HINT), mtm);
        typing.setFont(Some(&label_font()));
        typing.setTextColor(Some(&theme::color(Token::Sub)));
        let typing_w = text_size(TYPING_HINT, &label_font()).width.ceil();
        root.addSubview(&typing);

        let cb: Rc<RefCell<Option<HintCb>>> = Rc::default();
        let mut groups = Vec::new();
        let mut widths = [0.0; 4];
        for (i, hint) in Hint::ALL.iter().copied().enumerate() {
            let label_w = text_size(hint.label(), &label_font()).width.ceil();
            let keys_w = text_size(hint.keys(), &keys_font()).width.ceil();
            let w = group_width(label_w, keys_w);
            widths[i] = w;
            let iv = GroupIvars { hint, hover: Cell::new(false), cb: cb.clone(), label_w, keys_w };
            let g: Retained<HintGroup> = unsafe {
                msg_send![
                    super(mtm.alloc::<HintGroup>().set_ivars(iv)),
                    initWithFrame: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(w, HOVER_H))
                ]
            };
            g.setAutoresizingMask(NSAutoresizingMaskOptions::empty());
            g.setToolTip(Some(&NSString::from_str(&format!("{} {}", hint.label(), hint.keys()))));
            let area = unsafe {
                NSTrackingArea::initWithRect_options_owner_userInfo(
                    mtm.alloc(),
                    NSRect::ZERO,
                    NSTrackingAreaOptions::MouseEnteredAndExited
                        | NSTrackingAreaOptions::ActiveAlways
                        | NSTrackingAreaOptions::InVisibleRect,
                    Some(&g),
                    None,
                )
            };
            g.addTrackingArea(&area);
            root.addSubview(&g);
            groups.push(g);
        }
        let bar = HintBar {
            root,
            typing,
            groups,
            sizes: Sizes { typing: typing_w, groups: widths },
            last: Cell::new(VisibleHints { typing: false, groups: 4 }),
            cb,
        };
        bar.layout(width);
        bar
    }

    pub fn view(&self) -> &NSView {
        &self.root
    }

    /// Measured widths (for the demo and tests of the rule).
    pub fn sizes(&self) -> Sizes {
        self.sizes
    }

    /// What is shown after the last [`HintBar::layout`].
    pub fn visible(&self) -> VisibleHints {
        self.last.get()
    }

    /// Called with the hint that was clicked.
    pub fn set_on_click(&self, cb: Box<dyn Fn(Hint)>) {
        *self.cb.borrow_mut() = Some(Rc::from(cb));
    }

    /// Frame (bar coordinates) of the group of `hint`, when it is shown.
    pub fn group_frame(&self, hint: Hint) -> Option<NSRect> {
        let i = Hint::ALL.iter().position(|h| *h == hint)?;
        let g = &self.groups[i];
        (!g.isHidden()).then(|| g.frame())
    }

    /// Place the typing hint and the groups for a bar `width` points wide: groups right to left
    /// in priority order, the ones that do not fit are hidden.
    pub fn layout(&self, width: f64) {
        let vis = visible_hints(width, &self.sizes);
        self.last.set(vis);
        let y = ((HEIGHT - 1.0 - HOVER_H) / 2.0).floor();
        let mut right = width - PAD;
        for (i, g) in self.groups.iter().enumerate() {
            let shown = i < vis.groups;
            g.setHidden(!shown);
            if shown {
                let w = self.sizes.groups[i];
                g.setFrame(NSRect::new(NSPoint::new(right - w, y), NSSize::new(w, HOVER_H)));
                right -= w + GROUP_GAP;
            }
        }
        self.typing.setHidden(!vis.typing);
        if vis.typing {
            let h = 14.0;
            self.typing.setFrame(NSRect::new(
                NSPoint::new(PAD + HOVER_PAD, ((HEIGHT - 1.0 - h) / 2.0).floor()),
                NSSize::new(self.sizes.typing + 4.0, h),
            ));
        }
    }
}

impl Default for HintBar {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Widths like the ones AppKit measures for the real strings (11 pt system font).
    fn sizes() -> Sizes {
        Sizes { typing: 156.0, groups: [90.0, 89.0, 78.0, 75.0] }
    }

    fn at(w: f64) -> VisibleHints {
        visible_hints(w, &sizes())
    }

    #[test]
    fn at_the_minimum_width_the_lowest_priorities_go_first() {
        // avail 296: Actions + Browse + Pin = 90 + 89 + 78 + 2 * 8 = 273 fits, New (+ 83) does not.
        assert_eq!(at(320.0), VisibleHints { typing: false, groups: 3 });
        // avail 176: Actions + Browse = 187 does not fit, Actions alone does.
        assert_eq!(at(200.0), VisibleHints { typing: false, groups: 1 });
    }

    #[test]
    fn mid_widths_drop_whole_groups_never_half_one() {
        assert_eq!(at(400.0), VisibleHints { typing: false, groups: 4 });
        assert_eq!(at(480.0), VisibleHints { typing: false, groups: 4 });
        assert_eq!(at(250.0), VisibleHints { typing: false, groups: 2 });
    }

    #[test]
    fn the_typing_hint_comes_back_when_everything_else_fits() {
        // 156 + 16 + (90 + 89 + 78 + 75 + 24) + 24 = 552
        assert_eq!(at(551.0), VisibleHints { typing: false, groups: 4 });
        assert_eq!(at(552.0), VisibleHints { typing: true, groups: 4 });
        assert_eq!(at(560.0), VisibleHints { typing: true, groups: 4 });
        assert_eq!(at(700.0), VisibleHints { typing: true, groups: 4 });
    }

    #[test]
    fn whatever_is_shown_keeps_the_padding() {
        for w in (320..900).step_by(7) {
            let v = visible_hints(w as f64, &sizes());
            let need = groups_width(&sizes(), v.groups) + if v.typing { sizes().typing + TYPING_GAP } else { 0.0 };
            if v.groups > 1 || v.typing {
                assert!(need + 2.0 * PAD <= w as f64, "width {w}: {v:?} needs {need}");
            }
            assert!(v.groups >= 1, "Actions always stays");
            assert!(!v.typing || v.groups == 4, "the typing hint goes before any group");
        }
    }

    #[test]
    fn a_window_too_narrow_for_anything_keeps_actions() {
        assert_eq!(visible_hints(100.0, &sizes()), VisibleHints { typing: false, groups: 1 });
    }

    #[test]
    fn hints_are_in_priority_order_with_their_key_caps() {
        let all: Vec<_> = Hint::ALL.iter().map(|h| (h.label(), h.keys())).collect();
        assert_eq!(all, vec![("Actions", "⌘K"), ("Browse", "⌘P"), ("Pin", "⇧⌘P"), ("New", "⌘N")]);
    }
}
