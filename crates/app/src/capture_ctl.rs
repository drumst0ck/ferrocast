//! Screenshot commands (Capture Area / Window / Screen, S1 S3 S4 S6): which commands exist, what
//! a stored shortcut says when they do not, how a selection becomes a helper target, where the
//! file goes and what happens after. Everything that decides is a pure function; the AppKit and
//! helper calls are thin shells around them.

use chrono::NaiveDateTime;
use ferrocast_core::capture::protocol::{Display as HelperDisplay, Shareable, Target, Window as HelperWindow};
use ferrocast_core::capture::region::{capture_file_name, Point, Rect, Window};
use ferrocast_core::capture::settings::AfterCapture;
use ferrocast_core::capture::stitch::{StopReason, Strip, MAX_STEPS};
use ferrocast_macos::selector::Selection;
use std::path::{Path, PathBuf};

/// S4: shown verbatim when the helper reports `permission-screen`.
pub const PERMISSION_TOAST: &str = "Screen Recording is off for Ferrocast. Turn it on in System Settings → Privacy & Security → Screen & System Audio Recording, then try again.";
/// Pane opened together with the S4 toast.
pub const SCREEN_RECORDING_PANE: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture";
/// S3 when the helper is not shipped.
pub const NO_HELPER_MESSAGE: &str = "Screen capture is not available in this build.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureKind {
    Area,
    Window,
    Screen,
    Scrolling,
}

impl CaptureKind {
    pub const ALL: [CaptureKind; 4] = [CaptureKind::Area, CaptureKind::Window, CaptureKind::Screen, CaptureKind::Scrolling];

