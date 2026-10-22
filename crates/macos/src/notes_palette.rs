//! Searchable list overlay used by Browse Notes (⌘P), the ⌘K action panel, its submenus and
//! Recently Deleted (S10-S13, S16). An NSView added over the Notes window's content view (so
//! it moves and resizes with the window): dimmed backdrop, a rounded card with a search field
//! on top and a list below. The controller owns the data and the decisions: the palette only
//! reports query edits, picks and closes through callbacks. All AppKit calls happen on the
//! main thread; the selection / layout helpers are pure and unit-tested below.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{define_class, msg_send, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSBezierPath, NSBox, NSBoxType, NSColor, NSControlTextEditingDelegate, NSEvent, NSEventModifierFlags,
    NSEventType, NSFocusRingType, NSFont, NSFontWeightSemibold, NSImage, NSImageScaling, NSImageView, NSLineBreakMode, NSScrollView,
    NSTextAlignment, NSTextField, NSTextFieldDelegate, NSView, NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState,
    NSVisualEffectView,
};
use objc2_foundation::{NSNotification, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};

/// One row of the palette.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct PaletteItem {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    /// A short string, or an SF Symbol name when it starts with `sf:`.
    pub glyph: Option<String>,
    /// Right-aligned shortcut or hint, such as `⌘1`.
    pub accessory: Option<String>,
    /// Consecutive rows with the same section sit under one small grey header.
    pub section: Option<String>,
    /// Title and glyph in the system red.
    pub danger: bool,
    /// Draws a chevron (opens a submenu).
    pub has_children: bool,
}

const CARD_W: f64 = 420.0;
const WINDOW_MARGIN: f64 = 16.0;
const TOP_MARGIN: f64 = 52.0;
const SEARCH_H: f64 = 44.0;
const MAX_LIST_H: f64 = 296.0;
const MIN_LIST_H: f64 = 64.0;
const ROW_H: f64 = 36.0;
const ROW_H_COMPACT: f64 = 28.0;
const HEADER_ROW_H: f64 = 26.0;
const LIST_PAD: f64 = 4.0;
const ROW_INSET: f64 = 6.0;
const EMPTY_H: f64 = 64.0;
const ACCESSORY_W: f64 = 80.0;
const CHEVRON_W: f64 = 18.0;
const KEY_ESCAPE: u16 = 53;
const KEY_RETURN: u16 = 36;
const KEY_ENTER: u16 = 76;
const KEY_DOWN: u16 = 125;
const KEY_UP: u16 = 126;

// ---------------------------------------------------------------- pure logic

/// What one display row shows.
#[derive(Clone, Debug, PartialEq, Eq)]
enum RowKind {
    /// Section header (never selectable).
    Header(String),
    /// Index into the item list.
    Item(usize),
}

/// Display rows: a header is inserted before each run of items that share a `Some` section.
fn build_rows(items: &[PaletteItem]) -> Vec<RowKind> {
    let mut rows = Vec::new();
    let mut last: Option<&str> = None;
    for (i, item) in items.iter().enumerate() {
        let sec = item.section.as_deref();
        if let Some(s) = sec {
            if last != Some(s) {
                rows.push(RowKind::Header(s.to_string()));
            }
        }
        last = sec;
        rows.push(RowKind::Item(i));
    }
    rows
}

fn item_height(item: &PaletteItem) -> f64 {
    if item.subtitle.is_some() {
        ROW_H
    } else {
        ROW_H_COMPACT
    }
}

/// `(y, height)` of every row from the top of the list (flipped coordinates), after the pad.
fn row_geometry(items: &[PaletteItem], rows: &[RowKind]) -> Vec<(f64, f64)> {
    let mut y = LIST_PAD;
    rows.iter()
        .map(|r| {
            let h = match r {
                RowKind::Header(_) => HEADER_ROW_H,
                RowKind::Item(i) => item_height(&items[*i]),
            };
            let g = (y, h);
            y += h;
            g
        })
        .collect()
}

/// Total height of the scrollable content.
fn content_height(geoms: &[(f64, f64)]) -> f64 {
    match geoms.last() {
        Some((y, h)) => y + h + LIST_PAD,
        None => EMPTY_H,
    }
}

/// Next selectable row (never a header) moving by `delta`, wrapping around. With no current
/// row, a positive delta lands on the first and a negative one on the last selectable row.
fn step_selection(rows: &[RowKind], current: Option<usize>, delta: i32) -> Option<usize> {
    let sel: Vec<usize> = rows.iter().enumerate().filter(|(_, r)| matches!(r, RowKind::Item(_))).map(|(i, _)| i).collect();
    if sel.is_empty() {
        return None;
    }
    let len = sel.len() as i64;
    let pos = match current.and_then(|c| sel.iter().position(|&i| i == c)) {
        Some(p) => (p as i64 + delta as i64).rem_euclid(len),
        None if delta < 0 => len - 1,
        None => 0,
    };
    Some(sel[pos as usize])
}

