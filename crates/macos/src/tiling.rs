//! Automatic window tiling on macOS: observes windows and applies the frames the pure engine
//! (`ferrocast_core::tiling`) computes.
//!
//! Observation is a polling worker (about every 400 ms) rather than `AXObserver` notifications:
//! observers need a run loop per process and a subscription per app that must be renewed as apps
//! launch and quit, and a missed notification leaves the layout stale, whereas a poll of the
//! Accessibility window lists sees launches, quits, new, closed and user-moved windows with the
//! same code path. Everything runs on the worker thread. Nothing here touches app state: the worker
//! is told what to do with [`Cmd`] and reports back with [`Event`], which the app forwards through
//! its message bus.
//!
//! Nothing starts unless the app spawns the worker, and the app only does so when automatic
//! tiling (or hover activation) is switched on in Settings.

use crate::running;
use crate::windows::{self, copy_attr, frame_of, get_bool, set_bool, set_frame, AxElement};
use accessibility_sys::*;
use core_foundation::base::{CFRetain, CFTypeRef, TCFType};
use core_foundation::string::CFString;
use ferrocast_core::tiling as t;
use std::ffi::c_void;
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

const POLL: Duration = Duration::from_millis(400);
/// How long the pointer rests on a window before hover activation raises it.
const HOVER_DWELL: Duration = Duration::from_millis(250);
/// How often a refused frame is retried before the window is left alone until its target changes.
const MAX_TRIES: u8 = 3;

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub enabled: bool,
    pub paused: bool,
    /// Layout 1-4 for displays without their own.
    pub layout: u8,
    /// Layout per display index.
    pub display_layouts: std::collections::HashMap<usize, u8>,
    pub gap: f64,
    /// Bundle ids (or app names) whose windows are not tiled.
    pub excluded: Vec<String>,
    pub hover_activate: bool,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            enabled: false,
            paused: false,
            layout: 1,
            display_layouts: Default::default(),
            gap: 8.0,
            excluded: vec![],
            hover_activate: false,
        }
    }
}

pub enum Cmd {
    Config(Config),
    /// Minimum sizes loaded from the local data folder.
    MinSizes(t::MinSizes),
    /// Set the layout of the display holding the focused window of `pid` (or the pointer's).
    Layout {
        n: u8,
        pid: Option<i32>,
    },
    /// Enter or leave focus mode.
    Focus {
        pid: Option<i32>,
    },
    /// Move the focused window of `pid` to the next slot or display.
    Move {
        dir: t::Dir,
        pid: Option<i32>,
    },
}

pub enum Event {
    /// A display got its own layout.
    DisplayLayout { display: usize, layout: u8 },
    /// The learned minimum sizes changed; persist them.
    MinSizes(t::MinSizes),
    /// Something the user should read.
    Notice(String),
    /// Stage Manager turned on (tiling pauses) or off.
    StageManager(bool),
}

#[derive(Clone)]
pub struct Handle(Sender<Cmd>);

impl Handle {
    /// A handle on an existing channel (the app's tests use one without a worker behind it).
    pub fn from_sender(tx: Sender<Cmd>) -> Handle {
        Handle(tx)
    }
    pub fn send(&self, c: Cmd) {
        let _ = self.0.send(c);
    }
}

