//! Recording countdown overlay and recording HUD (S13).
//!
//! * The countdown is a big `3…2…1` numeral on a frosted disc, centred on the display being
//!   recorded. It ignores the mouse.
//! * The HUD is a small always-on-top pill with the elapsed `mm:ss`, Pause/Resume and Stop buttons.
//!   It is draggable by its background and never takes focus.
//!
//! Neither is part of the recording. Two layers: both panels set `sharingType = .none`, and the
//! caller passes [`excluded_window_ids`] as `excludes.window_ids` of `record.start`, which the
//! helper turns into `SCContentFilter(display:excludingWindows:)` (`Recorder.swift`, matching
//! `SCWindow.windowID` against `NSWindow.windowNumber`). A window only has a number once it is on
//! screen: show the countdown/HUD first, then read the ids, then start the recording.
//!
//! Every AppKit function runs on the main thread (`MainThreadMarker`). The time, countdown, disk
//! and layout logic is pure and unit-tested below; the AppKit code only calls it.

use std::cell::RefCell;
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSBackingStoreType, NSBezelStyle, NSButton, NSColor, NSFont, NSPanel, NSPopUpMenuWindowLevel, NSScreen, NSTextAlignment, NSTextField,
    NSView, NSVisualEffectBlendingMode, NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView, NSWindowCollectionBehavior,
    NSWindowSharingType, NSWindowStyleMask,
};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize, NSString};

// ------------------------------------------------------------------ pure logic

/// Free space below which the HUD warns (S13: `< 2 GB`).
pub const LOW_DISK_WARN_BYTES: u64 = 2_000_000_000;
/// Free space below which the recording stops cleanly, keeping the file (S13: `< 200 MB`).
pub const LOW_DISK_STOP_BYTES: u64 = 200_000_000;

/// Elapsed time as `mm:ss`, or `h:mm:ss` from one hour on. Negative and NaN count as zero.
pub fn format_elapsed(secs: f64) -> String {
    let total = if secs.is_finite() && secs > 0.0 { secs as u64 } else { 0 };
    let (h, m, s) = (total / 3600, total % 3600 / 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

/// Menu-bar title while recording: `●  mm:ss`.
pub fn tray_title(secs: f64) -> String {
    format!("\u{25CF}  {}", format_elapsed(secs))
}

/// What the disk guard decides for a given amount of free space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiskLevel {
    Ok,
    /// Below [`LOW_DISK_WARN_BYTES`]: show the warning, keep recording.
    Low,
    /// Below [`LOW_DISK_STOP_BYTES`]: stop cleanly and keep the file.
    Critical,
}

pub fn disk_level(free_bytes: u64) -> DiskLevel {
    if free_bytes < LOW_DISK_STOP_BYTES {
        DiskLevel::Critical
    } else if free_bytes < LOW_DISK_WARN_BYTES {
        DiskLevel::Low
    } else {
        DiskLevel::Ok
    }
}

/// `Low disk space: N GB left` (decimal GB, one decimal rounded down so 1.99 GB never reads as
/// 2.0 GB); `None` when there is enough room.
pub fn low_disk_message(free_bytes: u64) -> Option<String> {
    (disk_level(free_bytes) != DiskLevel::Ok).then(|| {
        let tenths = free_bytes / 100_000_000;
        format!("Low disk space: {}.{} GB left", tenths / 10, tenths % 10)
    })
}

/// One step of the countdown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CountdownStep {
    /// Show this numeral.
    Show(u32),
    /// The countdown reached zero: start recording.
    Finished,
}

/// `3…2…1` state machine. The caller shows [`Countdown::current`] at once and calls
/// [`Countdown::tick`] once per second.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Countdown {
    remaining: u32,
}

impl Countdown {
    pub fn new(seconds: u32) -> Self {
        Self { remaining: seconds }
    }

    /// Numeral to show now; `None` when there is nothing (left) to count.
    pub fn current(&self) -> Option<u32> {
        (self.remaining > 0).then_some(self.remaining)
    }

    pub fn is_finished(&self) -> bool {
        self.remaining == 0
    }

    /// One second passed. `Finished` is returned on the tick that reaches zero and on every
    /// later one.
    pub fn tick(&mut self) -> CountdownStep {
        self.remaining = self.remaining.saturating_sub(1);
        match self.current() {
            Some(n) => CountdownStep::Show(n),
            None => CountdownStep::Finished,
        }
    }
}