/// First selectable row.
fn first_selectable(rows: &[RowKind]) -> Option<usize> {
    rows.iter().position(|r| matches!(r, RowKind::Item(_)))
}

/// Row that holds the item with `id`.
fn row_of_id(items: &[PaletteItem], rows: &[RowKind], id: &str) -> Option<usize> {
    rows.iter().position(|r| matches!(r, RowKind::Item(i) if items[*i].id == id))
}

/// Selected row after the items changed: the same id when it still exists, else the first.
fn keep_selection(items: &[PaletteItem], rows: &[RowKind], previous_id: Option<&str>) -> Option<usize> {
    previous_id.and_then(|id| row_of_id(items, rows, id)).or_else(|| first_selectable(rows))
}

/// Row under `y` (list coordinates).
fn row_at(geoms: &[(f64, f64)], y: f64) -> Option<usize> {
    geoms.iter().position(|(ry, rh)| y >= *ry && y < ry + rh)
}

/// New scroll offset that brings `[row_y, row_y + row_h]` into the viewport.
fn ensure_visible(scroll_y: f64, view_h: f64, row_y: f64, row_h: f64) -> f64 {
    if row_y < scroll_y {
        row_y
    } else if row_y + row_h > scroll_y + view_h {
        row_y + row_h - view_h
    } else {
        scroll_y
    }
}

/// Card width: 420, never wider than the window minus a margin on each side.
fn card_width(window_w: f64) -> f64 {
    CARD_W.min(window_w - 2.0 * WINDOW_MARGIN).max(160.0)
}

/// Visible list height for `content_h` of content in a window of height `window_h`.
fn list_height(content_h: f64, window_h: f64) -> f64 {
    let room = (window_h - TOP_MARGIN - WINDOW_MARGIN - SEARCH_H).max(MIN_LIST_H);
    content_h.min(MAX_LIST_H).min(room)
}

// ---------------------------------------------------------------- state

type QueryCb = Rc<dyn Fn(&str)>;
type PickCb = Rc<dyn Fn(String)>;
type CloseCb = Rc<dyn Fn()>;

/// A level kept on the stack while a submenu is open.
struct Level {
    placeholder: String,
    items: Vec<PaletteItem>,
    selected_id: Option<String>,
    query: String,
}

#[derive(Default)]
struct State {
    visible: bool,
    placeholder: String,
    empty_text: String,
    items: Vec<PaletteItem>,
    rows: Vec<RowKind>,
    geoms: Vec<(f64, f64)>,
    selected: Option<usize>,
    stack: Vec<Level>,
    on_query: Option<QueryCb>,
    on_pick: Option<PickCb>,
    on_close: Option<CloseCb>,
}

enum GlyphView {
    Text(Retained<NSTextField>),
    Image(Retained<NSImageView>),
}

impl GlyphView {
    fn set_color(&self, c: &NSColor) {
        match self {
            GlyphView::Text(t) => t.setTextColor(Some(c)),
            GlyphView::Image(i) => i.setContentTintColor(Some(c)),
        }
    }
}

/// The views of one display row, for restyling on selection changes.
struct RowViews {
    glyph: Option<GlyphView>,
    title: Option<Retained<NSTextField>>,
    subtitle: Option<Retained<NSTextField>>,
    accessory: Option<Retained<NSTextField>>,
    chevron: Option<Retained<NSImageView>>,
    danger: bool,
}

struct Ui {
    root: Retained<RootView>,
    card: Retained<NSVisualEffectView>,
    backing: Retained<BackingView>,
    icon: Retained<NSImageView>,
    field: Retained<NSTextField>,
    sep: Retained<NSBox>,
    scroll: Retained<NSScrollView>,
    rows_view: Retained<RowsView>,
    empty: Retained<NSTextField>,
    labels: RefCell<Vec<RowViews>>,
    /// Kept alive: the field delegate is a weak reference.
    _target: Retained<PaletteTarget>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State { empty_text: "No results".to_string(), ..State::default() });
    static UI: RefCell<Option<Rc<Ui>>> = const { RefCell::new(None) };
}

fn st<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

fn ui() -> Option<Rc<Ui>> {
    UI.with(|u| u.borrow().clone())
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
}

// ---------------------------------------------------------------- AppKit classes