/// Start the worker. `emit` is called on the worker thread: it must only forward the event.
pub fn spawn(emit: Box<dyn Fn(Event) + Send>) -> Handle {
    let (tx, rx) = channel::<Cmd>();
    let _ = std::thread::Builder::new().name("tiling".into()).spawn(move || {
        let mut w = Worker::new(emit);
        loop {
            match rx.recv_timeout(POLL) {
                Ok(c) => {
                    w.command(c);
                    while let Ok(c) = rx.try_recv() {
                        w.command(c);
                    }
                    w.tick();
                }
                Err(RecvTimeoutError::Timeout) => w.tick(),
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    });
    Handle(tx)
}

// --------------------------------------------------------------------------- system queries

extern "C" {
    static _dispatch_main_q: u8;
    fn dispatch_sync_f(queue: *const c_void, ctx: *mut c_void, work: extern "C" fn(*mut c_void));
    fn CGEventSourceButtonState(state: i32, button: u32) -> bool;
    fn CFEqual(a: CFTypeRef, b: CFTypeRef) -> u8;
    fn CFPreferencesCopyAppValue(key: core_foundation::string::CFStringRef, app: core_foundation::string::CFStringRef) -> CFTypeRef;
}

fn conv(r: windows::Rect) -> t::Rect {
    t::Rect::new(r.x, r.y, r.w, r.h)
}
fn unconv(r: t::Rect) -> windows::Rect {
    windows::Rect::new(r.x, r.y, r.w, r.h)
}

/// Visible frames of the displays. `NSScreen` is main-thread only, so the query hops to the main
/// queue (the app's run loop services it).
fn screens_on_main() -> Vec<t::Rect> {
    extern "C" fn run(ctx: *mut c_void) {
        let out = unsafe { &mut *(ctx as *mut Vec<windows::Rect>) };
        *out = windows::screens();
    }
    let mut out: Vec<windows::Rect> = vec![];
    unsafe { dispatch_sync_f(&raw const _dispatch_main_q as *const c_void, &mut out as *mut _ as *mut c_void, run) };
    out.into_iter().map(conv).filter(|r| t::valid_screen(*r)).collect()
}

fn left_button_down() -> bool {
    unsafe { CGEventSourceButtonState(0, 0) }
}

fn pointer() -> Option<(f64, f64)> {
    use core_graphics::event::CGEvent;
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    let src = CGEventSource::new(CGEventSourceStateID::HIDSystemState).ok()?;
    let p = CGEvent::new(src).ok()?.location();
    Some((p.x, p.y))
}

/// Is Stage Manager on? Read from the WindowManager preferences domain.
pub fn stage_manager_on() -> bool {
    unsafe {
        let key = CFString::new("GloballyEnabled");
        let app = CFString::new("com.apple.WindowManager");
        let v = CFPreferencesCopyAppValue(key.as_concrete_TypeRef(), app.as_concrete_TypeRef());
        if v.is_null() {
            return false;
        }
        truthy(core_foundation::base::CFType::wrap_under_create_rule(v))
    }
}

/// A preference value read as a flag: a boolean, or a number other than 0.
fn truthy(v: core_foundation::base::CFType) -> bool {
    use core_foundation::boolean::CFBoolean;
    use core_foundation::number::CFNumber;
    if let Some(b) = v.downcast::<CFBoolean>() {
        return b == CFBoolean::true_value();
    }
    v.downcast::<CFNumber>().and_then(|n| n.to_i64()).is_some_and(|n| n != 0)
}

/// Bound every Accessibility call on `el` so a hung app cannot stall the worker.
fn bound(el: AXUIElementRef) {
    unsafe { AXUIElementSetMessagingTimeout(el, 0.25) };
}

fn same(a: &AxElement, b: &AxElement) -> bool {
    unsafe { CFEqual(a.0 as CFTypeRef, b.0 as CFTypeRef) != 0 }
}

fn copy_string(el: AXUIElementRef, attr: &str) -> Option<String> {
    copy_attr(el, attr).map(|v| unsafe { CFString::wrap_under_create_rule(v as _) }.to_string())
}

fn settable(el: AXUIElementRef, attr: &str) -> bool {
    let name = CFString::new(attr);
    let mut ok: u8 = 0;
    let err = unsafe { AXUIElementIsAttributeSettable(el, name.as_concrete_TypeRef(), &mut ok) };
    err == kAXErrorSuccess && ok != 0
}

/// Standard, movable, resizable, non-minimized, non-full-screen windows only: dialogs, sheets,
/// panels, floating and system windows are never tiled.
fn tileable(el: AXUIElementRef) -> bool {
    copy_string(el, kAXRoleAttribute).as_deref() == Some(kAXWindowRole)
        && copy_string(el, kAXSubroleAttribute).as_deref() == Some(kAXStandardWindowSubrole)
        && get_bool(el, kAXMinimizedAttribute) != Some(true)
        && get_bool(el, "AXFullScreen") != Some(true)
        && settable(el, kAXSizeAttribute)
        && settable(el, kAXPositionAttribute)
}

struct Found {
    pid: i32,
    app: String,
    el: AxElement,
    frame: t::Rect,
}

struct Enumerated {
    found: Vec<Found>,
    /// Apps whose window list could not be read this time: their tracked windows are kept.
    failed: Vec<i32>,
}

/// Ids that stay tracked: seen this time, or belonging to an app that could not be read.
fn survivors(tracked: &[(u64, i32)], seen: &[u64], failed: &[i32]) -> Vec<u64> {
    tracked.iter().filter(|(id, pid)| seen.contains(id) || failed.contains(pid)).map(|(id, _)| *id).collect()
}

fn enumerate(excluded: &[String]) -> Enumerated {
    let me = std::process::id() as i32;
    let mut out = vec![];
    let mut failed = vec![];
    for app in running::running_apps() {
        let key = app.bundle_id.clone().unwrap_or_else(|| app.name.clone());
        if app.pid == me || excluded.iter().any(|e| *e == key || *e == app.name) {
            continue;
        }
        unsafe {
            let ax = AxElement(AXUIElementCreateApplication(app.pid));
            bound(ax.0);
            let Some(list) = copy_attr(ax.0, kAXWindowsAttribute) else {
                failed.push(app.pid);
                continue;
            };
            let arr: core_foundation::array::CFArray<*const c_void> = core_foundation::array::CFArray::wrap_under_create_rule(list as _);
            for w in arr.iter() {
                let el = *w as AXUIElementRef;
                bound(el);
                if !tileable(el) {
                    continue;
                }
                let Some(f) = frame_of(el) else {
                    failed.push(app.pid);
                    continue;
                };
                if f.w < 120.0 || f.h < 80.0 {
                    continue;
                }
                CFRetain(el as CFTypeRef);
                out.push(Found { pid: app.pid, app: key.clone(), el: AxElement(el), frame: conv(f) });
            }
        }
    }
    Enumerated { found: out, failed }
}

fn display_of(p: (f64, f64), screens: &[t::Rect]) -> usize {
    screens
        .iter()
        .position(|s| s.contains(p))
        .or_else(|| {
            screens
                .iter()
                .enumerate()
                .min_by(|a, b| {
                    let d = |r: &t::Rect| (r.center().0 - p.0).powi(2) + (r.center().1 - p.1).powi(2);
                    d(a.1).total_cmp(&d(b.1))
                })
                .map(|(i, _)| i)
        })
        .unwrap_or(0)
}

fn far(a: t::Rect, b: t::Rect, tol: f64) -> bool {
    (a.x - b.x).abs() > tol || (a.y - b.y).abs() > tol || (a.w - b.w).abs() > tol || (a.h - b.h).abs() > tol
}

fn raise(w: &AxElement, pid: i32) {
    let action = CFString::new(kAXRaiseAction);
    unsafe {
        bound(w.0);
        AXUIElementPerformAction(w.0, action.as_concrete_TypeRef());
        let app = AxElement(AXUIElementCreateApplication(pid));
        bound(app.0);
        set_bool(app.0, "AXFrontmost", true);
    }
}

/// On-screen windows front to back as (pid, bounds, layer).
fn window_stack() -> Vec<(i32, t::Rect, i64)> {
    use core_foundation::base::CFType;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::number::CFNumber;
    use core_graphics::window::*;
    let Some(arr) = copy_window_info(kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements, kCGNullWindowID) else {
        return vec![];
    };
    let mut out = vec![];
    for item in arr.iter() {
        let dict: CFDictionary<CFString, CFType> = unsafe { CFDictionary::wrap_under_get_rule(*item as _) };
        let num = |d: &CFDictionary<CFString, CFType>, k: &str| {
            d.find(CFString::new(k)).and_then(|v| v.downcast::<CFNumber>()).and_then(|n| n.to_f64())
        };
        let (Some(layer), Some(pid)) = (num(&dict, "kCGWindowLayer"), num(&dict, "kCGWindowOwnerPID")) else { continue };
        let Some(b) = dict.find(CFString::new("kCGWindowBounds")) else { continue };
        let b: CFDictionary<CFString, CFType> = unsafe { CFDictionary::wrap_under_get_rule(b.as_CFTypeRef() as _) };
        let (Some(x), Some(y), Some(w), Some(h)) = (num(&b, "X"), num(&b, "Y"), num(&b, "Width"), num(&b, "Height")) else { continue };
        out.push((pid as i32, t::Rect::new(x, y, w, h), layer as i64));
    }
    out
}

// --------------------------------------------------------------------------- worker

struct Tracked {
    id: u64,
    pid: i32,
    /// Bundle id (or app name): the key of the minimum-size store.
    app: String,
    el: AxElement,
    frame: t::Rect,
    display: usize,
    target: Option<t::Rect>,
    tries: u8,
    /// The frame at the previous read.
    prev: Option<t::Rect>,
    /// The frame our last arrange step asked for.
    applied: Option<t::Rect>,
    /// We applied a frame since the last read: the next read shows what the app made of it.
    verify: bool,
    /// The refusal seen on the previous read, waiting to be seen again before it is learned.
    pending_refusal: Option<(f64, f64)>,
}

struct Worker {
    cfg: Config,
    mins: t::MinSizes,
    wins: Vec<Tracked>,
    /// Window ids per display, in layout order.
    order: Vec<Vec<u64>>,
    next_id: u64,
    focus: Option<t::FocusSaved>,
    emit: Box<dyn Fn(Event) + Send>,
    screens: (Option<Instant>, Vec<t::Rect>),
    stage: bool,
    hover: t::Hover,
    clock: Instant,
    /// Tiling was paused (or Stage Manager on) since windows were last synced.
    was_idle: bool,
}

impl Worker {
    fn new(emit: Box<dyn Fn(Event) + Send>) -> Worker {
        Worker {
            cfg: Config::default(),
            mins: Default::default(),
            wins: vec![],
            order: vec![],
            next_id: 1,
            focus: None,
            emit,
            screens: (None, vec![]),
            stage: false,
            hover: Default::default(),
            clock: Instant::now(),
            was_idle: false,
        }
    }

    fn screens(&mut self) -> Vec<t::Rect> {
        if self.screens.0.is_none_or(|t| t.elapsed() > Duration::from_millis(1500)) {
            self.screens = (Some(Instant::now()), screens_on_main());
        }
        self.screens.1.clone()
    }

    fn active(&self) -> bool {
        self.cfg.enabled && !self.cfg.paused && !self.stage
    }

    fn layout_of(&self, display: usize) -> t::Layout {
        let n = self.cfg.display_layouts.get(&display).copied().unwrap_or(self.cfg.layout);
        t::Layout::from_number(n).unwrap_or(t::Layout::Columns)
    }

    fn command(&mut self, c: Cmd) {
        match c {
            Cmd::Config(c) => {
                let was = self.cfg.enabled;
                self.cfg = c;
                if !self.cfg.enabled && was {
                    self.focus = None;
                }
            }
            Cmd::MinSizes(m) => self.mins = m,
            Cmd::Layout { n, pid } => {
                if !self.cfg.enabled || t::Layout::from_number(n).is_none() {
                    return;
                }
                let screens = self.screens();
                let display =
                    self.focused(pid).map(|i| self.wins[i].display).or_else(|| pointer().map(|p| display_of(p, &screens))).unwrap_or(0);
                self.cfg.display_layouts.insert(display, n);
                (self.emit)(Event::DisplayLayout { display, layout: n });
                for w in &mut self.wins {
                    w.tries = 0;
                }
            }
            Cmd::Focus { pid } => self.toggle_focus(pid),
            Cmd::Move { dir, pid } => self.move_window(dir, pid),
        }
    }

    /// Index of the focused window of `pid` among the tracked ones.
    fn focused(&self, pid: Option<i32>) -> Option<usize> {
        let pid = pid.or_else(|| running::frontmost().map(|f| f.pid))?;
        let f = windows::focused_window(pid)?;
        bound(f.0);
        self.wins.iter().position(|w| w.pid == pid && same(&w.el, &f))
    }

    fn refresh(&mut self) -> Vec<t::Rect> {
        let screens = self.screens();
        if screens.is_empty() || !windows::accessibility_trusted(false) {
            return screens;
        }
        let excluded = self.cfg.excluded.clone();
        let e = enumerate(&excluded);
        self.sync(e, &screens);
        screens
    }

    fn sync(&mut self, e: Enumerated, screens: &[t::Rect]) {
        self.order = t::reconcile_displays(std::mem::take(&mut self.order), screens.len());
        // a window's display is the list it is in (windows of a lost display moved to display 0)
        for w in &mut self.wins {
            w.display = t::display_index(&self.order, w.id).unwrap_or(0);
        }
        let resumed = self.active() && std::mem::take(&mut self.was_idle);
        let mut seen = vec![];
        for f in e.found {
            match self.wins.iter().position(|w| w.pid == f.pid && same(&w.el, &f.el)) {
                Some(i) => {
                    self.wins[i].frame = f.frame;
                    seen.push(self.wins[i].id);
                }
                None => {
                    let id = self.next_id;
                    self.next_id += 1;
                    let display = display_of(f.frame.center(), screens);
                    self.order[display].push(id);
                    self.wins.push(Tracked {
                        id,
                        pid: f.pid,
                        app: f.app,
                        el: f.el,
                        frame: f.frame,
                        display,
                        target: None,
                        tries: 0,
                        prev: None,
                        applied: None,
                        verify: false,
                        pending_refusal: None,
                    });
                    seen.push(id);
                }
            }
        }
        let tracked: Vec<(u64, i32)> = self.wins.iter().map(|w| (w.id, w.pid)).collect();
        let alive = survivors(&tracked, &seen, &e.failed);
        self.wins.retain(|w| alive.contains(&w.id));
        for o in &mut self.order {
            o.retain(|id| alive.contains(id));
        }
        if self.focus.as_ref().is_some_and(|f| !alive.contains(&f.focused)) {
            self.focus = None;
        }
        // an app that came out larger than the frame we asked for has taught us its minimum size;
        // one that accepted less than a stored minimum shows that value was wrong
        let focus_active = self.focus.is_some();
        let mut changed = false;
        for w in &mut self.wins {
            if !w.verify || focus_active {
                w.pending_refusal = None;
                continue;
            }
            let Some(ap) = w.applied else { continue };
            let cap = screens.get(w.display).map_or((f64::INFINITY, f64::INFINITY), |s| (s.w, s.h));
            let seen = t::should_learn(focus_active, w.verify, w.applied, w.frame);
            // an app that resizes asynchronously shows a transient size: learn only a repeated refusal
            match (seen, t::confirm_refusal(&mut w.pending_refusal, seen)) {
                (_, Some(got)) => changed |= self.mins.learn_capped(&w.app, (ap.w, ap.h), got, cap),
                (None, None) => changed |= self.mins.confirm(&w.app, (ap.w, ap.h), (w.frame.w, w.frame.h)),
                (Some(_), None) => {}
            }
        }
        if changed {
            (self.emit)(Event::MinSizes(self.mins.clone()));
            for w in &mut self.wins {
                w.tries = 0;
            }
        }
        // the user moved a tiled window: it changes display or swaps with the window it was dropped on
        if self.active() && !focus_active && !resumed {
            let moved: Vec<(u64, t::Rect)> = self
                .wins
                .iter()
                .filter(|w| w.target.is_some_and(|tg| t::user_moved(w.prev, w.frame, tg, w.verify)))
                .map(|w| (w.id, w.frame))
                .collect();
            for (id, frame) in moved {
                let to = display_of(frame.center(), screens);
                let from = self.wins.iter().find(|w| w.id == id).map_or(to, |w| w.display);
                if to != from {
                    t::move_to_display(&mut self.order, id, to);
                    self.set_display(id, to);
                } else if let Some(other) = self
                    .wins
                    .iter()
                    .find(|w| w.id != id && w.display == to && w.target.is_some_and(|tg| tg.contains(frame.center())))
                    .map(|w| w.id)
                {
                    let o = &mut self.order[to];
                    if let (Some(a), Some(b)) = (o.iter().position(|i| *i == id), o.iter().position(|i| *i == other)) {
                        o.swap(a, b);
                    }
                }
                if let Some(w) = self.wins.iter_mut().find(|w| w.id == id) {
                    w.tries = 0;
                }
            }
        }
        for w in &mut self.wins {
            w.prev = Some(w.frame);
            w.verify = false;
        }
    }

    fn set_display(&mut self, id: u64, d: usize) {
        if let Some(w) = self.wins.iter_mut().find(|w| w.id == id) {
            w.display = d;
        }
    }

    fn tick(&mut self) {
        if !windows::accessibility_trusted(false) {
            return;
        }
        let stage = stage_manager_on();
        if stage != self.stage {
            self.stage = stage;
            if self.cfg.enabled {
                (self.emit)(Event::StageManager(stage));
            }
        }
        if self.idle() {
            return;
        }
        if left_button_down() {
            // a pressed button means the user is dragging
            return;
        }
        let screens = self.refresh();
        if screens.is_empty() {
            return;
        }
        self.hover_activate();
        if self.active() && self.focus.is_none() {
            self.arrange(&screens);
        }
    }

    /// Nothing to observe or arrange this tick. Paused or under Stage Manager the tracked windows,
    /// their order and focus mode are kept (only enumeration and arranging stop); when tiling is
    /// off altogether the state is dropped.
    fn idle(&mut self) -> bool {
        if !self.active() {
            self.was_idle = true;
        }
        if self.active() || self.cfg.hover_activate {
            return false;
        }
        if !self.cfg.enabled {
            self.wins.clear();
            self.order.clear();
            self.focus = None;
            self.hover = Default::default();
        }
        true
    }

    fn arrange(&mut self, screens: &[t::Rect]) {
        for (d, screen) in screens.iter().enumerate().take(self.order.len()) {
            let list: Vec<t::Win> = self.order[d]
                .iter()
                .filter_map(|id| self.wins.iter().find(|w| w.id == *id))
                .map(|w| t::Win { id: w.id, min: self.mins.get(&w.app) })
                .collect();
            for p in t::tile(*screen, t::clamp_gap(self.cfg.gap), self.layout_of(d), &list) {
                if let Some(w) = self.wins.iter_mut().find(|w| w.id == p.id) {
                    if w.target.is_none_or(|tg| far(tg, p.frame, 1.0)) {
                        w.tries = 0;
                    }
                    w.target = Some(p.frame);
                    if !far(w.frame, p.frame, 2.0) {
                        w.tries = 0;
                    } else if w.tries < MAX_TRIES {
                        w.tries += 1;
                        w.applied = Some(p.frame);
                        w.verify = true;
                        set_frame(w.el.0, unconv(p.frame));
                    }
                }
            }
        }
    }

    fn hover_activate(&mut self) {
        if !self.cfg.hover_activate || self.stage {
            return;
        }
        let hit = pointer().and_then(|p| {
            let stack = window_stack();
            let top = t::top_window_at(&stack, p)?;
            let (pid, bounds, _) = stack[top];
            let mine: Vec<&Tracked> = self.wins.iter().filter(|w| w.pid == pid && w.frame.contains(p)).collect();
            mine.iter().find(|w| !far(w.frame, bounds, 6.0)).or(mine.first()).map(|w| w.id)
        });
        if let Some(id) = self.hover.step(hit, self.clock.elapsed().as_secs_f64(), HOVER_DWELL.as_secs_f64()) {
            if let Some(w) = self.wins.iter().find(|w| w.id == id) {
                raise(&w.el, w.pid);
            }
        }
    }

    fn toggle_focus(&mut self, pid: Option<i32>) {
        self.stage = stage_manager_on();
        if self.stage {
            // Stage Manager owns the layout: focus mode does nothing
            (self.emit)(Event::Notice("Focus mode is paused while Stage Manager is on".into()));
            return;
        }
        if !self.cfg.enabled {
            (self.emit)(Event::Notice("Turn on automatic tiling in Settings first".into()));
            return;
        }
        if let Some(saved) = self.focus.take() {
            for (id, r) in t::focus_restore(&saved) {
                if let Some(w) = self.wins.iter_mut().find(|w| w.id == id) {
                    set_frame(w.el.0, unconv(r));
                    w.tries = 0;
                    w.applied = None;
                    w.verify = false;
                }
            }
            return;
        }
        let screens = self.refresh();
        let Some(i) = self.focused(pid) else {
            (self.emit)(Event::Notice("No window to focus".into()));
            return;
        };
        if screens.is_empty() {
            return;
        }
        let current: Vec<(u64, t::Rect)> = self.wins.iter().map(|w| (w.id, w.frame)).collect();
        let screen = screens[self.wins[i].display.min(screens.len() - 1)];
        if let Some((saved, full)) = t::focus_enter(&current, self.wins[i].id, screen, t::clamp_gap(self.cfg.gap)) {
            for w in &mut self.wins {
                // whatever the frames do from now on is not the apps' minimum sizes
                w.applied = None;
                w.verify = false;
            }
            set_frame(self.wins[i].el.0, unconv(full));
            raise(&self.wins[i].el, self.wins[i].pid);
            self.focus = Some(saved);
        }
    }

    fn move_window(&mut self, dir: t::Dir, pid: Option<i32>) {
        if !self.active() || self.focus.is_some() {
            return;
        }
        let screens = self.refresh();
        let Some(i) = self.focused(pid) else { return };
        let (id, d) = (self.wins[i].id, self.wins[i].display);
        let frames: Vec<(u64, t::Rect)> = self.order[d]
            .iter()
            .filter_map(|id| self.wins.iter().find(|w| w.id == *id))
            .filter_map(|w| w.target.map(|t| (w.id, t)))
            .collect();
        if let Some(n) = t::neighbour(&frames, id, dir) {
            let o = &mut self.order[d];
            if let (Some(a), Some(b)) = (o.iter().position(|x| *x == id), o.iter().position(|x| *x == n)) {
                o.swap(a, b);
            }
        } else if let Some(to) = t::display_in_direction(&screens, d, dir) {
            t::move_to_display(&mut self.order, id, to);
            self.set_display(id, to);
        } else {
            return;
        }
        for w in &mut self.wins {
            w.tries = 0;
        }
        self.arrange(&screens);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiling_is_off_by_default_and_nothing_runs_without_a_worker() {
        let c = Config::default();
        assert!(!c.enabled && !c.paused && !c.hover_activate);
        assert_eq!(c.gap, 8.0);
    }

    fn tracked(id: u64) -> Tracked {
        // no real window: the element is never touched (the worker is never dropped)
        Tracked {
            id,
            pid: 1,
            app: "a".into(),
            el: AxElement(std::ptr::null_mut()),
            frame: t::Rect::new(0.0, 0.0, 100.0, 100.0),
            display: 0,
            target: None,
            tries: 0,
            prev: None,
            applied: None,
            verify: false,
            pending_refusal: None,
        }
    }

    fn worker_with_two_windows() -> std::mem::ManuallyDrop<Worker> {
        let mut w = Worker::new(Box::new(|_| {}));
        w.cfg.enabled = true;
        w.wins = vec![tracked(1), tracked(2)];
        w.order = vec![vec![2, 1]];
        w.focus = Some(t::FocusSaved { frames: vec![(1, t::Rect::new(1.0, 2.0, 3.0, 4.0))], focused: 1 });
        std::mem::ManuallyDrop::new(w)
    }

    #[test]
    fn pause_and_stage_manager_keep_the_order_and_focus_mode() {
        let mut w = worker_with_two_windows();
        w.cfg.paused = true;
        assert!(w.idle(), "paused: nothing to do");
        assert_eq!(w.order, vec![vec![2, 1]]);
        assert_eq!(w.wins.len(), 2);
        assert!(w.focus.is_some(), "focus mode survives a pause");
        w.cfg.paused = false;
        w.stage = true;
        assert!(w.idle());
        assert_eq!(w.order, vec![vec![2, 1]]);
        assert!(w.focus.is_some());
        assert!(w.was_idle, "the next sync knows tiling was away");
        // with hover on, the worker keeps observing (not idle) and still keeps its state
        w.cfg.hover_activate = true;
        assert!(!w.idle());
        assert_eq!(w.order, vec![vec![2, 1]]);
        // back on: the focus saved before the pause is still there to restore
        w.stage = false;
        assert!(!w.idle());
        assert_eq!(t::focus_restore(w.focus.as_ref().unwrap()).len(), 1);
    }

    #[test]
    fn turning_tiling_off_drops_the_tracked_state() {
        let mut w = worker_with_two_windows();
        w.cfg.enabled = false;
        assert!(w.idle());
        assert!(w.wins.is_empty() && w.order.is_empty() && w.focus.is_none());
    }

    #[test]
    fn display_of_picks_the_containing_then_the_nearest_display() {
        let s = [t::Rect::new(0.0, 0.0, 100.0, 100.0), t::Rect::new(100.0, 0.0, 100.0, 100.0)];
        assert_eq!(display_of((150.0, 50.0), &s), 1);
        assert_eq!(display_of((-30.0, 50.0), &s), 0);
        assert_eq!(display_of((400.0, 50.0), &s), 1);
    }

    #[test]
    fn windows_of_an_app_that_timed_out_keep_their_ids() {
        let tracked = [(1, 10), (2, 10), (3, 20)];
        assert_eq!(survivors(&tracked, &[3], &[10]), vec![1, 2, 3]);
        assert_eq!(survivors(&tracked, &[3], &[]), vec![3], "closed windows go");
        assert_eq!(survivors(&tracked, &[1, 2, 3], &[]), vec![1, 2, 3]);
    }

    #[test]
    fn a_stage_manager_preference_can_be_a_boolean_or_a_number() {
        use core_foundation::base::TCFType;
        use core_foundation::boolean::CFBoolean;
        use core_foundation::number::CFNumber;
        assert!(truthy(CFBoolean::true_value().as_CFType()));
        assert!(!truthy(CFBoolean::false_value().as_CFType()));
        assert!(truthy(CFNumber::from(1i32).as_CFType()));
        assert!(!truthy(CFNumber::from(0i32).as_CFType()));
        assert!(!truthy(CFString::new("yes").as_CFType()));
    }

    #[test]
    fn far_uses_a_tolerance_on_every_edge() {
        let a = t::Rect::new(0.0, 0.0, 100.0, 100.0);
        assert!(!far(a, t::Rect::new(1.0, 1.0, 101.0, 99.0), 2.0));
        assert!(far(a, t::Rect::new(0.0, 0.0, 100.0, 103.0), 2.0));
    }
}
