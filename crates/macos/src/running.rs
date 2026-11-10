//! Running applications, frontmost app, open windows list.

use core_foundation::base::{CFType, TCFType};
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use objc2_app_kit::{NSApplicationActivationOptions, NSApplicationActivationPolicy, NSRunningApplication, NSWorkspace};

#[derive(Debug, Clone, PartialEq)]
pub struct RunningApp {
    pub pid: i32,
    pub name: String,
    pub bundle_id: Option<String>,
    pub path: Option<String>,
}

fn to_info(a: &NSRunningApplication) -> RunningApp {
    RunningApp {
        pid: a.processIdentifier(),
        name: a.localizedName().map(|s| s.to_string()).unwrap_or_default(),
        bundle_id: a.bundleIdentifier().map(|s| s.to_string()),
        path: a.bundleURL().and_then(|u| u.path()).map(|p| p.to_string()),
    }
}

/// The frontmost app (other than us, if possible).
pub fn frontmost() -> Option<RunningApp> {
    NSWorkspace::sharedWorkspace().frontmostApplication().map(|a| to_info(&a))
}

/// Regular (Dock-visible) running apps.
pub fn running_apps() -> Vec<RunningApp> {
    NSWorkspace::sharedWorkspace()
        .runningApplications()
        .iter()
        .filter(|a| a.activationPolicy() == NSApplicationActivationPolicy::Regular)
        .map(|a| to_info(&a))
        .collect()
}

/// Every running application, whatever its activation policy (menu-bar and background apps too).
pub fn all_running_apps() -> Vec<RunningApp> {
    NSWorkspace::sharedWorkspace().runningApplications().iter().map(|a| to_info(&a)).collect()
}

fn by_pid(pid: i32) -> Option<objc2::rc::Retained<NSRunningApplication>> {
    NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
}

/// Which API brings an app to the front. `activateIgnoringOtherApps` is deprecated (and ignored)
/// from macOS 14, where activation is cooperative; macOS 13 still needs the old call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationApi {
    /// `activateWithOptions(ActivateIgnoringOtherApps)`.
    Legacy,
    /// `NSApplication.activate()` / `yieldActivationToApplication` + `NSRunningApplication.activate()`.
    Cooperative,
}

/// Pick the activation API for a macOS major version.
pub fn activation_api_for(macos_major: isize) -> ActivationApi {
    if macos_major >= 14 {
        ActivationApi::Cooperative
    } else {
        ActivationApi::Legacy
    }
}

fn activation_api() -> ActivationApi {
    static API: std::sync::OnceLock<ActivationApi> = std::sync::OnceLock::new();
    *API.get_or_init(|| {
        let v = objc2_foundation::NSProcessInfo::processInfo().operatingSystemVersion();
        activation_api_for(v.majorVersion)
    })
}

/// Bring Ferrocast itself to the front (launcher, notes, selector, permission helper).
pub fn activate_self(app: &objc2_app_kit::NSApplication) {
    match activation_api() {
        ActivationApi::Cooperative => app.activate(),
        ActivationApi::Legacy => {
            #[allow(deprecated)]
            app.activateIgnoringOtherApps(true);
        }
    }
}

/// Bring an app to the front.
pub fn activate(pid: i32) -> bool {
    let Some(target) = by_pid(pid) else { return false };
    match activation_api() {
        ActivationApi::Cooperative => {
            // we are the one handing focus over: yield first so the target may take it
            if let Some(mtm) = objc2::MainThreadMarker::new() {
                objc2_app_kit::NSApplication::sharedApplication(mtm).yieldActivationToApplication(&target);
            }
            // `-[NSRunningApplication activateFromApplication:options:]` (macOS 14+); if the system
            // refuses the cooperative handover, a plain activation of its windows still asks politely.
            let me = NSRunningApplication::currentApplication();
            target.activateFromApplication_options(&me, NSApplicationActivationOptions::empty())
                || target.activateWithOptions(NSApplicationActivationOptions::ActivateAllWindows)
        }
        ActivationApi::Legacy =>
        {
            #[allow(deprecated)]
            target.activateWithOptions(NSApplicationActivationOptions::ActivateIgnoringOtherApps)
        }
    }
}

