//! Notes window shell (S2, S5, S19): a floating native `NSPanel` holding the rich-text editor
//! (`notes_editor`) in a scroll view, with a slim toolbar (live title, Browse Notes, New Note).
//! The text is Markdown (`set_text` / `text` are aliases of `set_markdown` / `markdown`).
//! All AppKit calls happen on the main thread; the title logic is a plain function with unit
//! tests below.

use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{define_class, msg_send, sel, MainThreadMarker, MainThreadOnly};

pub use crate::notes_editor::{
    active_formats, append_markdown, apply_format, emoji_commit, insert_at_cursor, insert_markdown_after_selection_line,
    link_prompt_cancel, link_prompt_open, link_prompt_submit, link_prompt_value, markdown, markdown_of_selection, paragraph_start,
    replace_range, replace_selection_markdown, select_range, selection, selection_range, set_markdown, set_on_emoji, set_on_slash,
    set_on_zoom_change, set_popup_key_handler, set_zoom, slash_commit, text_in_range, toggle_task_at_cursor, window_point_for_char, zoom,
    Anchor, EmojiEvent, PopupKey, SlashEvent,
};
use crate::notes_format::Format;
use crate::notes_popups::{emoji_items, slash_items, FormatBar, Item, Rect as PopupRect, SuggestionPopup};
use objc2_app_kit::{
    NSApplication, NSAutoresizingMaskOptions, NSBackingStoreType, NSBitmapImageFileType, NSButton, NSColor, NSEvent, NSEventModifierFlags,
    NSEventType, NSFloatingWindowLevel, NSFont, NSFontWeightSemibold, NSImage, NSLineBreakMode, NSPanel, NSScrollView, NSTextAlignment,
    NSTextDelegate, NSTextField, NSTextView, NSTextViewDelegate, NSView, NSVisualEffectBlendingMode, NSVisualEffectMaterial,
    NSVisualEffectState, NSVisualEffectView, NSWindowButton, NSWindowCollectionBehavior, NSWindowDelegate, NSWindowStyleMask,
    NSWindowTitleVisibility,
};
use objc2_foundation::{NSDictionary, NSEdgeInsets, NSNotification, NSObjectProtocol, NSPoint, NSRange, NSRect, NSSize, NSString};

/// Toolbar actions the host reacts to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    New,
    Browse,
    /// ⌘K: open the action panel.
    Actions,
    /// ⌘[: back to the previously opened note.
    Back,
    /// ⌘]: forward again.
    Forward,
    /// ⇧⌘P: pin or unpin the current note.
    TogglePin,
    /// ⌘1 to ⌘9: open the nth pinned note.
    Pinned(u8),
    /// The window became visible.
    Shown,
    /// The window was hidden (⌘W, Esc, toggle, `hide()`).
    Hidden,
}

const TOOLBAR_H: f64 = 44.0;
const DEFAULT_W: f64 = 560.0;
const DEFAULT_H: f64 = 420.0;
const MIN_W: f64 = 320.0;
const MIN_H: f64 = 220.0;
const INSET: f64 = 16.0;
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
    /// The editor scroll view; its bottom edge moves up when the Format Bar is visible.
    scroll: Retained<NSScrollView>,
    bar: Rc<FormatBar>,
    /// Horizontal scroller-less container of the Format Bar (the bar is wider than the window minimum).
    bar_scroll: Retained<NSScrollView>,
    /// Kept alive: the text view delegate and button target are weak references.
    _target: Retained<NotesTarget>,
}

type ChangeCb = Rc<dyn Fn(String)>;
type EventCb = Rc<dyn Fn(Event)>;
/// `(emoji, name)` pairs.
pub type EmojiPairs = Vec<(String, String)>;
type EmojiProvider = Rc<dyn Fn(&str) -> EmojiPairs>;

/// The two suggestion popups, created on first use and reused.
#[derive(Default)]
struct Popups {
    slash: Option<Rc<SuggestionPopup>>,
    /// Format of every row currently listed in the `/` popup.
    slash_formats: Vec<Format>,
    slash_anchor: Option<PopupRect>,
    emoji: Option<Rc<SuggestionPopup>>,
    /// Emoji of every row currently listed in the `:` popup.
    emoji_glyphs: Vec<String>,
    emoji_anchor: Option<PopupRect>,
}

