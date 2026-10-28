//! Pure parts of the host calls an extension can make beyond clipboard / storage / open: reading the
//! selected text, applications and launching another command. The app does the I/O (AppleScript,
//! the app index); this module builds the scripts and checks the arguments, so both are testable.

use serde_json::{Map, Value};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A string as an AppleScript literal. Control characters (a newline would end the literal and
/// start new script text) and NUL are refused.
pub fn applescript_string(s: &str) -> Result<String, String> {
    if s.chars().any(char::is_control) {
        return Err("the text contains control characters".into());
    }
    Ok(format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")))
}

/// Host calls answered by a background thread that may be in flight at once, across every command
/// of the extension runtime. A command that asks for more is told to wait: it cannot pile up threads
/// (each can block for seconds). The count is shared and never reset: relaunching a command does not
/// give it a fresh budget; a slot comes back only when the thread that holds it finishes.
pub const MAX_PENDING_CALLS: usize = 8;

#[derive(Debug, Default, Clone)]
pub struct Pending(Arc<AtomicUsize>);

/// A taken slot; it is given back when dropped (i.e. when the thread holding it ends).
#[derive(Debug)]
pub struct Slot(Arc<AtomicUsize>);

impl Drop for Slot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

impl Pending {
    /// Take a slot; `None` when `MAX_PENDING_CALLS` are already in flight.
    pub fn acquire(&self) -> Option<Slot> {
        self.0.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| (n < MAX_PENDING_CALLS).then_some(n + 1)).ok()?;
        Some(Slot(self.0.clone()))
    }

    pub fn in_flight(&self) -> usize {
        self.0.load(Ordering::SeqCst)
    }
}

/// `launchCommand` may start at most `MAX_LAUNCHES` commands per `LAUNCH_WINDOW`.
pub const MAX_LAUNCHES: usize = 5;
pub const LAUNCH_WINDOW: Duration = Duration::from_secs(10);

#[derive(Debug, Default)]
pub struct LaunchLimiter(VecDeque<Instant>);

impl LaunchLimiter {
    /// Record a launch at `now`; false (nothing recorded) when the window is already full.
    pub fn allow(&mut self, now: Instant) -> bool {
        while self.0.front().is_some_and(|t| now.saturating_duration_since(*t) >= LAUNCH_WINDOW) {
            self.0.pop_front();
        }
        if self.0.len() >= MAX_LAUNCHES {
            return false;
        }
        self.0.push_back(now);
        true
    }
}

/// How long a view may stay hidden before its command is stopped: the launcher also forgets its
/// screens after this long (`show` resets to the root after 90 s).
pub const HIDDEN_VIEW_LIMIT: Duration = Duration::from_secs(90);

/// Calls that read what the user is doing outside the launcher (the selection of the app they came
/// from) or put a dialog in front of them. A command with a view has no business doing that while
/// its launcher is hidden; a no-view command runs hidden on purpose.
pub fn call_allowed(method: &str, is_view: bool, launcher_visible: bool) -> Result<(), String> {
    if is_view && !launcher_visible && matches!(method, "selection.text" | "alert.confirm") {
        return Err("not available while the launcher is hidden".into());
    }
    Ok(())
}

/// A command with a view whose launcher has been hidden for more than `HIDDEN_VIEW_LIMIT`.
pub fn view_expired(is_view: bool, launcher_visible: bool, hidden_for: Duration) -> bool {
    is_view && !launcher_visible && hidden_for > HIDDEN_VIEW_LIMIT
}

/// Start of the clock for a command about to be launched: a no-view command relaunched by a no-view
/// command (itself, typically) keeps the original start, so the no-view time limit covers the chain.
/// `prev` is `(is_view, started)` of the session being replaced.
pub fn carried_origin(prev: Option<(bool, Instant)>, new_is_view: bool, now: Instant) -> Instant {
    match prev {
        Some((false, t)) if !new_is_view => t,
        _ => now,
    }
}

/// AppleScript that returns the selected text of the focused element of process `pid` (the app the
/// user came from). Needs the Accessibility permission, like the launcher's other text features.
pub fn selection_script(pid: i32) -> String {
    format!(
        "tell application \"System Events\"\n\
         set p to first application process whose unix id is {pid}\n\
         set el to value of attribute \"AXFocusedUIElement\" of p\n\
         return value of attribute \"AXSelectedText\" of el\n\
         end tell"
    )
}

/// The selected text from `osascript` output (it adds a trailing newline), or Raycast's error.
pub fn parse_selection(out: &str) -> Result<String, String> {
    let t = out.strip_suffix('\n').unwrap_or(out);
    if t.trim().is_empty() || t == "missing value" {
        Err("Unable to get selected text".into())
    } else {
        Ok(t.to_string())
    }
}

/// AppleScript that returns the POSIX path of the default application for the file at `path`.
pub fn default_app_script(path: &str) -> Result<String, String> {
    if !path.starts_with('/') {
        return Err("getDefaultApplication needs an absolute file path".into());
    }
    let lit = applescript_string(path)?;
    Ok(format!(
        "set f to POSIX file {lit}\n\
         tell application \"System Events\" to set a to default application of (info for f)\n\
         return POSIX path of (a as alias)"
    ))
}

/// AppleScript for `confirmAlert`: a native alert with two buttons (the primary one is the default).
/// The script ends normally when the primary button is pressed; the other button makes `osascript`
/// fail (user cancelled), see `parse_confirm`. Text is escaped and capped.
pub fn confirm_script(params: &Value) -> Result<(String, String), String> {
    let get = |k: &str, max: usize, default: &str| -> Result<(String, String), String> {
        let raw: String =
            params.get(k).and_then(Value::as_str).unwrap_or("").chars().filter(|c| !c.is_control() || *c == '\n').take(max).collect();
        // newlines are fine inside the message but must not end the literal
        let mut clean = raw.replace('\n', " ");
        if clean.is_empty() && !default.is_empty() {
            clean = default.to_string();
        }
        Ok((applescript_string(&clean)?, clean))
    };
    let title = get("title", 200, "")?.0;
    let message = get("message", 600, "")?.0;
    let (primary_lit, primary) = get("primary", 40, "OK")?;
    let dismiss = get("dismiss", 40, "Cancel")?.0;
    let kind = if params.get("style").and_then(Value::as_str) == Some("destructive") { "critical" } else { "informational" };
    let script = format!(
        "display alert {title} message {message} as {kind} buttons {{{dismiss}, {primary_lit}}} default button {primary_lit} cancel button {dismiss}"
    );
    Ok((script, primary))
}

/// The user pressed the primary button when `osascript` ran `confirm_script` successfully and the
/// button it returned is exactly `primary` (the cleaned label `confirm_script` gave back).
pub fn parse_confirm(result: Result<String, ()>, primary: &str) -> bool {
    result.is_ok_and(|out| {
        out.lines()
            .find_map(|l| l.strip_prefix("button returned:"))
            .is_some_and(|rest| rest.split(", gave up:").next().is_some_and(|b| b == primary))
    })
}

/// The JSON of a Raycast `Application`.
pub fn application_json(name: &str, path: &str, bundle_id: Option<&str>) -> Value {
    let mut m = Map::new();
    m.insert("name".into(), name.into());
    m.insert("path".into(), path.into());
    if let Some(b) = bundle_id {
        m.insert("bundleId".into(), b.into());
    }
    Value::Object(m)
}

/// `Some("Foo")` for `/Applications/Foo.app/`.
pub fn app_name_from_path(path: &str) -> Option<String> {
    let p = path.trim_end_matches('/');
    let file = p.rsplit('/').next()?;
    let name = file.strip_suffix(".app").unwrap_or(file);
    (!name.is_empty()).then(|| name.to_string())
}

/// A `launchCommand` request: the command of the *same* extension to start, and its arguments.
/// Another extension (`extensionName` naming a different one, or `ownerOrAuthorName` that is not one
/// of the running extension's `owners`: its manifest owner / author) is refused.
pub fn parse_launch(params: &Value, current_ext: &str, owners: &[&str]) -> Result<(String, Map<String, Value>), String> {
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    if name.is_empty() || name.contains(['/', '\\', '\0']) || name == "." || name == ".." {
        return Err("launchCommand: invalid command name".into());
    }
    if let Some(e) = params.get("extensionName").and_then(Value::as_str) {
        if e != current_ext {
            return Err("launchCommand: launching another extension is not supported in Ferrocast yet".into());
        }
    }
    if let Some(o) = params.get("ownerOrAuthorName").and_then(Value::as_str).filter(|o| !o.is_empty()) {
        if !owners.contains(&o) {
            return Err("launchCommand: launching another extension is not supported in Ferrocast yet".into());
        }
    }
    let args = params.get("arguments").and_then(Value::as_object).cloned().unwrap_or_default();
    Ok((name.to_string(), args))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn applescript_strings_escape_quotes_and_refuse_newlines() {
        assert_eq!(applescript_string(r#"/tmp/a "b"\c"#).unwrap(), r#""/tmp/a \"b\"\\c""#);
        assert!(applescript_string("/tmp/a\nend tell").is_err());
        assert!(applescript_string("/tmp/a\0").is_err());
    }

    #[test]
    fn pending_calls_are_capped_and_a_slot_is_freed_only_when_its_thread_finishes() {
        let p = Pending::default();
        let mut slots: Vec<_> = (0..MAX_PENDING_CALLS).map(|_| p.acquire().expect("under the cap")).collect();
        assert!(p.acquire().is_none(), "one more than the cap is refused");
        // a clone (the state kept across a relaunch) sees the same count: no way to reset it
        assert!(p.clone().acquire().is_none());
        drop(slots.pop());
        assert_eq!(p.in_flight(), MAX_PENDING_CALLS - 1);
        let again = p.acquire();
        assert!(again.is_some());
        assert!(p.acquire().is_none());
        drop(again);
        drop(slots);
        assert_eq!(p.in_flight(), 0);
    }

    #[test]
    fn launch_command_is_rate_limited_per_window() {
        use std::time::{Duration, Instant};
        let mut l = LaunchLimiter::default();
        let t0 = Instant::now();
        for i in 0..MAX_LAUNCHES {
            assert!(l.allow(t0 + Duration::from_millis(i as u64)), "launch {i}");
        }
        assert!(!l.allow(t0 + Duration::from_secs(5)), "the sixth in the window is refused");
        assert!(!l.allow(t0 + Duration::from_secs(9)), "a refused one is not counted but the window is still full");
        assert!(l.allow(t0 + LAUNCH_WINDOW + Duration::from_secs(1)), "after the window it works again");
    }

    #[test]
    fn host_calls_that_read_the_user_s_context_are_refused_to_a_view_while_the_launcher_is_hidden() {
        assert!(call_allowed("selection.text", true, true).is_ok());
        assert!(call_allowed("selection.text", true, false).is_err());
        assert!(call_allowed("alert.confirm", true, false).is_err());
        assert!(call_allowed("alert.confirm", true, true).is_ok());
        // a no-view command runs with the launcher hidden on purpose
        assert!(call_allowed("selection.text", false, false).is_ok());
        assert!(call_allowed("clipboard.copy", true, false).is_ok());
    }

    #[test]
    fn a_view_hidden_for_longer_than_the_restore_window_expires() {
        use std::time::Duration;
        assert!(!view_expired(true, true, Duration::from_secs(3600)), "visible never expires");
        assert!(!view_expired(true, false, Duration::from_secs(89)));
        assert!(view_expired(true, false, Duration::from_secs(91)));
        assert!(!view_expired(false, false, Duration::from_secs(3600)), "no-view commands have their own limit");
    }

    #[test]
    fn a_relaunched_no_view_command_keeps_its_original_start() {
        use std::time::{Duration, Instant};
        let t0 = Instant::now();
        let now = t0 + Duration::from_secs(50);
        assert_eq!(carried_origin(Some((false, t0)), false, now), t0);
        assert_eq!(carried_origin(Some((true, t0)), false, now), now, "a view that launches a no-view command starts a fresh clock");
        assert_eq!(carried_origin(Some((false, t0)), true, now), now);
        assert_eq!(carried_origin(None, false, now), now);
    }

    #[test]
    fn selection_script_targets_the_given_process_and_the_output_is_parsed() {
        let s = selection_script(123);
        assert!(s.contains("unix id is 123") && s.contains("AXSelectedText"));
        assert_eq!(parse_selection("hello world\n").unwrap(), "hello world");
        assert_eq!(parse_selection("two\nlines\n").unwrap(), "two\nlines");
        assert_eq!(parse_selection("\n").unwrap_err(), "Unable to get selected text");
        assert_eq!(parse_selection("missing value\n").unwrap_err(), "Unable to get selected text");
    }

    #[test]
    fn default_app_script_only_takes_absolute_safe_paths() {
        assert!(default_app_script("/tmp/a.txt").unwrap().contains("POSIX file \"/tmp/a.txt\""));
        assert!(default_app_script("a.txt").is_err());
        assert!(default_app_script("https://example.test").is_err());
        assert!(default_app_script("/tmp/x\" & (do shell script \"id\") & \"").unwrap().contains("\\\"")); // escaped, not executable
        assert!(default_app_script("/tmp/x\ndo shell script \"id\"").is_err());
    }

    #[test]
    fn confirm_script_escapes_and_names_both_buttons() {
        let (s, primary) =
            confirm_script(&json!({"title":"Del \"x\"?","message":"a\nb","primary":"Delete","dismiss":"Keep","style":"destructive"}))
                .unwrap();
        assert_eq!(primary, "Delete");
        assert!(s.starts_with("display alert \"Del \\\"x\\\"?\" message \"a b\" as critical"));
        assert!(s.contains("buttons {\"Keep\", \"Delete\"} default button \"Delete\" cancel button \"Keep\""));
        let plain = confirm_script(&json!({"title":"t","message":"m","primary":"OK","dismiss":"Cancel"})).unwrap().0;
        assert!(plain.contains("as informational"));
        assert!(parse_confirm(Ok("button returned:Delete, gave up:false\n".into()), "Delete"));
        assert!(!parse_confirm(Ok("button returned:Keep\n".into()), "Delete"));
        assert!(!parse_confirm(Err(()), "Delete"), "cancel (osascript error) is a no");
    }

    #[test]
    fn confirm_compares_against_the_cleaned_primary_the_script_used() {
        // 60 chars with a newline: the script shows 40 chars with the newline as a space
        let long = format!("Yes\n{}", "x".repeat(56));
        let (script, primary) = confirm_script(&json!({"title":"t","message":"m","primary":long,"dismiss":"No"})).unwrap();
        assert_eq!(primary.chars().count(), 40);
        assert!(!primary.contains('\n') && primary.starts_with("Yes x"));
        assert!(script.contains(&format!("default button \"{primary}\"")));
        assert!(parse_confirm(Ok(format!("button returned:{primary}\n")), &primary));
        assert!(!parse_confirm(Ok(format!("button returned:{long}\n")), &long), "the raw text never matches what the dialog returned");
        // a button label that merely starts with the primary one is not a yes
        assert!(!parse_confirm(Ok("button returned:OKAY\n".into()), "OK"));
        assert!(parse_confirm(Ok("button returned:OK, gave up:false\n".into()), "OK"));
        // missing labels get defaults
        let (s2, p2) = confirm_script(&json!({"title":"t"})).unwrap();
        assert_eq!(p2, "OK");
        assert!(s2.contains("buttons {\"Cancel\", \"OK\"}"));
    }

    #[test]
    fn applications_have_raycast_shape() {
        assert_eq!(
            application_json("Safari", "/Applications/Safari.app", Some("com.apple.Safari")),
            json!({"name":"Safari","path":"/Applications/Safari.app","bundleId":"com.apple.Safari"})
        );
        assert_eq!(application_json("X", "/x", None), json!({"name":"X","path":"/x"}));
        assert_eq!(app_name_from_path("/System/Applications/TextEdit.app/").as_deref(), Some("TextEdit"));
        assert_eq!(app_name_from_path("/"), None);
    }

    #[test]
    fn launch_stays_inside_the_running_extension() {
        let (n, a) = parse_launch(&json!({"name":"open-page","arguments":{"title":"x"},"type":"userInitiated"}), "wikipedia", &[]).unwrap();
        assert_eq!((n.as_str(), a["title"].as_str()), ("open-page", Some("x")));
        assert!(parse_launch(&json!({"name":"c","extensionName":"wikipedia"}), "wikipedia", &[]).is_ok());
        assert!(parse_launch(&json!({"name":"c","extensionName":"other"}), "wikipedia", &[]).is_err());
        assert!(parse_launch(&json!({"name":"../x"}), "wikipedia", &[]).is_err());
        assert!(parse_launch(&json!({}), "wikipedia", &[]).is_err());
    }

    #[test]
    fn launch_refuses_an_owner_that_is_not_the_extensions_owner_or_author() {
        let mine = ["koenvanzuijlen", "raycast"];
        assert!(parse_launch(&json!({"name":"c","ownerOrAuthorName":"raycast"}), "e", &mine).is_ok());
        assert!(parse_launch(&json!({"name":"c","ownerOrAuthorName":"koenvanzuijlen"}), "e", &mine).is_ok());
        assert!(parse_launch(&json!({"name":"c","ownerOrAuthorName":"someone-else"}), "e", &mine).is_err());
        assert!(
            parse_launch(&json!({"name":"c","ownerOrAuthorName":"someone-else"}), "e", &[]).is_err(),
            "no known owner: nothing matches"
        );
        // the runtime sends '' / null when the extension did not set it
        assert!(parse_launch(&json!({"name":"c","ownerOrAuthorName":null}), "e", &mine).is_ok());
        assert!(parse_launch(&json!({"name":"c","ownerOrAuthorName":""}), "e", &mine).is_ok());
        assert!(parse_launch(&json!({"name":"c","extensionName":"e","ownerOrAuthorName":"x"}), "e", &mine).is_err());
    }
}