/// `activate`, run on the main thread: a background thread cannot yield Ferrocast's activation,
/// which the cooperative API (macOS 14+) needs.
pub fn activate_on_main(pid: i32) {
    if objc2::MainThreadMarker::new().is_some() {
        activate(pid);
        return;
    }
    extern "C" {
        static _dispatch_main_q: std::ffi::c_void;
        fn dispatch_async_f(queue: *const std::ffi::c_void, context: *mut std::ffi::c_void, work: extern "C" fn(*mut std::ffi::c_void));
    }
    extern "C" fn run(ctx: *mut std::ffi::c_void) {
        let pid = unsafe { *Box::from_raw(ctx as *mut i32) };
        activate(pid);
    }
    unsafe { dispatch_async_f(&_dispatch_main_q, Box::into_raw(Box::new(pid)) as *mut _, run) };
}

/// Show a hidden (⌘H) app again.
pub fn unhide(pid: i32) -> bool {
    by_pid(pid).is_some_and(|a| a.unhide())
}

pub fn quit(pid: i32) -> bool {
    by_pid(pid).is_some_and(|a| a.terminate())
}

pub fn force_quit(pid: i32) -> bool {
    by_pid(pid).is_some_and(|a| a.forceTerminate())
}

pub fn hide(pid: i32) -> bool {
    by_pid(pid).is_some_and(|a| a.hide())
}

/// Where a listed window is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowState {
    /// Visible on the current Space.
    OnScreen,
    /// Minimized to the Dock.
    Minimized,
    /// Open, but on another Space.
    OtherSpace,
    /// The app is hidden (⌘H).
    Hidden,
}

/// A window from CGWindowList.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowInfo {
    pub pid: i32,
    pub app: String,
    pub title: String,
    pub id: u32,
    pub state: WindowState,
}

/// One CGWindowList entry, before filtering.
#[derive(Debug, Clone, PartialEq)]
pub struct RawWindow {
    pub pid: i32,
    pub app: String,
    pub title: String,
    pub id: u32,
    pub layer: i64,
    pub width: f64,
    pub height: f64,
    pub alpha: f64,
    pub on_screen: bool,
}

/// Smaller windows are helpers, not something to switch to.
const MIN_WINDOW_W: f64 = 100.0;
const MIN_WINDOW_H: f64 = 50.0;

/// Keep real app windows, drop system/utility/tiny ones and duplicates. Off-screen windows (other
/// Spaces, minimized) need a title: untitled off-screen windows are almost always invisible helpers.
/// `minimized` holds the window ids Accessibility reports as minimized.
pub fn filter_windows(
    raw: &[RawWindow],
    minimized: &std::collections::HashSet<u32>,
    hidden: &std::collections::HashSet<i32>,
) -> Vec<WindowInfo> {
    let mut seen = std::collections::HashSet::new();
    let mut out = vec![];
    for w in raw {
        if w.layer != 0 || w.app.is_empty() || w.app == "Ferrocast" || w.app == "Window Server" {
            continue;
        }
        if w.width < MIN_WINDOW_W || w.height < MIN_WINDOW_H {
            continue;
        }
        let is_min = minimized.contains(&w.id);
        if !w.on_screen && !is_min && (w.title.is_empty() || w.alpha <= 0.0) {
            continue;
        }
        if is_min && w.title.is_empty() {
            continue;
        }
        if !seen.insert(w.id) {
            continue;
        }
        let state = if is_min {
            WindowState::Minimized
        } else if w.on_screen {
            WindowState::OnScreen
        } else if hidden.contains(&w.pid) {
            WindowState::Hidden
        } else {
            WindowState::OtherSpace
        };
        out.push(WindowInfo { pid: w.pid, app: w.app.clone(), title: w.title.clone(), id: w.id, state });
    }
    out
}