thread_local! {
    static UI: RefCell<Option<Ui>> = const { RefCell::new(None) };
    static ON_CHANGE: RefCell<Option<ChangeCb>> = const { RefCell::new(None) };
    static ON_EVENT: RefCell<Option<EventCb>> = const { RefCell::new(None) };
    static ESC_LOSES_FOCUS: Cell<bool> = const { Cell::new(false) };
    static POPUPS: RefCell<Popups> = RefCell::new(Popups::default());
    static EMOJI_PROVIDER: RefCell<Option<EmojiProvider>> = const { RefCell::new(None) };
    static BAR_VISIBLE: Cell<bool> = const { Cell::new(false) };
    /// Last visibility reported to the host, so `Shown` / `Hidden` fire once per transition.
    static LAST_VISIBLE: Cell<bool> = const { Cell::new(false) };
    /// Keeps the local key monitor (⌘K while the link prompt owns the keyboard) alive.
    static KEY_MONITOR: RefCell<Option<Retained<AnyObject>>> = const { RefCell::new(None) };
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

pub(crate) fn refresh_title(title: &str) {
    let label = UI.with(|u| u.borrow().as_ref().map(|u| u.title.clone()));
    if let Some(label) = label {
        label.setStringValue(&NSString::from_str(title));
    }
}

/// Forward a user edit (full Markdown) to the host callback.
pub(crate) fn notify_change(text: String) {
    let cb = ON_CHANGE.with(|c| c.borrow().clone());
    if let Some(cb) = cb {
        cb(text);
    }
}

/// The editor view. Creates the hidden window on first use, so content set before the window
/// is first shown is not lost.
pub(crate) fn text_view() -> Option<Retained<NSTextView>> {
    parts().map(|(_, tv)| tv)
}

/// The window content view (the overlay host), creating the hidden window on first use.
/// Only the palette asks for it, when it opens: the suggestion popups give way to it.
pub(crate) fn content_view() -> Option<Retained<NSView>> {
    let (panel, _) = parts()?;
    dismiss_popups();
    panel.contentView()
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

fn editor_has_focus(panel: &NotesPanel) -> bool {
    let Some(tv) = text_view() else { return false };
    crate::notes_editor::is_editor(panel.firstResponder().as_deref(), &tv)
}

/// Handle the window-level shortcuts; true when the event was consumed.
fn handle_shortcut(panel: &NotesPanel, event: &NSEvent) -> bool {
    if event.r#type() != NSEventType::KeyDown {
        return false;
    }
    let mods = event.modifierFlags()
        & (NSEventModifierFlags::Command | NSEventModifierFlags::Shift | NSEventModifierFlags::Option | NSEventModifierFlags::Control);
    if mods.contains(NSEventModifierFlags::Command) && editor_has_focus(panel) && crate::notes_editor::handle_key(event) {
        return true;
    }
    if mods.is_empty() && event.keyCode() == KEY_ESCAPE {
        if ESC_LOSES_FOCUS.get() {
            resign_key(panel);
        } else {
            panel.orderOut(None);
            sync_visibility();
        }
        return true;
    }
    let key = event.charactersIgnoringModifiers().map(|s| s.to_string().to_lowercase()).unwrap_or_default();
    if mods == NSEventModifierFlags::Command | NSEventModifierFlags::Shift {
        if key == "p" {
            fire_event(Event::TogglePin);
            return true;
        }
        return false;
    }
    if mods != NSEventModifierFlags::Command {
        return false;
    }
    match key.as_str() {
        "n" => fire_event(Event::New),
        "p" => fire_event(Event::Browse),
        "w" => {
            panel.orderOut(None);
            sync_visibility();
        }
        "k" => action_panel_requested(),
        "[" => fire_event(Event::Back),
        "]" => fire_event(Event::Forward),
        d if d.len() == 1 && matches!(d.as_bytes()[0], b'1'..=b'9') => fire_event(Event::Pinned(d.as_bytes()[0] - b'0')),
        _ => return false,
    }
    true
}

/// ⌘K: close whatever is open on top of the editor (pop-ups, link prompt), then ask the host
/// for the action panel.
fn action_panel_requested() {
    crate::notes_editor::link_prompt_cancel();
    dismiss_popups();
    fire_event(Event::Actions);
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
    if crate::notes_palette::is_visible() {
        // The palette owns the keyboard: only its own keys and the edit shortcuts of its
        // search field act; every other command shortcut is swallowed, never edits the note.
        if crate::notes_palette::handle_key(event) {
            return true;
        }
        if event.r#type() != NSEventType::KeyDown || !event.modifierFlags().contains(NSEventModifierFlags::Command) {
            return false;
        }
        let handled: bool = unsafe { msg_send![super(panel), performKeyEquivalent: event] };
        if !handled {
            if let (Some(action), Some(mtm)) = (edit_action(event), MainThreadMarker::new()) {
                unsafe { NSApplication::sharedApplication(mtm).sendAction_to_from(action, None, None) };
            }
        }
        return true;
    }
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
            if event.r#type() == NSEventType::KeyDown && crate::notes_palette::is_visible() {
                if crate::notes_palette::handle_key(event) {
                    return;
                }
            } else if event.r#type() == NSEventType::KeyDown {
                if editor_has_focus(self) && crate::notes_editor::handle_popup_key(event) {
                    return;
                }
                if event.keyCode() == KEY_ESCAPE && handle_shortcut(self, event) {
                    return;
                }
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

    unsafe impl NSWindowDelegate for NotesTarget {
        #[unsafe(method(windowDidResize:))]
        fn window_did_resize(&self, _n: &NSNotification) {
            layout_bar();
        }

        #[unsafe(method(windowDidResignKey:))]
        fn window_did_resign_key(&self, _n: &NSNotification) {
            dismiss_popups();
        }
    }

    unsafe impl NSTextDelegate for NotesTarget {
        #[unsafe(method(textDidChange:))]
        fn text_did_change(&self, _n: &NSNotification) {
            crate::notes_editor::did_change();
        }
    }

    unsafe impl NSTextViewDelegate for NotesTarget {
        #[unsafe(method(textView:shouldChangeTextInRange:replacementString:))]
        fn text_view_should_change(&self, _tv: &NSTextView, range: NSRange, replacement: Option<&NSString>) -> bool {
            crate::notes_editor::should_change(range, replacement)
        }

        #[unsafe(method(textView:doCommandBySelector:))]
        unsafe fn text_view_do_command(&self, _tv: &NSTextView, command: objc2::runtime::Sel) -> bool {
            crate::notes_editor::do_command(command)
        }

        #[unsafe(method(textView:willChangeSelectionFromCharacterRange:toCharacterRange:))]
        fn text_view_will_change_selection(&self, _tv: &NSTextView, old: NSRange, new: NSRange) -> NSRange {
            crate::notes_editor::will_change_selection(old, new)
        }

        #[unsafe(method(textViewDidChangeSelection:))]
        fn text_view_did_change_selection(&self, _n: &NSNotification) {
            crate::notes_editor::did_change_selection();
        }
    }
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

    let text_view = crate::notes_editor::new_text_view(mtm, rect(0.0, 0.0, DEFAULT_W, DEFAULT_H));
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
    text_view.setTextContainerInset(NSSize::new(INSET, INSET));
    text_view.setAutomaticQuoteSubstitutionEnabled(false);
    text_view.setAutomaticDashSubstitutionEnabled(false);
    text_view.setAutomaticTextReplacementEnabled(false);
    text_view.setAutomaticLinkDetectionEnabled(false);
    let delegate: &ProtocolObject<dyn NSTextViewDelegate> = ProtocolObject::from_ref(&*target);
    text_view.setDelegate(Some(delegate));
    scroll.setDocumentView(Some(&text_view));
    content.addSubview(&scroll);

    // Format Bar: a scroller-less horizontal scroll view at the bottom, hidden by default.
    let bar = Rc::new(FormatBar::new());
    bar.set_on_format(Box::new(|f| {
        crate::notes_editor::apply_format(f);
        if f != Format::Link {
            focus_editor();
        }
    }));
    let bar_doc = bar.view();
    bar_doc.setAutoresizingMask(NSAutoresizingMaskOptions::empty());
    bar_doc.setFrame(rect(0.0, 0.0, bar.content_width().max(DEFAULT_W), FormatBar::HEIGHT));
    let bar_scroll = NSScrollView::initWithFrame(mtm.alloc::<NSScrollView>(), rect(0.0, 0.0, DEFAULT_W, FormatBar::HEIGHT));
    bar_scroll.setHasHorizontalScroller(false);
    bar_scroll.setHasVerticalScroller(false);
    bar_scroll.setDrawsBackground(false);
    bar_scroll.setBorderType(objc2_app_kit::NSBorderType::NoBorder);
    bar_scroll.setVerticalScrollElasticity(objc2_app_kit::NSScrollElasticity::None);
    bar_scroll.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    bar_scroll.setDocumentView(Some(&bar_doc));
    bar_scroll.setHidden(true);
    content.addSubview(&bar_scroll);

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
    let window_delegate: &ProtocolObject<dyn NSWindowDelegate> = ProtocolObject::from_ref(&*target);
    panel.setDelegate(Some(window_delegate));
    install_popup_hooks();
    install_key_monitor();

    Ui { panel, text_view, title, scroll, bar, bar_scroll, _target: target }
}

