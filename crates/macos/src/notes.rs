//! Notes window shell (S2, S5, S19): a floating native `NSPanel` holding an `NSTextView`
//! in a scroll view, with a slim toolbar (live title, Browse Notes, New Note).
//! Plain text only for now. All AppKit calls happen on the main thread; the title logic
//! is a plain function with unit tests below.

use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{define_class, msg_send, sel, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSAutoresizingMaskOptions, NSBackingStoreType, NSBitmapImageFileType, NSButton, NSColor, NSEvent, NSEventModifierFlags,
    NSEventType, NSFloatingWindowLevel, NSFont, NSFontWeightSemibold, NSImage, NSLineBreakMode, NSPanel, NSScrollView, NSTextAlignment,
    NSTextDelegate, NSTextField, NSTextView, NSTextViewDelegate, NSView, NSVisualEffectBlendingMode, NSVisualEffectMaterial,
    NSVisualEffectState, NSVisualEffectView, NSWindowButton, NSWindowCollectionBehavior, NSWindowStyleMask, NSWindowTitleVisibility,
};
use objc2_foundation::{NSDictionary, NSEdgeInsets, NSNotification, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};

/// Toolbar actions the host reacts to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    New,
    Browse,
}

const TOOLBAR_H: f64 = 44.0;
const DEFAULT_W: f64 = 560.0;
const DEFAULT_H: f64 = 420.0;
const MIN_W: f64 = 320.0;
const MIN_H: f64 = 220.0;
const INSET: f64 = 16.0;
const FONT_SIZE: f64 = 14.0;
const MAX_TITLE_CHARS: usize = 60;
const FRAME_NAME: &str = "FerrocastNotesWindow";
const KEY_ESCAPE: u16 = 53;

// ---------------------------------------------------------------- pure logic

/// S5: the first non-blank line with leading `#`, `>`, `-`, `*` and spaces stripped, at most
/// 60 characters; "Untitled" when nothing is left. Lines that are only markers are skipped.
pub fn title_from_text(text: &str) -> String {
    for line in text.lines() {
        let stripped = line.trim_start_matches(|c: char| c.is_whitespace() || matches!(c, '#' | '>' | '-' | '*')).trim_end();
        if !stripped.is_empty() {
            return stripped.chars().take(MAX_TITLE_CHARS).collect::<String>().trim_end().to_string();
        }
    }
    "Untitled".to_string()
}

// ------------------------------------------------------------------ state

struct Ui {
    panel: Retained<NotesPanel>,
    text_view: Retained<NSTextView>,
    title: Retained<NSTextField>,
    /// Kept alive: the text view delegate and button target are weak references.
    _target: Retained<NotesTarget>,
}

type ChangeCb = Rc<dyn Fn(String)>;
type EventCb = Rc<dyn Fn(Event)>;

thread_local! {
    static UI: RefCell<Option<Ui>> = const { RefCell::new(None) };
    static ON_CHANGE: RefCell<Option<ChangeCb>> = const { RefCell::new(None) };
    static ON_EVENT: RefCell<Option<EventCb>> = const { RefCell::new(None) };
    static ESC_LOSES_FOCUS: Cell<bool> = const { Cell::new(false) };
    /// True while `set_text` writes into the text view, so no change callback fires.
    static SUPPRESS: Cell<bool> = const { Cell::new(false) };
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
}

fn fire_event(e: Event) {
    let cb = ON_EVENT.with(|c| c.borrow().clone());
    if let Some(cb) = cb {
        cb(e);
    }
}

fn refresh_title(text: &str) {
    let label = UI.with(|u| u.borrow().as_ref().map(|u| u.title.clone()));
    if let Some(label) = label {
        label.setStringValue(&NSString::from_str(&title_from_text(text)));
    }
}

// ---------------------------------------------------------------- key handling

/// Esc in "loses focus" mode: give key status to another app window or the previous app.
fn resign_key(panel: &NotesPanel) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let app = NSApplication::sharedApplication(mtm);
    let me: &NSPanel = panel;
    let other = app
        .windows()
        .iter()
        .find(|w| w.isVisible() && w.canBecomeKeyWindow() && !std::ptr::eq(&**w as *const _ as *const u8, me as *const _ as *const u8));
    match other {
        Some(w) => w.makeKeyAndOrderFront(None),
        None => app.deactivate(),
    }
}

