//! Floating file shelf (T15, S5 "Float"): a native always-on-top panel that keeps files
//! pinned to a screen edge. Drop files in, drag them (one or all) back out.
//! All AppKit calls happen on the main thread; the pure logic is unit-tested below.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameVibrantDark, NSApplication, NSBackingStoreType, NSBezelStyle, NSButton,
    NSColor, NSDragOperation, NSDraggingDestination, NSDraggingInfo, NSDraggingItem, NSDraggingSource, NSEvent, NSEventType,
    NSFloatingWindowLevel, NSImageView, NSLineBreakMode, NSPanel, NSPasteboardTypeFileURL, NSPasteboardWriting, NSScreen, NSTextField,
    NSView, NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView, NSWindowCollectionBehavior,
    NSWindowStyleMask, NSWorkspace,
};
use objc2_foundation::{NSArray, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};

/// Screen edge the shelf is pinned to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Left,
    Right,
}

const WIDTH: f64 = 260.0;
const HEADER: f64 = 40.0;
const ROW: f64 = 44.0;
const PAD: f64 = 8.0;
const MARGIN: f64 = 8.0;
const MAX_ROWS: usize = 10;
const EMPTY_H: f64 = 56.0;

// ---------------------------------------------------------------- pure logic

/// Append the paths not already present; true when the list changed.
fn add_unique(list: &mut Vec<PathBuf>, new: &[PathBuf]) -> bool {
    let before = list.len();
    for p in new {
        if !list.contains(p) {
            list.push(p.clone());
        }
    }
    list.len() != before
}

/// Remove one path; true when it was present.
fn remove_path(list: &mut Vec<PathBuf>, path: &Path) -> bool {
    let before = list.len();
    list.retain(|p| p != path);
    list.len() != before
}

/// Bottom-left origin of a panel pinned to `edge`, vertically centred in `visible`
/// (x, y, w, h in AppKit's bottom-up coordinates).
fn edge_origin(visible: (f64, f64, f64, f64), panel: (f64, f64), edge: Edge, margin: f64) -> (f64, f64) {
    let (vx, vy, vw, vh) = visible;
    let x = match edge {
        Edge::Left => vx + margin,
        Edge::Right => vx + vw - panel.0 - margin,
    };
    (x, vy + (vh - panel.1) / 2.0)
}

/// Which edge a panel whose centre is at `panel_mid_x` is closest to.
fn nearest_edge(panel_mid_x: f64, screen_x: f64, screen_w: f64) -> Edge {
    if panel_mid_x < screen_x + screen_w / 2.0 {
        Edge::Left
    } else {
        Edge::Right
    }
}

fn panel_height(rows: usize) -> f64 {
    let body = if rows == 0 { EMPTY_H } else { rows.min(MAX_ROWS) as f64 * ROW + PAD };
    HEADER + body
}

// ------------------------------------------------------------------ state

struct Ui {
    panel: Retained<ShelfPanel>,
    content: Retained<ShelfView>,
    title_row: Retained<ShelfRow>,
    title: Retained<NSTextField>,
    empty: Retained<NSTextField>,
    target: Retained<ShelfTarget>,
    dynamic: Vec<Retained<NSView>>,
}

type ChangeCb = Rc<dyn Fn(Vec<PathBuf>)>;

thread_local! {
    static UI: RefCell<Option<Ui>> = const { RefCell::new(None) };
    static ITEMS: RefCell<Vec<PathBuf>> = const { RefCell::new(Vec::new()) };
    static EDGE: Cell<Edge> = const { Cell::new(Edge::Right) };
    static ON_CHANGE: RefCell<Option<ChangeCb>> = const { RefCell::new(None) };
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
}

fn notify() {
    let cb = ON_CHANGE.with(|c| c.borrow().clone());
    if let Some(cb) = cb {
        cb(items());
    }
}