define_class!(
    /// Flipped document view of the list: draws the selection and turns clicks into picks.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastPaletteRows"]
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
            let rect_sel = STATE.with(|s| {
                let s = s.try_borrow().ok()?;
                s.selected.and_then(|r| s.geoms.get(r).copied())
            });
            if let Some((y, h)) = rect_sel {
                let w = self.bounds().size.width;
                NSColor::controlAccentColor().setFill();
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect(ROW_INSET, y, w - 2.0 * ROW_INSET, h), 6.0, 6.0).fill();
            }
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let p = self.convertPoint_fromView(event.locationInWindow(), None);
            let row = st(|s| row_at(&s.geoms, p.y).filter(|r| matches!(s.rows[*r], RowKind::Item(_))));
            let Some(row) = row else { return };
            st(|s| s.selected = Some(row));
            restyle();
            self.setNeedsDisplay(true);
            pick_selected();
        }
    }

    unsafe impl NSObjectProtocol for RowsView {}
);

define_class!(
    /// Full-size overlay: dims the window and closes the palette on a click outside the card.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastPaletteRoot"]
    struct RootView;

    impl RootView {
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

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            NSColor::blackColor().colorWithAlphaComponent(0.28).setFill();
            NSBezierPath::fillRect(self.bounds());
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let p = self.convertPoint_fromView(event.locationInWindow(), None);
            let inside_card = ui().is_some_and(|u| {
                let f = u.card.frame();
                p.x >= f.origin.x && p.x <= f.origin.x + f.size.width && p.y >= f.origin.y && p.y <= f.origin.y + f.size.height
            });
            if !inside_card {
                hide();
            }
        }

        #[unsafe(method(resizeSubviewsWithOldSize:))]
        fn resize_subviews(&self, old: NSSize) {
            unsafe {
                let _: () = msg_send![super(self), resizeSubviewsWithOldSize: old];
            }
            refresh();
        }
    }

    unsafe impl NSObjectProtocol for RootView {}
);

define_class!(
    /// Near-opaque card base drawn with a semantic color (resolved per appearance at draw time).
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastPaletteBacking"]
    struct BackingView;

    impl BackingView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            NSColor::windowBackgroundColor().colorWithAlphaComponent(0.94).setFill();
            NSBezierPath::fillRect(self.bounds());
        }
    }

    unsafe impl NSObjectProtocol for BackingView {}
);

define_class!(
    /// Search field delegate: reports every edit of the text.
    #[unsafe(super(objc2_foundation::NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastPaletteTarget"]
    struct PaletteTarget;

    impl PaletteTarget {}

    unsafe impl NSObjectProtocol for PaletteTarget {}

    unsafe impl NSControlTextEditingDelegate for PaletteTarget {
        #[unsafe(method(controlTextDidChange:))]
        fn control_text_did_change(&self, _n: &NSNotification) {
            fire_query();
        }
    }

    unsafe impl NSTextFieldDelegate for PaletteTarget {}
);

// ---------------------------------------------------------------- build

fn label(mtm: MainThreadMarker, text: &str, size: f64, align: NSTextAlignment) -> Retained<NSTextField> {
    let l = NSTextField::labelWithString(&NSString::from_str(text), mtm);
    l.setFont(Some(&NSFont::systemFontOfSize(size)));
    l.setAlignment(align);
    l.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    l
}

fn symbol(name: &str) -> Option<Retained<NSImage>> {
    NSImage::imageWithSystemSymbolName_accessibilityDescription(&NSString::from_str(name), None)
}

fn symbol_view(mtm: MainThreadMarker, name: &str) -> Option<Retained<NSImageView>> {
    let image = symbol(name)?;
    let v = NSImageView::imageViewWithImage(&image, mtm);
    v.setImageScaling(NSImageScaling::ScaleProportionallyDown);
    Some(v)
}

