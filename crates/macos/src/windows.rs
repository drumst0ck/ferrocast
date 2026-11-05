//! Window management via the Accessibility API, plus pure layout geometry.

use accessibility_sys::*;
use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::string::CFString;
use core_graphics::geometry::{CGPoint, CGSize};
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect { x, y, w, h }
    }
    fn center(&self) -> (f64, f64) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
    fn contains(&self, p: (f64, f64)) -> bool {
        p.0 >= self.x && p.0 < self.x + self.w && p.1 >= self.y && p.1 < self.y + self.h
    }
    fn inset(&self, g: f64) -> Rect {
        Rect::new(self.x + g, self.y + g, (self.w - 2.0 * g).max(1.0), (self.h - 2.0 * g).max(1.0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WindowCommand {
    LeftHalf,
    RightHalf,
    TopHalf,
    BottomHalf,
    TopLeftQuarter,
    TopRightQuarter,
    BottomLeftQuarter,
    BottomRightQuarter,
    FirstThird,
    CenterThird,
    LastThird,
    FirstTwoThirds,
    LastTwoThirds,
    FirstFourth,
    SecondFourth,
    ThirdFourth,
    LastFourth,
    TopLeftSixth,
    TopCenterSixth,
    TopRightSixth,
    BottomLeftSixth,
    BottomCenterSixth,
    BottomRightSixth,
    Maximize,
    AlmostMaximize,
    MaximizeWidth,
    MaximizeHeight,
    Center,
    ReasonableSize,
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    MakeLarger,
    MakeSmaller,
    Restore,
    NextDisplay,
    PreviousDisplay,
    ToggleFullscreen,
    Minimize,
}

impl WindowCommand {
    pub const ALL: &'static [WindowCommand] = &[
        WindowCommand::LeftHalf,
        WindowCommand::RightHalf,
        WindowCommand::TopHalf,
        WindowCommand::BottomHalf,
        WindowCommand::TopLeftQuarter,
        WindowCommand::TopRightQuarter,
        WindowCommand::BottomLeftQuarter,
        WindowCommand::BottomRightQuarter,
        WindowCommand::FirstThird,
        WindowCommand::CenterThird,
        WindowCommand::LastThird,
        WindowCommand::FirstTwoThirds,
        WindowCommand::LastTwoThirds,
        WindowCommand::FirstFourth,
        WindowCommand::SecondFourth,
        WindowCommand::ThirdFourth,
        WindowCommand::LastFourth,
        WindowCommand::TopLeftSixth,
        WindowCommand::TopCenterSixth,
        WindowCommand::TopRightSixth,
        WindowCommand::BottomLeftSixth,
        WindowCommand::BottomCenterSixth,
        WindowCommand::BottomRightSixth,
        WindowCommand::Maximize,
        WindowCommand::AlmostMaximize,
        WindowCommand::MaximizeWidth,
        WindowCommand::MaximizeHeight,
        WindowCommand::Center,
        WindowCommand::ReasonableSize,
        WindowCommand::MoveUp,
        WindowCommand::MoveDown,
        WindowCommand::MoveLeft,
        WindowCommand::MoveRight,
        WindowCommand::MakeLarger,
        WindowCommand::MakeSmaller,
        WindowCommand::Restore,
        WindowCommand::NextDisplay,
        WindowCommand::PreviousDisplay,
        WindowCommand::ToggleFullscreen,
        WindowCommand::Minimize,
    ];

    pub fn title(&self) -> &'static str {
        use WindowCommand::*;
        match self {
            LeftHalf => "Left Half",
            RightHalf => "Right Half",
            TopHalf => "Top Half",
            BottomHalf => "Bottom Half",
            TopLeftQuarter => "Top Left Quarter",
            TopRightQuarter => "Top Right Quarter",
            BottomLeftQuarter => "Bottom Left Quarter",
            BottomRightQuarter => "Bottom Right Quarter",
            FirstThird => "First Third",
            CenterThird => "Center Third",
            LastThird => "Last Third",
            FirstTwoThirds => "First Two Thirds",
            LastTwoThirds => "Last Two Thirds",
            FirstFourth => "First Fourth",
            SecondFourth => "Second Fourth",
            ThirdFourth => "Third Fourth",
            LastFourth => "Last Fourth",
            TopLeftSixth => "Top Left Sixth",
            TopCenterSixth => "Top Center Sixth",
            TopRightSixth => "Top Right Sixth",
            BottomLeftSixth => "Bottom Left Sixth",
            BottomCenterSixth => "Bottom Center Sixth",
            BottomRightSixth => "Bottom Right Sixth",
            Maximize => "Maximize",
            AlmostMaximize => "Almost Maximize",
            MaximizeWidth => "Maximize Width",
            MaximizeHeight => "Maximize Height",
            Center => "Center",
            ReasonableSize => "Reasonable Size",
            MoveUp => "Move Up",
            MoveDown => "Move Down",
            MoveLeft => "Move Left",
            MoveRight => "Move Right",
            MakeLarger => "Make Larger",
            MakeSmaller => "Make Smaller",
            Restore => "Restore",
            NextDisplay => "Next Display",
            PreviousDisplay => "Previous Display",
            ToggleFullscreen => "Toggle Fullscreen",
            Minimize => "Minimize",
        }
    }

    pub fn id(&self) -> String {
        format!("window:{}", self.title().to_lowercase().replace(' ', "-"))
    }
}

/// Compute the target frame for `cmd`, given the current window frame and the
/// visible frame of its screen (top-left origin coordinates). Returns None for
/// commands that aren't pure geometry (display moves, restore, fullscreen).
pub fn layout(cmd: WindowCommand, win: Rect, s: Rect, gap: f64) -> Option<Rect> {
    use WindowCommand::*;
    let frac = |fx: f64, fy: f64, fw: f64, fh: f64| Rect::new(s.x + s.w * fx, s.y + s.h * fy, s.w * fw, s.h * fh).inset(gap);
    let clamp_w = win.w.min(s.w);
    let clamp_h = win.h.min(s.h);
    Some(match cmd {
        LeftHalf => frac(0.0, 0.0, 0.5, 1.0),
        RightHalf => frac(0.5, 0.0, 0.5, 1.0),
        TopHalf => frac(0.0, 0.0, 1.0, 0.5),
        BottomHalf => frac(0.0, 0.5, 1.0, 0.5),
        TopLeftQuarter => frac(0.0, 0.0, 0.5, 0.5),
        TopRightQuarter => frac(0.5, 0.0, 0.5, 0.5),
        BottomLeftQuarter => frac(0.0, 0.5, 0.5, 0.5),
        BottomRightQuarter => frac(0.5, 0.5, 0.5, 0.5),
        FirstThird => frac(0.0, 0.0, 1.0 / 3.0, 1.0),
        CenterThird => frac(1.0 / 3.0, 0.0, 1.0 / 3.0, 1.0),
        LastThird => frac(2.0 / 3.0, 0.0, 1.0 / 3.0, 1.0),
        FirstTwoThirds => frac(0.0, 0.0, 2.0 / 3.0, 1.0),
        LastTwoThirds => frac(1.0 / 3.0, 0.0, 2.0 / 3.0, 1.0),
        FirstFourth => frac(0.0, 0.0, 0.25, 1.0),
        SecondFourth => frac(0.25, 0.0, 0.25, 1.0),
        ThirdFourth => frac(0.5, 0.0, 0.25, 1.0),
        LastFourth => frac(0.75, 0.0, 0.25, 1.0),
        TopLeftSixth => frac(0.0, 0.0, 1.0 / 3.0, 0.5),
        TopCenterSixth => frac(1.0 / 3.0, 0.0, 1.0 / 3.0, 0.5),
        TopRightSixth => frac(2.0 / 3.0, 0.0, 1.0 / 3.0, 0.5),
        BottomLeftSixth => frac(0.0, 0.5, 1.0 / 3.0, 0.5),
        BottomCenterSixth => frac(1.0 / 3.0, 0.5, 1.0 / 3.0, 0.5),
        BottomRightSixth => frac(2.0 / 3.0, 0.5, 1.0 / 3.0, 0.5),
        Maximize => s.inset(gap),
        AlmostMaximize => frac(0.05, 0.05, 0.9, 0.9),
        MaximizeWidth => Rect::new(s.x + gap, win.y, s.w - 2.0 * gap, win.h),
        MaximizeHeight => Rect::new(win.x, s.y + gap, win.w, s.h - 2.0 * gap),
        Center => Rect::new(s.x + (s.w - clamp_w) / 2.0, s.y + (s.h - clamp_h) / 2.0, clamp_w, clamp_h),
        ReasonableSize => {
            let w = (s.w * 0.6).min(1025.0);
            let h = (s.h * 0.6).min(900.0);
            Rect::new(s.x + (s.w - w) / 2.0, s.y + (s.h - h) / 2.0, w, h)
        }
        MoveUp => Rect::new(win.x, s.y + gap, win.w, win.h),
        MoveDown => Rect::new(win.x, s.y + s.h - win.h - gap, win.w, win.h),
        MoveLeft => Rect::new(s.x + gap, win.y, win.w, win.h),
        MoveRight => Rect::new(s.x + s.w - win.w - gap, win.y, win.w, win.h),
        MakeLarger | MakeSmaller => {
            let d = if cmd == MakeLarger { 30.0 } else { -30.0 };
            let w = (win.w + 2.0 * d).clamp(200f64.min(s.w), s.w);
            let h = (win.h + 2.0 * d).clamp(150f64.min(s.h), s.h);
            let x = (win.x - (w - win.w) / 2.0).clamp(s.x, s.x + s.w - w);
            let y = (win.y - (h - win.h) / 2.0).clamp(s.y, s.y + s.h - h);
            Rect::new(x, y, w, h)
        }
        Restore | NextDisplay | PreviousDisplay | ToggleFullscreen | Minimize => return None,
    })
}

/// Move a frame from one screen to another, preserving relative position/size.
pub fn translate_between(win: Rect, from: Rect, to: Rect) -> Rect {
    let rx = (win.x - from.x) / from.w;
    let ry = (win.y - from.y) / from.h;
    let w = (win.w / from.w * to.w).min(to.w);
    let h = (win.h / from.h * to.h).min(to.h);
    Rect::new(to.x + rx * to.w, to.y + ry * to.h, w, h)
}

// ---------------------------------------------------------------------------
// Accessibility

/// Is this process trusted for Accessibility? Optionally show the system prompt.
pub fn accessibility_trusted(prompt: bool) -> bool {
    unsafe {
        let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
        let val = if prompt { CFBoolean::true_value() } else { CFBoolean::false_value() };
        let dict = core_foundation::dictionary::CFDictionary::from_CFType_pairs(&[(key, val)]);
        AXIsProcessTrustedWithOptions(dict.as_concrete_TypeRef())
    }
}

/// Is this process allowed to listen to keystrokes (Input Monitoring, used by snippet
/// auto-expansion)? Read-only preflight: it never shows the system prompt.
pub fn input_monitoring_granted() -> bool {
    objc2_core_graphics::CGPreflightListenEventAccess()
}

/// Ask for Input Monitoring: shows the system prompt the first time and adds Ferrocast to the list
/// in System Settings. Returns whether access is already granted.
/// Ask macOS for Automation access to Finder, in this process (so Ferrocast itself is listed under
/// Privacy & Security > Automation). BLOCKS while the system prompt is open: call off the main
/// thread. `Ok(true)` granted, `Ok(false)` denied earlier (no prompt is shown again), `Err` otherwise.
pub fn request_finder_automation() -> Result<bool, i32> {
    finder_automation(true)
}

/// Is Automation for Finder granted? Silent: never shows the prompt.
pub fn finder_automation_granted() -> bool {
    matches!(finder_automation(false), Ok(true))
}

fn finder_automation(ask: bool) -> Result<bool, i32> {
    use std::ffi::c_void;
    #[repr(C)]
    struct AEDesc {
        descriptor_type: u32,
        data_handle: *mut c_void,
    }
    #[link(name = "CoreServices", kind = "framework")]
    extern "C" {
        fn AECreateDesc(type_code: u32, data: *const c_void, size: isize, result: *mut AEDesc) -> i16;
        fn AEDisposeDesc(desc: *mut AEDesc) -> i16;
        fn AEDeterminePermissionToAutomateTarget(target: *const AEDesc, class: u32, id: u32, ask: bool) -> i32;
    }
    const TYPE_BUNDLE_ID: u32 = u32::from_be_bytes(*b"bund");
    const WILDCARD: u32 = u32::from_be_bytes(*b"****");
    const DENIED: i32 = -1743;
    let bundle = b"com.apple.finder";
    let mut desc = AEDesc { descriptor_type: 0, data_handle: std::ptr::null_mut() };
    unsafe {
        let e = AECreateDesc(TYPE_BUNDLE_ID, bundle.as_ptr() as *const c_void, bundle.len() as isize, &mut desc);
        if e != 0 {
            return Err(e as i32);
        }
        let r = AEDeterminePermissionToAutomateTarget(&desc, WILDCARD, WILDCARD, ask);
        AEDisposeDesc(&mut desc);
        match r {
            0 => Ok(true),
            DENIED => Ok(false),
            other => Err(other),
        }
    }
}

pub fn request_input_monitoring() -> bool {
    objc2_core_graphics::CGRequestListenEventAccess()
}

pub(crate) struct AxElement(pub(crate) AXUIElementRef);
impl Drop for AxElement {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0 as CFTypeRef) }
        }
    }
}