fn mtm_alloc_item() -> objc2::rc::Allocated<NSDraggingItem> {
    MainThreadMarker::new().expect("main thread").alloc::<NSDraggingItem>()
}

fn icon_for(path: &Path) -> Retained<objc2_app_kit::NSImage> {
    NSWorkspace::sharedWorkspace().iconForFile(&NSString::from_str(&path.to_string_lossy()))
}

// ---------------------------------------------------------------- AppKit classes

struct RowIvars {
    /// `Some` = drags this file; `None` = the header handle, drags the whole shelf.
    path: Option<PathBuf>,
}

define_class!(
    /// A shelf row: shows icon + name and starts a file drag-out.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastShelfRow"]
    #[ivars = RowIvars]
    struct ShelfRow;

    impl ShelfRow {
        #[unsafe(method(mouseDownCanMoveWindow))]
        fn mouse_down_can_move_window(&self) -> bool {
            false
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        // Subviews (icon, label) must not swallow the drag.
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

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            let paths: Vec<PathBuf> = match &self.ivars().path {
                Some(p) => vec![p.clone()],
                None => items(),
            };
            if paths.is_empty() {
                return;
            }
            let mut drag_items = Vec::new();
            for (i, p) in paths.iter().enumerate() {
                let Some(url) = crate::clipboard::ns_file_url(p) else { continue };
                let writer: &ProtocolObject<dyn NSPasteboardWriting> =
                    ProtocolObject::from_ref(&*url);
                let item = NSDraggingItem::initWithPasteboardWriter(
                    mtm_alloc_item(),
                    writer,
                );
                let off = (i.min(5) as f64) * 6.0;
                let icon = icon_for(p);
                unsafe {
                    item.setDraggingFrame_contents(
                        rect(8.0 + off, 6.0 - off, 32.0, 32.0),
                        Some(&*icon),
                    );
                }
                drag_items.push(item);
            }
            let array = NSArray::from_retained_slice(&drag_items);
            let source: &ProtocolObject<dyn NSDraggingSource> = ProtocolObject::from_ref(self);
            self.beginDraggingSessionWithItems_event_source(&array, event, source);
        }
    }

    unsafe impl NSObjectProtocol for ShelfRow {}

    unsafe impl NSDraggingSource for ShelfRow {
        #[unsafe(method(draggingSession:sourceOperationMaskForDraggingContext:))]
        fn source_mask(
            &self,
            _session: &objc2_app_kit::NSDraggingSession,
            _context: objc2_app_kit::NSDraggingContext,
        ) -> NSDragOperation {
            NSDragOperation::Copy | NSDragOperation::Move | NSDragOperation::Generic
        }
    }
);

define_class!(
    /// Content view: rounded HUD background that accepts file drops.
    #[unsafe(super(NSVisualEffectView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastShelfView"]
    struct ShelfView;

    impl ShelfView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }
    }

    unsafe impl NSObjectProtocol for ShelfView {}

    unsafe impl NSDraggingDestination for ShelfView {
        #[unsafe(method(draggingEntered:))]
        fn dragging_entered(&self, _sender: &ProtocolObject<dyn NSDraggingInfo>) -> NSDragOperation {
            NSDragOperation::Copy
        }

        #[unsafe(method(performDragOperation:))]
        fn perform_drag_operation(&self, sender: &ProtocolObject<dyn NSDraggingInfo>) -> bool {
            let pb = sender.draggingPasteboard();
            let paths: Vec<PathBuf> = unsafe {
                pb.pasteboardItems()
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|it| it.stringForType(NSPasteboardTypeFileURL))
                            .filter_map(|s| crate::clipboard::file_url_to_path(&s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default()
            };
            let ok = !paths.is_empty();
            if ok {
                add(&paths);
            }
            ok
        }
    }
);