    /// The command id of the matching launcher builtin.
    pub fn command_id(self) -> &'static str {
        match self {
            CaptureKind::Area => "builtin:capture-area",
            CaptureKind::Window => "builtin:capture-window",
            CaptureKind::Screen => "builtin:capture-screen",
            CaptureKind::Scrolling => "builtin:capture-scrolling",
        }
    }

    pub fn from_command_id(id: &str) -> Option<CaptureKind> {
        Self::ALL.into_iter().find(|k| k.command_id() == id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Availability {
    Available,
    /// macOS older than 14 (the running major).
    NeedsMacOs14(u32),
    NoHelper,
}

/// S1/S3: commands exist on macOS 14+ with the helper present.
pub fn availability(os_major: u32, helper_exists: bool) -> Availability {
    if os_major < 14 {
        Availability::NeedsMacOs14(os_major)
    } else if !helper_exists {
        Availability::NoHelper
    } else {
        Availability::Available
    }
}

impl Availability {
    /// The S3 sentence for a stored shortcut; `None` when capture works.
    pub fn message(self) -> Option<String> {
        match self {
            Availability::Available => None,
            Availability::NeedsMacOs14(v) => Some(format!("Screen capture needs macOS 14. This Mac runs macOS {v}.")),
            Availability::NoHelper => Some(NO_HELPER_MESSAGE.into()),
        }
    }
}

/// The running macOS major (0 when unknown), found through the cached `os_at_least`.
pub fn current_os_major() -> u32 {
    (10..=60).rev().find(|m| ferrocast_macos::system::os_at_least(*m)).unwrap_or(0)
}

/// Availability of this Mac with the app executable `exe` (already `instance::resolve_exe`d).
pub fn detect(exe: &Path) -> Availability {
    availability(current_os_major(), ferrocast_macos::capture::helper_path(exe).is_file())
}

/// Per-session capture state: what is available and whether the S3 sentence was already shown.
#[derive(Clone, Debug)]
pub struct CaptureState {
    pub exe: PathBuf,
    pub availability: Availability,
    warned: bool,
}

/// What a capture request does first.
#[derive(Debug, PartialEq, Eq)]
pub enum Gate {
    Go,
    /// Unavailable; say this (first time this session).
    Say(String),
    /// Unavailable; already said.
    Quiet,
}

impl CaptureState {
    pub fn detect(exe: PathBuf) -> Self {
        let availability = detect(&exe);
        CaptureState { exe, availability, warned: false }
    }

    #[cfg(test)]
    pub fn with(exe: PathBuf, availability: Availability) -> Self {
        CaptureState { exe, availability, warned: false }
    }

    pub fn gate(&mut self) -> Gate {
        match self.availability.message() {
            None => Gate::Go,
            Some(_) if self.warned => Gate::Quiet,
            Some(m) => {
                self.warned = true;
                Gate::Say(m)
            }
        }
    }
}

/// Helper windows as selector windows (front to back, normal on-screen windows only).
pub fn selector_windows(windows: &[HelperWindow]) -> Vec<Window> {
    windows
        .iter()
        .filter(|w| w.on_screen && w.layer == 0)
        .map(|w| Window { id: w.id, frame: Rect::new(w.frame[0], w.frame[1], w.frame[2], w.frame[3]) })
        .collect()
}

/// What to do with the user's selection.
#[derive(Debug, PartialEq)]
pub enum Plan {
    Shoot(Target),
    Nothing,
}

/// Turns the selection (`None` for Capture Screen) into a helper target. Areas are converted from
/// global top-left points to display-local; Capture Screen takes the display under `pointer`.
pub fn capture_plan(kind: CaptureKind, selection: Option<Selection>, pointer: Point, shareable: &Shareable) -> Plan {
    let rect_of = |d: &HelperDisplay| Rect::new(d.frame[0], d.frame[1], d.frame[2], d.frame[3]);
    match (kind, selection) {
        (CaptureKind::Screen, _) => {
            let under = shareable.displays.iter().find(|d| rect_of(d).contains(pointer));
            match under.or_else(|| shareable.displays.iter().find(|d| d.main)).or(shareable.displays.first()) {
                Some(d) => Plan::Shoot(Target::Display { display: d.id }),
                None => Plan::Nothing,
            }
        }
        (_, Some(Selection::Window(id))) => Plan::Shoot(Target::Window { window: id }),
        (_, Some(Selection::Area(r))) => {
            let centre = Point { x: r.x + r.w / 2.0, y: r.y + r.h / 2.0 };
            match shareable.displays.iter().find(|d| rect_of(d).contains(centre)) {
                Some(d) => Plan::Shoot(Target::Rect { display: d.id, rect: [r.x - d.frame[0], r.y - d.frame[1], r.w, r.h] }),
                None => Plan::Nothing,
            }
        }
        (_, Some(Selection::Cancelled) | None) => Plan::Nothing,
    }
}

/// `dir/Capture YYYY-MM-DD at HH.MM.SS[ n].png`, never an existing file.
pub fn save_path(dir: &Path, now: NaiveDateTime) -> PathBuf {
    dir.join(capture_file_name(now, |n| dir.join(n).exists()))
}

#[derive(Debug, PartialEq, Eq)]
pub enum AfterAction {
    /// Open the Capture window (T8); until then reveal the file.
    Show,
    CopyImage,
    Nothing,
}

pub fn after_action(after: AfterCapture) -> AfterAction {
    match after {
        AfterCapture::Show => AfterAction::Show,
        AfterCapture::Copy => AfterAction::CopyImage,
        AfterCapture::Save => AfterAction::Nothing,
    }
}

/// How a capture ended.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Saved(PathBuf),
    Cancelled,
    PermissionDenied,
    Failed(String),
}

/// Toast text for an outcome (`None`: nothing to say).
pub fn outcome_toast(o: &Outcome) -> Option<String> {
    match o {
        Outcome::PermissionDenied => Some(PERMISSION_TOAST.into()),
        Outcome::Failed(e) => Some(format!("Capture failed: {e}")),
        Outcome::Saved(_) | Outcome::Cancelled => None,
    }
}

/// Opens the Screen Recording pane of System Settings (never in tests).
pub fn open_screen_recording_pane() {
    if !cfg!(test) {
        // spawn, do not wait: this runs on the main thread inside a winit callback
        let _ = std::process::Command::new("/usr/bin/open").arg(SCREEN_RECORDING_PANE).spawn();
    }
}

pub(crate) fn pointer_top_left(mtm: objc2::MainThreadMarker) -> Point {
    let primary_h = objc2_app_kit::NSScreen::screens(mtm).iter().next().map(|s| s.frame().size.height).unwrap_or(0.0);
    let m = objc2_app_kit::NSEvent::mouseLocation();
    ferrocast_core::capture::region::flip_point(Point { x: m.x, y: m.y }, primary_h)
}

/// The flows that pick something on screen first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    Capture(CaptureKind),
    Record(crate::record_ctl::RecordKind),
}