/// Take ownership of a copied AX value and return it as a `String` only when it really is a
/// CFString: an app returning another type for AXTitle/AXRole must not reach `CFStringGetLength`.
pub(crate) fn owned_string(v: CFTypeRef) -> Option<String> {
    let t = unsafe { core_foundation::base::CFType::wrap_under_create_rule(v) };
    t.downcast_into::<CFString>().map(|s| s.to_string())
}

/// A string attribute of `el`, `None` when missing or not a CFString.
pub(crate) fn copy_string_attr(el: AXUIElementRef, attr: &str) -> Option<String> {
    copy_attr(el, attr).and_then(owned_string)
}

pub(crate) fn copy_attr(el: AXUIElementRef, attr: &str) -> Option<CFTypeRef> {
    let name = CFString::new(attr);
    let mut v: CFTypeRef = std::ptr::null();
    let err = unsafe { AXUIElementCopyAttributeValue(el, name.as_concrete_TypeRef(), &mut v) };
    (err == kAXErrorSuccess && !v.is_null()).then_some(v)
}

fn get_point(el: AXUIElementRef) -> Option<CGPoint> {
    let v = copy_attr(el, kAXPositionAttribute)?;
    let mut p = CGPoint::new(0.0, 0.0);
    let ok = unsafe { AXValueGetValue(v as AXValueRef, kAXValueTypeCGPoint, &mut p as *mut _ as *mut c_void) };
    unsafe { CFRelease(v) };
    ok.then_some(p)
}