define_class!(
    /// Borderless non-activating panel; snaps to the nearest edge after a background drag.
    #[unsafe(super(NSPanel))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastShelfPanel"]
    struct ShelfPanel;

    impl ShelfPanel {
        #[unsafe(method(sendEvent:))]
        fn send_event(&self, event: &NSEvent) {
            unsafe {
                let _: () = msg_send![super(self), sendEvent: event];
            }
            let t = event.r#type();
            if t == NSEventType::LeftMouseUp || t == NSEventType::LeftMouseDown {
                snap_to_edge(self);
            }
        }

        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key(&self) -> bool {
            false
        }
    }
);

define_class!(
    /// Target for the header/row buttons.
    #[unsafe(super(objc2_foundation::NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastShelfTarget"]
    struct ShelfTarget;

    impl ShelfTarget {
        #[unsafe(method(shelfClose:))]
        fn shelf_close(&self, _sender: Option<&AnyObject>) {
            hide();
        }

        #[unsafe(method(shelfClear:))]
        fn shelf_clear(&self, _sender: Option<&AnyObject>) {
            clear();
        }

        #[unsafe(method(shelfRemove:))]
        fn shelf_remove(&self, sender: &AnyObject) {
            let tag: isize = unsafe { msg_send![sender, tag] };
            let path = ITEMS.with(|i| i.borrow().get(tag as usize).cloned());
            if let Some(p) = path {
                remove(&p);
            }
        }
    }

    unsafe impl NSObjectProtocol for ShelfTarget {}
);

// ---------------------------------------------------------------- placement

fn screen_under_mouse(mtm: MainThreadMarker) -> Option<Retained<NSScreen>> {
    let m = NSEvent::mouseLocation();
    let screens = NSScreen::screens(mtm);
    for s in screens.iter() {
        let f = s.frame();
        if m.x >= f.origin.x && m.x < f.origin.x + f.size.width && m.y >= f.origin.y && m.y < f.origin.y + f.size.height {
            return Some(s);
        }
    }
    NSScreen::mainScreen(mtm)
}

fn visible_tuple(s: &NSScreen) -> (f64, f64, f64, f64) {
    let v = s.visibleFrame();
    (v.origin.x, v.origin.y, v.size.width, v.size.height)
}

/// Pin to the current edge on `screen`, vertically centred.
fn pin(panel: &ShelfPanel, screen: &NSScreen) {
    let h = panel.frame().size.height;
    let (x, y) = edge_origin(visible_tuple(screen), (WIDTH, h), EDGE.get(), MARGIN);
    panel.setFrame_display(rect(x, y, WIDTH, h), true);
}

/// After the user moved the panel: pick the nearest edge of its screen, keep its height
/// position (clamped to the visible area).
fn snap_to_edge(panel: &ShelfPanel) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let f = panel.frame();
    let mid = (f.origin.x + f.size.width / 2.0, f.origin.y + f.size.height / 2.0);
    let screens = NSScreen::screens(mtm);
    let screen = screens.iter().find(|s| {
        let sf = s.frame();
        mid.0 >= sf.origin.x && mid.0 < sf.origin.x + sf.size.width && mid.1 >= sf.origin.y && mid.1 < sf.origin.y + sf.size.height
    });
    let Some(screen) = screen.or_else(|| NSScreen::mainScreen(mtm)) else { return };
    let v = visible_tuple(&screen);
    let edge = nearest_edge(mid.0, v.0, v.2);
    EDGE.set(edge);
    let (x, _) = edge_origin(v, (f.size.width, f.size.height), edge, MARGIN);
    let y = f.origin.y.clamp(v.1 + MARGIN, (v.1 + v.3 - f.size.height - MARGIN).max(v.1));
    if (x - f.origin.x).abs() > 0.5 || (y - f.origin.y).abs() > 0.5 {
        panel.setFrame_display_animate(rect(x, y, f.size.width, f.size.height), true, true);
    }
}

// ---------------------------------------------------------------- build & refresh