const COUNTDOWN_SIZE: f64 = 240.0;
const HUD_W: f64 = 264.0;
const HUD_H: f64 = 44.0;
const HUD_BOTTOM_GAP: f64 = 28.0;

/// Cocoa (bottom-left) frame of a display given as a top-left `[x, y, w, h]` rectangle.
pub fn cocoa_frame(top_left: [f64; 4], primary_height: f64) -> [f64; 4] {
    [top_left[0], primary_height - top_left[1] - top_left[3], top_left[2], top_left[3]]
}

/// A `w` × `h` frame centred in `outer`.
pub fn centered_frame(outer: [f64; 4], w: f64, h: f64) -> [f64; 4] {
    [outer[0] + (outer[2] - w) / 2.0, outer[1] + (outer[3] - h) / 2.0, w, h]
}

/// Where the HUD first appears on `display` (Cocoa frame): bottom centre, above the Dock area
/// when `display` is the visible frame.
pub fn hud_frame(display: [f64; 4]) -> [f64; 4] {
    [display[0] + (display[2] - HUD_W) / 2.0, display[1] + HUD_BOTTOM_GAP, HUD_W, HUD_H]
}

/// Window numbers worth excluding: only real (positive) ones.
pub fn usable_window_ids(numbers: &[Option<i64>]) -> Vec<u32> {
    numbers.iter().flatten().filter(|n| **n > 0).filter_map(|n| u32::try_from(*n).ok()).collect()
}

// ------------------------------------------------------------------ AppKit

define_class!(
    /// Borderless panel that never takes focus (the HUD still receives clicks).
    #[unsafe(super(NSPanel))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastRecordingPanel"]
    struct RecordingPanel;

    impl RecordingPanel {
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

type Callback = Rc<dyn Fn()>;

struct TargetIvars {
    on_pause: RefCell<Option<Callback>>,
    on_stop: RefCell<Option<Callback>>,
}

define_class!(
    /// Button target of the HUD.
    #[unsafe(super(objc2_foundation::NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastRecordingHudTarget"]
    #[ivars = TargetIvars]
    struct HudTarget;

    impl HudTarget {
        #[unsafe(method(pauseClicked:))]
        fn pause_clicked(&self, _sender: &AnyObject) {
            let cb = self.ivars().on_pause.borrow().clone();
            if let Some(cb) = cb {
                cb();
            }
        }

        #[unsafe(method(stopClicked:))]
        fn stop_clicked(&self, _sender: &AnyObject) {
            let cb = self.ivars().on_stop.borrow().clone();
            if let Some(cb) = cb {
                cb();
            }
        }
    }

    unsafe impl NSObjectProtocol for HudTarget {}
);

struct CountdownParts {
    panel: Retained<RecordingPanel>,
    label: Retained<NSTextField>,
}

struct HudParts {
    panel: Retained<RecordingPanel>,
    time: Retained<NSTextField>,
    pause: Retained<NSButton>,
    /// Kept alive: buttons hold their target weakly.
    _target: Retained<HudTarget>,
}

thread_local! {
    static COUNTDOWN: RefCell<Option<CountdownParts>> = const { RefCell::new(None) };
    static HUD: RefCell<Option<HudParts>> = const { RefCell::new(None) };
}

fn rect(r: [f64; 4]) -> NSRect {
    NSRect::new(NSPoint::new(r[0], r[1]), NSSize::new(r[2], r[3]))
}

fn primary_height(mtm: MainThreadMarker) -> f64 {
    NSScreen::screens(mtm).iter().next().map(|s| s.frame().size.height).unwrap_or(0.0)
}

/// Cocoa frame of the display to use: `display` (top-left global points, as the helper reports
/// displays) or the main screen.
fn display_frame(mtm: MainThreadMarker, display: Option<[f64; 4]>) -> [f64; 4] {
    match display {
        Some(d) => cocoa_frame(d, primary_height(mtm)),
        None => {
            let f = NSScreen::mainScreen(mtm).map(|s| s.frame()).unwrap_or(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1440.0, 900.0)));
            [f.origin.x, f.origin.y, f.size.width, f.size.height]
        }
    }
}