fn build(mtm: MainThreadMarker) -> Rc<Ui> {
    let frame = rect(0.0, 0.0, 560.0, 420.0);
    let root: Retained<RootView> = unsafe { msg_send![mtm.alloc::<RootView>(), initWithFrame: frame] };
    root.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable);

    let card = NSVisualEffectView::initWithFrame(mtm.alloc::<NSVisualEffectView>(), rect(0.0, 0.0, CARD_W, 200.0));
    card.setMaterial(NSVisualEffectMaterial::Popover);
    card.setBlendingMode(NSVisualEffectBlendingMode::WithinWindow);
    card.setState(NSVisualEffectState::Active);
    card.setWantsLayer(true);
    unsafe {
        let layer: Option<Retained<AnyObject>> = msg_send![&*card, layer];
        if let Some(layer) = layer {
            let _: () = msg_send![&*layer, setCornerRadius: 12.0f64];
            let _: () = msg_send![&*layer, setMasksToBounds: true];
            let _: () = msg_send![&*layer, setBorderWidth: 1.0f64];
            // Hairline in the semantic separator color (resolved for the current appearance).
            let color = NSColor::separatorColor().CGColor();
            let _: () = msg_send![&*layer, setBorderColor: &*color];
        }
    }
    root.addSubview(&card);

    // Near-opaque base under the vibrancy so the note behind never competes with the rows.
    let backing: Retained<BackingView> = unsafe { msg_send![mtm.alloc::<BackingView>(), initWithFrame: rect(0.0, 0.0, CARD_W, 200.0)] };
    card.addSubview(&backing);

    let target: Retained<PaletteTarget> = unsafe { msg_send![mtm.alloc::<PaletteTarget>(), init] };

    let icon = symbol_view(mtm, "magnifyingglass")
        .unwrap_or_else(|| NSImageView::initWithFrame(mtm.alloc::<NSImageView>(), rect(0.0, 0.0, 16.0, 16.0)));
    icon.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
    card.addSubview(&icon);

    let field = NSTextField::initWithFrame(mtm.alloc::<NSTextField>(), rect(0.0, 0.0, 300.0, 22.0));
    field.setBordered(false);
    field.setBezeled(false);
    field.setDrawsBackground(false);
    field.setFocusRingType(NSFocusRingType::None);
    field.setEditable(true);
    field.setSelectable(true);
    field.setFont(Some(&NSFont::systemFontOfSize(16.0)));
    field.setTextColor(Some(&NSColor::labelColor()));
    let delegate: &ProtocolObject<dyn NSTextFieldDelegate> = ProtocolObject::from_ref(&*target);
    unsafe { field.setDelegate(Some(delegate)) };
    card.addSubview(&field);

    let sep = NSBox::initWithFrame(mtm.alloc::<NSBox>(), rect(0.0, 0.0, CARD_W, 1.0));
    sep.setBoxType(NSBoxType::Separator);
    card.addSubview(&sep);

    let scroll = NSScrollView::initWithFrame(mtm.alloc::<NSScrollView>(), rect(0.0, 0.0, CARD_W, 100.0));
    scroll.setHasVerticalScroller(true);
    scroll.setAutohidesScrollers(true);
    scroll.setDrawsBackground(false);
    scroll.setBorderType(objc2_app_kit::NSBorderType::NoBorder);
    let rows_view: Retained<RowsView> = unsafe { msg_send![mtm.alloc::<RowsView>(), initWithFrame: rect(0.0, 0.0, CARD_W, 100.0)] };
    scroll.setDocumentView(Some(&rows_view));
    card.addSubview(&scroll);

    let empty = label(mtm, "", 13.0, NSTextAlignment::Center);
    empty.setTextColor(Some(&NSColor::secondaryLabelColor()));
    empty.setHidden(true);
    card.addSubview(&empty);

    Rc::new(Ui { root, card, backing, icon, field, sep, scroll, rows_view, empty, labels: RefCell::new(Vec::new()), _target: target })
}

fn ensure_ui(mtm: MainThreadMarker) -> Rc<Ui> {
    UI.with(|u| u.borrow_mut().get_or_insert_with(|| build(mtm)).clone())
}

// ---------------------------------------------------------------- layout and styling