/// Handle the window-level shortcuts; true when the event was consumed.
fn handle_shortcut(panel: &NotesPanel, event: &NSEvent) -> bool {
    if event.r#type() != NSEventType::KeyDown {
        return false;
    }
    let mods = event.modifierFlags()
        & (NSEventModifierFlags::Command | NSEventModifierFlags::Shift | NSEventModifierFlags::Option | NSEventModifierFlags::Control);
    if mods.is_empty() && event.keyCode() == KEY_ESCAPE {
        if ESC_LOSES_FOCUS.get() {
            resign_key(panel);
        } else {
            panel.orderOut(None);
        }
        return true;
    }
    if mods != NSEventModifierFlags::Command {
        return false;
    }
    let key = event.charactersIgnoringModifiers().map(|s| s.to_string().to_lowercase()).unwrap_or_default();
    match key.as_str() {
        "n" => fire_event(Event::New),
        "p" => fire_event(Event::Browse),
        "w" => panel.orderOut(None),
        _ => return false,
    }
    true
}

/// Standard edit shortcuts. An Accessory app may have no Edit menu, so route them to the
/// first responder ourselves when AppKit did not handle them.
fn edit_action(event: &NSEvent) -> Option<objc2::runtime::Sel> {
    let mods = event.modifierFlags()
        & (NSEventModifierFlags::Command | NSEventModifierFlags::Shift | NSEventModifierFlags::Option | NSEventModifierFlags::Control);
    let key = event.charactersIgnoringModifiers()?.to_string().to_lowercase();
    if mods == NSEventModifierFlags::Command {
        match key.as_str() {
            "c" => Some(sel!(copy:)),
            "v" => Some(sel!(paste:)),
            "x" => Some(sel!(cut:)),
            "a" => Some(sel!(selectAll:)),
            "z" => Some(sel!(undo:)),
            _ => None,
        }
    } else if mods == NSEventModifierFlags::Command | NSEventModifierFlags::Shift && key == "z" {
        Some(sel!(redo:))
    } else {
        None
    }
}

fn key_equivalent(panel: &NotesPanel, event: &NSEvent) -> bool {
    if handle_shortcut(panel, event) {
        return true;
    }
    let handled: bool = unsafe { msg_send![super(panel), performKeyEquivalent: event] };
    if handled {
        return true;
    }
    match (edit_action(event), MainThreadMarker::new()) {
        (Some(action), Some(mtm)) => unsafe { NSApplication::sharedApplication(mtm).sendAction_to_from(action, None, None) },
        _ => false,
    }
}

// ---------------------------------------------------------------- AppKit classes

define_class!(
    /// Titled floating panel that can take keyboard focus.
    #[unsafe(super(NSPanel))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastNotesPanel"]
    struct NotesPanel;

    impl NotesPanel {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key(&self) -> bool {
            true
        }

        #[unsafe(method(canBecomeMainWindow))]
        fn can_become_main(&self) -> bool {
            true
        }

        #[unsafe(method(performKeyEquivalent:))]
        fn perform_key_equivalent(&self, event: &NSEvent) -> bool {
            key_equivalent(self, event)
        }

        // Esc is not a key equivalent for AppKit: intercept it before it reaches the text view.
        #[unsafe(method(sendEvent:))]
        fn send_event(&self, event: &NSEvent) {
            if event.r#type() == NSEventType::KeyDown && event.keyCode() == KEY_ESCAPE && handle_shortcut(self, event) {
                return;
            }
            unsafe {
                let _: () = msg_send![super(self), sendEvent: event];
            }
        }
    }
);