fn get_size(el: AXUIElementRef) -> Option<CGSize> {
    let v = copy_attr(el, kAXSizeAttribute)?;
    let mut s = CGSize::new(0.0, 0.0);
    let ok = unsafe { AXValueGetValue(v as AXValueRef, kAXValueTypeCGSize, &mut s as *mut _ as *mut c_void) };
    unsafe { CFRelease(v) };
    ok.then_some(s)
}

fn set_value(el: AXUIElementRef, attr: &str, ty: u32, ptr: *const c_void) -> bool {
    unsafe {
        let v = AXValueCreate(ty, ptr);
        if v.is_null() {
            return false;
        }
        let name = CFString::new(attr);
        let err = AXUIElementSetAttributeValue(el, name.as_concrete_TypeRef(), v as CFTypeRef);
        CFRelease(v as CFTypeRef);
        err == kAXErrorSuccess
    }
}

pub(crate) fn set_bool(el: AXUIElementRef, attr: &str, b: bool) -> bool {
    let name = CFString::new(attr);
    let v = if b { CFBoolean::true_value() } else { CFBoolean::false_value() };
    unsafe { AXUIElementSetAttributeValue(el, name.as_concrete_TypeRef(), v.as_CFTypeRef()) == kAXErrorSuccess }
}

pub(crate) fn get_bool(el: AXUIElementRef, attr: &str) -> Option<bool> {
    let v = copy_attr(el, attr)?;
    let b = unsafe { CFBoolean::wrap_under_create_rule(v as _) };
    Some(b == CFBoolean::true_value())
}

