//! Notes pop-ups (S6, S9): the suggestion popup shared by the `/` menu and the `:` emoji list,
//! and the Format Bar at the bottom of the window. Standalone views with a small API; the
//! editor owns the wiring. All AppKit calls happen on the main thread; the pure helpers are
//! unit-tested below.
//!
//! Look (T3, S2/S4/S8): every colour comes from `notes_theme` dynamic tokens, never from system
//! semantic colours. The selected row is the launcher's neutral `Sel` fill with the normal text
//! colours (D5); coral stays on the Format Bar's active buttons.

use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{define_class, msg_send, sel, AllocAnyThread, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAutoresizingMaskOptions, NSBackingStoreType, NSBezierPath, NSBitmapImageFileType, NSButton,
    NSColor, NSControlStateValueOff, NSControlStateValueOn, NSEvent, NSFont, NSFontAttributeName, NSFontWeightSemibold,
    NSForegroundColorAttributeName, NSImage, NSImageScaling, NSLineBreakMode, NSPanel, NSPopUpMenuWindowLevel, NSScreen, NSScrollView,
    NSTextAlignment, NSTextField, NSTrackingArea, NSTrackingAreaOptions, NSView, NSWindowStyleMask,
};
use objc2_foundation::{NSAttributedString, NSDictionary, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};

use crate::notes_format::Format;
use crate::notes_theme::{self as theme, Token};

/// Corner radius of the cards (the app's `ui::RADIUS`).
pub(crate) const CARD_RADIUS: f64 = 14.0;
/// Corner radius of a selected or hovered row and of a Format Bar button.
const ROW_RADIUS: f64 = 8.0;
/// Alpha of the active Format Bar button's accent tint.
const ACTIVE_TINT_ALPHA: f64 = 0.18;

/// A rectangle in AppKit screen coordinates (bottom-up): the caret the popup hangs from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// One row of the suggestion popup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    /// Short glyph: an emoji or a symbol.
    pub glyph: String,
    pub title: String,
    /// Right-aligned secondary text, e.g. the shortcut.
    pub detail: Option<String>,
}

/// Row height in points.
pub const ROW_HEIGHT: f64 = 28.0;
/// Vertical padding above the first and below the last row.
pub const POPUP_PAD: f64 = 4.0;
/// Popup width in points.
pub const POPUP_WIDTH: f64 = 300.0;
/// Rows visible at once; more scroll.
pub const MAX_VISIBLE_ROWS: usize = 8;

const ROW_INSET: f64 = 4.0;
const GAP: f64 = 4.0;
const MAX_EMOJI: usize = 20;

// ---------------------------------------------------------------- pure logic

/// Next selection index with wrap-around; 0 for an empty list.
fn wrap_index(current: usize, delta: i32, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    (current as i64 + delta as i64).rem_euclid(len as i64) as usize
}

/// Bottom-left origin for a popup of `size` hanging below `anchor`, flipped above it when it
/// would run past the bottom of `visible`, then clamped into `visible`.
fn place(anchor: Rect, size: (f64, f64), visible: Rect) -> (f64, f64) {
    let (w, h) = size;
    let max_x = (visible.x + visible.w - w).max(visible.x);
    let x = anchor.x.clamp(visible.x, max_x);
    let below = anchor.y - GAP - h;
    let above = anchor.y + anchor.h + GAP;
    let y = if below >= visible.y {
        below
    } else if above + h <= visible.y + visible.h {
        above
    } else {
        below
    };
    let max_y = (visible.y + visible.h - h).max(visible.y);
    (x, y.clamp(visible.y, max_y))
}

fn popup_height(rows: usize) -> f64 {
    rows.min(MAX_VISIBLE_ROWS) as f64 * ROW_HEIGHT + 2.0 * POPUP_PAD
}