define_class!(
    /// Text view delegate and toolbar button target.
    #[unsafe(super(objc2_foundation::NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastNotesTarget"]
    struct NotesTarget;

    impl NotesTarget {
        #[unsafe(method(notesNew:))]
        fn notes_new(&self, _sender: Option<&AnyObject>) {
            fire_event(Event::New);
        }

        #[unsafe(method(notesBrowse:))]
        fn notes_browse(&self, _sender: Option<&AnyObject>) {
            fire_event(Event::Browse);
        }
    }

    unsafe impl NSObjectProtocol for NotesTarget {}

    unsafe impl NSTextDelegate for NotesTarget {
        #[unsafe(method(textDidChange:))]
        fn text_did_change(&self, _n: &NSNotification) {
            if SUPPRESS.get() {
                return;
            }
            let text = text();
            refresh_title(&text);
            let cb = ON_CHANGE.with(|c| c.borrow().clone());
            if let Some(cb) = cb {
                cb(text);
            }
        }
    }

    unsafe impl NSTextViewDelegate for NotesTarget {}
);

// ---------------------------------------------------------------- build

fn icon_button(
    mtm: MainThreadMarker,
    symbol: &str,
    tip: &str,
    target: &NotesTarget,
    action: objc2::runtime::Sel,
    x: f64,
) -> Retained<NSButton> {
    let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(&NSString::from_str(symbol), Some(&NSString::from_str(tip)));
    let b = unsafe { NSButton::buttonWithImage_target_action(image.as_deref().expect("SF Symbol"), Some(target), Some(action), mtm) };
    b.setBordered(false);
    b.setToolTip(Some(&NSString::from_str(tip)));
    b.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
    b.setFrame(rect(x, (TOOLBAR_H - 28.0) / 2.0, 28.0, 28.0));
    b.setAutoresizingMask(NSAutoresizingMaskOptions::ViewMinXMargin);
    b
}

