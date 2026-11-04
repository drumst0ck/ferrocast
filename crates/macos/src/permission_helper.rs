//! Permissions helper (S1): next to the System Settings privacy pane, a small floating panel
//! with the Ferrocast icon that the user drags into the permission list (the "+" list of
//! Accessibility, Screen Recording and Input Monitoring), as Bartender or Rectangle do.
//!
//! The decisions (which pane belongs to which permission, whether to show the helper, when to
//! close it, where to put it, which bundle to drag) are pure functions below; the AppKit part
//! only calls them. AppKit runs on the main thread: [`open_pane_with_helper`] called from any
//! other thread just opens the pane.

use std::path::{Path, PathBuf};

pub const ACCESSIBILITY_PANE: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";
pub const SCREEN_RECORDING_PANE: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture";
pub const INPUT_MONITORING_PANE: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent";
pub const AUTOMATION_PANE: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_Automation";
pub const CAMERA_PANE: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_Camera";
pub const MICROPHONE_PANE: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone";
pub const SPEECH_PANE: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_SpeechRecognition";

/// Bundle id of System Settings.
pub const SETTINGS_BUNDLE_ID: &str = "com.apple.systempreferences";
/// How long the helper waits for System Settings to show up before it gives up.
pub const SETTINGS_GRACE_SECS: f64 = 15.0;
/// Seconds between two checks of the permission.
pub const POLL_SECS: f64 = 1.0;
/// Gap between the System Settings window and the helper.
const GAP: f64 = 12.0;

/// The privacy permissions Ferrocast sends the user to System Settings for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Permission {
    Accessibility,
    ScreenRecording,
    InputMonitoring,
    Automation,
    Camera,
    Microphone,
    SpeechRecognition,
}

impl Permission {
    pub const ALL: [Permission; 7] = [
        Permission::Accessibility,
        Permission::ScreenRecording,
        Permission::InputMonitoring,
        Permission::Automation,
        Permission::Camera,
        Permission::Microphone,
        Permission::SpeechRecognition,
    ];

    /// The System Settings URL of this permission's pane.
    pub fn pane_url(self) -> &'static str {
        match self {
            Permission::Accessibility => ACCESSIBILITY_PANE,
            Permission::ScreenRecording => SCREEN_RECORDING_PANE,
            Permission::InputMonitoring => INPUT_MONITORING_PANE,
            Permission::Automation => AUTOMATION_PANE,
            Permission::Camera => CAMERA_PANE,
            Permission::Microphone => MICROPHONE_PANE,
            Permission::SpeechRecognition => SPEECH_PANE,
        }
    }

    /// The permission a pane URL belongs to.
    pub fn from_pane_url(url: &str) -> Option<Permission> {
        Permission::ALL.into_iter().find(|p| p.pane_url() == url)
    }

    /// Does the pane have a list the user adds apps to by dragging (or "+")? Camera, Microphone,
    /// Speech and Automation only list apps that already asked, so dragging there does nothing.
    pub fn has_drop_list(self) -> bool {
        matches!(self, Permission::Accessibility | Permission::ScreenRecording | Permission::InputMonitoring)
    }
}

/// The `.app` bundle that contains `exe` (`/Applications/Ferrocast.app/Contents/MacOS/ferrocast`),
/// or `None` for a binary outside a bundle (a `cargo run` build).
pub fn bundle_path_from_exe(exe: &Path) -> Option<PathBuf> {
    exe.ancestors().find(|a| a.extension().is_some_and(|e| e == "app")).map(Path::to_path_buf)
}

/// What to do when the app is asked to send the user to a pane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenPlan {
    /// Open the pane only (no list to drop into, or no bundle to drag).
    PaneOnly,
    /// Open the pane and show the helper (replacing a helper for another permission).
    Show,
    /// Open the pane; the helper for this permission is already up: keep it.
    Keep,
}

/// `shown` is the permission of the helper on screen now.
pub fn plan_open(perm: Permission, shown: Option<Permission>, bundle: Option<&Path>) -> OpenPlan {
    if !perm.has_drop_list() || bundle.is_none() {
        OpenPlan::PaneOnly
    } else if shown == Some(perm) {
        OpenPlan::Keep
    } else {
        OpenPlan::Show
    }
}

