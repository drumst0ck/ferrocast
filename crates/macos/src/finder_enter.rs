//! "Enter on a Finder selection": a default (not listen-only) CGEventTap that swallows a plain
//! Return only when Finder is frontmost and a file selection is focused.
//!
//! HIGH RISK: a tap sits in the path of every key press on the Mac. A callback that stalls
//! freezes keyboard input system-wide (macOS then disables the tap). So the callback does only
//! O(1) work: atomics, up to four Accessibility reads with a 50 ms timeout each (only for a plain
//! Return while Finder is frontmost), and one channel send. No locks, no file or process I/O.
//! Every decision about whether to swallow lives in `ferrocast_core::finder_enter`.
//!
//! The tap is created by `start` and removed by dropping the returned handle. With the feature
//! off none of this exists: no tap, no observer, no Accessibility prompt.

use accessibility_sys::*;
use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::runloop::{kCFRunLoopCommonModes, kCFRunLoopDefaultMode, CFRunLoop};
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::event::{
    CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType, CallbackResult, EventField,
};
use ferrocast_core::finder_enter as rules;
use ferrocast_core::finder_enter::InFlight;
use std::ffi::{c_char, c_void};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicPtr, AtomicU64, AtomicU8, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

extern "C" {
    fn CGEventTapEnable(tap: *mut c_void, enable: bool);
    fn CFStringGetCString(s: CFStringRef, buf: *mut c_char, size: isize, encoding: u32) -> u8;
}
const UTF8: u32 = 0x0800_0100;

/// AX messaging timeout while the tap is alive (seconds). Set once per tap.
const AX_TIMEOUT: f32 = 0.05;

// ---------------------------------------------------------------------------
// Pure role mapping

/// The text in a NUL-terminated buffer, if valid UTF-8 and non-empty.
pub fn text_of(buf: &[u8]) -> Option<&str> {
    let end = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
    std::str::from_utf8(&buf[..end]).ok().filter(|s| !s.is_empty())
}

/// Map an AXRole string to the static role name the rules know, or `None` when it is not one of
/// the file-selection roles (text fields, search fields, unknown roles all become `None`).
pub fn selection_role(raw: &[u8]) -> Option<&'static str> {
    let t = text_of(raw)?;
    rules::SELECTION_ROLES.iter().copied().find(|r| *r == t)
}

/// Map an AXSubrole string to the normal-window subrole, or `None` for anything else.
pub fn normal_window_subrole(raw: &[u8]) -> Option<&'static str> {
    let t = text_of(raw)?;
    (t == rules::NORMAL_WINDOW_SUBROLE).then_some(rules::NORMAL_WINDOW_SUBROLE)
}

// ---------------------------------------------------------------------------
// Diagnostics: plain atomics, written by the callback (Relaxed, no locks, no allocation, no I/O)
// and read by the tap thread's 2 s tick, which formats and logs one line.

/// Why `focused_selection_role` returned `None` (or `REASON_OK`).
pub const REASON_OK: u8 = 0;
pub const REASON_NO_FOCUSED: u8 = 1;
pub const REASON_NO_ROLE: u8 = 2;
pub const REASON_ROLE_NOT_SELECTION: u8 = 3;
pub const REASON_NO_WINDOW: u8 = 4;
pub const REASON_NO_SUBROLE: u8 = 5;
pub const REASON_SUBROLE_NOT_STANDARD: u8 = 6;

/// Short name of a reason code, for the log line.
pub fn reason_name(code: u8) -> &'static str {
    match code {
        REASON_OK => "ok",
        REASON_NO_FOCUSED => "no-focused-element",
        REASON_NO_ROLE => "no-role",
        REASON_ROLE_NOT_SELECTION => "role-not-a-selection-role",
        REASON_NO_WINDOW => "no-window",
        REASON_NO_SUBROLE => "no-window-subrole",
        REASON_SUBROLE_NOT_STANDARD => "window-not-standard",
        _ => "unknown",
    }
}