pub(crate) fn frame_of(el: AXUIElementRef) -> Option<Rect> {
    let p = get_point(el)?;
    let s = get_size(el)?;
    Some(Rect::new(p.x, p.y, s.width, s.height))
}

pub(crate) fn set_frame(el: AXUIElementRef, r: Rect) {
    let p = CGPoint::new(r.x, r.y);
    let s = CGSize::new(r.w, r.h);
    // size → position → size handles moves across displays with different sizes
    set_value(el, kAXSizeAttribute, kAXValueTypeCGSize, &s as *const _ as *const c_void);
    set_value(el, kAXPositionAttribute, kAXValueTypeCGPoint, &p as *const _ as *const c_void);
    set_value(el, kAXSizeAttribute, kAXValueTypeCGSize, &s as *const _ as *const c_void);
}

/// The focused window of the given pid.
pub(crate) fn focused_window(pid: i32) -> Option<AxElement> {
    unsafe {
        let app = AxElement(AXUIElementCreateApplication(pid));
        // a hung app must not stall the caller (the tiling worker polls this)
        AXUIElementSetMessagingTimeout(app.0, 0.25);
        let w = copy_attr(app.0, kAXFocusedWindowAttribute).or_else(|| copy_attr(app.0, "AXMainWindow"))?;
        let w = AxElement(w as AXUIElementRef);
        AXUIElementSetMessagingTimeout(w.0, 0.25);
        Some(w)
    }
}