/// Why the helper goes away.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseReason {
    Granted,
    SettingsClosed,
}

/// Poll decision. `seen_settings` is whether System Settings has been running at some earlier
/// check; `elapsed` the seconds since the helper appeared.
pub fn close_reason(granted: bool, settings_running: bool, seen_settings: bool, elapsed: f64) -> Option<CloseReason> {
    if granted {
        Some(CloseReason::Granted)
    } else if settings_running {
        None
    } else if seen_settings || elapsed > SETTINGS_GRACE_SECS {
        Some(CloseReason::SettingsClosed)
    } else {
        None
    }
}

/// Size `(w, h)` of the helper for a hint `text_w` points wide.
pub fn helper_size(text_w: f64) -> (f64, f64) {
    // padding, icon, gaps and the close button around the hint
    const FIXED: f64 = 110.0;
    ((text_w.max(0.0) + FIXED).clamp(240.0, 420.0), 64.0)
}

/// Cocoa (bottom-left) frame `[x, y, w, h]` of the helper: centred below the System Settings
/// window when it fits on `screen`, else bottom centre of `screen`.
pub fn helper_frame(settings: Option<[f64; 4]>, screen: [f64; 4], size: (f64, f64)) -> [f64; 4] {
    let (w, h) = size;
    let max_x = (screen[0] + screen[2] - w).max(screen[0]);
    if let Some(s) = settings {
        let y = s[1] - h - GAP;
        if y >= screen[1] {
            return [(s[0] + (s[2] - w) / 2.0).clamp(screen[0], max_x), y, w, h];
        }
    }
    [(screen[0] + (screen[2] - w) / 2.0).clamp(screen[0], max_x), screen[1] + 48.0, w, h]
}

// ------------------------------------------------------------------ AppKit

use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameVibrantDark, NSBackingStoreType, NSButton, NSColor, NSDragOperation,
    NSDraggingContext, NSDraggingItem, NSDraggingSession, NSDraggingSource, NSEvent, NSFont, NSImageView, NSPanel, NSPasteboardWriting,
    NSPopUpMenuWindowLevel, NSRunningApplication, NSScreen, NSTextField, NSView, NSWindowCollectionBehavior, NSWindowStyleMask,
    NSWorkspace,
};
use objc2_foundation::{NSArray, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString, NSTimer};

/// The `.app` bundle the running Ferrocast lives in (`None`: a dev build).
pub fn running_bundle() -> Option<PathBuf> {
    bundle_path_from_exe(&std::env::current_exe().ok()?)
}

/// Is the permission granted right now? Silent: never shows a system prompt. Permissions
/// without a drop list are not polled and count as not granted.
pub fn is_granted(perm: Permission) -> bool {
    match perm {
        Permission::Accessibility => crate::windows::accessibility_trusted(false),
        Permission::ScreenRecording => objc2_core_graphics::CGPreflightScreenCaptureAccess(),
        Permission::InputMonitoring => crate::windows::input_monitoring_granted(),
        _ => false,
    }
}

/// Opens the pane of `perm` in System Settings and, when it has a list to drag into and Ferrocast
/// runs from a `.app`, shows the helper next to it. Never blocks; the helper only appears when
/// called on the main thread.
pub fn open_pane_with_helper(perm: Permission) {
    let _ = std::process::Command::new("/usr/bin/open").arg(perm.pane_url()).spawn();
    let Some(mtm) = MainThreadMarker::new() else { return };
    let bundle = running_bundle();
    let shown = HELPER.with(|h| h.borrow().as_ref().map(|p| p.perm));
    if plan_open(perm, shown, bundle.as_deref()) == OpenPlan::Show {
        if let Some(bundle) = bundle {
            show(mtm, perm, &bundle);
        }
    }
}

/// Is the helper on screen?
pub fn helper_visible() -> bool {
    HELPER.with(|h| h.borrow().is_some())
}

/// Closes the helper, if any.
pub fn close_helper() {
    let parts = HELPER.with(|h| h.borrow_mut().take());
    if let Some(p) = parts {
        p.timer.invalidate();
        p.panel.orderOut(None);
    }
}

