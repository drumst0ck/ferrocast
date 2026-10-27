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
    out.into_iter().map(conv).collect()
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
        core_foundation::boolean::CFBoolean::wrap_under_create_rule(v as _) == core_foundation::boolean::CFBoolean::true_value()
    }
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

fn enumerate(excluded: &[String]) -> Vec<Found> {
    let me = std::process::id() as i32;
    let mut out = vec![];
    for app in running::running_apps() {
        let key = app.bundle_id.clone().unwrap_or_else(|| app.name.clone());
        if app.pid == me || excluded.iter().any(|e| *e == key || *e == app.name) {
            continue;
        }
        unsafe {
            let ax = AxElement(AXUIElementCreateApplication(app.pid));
            AXUIElementSetMessagingTimeout(ax.0, 0.25);
            let Some(list) = copy_attr(ax.0, kAXWindowsAttribute) else { continue };
            let arr: core_foundation::array::CFArray<*const c_void> = core_foundation::array::CFArray::wrap_under_create_rule(list as _);
            for w in arr.iter() {
                let el = *w as AXUIElementRef;
                if !tileable(el) {
                    continue;
                }
                let Some(f) = frame_of(el) else { continue };
                if f.w < 120.0 || f.h < 80.0 {
                    continue;
                }
                CFRetain(el as CFTypeRef);
                out.push(Found { pid: app.pid, app: key.clone(), el: AxElement(el), frame: conv(f) });
            }
        }
    }
    out
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
        AXUIElementPerformAction(w.0, action.as_concrete_TypeRef());
        let app = AxElement(AXUIElementCreateApplication(pid));
        set_bool(app.0, "AXFrontmost", true);
    }
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
    hover: Option<(u64, Instant)>,
    hovered: Option<u64>,
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
            hover: None,
            hovered: None,
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
        self.wins.iter().position(|w| w.pid == pid && same(&w.el, &f))
    }

    fn refresh(&mut self) -> Vec<t::Rect> {
        let screens = self.screens();
        if screens.is_empty() || !windows::accessibility_trusted(false) {
            return screens;
        }
        let excluded = self.cfg.excluded.clone();
        let found = enumerate(&excluded);
        self.sync(found, &screens);
        screens
    }

    fn sync(&mut self, found: Vec<Found>, screens: &[t::Rect]) {
        self.order.resize(screens.len(), vec![]);
        // a display went away: its windows move to the first one
        for d in screens.len()..self.order.len() {
            let gone: Vec<u64> = self.order[d].drain(..).collect();
            self.order[0].extend(gone);
        }
        self.order.truncate(screens.len());
        for w in &mut self.wins {
            w.display = w.display.min(screens.len() - 1);
        }
        let mut alive = vec![];
        for f in found {
            match self.wins.iter().position(|w| w.pid == f.pid && same(&w.el, &f.el)) {
                Some(i) => {
                    self.wins[i].frame = f.frame;
                    alive.push(self.wins[i].id);
                }
                None => {
                    let id = self.next_id;
                    self.next_id += 1;
                    let display = display_of(f.frame.center(), screens);
                    self.order[display].push(id);
                    self.wins.push(Tracked { id, pid: f.pid, app: f.app, el: f.el, frame: f.frame, display, target: None, tries: 0 });
                    alive.push(id);
                }
            }
        }
        self.wins.retain(|w| alive.contains(&w.id));
        for o in &mut self.order {
            o.retain(|id| alive.contains(id));
        }
        if self.focus.as_ref().is_some_and(|f| !alive.contains(&f.focused)) {
            self.focus = None;
        }
        // an app that came out larger than asked has taught us its minimum size
        let mut learned = false;
        for w in &self.wins {
            if let (Some(tg), true) = (w.target, w.tries > 0) {
                learned |= self.mins.learn(&w.app, (tg.w, tg.h), (w.frame.w, w.frame.h));
            }
        }
        if learned {
            (self.emit)(Event::MinSizes(self.mins.clone()));
            for w in &mut self.wins {
                w.tries = 0;
            }
        }
        // the user moved a tiled window: it changes display or swaps with the window it was dropped on
        if self.active() && self.focus.is_none() {
            let moved: Vec<(u64, t::Rect)> = self
                .wins
                .iter()
                .filter(|w| w.target.is_some_and(|tg| (w.frame.x - tg.x).abs() > 4.0 || (w.frame.y - tg.y).abs() > 4.0))
                .map(|w| (w.id, w.frame))
                .collect();
            for (id, frame) in moved {
                let to = display_of(frame.center(), screens);
                let from = self.wins.iter().find(|w| w.id == id).map_or(to, |w| w.display);
                if to != from {
                    self.order[from].retain(|i| *i != id);
                    self.order[to].push(id);
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
        if !(self.cfg.enabled || self.cfg.hover_activate) {
            self.wins.clear();
            self.order.clear();
            return;
        }
        if self.stage || left_button_down() {
            // Stage Manager owns the layout; a pressed button means the user is dragging
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

    fn arrange(&mut self, screens: &[t::Rect]) {
        for (d, screen) in screens.iter().enumerate().take(self.order.len()) {
            let list: Vec<t::Win> = self.order[d]
                .iter()
                .filter_map(|id| self.wins.iter().find(|w| w.id == *id))
                .map(|w| t::Win { id: w.id, min: self.mins.get(&w.app) })
                .collect();
            for p in t::tile(*screen, self.cfg.gap, self.layout_of(d), &list) {
                if let Some(w) = self.wins.iter_mut().find(|w| w.id == p.id) {
                    if w.target.is_none_or(|tg| far(tg, p.frame, 1.0)) {
                        w.tries = 0;
                    }
                    w.target = Some(p.frame);
                    if !far(w.frame, p.frame, 2.0) {
                        w.tries = 0;
                    } else if w.tries < MAX_TRIES {
                        w.tries += 1;
                        set_frame(w.el.0, unconv(p.frame));
                    }
                }
            }
        }
    }

    fn hover_activate(&mut self) {
        if !self.cfg.hover_activate {
            return;
        }
        let Some(p) = pointer() else { return };
        let Some(hit) = self.wins.iter().rev().find(|w| w.frame.contains(p)).map(|w| w.id) else {
            self.hover = None;
            return;
        };
        match self.hover {
            Some((id, since)) if id == hit => {
                if since.elapsed() >= HOVER_DWELL && self.hovered != Some(hit) {
                    if let Some(w) = self.wins.iter().find(|w| w.id == hit) {
                        raise(&w.el, w.pid);
                    }
                    self.hovered = Some(hit);
                }
            }
            _ => self.hover = Some((hit, Instant::now())),
        }
    }

    fn toggle_focus(&mut self, pid: Option<i32>) {
        if !self.cfg.enabled {
            (self.emit)(Event::Notice("Turn on automatic tiling in Settings first".into()));
            return;
        }
        if let Some(saved) = self.focus.take() {
            for (id, r) in t::focus_restore(&saved) {
                if let Some(w) = self.wins.iter_mut().find(|w| w.id == id) {
                    set_frame(w.el.0, unconv(r));
                    w.tries = 0;
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
        if let Some((saved, full)) = t::focus_enter(&current, self.wins[i].id, screen, self.cfg.gap) {
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
            self.order[d].retain(|x| *x != id);
            self.order[to].push(id);
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

    #[test]
    fn display_of_picks_the_containing_then_the_nearest_display() {
        let s = [t::Rect::new(0.0, 0.0, 100.0, 100.0), t::Rect::new(100.0, 0.0, 100.0, 100.0)];
        assert_eq!(display_of((150.0, 50.0), &s), 1);
        assert_eq!(display_of((-30.0, 50.0), &s), 0);
        assert_eq!(display_of((400.0, 50.0), &s), 1);
    }

    #[test]
    fn far_uses_a_tolerance_on_every_edge() {
        let a = t::Rect::new(0.0, 0.0, 100.0, 100.0);
        assert!(!far(a, t::Rect::new(1.0, 1.0, 101.0, 99.0), 2.0));
        assert!(far(a, t::Rect::new(0.0, 0.0, 100.0, 103.0), 2.0));
    }
}