fn small_button(
    mtm: MainThreadMarker,
    title: &str,
    target: &ShelfTarget,
    action: objc2::runtime::Sel,
    frame: NSRect,
) -> Retained<NSButton> {
    let b = unsafe { NSButton::buttonWithTitle_target_action(&NSString::from_str(title), Some(target), Some(action), mtm) };
    b.setBezelStyle(NSBezelStyle::SmallSquare);
    b.setFrame(frame);
    b
}

fn build(mtm: MainThreadMarker) -> Ui {
    let h = panel_height(0);
    let frame = rect(0.0, 0.0, WIDTH, h);
    let style = NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel;
    let panel: Retained<ShelfPanel> = unsafe {
        msg_send![
            super(mtm.alloc::<ShelfPanel>().set_ivars(())),
            initWithContentRect: frame,
            styleMask: style,
            backing: NSBackingStoreType::Buffered,
            defer: false
        ]
    };
    unsafe { panel.setReleasedWhenClosed(false) };
    panel.setLevel(NSFloatingWindowLevel);
    panel.setFloatingPanel(true);
    panel.setHidesOnDeactivate(false);
    panel.setMovableByWindowBackground(true);
    panel.setOpaque(false);
    panel.setHasShadow(true);
    panel.setBackgroundColor(Some(&NSColor::clearColor()));
    panel.setCollectionBehavior(NSWindowCollectionBehavior::CanJoinAllSpaces | NSWindowCollectionBehavior::FullScreenAuxiliary);
    panel.setAppearance(NSAppearance::appearanceNamed(unsafe { NSAppearanceNameVibrantDark }).as_deref());

    let content: Retained<ShelfView> = unsafe { msg_send![super(mtm.alloc::<ShelfView>().set_ivars(())), initWithFrame: frame] };
    content.setMaterial(NSVisualEffectMaterial::HUDWindow);
    content.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
    content.setState(NSVisualEffectState::Active);
    content.setWantsLayer(true);
    unsafe {
        let layer: Option<Retained<AnyObject>> = msg_send![&*content, layer];
        if let Some(layer) = layer {
            let _: () = msg_send![&*layer, setCornerRadius: 14.0f64];
            let _: () = msg_send![&*layer, setMasksToBounds: true];
        }
    }
    content.registerForDraggedTypes(&NSArray::from_slice(&[unsafe { NSPasteboardTypeFileURL }]));
    panel.setContentView(Some(&content));

    let target: Retained<ShelfTarget> = unsafe { msg_send![mtm.alloc::<ShelfTarget>(), init] };

    // Header: drag-all handle + Clear + close.
    let title_row: Retained<ShelfRow> = unsafe {
        msg_send![
            super(mtm.alloc::<ShelfRow>().set_ivars(RowIvars { path: None })),
            initWithFrame: rect(10.0, 8.0, 150.0, 24.0)
        ]
    };
    let title = NSTextField::labelWithString(&NSString::from_str(&ferrocast_core::i18n::tr("Shelf")), mtm);
    title.setFrame(rect(4.0, 3.0, 142.0, 18.0));
    title_row.addSubview(&title);
    content.addSubview(&title_row);

    let clear = small_button(mtm, &ferrocast_core::i18n::tr("Clear"), &target, sel!(shelfClear:), rect(164.0, 10.0, 50.0, 20.0));
    content.addSubview(&clear);
    let close = small_button(mtm, "×", &target, sel!(shelfClose:), rect(222.0, 10.0, 28.0, 20.0));
    content.addSubview(&close);

    let empty = NSTextField::labelWithString(&NSString::from_str(&ferrocast_core::i18n::tr("Drop files here")), mtm);
    empty.setFrame(rect(PAD, HEADER + 12.0, WIDTH - 2.0 * PAD, 18.0));
    empty.setAlignment(objc2_app_kit::NSTextAlignment::Center);
    content.addSubview(&empty);

    Ui { panel, content, title_row, title, empty, target, dynamic: Vec::new() }
}