/// Menu entries of the `/` menu in display order: format, glyph, aliases. Links are not
/// part of it (S8).
fn slash_table() -> [(Format, &'static str, &'static [&'static str]); 9] {
    [
        (Format::Heading(1), "H1", &["h1", "heading1", "title"]),
        (Format::Heading(2), "H2", &["h2", "heading2", "subtitle"]),
        (Format::Heading(3), "H3", &["h3", "heading3"]),
        (Format::BulletList, "\u{2022}", &["ul", "bullets", "unordered"]),
        (Format::OrderedList, "1.", &["ol", "numbered", "number"]),
        (Format::TaskList, "\u{2611}", &["todo", "checkbox", "check", "tasks"]),
        (Format::Quote, "\u{275D}", &["quote", "bq"]),
        (Format::CodeBlock, "{}", &["code", "snippet"]),
        (Format::Rule, "\u{2014}", &["hr", "line", "divider", "separator"]),
    ]
}

/// Match quality of `needle` against one candidate (lower is better), `None` for no match.
fn match_rank(needle: &str, candidate: &str) -> Option<u8> {
    let cand = candidate.to_lowercase();
    if cand.starts_with(needle) {
        return Some(0);
    }
    if cand.split_whitespace().any(|w| w.starts_with(needle)) {
        return Some(1);
    }
    let mut chars = cand.chars();
    needle.chars().all(|n| chars.any(|c| c == n)).then_some(2)
}

/// The `/` menu filtered by a case-insensitive prefix / word-prefix / subsequence match on
/// the name and its aliases, best matches first; the full list for an empty query.
pub fn slash_items(query: &str) -> Vec<(Format, Item)> {
    let needle = query.trim().to_lowercase();
    let mut hits: Vec<(u8, usize, Format, Item)> = Vec::new();
    for (idx, (format, glyph, aliases)) in slash_table().into_iter().enumerate() {
        // the menu shows the name in the interface language and matches it as well as the English one
        let shown_name = ferrocast_core::i18n::tr(format.name()).into_owned();
        let shown_name = shown_name.as_str();
        let rank = if needle.is_empty() {
            Some(0)
        } else {
            std::iter::once(format.name())
                .chain(std::iter::once(shown_name))
                .chain(aliases.iter().copied())
                .filter_map(|c| match_rank(&needle, c))
                .min()
        };
        if let Some(rank) = rank {
            let item = Item { glyph: glyph.to_string(), title: shown_name.to_string(), detail: format.shortcut().map(str::to_string) };
            hits.push((rank, idx, format, item));
        }
    }
    hits.sort_by_key(|(rank, idx, ..)| (*rank, *idx));
    hits.into_iter().map(|(_, _, f, i)| (f, i)).collect()
}

/// Emoji list for the `:` picker: `provider` maps a query to `(emoji, name)` pairs (the
/// caller owns the data). Nothing for queries shorter than 2 characters; at most 20 rows.
pub fn emoji_items(query: &str, provider: &dyn Fn(&str) -> Vec<(String, String)>) -> Vec<Item> {
    if query.chars().count() < 2 {
        return Vec::new();
    }
    provider(query).into_iter().take(MAX_EMOJI).map(|(glyph, title)| Item { glyph, title, detail: None }).collect()
}

/// Format Bar group of an action; a thin separator sits between different groups.
fn bar_group(f: Format) -> u8 {
    match f {
        Format::Bold | Format::Italic | Format::Underline | Format::Strikethrough | Format::Code => 0,
        Format::Link => 1,
        Format::Heading(_) => 2,
        Format::BulletList | Format::OrderedList | Format::TaskList => 3,
        Format::Quote | Format::CodeBlock => 4,
        Format::Rule => 5,
    }
}

/// SF Symbol and fallback title of a Format Bar button. Headings use text titles only.
fn bar_icon(f: Format) -> (Option<&'static str>, &'static str) {
    match f {
        Format::Bold => (Some("bold"), "B"),
        Format::Italic => (Some("italic"), "I"),
        Format::Underline => (Some("underline"), "U"),
        Format::Strikethrough => (Some("strikethrough"), "S"),
        Format::Code => (Some("chevron.left.forwardslash.chevron.right"), "<>"),
        Format::Link => (Some("link"), "Link"),
        Format::Heading(1) => (None, "H1"),
        Format::Heading(2) => (None, "H2"),
        Format::Heading(_) => (None, "H3"),
        Format::BulletList => (Some("list.bullet"), "\u{2022}"),
        Format::OrderedList => (Some("list.number"), "1."),
        Format::TaskList => (Some("checklist"), "\u{2611}"),
        Format::Quote => (Some("text.quote"), "\u{275D}"),
        Format::CodeBlock => (Some("curlybraces"), "{}"),
        Format::Rule => (Some("minus"), "\u{2014}"),
    }
}

/// Tooltip: name plus shortcut.
fn bar_tooltip(f: Format) -> String {
    match f.shortcut() {
        Some(s) => format!("{} {s}", ferrocast_core::i18n::tr(f.name())),
        None => ferrocast_core::i18n::tr(f.name()).into_owned(),
    }
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
}

// ---------------------------------------------------------------- shared look

/// `token` resolved for `appearance`, its alpha multiplied by `alpha` (a plain, non-dynamic colour).
pub(crate) fn token_alpha(token: Token, alpha: f64, appearance: &NSAppearance) -> Retained<NSColor> {
    let c = theme::resolve(token, theme::is_dark_appearance(appearance));
    NSColor::colorWithSRGBRed_green_blue_alpha(c.0 as f64 / 255.0, c.1 as f64 / 255.0, c.2 as f64 / 255.0, c.3 as f64 / 255.0 * alpha)
}

define_class!(
    /// Card: opaque `Bg` rounded rectangle (radius 14) with a 1 pt `Border` stroke inside its
    /// edge. Corners stay transparent, so a window or layer shadow follows the rounded shape.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastNotesCard"]
    pub(crate) struct CardView;

    impl CardView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let b = self.bounds();
            theme::color(Token::Bg).setFill();
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(b, CARD_RADIUS, CARD_RADIUS).fill();
            let inner = rect(b.origin.x + 0.5, b.origin.y + 0.5, b.size.width - 1.0, b.size.height - 1.0);
            let r = CARD_RADIUS - 0.5;
            let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(inner, r, r);
            path.setLineWidth(1.0);
            theme::color(Token::Border).setStroke();
            path.stroke();
        }
    }
);

