//! Pure decisions for "Enter on a Finder selection" (opt-in).
//!
//! The macOS event tap (`ferrocast-macos::finder_enter`) only gathers facts and asks these
//! functions what to do, so every rule that decides whether a Return key press is swallowed is
//! testable without a session. Anything not explicitly allowed here passes through untouched.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub const FINDER_BUNDLE: &str = "com.apple.finder";

/// Focused-element roles that mean "a file selection" in Finder (list, column, icon views).
pub const SELECTION_ROLES: [&str; 5] = ["AXList", "AXOutline", "AXBrowser", "AXScrollArea", "AXImage"];

/// Marker in the CGEvent user-data field of the Return we re-post. The tap always lets an event
/// carrying it through, so a re-posted Return can never be intercepted again. Distinct from
/// `keyboard::SYNTHETIC_MARKER`.
pub const REPOST_MARKER: i64 = 0x00F3_CA58;

/// Subrole of an ordinary document window; sheets, dialogs, panels (Quick Look) differ.
pub const NORMAL_WINDOW_SUBROLE: &str = "AXStandardWindow";

const MOD_SHIFT: u64 = 0x0002_0000;
const MOD_CONTROL: u64 = 0x0004_0000;
const MOD_OPTION: u64 = 0x0008_0000;
const MOD_COMMAND: u64 = 0x0010_0000;

/// Should a Return key press be swallowed and turned into "open the Convert wheel"?
pub fn should_intercept(frontmost_bundle: Option<&str>, focused_role: Option<&str>, mods_empty: bool, enabled: bool) -> bool {
    enabled && mods_empty && frontmost_bundle == Some(FINDER_BUNDLE) && focused_role.is_some_and(|r| SELECTION_ROLES.contains(&r))
}

/// The cheap checks done before any Accessibility call: the setting is on, Finder is frontmost,
/// it is a key-down that is not an auto-repeat, has no modifiers and is not our own re-post.
pub fn pre_check(user_data: i64, key_down: bool, autorepeat: bool, mods_empty: bool, enabled: bool, finder_front: bool) -> bool {
    !is_repost(user_data) && key_down && !autorepeat && mods_empty && enabled && finder_front
}

/// Does this event carry our re-post marker?
pub fn is_repost(user_data: i64) -> bool {
    user_data == REPOST_MARKER
}

/// True when none of shift, control, option, command is held (caps lock, fn etc. are ignored).
pub fn modifiers_empty(flags: u64) -> bool {
    flags & (MOD_SHIFT | MOD_CONTROL | MOD_OPTION | MOD_COMMAND) == 0
}

/// Only an ordinary window counts; a sheet, dialog, panel or an unknown subrole passes through.
pub fn window_is_normal(subrole: Option<&str>) -> bool {
    subrole == Some(NORMAL_WINDOW_SUBROLE)
}

/// After a re-post the tap passes every Return through for this long.
pub const REPOST_HOLD: Duration = Duration::from_millis(300);

/// Defense in depth: the focused element must belong to Finder's process. `finder_pid` is 0 when
/// Finder is not frontmost; `focused_pid` is `None` when it could not be read.
pub fn pid_ok(finder_pid: i32, focused_pid: Option<i32>) -> bool {
    finder_pid != 0 && focused_pid == Some(finder_pid)
}

/// One Return at a time: set by the tap when it swallows a key press, cleared when the handler
/// has finished. While set, further Returns pass through. After a re-post (`hold_from`) Returns
/// also pass through for `REPOST_HOLD`; a new re-post restarts that window.
pub struct InFlight {
    busy: AtomicBool,
    base: Instant,
    /// End of the hold window in nanoseconds since `base`; 0 = no window.
    hold_until: AtomicU64,
}

impl Default for InFlight {
    fn default() -> Self {
        InFlight { busy: AtomicBool::new(false), base: Instant::now(), hold_until: AtomicU64::new(0) }
    }
}