impl Flow {
    /// The capture kind that decides the overlay mode and the target.
    pub fn capture_kind(self) -> CaptureKind {
        match self {
            Flow::Capture(k) => k,
            Flow::Record(k) => k.capture_kind(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    /// The helper is listing displays and windows on a worker thread.
    Listing,
    /// The overlay is on screen and waits for the user.
    Selecting,
}

/// At most one select-then-act flow at a time (the overlay is a single shared panel set).
#[derive(Debug, Default)]
pub struct PendingSelection {
    current: Option<(Flow, Stage)>,
}

impl PendingSelection {
    #[cfg(test)]
    pub fn is_pending(&self) -> bool {
        self.current.is_some()
    }

    /// Claims the slot; `Err` when another flow is pending (the caller refuses the command).
    pub fn begin(&mut self, flow: Flow) -> Result<(), ()> {
        if self.current.is_some() {
            return Err(());
        }
        self.current = Some((flow, Stage::Listing));
        Ok(())
    }

    /// The listing finished: true when `flow` is the pending one and now waits for the overlay.
    /// A stale message (flow cancelled meanwhile) returns false and changes nothing.
    pub fn listed(&mut self, flow: Flow) -> bool {
        match self.current {
            Some((f, Stage::Listing)) if f == flow => {
                self.current = Some((flow, Stage::Selecting));
                true
            }
            _ => false,
        }
    }

    /// The selection arrived (a `Screen` flow has none and skips the overlay): true when `flow` is
    /// the pending one; the slot is free again.
    pub fn finish(&mut self, flow: Flow) -> bool {
        match self.current {
            Some((f, _)) if f == flow => {
                self.current = None;
                true
            }
            _ => false,
        }
    }

    /// Drops the pending flow (failure or cancel).
    pub fn cancel(&mut self) {
        self.current = None;
    }
}

/// Sentence when a command arrives while a selection is pending.
pub const SELECTION_BUSY: &str = "A selection is already in progress. Press Esc to cancel it.";

/// WORKER thread: ask the helper what can be captured (up to 10 s). Never call this on the main
/// thread: the overlay is started afterwards from the main loop (`selector::start`).
pub fn list_shareable(exe: &Path, log: impl Fn(&str) + Send + 'static) -> Result<Shareable, Outcome> {
    use ferrocast_core::capture::protocol::{parse_body, Command};
    use ferrocast_macos::capture::{helper_path, timeout_for, Helper, ScreenshotError};
    let mut helper = Helper::spawn(&helper_path(exe), log).map_err(|e| Outcome::Failed(e.to_string()))?;
    let body = helper.call(Command::Shareable, timeout_for(&Command::Shareable)).map_err(|e| match ScreenshotError::from(e) {
        ScreenshotError::PermissionDenied => Outcome::PermissionDenied,
        other => Outcome::Failed(other.to_string()),
    })?;
    parse_body(&body).map_err(Outcome::Failed)
}

/// Overlay start mode per capture kind: Capture Window starts in window mode (S5); Screen has no overlay.
pub fn select_mode(kind: CaptureKind) -> Option<ferrocast_macos::selector::Mode> {
    use ferrocast_macos::selector::Mode;
    match kind {
        CaptureKind::Area | CaptureKind::Scrolling => Some(Mode::Area),
        CaptureKind::Window => Some(Mode::Window),
        CaptureKind::Screen => None,
    }
}

/// Copy Text result (S8): the text to put on the clipboard (`None`: leave it alone) and the toast.
/// A Vision failure toasts its own text.
pub fn ocr_outcome(result: Result<Vec<ferrocast_core::capture::ocr::TextBox>, String>) -> (Option<String>, String) {
    use ferrocast_core::capture::ocr;
    match result {
        Ok(boxes) => {
            let text = ocr::join(&boxes);
            let text = text.trim().to_string();
            let toast = ocr::toast(&text);
            (if text.is_empty() { None } else { Some(text) }, toast)
        }
        Err(e) => (None, e),
    }
}

/// Background thread: recognise the text of `file` (nothing leaves the Mac).
pub fn recognize(file: &Path) -> (Option<String>, String) {
    ocr_outcome(ferrocast_macos::vision::recognize_text(file))
}

/// Pin arguments (S9): pixel size and the scale of the capture's display (2.0 when unknown or
/// not a positive number). `None` for an empty image.
pub fn pin_args(size: (u32, u32), scale: Option<f64>) -> Option<((u32, u32), f64)> {
    if size.0 == 0 || size.1 == 0 {
        return None;
    }
    Some((size, scale.filter(|s| s.is_finite() && *s > 0.0).unwrap_or(2.0)))
}

/// Background thread: take the picture into `dir` (the helper leaves no partial file on failure).
pub fn shoot(exe: &Path, target: Target, dir: &Path, cursor: bool, log: impl Fn(&str) + Send + 'static) -> Outcome {
    use ferrocast_macos::capture::{screenshot, ScreenshotError};
    if let Err(e) = std::fs::create_dir_all(dir) {
        return Outcome::Failed(format!("cannot create {}: {e}", dir.display()));
    }
    // let the selection overlay and the launcher finish disappearing
    std::thread::sleep(std::time::Duration::from_millis(250));
    let path = save_path(dir, chrono::Local::now().naive_local());
    match screenshot(exe, log, target, &path, cursor) {
        Ok(_) => Outcome::Saved(path),
        Err(ScreenshotError::PermissionDenied) => Outcome::PermissionDenied,
        Err(e) => Outcome::Failed(e.to_string()),
    }
}

// ---- Capture Scrolling (S10): pure decisions around the driver ----

/// Pause after each scroll so the content (and any smooth scrolling) settles before the next strip.
pub const SCROLL_SETTLE_MS: u64 = 450;

/// Wheel pixels to post for an area `height` points tall (negative: reveal what is below).
pub fn scroll_delta(height: f64) -> i32 {
    let px = (height * 0.8).round().clamp(1.0, 2000.0);
    -(px as i32)
}

/// Strips the driver may take: one more than [`MAX_STEPS`], so the stitcher can see that the cap was hit.
pub fn strip_budget() -> usize {
    MAX_STEPS + 1
}

/// What the driver does after a strip was captured.
#[derive(Debug, PartialEq, Eq)]
pub enum Step {
    Continue,
    Finish,
    Cancel,
}

/// `captured` strips so far (the newest included); `identical` when the newest equals the one before.
pub fn next_step(captured: usize, identical: bool, cancelled: bool) -> Step {
    if cancelled {
        Step::Cancel
    } else if identical || captured >= strip_budget() {
        Step::Finish
    } else {
        Step::Continue
    }
}

/// Toast for how the stitcher stopped (`None`: nothing to say).
pub fn scroll_toast(stop: StopReason, used: usize) -> Option<String> {
    match stop {
        StopReason::StoppedAtCap => Some(format!("Scrolling capture stopped at {MAX_STEPS} screens")),
        StopReason::LowConfidence { at } => Some(format!(
            "Scrolling capture lost track at screen {}; saved the {used} screen{} captured before it",
            at + 1,
            if used == 1 { "" } else { "s" }
        )),
        StopReason::EndOfScroll | StopReason::Exhausted => None,
    }
}

/// The area a scrolling capture works on.
#[derive(Debug, PartialEq)]
pub struct ScrollArea {
    pub target: Target,
    /// Centre in global top-left points: where the scroll events are posted.
    pub centre: (f64, f64),
    /// Height in points.
    pub height: f64,
}

pub fn scroll_area(selection: Option<Selection>, shareable: &Shareable) -> Option<ScrollArea> {
    let Some(Selection::Area(r)) = selection else { return None };
    let pointer = Point { x: 0.0, y: 0.0 };
    match capture_plan(CaptureKind::Area, selection, pointer, shareable) {
        Plan::Shoot(target) => Some(ScrollArea { target, centre: (r.x + r.w / 2.0, r.y + r.h / 2.0), height: r.h }),
        Plan::Nothing => None,
    }
}

/// Sentence for a failed scroll.
pub fn scroll_error_message(e: ferrocast_macos::scroll::ScrollError) -> String {
    use ferrocast_macos::scroll::ScrollError;
    match e {
        ScrollError::NotTrusted => "Scrolling capture needs Accessibility. Turn Ferrocast on in System Settings → Privacy & Security → Accessibility, then try again.".into(),
        ScrollError::EventFailed => "Scrolling capture could not scroll the page".into(),
    }
}

/// Sleeps `ms`, returning true when Esc was pressed meanwhile.
fn sleep_unless_escape(ms: u64) -> bool {
    let end = std::time::Instant::now() + std::time::Duration::from_millis(ms);
    while std::time::Instant::now() < end {
        if ferrocast_macos::scroll::escape_pressed() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    false
}

/// One strip of `target` as RGBA (through a temporary PNG that is always removed).
fn capture_strip(exe: &Path, target: &Target, n: usize, log: &(impl Fn(&str) + Clone + Send + 'static)) -> Result<Strip, Outcome> {
    use ferrocast_macos::capture::{screenshot, ScreenshotError};
    let tmp = std::env::temp_dir().join(format!("ferrocast-scroll-{}-{n}.png", std::process::id()));
    let shot = screenshot(exe, log.clone(), target.clone(), &tmp, false);
    let decoded = match shot {
        Ok(_) => image::open(&tmp).map_err(|e| Outcome::Failed(e.to_string())).and_then(|img| {
            let rgba = img.to_rgba8();
            let (w, h) = (rgba.width() as usize, rgba.height() as usize);
            Strip::new(w, h, rgba.into_raw()).map_err(|e| Outcome::Failed(e.to_string()))
        }),
        Err(ScreenshotError::PermissionDenied) => Err(Outcome::PermissionDenied),
        Err(e) => Err(Outcome::Failed(e.to_string())),
    };
    let _ = std::fs::remove_file(&tmp);
    decoded
}

/// Background thread: scroll `area` step by step, capture a strip per step, stitch and save into
/// `dir`. Returns the outcome and an optional toast (cap, lost track). Esc cancels with no file.
pub fn scroll_capture(exe: &Path, area: &ScrollArea, dir: &Path, log: impl Fn(&str) + Clone + Send + 'static) -> (Outcome, Option<String>) {
    use ferrocast_core::capture::stitch::stitch;
    let fail = |m: String| (Outcome::Failed(m), None);
    if let Err(e) = std::fs::create_dir_all(dir) {
        return fail(format!("cannot create {}: {e}", dir.display()));
    }
    // let the selection overlay and the launcher finish disappearing
    if sleep_unless_escape(250) {
        return (Outcome::Cancelled, None);
    }
    let delta = scroll_delta(area.height);
    let mut strips: Vec<Strip> = Vec::new();
    loop {
        let strip = match capture_strip(exe, &area.target, strips.len(), &log) {
            Ok(s) => s,
            Err(o) => return (o, None),
        };
        let identical = strips.last() == Some(&strip);
        strips.push(strip);
        match next_step(strips.len(), identical, ferrocast_macos::scroll::escape_pressed()) {
            Step::Cancel => return (Outcome::Cancelled, None),
            Step::Finish => break,
            Step::Continue => {}
        }
        if let Err(e) = ferrocast_macos::scroll::post_scroll(area.centre.0, area.centre.1, delta) {
            return fail(scroll_error_message(e));
        }
        if sleep_unless_escape(SCROLL_SETTLE_MS) {
            return (Outcome::Cancelled, None);
        }
    }
    let stitched = match stitch(&strips) {
        Ok(s) => s,
        Err(e) => return fail(e.to_string()),
    };
    let toast = scroll_toast(stitched.stop, stitched.used);
    let path = save_path(dir, chrono::Local::now().naive_local());
    let partial = path.with_extension("png.partial");
    let img = &stitched.image;
    let saved = image::RgbaImage::from_raw(img.width as u32, img.height as u32, img.rgba.clone())
        .ok_or_else(|| "stitched image has a bad size".to_string())
        .and_then(|buf| buf.save_with_format(&partial, image::ImageFormat::Png).map_err(|e| e.to_string()))
        .and_then(|_| std::fs::rename(&partial, &path).map_err(|e| e.to_string()));
    match saved {
        Ok(()) => (Outcome::Saved(path), toast),
        Err(e) => {
            let _ = std::fs::remove_file(&partial);
            fail(e)
        }
    }
}

/// The Capture window's buttons (S7), in display order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResultButton {
    Copy,
    Save,
    Annotate,
    Background,
    CopyText,
    Pin,
}

/// One button of the Capture window: label, the shortcut it shows and the key that runs it.
#[derive(Clone, Copy, Debug)]
pub struct ButtonSpec {
    pub id: ResultButton,
    pub label: &'static str,
    pub hint: &'static str,
    pub shortcut: Option<(egui::Modifiers, egui::Key)>,
}

pub fn result_buttons() -> Vec<ButtonSpec> {
    use egui::{Key, Modifiers};
    let cmd = Modifiers::COMMAND;
    let cmd_shift = Modifiers { shift: true, ..Modifiers::COMMAND };
    let spec = |id, label, hint, shortcut| ButtonSpec { id, label, hint, shortcut: Some(shortcut) };
    vec![
        spec(ResultButton::Copy, "Copy", "↵", (Modifiers::NONE, Key::Enter)),
        spec(ResultButton::Save, "Save", "⌘S", (cmd, Key::S)),
        spec(ResultButton::Annotate, "Annotate…", "⌘⇧A", (cmd_shift, Key::A)),
        spec(ResultButton::Background, "Background…", "⌘⇧B", (cmd_shift, Key::B)),
        spec(ResultButton::CopyText, "Copy Text", "⌘T", (cmd, Key::T)),
        spec(ResultButton::Pin, "Pin", "⌘P", (cmd, Key::P)),
    ]
}

/// The button a key press runs (Enter included: Copy and close).
pub fn result_shortcut(m: egui::Modifiers, key: egui::Key) -> Option<ResultButton> {
    result_buttons().into_iter().find(|b| b.shortcut == Some((m, key))).map(|b| b.id)
}

/// Every button needs the saved file; nothing is enabled once it is gone.
pub fn button_enabled(_b: ResultButton, file_exists: bool) -> bool {
    file_exists
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn disp(id: u32, frame: [f64; 4], main: bool) -> HelperDisplay {
        HelperDisplay { id, frame, scale: 2.0, main, name: String::new() }
    }
    fn win(id: u32, layer: i32, on_screen: bool) -> HelperWindow {
        HelperWindow {
            id,
            app: String::new(),
            bundle: String::new(),
            title: String::new(),
            frame: [1.0, 2.0, 30.0, 40.0],
            layer,
            on_screen,
        }
    }
    fn two() -> Shareable {
        Shareable {
            displays: vec![disp(1, [0.0, 0.0, 1000.0, 800.0], true), disp(2, [1000.0, -200.0, 1200.0, 900.0], false)],
            windows: vec![],
        }
    }

    #[test]
    fn availability_by_os_and_helper() {
        assert_eq!(availability(14, true), Availability::Available);
        assert_eq!(availability(27, true), Availability::Available);
        assert_eq!(availability(13, true), Availability::NeedsMacOs14(13));
        assert_eq!(availability(13, false), Availability::NeedsMacOs14(13), "the OS sentence wins");
        assert_eq!(availability(14, false), Availability::NoHelper);
    }

    #[test]
    fn s3_messages_are_exact() {
        assert_eq!(availability(13, true).message().as_deref(), Some("Screen capture needs macOS 14. This Mac runs macOS 13."));
        assert_eq!(availability(14, false).message().as_deref(), Some("Screen capture is not available in this build."));
        assert_eq!(availability(15, true).message(), None);
    }

    #[test]
    fn gate_says_once_per_session() {
        let mut s = CaptureState::with(PathBuf::new(), Availability::NeedsMacOs14(13));
        assert_eq!(s.gate(), Gate::Say("Screen capture needs macOS 14. This Mac runs macOS 13.".into()));
        assert_eq!(s.gate(), Gate::Quiet);
        assert_eq!(CaptureState::with(PathBuf::new(), Availability::Available).gate(), Gate::Go);
    }

    #[test]
    fn command_ids_round_trip() {
        for k in CaptureKind::ALL {
            assert_eq!(CaptureKind::from_command_id(k.command_id()), Some(k));
        }
        assert_eq!(CaptureKind::from_command_id("builtin:notes"), None);
    }

    #[test]
    fn selector_windows_keep_order_and_drop_hidden_and_non_normal() {
        let ws = selector_windows(&[win(1, 0, true), win(2, 0, false), win(3, 25, true), win(4, 0, true)]);
        assert_eq!(ws.iter().map(|w| w.id).collect::<Vec<_>>(), [1, 4]);
        assert_eq!(ws[0].frame, Rect::new(1.0, 2.0, 30.0, 40.0));
    }

    #[test]
    fn area_becomes_display_local_rect_on_its_display() {
        let sel = Selection::Area(Rect::new(1100.0, -100.0, 300.0, 200.0));
        let plan = capture_plan(CaptureKind::Area, Some(sel), Point { x: 0.0, y: 0.0 }, &two());
        assert_eq!(plan, Plan::Shoot(Target::Rect { display: 2, rect: [100.0, 100.0, 300.0, 200.0] }));
    }

    #[test]
    fn window_selection_targets_the_window_and_cancel_is_nothing() {
        let p = Point { x: 0.0, y: 0.0 };
        assert_eq!(capture_plan(CaptureKind::Window, Some(Selection::Window(77)), p, &two()), Plan::Shoot(Target::Window { window: 77 }));
        assert_eq!(capture_plan(CaptureKind::Area, Some(Selection::Cancelled), p, &two()), Plan::Nothing);
        assert_eq!(capture_plan(CaptureKind::Area, None, p, &two()), Plan::Nothing);
    }

    #[test]
    fn screen_takes_the_display_under_the_pointer_else_main() {
        let under = capture_plan(CaptureKind::Screen, None, Point { x: 1500.0, y: 0.0 }, &two());
        assert_eq!(under, Plan::Shoot(Target::Display { display: 2 }));
        let off = capture_plan(CaptureKind::Screen, None, Point { x: -5000.0, y: 0.0 }, &two());
        assert_eq!(off, Plan::Shoot(Target::Display { display: 1 }));
        let none = Shareable { displays: vec![], windows: vec![] };
        assert_eq!(capture_plan(CaptureKind::Screen, None, Point { x: 0.0, y: 0.0 }, &none), Plan::Nothing);
    }

    #[test]
    fn save_path_uses_local_name_and_avoids_collisions() {
        let dir = std::env::temp_dir().join(format!("fc-capture-ctl-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let at = NaiveDate::from_ymd_opt(2026, 10, 23).unwrap().and_hms_opt(14, 5, 22).unwrap();
        let first = save_path(&dir, at);
        assert_eq!(first, dir.join("Capture 2026-10-23 at 14.05.22.png"));
        std::fs::write(&first, b"x").unwrap();
        assert_eq!(save_path(&dir, at), dir.join("Capture 2026-10-23 at 14.05.22 2.png"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn after_routing_follows_settings() {
        assert_eq!(after_action(AfterCapture::Show), AfterAction::Show);
        assert_eq!(after_action(AfterCapture::Copy), AfterAction::CopyImage);
        assert_eq!(after_action(AfterCapture::Save), AfterAction::Nothing);
    }

    #[test]
    fn s4_toast_is_exact_and_cancel_is_silent() {
        assert_eq!(
            outcome_toast(&Outcome::PermissionDenied).as_deref(),
            Some("Screen Recording is off for Ferrocast. Turn it on in System Settings → Privacy & Security → Screen & System Audio Recording, then try again.")
        );
        assert_eq!(outcome_toast(&Outcome::Cancelled), None);
        assert_eq!(outcome_toast(&Outcome::Saved(PathBuf::from("/x.png"))), None);
        assert_eq!(outcome_toast(&Outcome::Failed("boom".into())).as_deref(), Some("Capture failed: boom"));
        assert_eq!(SCREEN_RECORDING_PANE, "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture");
    }

    #[test]
    fn s7_result_buttons_are_the_six_in_order_with_the_plan_shortcuts() {
        let b = result_buttons();
        let labels: Vec<_> = b.iter().map(|x| x.label).collect();
        assert_eq!(labels, ["Copy", "Save", "Annotate…", "Background…", "Copy Text", "Pin"]);
        let hints: Vec<_> = b.iter().map(|x| x.hint).collect();
        assert_eq!(hints, ["↵", "⌘S", "⌘⇧A", "⌘⇧B", "⌘T", "⌘P"]);
    }

    #[test]
    fn s7_shortcuts_map_to_buttons_and_nothing_else_does() {
        use egui::{Key, Modifiers};
        let cmd = Modifiers::COMMAND;
        let cs = Modifiers { shift: true, ..Modifiers::COMMAND };
        assert_eq!(result_shortcut(Modifiers::NONE, Key::Enter), Some(ResultButton::Copy));
        assert_eq!(result_shortcut(cmd, Key::S), Some(ResultButton::Save));
        assert_eq!(result_shortcut(cmd, Key::T), Some(ResultButton::CopyText));
        assert_eq!(result_shortcut(cmd, Key::P), Some(ResultButton::Pin));
        assert_eq!(result_shortcut(cs, Key::A), Some(ResultButton::Annotate));
        assert_eq!(result_shortcut(cs, Key::B), Some(ResultButton::Background));
        assert_eq!(result_shortcut(Modifiers::NONE, Key::S), None, "a bare letter is not a shortcut");
        assert_eq!(result_shortcut(cs, Key::S), None, "extra shift is a different chord");
        assert_eq!(result_shortcut(cmd, Key::Enter), None);
    }

    #[test]
    fn s7_buttons_need_the_saved_file() {
        assert_eq!(result_buttons().len(), 6);
        for b in result_buttons() {
            assert!(button_enabled(b.id, true), "{:?}", b.id);
            assert!(!button_enabled(b.id, false), "{:?}", b.id);
        }
    }

    #[test]
    fn selector_mode_follows_capture_kind() {
        use ferrocast_macos::selector::Mode;
        assert_eq!(select_mode(CaptureKind::Area), Some(Mode::Area));
        assert_eq!(select_mode(CaptureKind::Window), Some(Mode::Window));
        assert_eq!(select_mode(CaptureKind::Screen), None);
    }

    fn tb(text: &str, y: f64) -> ferrocast_core::capture::ocr::TextBox {
        ferrocast_core::capture::ocr::TextBox { text: text.into(), x: 0.0, y, w: 0.5, h: 0.05, confidence: 1.0 }
    }

    #[test]
    fn ocr_outcome_copies_text_and_counts_characters() {
        let (copy, toast) = ocr_outcome(Ok(vec![tb("héllo", 0.1), tb("ab", 0.5)]));
        let text = copy.expect("text to copy");
        assert!(text.contains("héllo") && text.contains("ab"));
        assert_eq!(toast, format!("Copied {} characters", text.chars().count()));
    }

    #[test]
    fn ocr_outcome_without_text_leaves_clipboard_alone() {
        assert_eq!(ocr_outcome(Ok(vec![])), (None, "No text found".to_string()));
        assert_eq!(ocr_outcome(Ok(vec![tb("   ", 0.1)])), (None, "No text found".to_string()));
    }

    #[test]
    fn ocr_outcome_failure_toasts_the_error_text() {
        assert_eq!(ocr_outcome(Err("inference context".into())), (None, "inference context".to_string()));
    }

    #[test]
    fn pin_args_default_to_retina_and_reject_bad_input() {
        assert_eq!(pin_args((800, 600), None), Some(((800, 600), 2.0)));
        assert_eq!(pin_args((800, 600), Some(1.0)), Some(((800, 600), 1.0)));
        assert_eq!(pin_args((800, 600), Some(0.0)), Some(((800, 600), 2.0)));
        assert_eq!(pin_args((800, 600), Some(f64::NAN)), Some(((800, 600), 2.0)));
        assert_eq!(pin_args((0, 600), Some(2.0)), None);
    }

    #[test]
    fn scroll_delta_is_most_of_the_area_height_downwards() {
        assert_eq!(scroll_delta(500.0), -400);
        assert_eq!(scroll_delta(1000.0), -800);
        assert_eq!(scroll_delta(1.0), -1, "never zero");
        assert_eq!(scroll_delta(0.0), -1);
        assert_eq!(scroll_delta(100000.0), -2000, "clamped");
        assert!(scroll_delta(480.0) < 0 && (-scroll_delta(480.0)) < 480, "leaves overlap to stitch");
    }

    #[test]
    fn strip_budget_is_one_over_the_cap() {
        assert_eq!(strip_budget(), MAX_STEPS + 1);
        assert_eq!(MAX_STEPS, 60);
    }

    #[test]
    fn next_step_follows_stop_rules() {
        assert_eq!(next_step(1, false, false), Step::Continue);
        assert_eq!(next_step(5, true, false), Step::Finish, "two identical strips: end of scroll");
        assert_eq!(next_step(MAX_STEPS, false, false), Step::Continue);
        assert_eq!(next_step(MAX_STEPS + 1, false, false), Step::Finish, "cap reached");
        assert_eq!(next_step(3, false, true), Step::Cancel);
        assert_eq!(next_step(3, true, true), Step::Cancel, "cancel wins");
    }

    #[test]
    fn scroll_toasts_are_exact_and_silent_when_fine() {
        assert_eq!(scroll_toast(StopReason::StoppedAtCap, 60).as_deref(), Some("Scrolling capture stopped at 60 screens"));
        assert_eq!(scroll_toast(StopReason::EndOfScroll, 4), None);
        assert_eq!(scroll_toast(StopReason::Exhausted, 4), None);
        let low = scroll_toast(StopReason::LowConfidence { at: 6 }, 6).expect("a clear toast");
        assert!(low.contains("screen 7") && low.contains("saved"), "{low}");
    }

    #[test]
    fn scroll_area_is_display_local_target_with_global_centre() {
        let sel = Selection::Area(Rect::new(1100.0, -100.0, 300.0, 200.0));
        let a = scroll_area(Some(sel), &two()).expect("area");
        assert_eq!(a.target, Target::Rect { display: 2, rect: [100.0, 100.0, 300.0, 200.0] });
        assert_eq!(a.centre, (1250.0, 0.0));
        assert_eq!(a.height, 200.0);
        assert_eq!(scroll_area(Some(Selection::Cancelled), &two()), None);
        assert_eq!(scroll_area(Some(Selection::Window(3)), &two()), None, "windows are not scrolled");
        assert_eq!(scroll_area(None, &two()), None);
        assert_eq!(scroll_area(Some(Selection::Area(Rect::new(-9000.0, 0.0, 10.0, 10.0))), &two()), None);
    }

    #[test]
    fn scroll_errors_say_how_to_fix_accessibility() {
        use ferrocast_macos::scroll::ScrollError;
        let m = scroll_error_message(ScrollError::NotTrusted);
        assert!(m.contains("Accessibility") && m.contains("System Settings"), "{m}");
        assert!(!scroll_error_message(ScrollError::EventFailed).is_empty());
    }

    #[test]
    fn scrolling_is_a_capture_command_with_the_same_gating() {
        assert_eq!(CaptureKind::Scrolling.command_id(), "builtin:capture-scrolling");
        assert_eq!(CaptureKind::from_command_id("builtin:capture-scrolling"), Some(CaptureKind::Scrolling));
        assert_eq!(select_mode(CaptureKind::Scrolling), Some(ferrocast_macos::selector::Mode::Area));
    }

    #[test]
    fn one_pending_selection_at_a_time() {
        use crate::record_ctl::RecordKind;
        let mut p = PendingSelection::default();
        assert!(!p.is_pending());
        assert_eq!(p.begin(Flow::Capture(CaptureKind::Area)), Ok(()));
        assert!(p.is_pending());
        assert_eq!(p.begin(Flow::Capture(CaptureKind::Screen)), Err(()), "a second command while pending is refused");
        assert_eq!(p.begin(Flow::Record(RecordKind::Area)), Err(()), "also across capture and record");
        assert!(p.is_pending(), "the refused command left the first one alone");
        assert!(p.finish(Flow::Capture(CaptureKind::Area)));
        assert_eq!(p.begin(Flow::Record(RecordKind::Area)), Ok(()), "free again once the first finished");
    }

    #[test]
    fn cancel_clears_the_pending_flow_and_makes_late_messages_stale() {
        let area = Flow::Capture(CaptureKind::Area);
        let mut p = PendingSelection::default();
        p.begin(area).unwrap();
        p.cancel();
        assert!(!p.is_pending());
        assert!(!p.listed(area), "the listing that comes back after a cancel is ignored");
        assert!(!p.finish(area), "so is a selection");
        p.cancel();
        assert!(!p.is_pending());
    }

    #[test]
    fn a_flow_goes_listing_then_selecting_then_done_in_that_order() {
        let (area, window) = (Flow::Capture(CaptureKind::Area), Flow::Capture(CaptureKind::Window));
        let mut p = PendingSelection::default();
        p.begin(area).unwrap();
        assert!(!p.listed(window), "another flow's listing is not ours");
        assert!(p.listed(area));
        assert!(!p.listed(area), "the listing is only accepted once");
        assert!(!p.finish(window));
        assert!(p.is_pending());
        assert!(p.finish(area));
        assert!(!p.finish(area), "a selection is only accepted once");
    }

    #[test]
    fn a_flow_maps_to_the_capture_kind_that_picks_its_overlay() {
        use crate::record_ctl::RecordKind;
        assert_eq!(Flow::Capture(CaptureKind::Window).capture_kind(), CaptureKind::Window);
        assert_eq!(select_mode(Flow::Record(RecordKind::Window).capture_kind()), Some(ferrocast_macos::selector::Mode::Window));
        assert_eq!(select_mode(Flow::Record(RecordKind::Screen).capture_kind()), None, "Record Screen shows no overlay");
    }

    #[test]
    fn a_cancelled_selection_plans_nothing_for_every_flow() {
        let sh = two();
        for kind in [CaptureKind::Area, CaptureKind::Window, CaptureKind::Scrolling] {
            assert_eq!(capture_plan(kind, Some(Selection::Cancelled), Point { x: 0.0, y: 0.0 }, &sh), Plan::Nothing, "{kind:?}");
        }
        assert_eq!(scroll_area(Some(Selection::Cancelled), &sh), None);
    }
}