/// First 8 bytes of a NUL-terminated text, zero padded, as a big-endian number (no allocation).
pub fn pack_text(buf: &[u8]) -> u64 {
    let end = buf.iter().position(|b| *b == 0).unwrap_or(buf.len()).min(8);
    let mut b = [0u8; 8];
    b[..end].copy_from_slice(&buf[..end]);
    u64::from_be_bytes(b)
}

/// The text packed by `pack_text` ("" for 0; invalid UTF-8 bytes become '?').
pub fn unpack_text(v: u64) -> String {
    let b = v.to_be_bytes();
    let end = b.iter().position(|c| *c == 0).unwrap_or(8);
    String::from_utf8_lossy(&b[..end]).replace('\u{fffd}', "?")
}

#[derive(Default)]
pub struct Diag {
    pub returns_seen: AtomicU64,
    pub passed_precheck: AtomicU64,
    pub pc_marker: AtomicU64,
    pub pc_repeat: AtomicU64,
    pub pc_modifiers: AtomicU64,
    pub pc_disabled: AtomicU64,
    pub pc_not_finder_front: AtomicU64,
    pub passed_pid: AtomicU64,
    pub passed_role: AtomicU64,
    pub passed_busy: AtomicU64,
    pub passed_send: AtomicU64,
    pub intercepted: AtomicU64,
    /// Times the callback saw TapDisabledByTimeout / TapDisabledByUserInput and re-enabled.
    pub reenabled: AtomicU64,
    pub last_focused_pid: AtomicI32,
    pub last_finder_pid: AtomicI32,
    pub last_role: AtomicU64,
    pub last_reason: AtomicU8,
    pub last_window_subrole: AtomicU64,
}

/// Plain numbers and unpacked text of a `Diag`, for formatting.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub returns_seen: u64,
    pub intercepted: u64,
    pub precheck: u64,
    pub pc_marker: u64,
    pub pc_repeat: u64,
    pub pc_modifiers: u64,
    pub pc_disabled: u64,
    pub pc_not_finder_front: u64,
    pub pid: u64,
    pub role: u64,
    pub busy: u64,
    pub send: u64,
    pub finder_pid: i32,
    pub focused_pid: i32,
    pub last_role: String,
    pub last_window_subrole: String,
    pub reason: u8,
}

impl Diag {
    fn bump(c: &AtomicU64) {
        c.fetch_add(1, Ordering::Relaxed);
    }
    pub fn seen(&self) {
        Self::bump(&self.returns_seen);
    }
    /// Count a pre-check failure and, where it can be told without allocation, why. The first
    /// matching cause wins, in the order of `rules::pre_check`'s own conjuncts.
    /// "Finder not frontmost" is the cause by elimination, so `_finder_front` is not read.
    pub fn precheck_failed(&self, user_data: i64, autorepeat: bool, mods_empty: bool, enabled: bool, _finder_front: bool) {
        Self::bump(&self.passed_precheck);
        let c = if rules::is_repost(user_data) {
            &self.pc_marker
        } else if autorepeat {
            &self.pc_repeat
        } else if !mods_empty {
            &self.pc_modifiers
        } else if !enabled {
            &self.pc_disabled
        } else {
            &self.pc_not_finder_front
        };
        Self::bump(c);
    }
    pub fn pid_failed(&self) {
        Self::bump(&self.passed_pid);
    }
    pub fn role_failed(&self) {
        Self::bump(&self.passed_role);
    }
    pub fn busy_failed(&self) {
        Self::bump(&self.passed_busy);
    }
    pub fn send_failed(&self) {
        Self::bump(&self.passed_send);
    }
    pub fn intercepted(&self) {
        Self::bump(&self.intercepted);
    }
    pub fn tap_reenabled(&self) {
        Self::bump(&self.reenabled);
    }
    pub fn snapshot(&self) -> Snapshot {
        let l = |c: &AtomicU64| c.load(Ordering::Relaxed);
        Snapshot {
            returns_seen: l(&self.returns_seen),
            intercepted: l(&self.intercepted),
            precheck: l(&self.passed_precheck),
            pc_marker: l(&self.pc_marker),
            pc_repeat: l(&self.pc_repeat),
            pc_modifiers: l(&self.pc_modifiers),
            pc_disabled: l(&self.pc_disabled),
            pc_not_finder_front: l(&self.pc_not_finder_front),
            pid: l(&self.passed_pid),
            role: l(&self.passed_role),
            busy: l(&self.passed_busy),
            send: l(&self.passed_send),
            finder_pid: self.last_finder_pid.load(Ordering::Relaxed),
            focused_pid: self.last_focused_pid.load(Ordering::Relaxed),
            last_role: unpack_text(l(&self.last_role)),
            last_window_subrole: unpack_text(l(&self.last_window_subrole)),
            reason: self.last_reason.load(Ordering::Relaxed),
        }
    }
}