fn build(mtm: MainThreadMarker) -> Ui {
    let frame = rect(0.0, 0.0, DEFAULT_W, DEFAULT_H);
    let style = NSWindowStyleMask::Titled | NSWindowStyleMask::Resizable | NSWindowStyleMask::FullSizeContentView;
    let panel: Retained<NotesPanel> = unsafe {
        msg_send![
            super(mtm.alloc::<NotesPanel>().set_ivars(())),
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
    panel.setBecomesKeyOnlyIfNeeded(false);
    panel.setMovableByWindowBackground(true);
    panel.setTitlebarAppearsTransparent(true);
    panel.setTitleVisibility(NSWindowTitleVisibility::Hidden);
    panel.setMinSize(NSSize::new(MIN_W, MIN_H));
    panel.setCollectionBehavior(NSWindowCollectionBehavior::CanJoinAllSpaces | NSWindowCollectionBehavior::FullScreenAuxiliary);
    for kind in [NSWindowButton::CloseButton, NSWindowButton::MiniaturizeButton, NSWindowButton::ZoomButton] {
        if let Some(b) = panel.standardWindowButton(kind) {
            b.setHidden(true);
        }
    }

    let target: Retained<NotesTarget> = unsafe { msg_send![mtm.alloc::<NotesTarget>(), init] };

    // Content: translucent window background, scroll view, toolbar on top.
    let content = NSVisualEffectView::initWithFrame(mtm.alloc::<NSVisualEffectView>(), frame);
    content.setMaterial(NSVisualEffectMaterial::WindowBackground);
    content.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
    content.setState(NSVisualEffectState::FollowsWindowActiveState);
    panel.setContentView(Some(&content));
    let all = NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable;

    let scroll = NSScrollView::initWithFrame(mtm.alloc::<NSScrollView>(), frame);
    scroll.setHasVerticalScroller(true);
    scroll.setDrawsBackground(false);
    scroll.setBorderType(objc2_app_kit::NSBorderType::NoBorder);
    scroll.setAutohidesScrollers(true);
    scroll.setAutoresizingMask(all);
    scroll.setAutomaticallyAdjustsContentInsets(false);
    scroll.setContentInsets(NSEdgeInsets { top: TOOLBAR_H, left: 0.0, bottom: 0.0, right: 0.0 });
    scroll.setScrollerInsets(NSEdgeInsets { top: 0.0, left: 0.0, bottom: 0.0, right: 0.0 });

    let text_view = NSTextView::initWithFrame(mtm.alloc::<NSTextView>(), rect(0.0, 0.0, DEFAULT_W, DEFAULT_H));
    text_view.setMinSize(NSSize::new(0.0, 0.0));
    text_view.setMaxSize(NSSize::new(f64::MAX, f64::MAX));
    text_view.setVerticallyResizable(true);
    text_view.setHorizontallyResizable(false);
    text_view.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    if let Some(tc) = unsafe { text_view.textContainer() } {
        tc.setWidthTracksTextView(true);
        tc.setContainerSize(NSSize::new(DEFAULT_W, f64::MAX));
    }
    text_view.setRichText(true);
    text_view.setAllowsUndo(true);
    text_view.setDrawsBackground(false);
    text_view.setFont(Some(&NSFont::systemFontOfSize(FONT_SIZE)));
    text_view.setTextColor(Some(&NSColor::textColor()));
    text_view.setTextContainerInset(NSSize::new(INSET, INSET));
    text_view.setAutomaticQuoteSubstitutionEnabled(false);
    text_view.setAutomaticDashSubstitutionEnabled(false);
    text_view.setAutomaticTextReplacementEnabled(false);
    text_view.setAutomaticLinkDetectionEnabled(false);
    let delegate: &ProtocolObject<dyn NSTextViewDelegate> = ProtocolObject::from_ref(&*target);
    text_view.setDelegate(Some(delegate));
    scroll.setDocumentView(Some(&text_view));
    content.addSubview(&scroll);

    // Toolbar strip (blurs the text scrolling under it).
    let toolbar =
        NSVisualEffectView::initWithFrame(mtm.alloc::<NSVisualEffectView>(), rect(0.0, DEFAULT_H - TOOLBAR_H, DEFAULT_W, TOOLBAR_H));
    toolbar.setMaterial(NSVisualEffectMaterial::WindowBackground);
    toolbar.setBlendingMode(NSVisualEffectBlendingMode::WithinWindow);
    toolbar.setState(NSVisualEffectState::FollowsWindowActiveState);
    toolbar.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewMinYMargin);

    let title = NSTextField::labelWithString(&NSString::from_str("Untitled"), mtm);
    title.setFont(Some(&NSFont::systemFontOfSize_weight(13.0, unsafe { NSFontWeightSemibold })));
    title.setTextColor(Some(&NSColor::textColor()));
    title.setAlignment(NSTextAlignment::Center);
    title.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    title.setFrame(rect(80.0, (TOOLBAR_H - 17.0) / 2.0, DEFAULT_W - 160.0, 17.0));
    title.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    toolbar.addSubview(&title);

    let browse = icon_button(mtm, "list.bullet", "Browse Notes ⌘P", &target, sel!(notesBrowse:), DEFAULT_W - 76.0);
    let new = icon_button(mtm, "plus", "New Note ⌘N", &target, sel!(notesNew:), DEFAULT_W - 44.0);
    toolbar.addSubview(&browse);
    toolbar.addSubview(&new);
    content.addSubview(&toolbar);

    let name = NSString::from_str(FRAME_NAME);
    if !panel.setFrameUsingName(&name) {
        panel.center();
    }
    panel.setFrameAutosaveName(&name);

    Ui { panel, text_view, title, _target: target }
}

/// Retained handles to the window parts, creating the window on first use.
fn parts() -> Option<(Retained<NotesPanel>, Retained<NSTextView>)> {
    let mtm = MainThreadMarker::new()?;
    UI.with(|u| {
        let mut u = u.borrow_mut();
        let ui = u.get_or_insert_with(|| build(mtm));
        Some((ui.panel.clone(), ui.text_view.clone()))
    })
}

// ---------------------------------------------------------------- public API

/// Show the window, activate the app and make the window key with the editor focused.
pub fn show() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let Some((panel, _)) = parts() else { return };
    let app = NSApplication::sharedApplication(mtm);
    if app.isHidden() {
        app.unhideWithoutActivation();
    }
    panel.makeKeyAndOrderFront(None);
    app.activate();
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);
    focus_editor();
}

