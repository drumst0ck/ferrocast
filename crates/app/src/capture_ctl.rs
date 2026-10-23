//! Screenshot commands (Capture Area / Window / Screen, S1 S3 S4 S6): which commands exist, what
//! a stored shortcut says when they do not, how a selection becomes a helper target, where the
//! file goes and what happens after. Everything that decides is a pure function; the AppKit and
//! helper calls are thin shells around them.

use chrono::NaiveDateTime;
use ferrocast_core::capture::protocol::{Display as HelperDisplay, Shareable, Target, Window as HelperWindow};
use ferrocast_core::capture::region::{capture_file_name, Point, Rect, Window};
use ferrocast_core::capture::settings::AfterCapture;
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
}

impl CaptureKind {
    pub const ALL: [CaptureKind; 3] = [CaptureKind::Area, CaptureKind::Window, CaptureKind::Screen];

    /// The command id of the matching launcher builtin.
    pub fn command_id(self) -> &'static str {
        match self {
            CaptureKind::Area => "builtin:capture-area",
            CaptureKind::Window => "builtin:capture-window",
            CaptureKind::Screen => "builtin:capture-screen",
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
        let _ = std::process::Command::new("/usr/bin/open").arg(SCREEN_RECORDING_PANE).status();
    }
}

fn pointer_top_left(mtm: objc2::MainThreadMarker) -> Point {
    let primary_h = objc2_app_kit::NSScreen::screens(mtm).iter().next().map(|s| s.frame().size.height).unwrap_or(0.0);
    let m = objc2_app_kit::NSEvent::mouseLocation();
    ferrocast_core::capture::region::flip_point(Point { x: m.x, y: m.y }, primary_h)
}

/// Main thread: ask the helper what can be captured, let the user choose (Area and Window) and
/// return the target. `Err` is the finished outcome (cancel, permission, failure).
pub fn choose_target(
    exe: &Path,
    kind: CaptureKind,
    mtm: objc2::MainThreadMarker,
    log: impl Fn(&str) + Send + 'static,
) -> Result<Target, Outcome> {
    use ferrocast_core::capture::protocol::{parse_body, Command};
    use ferrocast_macos::capture::{helper_path, timeout_for, Helper, ScreenshotError};
    let fail = |e: ScreenshotError| match e {
        ScreenshotError::PermissionDenied => Outcome::PermissionDenied,
        other => Outcome::Failed(other.to_string()),
    };
    let mut helper = Helper::spawn(&helper_path(exe), log).map_err(|e| Outcome::Failed(e.to_string()))?;
    let body = helper.call(Command::Shareable, timeout_for(&Command::Shareable)).map_err(|e| fail(e.into()))?;
    let shareable: Shareable = parse_body(&body).map_err(Outcome::Failed)?;
    drop(helper);
    let selection = match kind {
        CaptureKind::Screen => None,
        // T7 gap: `selector::select` has no start-in-window-mode parameter, so Capture Window
        // opens the same overlay and the user presses Space (S5).
        CaptureKind::Area | CaptureKind::Window => Some(ferrocast_macos::selector::select(mtm, selector_windows(&shareable.windows))),
    };
    match capture_plan(kind, selection, pointer_top_left(mtm), &shareable) {
        Plan::Shoot(t) => Ok(t),
        Plan::Nothing => Err(Outcome::Cancelled),
    }
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
}