/// Visible frames of all screens in top-left-origin global coordinates.
pub fn screens() -> Vec<Rect> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSScreen;
    let Some(mtm) = MainThreadMarker::new() else {
        // Fallback off main thread: CoreGraphics display bounds (no menu bar inset).
        return core_graphics::display::CGDisplay::active_displays()
            .unwrap_or_default()
            .into_iter()
            .map(|id| {
                let b = core_graphics::display::CGDisplay::new(id).bounds();
                Rect::new(b.origin.x, b.origin.y, b.size.width, b.size.height)
            })
            .collect();
    };
    let screens = NSScreen::screens(mtm);
    let primary_h = screens.iter().next().map(|s| s.frame().size.height).unwrap_or(0.0);
    screens
        .iter()
        .map(|s| {
            let v = s.visibleFrame();
            Rect::new(v.origin.x, primary_h - (v.origin.y + v.size.height), v.size.width, v.size.height)
        })
        .collect()
}

fn screen_index_for(win: Rect, screens: &[Rect]) -> usize {
    let c = win.center();
    screens
        .iter()
        .position(|s| s.contains(c))
        .or_else(|| {
            // nearest by center distance
            screens
                .iter()
                .enumerate()
                .min_by(|a, b| {
                    let d = |r: &Rect| {
                        let (x, y) = r.center();
                        (x - c.0).powi(2) + (y - c.1).powi(2)
                    };
                    d(a.1).partial_cmp(&d(b.1)).unwrap()
                })
                .map(|(i, _)| i)
        })
        .unwrap_or(0)
}

static PREVIOUS: Mutex<Option<HashMap<(i32, String), Rect>>> = Mutex::new(None);

fn window_key(pid: i32, el: AXUIElementRef) -> (i32, String) {
    let title = copy_string_attr(el, "AXTitle").unwrap_or_default();
    (pid, title)
}