fn new_panel(mtm: MainThreadMarker, frame: [f64; 4], ignores_mouse: bool) -> Retained<RecordingPanel> {
    let style = NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel;
    let p: Retained<RecordingPanel> = unsafe {
        msg_send![
            super(mtm.alloc::<RecordingPanel>().set_ivars(())),
            initWithContentRect: rect(frame),
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
    p.setHasShadow(true);
    p.setBackgroundColor(Some(&NSColor::clearColor()));
    p.setIgnoresMouseEvents(ignores_mouse);
    p.setExcludedFromWindowsMenu(true);
    // Not part of the recording, whatever the capture filter does.
    p.setSharingType(NSWindowSharingType::None);
    p.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::Stationary
            | NSWindowCollectionBehavior::IgnoresCycle,
    );
    p
}

fn frosted(mtm: MainThreadMarker, frame: [f64; 4], radius: f64) -> Retained<NSVisualEffectView> {
    let fx = NSVisualEffectView::initWithFrame(mtm.alloc::<NSVisualEffectView>(), rect(frame));
    fx.setMaterial(NSVisualEffectMaterial::HUDWindow);
    fx.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
    fx.setState(NSVisualEffectState::Active);
    fx.setWantsLayer(true);
    if let Some(l) = fx.layer() {
        l.setCornerRadius(radius);
        l.setMasksToBounds(true);
    }
    fx
}

fn label(mtm: MainThreadMarker, text: &str, font: &NSFont, color: &NSColor) -> Retained<NSTextField> {
    let l = NSTextField::labelWithString(&NSString::from_str(text), mtm);
    l.setFont(Some(font));
    l.setTextColor(Some(color));
    l.setAlignment(NSTextAlignment::Center);
    l
}

/// Centre a label inside a container of `w` × `h` after sizing it to its text.
fn center_label(l: &NSTextField, w: f64, h: f64) {
    l.sizeToFit();
    let s = l.frame().size;
    l.setFrame(rect(centered_frame([0.0, 0.0, w, h], s.width, s.height)));
}

/// Show (or update) the countdown numeral `n` on `display` (top-left points; `None` = main
/// screen). The panel ignores the mouse and is excluded from screen sharing.
pub fn countdown_show(mtm: MainThreadMarker, display: Option<[f64; 4]>, n: u32) {
    let frame = centered_frame(display_frame(mtm, display), COUNTDOWN_SIZE, COUNTDOWN_SIZE);
    COUNTDOWN.with(|slot| {
        let mut slot = slot.borrow_mut();
        let parts = slot.get_or_insert_with(|| {
            let panel = new_panel(mtm, frame, true);
            let fx = frosted(mtm, [0.0, 0.0, COUNTDOWN_SIZE, COUNTDOWN_SIZE], COUNTDOWN_SIZE / 2.0);
            let font = NSFont::monospacedDigitSystemFontOfSize_weight(150.0, 0.4);
            let l = label(mtm, "", &font, &NSColor::whiteColor());
            fx.addSubview(&l);
            panel.setContentView(Some(&fx));
            CountdownParts { panel, label: l }
        });
        parts.panel.setFrame_display(rect(frame), true);
        parts.label.setStringValue(&NSString::from_str(&n.to_string()));
        center_label(&parts.label, COUNTDOWN_SIZE, COUNTDOWN_SIZE);
        parts.panel.orderFrontRegardless();
    });
}

/// Remove the countdown panel.
pub fn countdown_hide(_mtm: MainThreadMarker) {
    COUNTDOWN.with(|slot| {
        if let Some(p) = slot.borrow_mut().take() {
            p.panel.orderOut(None);
        }
    });
}

/// Show the recording HUD on `display` (top-left points; `None` = main screen) at bottom centre.
/// `on_pause_toggle` runs for the Pause/Resume button, `on_stop` for Stop; both on the main
/// thread. An HUD already on screen is replaced.
pub fn hud_show(mtm: MainThreadMarker, display: Option<[f64; 4]>, on_pause_toggle: Box<dyn Fn()>, on_stop: Box<dyn Fn()>) {
    hud_hide(mtm);
    let frame = hud_frame(display_frame(mtm, display));
    let panel = new_panel(mtm, frame, false);
    panel.setMovableByWindowBackground(true);
    let fx = frosted(mtm, [0.0, 0.0, HUD_W, HUD_H], HUD_H / 2.0);

    let target: Retained<HudTarget> = unsafe {
        msg_send![
            super(mtm.alloc::<HudTarget>().set_ivars(TargetIvars {
                on_pause: RefCell::new(Some(Rc::from(on_pause_toggle))),
                on_stop: RefCell::new(Some(Rc::from(on_stop))),
            })),
            init
        ]
    };
    let t: &AnyObject = &target;

    let dot = label(mtm, "\u{25CF}", &NSFont::systemFontOfSize(14.0), &NSColor::systemRedColor());
    dot.sizeToFit();
    dot.setFrame(rect([16.0, (HUD_H - dot.frame().size.height) / 2.0, 16.0, dot.frame().size.height]));
    fx.addSubview(&dot);

    let time = label(mtm, &format_elapsed(0.0), &NSFont::monospacedDigitSystemFontOfSize_weight(16.0, 0.3), &NSColor::whiteColor());
    time.setAlignment(NSTextAlignment::Left);
    time.sizeToFit();
    let th = time.frame().size.height;
    time.setFrame(rect([36.0, (HUD_H - th) / 2.0, 76.0, th]));
    fx.addSubview(&time);

    let pause = unsafe { NSButton::buttonWithTitle_target_action(&NSString::from_str("Pause"), Some(t), Some(sel!(pauseClicked:)), mtm) };
    pause.setBezelStyle(NSBezelStyle::Push);
    pause.setRefusesFirstResponder(true);
    pause.setFrame(rect([116.0, (HUD_H - 24.0) / 2.0, 70.0, 24.0]));
    fx.addSubview(&pause);

    let stop = unsafe { NSButton::buttonWithTitle_target_action(&NSString::from_str("Stop"), Some(t), Some(sel!(stopClicked:)), mtm) };
    stop.setBezelStyle(NSBezelStyle::Push);
    stop.setRefusesFirstResponder(true);
    stop.setFrame(rect([192.0, (HUD_H - 24.0) / 2.0, 58.0, 24.0]));
    fx.addSubview(&stop);

    let view: &NSView = &fx;
    panel.setContentView(Some(view));
    panel.orderFrontRegardless();
    HUD.with(|slot| *slot.borrow_mut() = Some(HudParts { panel, time, pause, _target: target }));
}

/// Refresh the HUD: elapsed time and the Pause/Resume caption. No-op without an HUD.
pub fn hud_update(elapsed_secs: f64, paused: bool) {
    HUD.with(|slot| {
        if let Some(h) = slot.borrow().as_ref() {
            h.time.setStringValue(&NSString::from_str(&format_elapsed(elapsed_secs)));
            h.pause.setTitle(&NSString::from_str(if paused { "Resume" } else { "Pause" }));
        }
    });
}

/// Remove the HUD.
pub fn hud_hide(_mtm: MainThreadMarker) {
    HUD.with(|slot| {
        if let Some(h) = slot.borrow_mut().take() {
            h.panel.orderOut(None);
        }
    });
}

/// `NSWindow.windowNumber` of the countdown panel while it is on screen.
pub fn countdown_window_number() -> Option<i64> {
    COUNTDOWN.with(|s| s.borrow().as_ref().map(|c| c.panel.windowNumber() as i64))
}

/// `NSWindow.windowNumber` of the HUD while it is on screen.
pub fn hud_window_number() -> Option<i64> {
    HUD.with(|s| s.borrow().as_ref().map(|h| h.panel.windowNumber() as i64))
}

/// Window ids for `record.start`'s `excludes.window_ids`: every countdown/HUD panel currently on
/// screen. Main thread only (empty elsewhere).
pub fn excluded_window_ids() -> Vec<u32> {
    usable_window_ids(&[countdown_window_number(), hud_window_number()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elapsed_is_mm_ss() {
        assert_eq!(format_elapsed(0.0), "00:00");
        assert_eq!(format_elapsed(5.9), "00:05");
        assert_eq!(format_elapsed(72.0), "01:12");
        assert_eq!(format_elapsed(3599.9), "59:59");
    }

    #[test]
    fn elapsed_grows_an_hour_field_past_one_hour() {
        assert_eq!(format_elapsed(3600.0), "1:00:00");
        assert_eq!(format_elapsed(3725.0), "1:02:05");
        assert_eq!(format_elapsed(36000.0), "10:00:00");
    }

    #[test]
    fn elapsed_ignores_garbage() {
        assert_eq!(format_elapsed(-3.0), "00:00");
        assert_eq!(format_elapsed(f64::NAN), "00:00");
        assert_eq!(format_elapsed(f64::INFINITY), "00:00");
    }

    #[test]
    fn tray_title_has_dot_two_spaces_and_time() {
        assert_eq!(tray_title(12.0), "\u{25CF}  00:12");
        assert_eq!(tray_title(3661.0), "\u{25CF}  1:01:01");
    }

    #[test]
    fn countdown_counts_3_2_1_then_finishes() {
        let mut c = Countdown::new(3);
        assert_eq!(c.current(), Some(3));
        assert!(!c.is_finished());
        assert_eq!(c.tick(), CountdownStep::Show(2));
        assert_eq!(c.tick(), CountdownStep::Show(1));
        assert_eq!(c.tick(), CountdownStep::Finished);
        assert!(c.is_finished());
        assert_eq!(c.current(), None);
        assert_eq!(c.tick(), CountdownStep::Finished);
    }

    #[test]
    fn zero_countdown_is_finished_at_once() {
        let c = Countdown::new(0);
        assert!(c.is_finished());
        assert_eq!(c.current(), None);
    }

    #[test]
    fn disk_thresholds_match_s13() {
        assert_eq!(disk_level(5_000_000_000), DiskLevel::Ok);
        assert_eq!(disk_level(LOW_DISK_WARN_BYTES), DiskLevel::Ok);
        assert_eq!(disk_level(LOW_DISK_WARN_BYTES - 1), DiskLevel::Low);
        assert_eq!(disk_level(LOW_DISK_STOP_BYTES), DiskLevel::Low);
        assert_eq!(disk_level(LOW_DISK_STOP_BYTES - 1), DiskLevel::Critical);
        assert_eq!(disk_level(0), DiskLevel::Critical);
    }

    #[test]
    fn low_disk_message_text() {
        assert_eq!(low_disk_message(1_500_000_000).as_deref(), Some("Low disk space: 1.5 GB left"));
        assert_eq!(low_disk_message(1_999_999_999).as_deref(), Some("Low disk space: 1.9 GB left"));
        assert_eq!(low_disk_message(250_000_000).as_deref(), Some("Low disk space: 0.2 GB left"));
        assert_eq!(low_disk_message(2_000_000_000), None);
        assert_eq!(low_disk_message(50_000_000_000), None);
    }

    #[test]
    fn cocoa_frame_flips_y() {
        assert_eq!(cocoa_frame([0.0, 0.0, 1440.0, 900.0], 900.0), [0.0, 0.0, 1440.0, 900.0]);
        // a display above the primary has a negative top-left y
        assert_eq!(cocoa_frame([0.0, -1080.0, 1920.0, 1080.0], 900.0), [0.0, 900.0, 1920.0, 1080.0]);
        // a display to the right, lower than the primary
        assert_eq!(cocoa_frame([1440.0, 300.0, 1920.0, 1080.0], 900.0), [1440.0, -480.0, 1920.0, 1080.0]);
    }

    #[test]
    fn countdown_panel_is_centered_on_its_display() {
        let d = cocoa_frame([1440.0, 300.0, 1920.0, 1080.0], 900.0);
        let f = centered_frame(d, COUNTDOWN_SIZE, COUNTDOWN_SIZE);
        assert_eq!(f, [1440.0 + 840.0, -480.0 + 420.0, 240.0, 240.0]);
    }

    #[test]
    fn hud_starts_bottom_center_of_the_display() {
        let f = hud_frame([100.0, 50.0, 1000.0, 800.0]);
        assert_eq!(f, [100.0 + (1000.0 - HUD_W) / 2.0, 50.0 + HUD_BOTTOM_GAP, HUD_W, HUD_H]);
    }

    #[test]
    fn only_real_window_numbers_are_excluded() {
        assert_eq!(usable_window_ids(&[Some(9001), None, Some(-1), Some(0), Some(42)]), vec![9001, 42]);
        assert!(usable_window_ids(&[]).is_empty());
    }
}