/// The one log line for a snapshot. Never contains key content or file paths.
pub fn format_diag(s: &Snapshot) -> String {
    let causes: Vec<String> = [
        ("marker", s.pc_marker),
        ("repeat", s.pc_repeat),
        ("modifiers", s.pc_modifiers),
        ("disabled", s.pc_disabled),
        ("not-finder-front", s.pc_not_finder_front),
    ]
    .iter()
    .filter(|(_, n)| *n > 0)
    .map(|(name, n)| format!("{name} {n}"))
    .collect();
    let causes = if causes.is_empty() { String::new() } else { format!(" ({})", causes.join(", ")) };
    let text = |t: &str| if t.is_empty() { "-".to_string() } else { t.to_string() };
    format!(
        "finder-enter: Return seen {}, intercepted {}, passed: precheck {}{causes}, pid {} (finder_pid={} focused_pid={}), \
         role {} (role={} window_subrole={} why={}), busy {}, send {}",
        s.returns_seen,
        s.intercepted,
        s.precheck,
        s.pid,
        s.finder_pid,
        s.focused_pid,
        s.role,
        text(&s.last_role),
        text(&s.last_window_subrole),
        reason_name(s.reason),
        s.busy,
        s.send
    )
}

// ---------------------------------------------------------------------------
// Accessibility reads (callback side; no allocation: names are made once, text goes to the stack)

struct Names {
    focused: CFString,
    role: CFString,
    window: CFString,
    subrole: CFString,
}
// SAFETY: immutable CFStrings are thread-safe; the closure holding them is only ever called on
// the tap thread anyway.
unsafe impl Send for Names {}

struct SystemWide(AXUIElementRef);
impl Drop for SystemWide {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0 as CFTypeRef) }
    }
}
// SAFETY: the system-wide element is a process-global handle used from one thread.
unsafe impl Send for SystemWide {}

struct Owned(CFTypeRef);
impl Drop for Owned {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0) }
    }
}

fn copy_attr(el: AXUIElementRef, name: &CFString) -> Option<Owned> {
    let mut v: CFTypeRef = std::ptr::null();
    let err = unsafe { AXUIElementCopyAttributeValue(el, name.as_concrete_TypeRef(), &mut v) };
    (err == kAXErrorSuccess && !v.is_null()).then_some(Owned(v))
}

fn with_text<R>(v: &Owned, f: impl FnOnce(&[u8]) -> R) -> Option<R> {
    let mut buf = [0u8; 64];
    let ok = unsafe { CFStringGetCString(v.0 as CFStringRef, buf.as_mut_ptr() as *mut c_char, buf.len() as isize, UTF8) };
    (ok != 0).then(|| f(&buf))
}