/// Apply a window command to the focused window of `pid` (the app that was
/// frontmost before the launcher opened).
pub fn apply(cmd: WindowCommand, pid: i32, gap: f64) -> anyhow::Result<()> {
    if !accessibility_trusted(true) {
        anyhow::bail!("Grant Accessibility permission in System Settings → Privacy & Security → Accessibility");
    }
    let win = focused_window(pid).ok_or_else(|| anyhow::anyhow!("No focused window"))?;
    let frame = frame_of(win.0).ok_or_else(|| anyhow::anyhow!("Window has no frame"))?;
    let screens = screens();
    anyhow::ensure!(!screens.is_empty(), "No screens");
    let idx = screen_index_for(frame, &screens);
    let key = window_key(pid, win.0);
    let mut prev = PREVIOUS.lock().unwrap();
    let prev = prev.get_or_insert_with(HashMap::new);

    let target = match cmd {
        WindowCommand::ToggleFullscreen => {
            let fs = get_bool(win.0, "AXFullScreen").unwrap_or(false);
            set_bool(win.0, "AXFullScreen", !fs);
            return Ok(());
        }
        WindowCommand::Minimize => {
            set_bool(win.0, kAXMinimizedAttribute, true);
            return Ok(());
        }
        WindowCommand::Restore => prev.get(&key).copied().ok_or_else(|| anyhow::anyhow!("Nothing to restore"))?,
        WindowCommand::NextDisplay | WindowCommand::PreviousDisplay => {
            anyhow::ensure!(screens.len() > 1, "Only one display");
            let n = screens.len();
            let j = if cmd == WindowCommand::NextDisplay { (idx + 1) % n } else { (idx + n - 1) % n };
            translate_between(frame, screens[idx], screens[j])
        }
        _ => layout(cmd, frame, screens[idx], gap).ok_or_else(|| anyhow::anyhow!("Unsupported window command"))?,
    };
    if cmd != WindowCommand::Restore {
        prev.insert(key, frame);
    }
    set_frame(win.0, target);
    Ok(())
}

/// Messaging timeout for AX calls made on the UI thread: a hung app must not freeze the launcher.
const RAISE_AX_TIMEOUT: f32 = 0.25;

extern "C" {
    /// Private but long-stable HIServices call: the CGWindowID behind an AX window element.
    fn _AXUIElementGetWindow(element: AXUIElementRef, id: *mut u32) -> i32;
}

fn ax_window_id(el: AXUIElementRef) -> Option<u32> {
    let mut id = 0u32;
    (unsafe { _AXUIElementGetWindow(el, &mut id) } == 0 && id != 0).then_some(id)
}

/// Which AX window to raise. `candidates` are each window's (CGWindowID if known, title). A known
/// `id` wins (titles are empty without Screen Recording and can repeat); titles are only the
/// fallback for windows whose id the AX API would not give.
fn pick_window(candidates: &[(Option<u32>, String)], id: u32, title: &str) -> Option<usize> {
    if id != 0 {
        if let Some(i) = candidates.iter().position(|(c, _)| *c == Some(id)) {
            return Some(i);
        }
    }
    // never guess by an empty title: it matches any untitled window
    if title.is_empty() {
        return None;
    }
    candidates.iter().position(|(c, t)| c.is_none() && t == title)
}