/// Lay out the card for the current window size and rebuild the row views.
fn refresh() {
    let Some(ui) = ui() else { return };
    if !st(|s| s.visible) {
        return;
    }
    let Some(mtm) = MainThreadMarker::new() else { return };
    let size = ui.root.bounds().size;
    let (items, rows, geoms, empty_text, placeholder) =
        st(|s| (s.items.clone(), s.rows.clone(), s.geoms.clone(), s.empty_text.clone(), s.placeholder.clone()));
    let w = card_width(size.width);
    let ch = content_height(&geoms);
    let lh = list_height(ch, size.height);
    let h = SEARCH_H + lh;
    ui.card.setFrame(rect((size.width - w) / 2.0, TOP_MARGIN, w, h));
    // Card coordinates are bottom-up: the list sits at the bottom, the search row on top.
    ui.backing.setFrame(rect(0.0, 0.0, w, h));
    ui.icon.setFrame(rect(14.0, lh + (SEARCH_H - 16.0) / 2.0, 16.0, 16.0));
    ui.field.setFrame(rect(40.0, lh + (SEARCH_H - 22.0) / 2.0, w - 40.0 - 14.0, 22.0));
    ui.field.setPlaceholderString(Some(&NSString::from_str(&placeholder)));
    ui.sep.setFrame(rect(0.0, lh, w, 1.0));
    ui.scroll.setFrame(rect(0.0, 0.0, w, lh));
    ui.rows_view.setFrame(rect(0.0, 0.0, w, ch.max(lh)));
    let is_empty = items.is_empty();
    ui.scroll.setHidden(is_empty);
    ui.empty.setHidden(!is_empty);
    ui.empty.setStringValue(&NSString::from_str(&empty_text));
    ui.empty.setFrame(rect(16.0, (lh - 17.0) / 2.0, w - 32.0, 17.0));

    for v in ui.rows_view.subviews().iter() {
        v.removeFromSuperview();
    }
    let mut labels = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let (y, rh) = geoms[i];
        match row {
            RowKind::Header(text) => {
                let l = label(mtm, text, 11.0, NSTextAlignment::Left);
                l.setFont(Some(&NSFont::systemFontOfSize_weight(11.0, unsafe { NSFontWeightSemibold })));
                l.setTextColor(Some(&NSColor::secondaryLabelColor()));
                l.setFrame(rect(16.0, y + rh - 18.0, w - 32.0, 14.0));
                ui.rows_view.addSubview(&l);
                labels.push(RowViews { glyph: None, title: None, subtitle: None, accessory: None, chevron: None, danger: false });
            }
            RowKind::Item(idx) => {
                let item = &items[*idx];
                let glyph = item.glyph.as_deref().filter(|g| !g.is_empty()).map(|g| match g.strip_prefix("sf:") {
                    Some(name) => match symbol_view(mtm, name) {
                        Some(v) => {
                            v.setFrame(rect(14.0, y + (rh - 16.0) / 2.0, 20.0, 16.0));
                            ui.rows_view.addSubview(&v);
                            GlyphView::Image(v)
                        }
                        None => GlyphView::Text(label(mtm, "", 14.0, NSTextAlignment::Center)),
                    },
                    None => {
                        let l = label(mtm, g, 15.0, NSTextAlignment::Center);
                        l.setFrame(rect(12.0, y + (rh - 20.0) / 2.0, 24.0, 20.0));
                        ui.rows_view.addSubview(&l);
                        GlyphView::Text(l)
                    }
                });
                let chevron = item.has_children.then(|| symbol_view(mtm, "chevron.right")).flatten().inspect(|c| {
                    c.setFrame(rect(w - ROW_INSET - 6.0 - 10.0, y + (rh - 12.0) / 2.0, 10.0, 12.0));
                    ui.rows_view.addSubview(c);
                });
                let mut right = ROW_INSET + 8.0;
                if item.has_children {
                    right += CHEVRON_W;
                }
                let accessory = item.accessory.as_deref().map(|a| {
                    let l = label(mtm, a, 12.0, NSTextAlignment::Right);
                    l.setFrame(rect(w - right - ACCESSORY_W, y + (rh - 15.0) / 2.0, ACCESSORY_W, 15.0));
                    ui.rows_view.addSubview(&l);
                    l
                });
                let title_right = right + if item.accessory.is_some() { ACCESSORY_W + 8.0 } else { 0.0 };
                let title_x = 44.0;
                let title_w = (w - title_x - title_right).max(20.0);
                let title = label(mtm, &item.title, 13.0, NSTextAlignment::Left);
                let subtitle = item.subtitle.as_deref().map(|sub| {
                    let l = label(mtm, sub, 11.0, NSTextAlignment::Left);
                    l.setFrame(rect(title_x, y + 19.0, title_w, 14.0));
                    ui.rows_view.addSubview(&l);
                    l
                });
                title.setFrame(rect(title_x, if subtitle.is_some() { y + 3.0 } else { y + (rh - 17.0) / 2.0 }, title_w, 17.0));
                ui.rows_view.addSubview(&title);
                labels.push(RowViews { glyph, title: Some(title), subtitle, accessory, chevron, danger: item.danger });
            }
        }
    }
    *ui.labels.borrow_mut() = labels;
    restyle();
    ui.rows_view.setNeedsDisplay(true);
    scroll_to_selected();
}

/// Semantic colors; the selected row sits on the accent color with white text.
fn restyle() {
    let Some(ui) = ui() else { return };
    let selected = st(|s| s.selected);
    for (i, l) in ui.labels.borrow().iter().enumerate() {
        let on = selected == Some(i);
        let white = NSColor::alternateSelectedControlTextColor();
        let main = if on {
            white.clone()
        } else if l.danger {
            NSColor::systemRedColor()
        } else {
            NSColor::labelColor()
        };
        let soft = if on { white.colorWithAlphaComponent(0.8) } else { NSColor::secondaryLabelColor() };
        if let Some(g) = &l.glyph {
            g.set_color(&main);
        }
        if let Some(t) = &l.title {
            t.setTextColor(Some(&main));
        }
        if let Some(t) = &l.subtitle {
            t.setTextColor(Some(&soft));
        }
        if let Some(t) = &l.accessory {
            t.setTextColor(Some(&soft));
        }
        if let Some(c) = &l.chevron {
            c.setContentTintColor(Some(&soft));
        }
    }
}

