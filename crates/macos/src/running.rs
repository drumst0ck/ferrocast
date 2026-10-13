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

fn by_pid(pid: i32) -> Option<objc2::rc::Retained<NSRunningApplication>> {
    NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
}

/// Bring an app to the front.
pub fn activate(pid: i32) -> bool {
    #[allow(deprecated)]
    by_pid(pid).is_some_and(|a| a.activateWithOptions(NSApplicationActivationOptions::ActivateIgnoringOtherApps))
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

/// An on-screen window from CGWindowList.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowInfo {
    pub pid: i32,
    pub app: String,
    pub title: String,
    pub id: u32,
}

/// List normal (layer 0) windows across apps, front to back.
pub fn list_windows() -> Vec<WindowInfo> {
    use core_graphics::window::*;
    let Some(arr) = copy_window_info(kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements, kCGNullWindowID) else {
        return vec![];
    };
    let mut out = vec![];
    for item in arr.iter() {
        let dict: CFDictionary<CFString, CFType> = unsafe { CFDictionary::wrap_under_get_rule(*item as _) };
        let get_num = |k: &str| dict.find(CFString::new(k)).and_then(|v| v.downcast::<CFNumber>()).and_then(|n| n.to_i64());
        let get_str = |k: &str| dict.find(CFString::new(k)).and_then(|v| v.downcast::<CFString>()).map(|s| s.to_string());
        if get_num("kCGWindowLayer") != Some(0) {
            continue;
        }
        let app = get_str("kCGWindowOwnerName").unwrap_or_default();
        if app == "Ferrocast" || app == "Window Server" || app.is_empty() {
            continue;
        }
        let title = get_str("kCGWindowName").unwrap_or_default();
        // Tiny windows are usually invisible helpers
        let pid = get_num("kCGWindowOwnerPID").unwrap_or(0) as i32;
        let id = get_num("kCGWindowNumber").unwrap_or(0) as u32;
        out.push(WindowInfo { pid, app, title, id });
    }
    out
}