struct Parts {
    perm: Permission,
    panel: Retained<NSPanel>,
    timer: Retained<NSTimer>,
    /// Kept alive: buttons hold their target weakly.
    _target: Retained<CloseTarget>,
}

thread_local! {
    static HELPER: RefCell<Option<Parts>> = const { RefCell::new(None) };
}

fn rect(r: [f64; 4]) -> NSRect {
    NSRect::new(NSPoint::new(r[0], r[1]), NSSize::new(r[2], r[3]))
}

define_class!(
    /// Non-activating panel: clicking or dragging the helper never steals focus from System Settings.
    #[unsafe(super(NSPanel))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastPermissionHelperPanel"]
    struct HelperPanel;

    impl HelperPanel {
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
    /// Target of the close button.
    #[unsafe(super(objc2_foundation::NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastPermissionHelperTarget"]
    struct CloseTarget;

    impl CloseTarget {
        #[unsafe(method(closeClicked:))]
        fn close_clicked(&self, _sender: &AnyObject) {
            close_helper();
        }
    }

    unsafe impl NSObjectProtocol for CloseTarget {}
);

struct IconIvars {
    bundle: PathBuf,
}

define_class!(
    /// The icon: a drag source of the app bundle's file URL.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastPermissionIcon"]
    #[ivars = IconIvars]
    struct IconView;

    impl IconView {
        #[unsafe(method(mouseDownCanMoveWindow))]
        fn mouse_down_can_move_window(&self) -> bool {
            false
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        // The image subview must not swallow the drag.
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
            let Some(url) = crate::clipboard::ns_file_url(&self.ivars().bundle) else { return };
            let writer: &ProtocolObject<dyn NSPasteboardWriting> = ProtocolObject::from_ref(&*url);
            let item = NSDraggingItem::initWithPasteboardWriter(self.mtm().alloc::<NSDraggingItem>(), writer);
            let icon = NSWorkspace::sharedWorkspace().iconForFile(&NSString::from_str(&self.ivars().bundle.to_string_lossy()));
            unsafe { item.setDraggingFrame_contents(rect([0.0, 0.0, 40.0, 40.0]), Some(&*icon)) };
            let array = NSArray::from_retained_slice(&[item]);
            let source: &ProtocolObject<dyn NSDraggingSource> = ProtocolObject::from_ref(self);
            self.beginDraggingSessionWithItems_event_source(&array, event, source);
        }
    }

    unsafe impl NSObjectProtocol for IconView {}

    unsafe impl NSDraggingSource for IconView {
        #[unsafe(method(draggingSession:sourceOperationMaskForDraggingContext:))]
        fn source_mask(&self, _session: &NSDraggingSession, _context: NSDraggingContext) -> NSDragOperation {
            NSDragOperation::Copy | NSDragOperation::Generic
        }
    }
);

/// Is System Settings running, and where is its biggest window (top-left global points)?
fn settings_state() -> (bool, Option<[f64; 4]>) {
    let apps = NSRunningApplication::runningApplicationsWithBundleIdentifier(&NSString::from_str(SETTINGS_BUNDLE_ID));
    let Some(pid) = apps.iter().next().map(|a| a.processIdentifier()) else { return (false, None) };
    (true, settings_window(pid))
}

fn settings_window(pid: i32) -> Option<[f64; 4]> {
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use core_graphics::window::*;
    let arr = copy_window_info(kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements, kCGNullWindowID)?;
    let mut best: Option<[f64; 4]> = None;
    for item in arr.iter() {
        let dict: CFDictionary<CFString, CFType> = unsafe { CFDictionary::wrap_under_get_rule(*item as _) };
        let num = |d: &CFDictionary<CFString, CFType>, k: &str| {
            d.find(CFString::new(k)).and_then(|v| v.downcast::<CFNumber>()).and_then(|n| n.to_f64())
        };
        if num(&dict, "kCGWindowOwnerPID") != Some(pid as f64) || num(&dict, "kCGWindowLayer") != Some(0.0) {
            continue;
        }
        let Some(b) = dict.find(CFString::new("kCGWindowBounds")).and_then(|v| v.downcast::<CFDictionary>()) else { continue };
        let b: CFDictionary<CFString, CFType> = unsafe { CFDictionary::wrap_under_get_rule(b.as_concrete_TypeRef()) };
        let (Some(x), Some(y), Some(w), Some(h)) = (num(&b, "X"), num(&b, "Y"), num(&b, "Width"), num(&b, "Height")) else { continue };
        if best.is_none_or(|o| w * h > o[2] * o[3]) {
            best = Some([x, y, w, h]);
        }
    }
    best
}

/// Cocoa frame of the screen holding the centre of `win` (top-left points), else the main screen.
fn screen_for(mtm: MainThreadMarker, win: Option<[f64; 4]>) -> [f64; 4] {
    let screens = NSScreen::screens(mtm);
    let primary_h = screens.iter().next().map(|s| s.frame().size.height).unwrap_or(0.0);
    let frame = |s: &NSScreen| {
        let f = s.frame();
        [f.origin.x, f.origin.y, f.size.width, f.size.height]
    };
    if let Some(w) = win {
        let (cx, cy) = (w[0] + w[2] / 2.0, primary_h - (w[1] + w[3] / 2.0));
        if let Some(s) = screens.iter().map(|s| frame(&s)).find(|f| cx >= f[0] && cx < f[0] + f[2] && cy >= f[1] && cy < f[1] + f[3]) {
            return s;
        }
    }
    NSScreen::mainScreen(mtm).map(|s| frame(&s)).unwrap_or([0.0, 0.0, 1440.0, 900.0])
}

fn place(mtm: MainThreadMarker, panel: &NSPanel, size: (f64, f64)) {
    let (_, win) = settings_state();
    let primary_h = NSScreen::screens(mtm).iter().next().map(|s| s.frame().size.height).unwrap_or(0.0);
    let cocoa_win = win.map(|w| [w[0], primary_h - w[1] - w[3], w[2], w[3]]);
    let screen = screen_for(mtm, win);
    panel.setFrame_display(rect(helper_frame(cocoa_win, screen, size)), true);
}

fn show(mtm: MainThreadMarker, perm: Permission, bundle: &Path) {
    use ferrocast_core::i18n::tr;
    close_helper();
    let hint = tr("Drag Ferrocast into the list").into_owned();
    let size = helper_size(crate::hud::text_width(&hint, 13.0));
    let (w, h) = size;
    let style = NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel;
    let panel: Retained<HelperPanel> = unsafe {
        msg_send![
            super(mtm.alloc::<HelperPanel>().set_ivars(())),
            initWithContentRect: rect([0.0, 0.0, w, h]),
            styleMask: style,
            backing: NSBackingStoreType::Buffered,
            defer: false
        ]
    };
    unsafe { panel.setReleasedWhenClosed(false) };
    panel.setLevel(NSPopUpMenuWindowLevel);
    panel.setFloatingPanel(true);
    panel.setHidesOnDeactivate(false);
    panel.setOpaque(false);
    panel.setHasShadow(true);
    panel.setBackgroundColor(Some(&NSColor::clearColor()));
    panel.setExcludedFromWindowsMenu(true);
    panel.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::IgnoresCycle,
    );
    if let Some(dark) = NSAppearance::appearanceNamed(unsafe { NSAppearanceNameVibrantDark }) {
        panel.setAppearance(Some(&dark));
    }

    // dark, rounded card with a hairline border (Ferrocast's black field)
    let card = NSView::initWithFrame(mtm.alloc::<NSView>(), rect([0.0, 0.0, w, h]));
    card.setWantsLayer(true);
    if let Some(l) = card.layer() {
        l.setCornerRadius(16.0);
        l.setMasksToBounds(true);
        l.setBorderWidth(1.0);
        l.setBackgroundColor(Some(&NSColor::colorWithSRGBRed_green_blue_alpha(0.055, 0.055, 0.063, 0.96).CGColor()));
        l.setBorderColor(Some(&NSColor::colorWithSRGBRed_green_blue_alpha(1.0, 1.0, 1.0, 0.12).CGColor()));
    }

    let icon: Retained<IconView> = unsafe {
        msg_send![super(mtm.alloc::<IconView>().set_ivars(IconIvars { bundle: bundle.to_path_buf() })), initWithFrame: rect([14.0, (h - 40.0) / 2.0, 40.0, 40.0])]
    };
    let image = NSWorkspace::sharedWorkspace().iconForFile(&NSString::from_str(&bundle.to_string_lossy()));
    let iv = NSImageView::imageViewWithImage(&image, mtm);
    iv.setFrame(rect([0.0, 0.0, 40.0, 40.0]));
    icon.addSubview(&iv);
    icon.setToolTip(Some(&NSString::from_str(&hint)));
    card.addSubview(&icon);

    let text = NSTextField::labelWithString(&NSString::from_str(&hint), mtm);
    text.setFont(Some(&NSFont::systemFontOfSize_weight(13.0, 0.3)));
    text.setTextColor(Some(&NSColor::whiteColor()));
    text.sizeToFit();
    let th = text.frame().size.height;
    text.setFrame(rect([66.0, (h - th) / 2.0, w - 66.0 - 44.0, th]));
    card.addSubview(&text);

    let target: Retained<CloseTarget> = unsafe { msg_send![super(mtm.alloc::<CloseTarget>().set_ivars(())), init] };
    let t: &AnyObject = &target;
    let close =
        unsafe { NSButton::buttonWithTitle_target_action(&NSString::from_str("\u{2715}"), Some(t), Some(sel!(closeClicked:)), mtm) };
    close.setBordered(false);
    close.setRefusesFirstResponder(true);
    close.setToolTip(Some(&NSString::from_str(&tr("Close"))));
    close.setFrame(rect([w - 34.0, (h - 22.0) / 2.0, 22.0, 22.0]));
    card.addSubview(&close);

    panel.setContentView(Some(&card));
    let panel: Retained<NSPanel> = Retained::into_super(panel);
    place(mtm, &panel, size);
    panel.orderFrontRegardless();

    let started = std::time::Instant::now();
    let seen = std::cell::Cell::new(false);
    let block = block2::RcBlock::new(move |_t: std::ptr::NonNull<NSTimer>| {
        let (running, _) = settings_state();
        let reason = close_reason(is_granted(perm), running, seen.get(), started.elapsed().as_secs_f64());
        seen.set(seen.get() || running);
        if reason.is_some() {
            close_helper();
        }
    });
    let timer = unsafe { NSTimer::scheduledTimerWithTimeInterval_repeats_block(POLL_SECS, true, &block) };
    HELPER.with(|h| *h.borrow_mut() = Some(Parts { perm, panel, timer, _target: target }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_permission_has_its_own_pane_that_maps_back() {
        let mut urls = std::collections::HashSet::new();
        for p in Permission::ALL {
            assert!(p.pane_url().starts_with("x-apple.systempreferences:com.apple.preference.security?Privacy_"));
            assert!(urls.insert(p.pane_url()), "{p:?} shares a pane");
            assert_eq!(Permission::from_pane_url(p.pane_url()), Some(p));
        }
        assert_eq!(Permission::from_pane_url("https://example.com"), None);
        assert!(Permission::Accessibility.pane_url().ends_with("Privacy_Accessibility"));
        assert!(Permission::ScreenRecording.pane_url().ends_with("Privacy_ScreenCapture"));
    }

    #[test]
    fn capture_panes_are_the_same_urls() {
        assert_eq!(crate::capture::CAMERA_PANE, Permission::Camera.pane_url());
        assert_eq!(crate::capture::MICROPHONE_PANE, Permission::Microphone.pane_url());
    }

    #[test]
    fn only_listed_permissions_get_a_drop_helper() {
        assert!(Permission::Accessibility.has_drop_list());
        assert!(Permission::ScreenRecording.has_drop_list());
        assert!(Permission::InputMonitoring.has_drop_list());
        for p in [Permission::Automation, Permission::Camera, Permission::Microphone, Permission::SpeechRecognition] {
            assert!(!p.has_drop_list(), "{p:?}");
        }
    }

    #[test]
    fn bundle_is_the_enclosing_app() {
        let exe = Path::new("/Applications/Ferrocast.app/Contents/MacOS/ferrocast");
        assert_eq!(bundle_path_from_exe(exe), Some(PathBuf::from("/Applications/Ferrocast.app")));
        let nested = Path::new("/Applications/Ferrocast.app/Contents/Helpers/Other.app/Contents/MacOS/x");
        assert_eq!(bundle_path_from_exe(nested), Some(PathBuf::from("/Applications/Ferrocast.app/Contents/Helpers/Other.app")));
    }

    #[test]
    fn no_bundle_for_a_dev_build() {
        assert_eq!(bundle_path_from_exe(Path::new("/Users/me/raycast/target/debug/ferrocast")), None);
        assert_eq!(bundle_path_from_exe(Path::new("/opt/my.application/bin/ferrocast")), None, "only a .app directory counts");
        assert_eq!(bundle_path_from_exe(Path::new("")), None);
    }

    #[test]
    fn plan_shows_the_helper_once_per_permission() {
        let app = Some(Path::new("/Applications/Ferrocast.app"));
        assert_eq!(plan_open(Permission::Accessibility, None, app), OpenPlan::Show);
        assert_eq!(plan_open(Permission::Accessibility, Some(Permission::Accessibility), app), OpenPlan::Keep);
        assert_eq!(plan_open(Permission::ScreenRecording, Some(Permission::Accessibility), app), OpenPlan::Show);
    }

    #[test]
    fn plan_is_pane_only_without_list_or_bundle() {
        let app = Some(Path::new("/Applications/Ferrocast.app"));
        assert_eq!(plan_open(Permission::Camera, None, app), OpenPlan::PaneOnly);
        assert_eq!(plan_open(Permission::Accessibility, None, None), OpenPlan::PaneOnly);
        assert_eq!(plan_open(Permission::Microphone, Some(Permission::Accessibility), app), OpenPlan::PaneOnly);
    }

    #[test]
    fn closes_when_granted() {
        assert_eq!(close_reason(true, true, true, 1.0), Some(CloseReason::Granted));
        assert_eq!(close_reason(true, false, false, 0.0), Some(CloseReason::Granted));
    }

    #[test]
    fn closes_when_settings_was_open_and_is_gone() {
        assert_eq!(close_reason(false, false, true, 30.0), Some(CloseReason::SettingsClosed));
        assert_eq!(close_reason(false, true, true, 30.0), None);
    }

    #[test]
    fn waits_for_settings_to_launch_then_gives_up() {
        assert_eq!(close_reason(false, false, false, 2.0), None);
        assert_eq!(close_reason(false, false, false, SETTINGS_GRACE_SECS + 0.1), Some(CloseReason::SettingsClosed));
        assert_eq!(close_reason(false, true, false, 100.0), None, "settings is running: keep");
    }

    #[test]
    fn size_grows_with_the_hint_and_stays_bounded() {
        let (w0, h0) = helper_size(100.0);
        let (w1, h1) = helper_size(180.0);
        assert!(w1 > w0 && h0 == h1 && h0 > 40.0);
        assert!(helper_size(5000.0).0 <= 420.0, "a long translation does not make a huge panel");
        assert!(helper_size(0.0).0 >= 200.0);
    }

    const SCREEN: [f64; 4] = [0.0, 0.0, 1440.0, 900.0];

    #[test]
    fn sits_centred_below_the_settings_window() {
        let f = helper_frame(Some([400.0, 300.0, 700.0, 500.0]), SCREEN, (300.0, 64.0));
        assert_eq!(f, [600.0, 300.0 - 64.0 - GAP, 300.0, 64.0]);
    }

    #[test]
    fn falls_back_to_bottom_centre() {
        let f = helper_frame(None, SCREEN, (300.0, 64.0));
        assert_eq!(f[0], 570.0);
        assert!(f[1] > 0.0 && f[1] < 100.0);
        // settings flush with the bottom: no room below
        let g = helper_frame(Some([400.0, 10.0, 700.0, 500.0]), SCREEN, (300.0, 64.0));
        assert_eq!(g, f);
    }

    #[test]
    fn stays_inside_the_screen_sideways() {
        let f = helper_frame(Some([-100.0, 300.0, 300.0, 400.0]), SCREEN, (300.0, 64.0));
        assert_eq!(f[0], 0.0);
        let g = helper_frame(Some([1300.0, 300.0, 400.0, 400.0]), SCREEN, (300.0, 64.0));
        assert_eq!(g[0], 1140.0);
    }
}