fn scroll_to_selected() {
    let Some(ui) = ui() else { return };
    let Some((row, (y, h))) = st(|s| s.selected.and_then(|r| s.geoms.get(r).map(|g| (r, *g)))) else { return };
    let clip = ui.scroll.contentView();
    let b = clip.bounds();
    // The first selectable row scrolls to the top so its section header stays visible.
    let first = st(|s| first_selectable(&s.rows) == Some(row));
    let last = st(|s| s.selected == Some(s.rows.len().saturating_sub(1)));
    // The first and last rows bring their padding along, so header and bottom edge show.
    let (y, h) = match (first, last) {
        (true, _) => (0.0, h + y),
        (_, true) => (y, h + LIST_PAD),
        _ => (y, h),
    };
    let target = ensure_visible(b.origin.y, b.size.height, y, h);
    if (target - b.origin.y).abs() > f64::EPSILON {
        ui.rows_view.scrollPoint(NSPoint::new(0.0, target));
    }
}

// ---------------------------------------------------------------- callbacks

fn fire_query() {
    let Some(ui) = ui() else { return };
    let q = ui.field.stringValue().to_string();
    let cb = st(|s| s.on_query.clone());
    if let Some(cb) = cb {
        cb(&q);
    }
}

fn pick_selected() {
    let (id, cb) = st(|s| {
        let id = s.selected.and_then(|r| match s.rows.get(r) {
            Some(RowKind::Item(i)) => Some(s.items[*i].id.clone()),
            _ => None,
        });
        (id, s.on_pick.clone())
    });
    if let (Some(id), Some(cb)) = (id, cb) {
        cb(id);
    }
}

fn move_selection(delta: i32) {
    let changed = st(|s| {
        let next = step_selection(&s.rows, s.selected, delta);
        let changed = next != s.selected;
        s.selected = next;
        changed
    });
    if changed {
        restyle();
        if let Some(ui) = ui() {
            ui.rows_view.setNeedsDisplay(true);
        }
        scroll_to_selected();
    }
}

fn set_level(placeholder: &str, items: Vec<PaletteItem>, selected_id: Option<&str>) {
    st(|s| {
        s.placeholder = placeholder.to_string();
        s.rows = build_rows(&items);
        s.geoms = row_geometry(&items, &s.rows);
        s.selected = keep_selection(&items, &s.rows, selected_id);
        s.items = items;
    });
    refresh();
}

fn selected_id() -> Option<String> {
    st(|s| {
        s.selected.and_then(|r| match s.rows.get(r) {
            Some(RowKind::Item(i)) => Some(s.items[*i].id.clone()),
            _ => None,
        })
    })
}

// ---------------------------------------------------------------- key routing

/// Handle the palette's own keys while it is visible (called by the notes window before any
/// other shortcut): Up/Down, Enter, Esc and ⌘[. True when the event was consumed.
pub(crate) fn handle_key(event: &NSEvent) -> bool {
    if !is_visible() || event.r#type() != NSEventType::KeyDown {
        return false;
    }
    let mods = event.modifierFlags()
        & (NSEventModifierFlags::Command | NSEventModifierFlags::Shift | NSEventModifierFlags::Option | NSEventModifierFlags::Control);
    if mods.is_empty() {
        match event.keyCode() {
            KEY_DOWN => move_selection(1),
            KEY_UP => move_selection(-1),
            KEY_RETURN | KEY_ENTER => pick_selected(),
            KEY_ESCAPE => {
                if depth() > 1 {
                    pop_level();
                } else {
                    hide();
                }
            }
            _ => return false,
        }
        return true;
    }
    if mods == NSEventModifierFlags::Command && event.charactersIgnoringModifiers().is_some_and(|s| s.to_string() == "[") {
        pop_level();
        return true;
    }
    false
}

// ---------------------------------------------------------------- public API

/// Open the palette at the root level with `items`; the search field takes the focus.
pub fn show(placeholder: &str, items: Vec<PaletteItem>) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let Some(content) = crate::notes::content_view() else { return };
    let ui = ensure_ui(mtm);
    st(|s| {
        s.stack.clear();
        s.visible = true;
    });
    ui.field.setStringValue(&NSString::from_str(""));
    if unsafe { ui.root.superview() }.is_none() {
        ui.root.setFrame(content.bounds());
        content.addSubview(&ui.root);
    }
    set_level(placeholder, items, None);
    if let Some(window) = ui.root.window() {
        let _ = window.makeFirstResponder(Some(&ui.field));
    }
}

/// Replace the rows of the current level; the selection stays on the same id when it still
/// exists, else moves to the first row.
pub fn set_items(items: Vec<PaletteItem>) {
    if !is_visible() {
        return;
    }
    let (placeholder, prev) = (st(|s| s.placeholder.clone()), selected_id());
    set_level(&placeholder, items, prev.as_deref());
}