define_class!(
    /// Plain view that fills its own bounds with a theme token (dynamic colour).
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastNotesTokenFill"]
    #[ivars = std::cell::Cell<Token>]
    pub(crate) struct TokenFill;

    impl TokenFill {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            // Own bounds, not the dirty rect: a cached snapshot passes a larger one.
            theme::color(self.ivars().get()).setFill();
            NSBezierPath::fillRect(self.bounds());
        }
    }
);

pub(crate) fn token_fill(mtm: MainThreadMarker, frame: NSRect, token: Token) -> Retained<TokenFill> {
    unsafe { msg_send![super(mtm.alloc::<TokenFill>().set_ivars(std::cell::Cell::new(token))), initWithFrame: frame] }
}

/// Track mouse enter, move and exit over the visible part of `view` (owner: `view`).
fn track_mouse(mtm: MainThreadMarker, view: &NSView, moved: bool) {
    let mut opts =
        NSTrackingAreaOptions::MouseEnteredAndExited | NSTrackingAreaOptions::ActiveAlways | NSTrackingAreaOptions::InVisibleRect;
    if moved {
        opts |= NSTrackingAreaOptions::MouseMoved;
    }
    let owner: &AnyObject = view;
    let area = unsafe { NSTrackingArea::initWithRect_options_owner_userInfo(mtm.alloc(), NSRect::ZERO, opts, Some(owner), None) };
    view.addTrackingArea(&area);
}

// ---------------------------------------------------------------- suggestion popup

type PickCb = Rc<dyn Fn(usize)>;

#[derive(Default)]
struct Shared {
    items: RefCell<Vec<Item>>,
    selected: Cell<usize>,
    /// Row under the mouse, if any.
    hovered: Cell<Option<usize>>,
    on_pick: RefCell<Option<PickCb>>,
}

struct RowsIvars {
    shared: Rc<Shared>,
}

define_class!(
    /// Flipped document view of the popup: draws the selection and turns clicks into picks.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastPopupRows"]
    #[ivars = RowsIvars]
    struct RowsView;

    impl RowsView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(mouseDownCanMoveWindow))]
        fn mouse_down_can_move_window(&self) -> bool {
            false
        }

        // The labels must not swallow the click.
        #[unsafe(method(hitTest:))]
        fn hit_test(&self, point: NSPoint) -> Option<&NSView> {
            let f = self.frame();
            let inside = point.x >= f.origin.x
                && point.x <= f.origin.x + f.size.width
                && point.y >= f.origin.y
                && point.y <= f.origin.y + f.size.height;
            let me: &NSView = self;
            if inside { Some(me) } else { None }
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            let shared = &self.ivars().shared;
            if shared.items.borrow().is_empty() {
                return;
            }
            let w = self.bounds().size.width;
            let row = |i: usize| rect(ROW_INSET, i as f64 * ROW_HEIGHT, w - 2.0 * ROW_INSET, ROW_HEIGHT);
            let selected = shared.selected.get();
            if let Some(h) = shared.hovered.get().filter(|h| *h != selected && *h < shared.items.borrow().len()) {
                theme::color(Token::Field).setFill();
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(row(h), ROW_RADIUS, ROW_RADIUS).fill();
            }
            theme::color(Token::Sel).setFill();
            NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(row(selected), ROW_RADIUS, ROW_RADIUS).fill();
        }

        #[unsafe(method(mouseEntered:))]
        fn mouse_entered(&self, event: &NSEvent) {
            update_hover(self, Some(event));
        }

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            update_hover(self, Some(event));
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _event: &NSEvent) {
            update_hover(self, None);
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let p = self.convertPoint_fromView(event.locationInWindow(), None);
            if p.y < 0.0 {
                return;
            }
            let idx = (p.y / ROW_HEIGHT) as usize;
            let shared = self.ivars().shared.clone();
            if idx >= shared.items.borrow().len() {
                return;
            }
            shared.selected.set(idx);
            self.setNeedsDisplay(true);
            let cb = shared.on_pick.borrow().clone();
            if let Some(cb) = cb {
                cb(idx);
            }
        }
    }

    unsafe impl NSObjectProtocol for RowsView {}
);

/// Hover row under the mouse event (`None`: the mouse left the list).
fn update_hover(view: &RowsView, event: Option<&NSEvent>) {
    let shared = &view.ivars().shared;
    let row = event.and_then(|e| {
        let p = view.convertPoint_fromView(e.locationInWindow(), None);
        (p.y >= 0.0).then(|| (p.y / ROW_HEIGHT) as usize).filter(|i| *i < shared.items.borrow().len())
    });
    if shared.hovered.get() != row {
        shared.hovered.set(row);
        view.setNeedsDisplay(true);
    }
}