/// Hide the window (the note text is kept).
pub fn hide() {
    UI.with(|u| {
        if let Some(ui) = u.borrow().as_ref() {
            ui.panel.orderOut(None);
        }
    });
}

/// Show when hidden or not key-focused-visible, hide when visible.
pub fn toggle() {
    if is_visible() {
        hide();
    } else {
        show();
    }
}

pub fn is_visible() -> bool {
    UI.with(|u| u.borrow().as_ref().is_some_and(|u| u.panel.isVisible()))
}

pub fn is_key() -> bool {
    UI.with(|u| u.borrow().as_ref().is_some_and(|u| u.panel.isKeyWindow()))
}

/// Make the text view the first responder.
pub fn focus_editor() {
    let Some((panel, tv)) = parts() else { return };
    let _ = panel.makeFirstResponder(Some(&tv));
}

/// Replace the text without firing the change callback; resets undo.
pub fn set_text(text: &str) {
    let Some((_, tv)) = parts() else { return };
    SUPPRESS.set(true);
    tv.setString(&NSString::from_str(text));
    // setString on a rich-text view falls back to the default attributes
    tv.setFont(Some(&NSFont::systemFontOfSize(FONT_SIZE)));
    tv.setTextColor(Some(&NSColor::textColor()));
    if let Some(um) = tv.undoManager() {
        um.removeAllActions();
    }
    SUPPRESS.set(false);
    refresh_title(text);
}

/// Current editor text.
pub fn text() -> String {
    UI.with(|u| u.borrow().as_ref().map(|u| u.text_view.string().to_string())).unwrap_or_default()
}

/// Title as shown in the toolbar (S5).
pub fn title() -> String {
    title_from_text(&text())
}

/// Called on every user edit with the full text.
pub fn set_on_change(cb: Box<dyn Fn(String)>) {
    ON_CHANGE.with(|c| *c.borrow_mut() = Some(Rc::from(cb)));
}

/// Called when the New or Browse toolbar items / shortcuts fire.
pub fn set_on_event(cb: Box<dyn Fn(Event)>) {
    ON_EVENT.with(|c| *c.borrow_mut() = Some(Rc::from(cb)));
}

/// false: Esc hides the window. true: Esc only gives up key status.
pub fn set_escape_loses_focus(on: bool) {
    ESC_LOSES_FOCUS.set(on);
}

/// Render the whole window content view to a PNG file; false on any failure.
pub fn snapshot_png(path: &Path) -> bool {
    let Some((panel, _)) = parts() else { return false };
    let Some(view) = panel.contentView() else { return false };
    let view: &NSView = &view;
    let bounds = view.bounds();
    let Some(rep) = view.bitmapImageRepForCachingDisplayInRect(bounds) else { return false };
    view.cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
    let Some(data) = (unsafe { rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new()) }) else {
        return false;
    };
    data.writeToFile_atomically(&NSString::from_str(&path.to_string_lossy()), true)
}

#[cfg(test)]
mod tests {
    use super::title_from_text;

    #[test]
    fn empty_is_untitled() {
        assert_eq!(title_from_text(""), "Untitled");
        assert_eq!(title_from_text("  \n\t\n"), "Untitled");
        assert_eq!(title_from_text("###\n>\n- "), "Untitled");
    }

    #[test]
    fn strips_markers_and_spaces() {
        assert_eq!(title_from_text("# Groceries\nmilk"), "Groceries");
        assert_eq!(title_from_text("  ## > - * Deep"), "Deep");
        assert_eq!(title_from_text("- item"), "item");
    }

    #[test]
    fn first_non_blank_line_wins() {
        assert_eq!(title_from_text("\n\n  \nHello\nWorld"), "Hello");
        assert_eq!(title_from_text("#\nReal title"), "Real title");
    }

    #[test]
    fn keeps_inner_markers() {
        assert_eq!(title_from_text("a - b # c"), "a - b # c");
    }

    #[test]
    fn caps_at_60_chars() {
        let long = "x".repeat(100);
        assert_eq!(title_from_text(&long).chars().count(), 60);
        let wide = "é".repeat(80);
        assert_eq!(title_from_text(&wide).chars().count(), 60);
    }
}