/// Open a submenu: the current level stays on a stack and the search text is cleared.
pub fn push_level(placeholder: &str, items: Vec<PaletteItem>) {
    if !is_visible() {
        return;
    }
    let query = query();
    let prev = selected_id();
    st(|s| {
        let level = Level { placeholder: s.placeholder.clone(), items: std::mem::take(&mut s.items), selected_id: prev, query };
        s.stack.push(level);
    });
    if let Some(ui) = ui() {
        ui.field.setStringValue(&NSString::from_str(""));
    }
    set_level(placeholder, items, None);
}

/// Go back one level, restoring its rows, selection and search text. False at the root.
pub fn pop_level() -> bool {
    let Some(level) = st(|s| s.stack.pop()) else { return false };
    if let Some(ui) = ui() {
        ui.field.setStringValue(&NSString::from_str(&level.query));
    }
    set_level(&level.placeholder, level.items, level.selected_id.as_deref());
    true
}

/// Number of open levels: 0 when hidden, 1 at the root, more inside submenus.
pub fn depth() -> usize {
    st(|s| if s.visible { s.stack.len() + 1 } else { 0 })
}

/// Close the palette, give the focus back to the editor and fire the close callback.
pub fn hide() {
    let was = st(|s| {
        s.stack.clear();
        std::mem::replace(&mut s.visible, false)
    });
    if !was {
        return;
    }
    if let Some(ui) = ui() {
        ui.root.removeFromSuperview();
    }
    crate::notes::focus_editor();
    let cb = st(|s| s.on_close.clone());
    if let Some(cb) = cb {
        cb();
    }
}

pub fn is_visible() -> bool {
    STATE.with(|s| s.try_borrow().map(|s| s.visible).unwrap_or(true))
}

/// Current text of the search field.
pub fn query() -> String {
    ui().map(|u| u.field.stringValue().to_string()).unwrap_or_default()
}

/// Called on every edit of the search text; the controller refilters and calls `set_items`.
pub fn set_on_query(cb: Box<dyn Fn(&str)>) {
    st(|s| s.on_query = Some(Rc::from(cb)));
}

/// Called with the item id on Enter or a click. The palette does not close itself.
pub fn set_on_pick(cb: Box<dyn Fn(String)>) {
    st(|s| s.on_pick = Some(Rc::from(cb)));
}

/// Called whenever the palette hides (Esc at the root, backdrop click, `hide()`).
pub fn set_on_close(cb: Box<dyn Fn()>) {
    st(|s| s.on_close = Some(Rc::from(cb)));
}

/// Text shown when there are no rows.
pub fn set_empty_text(text: &str) {
    st(|s| s.empty_text = text.to_string());
    refresh();
}

/// Render the notes window content (with the palette over it) to a PNG; false when hidden.
pub fn snapshot_png(path: &Path) -> bool {
    is_visible() && crate::notes::snapshot_png(path)
}

// ---------------------------------------------------------------- inspection (checks)

/// Id of the highlighted row.
pub fn selected_item_id() -> Option<String> {
    selected_id()
}

/// Whether the search field is the window's first responder (its field editor has focus).
pub fn search_focused() -> bool {
    let Some(ui) = ui() else { return false };
    let Some(window) = ui.root.window() else { return false };
    let (Some(editor), Some(fr)) = (ui.field.currentEditor(), window.firstResponder()) else { return false };
    std::ptr::eq(&*editor as *const _ as *const u8, &*fr as *const _ as *const u8)
}

/// Whether the empty-state text is showing.
pub fn empty_text_visible() -> bool {
    is_visible() && ui().is_some_and(|u| !u.empty.isHidden() && !u.empty.stringValue().to_string().is_empty())
}

/// `(width, height)` of the card.
pub fn card_size() -> Option<(f64, f64)> {
    let f = ui()?.card.frame();
    Some((f.size.width, f.size.height))
}