define_class!(
    /// Borderless panel that never takes key or main status: the editor keeps the focus.
    #[unsafe(super(NSPanel))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastSuggestionPanel"]
    struct SuggestionPanel;

    impl SuggestionPanel {
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

struct RowLabels {
    glyph: Retained<NSTextField>,
    title: Retained<NSTextField>,
    detail: Option<Retained<NSTextField>>,
}

/// Small borderless popup listing rows under the caret. Main thread only.
pub struct SuggestionPopup {
    mtm: MainThreadMarker,
    panel: Retained<SuggestionPanel>,
    card: Retained<CardView>,
    scroll: Retained<NSScrollView>,
    rows: Retained<RowsView>,
    shared: Rc<Shared>,
    labels: RefCell<Vec<RowLabels>>,
    anchor: Cell<Rect>,
}

impl SuggestionPopup {
    /// Build the hidden popup. Panics when not on the main thread.
    pub fn new() -> Self {
        let mtm = MainThreadMarker::new().expect("SuggestionPopup must be created on the main thread");
        let frame = rect(0.0, 0.0, POPUP_WIDTH, popup_height(1));
        let style = NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel;
        let panel: Retained<SuggestionPanel> = unsafe {
            msg_send![
                super(mtm.alloc::<SuggestionPanel>().set_ivars(())),
                initWithContentRect: frame,
                styleMask: style,
                backing: NSBackingStoreType::Buffered,
                defer: false
            ]
        };
        unsafe { panel.setReleasedWhenClosed(false) };
        panel.setLevel(NSPopUpMenuWindowLevel);
        panel.setFloatingPanel(true);
        panel.setHidesOnDeactivate(false);
        panel.setBecomesKeyOnlyIfNeeded(true);
        panel.setOpaque(false);
        panel.setBackgroundColor(Some(&NSColor::clearColor()));
        panel.setHasShadow(true);

        // Opaque `Bg` card, no vibrancy; the clear panel's system shadow follows its rounded shape.
        let card: Retained<CardView> = unsafe { msg_send![mtm.alloc::<CardView>(), initWithFrame: frame] };
        panel.setContentView(Some(&card));
        theme::apply_to(&panel);

        let shared = Rc::new(Shared::default());
        let scroll = NSScrollView::initWithFrame(mtm.alloc::<NSScrollView>(), frame);
        scroll.setHasVerticalScroller(true);
        scroll.setAutohidesScrollers(true);
        scroll.setDrawsBackground(false);
        scroll.setBorderType(objc2_app_kit::NSBorderType::NoBorder);
        let rows: Retained<RowsView> =
            unsafe { msg_send![super(mtm.alloc::<RowsView>().set_ivars(RowsIvars { shared: shared.clone() })), initWithFrame: frame] };
        scroll.setDocumentView(Some(&rows));
        track_mouse(mtm, &rows, true);
        card.addSubview(&scroll);

        Self {
            mtm,
            panel,
            card,
            scroll,
            rows,
            shared,
            labels: RefCell::new(Vec::new()),
            anchor: Cell::new(Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 }),
        }
    }

    /// Show `items` just below `anchor` (the caret, screen coordinates); flips above it near
    /// the bottom of the screen and clamps to the visible frame. An empty list hides.
    pub fn show(&self, anchor: Rect, items: Vec<Item>) {
        if items.is_empty() {
            self.hide();
            return;
        }
        self.anchor.set(anchor);
        self.apply_theme();
        self.set_items(items);
        self.panel.orderFrontRegardless();
    }

    /// Give the panel the stored Notes appearance (`notes_theme::set_appearance`); `show` does
    /// it too, so a popup opened later is always right.
    pub fn apply_theme(&self) {
        theme::apply_to(&self.panel);
        self.card.setNeedsDisplay(true);
        self.rows.setNeedsDisplay(true);
    }

    /// Replace the rows (selection goes back to the first); an empty list hides the popup.
    pub fn update(&self, items: Vec<Item>) {
        if items.is_empty() {
            self.shared.items.borrow_mut().clear();
            self.hide();
            return;
        }
        self.set_items(items);
    }

    /// Move the selection by `delta` rows, wrapping around.
    pub fn move_selection(&self, delta: i32) {
        let len = self.shared.items.borrow().len();
        if len == 0 {
            return;
        }
        self.shared.selected.set(wrap_index(self.shared.selected.get(), delta, len));
        self.restyle();
        self.rows.setNeedsDisplay(true);
        let sel = self.shared.selected.get() as f64;
        self.rows.scrollRectToVisible(rect(0.0, sel * ROW_HEIGHT, POPUP_WIDTH, ROW_HEIGHT));
    }

    /// Index of the highlighted row, `None` when there are no rows.
    pub fn selected(&self) -> Option<usize> {
        (!self.shared.items.borrow().is_empty()).then(|| self.shared.selected.get())
    }

    pub fn hide(&self) {
        self.panel.orderOut(None);
    }

    pub fn is_visible(&self) -> bool {
        self.panel.isVisible()
    }

    /// Called with the row index when a row is clicked.
    pub fn set_on_pick(&self, cb: Box<dyn Fn(usize)>) {
        *self.shared.on_pick.borrow_mut() = Some(Rc::from(cb));
    }

    /// Current popup frame in screen coordinates.
    pub fn frame(&self) -> Rect {
        let f = self.panel.frame();
        Rect { x: f.origin.x, y: f.origin.y, w: f.size.width, h: f.size.height }
    }

    /// Window number, for routing synthetic events to the popup.
    pub fn window_number(&self) -> isize {
        self.panel.windowNumber()
    }

    /// Whether the popup window is key (it never should be).
    pub fn is_key(&self) -> bool {
        self.panel.isKeyWindow()
    }

    /// Render the popup to a PNG file; false on any failure.
    pub fn snapshot_png(&self, path: &Path) -> bool {
        let Some(view) = self.panel.contentView() else { return false };
        let bounds = view.bounds();
        let Some(rep) = view.bitmapImageRepForCachingDisplayInRect(bounds) else { return false };
        view.cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
        let Some(data) = (unsafe { rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new()) }) else {
            return false;
        };
        data.writeToFile_atomically(&NSString::from_str(&path.to_string_lossy()), true)
    }

    fn set_items(&self, items: Vec<Item>) {
        *self.shared.items.borrow_mut() = items;
        self.shared.selected.set(0);
        self.rebuild();
        self.layout();
    }

    fn rebuild(&self) {
        for v in self.rows.subviews().iter() {
            v.removeFromSuperview();
        }
        let mut labels = Vec::new();
        for (i, item) in self.shared.items.borrow().iter().enumerate() {
            let y = i as f64 * ROW_HEIGHT + (ROW_HEIGHT - 17.0) / 2.0;
            let make = |text: &str, x: f64, w: f64, size: f64, align: NSTextAlignment| {
                let l = NSTextField::labelWithString(&NSString::from_str(text), self.mtm);
                l.setFont(Some(&NSFont::systemFontOfSize(size)));
                l.setAlignment(align);
                l.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
                l.setFrame(rect(x, y, w, 17.0));
                self.rows.addSubview(&l);
                l
            };
            let glyph = make(&item.glyph, 8.0, 32.0, 14.0, NSTextAlignment::Center);
            let detail_w = if item.detail.is_some() { 84.0 } else { 0.0 };
            let title = make(&item.title, 46.0, POPUP_WIDTH - 46.0 - 14.0 - detail_w, 13.0, NSTextAlignment::Left);
            let detail = item.detail.as_deref().map(|d| make(d, POPUP_WIDTH - 12.0 - 80.0, 80.0, 12.0, NSTextAlignment::Right));
            labels.push(RowLabels { glyph, title, detail });
        }
        *self.labels.borrow_mut() = labels;
        self.restyle();
        self.rows.setNeedsDisplay(true);
    }

    /// Theme token colors; the selected row keeps the normal text colors (its `Sel` fill is
    /// drawn by the rows view), like the launcher.
    fn restyle(&self) {
        for l in self.labels.borrow().iter() {
            let (main, soft) = (theme::color(Token::Text), theme::color(Token::Sub));
            l.glyph.setTextColor(Some(&main));
            l.title.setTextColor(Some(&main));
            if let Some(d) = &l.detail {
                d.setTextColor(Some(&soft));
            }
        }
    }

    /// Size the panel for the rows and place it under (or above) the anchor.
    fn layout(&self) {
        let n = self.shared.items.borrow().len();
        let h = popup_height(n);
        let a = self.anchor.get();
        let visible = self.visible_frame(a);
        let (x, y) = place(a, (POPUP_WIDTH, h), visible);
        self.panel.setFrame_display(rect(x, y, POPUP_WIDTH, h), true);
        self.card.setFrame(rect(0.0, 0.0, POPUP_WIDTH, h));
        self.panel.invalidateShadow();
        self.scroll.setFrame(rect(0.0, POPUP_PAD, POPUP_WIDTH, h - 2.0 * POPUP_PAD));
        self.rows.setFrame(rect(0.0, 0.0, POPUP_WIDTH, n as f64 * ROW_HEIGHT));
        self.rows.scrollPoint(NSPoint::new(0.0, 0.0));
    }

    /// Visible frame of the screen holding the anchor, else the main screen.
    fn visible_frame(&self, anchor: Rect) -> Rect {
        let (cx, cy) = (anchor.x + anchor.w / 2.0, anchor.y + anchor.h / 2.0);
        let screens = NSScreen::screens(self.mtm);
        let screen = screens
            .iter()
            .find(|s| {
                let f = s.frame();
                cx >= f.origin.x && cx < f.origin.x + f.size.width && cy >= f.origin.y && cy < f.origin.y + f.size.height
            })
            .or_else(|| NSScreen::mainScreen(self.mtm));
        match screen {
            Some(s) => {
                let v = s.visibleFrame();
                Rect { x: v.origin.x, y: v.origin.y, w: v.size.width, h: v.size.height }
            }
            None => Rect { x: 0.0, y: 0.0, w: 1440.0, h: 900.0 },
        }
    }
}