impl InFlight {
    pub fn try_begin(&self) -> bool {
        self.try_begin_at(Instant::now())
    }
    /// `try_begin` with an explicit clock reading (testable).
    pub fn try_begin_at(&self, now: Instant) -> bool {
        if self.in_hold(now) {
            return false;
        }
        self.busy.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_ok()
    }
    pub fn end(&self) {
        self.busy.store(false, Ordering::SeqCst);
    }
    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::SeqCst)
    }
    /// Start (or restart) the post-re-post pass-through window at `now`.
    pub fn hold_from(&self, now: Instant) {
        let until = now.saturating_duration_since(self.base) + REPOST_HOLD;
        self.hold_until.store((until.as_nanos() as u64).max(1), Ordering::SeqCst);
    }
    /// Is `now` inside the post-re-post pass-through window?
    pub fn in_hold(&self, now: Instant) -> bool {
        let until = self.hold_until.load(Ordering::SeqCst);
        until != 0 && (now.saturating_duration_since(self.base).as_nanos() as u64) < until
    }
}

/// What to do once the Finder selection is known.
#[derive(Debug, PartialEq, Eq)]
pub enum EnterOutcome {
    OpenWheel(Vec<PathBuf>),
    /// Give the Return back to Finder.
    Repost,
}

/// A non-empty selection opens the wheel; an empty one or a failed read gives the Return back.
pub fn outcome(selection: &Result<Vec<PathBuf>, String>) -> EnterOutcome {
    match selection {
        Ok(files) if !files.is_empty() => EnterOutcome::OpenWheel(files.clone()),
        _ => EnterOutcome::Repost,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALLOWED: [&str; 5] = ["AXList", "AXOutline", "AXBrowser", "AXScrollArea", "AXImage"];

    #[test]
    fn intercepts_only_finder_selection_roles_without_modifiers_when_enabled() {
        for role in ALLOWED {
            assert!(should_intercept(Some(FINDER_BUNDLE), Some(role), true, true), "{role}");
        }
        for role in ["AXTextField", "AXTextArea", "AXSearchField", "AXComboBox", "AXButton", "AXWindow", "AXGroup", "", "axlist", "AXList "]
        {
            assert!(!should_intercept(Some(FINDER_BUNDLE), Some(role), true, true), "{role:?} must pass through");
        }
        assert!(!should_intercept(Some(FINDER_BUNDLE), None, true, true), "unknown role");
        assert!(!should_intercept(None, Some("AXList"), true, true), "no frontmost app");
        for other in ["com.apple.Safari", "com.apple.finder.Helper", "com.apple.Finder", "", "ferrocast"] {
            assert!(!should_intercept(Some(other), Some("AXList"), true, true), "{other}");
        }
        assert!(!should_intercept(Some(FINDER_BUNDLE), Some("AXList"), false, true), "modifiers held");
        assert!(!should_intercept(Some(FINDER_BUNDLE), Some("AXList"), true, false), "setting off");
        assert!(!should_intercept(None, None, false, false));
    }

    #[test]
    fn roles_list_matches_the_documented_set() {
        assert_eq!(SELECTION_ROLES, ALLOWED);
    }

    #[test]
    fn pre_check_rejects_repeat_key_up_modifiers_off_setting_other_apps_and_reposts() {
        assert!(pre_check(0, true, false, true, true, true));
        assert!(!pre_check(0, true, true, true, true, true), "auto-repeat passes through");
        assert!(!pre_check(0, false, false, true, true, true), "key-up passes through");
        assert!(!pre_check(0, true, false, false, true, true), "modifiers");
        assert!(!pre_check(0, true, false, true, false, true), "disabled");
        assert!(!pre_check(0, true, false, true, true, false), "Finder not frontmost");
        assert!(!pre_check(REPOST_MARKER, true, false, true, true, true), "our own re-post is never intercepted");
    }

    #[test]
    fn repost_marker_is_recognised_and_ordinary_events_are_not() {
        assert!(is_repost(REPOST_MARKER));
        assert!(!is_repost(0), "a normal Return has no user data");
        assert!(!is_repost(0x00F3_CA57), "the snippet marker is a different one");
        assert!(!is_repost(-1));
        assert_ne!(REPOST_MARKER, 0);
    }

    #[test]
    fn only_shift_control_option_command_count_as_modifiers() {
        assert!(modifiers_empty(0));
        assert!(modifiers_empty(0x0001_0000), "caps lock alone");
        assert!(modifiers_empty(0x0080_0000), "fn alone");
        assert!(!modifiers_empty(0x0002_0000), "shift");
        assert!(!modifiers_empty(0x0004_0000), "control");
        assert!(!modifiers_empty(0x0008_0000), "option");
        assert!(!modifiers_empty(0x0010_0000), "command");
        assert!(!modifiers_empty(0x0001_0000 | 0x0010_0000), "caps lock plus command");
    }

    #[test]
    fn only_a_standard_window_counts() {
        assert!(window_is_normal(Some("AXStandardWindow")));
        for s in ["AXDialog", "AXSystemDialog", "AXFloatingWindow", "AXSystemFloatingWindow", "AXSheet", ""] {
            assert!(!window_is_normal(Some(s)), "{s:?}");
        }
        assert!(!window_is_normal(None), "unknown window passes through");
    }

    #[test]
    fn only_one_return_is_in_flight_at_a_time() {
        let f = InFlight::default();
        assert!(!f.is_busy());
        assert!(f.try_begin());
        assert!(f.is_busy());
        assert!(!f.try_begin(), "a second Return while one is handled is not taken");
        f.end();
        assert!(!f.is_busy());
        assert!(f.try_begin(), "free again after the handler finished");
    }

    #[test]
    fn selection_opens_the_wheel_and_everything_else_reposts() {
        let files = vec![PathBuf::from("/tmp/a.png")];
        assert_eq!(outcome(&Ok(files.clone())), EnterOutcome::OpenWheel(files));
        assert_eq!(outcome(&Ok(vec![])), EnterOutcome::Repost);
        assert_eq!(outcome(&Err("not allowed".into())), EnterOutcome::Repost);
    }

    #[test]
    fn pid_check_requires_a_known_matching_finder_pid() {
        assert!(pid_ok(123, Some(123)), "same process");
        assert!(!pid_ok(123, Some(456)), "another app's element");
        assert!(!pid_ok(0, Some(0)), "no Finder pid known");
        assert!(!pid_ok(0, Some(123)));
        assert!(!pid_ok(123, None), "pid read failed");
        assert!(!pid_ok(0, None));
    }

    #[test]
    fn returns_pass_through_for_300ms_after_a_repost_and_a_new_repost_restarts_the_window() {
        let f = InFlight::default();
        let t0 = Instant::now();
        assert!(!f.in_hold(t0), "no window before any re-post");
        f.hold_from(t0);
        assert!(f.in_hold(t0 + Duration::from_millis(1)));
        assert!(f.in_hold(t0 + Duration::from_millis(299)));
        assert!(!f.try_begin_at(t0 + Duration::from_millis(299)), "inside the window a Return is not taken");
        assert!(!f.is_busy(), "a refused Return leaves no busy flag");
        assert!(!f.in_hold(t0 + Duration::from_millis(300)), "after 300 ms the window is over");
        assert!(f.try_begin_at(t0 + Duration::from_millis(301)), "a Return is taken again after the window");
        f.end();
        // a new re-post restarts the window
        f.hold_from(t0 + Duration::from_millis(250));
        assert!(f.in_hold(t0 + Duration::from_millis(500)), "250+300 > 500: restarted window still open");
        assert!(!f.in_hold(t0 + Duration::from_millis(550)));
        assert!(f.try_begin_at(t0 + Duration::from_millis(551)));
    }
}