/// Rebuild rows from ITEMS and resize the panel around its vertical centre.
fn refresh(mtm: MainThreadMarker) {
    let list = items();
    UI.with(|ui| {
        let mut ui = ui.borrow_mut();
        let Some(ui) = ui.as_mut() else { return };
        for v in ui.dynamic.drain(..) {
            v.removeFromSuperview();
        }
        ui.empty.setHidden(!list.is_empty());
        let label = match list.len() {
            0 => ferrocast_core::i18n::tr("Shelf").into_owned(),
            1 => ferrocast_core::i18n::tr("Shelf · 1 item").into_owned(),
            n => ferrocast_core::i18n::trf("Shelf · {n} items (drag all)", &[("n", &n.to_string())]),
        };
        ui.title.setStringValue(&NSString::from_str(&label));
        let _ = &ui.title_row;

        for (i, p) in list.iter().take(MAX_ROWS).enumerate() {
            let y = HEADER + i as f64 * ROW;
            let row: Retained<ShelfRow> = unsafe {
                msg_send![
                    super(mtm.alloc::<ShelfRow>().set_ivars(RowIvars { path: Some(p.clone()) })),
                    initWithFrame: rect(PAD, y, WIDTH - 2.0 * PAD - 26.0, ROW - 4.0)
                ]
            };
            let icon = NSImageView::imageViewWithImage(&icon_for(p), mtm);
            icon.setFrame(rect(4.0, 4.0, 32.0, 32.0));
            row.addSubview(&icon);
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.to_string_lossy().into_owned());
            let label = NSTextField::labelWithString(&NSString::from_str(&name), mtm);
            label.setLineBreakMode(NSLineBreakMode::ByTruncatingMiddle);
            label.setFrame(rect(44.0, 11.0, WIDTH - 2.0 * PAD - 26.0 - 48.0, 18.0));
            row.addSubview(&label);
            ui.content.addSubview(&row);
            ui.dynamic.push(Retained::into_super(row));

            let rm = small_button(mtm, "−", &ui.target, sel!(shelfRemove:), rect(WIDTH - PAD - 24.0, y + 8.0, 24.0, 20.0));
            rm.setTag(i as isize);
            ui.content.addSubview(&rm);
            ui.dynamic.push(Retained::into_super(Retained::into_super(rm)));
        }

        // Resize keeping the vertical centre (clamped to the screen).
        let old = ui.panel.frame();
        let h = panel_height(list.len());
        let mut y = old.origin.y + (old.size.height - h) / 2.0;
        if let Some(s) = ui.panel.screen() {
            let v = visible_tuple(&s);
            y = y.clamp(v.1, (v.1 + v.3 - h).max(v.1));
        }
        ui.panel.setFrame_display(rect(old.origin.x, y, WIDTH, h), true);
    });
}

// ---------------------------------------------------------------- public API

/// Add `paths` (deduplicated) to the shelf and show it, pinned to the current edge on the
/// screen under the mouse. Main thread only; silently returns otherwise.
pub fn show(paths: &[PathBuf]) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    // a launcher hide() just before this call hides the whole app; bring the panel back
    // without activating (the panel must not steal focus)
    let app = NSApplication::sharedApplication(mtm);
    if app.isHidden() {
        app.unhideWithoutActivation();
    }
    UI.with(|ui| {
        let mut ui = ui.borrow_mut();
        if ui.is_none() {
            *ui = Some(build(mtm));
        }
    });
    let changed = ITEMS.with(|i| add_unique(&mut i.borrow_mut(), paths));
    refresh(mtm);
    let already = is_visible();
    UI.with(|ui| {
        if let Some(ui) = ui.borrow().as_ref() {
            if !already {
                if let Some(s) = screen_under_mouse(mtm) {
                    pin(&ui.panel, &s);
                }
            }
            ui.panel.orderFrontRegardless();
        }
    });
    if changed {
        notify();
    }
}