impl Default for SuggestionPopup {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for SuggestionPopup {
    fn drop(&mut self) {
        self.panel.orderOut(None);
    }
}

// ---------------------------------------------------------------- format bar

type FormatCb = Rc<dyn Fn(Format)>;

struct TargetIvars {
    cb: RefCell<Option<FormatCb>>,
    /// Formats drawn as pressed; a click must not flip a button, only `set_active` does.
    active: RefCell<Vec<Format>>,
}

define_class!(
    /// Button target: maps the sender's tag to a `Format` and calls the callback.
    #[unsafe(super(objc2_foundation::NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastFormatBarTarget"]
    #[ivars = TargetIvars]
    struct FormatTarget;

    impl FormatTarget {
        #[unsafe(method(formatClicked:))]
        fn format_clicked(&self, sender: &AnyObject) {
            let tag: isize = unsafe { msg_send![sender, tag] };
            let Some(format) = usize::try_from(tag).ok().and_then(|i| Format::ALL.get(i).copied()) else { return };
            let cb = self.ivars().cb.borrow().clone();
            if let Some(cb) = cb {
                cb(format);
            }
            let on = self.ivars().active.borrow().contains(&format);
            let state: isize = if on { 1 } else { 0 };
            unsafe {
                let _: () = msg_send![sender, setState: state];
            }
        }
    }

    unsafe impl NSObjectProtocol for FormatTarget {}
);

define_class!(
    /// Borderless icon button: `Sub` icon, a rounded `Field` background and `Text` on hover, a
    /// rounded 18% `Accent` tint with the icon in `AccentText` while its state is On.
    #[unsafe(super(NSButton))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastFormatButton"]
    #[ivars = Cell<bool>]
    struct FormatButton;

    impl FormatButton {
        // The window is movable by its background: a click on a bar button must press it.
        #[unsafe(method(mouseDownCanMoveWindow))]
        fn mouse_down_can_move_window(&self) -> bool {
            false
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            unsafe {
                let _: () = msg_send![super(self), mouseDown: event];
            }
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, dirty: NSRect) {
            let fill = if self.state() == NSControlStateValueOn {
                Some(token_alpha(Token::Accent, ACTIVE_TINT_ALPHA, &self.effectiveAppearance()))
            } else if self.ivars().get() {
                Some(theme::color(Token::Field))
            } else {
                None
            };
            if let Some(fill) = fill {
                fill.setFill();
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(self.bounds(), ROW_RADIUS, ROW_RADIUS).fill();
            }
            unsafe {
                let _: () = msg_send![super(self), drawRect: dirty];
            }
        }

        #[unsafe(method(mouseEntered:))]
        fn mouse_entered(&self, _event: &NSEvent) {
            self.ivars().set(true);
            restyle_button(self);
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _event: &NSEvent) {
            self.ivars().set(false);
            restyle_button(self);
        }
    }
);

/// Colors of a Format Bar button for its state: `AccentText` when pressed, `Text` on hover,
/// else `Sub`. Icons take the tint; text titles (headings) need an attributed title.
fn restyle_button(b: &FormatButton) {
    let token = if b.state() == NSControlStateValueOn {
        Token::AccentText
    } else if b.ivars().get() {
        Token::Text
    } else {
        Token::Sub
    };
    let color = theme::color(token);
    b.setContentTintColor(Some(&color));
    if b.image().is_none() {
        let font = NSFont::systemFontOfSize_weight(12.0, unsafe { NSFontWeightSemibold });
        let (k1, k2) = unsafe { (NSFontAttributeName, NSForegroundColorAttributeName) };
        let attrs: Retained<NSDictionary<NSString, AnyObject>> =
            NSDictionary::from_slices(&[k1, k2], &[&*font as &AnyObject, &*color as &AnyObject]);
        let title = unsafe { NSAttributedString::initWithString_attributes(NSAttributedString::alloc(), &b.title(), Some(&attrs)) };
        b.setAttributedTitle(&title);
    }
    let v: &NSView = b;
    v.setNeedsDisplay(true);
}

const BUTTON: f64 = 28.0;
const BUTTON_GAP: f64 = 2.0;
const GROUP_GAP: f64 = 10.0;
const BAR_LEFT: f64 = 12.0;

/// The Format Bar strip for the bottom of the Notes window. Main thread only.
pub struct FormatBar {
    root: Retained<TokenFill>,
    width: f64,
    target: Retained<FormatTarget>,
    /// One button per `Format::ALL` entry, same order.
    buttons: Vec<Retained<FormatButton>>,
}

impl FormatBar {
    /// Bar height in points.
    pub const HEIGHT: f64 = 36.0;