/// Center of the row with this item id in window coordinates, for synthesized clicks.
pub fn row_window_point(id: &str) -> Option<(f64, f64)> {
    let ui = ui()?;
    let (y, h) = st(|s| row_of_id(&s.items, &s.rows, id).and_then(|r| s.geoms.get(r).copied()))?;
    let p = ui.rows_view.convertPoint_toView(NSPoint::new(ROW_INSET + 60.0, y + h / 2.0), None);
    Some((p.x, p.y))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, section: Option<&str>, subtitle: bool) -> PaletteItem {
        PaletteItem {
            id: id.to_string(),
            title: id.to_uppercase(),
            subtitle: subtitle.then(|| "sub".to_string()),
            section: section.map(str::to_string),
            ..PaletteItem::default()
        }
    }

    fn sample() -> Vec<PaletteItem> {
        vec![item("a", Some("One"), false), item("b", Some("One"), true), item("c", Some("Two"), false), item("d", None, false)]
    }

    #[test]
    fn headers_group_consecutive_sections() {
        let rows = build_rows(&sample());
        assert_eq!(
            rows,
            vec![
                RowKind::Header("One".into()),
                RowKind::Item(0),
                RowKind::Item(1),
                RowKind::Header("Two".into()),
                RowKind::Item(2),
                RowKind::Item(3)
            ]
        );
        // the same section name again after a gap starts a new header
        let again = vec![item("a", Some("S"), false), item("b", None, false), item("c", Some("S"), false)];
        assert_eq!(build_rows(&again).iter().filter(|r| matches!(r, RowKind::Header(_))).count(), 2);
        assert!(build_rows(&[]).is_empty());
    }

    #[test]
    fn geometry_uses_row_heights() {
        let items = sample();
        let rows = build_rows(&items);
        let g = row_geometry(&items, &rows);
        assert_eq!(g[0], (LIST_PAD, HEADER_ROW_H));
        assert_eq!(g[1], (LIST_PAD + HEADER_ROW_H, ROW_H_COMPACT));
        assert_eq!(g[2].1, ROW_H);
        assert_eq!(content_height(&g), g[5].0 + g[5].1 + LIST_PAD);
        assert_eq!(content_height(&[]), EMPTY_H);
        assert_eq!(row_at(&g, g[2].0 + 1.0), Some(2));
        assert_eq!(row_at(&g, 0.0), None);
        assert_eq!(row_at(&g, 10_000.0), None);
    }

    #[test]
    fn selection_wraps_and_skips_headers() {
        let rows = build_rows(&sample());
        // selectable rows: 1, 2, 4, 5
        assert_eq!(step_selection(&rows, Some(1), 1), Some(2));
        assert_eq!(step_selection(&rows, Some(2), 1), Some(4)); // jumps over the "Two" header
        assert_eq!(step_selection(&rows, Some(5), 1), Some(1)); // wraps forward
        assert_eq!(step_selection(&rows, Some(1), -1), Some(5)); // wraps backward
        assert_eq!(step_selection(&rows, Some(4), -1), Some(2));
        assert_eq!(step_selection(&rows, None, 1), Some(1));
        assert_eq!(step_selection(&rows, None, -1), Some(5));
        assert_eq!(step_selection(&rows, Some(0), 1), Some(1)); // a stale header index recovers
        assert_eq!(step_selection(&[], None, 1), None);
        assert_eq!(step_selection(&build_rows(&[item("x", None, false)]), Some(0), 1), Some(0));
    }

    #[test]
    fn selection_is_kept_by_id() {
        let items = sample();
        let rows = build_rows(&items);
        assert_eq!(keep_selection(&items, &rows, Some("c")), Some(4));
        assert_eq!(keep_selection(&items, &rows, Some("zzz")), Some(1));
        assert_eq!(keep_selection(&items, &rows, None), Some(1));
        let fewer = vec![item("d", None, false)];
        let r2 = build_rows(&fewer);
        assert_eq!(keep_selection(&fewer, &r2, Some("c")), Some(0));
        assert_eq!(keep_selection(&[], &[], Some("c")), None);
        assert_eq!(row_of_id(&items, &rows, "d"), Some(5));
    }

    #[test]
    fn ensure_visible_scrolls_minimally() {
        assert_eq!(ensure_visible(0.0, 100.0, 10.0, 28.0), 0.0); // already visible
        assert_eq!(ensure_visible(0.0, 100.0, 90.0, 28.0), 18.0); // below: bottom edge aligned
        assert_eq!(ensure_visible(50.0, 100.0, 20.0, 28.0), 20.0); // above: top edge aligned
        assert_eq!(ensure_visible(50.0, 100.0, 50.0, 50.0), 50.0); // exactly fits
    }

    #[test]
    fn card_and_list_sizes_are_capped() {
        assert_eq!(card_width(560.0), 420.0);
        assert_eq!(card_width(400.0), 368.0);
        assert_eq!(card_width(320.0), 288.0);
        assert!(card_width(100.0) >= 160.0);
        assert_eq!(list_height(100.0, 600.0), 100.0);
        assert_eq!(list_height(2000.0, 900.0), MAX_LIST_H);
        const { assert!(SEARCH_H + MAX_LIST_H <= 340.0) };
        // a short window shrinks the list but never below the minimum
        assert_eq!(list_height(2000.0, 300.0), 300.0 - TOP_MARGIN - WINDOW_MARGIN - SEARCH_H);
        assert_eq!(list_height(2000.0, 100.0), MIN_LIST_H);
    }
}