/// Hide the panel (items are kept).
pub fn hide() {
    UI.with(|ui| {
        if let Some(ui) = ui.borrow().as_ref() {
            ui.panel.orderOut(None);
        }
    });
}

pub fn is_visible() -> bool {
    UI.with(|ui| ui.borrow().as_ref().is_some_and(|u| u.panel.isVisible()))
}

/// Current shelf contents.
pub fn items() -> Vec<PathBuf> {
    ITEMS.with(|i| i.borrow().clone())
}

/// Pin the shelf to `edge` (on the screen it is on, or the one under the mouse).
pub fn set_edge(edge: Edge) {
    EDGE.set(edge);
    let Some(mtm) = MainThreadMarker::new() else { return };
    UI.with(|ui| {
        if let Some(ui) = ui.borrow().as_ref() {
            let screen = ui.panel.screen().or_else(|| screen_under_mouse(mtm));
            if let Some(s) = screen {
                pin(&ui.panel, &s);
            }
        }
    });
}

/// Called (main thread) with the new list whenever it changes.
pub fn set_on_change(f: Box<dyn Fn(Vec<PathBuf>)>) {
    ON_CHANGE.with(|c| *c.borrow_mut() = Some(Rc::from(f)));
}

fn add(paths: &[PathBuf]) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    if ITEMS.with(|i| add_unique(&mut i.borrow_mut(), paths)) {
        refresh(mtm);
        notify();
    }
}

/// Remove one item from the shelf (never touches the file).
pub fn remove(path: &Path) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    if ITEMS.with(|i| remove_path(&mut i.borrow_mut(), path)) {
        refresh(mtm);
        notify();
    }
}

/// Empty the shelf (never touches the files).
pub fn clear() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let had = ITEMS.with(|i| {
        let mut i = i.borrow_mut();
        let had = !i.is_empty();
        i.clear();
        had
    });
    if had {
        refresh(mtm);
        notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn add_dedups_and_reports_change() {
        let mut l = vec![p("/a")];
        assert!(add_unique(&mut l, &[p("/a"), p("/b"), p("/b")]));
        assert_eq!(l, vec![p("/a"), p("/b")]);
        assert!(!add_unique(&mut l, &[p("/a")]));
    }

    #[test]
    fn remove_only_the_named_path() {
        let mut l = vec![p("/a"), p("/b")];
        assert!(remove_path(&mut l, Path::new("/a")));
        assert!(!remove_path(&mut l, Path::new("/zzz")));
        assert_eq!(l, vec![p("/b")]);
    }

    #[test]
    fn edge_origin_left_right_centered() {
        let vis = (0.0, 25.0, 1440.0, 800.0);
        assert_eq!(edge_origin(vis, (260.0, 100.0), Edge::Left, 8.0), (8.0, 375.0));
        assert_eq!(edge_origin(vis, (260.0, 100.0), Edge::Right, 8.0), (1440.0 - 260.0 - 8.0, 375.0));
        // Second display offset to the right.
        let vis2 = (1440.0, 0.0, 1000.0, 600.0);
        assert_eq!(edge_origin(vis2, (260.0, 200.0), Edge::Left, 8.0), (1448.0, 200.0));
    }

    #[test]
    fn nearest_edge_by_half() {
        assert_eq!(nearest_edge(100.0, 0.0, 1440.0), Edge::Left);
        assert_eq!(nearest_edge(1000.0, 0.0, 1440.0), Edge::Right);
        assert_eq!(nearest_edge(1500.0, 1440.0, 1000.0), Edge::Left);
        assert_eq!(nearest_edge(2300.0, 1440.0, 1000.0), Edge::Right);
    }

    #[test]
    fn height_grows_then_caps() {
        assert_eq!(panel_height(0), HEADER + EMPTY_H);
        assert!(panel_height(2) > panel_height(1));
        assert_eq!(panel_height(50), panel_height(MAX_ROWS));
    }
}