    /// Build the strip. Panics when not on the main thread.
    pub fn new() -> FormatBar {
        let mtm = MainThreadMarker::new().expect("FormatBar must be created on the main thread");
        let width = 560.0;
        let root = token_fill(mtm, rect(0.0, 0.0, width, Self::HEIGHT), Token::Bg);
        root.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewMaxYMargin);

        // 1 pt `Sep` hairline on top.
        let top = token_fill(mtm, rect(0.0, Self::HEIGHT - 1.0, width, 1.0), Token::Sep);
        top.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewMinYMargin);
        root.addSubview(&top);

        let target: Retained<FormatTarget> = unsafe {
            msg_send![
                super(mtm.alloc::<FormatTarget>().set_ivars(TargetIvars { cb: RefCell::new(None), active: RefCell::new(Vec::new()) })),
                init
            ]
        };
        let mut buttons = Vec::new();
        let mut x = BAR_LEFT;
        let mut prev_group = bar_group(Format::ALL[0]);
        for (i, f) in Format::ALL.iter().copied().enumerate() {
            let group = bar_group(f);
            if group != prev_group {
                let sep =
                    token_fill(mtm, rect(x + (GROUP_GAP - BUTTON_GAP - 1.0) / 2.0, (Self::HEIGHT - 16.0) / 2.0, 1.0, 16.0), Token::Sep);
                root.addSubview(&sep);
                x += GROUP_GAP - BUTTON_GAP;
                prev_group = group;
            }
            let b = Self::make_button(mtm, f, i, &target, x);
            root.addSubview(&b);
            buttons.push(b);
            x += BUTTON + BUTTON_GAP;
        }
        // Right edge: the last button plus the left margin, so nothing is clipped.
        let width = x - BUTTON_GAP + BAR_LEFT;
        root.setFrame(rect(0.0, 0.0, width, Self::HEIGHT));
        FormatBar { root, width, target, buttons }
    }

    /// Width the buttons need (about 500 pt); the window may be narrower, so the host puts
    /// the strip in a horizontal scroll view.
    pub fn content_width(&self) -> f64 {
        self.width
    }

    fn make_button(mtm: MainThreadMarker, f: Format, tag: usize, target: &FormatTarget, x: f64) -> Retained<FormatButton> {
        let b: Retained<FormatButton> = unsafe {
            msg_send![
                super(mtm.alloc::<FormatButton>().set_ivars(Cell::new(false))),
                initWithFrame: rect(x, (Self::HEIGHT - 1.0 - BUTTON) / 2.0, BUTTON, BUTTON)
            ]
        };
        let tip = bar_tooltip(f);
        let (symbol, fallback) = bar_icon(f);
        // A missing symbol on this macOS version falls back to a text title.
        let image = symbol.and_then(|s| {
            NSImage::imageWithSystemSymbolName_accessibilityDescription(&NSString::from_str(s), Some(&NSString::from_str(&tip)))
        });
        match &image {
            Some(img) => {
                b.setImage(Some(img));
                b.setImagePosition(objc2_app_kit::NSCellImagePosition::ImageOnly);
                b.setImageScaling(NSImageScaling::ScaleProportionallyDown);
            }
            None => {
                b.setTitle(&NSString::from_str(fallback));
                b.setFont(Some(&NSFont::systemFontOfSize_weight(12.0, unsafe { NSFontWeightSemibold })));
                b.setImagePosition(objc2_app_kit::NSCellImagePosition::NoImage);
            }
        }
        b.setBordered(false);
        // Clicking a button must never take the focus away from the editor.
        b.setRefusesFirstResponder(true);
        b.setTag(tag as isize);
        b.setToolTip(Some(&NSString::from_str(&tip)));
        restyle_button(&b);
        track_mouse(mtm, &b, false);
        let t: &AnyObject = target;
        unsafe {
            b.setTarget(Some(t));
            b.setAction(Some(sel!(formatClicked:)));
        }
        b
    }

    /// The strip, to add at the bottom of the window content (frame width follows the parent).
    pub fn view(&self) -> Retained<NSView> {
        Retained::into_super(self.root.clone())
    }

    /// Called with the action of the clicked button.
    pub fn set_on_format(&self, cb: Box<dyn Fn(Format)>) {
        *self.target.ivars().cb.borrow_mut() = Some(Rc::from(cb));
    }

    /// Draw the buttons of `active` as pressed; every other button as normal.
    pub fn set_active(&self, active: &[Format]) {
        *self.target.ivars().active.borrow_mut() = active.to_vec();
        for (f, b) in Format::ALL.iter().zip(&self.buttons) {
            let on = active.contains(f);
            b.setState(if on { NSControlStateValueOn } else { NSControlStateValueOff });
            restyle_button(b);
        }
    }
}