/// Raise a specific window of an app (matched by window id, then by title) and activate the app.
pub fn raise_window(pid: i32, title: &str, id: u32) -> bool {
    unsafe {
        let app = AxElement(AXUIElementCreateApplication(pid));
        AXUIElementSetMessagingTimeout(app.0, RAISE_AX_TIMEOUT);
        let Some(list) = copy_attr(app.0, kAXWindowsAttribute) else { return false };
        let arr: core_foundation::array::CFArray<*const c_void> = core_foundation::array::CFArray::wrap_under_create_rule(list as _);
        let els: Vec<AXUIElementRef> = arr.iter().map(|w| *w as AXUIElementRef).collect();
        for el in &els {
            AXUIElementSetMessagingTimeout(*el, RAISE_AX_TIMEOUT);
        }
        let candidates: Vec<(Option<u32>, String)> = els
            .iter()
            .map(|&el| {
                let t = copy_string_attr(el, "AXTitle").unwrap_or_default();
                (ax_window_id(el), t)
            })
            .collect();
        let Some(i) = pick_window(&candidates, id, title) else { return false };
        let el = els[i];
        let action = CFString::new(kAXRaiseAction);
        AXUIElementPerformAction(el, action.as_concrete_TypeRef());
        set_bool(el, kAXMainAttribute, true);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const S: Rect = Rect { x: 0.0, y: 25.0, w: 1000.0, h: 800.0 };

    #[test]
    fn make_smaller_and_larger_do_not_panic_on_a_tiny_screen() {
        let tiny = Rect::new(0.0, 0.0, 150.0, 100.0);
        let win = Rect::new(10.0, 10.0, 100.0, 80.0);
        for cmd in [WindowCommand::MakeSmaller, WindowCommand::MakeLarger] {
            let r = layout(cmd, win, tiny, 0.0).unwrap();
            assert!(r.w <= 150.0 && r.h <= 100.0 && r.x >= 0.0 && r.y >= 0.0, "{r:?}");
        }
    }

    #[test]
    fn owned_string_rejects_a_non_string_value() {
        use core_foundation::base::TCFType;
        let n = core_foundation::number::CFNumber::from(7);
        let raw = n.as_CFTypeRef();
        unsafe { core_foundation::base::CFRetain(raw) };
        assert_eq!(owned_string(raw), None);
        let s = CFString::new("Doc");
        let raw = s.as_CFTypeRef();
        unsafe { core_foundation::base::CFRetain(raw) };
        assert_eq!(owned_string(raw).as_deref(), Some("Doc"));
    }

    #[test]
    fn pick_window_prefers_the_window_id_over_duplicate_or_empty_titles() {
        let c = vec![(Some(10), String::new()), (Some(11), String::new()), (Some(12), "Doc".into())];
        assert_eq!(pick_window(&c, 11, ""), Some(1), "second untitled window, not the first");
        assert_eq!(pick_window(&c, 12, "Other"), Some(2));
    }

    #[test]
    fn pick_window_never_guesses_when_nothing_identifies_the_window() {
        let c = vec![(Some(10), String::new()), (Some(11), "A".into())];
        assert_eq!(pick_window(&c, 99, ""), None, "unknown id + empty title must not raise a random window");
        assert_eq!(pick_window(&c, 99, "A"), None, "ids are known for every window: a gone id is not retitled");
        // ids unavailable from AX: title fallback
        let c = vec![(None, "A".into()), (None, "B".into())];
        assert_eq!(pick_window(&c, 7, "B"), Some(1));
        assert_eq!(pick_window(&[], 7, "B"), None);
    }

    #[test]
    fn input_monitoring_preflight_is_silent_and_stable() {
        // read-only: must neither prompt nor panic, and two reads agree
        assert_eq!(input_monitoring_granted(), input_monitoring_granted());
    }

    #[test]
    fn halves() {
        let w = Rect::new(100.0, 100.0, 300.0, 300.0);
        assert_eq!(layout(WindowCommand::LeftHalf, w, S, 0.0), Some(Rect::new(0.0, 25.0, 500.0, 800.0)));
        assert_eq!(layout(WindowCommand::BottomRightQuarter, w, S, 0.0), Some(Rect::new(500.0, 425.0, 500.0, 400.0)));
        assert_eq!(layout(WindowCommand::Maximize, w, S, 10.0), Some(Rect::new(10.0, 35.0, 980.0, 780.0)));
    }

    #[test]
    fn center_and_reasonable() {
        let w = Rect::new(0.0, 0.0, 200.0, 100.0);
        assert_eq!(layout(WindowCommand::Center, w, S, 0.0), Some(Rect::new(400.0, 375.0, 200.0, 100.0)));
        let r = layout(WindowCommand::ReasonableSize, w, S, 0.0).unwrap();
        assert_eq!((r.w, r.h), (600.0, 480.0));
    }

    #[test]
    fn move_edges() {
        let w = Rect::new(100.0, 100.0, 200.0, 100.0);
        assert_eq!(layout(WindowCommand::MoveRight, w, S, 0.0).unwrap().x, 800.0);
        assert_eq!(layout(WindowCommand::MoveDown, w, S, 0.0).unwrap().y, 725.0);
    }

    #[test]
    fn translate() {
        let a = Rect::new(0.0, 0.0, 1000.0, 1000.0);
        let b = Rect::new(1000.0, 0.0, 2000.0, 2000.0);
        assert_eq!(translate_between(Rect::new(0.0, 0.0, 500.0, 500.0), a, b), Rect::new(1000.0, 0.0, 1000.0, 1000.0));
    }

    #[test]
    fn ids_unique() {
        let mut v: Vec<_> = WindowCommand::ALL.iter().map(|c| c.id()).collect();
        let n = v.len();
        v.sort();
        v.dedup();
        assert_eq!(n, v.len());
    }
}