/// Retained handles to the window parts, creating the window on first use.
fn parts() -> Option<(Retained<NotesPanel>, Retained<NSTextView>)> {
    let mtm = MainThreadMarker::new()?;
    UI.with(|u| {
        // A nested call during the build gets nothing instead of a borrow panic.
        let mut u = u.try_borrow_mut().ok()?;
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
    sync_visibility();
}

/// Hide the window (the note text is kept).
pub fn hide() {
    let panel = UI.with(|u| u.borrow().as_ref().map(|ui| ui.panel.clone()));
    if let Some(panel) = panel {
        panel.orderOut(None);
        sync_visibility();
    }
}

/// Fire `Shown` / `Hidden` when the window visibility changed since the last report. Hiding
/// also closes the suggestion popups. The state is updated before the callback runs, so the
/// host may call `show` / `hide` from inside it.
fn sync_visibility() {
    let now = is_visible();
    if LAST_VISIBLE.replace(now) == now {
        return;
    }
    if !now {
        dismiss_popups();
    }
    fire_event(if now { Event::Shown } else { Event::Hidden });
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

/// Alias of [`set_markdown`]: replace the note with Markdown without firing the change
/// callback; resets undo.
pub fn set_text(text: &str) {
    set_markdown(text);
}

/// Alias of [`markdown`]: the note serialized as Markdown.
pub fn text() -> String {
    markdown()
}

/// Title as shown in the toolbar (S5): `Doc::title()` of the note, "Untitled" when empty.
pub fn title() -> String {
    crate::notes_editor::title()
}

/// Called on every user edit with the full text.
pub fn set_on_change(cb: Box<dyn Fn(String)>) {
    ON_CHANGE.with(|c| *c.borrow_mut() = Some(Rc::from(cb)));
}

/// Called when a toolbar item or window shortcut fires (see [`Event`]). The callback may call
/// back into this module (`show`, `hide`, `set_markdown`, the palette): no borrow is held while it runs.
pub fn set_on_event(cb: Box<dyn Fn(Event)>) {
    ON_EVENT.with(|c| *c.borrow_mut() = Some(Rc::from(cb)));
}

/// false: Esc hides the window. true: Esc only gives up key status.
pub fn set_escape_loses_focus(on: bool) {
    ESC_LOSES_FOCUS.set(on);
}

// ---------------------------------------------------------------- Format Bar

/// Show or hide the Format Bar at the bottom of the window (hidden by default). The editor
/// scroll view shrinks by the bar height while it is visible.
pub fn set_format_bar_visible(visible: bool) {
    if parts().is_none() {
        return;
    }
    BAR_VISIBLE.set(visible);
    layout_bar();
    refresh_format_bar();
}

pub fn format_bar_visible() -> bool {
    BAR_VISIBLE.get()
}

/// Content width of the Format Bar and the width of its visible part. The bar scrolls
/// horizontally when the first exceeds the second (narrow windows).
pub fn format_bar_metrics() -> Option<(f64, f64)> {
    UI.with(|u| u.borrow().as_ref().map(|ui| (ui.bar.content_width(), ui.bar_scroll.contentView().bounds().size.width)))
}

/// Scroll the Format Bar horizontally to `x` (clamped); returns the offset reached.
pub fn format_bar_scroll_to(x: f64) -> f64 {
    let Some(bar_scroll) = UI.with(|u| u.borrow().as_ref().map(|ui| ui.bar_scroll.clone())) else { return 0.0 };
    let clip = bar_scroll.contentView();
    let doc_w = bar_scroll.documentView().map_or(0.0, |d| d.frame().size.width);
    let max = (doc_w - clip.bounds().size.width).max(0.0);
    clip.scrollToPoint(NSPoint::new(x.clamp(0.0, max), 0.0));
    bar_scroll.reflectScrolledClipView(&clip);
    clip.bounds().origin.x
}

/// Place the bar container and the editor scroll view for the current window size and bar state.
fn layout_bar() {
    let parts = UI.with(|u| u.borrow().as_ref().map(|ui| (ui.panel.clone(), ui.scroll.clone(), ui.bar.clone(), ui.bar_scroll.clone())));
    let Some((panel, scroll, bar, bar_scroll)) = parts else { return };
    let Some(content) = panel.contentView() else { return };
    let b = content.bounds();
    let visible = BAR_VISIBLE.get();
    let bar_h = if visible { FormatBar::HEIGHT } else { 0.0 };
    bar_scroll.setHidden(!visible);
    bar_scroll.setFrame(rect(0.0, 0.0, b.size.width, FormatBar::HEIGHT));
    if let Some(doc) = bar_scroll.documentView() {
        doc.setFrame(rect(0.0, 0.0, bar.content_width().max(b.size.width), FormatBar::HEIGHT));
    }
    scroll.setFrame(rect(0.0, bar_h, b.size.width, (b.size.height - bar_h).max(0.0)));
}

/// Push the formats at the cursor / selection to the bar; a no-op while it is hidden.
pub(crate) fn refresh_format_bar() {
    if !BAR_VISIBLE.get() {
        return;
    }
    let bar = UI.with(|u| u.try_borrow().ok().and_then(|u| u.as_ref().map(|ui| ui.bar.clone())));
    if let Some(bar) = bar {
        bar.set_active(&crate::notes_editor::active_formats());
    }
}

// ---------------------------------------------------------------- suggestion popups

fn to_popup_rect(a: Anchor) -> PopupRect {
    PopupRect { x: a.x, y: a.y, w: a.w, h: a.h }
}

fn make_popup(on_pick: fn(usize)) -> Rc<SuggestionPopup> {
    let p = Rc::new(SuggestionPopup::new());
    p.set_on_pick(Box::new(on_pick));
    p
}

/// The `/` menu popup, once it has been opened.
pub fn slash_popup() -> Option<Rc<SuggestionPopup>> {
    POPUPS.with(|p| p.borrow().slash.clone())
}

/// The `:` emoji popup, once it has been opened.
pub fn emoji_popup() -> Option<Rc<SuggestionPopup>> {
    POPUPS.with(|p| p.borrow().emoji.clone())
}

/// Supply the emoji data of the `:` picker: `(emoji, name)` pairs for a query. Without a
/// provider the emoji popup never opens.
pub fn set_emoji_provider(provider: Box<dyn Fn(&str) -> EmojiPairs>) {
    EMOJI_PROVIDER.with(|p| *p.borrow_mut() = Some(Rc::from(provider)));
}

/// Hide both popups and end the open `/` and `:` sessions (the typed text stays).
pub fn dismiss_popups() {
    let (slash, emoji) = POPUPS.with(|p| {
        let mut p = p.borrow_mut();
        p.slash_anchor = None;
        p.emoji_anchor = None;
        (p.slash.clone(), p.emoji.clone())
    });
    let open = slash.as_ref().is_some_and(|p| p.is_visible()) || emoji.as_ref().is_some_and(|p| p.is_visible());
    for p in slash.iter().chain(emoji.iter()) {
        p.hide();
    }
    if open {
        crate::notes_editor::end_trigger_sessions();
    }
}

fn hide_slash() {
    let p = POPUPS.with(|p| {
        let mut p = p.borrow_mut();
        p.slash_anchor = None;
        p.slash.clone()
    });
    if let Some(p) = p {
        p.hide();
    }
}

fn hide_emoji() {
    let p = POPUPS.with(|p| {
        let mut p = p.borrow_mut();
        p.emoji_anchor = None;
        p.emoji.clone()
    });
    if let Some(p) = p {
        p.hide();
    }
}

fn pick_slash(idx: usize) {
    let format = POPUPS.with(|p| p.borrow().slash_formats.get(idx).copied());
    hide_slash();
    if let Some(f) = format {
        crate::notes_editor::slash_commit(f);
    }
}

fn pick_emoji(idx: usize) {
    let glyph = POPUPS.with(|p| p.borrow().emoji_glyphs.get(idx).cloned());
    hide_emoji();
    if let Some(g) = glyph {
        crate::notes_editor::emoji_commit(&g);
    }
}

fn show_slash(query: &str) {
    let Some(anchor) = POPUPS.with(|p| p.borrow().slash_anchor) else { return };
    let rows = slash_items(query);
    let popup = match slash_popup() {
        Some(p) => p,
        None => {
            let p = make_popup(pick_slash);
            POPUPS.with(|s| s.borrow_mut().slash = Some(p.clone()));
            p
        }
    };
    let (formats, items): (Vec<Format>, Vec<Item>) = rows.into_iter().unzip();
    POPUPS.with(|p| p.borrow_mut().slash_formats = formats);
    popup.show(anchor, items);
}

fn show_emoji(query: &str) {
    let Some(anchor) = POPUPS.with(|p| p.borrow().emoji_anchor) else { return };
    let Some(provider) = EMOJI_PROVIDER.with(|p| p.borrow().clone()) else { return };
    let items = emoji_items(query, &*provider);
    let popup = match emoji_popup() {
        Some(p) => p,
        None => {
            let p = make_popup(pick_emoji);
            POPUPS.with(|s| s.borrow_mut().emoji = Some(p.clone()));
            p
        }
    };
    let glyphs = items.iter().map(|i| i.glyph.clone()).collect();
    POPUPS.with(|p| p.borrow_mut().emoji_glyphs = glyphs);
    popup.show(anchor, items);
}

fn popups_allowed() -> bool {
    is_visible() && !crate::notes_palette::is_visible()
}

fn on_slash(e: SlashEvent) {
    match e {
        SlashEvent::Open { anchor } => {
            if !popups_allowed() {
                return;
            }
            POPUPS.with(|p| p.borrow_mut().slash_anchor = Some(to_popup_rect(anchor)));
            show_slash("");
        }
        SlashEvent::Query(q) => show_slash(&q),
        SlashEvent::Close => hide_slash(),
    }
}

fn on_emoji(e: EmojiEvent) {
    match e {
        EmojiEvent::Open { anchor } => {
            if !popups_allowed() || EMOJI_PROVIDER.with(|p| p.borrow().is_none()) {
                return;
            }
            POPUPS.with(|p| p.borrow_mut().emoji_anchor = Some(to_popup_rect(anchor)));
        }
        EmojiEvent::Query(q) => show_emoji(&q),
        EmojiEvent::Close => hide_emoji(),
    }
}

/// Up / Down / Enter / Tab drive the visible popup; Esc is left to the editor, which closes
/// the session (and with it the popup) without hiding the window.
fn on_popup_key(key: PopupKey) -> bool {
    if crate::notes_palette::is_visible() {
        dismiss_popups();
        return false;
    }
    let slash = slash_popup().filter(|p| p.is_visible());
    let (popup, is_slash) = match (slash, emoji_popup().filter(|p| p.is_visible())) {
        (Some(p), _) => (p, true),
        (None, Some(p)) => (p, false),
        (None, None) => return false,
    };
    match key {
        PopupKey::Up => popup.move_selection(-1),
        PopupKey::Down => popup.move_selection(1),
        PopupKey::Enter | PopupKey::Tab => {
            if let Some(i) = popup.selected() {
                if is_slash {
                    pick_slash(i);
                } else {
                    pick_emoji(i);
                }
            }
        }
        PopupKey::Esc => return false,
    }
    true
}

/// Route the editor's `/` and `:` sessions and the popup keys to the suggestion popups. Done
/// when the window is built; call it again after a host replaced the editor hooks.
pub fn install_popup_hooks() {
    crate::notes_editor::set_on_slash(Box::new(on_slash));
    crate::notes_editor::set_on_emoji(Box::new(on_emoji));
    crate::notes_editor::set_popup_key_handler(Box::new(on_popup_key));
}

// ---------------------------------------------------------------- key monitor

/// The link prompt is a popover window that takes the keyboard, so ⌘K never reaches the Notes
/// panel while it is open: a local monitor turns it into the usual `Actions` request.
fn install_key_monitor() {
    if KEY_MONITOR.with(|m| m.borrow().is_some()) {
        return;
    }
    let block = block2::RcBlock::new(|ev: std::ptr::NonNull<NSEvent>| -> *mut NSEvent {
        let e = unsafe { ev.as_ref() };
        let cmd_k = e.modifierFlags()
            & (NSEventModifierFlags::Command | NSEventModifierFlags::Shift | NSEventModifierFlags::Option | NSEventModifierFlags::Control)
            == NSEventModifierFlags::Command
            && e.charactersIgnoringModifiers().is_some_and(|s| s.to_string().eq_ignore_ascii_case("k"));
        if cmd_k && crate::notes_editor::link_prompt_open() {
            action_panel_requested();
            return std::ptr::null_mut();
        }
        ev.as_ptr()
    });
    // SAFETY: the block only touches main-thread state and returns the event or null.
    let monitor = unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(objc2_app_kit::NSEventMask::KeyDown, &block) };
    KEY_MONITOR.with(|m| *m.borrow_mut() = monitor);
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