impl Default for FormatBar {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn titles(q: &str) -> Vec<String> {
        slash_items(q).into_iter().map(|(_, i)| i.title).collect()
    }

    #[test]
    fn empty_query_lists_the_whole_menu_without_links() {
        let all = slash_items("");
        assert_eq!(all.len(), 9);
        assert!(all.iter().all(|(f, _)| *f != Format::Link));
        assert_eq!(all[0].0, Format::Heading(1));
        assert_eq!(all[8].0, Format::Rule);
        assert_eq!(all[0].1.detail.as_deref(), Format::Heading(1).shortcut());
        assert_eq!(all[8].1.detail, None);
    }

    #[test]
    fn aliases_and_case_are_matched() {
        assert_eq!(slash_items("H2")[0].0, Format::Heading(2));
        assert_eq!(slash_items("todo")[0].0, Format::TaskList);
        assert_eq!(slash_items("checkbox")[0].0, Format::TaskList);
        assert_eq!(slash_items("UL")[0].0, Format::BulletList);
        assert_eq!(slash_items("ol")[0].0, Format::OrderedList);
        assert_eq!(slash_items("quote")[0].0, Format::Quote);
        assert_eq!(slash_items("code")[0].0, Format::CodeBlock);
        assert_eq!(slash_items("hr")[0].0, Format::Rule);
        assert_eq!(slash_items("line")[0].0, Format::Rule);
    }