/// Apps worth an Accessibility query for minimized windows: only those with an off-screen window
/// that would be listed anyway (real size, titled, visible), and not hidden (their windows are
/// "Hidden" whatever AX says). Each AX query can cost up to its timeout on the UI thread.
pub fn pids_to_probe(raw: &[RawWindow], hidden: &std::collections::HashSet<i32>) -> Vec<i32> {
    let mut pids: Vec<i32> = raw
        .iter()
        .filter(|w| {
            !w.on_screen
                && w.layer == 0
                && !w.app.is_empty()
                && w.app != "Ferrocast"
                && w.app != "Window Server"
                && w.width >= MIN_WINDOW_W
                && w.height >= MIN_WINDOW_H
                && !w.title.is_empty()
                && w.alpha > 0.0
                && !hidden.contains(&w.pid)
        })
        .map(|w| w.pid)
        .collect();
    pids.sort_unstable();
    pids.dedup();
    pids
}

/// List normal (layer 0) windows across apps, on every Space and minimized ones too, front to back.
pub fn list_windows() -> Vec<WindowInfo> {
    use core_graphics::window::*;
    let Some(arr) = copy_window_info(kCGWindowListOptionAll | kCGWindowListExcludeDesktopElements, kCGNullWindowID) else {
        return vec![];
    };
    let mut raw = vec![];
    for item in arr.iter() {
        let dict: CFDictionary<CFString, CFType> = unsafe { CFDictionary::wrap_under_get_rule(*item as _) };
        let get_num = |k: &str| dict.find(CFString::new(k)).and_then(|v| v.downcast::<CFNumber>());
        let get_str = |k: &str| dict.find(CFString::new(k)).and_then(|v| v.downcast::<CFString>()).map(|s| s.to_string());
        let (mut width, mut height) = (f64::MAX, f64::MAX);
        if let Some(b) = dict.find(CFString::new("kCGWindowBounds")).and_then(|v| v.downcast::<CFDictionary>()) {
            let bd: CFDictionary<CFString, CFType> = unsafe { CFDictionary::wrap_under_get_rule(b.as_concrete_TypeRef()) };
            let dim = |k: &str| bd.find(CFString::new(k)).and_then(|v| v.downcast::<CFNumber>()).and_then(|n| n.to_f64());
            width = dim("Width").unwrap_or(f64::MAX);
            height = dim("Height").unwrap_or(f64::MAX);
        }
        raw.push(RawWindow {
            pid: get_num("kCGWindowOwnerPID").and_then(|n| n.to_i64()).unwrap_or(0) as i32,
            app: get_str("kCGWindowOwnerName").unwrap_or_default(),
            title: get_str("kCGWindowName").unwrap_or_default(),
            id: get_num("kCGWindowNumber").and_then(|n| n.to_i64()).unwrap_or(0) as u32,
            layer: get_num("kCGWindowLayer").and_then(|n| n.to_i64()).unwrap_or(-1),
            width,
            height,
            alpha: get_num("kCGWindowAlpha").and_then(|n| n.to_f64()).unwrap_or(1.0),
            on_screen: dict
                .find(CFString::new("kCGWindowIsOnscreen"))
                .and_then(|v| v.downcast::<core_foundation::boolean::CFBoolean>())
                .is_some_and(bool::from),
        });
    }
    let hidden: std::collections::HashSet<i32> = raw
        .iter()
        .map(|w| w.pid)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .filter(|p| by_pid(*p).is_some_and(|a| a.isHidden()))
        .collect();
    let minimized = pids_to_probe(&raw, &hidden).into_iter().flat_map(crate::windows::minimized_window_ids).collect();
    filter_windows(&raw, &minimized, &hidden)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn raw(id: u32, app: &str, title: &str, on_screen: bool) -> RawWindow {
        RawWindow { pid: 10, app: app.into(), title: title.into(), id, layer: 0, width: 800.0, height: 600.0, alpha: 1.0, on_screen }
    }

    #[test]
    fn activation_api_is_cooperative_from_macos_14_and_legacy_before() {
        assert_eq!(activation_api_for(13), ActivationApi::Legacy);
        assert_eq!(activation_api_for(12), ActivationApi::Legacy);
        assert_eq!(activation_api_for(14), ActivationApi::Cooperative);
        assert_eq!(activation_api_for(15), ActivationApi::Cooperative);
        assert_eq!(activation_api_for(26), ActivationApi::Cooperative);
    }

    #[test]
    fn windows_on_other_spaces_and_minimized_are_listed_with_their_state() {
        let all = [raw(1, "Notes", "Todo", true), raw(2, "Safari", "Docs", false), raw(3, "Mail", "Inbox", false)];
        let out = filter_windows(&all, &HashSet::from([3]), &HashSet::new());
        let st: Vec<_> = out.iter().map(|w| (w.id, w.state)).collect();
        assert_eq!(st, vec![(1, WindowState::OnScreen), (2, WindowState::OtherSpace), (3, WindowState::Minimized)]);
    }

    #[test]
    fn system_utility_tiny_and_helper_windows_are_dropped() {
        let mut menu = raw(1, "Control Center", "Item", true);
        menu.layer = 25;
        let mut tiny = raw(2, "Foo", "Bar", true);
        tiny.width = 20.0;
        let mut invisible = raw(3, "Foo", "Ghost", false);
        invisible.alpha = 0.0;
        let all = [
            menu,
            tiny,
            invisible,
            raw(4, "Ferrocast", "Launcher", true),
            raw(5, "Window Server", "x", true),
            raw(6, "", "x", true),
            raw(7, "Foo", "", false), // untitled off-screen: helper
            raw(8, "Foo", "", true),  // untitled on-screen stays (no Screen Recording permission)
        ];
        let out = filter_windows(&all, &HashSet::new(), &HashSet::new());
        assert_eq!(out.iter().map(|w| w.id).collect::<Vec<_>>(), vec![8]);
    }

    #[test]
    fn duplicate_window_ids_are_listed_once() {
        let all = [raw(1, "Notes", "A", true), raw(1, "Notes", "A", true), raw(2, "Notes", "A", true)];
        assert_eq!(filter_windows(&all, &HashSet::new(), &HashSet::new()).len(), 2);
    }

    #[test]
    fn windows_of_hidden_apps_are_marked_hidden_but_minimized_wins() {
        let mut other = raw(2, "Mail", "Inbox", false);
        other.pid = 20;
        let all = [raw(1, "Notes", "A", false), other, raw(3, "Notes", "B", false)];
        let out = filter_windows(&all, &HashSet::from([3]), &HashSet::from([10]));
        let st: Vec<_> = out.iter().map(|w| (w.id, w.state)).collect();
        assert_eq!(st, vec![(1, WindowState::Hidden), (2, WindowState::OtherSpace), (3, WindowState::Minimized)]);
    }

    #[test]
    fn only_apps_with_a_listable_off_screen_window_are_probed_for_minimized_state() {
        let mk = |pid: i32, f: &dyn Fn(&mut RawWindow)| {
            let mut w = raw(pid as u32, "App", "T", false);
            w.pid = pid;
            f(&mut w);
            w
        };
        let all = [
            mk(1, &|_| {}),                 // listable: probe
            mk(1, &|w| w.id = 11),          // same pid again: once
            mk(2, &|w| w.title.clear()),    // untitled helper
            mk(3, &|w| w.width = 10.0),     // tiny
            mk(4, &|w| w.alpha = 0.0),      // invisible
            mk(5, &|w| w.layer = 25),       // not a normal window
            mk(6, &|w| w.on_screen = true), // visible: AX not needed
            mk(7, &|w| w.app = "Ferrocast".into()),
            mk(8, &|_| {}), // hidden app
        ];
        assert_eq!(pids_to_probe(&all, &HashSet::from([8])), vec![1]);
    }

    #[test]
    fn a_minimized_window_is_marked_even_if_cg_says_on_screen() {
        let out = filter_windows(&[raw(1, "Notes", "A", true)], &HashSet::from([1]), &HashSet::new());
        assert_eq!(out[0].state, WindowState::Minimized);
    }
}