/// Role of the focused element and its owning process id (`None` when unreadable), but only when
/// it sits in an ordinary window; otherwise `None`. `diag` receives the last pid, role text,
/// window subrole text and the reason for a `None` (atomic stores only; the result is unchanged).
fn focused_selection_role(sys: &SystemWide, n: &Names, diag: &Diag) -> Option<(&'static str, Option<i32>)> {
    diag.last_focused_pid.store(0, Ordering::Relaxed); // 0 = not read in this call
    diag.last_role.store(0, Ordering::Relaxed);
    diag.last_window_subrole.store(0, Ordering::Relaxed);
    let Some(focused) = copy_attr(sys.0, &n.focused) else {
        diag.last_reason.store(REASON_NO_FOCUSED, Ordering::Relaxed);
        return None;
    };
    let el = focused.0 as AXUIElementRef;
    let Some((packed, mapped)) = copy_attr(el, &n.role).and_then(|r| with_text(&r, |b| (pack_text(b), selection_role(b)))) else {
        diag.last_reason.store(REASON_NO_ROLE, Ordering::Relaxed);
        return None;
    };
    diag.last_role.store(packed, Ordering::Relaxed);
    let Some(role) = mapped else {
        diag.last_reason.store(REASON_ROLE_NOT_SELECTION, Ordering::Relaxed);
        return None;
    };
    let mut pid: i32 = 0;
    let pid = (unsafe { AXUIElementGetPid(el, &mut pid) } == kAXErrorSuccess).then_some(pid);
    diag.last_focused_pid.store(pid.unwrap_or(-1), Ordering::Relaxed);
    let Some(win) = copy_attr(el, &n.window) else {
        diag.last_reason.store(REASON_NO_WINDOW, Ordering::Relaxed);
        return None;
    };
    let Some((packed, sub)) =
        copy_attr(win.0 as AXUIElementRef, &n.subrole).and_then(|s| with_text(&s, |b| (pack_text(b), normal_window_subrole(b))))
    else {
        diag.last_reason.store(REASON_NO_SUBROLE, Ordering::Relaxed);
        return None;
    };
    diag.last_window_subrole.store(packed, Ordering::Relaxed);
    if rules::window_is_normal(sub) {
        diag.last_reason.store(REASON_OK, Ordering::Relaxed);
        Some((role, pid))
    } else {
        diag.last_reason.store(REASON_SUBROLE_NOT_STANDARD, Ordering::Relaxed);
        None
    }
}

// ---------------------------------------------------------------------------
// AX messaging timeout guard

/// Sets the process-wide AX messaging timeout when built and ALWAYS restores the default in
/// `Drop`, so early returns and unwinding cannot leave the process at the short timeout.
/// The set/reset operations are injected so the contract is unit-testable.
pub struct AxTimeoutGuard<R: FnMut()> {
    reset: R,
}

impl<R: FnMut()> AxTimeoutGuard<R> {
    pub fn new(mut set: impl FnMut(f32), reset: R, seconds: f32) -> Self {
        set(seconds);
        AxTimeoutGuard { reset }
    }
}

impl<R: FnMut()> Drop for AxTimeoutGuard<R> {
    fn drop(&mut self) {
        (self.reset)();
    }
}

/// Set the system-wide AX messaging timeout (seconds; 0 = system default).
fn set_ax_timeout(seconds: f32) {
    unsafe {
        let el = AXUIElementCreateSystemWide();
        AXUIElementSetMessagingTimeout(el, seconds);
        CFRelease(el as CFTypeRef);
    }
}

// ---------------------------------------------------------------------------
// Handle

struct RunLoopHandle(CFRunLoop);
// SAFETY: CFRunLoopStop is documented as thread-safe.
unsafe impl Send for RunLoopHandle {}