    #[test]
    fn prefix_beats_subsequence_and_unknown_is_empty() {
        assert_eq!(titles("head"), ["Heading 1", "Heading 2", "Heading 3"]);
        // "bl": name prefix (Blockquote), then word prefix (Code Block), then subsequences
        assert_eq!(titles("bl")[..2], ["Blockquote", "Code Block"]);
        assert!(titles("bl").contains(&"Bullet List".to_string()));
        // subsequence: b-q-t hits Blockquote only
        assert_eq!(titles("bqt"), ["Blockquote"]);
        assert!(slash_items("zzz").is_empty());
        assert!(slash_items("link").is_empty());
    }

    #[test]
    fn emoji_needs_two_characters_and_caps_at_twenty() {
        let provider = |q: &str| (0..30).map(|i| (format!("e{i}"), format!("{q}{i}"))).collect::<Vec<_>>();
        assert!(emoji_items("", &provider).is_empty());
        assert!(emoji_items("s", &provider).is_empty());
        let items = emoji_items("sm", &provider);
        assert_eq!(items.len(), 20);
        assert_eq!(items[0], Item { glyph: "e0".into(), title: "sm0".into(), detail: None });
        let none = |_: &str| Vec::new();
        assert!(emoji_items("smile", &none).is_empty());
    }

    #[test]
    fn selection_wraps_both_ways() {
        assert_eq!(wrap_index(0, 1, 3), 1);
        assert_eq!(wrap_index(2, 1, 3), 0);
        assert_eq!(wrap_index(0, -1, 3), 2);
        assert_eq!(wrap_index(1, -4, 3), 0);
        assert_eq!(wrap_index(0, 5, 0), 0);
    }

    #[test]
    fn placement_below_flip_above_and_clamp() {
        let vis = Rect { x: 0.0, y: 0.0, w: 1000.0, h: 800.0 };
        let size = (300.0, 200.0);
        let caret = |x, y| Rect { x, y, w: 2.0, h: 16.0 };
        // room below: just under the caret
        assert_eq!(place(caret(100.0, 500.0), size, vis), (100.0, 500.0 - GAP - 200.0));
        // near the bottom: flips above the caret
        assert_eq!(place(caret(100.0, 100.0), size, vis), (100.0, 100.0 + 16.0 + GAP));
        // right edge: clamped in x
        assert_eq!(place(caret(950.0, 500.0), size, vis).0, 700.0);
        // left of the screen: clamped to the left edge
        assert_eq!(place(caret(-50.0, 500.0), size, vis).0, 0.0);
        // no room anywhere: stays inside the visible frame
        let tiny = Rect { x: 0.0, y: 0.0, w: 400.0, h: 150.0 };
        let (_, y) = place(caret(10.0, 70.0), size, tiny);
        assert_eq!(y, 0.0);
    }

    #[test]
    fn popup_height_caps_at_eight_rows() {
        assert_eq!(popup_height(3), 3.0 * 28.0 + 8.0);
        assert_eq!(popup_height(8), popup_height(30));
    }

    #[test]
    fn bar_groups_follow_format_all_order() {
        let groups: Vec<u8> = Format::ALL.iter().map(|f| bar_group(*f)).collect();
        assert!(groups.windows(2).all(|w| w[0] <= w[1]), "{groups:?}");
        assert_eq!(*groups.last().unwrap(), 5);
        assert_eq!(bar_tooltip(Format::Bold), "Bold \u{2318}B");
        assert_eq!(bar_tooltip(Format::Rule), "Horizontal Rule");
        assert!(Format::ALL.iter().all(|f| !bar_icon(*f).1.is_empty()));
    }
}