/// A live tap. Dropping it stops the thread and removes the tap and the activation observer.
pub struct FinderEnterTap {
    stop: Arc<AtomicBool>,
    enabled: Arc<AtomicBool>,
    run_loop: Option<RunLoopHandle>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for FinderEnterTap {
    fn drop(&mut self) {
        // the callback passes everything through from this moment on
        self.enabled.store(false, Ordering::SeqCst);
        self.stop.store(true, Ordering::SeqCst);
        if let Some(rl) = self.run_loop.take() {
            rl.0.stop();
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn is_finder_front() -> bool {
    finder_pid_now() != 0
}

/// Pid of the frontmost app when it is Finder, else 0.
fn finder_pid_now() -> i32 {
    crate::running::frontmost().filter(|f| f.bundle_id.as_deref() == Some(rules::FINDER_BUNDLE)).map_or(0, |f| f.pid)
}

/// Create the tap. Returns an error (no tap exists) when it cannot be created, normally because
/// Accessibility is not granted. `on_enter` runs on a helper thread, never inside the callback.
pub fn start(
    inflight: Arc<InFlight>,
    on_enter: Arc<dyn Fn() + Send + Sync>,
    log: Arc<dyn Fn(&str) + Send + Sync>,
) -> Result<FinderEnterTap, String> {
    let stop = Arc::new(AtomicBool::new(false));
    let enabled = Arc::new(AtomicBool::new(true));
    let (ready_tx, ready_rx) = mpsc::sync_channel::<Result<RunLoopHandle, String>>(1);
    let (tx, rx) = mpsc::channel::<()>();
    std::thread::Builder::new()
        .name("finder-enter-fwd".into())
        .spawn(move || {
            while rx.recv().is_ok() {
                on_enter();
            }
        })
        .map_err(|e| e.to_string())?;
    let (stop2, enabled2) = (stop.clone(), enabled.clone());
    let thread = std::thread::Builder::new()
        .name("finder-enter-tap".into())
        .spawn(move || run(stop2, enabled2, inflight, tx, ready_tx, log))
        .map_err(|e| e.to_string())?;
    match ready_rx.recv_timeout(Duration::from_secs(3)) {
        Ok(Ok(rl)) => Ok(FinderEnterTap { stop, enabled, run_loop: Some(rl), thread: Some(thread) }),
        Ok(Err(e)) => {
            let _ = thread.join();
            Err(e)
        }
        Err(_) => {
            stop.store(true, Ordering::SeqCst);
            enabled.store(false, Ordering::SeqCst);
            Err("the event tap thread did not start".into())
        }
    }
}

fn run(
    stop: Arc<AtomicBool>,
    enabled: Arc<AtomicBool>,
    inflight: Arc<InFlight>,
    tx: mpsc::Sender<()>,
    ready: mpsc::SyncSender<Result<RunLoopHandle, String>>,
    log: Arc<dyn Fn(&str) + Send + Sync>,
) {
    use objc2_app_kit::{NSWorkspace, NSWorkspaceDidActivateApplicationNotification};
    use objc2_foundation::NSNotification;

    let front = Arc::new(AtomicBool::new(is_finder_front()));
    // pid of Finder while it is frontmost, else 0 (seeded here, kept by the activation observer)
    let finder_pid = Arc::new(AtomicI32::new(finder_pid_now()));
    let port = Arc::new(AtomicPtr::<c_void>::new(std::ptr::null_mut()));
    let diag = Arc::new(Diag::default());

    let sys = SystemWide(unsafe { AXUIElementCreateSystemWide() });
    let names = Names {
        focused: CFString::new(kAXFocusedUIElementAttribute),
        role: CFString::new(kAXRoleAttribute),
        window: CFString::new(kAXWindowAttribute),
        subrole: CFString::new(kAXSubroleAttribute),
    };

    let (front_cb, enabled_cb, port_cb, pid_cb) = (front.clone(), enabled.clone(), port.clone(), finder_pid.clone());
    let diag_cb = diag.clone();
    let tap = CGEventTap::new(
        CGEventTapLocation::Session,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::Default,
        vec![CGEventType::KeyDown],
        move |_proxy, ty, ev| {
            match ty {
                CGEventType::TapDisabledByTimeout | CGEventType::TapDisabledByUserInput => {
                    diag_cb.tap_reenabled();
                    let p = port_cb.load(Ordering::Relaxed);
                    if !p.is_null() {
                        unsafe { CGEventTapEnable(p, true) };
                    }
                    return CallbackResult::Keep;
                }
                CGEventType::KeyDown => {}
                _ => return CallbackResult::Keep,
            }
            if ev.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE) != i64::from(crate::keyboard::keycode::RETURN) {
                return CallbackResult::Keep;
            }
            diag_cb.seen();
            diag_cb.last_finder_pid.store(pid_cb.load(Ordering::Relaxed), Ordering::Relaxed);
            let user_data = ev.get_integer_value_field(EventField::EVENT_SOURCE_USER_DATA);
            let repeat = ev.get_integer_value_field(EventField::KEYBOARD_EVENT_AUTOREPEAT) != 0;
            let mods = rules::modifiers_empty(ev.get_flags().bits());
            let finder = front_cb.load(Ordering::Relaxed);
            if !rules::pre_check(user_data, true, repeat, mods, enabled_cb.load(Ordering::Relaxed), finder) {
                diag_cb.precheck_failed(user_data, repeat, mods, enabled_cb.load(Ordering::Relaxed), finder);
                return CallbackResult::Keep;
            }
            let (role, focused_pid) = match focused_selection_role(&sys, &names, &diag_cb) {
                Some((r, p)) => (Some(r), p),
                None => (None, None),
            };
            // defense in depth: a stale `front` must not swallow Return in another app
            if !rules::pid_ok(pid_cb.load(Ordering::Relaxed), focused_pid) {
                diag_cb.pid_failed();
                return CallbackResult::Keep;
            }
            if !rules::should_intercept(finder.then_some(rules::FINDER_BUNDLE), role, mods, true) {
                diag_cb.role_failed();
                return CallbackResult::Keep;
            }
            // one Return at a time; an extra one passes through
            if !inflight.try_begin() {
                diag_cb.busy_failed();
                return CallbackResult::Keep;
            }
            if tx.send(()).is_err() {
                inflight.end();
                diag_cb.send_failed();
                return CallbackResult::Keep;
            }
            diag_cb.intercepted();
            CallbackResult::Drop
        },
    );
    let Ok(tap) = tap else {
        let _ = ready.send(Err("could not create the event tap".into()));
        return;
    };
    let Some(src) = tap.mach_port().create_runloop_source(0).ok() else {
        let _ = ready.send(Err("could not create the run-loop source".into()));
        return;
    };
    let rl = CFRunLoop::get_current();
    rl.add_source(&src, unsafe { kCFRunLoopCommonModes });
    port.store(tap.mach_port().as_concrete_TypeRef() as *mut c_void, Ordering::SeqCst);
    // The tap exists: only now shorten the process-wide AX timeout. The guard restores the default
    // on every exit path, including a panic.
    let _ax_timeout = AxTimeoutGuard::new(set_ax_timeout, || set_ax_timeout(0.0), AX_TIMEOUT);

    // Activation observer: keeps `front` current. Registered only while the tap lives.
    let center = NSWorkspace::sharedWorkspace().notificationCenter();
    let (front_obs, pid_obs) = (front.clone(), finder_pid.clone());
    let block = block2::RcBlock::new(move |_n: NonNull<NSNotification>| {
        let pid = finder_pid_now();
        pid_obs.store(pid, Ordering::Relaxed);
        front_obs.store(pid != 0, Ordering::Relaxed);
    });
    let token = unsafe {
        center.addObserverForName_object_queue_usingBlock(Some(NSWorkspaceDidActivateApplicationNotification), None, None, &block)
    };
    let pid = finder_pid_now();
    finder_pid.store(pid, Ordering::Relaxed);
    front.store(pid != 0, Ordering::Relaxed);

    tap.enable();
    let _ = ready.send(Ok(RunLoopHandle(rl)));

    let mut last_enable = Instant::now();
    let (mut logged_seen, mut logged_reenabled) = (0u64, 0u64);
    while !stop.load(Ordering::SeqCst) {
        let t = Instant::now();
        CFRunLoop::run_in_mode(unsafe { kCFRunLoopDefaultMode }, Duration::from_millis(500), false);
        // a wake-up from sleep or an unnoticed disable: re-enable every couple of seconds
        if last_enable.elapsed() > Duration::from_secs(2) {
            tap.enable();
            last_enable = Instant::now();
            // diagnostics: one line, and only while Returns are being seen (never key content or paths)
            let snap = diag.snapshot();
            if snap.returns_seen != logged_seen {
                logged_seen = snap.returns_seen;
                log(&format_diag(&snap));
            }
            let re = diag.reenabled.load(Ordering::Relaxed);
            if re != logged_reenabled {
                logged_reenabled = re;
                log(&format!("finder-enter: tap was disabled by macOS and re-enabled ({re} time(s) so far)"));
            }
        }
        if t.elapsed() < Duration::from_millis(20) && !stop.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    enabled.store(false, Ordering::SeqCst);
    unsafe {
        center.removeObserver(objc2::runtime::ProtocolObject::as_ref(&*token));
    }
}

/// Give a swallowed Return back to Finder, marked so the tap never intercepts it again. Does
/// nothing (returns false) when Finder is no longer frontmost: the key would land elsewhere.
pub fn repost_return() -> bool {
    if !is_finder_front() {
        return false;
    }
    crate::keyboard::repost_return();
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_text_stops_at_the_nul_and_rejects_empty_or_invalid() {
        assert_eq!(text_of(b"AXList\0garbage"), Some("AXList"));
        assert_eq!(text_of(b"AXList"), Some("AXList"));
        assert_eq!(text_of(b"\0"), None);
        assert_eq!(text_of(b""), None);
        assert_eq!(text_of(&[0xff, 0xfe, 0]), None);
    }

    #[test]
    fn only_selection_roles_map_and_text_inputs_do_not() {
        for r in ["AXList", "AXOutline", "AXBrowser", "AXScrollArea", "AXImage"] {
            assert_eq!(selection_role(format!("{r}\0").as_bytes()), Some(r));
        }
        for r in ["AXTextField", "AXTextArea", "AXSearchField", "AXComboBox", "AXButton", "AXList2", "axlist", "AXLis"] {
            assert_eq!(selection_role(format!("{r}\0").as_bytes()), None, "{r}");
        }
        assert_eq!(selection_role(b"\0"), None);
    }

    #[test]
    fn only_the_standard_window_subrole_maps() {
        assert_eq!(normal_window_subrole(b"AXStandardWindow\0"), Some("AXStandardWindow"));
        for r in ["AXDialog", "AXSystemDialog", "AXFloatingWindow", "AXSystemFloatingWindow", "AXSheet", "AXStandardWindows"] {
            assert_eq!(normal_window_subrole(format!("{r}\0").as_bytes()), None, "{r}");
        }
        assert_eq!(normal_window_subrole(b""), None);
    }

    fn sample() -> Snapshot {
        Snapshot {
            returns_seen: 3,
            intercepted: 1,
            precheck: 1,
            pc_not_finder_front: 1,
            pid: 0,
            role: 1,
            busy: 0,
            send: 0,
            finder_pid: 412,
            focused_pid: 412,
            last_role: "AXTextFi".into(),
            last_window_subrole: "AXStanda".into(),
            reason: REASON_ROLE_NOT_SELECTION,
            ..Snapshot::default()
        }
    }

    #[test]
    fn format_diag_is_exact_for_a_known_snapshot() {
        assert_eq!(
            format_diag(&sample()),
            "finder-enter: Return seen 3, intercepted 1, passed: precheck 1 (not-finder-front 1), pid 0 (finder_pid=412 focused_pid=412), \
             role 1 (role=AXTextFi window_subrole=AXStanda why=role-not-a-selection-role), busy 0, send 0"
        );
    }

    #[test]
    fn format_diag_lists_only_the_precheck_causes_that_happened_and_dashes_for_empty_text() {
        let s = Snapshot { returns_seen: 5, precheck: 4, pc_marker: 1, pc_repeat: 1, pc_modifiers: 2, ..Snapshot::default() };
        assert_eq!(
            format_diag(&s),
            "finder-enter: Return seen 5, intercepted 0, passed: precheck 4 (marker 1, repeat 1, modifiers 2), pid 0 (finder_pid=0 focused_pid=0), \
             role 0 (role=- window_subrole=- why=ok), busy 0, send 0"
        );
    }

    #[test]
    fn text_packs_into_eight_bytes_and_unpacks_again() {
        assert_eq!(unpack_text(pack_text(b"AXList\0junk")), "AXList");
        assert_eq!(unpack_text(pack_text(b"AXTextField\0")), "AXTextFi", "only the first 8 bytes");
        assert_eq!(unpack_text(pack_text(b"12345678")), "12345678");
        assert_eq!(unpack_text(pack_text(b"")), "");
        assert_eq!(unpack_text(0), "");
        assert_eq!(unpack_text(0x4158_4C69_7374_0000), "AXList");
        assert_eq!(pack_text(b"AXList\0"), 0x4158_4C69_7374_0000);
        assert_eq!(unpack_text(pack_text(&[0xff, b'a', 0])), "?a");
    }

    #[test]
    fn a_fresh_diag_is_all_zero_and_counters_add_up() {
        let d = Diag::default();
        assert_eq!(d.snapshot(), Snapshot::default());
        d.seen();
        d.seen();
        d.precheck_failed(rules::REPOST_MARKER, true, false, false, false);
        d.precheck_failed(0, true, false, false, false);
        d.precheck_failed(0, false, false, false, false);
        d.precheck_failed(0, false, true, false, false);
        d.precheck_failed(0, false, true, true, false);
        d.pid_failed();
        d.role_failed();
        d.role_failed();
        d.busy_failed();
        d.send_failed();
        d.intercepted();
        d.tap_reenabled();
        d.last_finder_pid.store(7, Ordering::Relaxed);
        d.last_role.store(pack_text(b"AXList"), Ordering::Relaxed);
        d.last_reason.store(REASON_NO_WINDOW, Ordering::Relaxed);
        let s = d.snapshot();
        assert_eq!((s.returns_seen, s.intercepted, s.precheck), (2, 1, 5));
        assert_eq!((s.pc_marker, s.pc_repeat, s.pc_modifiers, s.pc_disabled, s.pc_not_finder_front), (1, 1, 1, 1, 1));
        assert_eq!((s.pid, s.role, s.busy, s.send), (1, 2, 1, 1));
        assert_eq!(d.reenabled.load(Ordering::Relaxed), 1);
        assert_eq!((s.finder_pid, s.last_role.as_str(), s.reason), (7, "AXList", REASON_NO_WINDOW));
        assert_eq!(reason_name(REASON_NO_WINDOW), "no-window");
        assert_eq!(reason_name(200), "unknown");
    }

    use std::cell::RefCell;

    #[derive(Default)]
    struct Calls {
        sets: RefCell<Vec<f32>>,
        resets: RefCell<usize>,
    }
    fn guard(c: &Calls) -> AxTimeoutGuard<impl FnMut() + '_> {
        AxTimeoutGuard::new(|s| c.sets.borrow_mut().push(s), || *c.resets.borrow_mut() += 1, AX_TIMEOUT)
    }

    #[test]
    fn guard_sets_the_timeout_once_when_constructed() {
        let c = Calls::default();
        let _g = guard(&c);
        assert_eq!(*c.sets.borrow(), vec![0.05]);
        assert_eq!(*c.resets.borrow(), 0, "not reset while alive");
    }

    #[test]
    fn guard_resets_once_on_normal_drop() {
        let c = Calls::default();
        drop(guard(&c));
        assert_eq!(*c.sets.borrow(), vec![0.05]);
        assert_eq!(*c.resets.borrow(), 1, "reset exactly once");
    }

    #[test]
    fn guard_resets_once_on_an_early_return_with_err() {
        fn work(c: &Calls) -> Result<(), String> {
            let _g = guard(c);
            Err("could not create the event tap".into())
        }
        let c = Calls::default();
        assert!(work(&c).is_err());
        assert_eq!(*c.resets.borrow(), 1, "early return must still reset");
        assert_eq!(c.sets.borrow().len(), 1);
    }

    #[test]
    fn guard_resets_once_when_a_panic_unwinds() {
        let c = Calls::default();
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _g = guard(&c);
            panic!("boom");
        }));
        assert!(r.is_err());
        assert_eq!(*c.resets.borrow(), 1, "unwinding must reset");
        assert_eq!(c.sets.borrow().len(), 1);
    }
}
