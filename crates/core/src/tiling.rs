//! Automatic window tiling: the pure engine.
//!
//! Given a display's visible rect, the gap ("Space") and an ordered list of windows, `tile` returns a
//! frame for each one. Nothing here touches the OS, so every rule is unit tested; the macOS side
//! (`ferrocast_macos::tiling`) only observes windows and applies what this module computes.
//!
//! Coordinates are top-left origin points, like the Accessibility API uses.
//!
//! # Layouts (picked with 1-4)
//! 1. **Columns**: windows side by side, equal width, full height.
//! 2. **Main + stack**: the first window takes the left 60 %, the others stack as rows on the right.
//! 3. **Grid**: a square-ish grid filled row by row; a short last row stretches to the full width.
//! 4. **Spiral**: each window takes half of what is left, alternating side-by-side and
//!    top-and-bottom splits.
//!
//! # Rules
//! * Gap: `gap` is both the margin to the screen edge and the space between windows.
//! * Minimum sizes: a window may have a learned minimum size (an app refused a smaller frame).
//!   Split lines move to give such a window its minimum before the other windows share the rest.
//!   Windows without a learned size still get [`DEFAULT_MIN`], which also bounds how many
//!   windows a screen holds.
//! * Float: when the windows cannot all fit because of learned minimums, the window with the
//!   largest minimum is taken out of the layout and floats centred at its minimum size; this
//!   repeats until the rest fits.
//! * Stack when full: when more windows remain than slots fit, the extra ones share the last slot
//!   as an overlapping cascade (each window offset by [`CASCADE_STEP`] down and right, all inside
//!   that slot).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect { x, y, w, h }
    }
    pub fn center(&self) -> (f64, f64) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
    pub fn inset(&self, g: f64) -> Rect {
        Rect::new(self.x + g, self.y + g, (self.w - 2.0 * g).max(1.0), (self.h - 2.0 * g).max(1.0))
    }
    pub fn contains(&self, p: (f64, f64)) -> bool {
        p.0 >= self.x && p.0 < self.x + self.w && p.1 >= self.y && p.1 < self.y + self.h
    }
}

/// Smallest size a slot is assumed to need when an app has not taught us its own.
pub const DEFAULT_MIN: (f64, f64) = (320.0, 220.0);
/// Offset between windows stacked in the last slot.
pub const CASCADE_STEP: f64 = 28.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Layout {
    Columns,
    MainStack,
    Grid,
    Spiral,
}

impl Layout {
    pub const ALL: [Layout; 4] = [Layout::Columns, Layout::MainStack, Layout::Grid, Layout::Spiral];
    /// `1..=4` as in the ⌃⌥⌘1-4 shortcuts.
    pub fn from_number(n: u8) -> Option<Layout> {
        n.checked_sub(1).and_then(|i| Layout::ALL.get(i as usize)).copied()
    }
    pub fn number(self) -> u8 {
        Layout::ALL.iter().position(|l| *l == self).unwrap() as u8 + 1
    }
    pub fn name(self) -> &'static str {
        match self {
            Layout::Columns => "Columns",
            Layout::MainStack => "Main + stack",
            Layout::Grid => "Grid",
            Layout::Spiral => "Spiral",
        }
    }
}

/// A window to place. `min` is the learned minimum size, if any.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Win {
    pub id: u64,
    pub min: Option<(f64, f64)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Has a slot of its own.
    Tiled,
    /// Does not fit: centred at its minimum size, outside the layout.
    Floating,
    /// Shares the last slot with the other stacked windows.
    Stacked,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed {
    pub id: u64,
    pub frame: Rect,
    pub kind: Kind,
}

enum Node {
    Leaf(usize),
    /// Children with their weights, side by side (`vertical` split lines) or one under another.
    Split {
        vertical: bool,
        kids: Vec<(Node, f64)>,
    },
}

fn leaves(from: usize, to: usize) -> Vec<(Node, f64)> {
    (from..to).map(|i| (Node::Leaf(i), 1.0)).collect()
}

fn split(vertical: bool, mut kids: Vec<(Node, f64)>) -> Node {
    if kids.len() == 1 {
        kids.remove(0).0
    } else {
        Node::Split { vertical, kids }
    }
}

fn build(layout: Layout, k: usize) -> Node {
    match layout {
        Layout::Columns => split(true, leaves(0, k)),
        Layout::MainStack => {
            let mut kids = vec![(Node::Leaf(0), 0.6)];
            if k > 1 {
                kids.push((split(false, leaves(1, k)), 0.4));
            }
            split(true, kids)
        }
        Layout::Grid => {
            let cols = (k as f64).sqrt().ceil() as usize;
            let rows: Vec<(Node, f64)> = (0..k).step_by(cols).map(|r| (split(true, leaves(r, (r + cols).min(k))), 1.0)).collect();
            split(false, rows)
        }
        Layout::Spiral => spiral(0, k, true),
    }
}

fn spiral(i: usize, k: usize, vertical: bool) -> Node {
    if i + 1 >= k {
        Node::Leaf(i)
    } else {
        Node::Split { vertical, kids: vec![(Node::Leaf(i), 1.0), (spiral(i + 1, k, !vertical), 1.0)] }
    }
}

/// Smallest (width, height) a node needs.
fn need(n: &Node, gap: f64, mins: &[(f64, f64)]) -> (f64, f64) {
    match n {
        Node::Leaf(i) => mins[*i],
        Node::Split { vertical, kids } => {
            let sizes: Vec<(f64, f64)> = kids.iter().map(|(k, _)| need(k, gap, mins)).collect();
            let gaps = gap * (sizes.len() - 1) as f64;
            if *vertical {
                (sizes.iter().map(|s| s.0).sum::<f64>() + gaps, sizes.iter().map(|s| s.1).fold(0.0, f64::max))
            } else {
                (sizes.iter().map(|s| s.0).fold(0.0, f64::max), sizes.iter().map(|s| s.1).sum::<f64>() + gaps)
            }
        }
    }
}

/// Shares `total` between slots by weight, but never below a slot's minimum: a slot under its
/// minimum is pinned to it and the others share what is left. Assumes the minimums fit in `total`.
pub fn distribute(total: f64, weights: &[f64], mins: &[f64]) -> Vec<f64> {
    let n = weights.len();
    let mut size = vec![0.0; n];
    let mut pinned = vec![false; n];
    loop {
        let free: f64 = total - (0..n).filter(|i| pinned[*i]).map(|i| size[i]).sum::<f64>();
        let wsum: f64 = (0..n).filter(|i| !pinned[*i]).map(|i| weights[i]).sum();
        let mut again = false;
        for i in 0..n {
            if pinned[i] {
                continue;
            }
            size[i] = free * weights[i] / wsum;
        }
        for i in 0..n {
            if !pinned[i] && size[i] < mins[i] {
                size[i] = mins[i];
                pinned[i] = true;
                again = true;
            }
        }
        if !again {
            return size;
        }
    }
}

fn place(n: &Node, r: Rect, gap: f64, mins: &[(f64, f64)], out: &mut [Rect]) {
    match n {
        Node::Leaf(i) => out[*i] = r,
        Node::Split { vertical, kids } => {
            let total = if *vertical { r.w } else { r.h } - gap * (kids.len() - 1) as f64;
            let weights: Vec<f64> = kids.iter().map(|k| k.1).collect();
            let kmins: Vec<f64> = kids
                .iter()
                .map(|(k, _)| {
                    let s = need(k, gap, mins);
                    if *vertical {
                        s.0
                    } else {
                        s.1
                    }
                })
                .collect();
            let sizes = distribute(total, &weights, &kmins);
            let mut at = if *vertical { r.x } else { r.y };
            for ((k, _), s) in kids.iter().zip(sizes) {
                let kr = if *vertical { Rect::new(at, r.y, s, r.h) } else { Rect::new(r.x, at, r.w, s) };
                place(k, kr, gap, mins, out);
                at += s + gap;
            }
        }
    }
}

fn effective(m: Option<(f64, f64)>) -> (f64, f64) {
    m.map_or(DEFAULT_MIN, |m| (m.0.max(DEFAULT_MIN.0), m.1.max(DEFAULT_MIN.1)))
}

fn fits(layout: Layout, k: usize, area: Rect, gap: f64, mins: &[(f64, f64)]) -> bool {
    let (w, h) = need(&build(layout, k), gap, &mins[..k]);
    w <= area.w + 0.5 && h <= area.h + 0.5
}

/// How many slots of default minimum size the layout holds on this screen (at least one).
pub fn capacity(screen: Rect, gap: f64, layout: Layout, windows: usize) -> usize {
    let area = screen.inset(gap);
    let mins = vec![DEFAULT_MIN; windows.max(1)];
    let mut k = 1;
    while k < windows && fits(layout, k + 1, area, gap, &mins) {
        k += 1;
    }
    k
}

/// Frames for `wins` (in the same order) on `screen`. See the module docs for the rules.
pub fn tile(screen: Rect, gap: f64, layout: Layout, wins: &[Win]) -> Vec<Placed> {
    if !valid_screen(screen) {
        return vec![];
    }
    let gap = clamp_gap(gap);
    let area = screen.inset(gap);
    let mut result: Vec<Option<Placed>> = vec![None; wins.len()];
    let mut live: Vec<usize> = (0..wins.len()).collect();
    loop {
        if live.is_empty() {
            break;
        }
        let k = capacity(screen, gap, layout, live.len());
        let mins: Vec<(f64, f64)> = live[..k].iter().map(|i| effective(wins[*i].min)).collect();
        if !fits(layout, k, area, gap, &mins) {
            // cannot fit because of learned minimums: float the hungriest window and retry
            let worst = (0..k).max_by(|a, b| (mins[*a].0 * mins[*a].1).total_cmp(&(mins[*b].0 * mins[*b].1))).unwrap();
            let i = live.remove(worst);
            let m = effective(wins[i].min);
            let (w, h) = (m.0.min(area.w), m.1.min(area.h));
            let c = area.center();
            result[i] = Some(Placed { id: wins[i].id, frame: Rect::new(c.0 - w / 2.0, c.1 - h / 2.0, w, h), kind: Kind::Floating });
            continue;
        }
        let mut slots = vec![Rect::new(0.0, 0.0, 0.0, 0.0); k];
        place(&build(layout, k), area, gap, &mins, &mut slots);
        for (s, i) in live[..k].iter().enumerate() {
            let kind = if k < live.len() && s == k - 1 { Kind::Stacked } else { Kind::Tiled };
            result[*i] = Some(Placed { id: wins[*i].id, frame: slots[s], kind });
        }
        if k < live.len() {
            // the cascade shrinks each frame: a window whose learned minimum no longer fits floats
            let mut group: Vec<usize> = live[k - 1..].to_vec();
            let slot = slots[k - 1];
            loop {
                let step = CASCADE_STEP.min(slot.w.min(slot.h) / (2.0 * group.len() as f64));
                let total = step * (group.len() - 1) as f64;
                let (fw, fh) = ((slot.w - total).max(1.0), (slot.h - total).max(1.0));
                let too_big = group.iter().position(|i| wins[*i].min.is_some_and(|m| m.0 > fw + 0.5 || m.1 > fh + 0.5));
                if let Some(g) = too_big {
                    let i = group.remove(g);
                    let m = wins[i].min.unwrap();
                    let (w, h) = (m.0.min(area.w), m.1.min(area.h));
                    let c = area.center();
                    result[i] = Some(Placed { id: wins[i].id, frame: Rect::new(c.0 - w / 2.0, c.1 - h / 2.0, w, h), kind: Kind::Floating });
                    if group.is_empty() {
                        break;
                    }
                    continue;
                }
                let kind = if group.len() == 1 { Kind::Tiled } else { Kind::Stacked };
                for (j, i) in group.iter().enumerate() {
                    let off = step * j as f64;
                    let frame = Rect::new(slot.x + off, slot.y + off, fw, fh);
                    result[*i] = Some(Placed { id: wins[*i].id, frame, kind });
                }
                break;
            }
        }
        break;
    }
    // floating windows are centred; each further one is offset a little so they do not coincide
    let mut floats = 0.0;
    for p in result.iter_mut().flatten().filter(|p| p.kind == Kind::Floating) {
        let off = CASCADE_STEP * floats;
        p.frame.x = (p.frame.x + off).min((area.x + area.w - p.frame.w).max(area.x));
        p.frame.y = (p.frame.y + off).min((area.y + area.h - p.frame.h).max(area.y));
        floats += 1.0;
    }
    result.into_iter().flatten().collect()
}

/// Largest Space accepted, in points.
pub const MAX_GAP: f64 = 64.0;

/// `gap` limited to `0..=MAX_GAP`; a non-finite value counts as 0.
pub fn clamp_gap(gap: f64) -> f64 {
    if gap.is_finite() {
        gap.clamp(0.0, MAX_GAP)
    } else {
        0.0
    }
}

/// A display rect the engine can use: finite and at least 1 pt in each direction.
pub fn valid_screen(r: Rect) -> bool {
    [r.x, r.y, r.w, r.h].iter().all(|v| v.is_finite()) && r.w >= 1.0 && r.h >= 1.0
}

/// `order` (window ids per display) for `n` displays (at least 1): the lists of displays that went
/// away are appended to the first display, so every window stays in some list.
pub fn reconcile_displays(mut order: Vec<Vec<u64>>, n: usize) -> Vec<Vec<u64>> {
    let n = n.max(1);
    let mut seen = std::collections::HashSet::new();
    for l in order.iter_mut() {
        l.retain(|id| seen.insert(*id));
    }
    if order.len() > n {
        let gone: Vec<u64> = order.split_off(n).into_iter().flatten().collect();
        order[0].extend(gone);
    }
    order.resize(n, vec![]);
    order
}

const SLACK: f64 = 2.0;

/// Index of the display whose list holds window `id`.
pub fn display_index(order: &[Vec<u64>], id: u64) -> Option<usize> {
    order.iter().position(|l| l.contains(&id))
}

/// Put `id` at the end of display `to`'s list and take it out of every other list (and any
/// earlier place in that one), so a window is never listed twice.
pub fn move_to_display(order: &mut [Vec<u64>], id: u64, to: usize) {
    if order.is_empty() {
        return;
    }
    let to = to.min(order.len() - 1);
    for l in order.iter_mut() {
        l.retain(|i| *i != id);
    }
    order[to].push(id);
}

/// A refusal counts only when the same one is seen on two consecutive reads: an app that resizes
/// asynchronously shows a transient size once. `pending` is the previous read's refusal, `seen` the
/// current one; returns the size to learn.
pub fn confirm_refusal(pending: &mut Option<(f64, f64)>, seen: Option<(f64, f64)>) -> Option<(f64, f64)> {
    let Some(s) = seen else {
        *pending = None;
        return None;
    };
    if pending.is_some_and(|p| (p.0 - s.0).abs() <= SLACK && (p.1 - s.1).abs() <= SLACK) {
        *pending = None;
        Some(s)
    } else {
        *pending = Some(s);
        None
    }
}

/// The size an app refused to go below: `Some(read size)` when, in the step that applied
/// `applied`, the window kept its position but came out larger than asked.
pub fn refusal(applied: Rect, read: Rect) -> Option<(f64, f64)> {
    let kept = (read.x - applied.x).abs() <= 4.0 && (read.y - applied.y).abs() <= 4.0;
    (kept && (read.w > applied.w + SLACK || read.h > applied.h + SLACK)).then_some((read.w, read.h))
}

/// Whether a read-back may teach a minimum size: never during focus mode, only on the first read
/// after our own arrange step (`verify`), and only for a refusal of the frame we applied.
pub fn should_learn(focus_active: bool, verify: bool, applied: Option<Rect>, read: Rect) -> Option<(f64, f64)> {
    if focus_active || !verify {
        return None;
    }
    refusal(applied?, read)
}

/// Did the user move or resize the window? Only when it changed since the previous read (so an app
/// that keeps refusing our frame is not mistaken for a user) and not because we applied a frame.
pub fn user_moved(prev: Option<Rect>, now: Rect, target: Rect, applied_since_read: bool) -> bool {
    let differs =
        |a: Rect, b: Rect| (a.x - b.x).abs() > 4.0 || (a.y - b.y).abs() > 4.0 || (a.w - b.w).abs() > 4.0 || (a.h - b.h).abs() > 4.0;
    !applied_since_read && differs(now, target) && prev.is_some_and(|p| differs(p, now))
}

/// Hover activation: raise a window once the pointer has rested on it, again after the pointer
/// left it.
#[derive(Debug, Default)]
pub struct Hover {
    over: Option<(u64, f64)>,
    raised: Option<u64>,
}

impl Hover {
    /// `hit` is the window on top under the pointer; returns the window to raise now, if any.
    pub fn step(&mut self, hit: Option<u64>, now: f64, dwell: f64) -> Option<u64> {
        let Some(hit) = hit else {
            self.over = None;
            self.raised = None;
            return None;
        };
        if self.raised == Some(hit) {
            return None;
        }
        match self.over {
            Some((id, since)) if id == hit => {
                if now - since >= dwell {
                    self.raised = Some(hit);
                    Some(hit)
                } else {
                    None
                }
            }
            _ => {
                self.over = Some((hit, now));
                self.raised = None;
                None
            }
        }
    }
}

/// The first entry (front to back) whose bounds hold `p`, if it is an ordinary (layer 0) window.
pub fn top_window_at(entries: &[(i32, Rect, i64)], p: (f64, f64)) -> Option<usize> {
    let i = entries.iter().position(|(_, r, _)| r.contains(p))?;
    (entries[i].2 == 0).then_some(i)
}

/// What focus mode remembers: every frame, and the window that was expanded.
#[derive(Debug, Clone, PartialEq)]
pub struct FocusSaved {
    pub frames: Vec<(u64, Rect)>,
    pub focused: u64,
}

/// Enter focus mode: save every current frame and return the one frame for `focused` (the screen
/// minus the gap). `None` when `focused` is not among `current`.
pub fn focus_enter(current: &[(u64, Rect)], focused: u64, screen: Rect, gap: f64) -> Option<(FocusSaved, Rect)> {
    current.iter().any(|(id, _)| *id == focused).then(|| (FocusSaved { frames: current.to_vec(), focused }, screen.inset(gap)))
}

/// Leave focus mode: exactly the frames that were saved.
pub fn focus_restore(saved: &FocusSaved) -> Vec<(u64, Rect)> {
    saved.frames.clone()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

impl Dir {
    pub const ALL: [Dir; 4] = [Dir::Left, Dir::Right, Dir::Up, Dir::Down];
    pub fn name(self) -> &'static str {
        match self {
            Dir::Left => "Left",
            Dir::Right => "Right",
            Dir::Up => "Up",
            Dir::Down => "Down",
        }
    }
    /// (along, across) of the vector `d` for this direction: positive `along` is ahead.
    fn axes(self, d: (f64, f64)) -> (f64, f64) {
        match self {
            Dir::Left => (-d.0, d.1.abs()),
            Dir::Right => (d.0, d.1.abs()),
            Dir::Up => (-d.1, d.0.abs()),
            Dir::Down => (d.1, d.0.abs()),
        }
    }
}

/// The window next to `from` in direction `dir`: the nearest centre ahead, preferring the one most
/// in line.
pub fn neighbour(frames: &[(u64, Rect)], from: u64, dir: Dir) -> Option<u64> {
    let c = frames.iter().find(|f| f.0 == from)?.1.center();
    frames
        .iter()
        .filter(|f| f.0 != from)
        .filter_map(|(id, r)| {
            let rc = r.center();
            let (along, across) = dir.axes((rc.0 - c.0, rc.1 - c.1));
            (along > 1.0).then_some((*id, along + 2.0 * across))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(id, _)| id)
}

/// The display next to display `from` in direction `dir` (nearest centre ahead).
pub fn display_in_direction(screens: &[Rect], from: usize, dir: Dir) -> Option<usize> {
    let frames: Vec<(u64, Rect)> = screens.iter().enumerate().map(|(i, r)| (i as u64, *r)).collect();
    neighbour(&frames, from as u64, dir).map(|i| i as usize)
}

/// Smallest size each app accepts, learned when an app refuses a frame. Keyed by bundle id (or
/// app name); never by window title. Kept in the local data folder only.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MinSizes(pub BTreeMap<String, (f64, f64)>);

impl MinSizes {
    pub fn get(&self, app: &str) -> Option<(f64, f64)> {
        self.0.get(app).copied()
    }
    /// An app was asked for `asked` and ended up at `got`: when it came out larger, that is its
    /// minimum. Returns whether the stored minimum grew.
    pub fn learn_capped(&mut self, app: &str, asked: (f64, f64), got: (f64, f64), cap: (f64, f64)) -> bool {
        let (gw, gh) = (got.0 > asked.0 + SLACK, got.1 > asked.1 + SLACK);
        if !gw && !gh {
            return false;
        }
        let old = self.get(app).unwrap_or((0.0, 0.0));
        let new = (if gw { old.0.max(got.0.min(cap.0)) } else { old.0 }, if gh { old.1.max(got.1.min(cap.1)) } else { old.1 });
        let changed = new != old;
        if changed {
            self.0.insert(app.to_string(), new);
        }
        changed
    }
    /// The app was asked for `asked` and accepted it (`got` is no larger): a stored minimum above
    /// what it accepted was wrong and is lowered. Returns whether anything changed.
    pub fn confirm(&mut self, app: &str, asked: (f64, f64), got: (f64, f64)) -> bool {
        let Some(old) = self.get(app) else { return false };
        let (aw, ah) = (got.0 <= asked.0 + SLACK, got.1 <= asked.1 + SLACK);
        let new = (if aw && got.0 < old.0 { got.0 } else { old.0 }, if ah && got.1 < old.1 { got.1 } else { old.1 });
        if new == old {
            return false;
        }
        self.0.insert(app.to_string(), new);
        true
    }
    pub fn learn(&mut self, app: &str, asked: (f64, f64), got: (f64, f64)) -> bool {
        self.learn_capped(app, asked, got, (f64::INFINITY, f64::INFINITY))
    }
}

/// The launcher commands and hotkey targets of tiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    /// Turn automatic tiling on or off.
    Toggle,
    /// Pause or resume without touching the windows.
    Pause,
    Layout(u8),
    FocusMode,
    Move(Dir),
    /// Take the focused window out of the layout (it floats) or put it back.
    FloatToggle,
    /// Send the focused window to the next display.
    NextDisplay,
    /// Swap the focused window with the main slot (or the next slot when it is the main one).
    SwapMain,
    /// Stop tiling the focused window's app (adds it to the exclusion list).
    ExcludeApp,
}

impl Action {
    pub const ALL: [Action; 15] = [
        Action::Toggle,
        Action::Pause,
        Action::Layout(1),
        Action::Layout(2),
        Action::Layout(3),
        Action::Layout(4),
        Action::FocusMode,
        Action::Move(Dir::Left),
        Action::Move(Dir::Right),
        Action::Move(Dir::Up),
        Action::Move(Dir::Down),
        Action::FloatToggle,
        Action::NextDisplay,
        Action::SwapMain,
        Action::ExcludeApp,
    ];
    pub fn id(self) -> String {
        match self {
            Action::Toggle => "tiling:toggle".into(),
            Action::Pause => "tiling:pause".into(),
            Action::Layout(n) => format!("tiling:layout-{n}"),
            Action::FocusMode => "tiling:focus-mode".into(),
            Action::Move(d) => format!("tiling:move-{}", d.name().to_lowercase()),
            Action::FloatToggle => "tiling:float-toggle".into(),
            Action::NextDisplay => "tiling:next-display".into(),
            Action::SwapMain => "tiling:swap-main".into(),
            Action::ExcludeApp => "tiling:exclude-app".into(),
        }
    }
    pub fn from_id(id: &str) -> Option<Action> {
        Action::ALL.into_iter().find(|a| a.id() == id)
    }
    pub fn title(self) -> String {
        match self {
            Action::Toggle => "Tile Windows On/Off".into(),
            Action::Pause => "Pause Tiling".into(),
            Action::Layout(n) => format!("Tiling Layout {n}: {}", Layout::from_number(n).map_or("", Layout::name)),
            Action::FocusMode => "Tiling Focus Mode".into(),
            Action::Move(d) => format!("Move Window {}", d.name()),
            Action::FloatToggle => "Float or Tile This Window".into(),
            Action::NextDisplay => "Move Window to Next Display (Tiling)".into(),
            Action::SwapMain => "Swap Window with Main Slot".into(),
            Action::ExcludeApp => "Stop Tiling This App".into(),
        }
    }
}

/// Width of the highlight border, in points.
pub const BORDER_WIDTH: f64 = 3.0;
/// Corner radius of a macOS window; the border follows it.
pub const WINDOW_RADIUS: f64 = 10.0;

/// Where the border overlay goes: a frame just outside the window, and the corner radius.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BorderGeom {
    pub frame: Rect,
    pub radius: f64,
    pub width: f64,
}

/// The border overlay for `window` on `screen` (both top-left points); `None` when the window is
/// not on that screen or has no usable size.
pub fn border_geometry(window: Rect, screen: Rect, width: f64) -> Option<BorderGeom> {
    if !valid_screen(window) || !valid_screen(screen) {
        return None;
    }
    let apart = window.x >= screen.x + screen.w
        || window.x + window.w <= screen.x
        || window.y >= screen.y + screen.h
        || window.y + window.h <= screen.y;
    if apart {
        return None;
    }
    let width = width.clamp(1.0, 12.0);
    let frame = Rect::new(window.x - width, window.y - width, window.w + 2.0 * width, window.h + 2.0 * width);
    Some(BorderGeom { frame, radius: WINDOW_RADIUS + width, width })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BorderInputs {
    pub tiling_on: bool,
    pub border_on: bool,
    pub paused: bool,
    pub stage_manager: bool,
    /// Focus mode was just entered or left and the frames have not settled.
    pub focus_settling: bool,
    /// The focused window is one of the tiled ones.
    pub focused_tiled: bool,
    /// The mouse button is down (the user may be dragging or resizing the window).
    pub dragging: bool,
}

/// Is the highlight border shown?
pub fn border_visible(i: &BorderInputs) -> bool {
    i.tiling_on && i.border_on && !i.paused && !i.stage_manager && !i.focus_settling && i.focused_tiled && !i.dragging
}

pub const BAR_W: f64 = 196.0;
pub const BAR_H: f64 = 32.0;
const BAR_GAP: f64 = 6.0;
/// Height of the strip along a window's top edge where resting the pointer shows the bar.
pub const EDGE_BAND: f64 = 28.0;
pub const BAR_DWELL: f64 = 0.35;
pub const BAR_GRACE: f64 = 0.5;
/// Height of the title bar / tab strip the bar must never cover.
pub const TOP_STRIP: f64 = 38.0;

/// Frame of the quick-action bar for `window`: centred above its top edge when there is room on
/// `screen`; otherwise a compact pill at the window's top-right corner, below the title bar / tab
/// strip (`TOP_STRIP`) so it never covers tabs or window buttons. Always inside `screen`
/// horizontally.
pub fn bar_frame(window: Rect, screen: Rect) -> Rect {
    let above = window.y - BAR_H - BAR_GAP;
    let (x, y) = if above >= screen.y {
        (window.center().0 - BAR_W / 2.0, above)
    } else {
        ((window.x + window.w - BAR_W - BAR_GAP).max(window.x), window.y + TOP_STRIP + BAR_GAP)
    };
    Rect::new(x.min(screen.x + screen.w - BAR_W).max(screen.x), y, BAR_W, BAR_H)
}

/// Is `p` in the place that shows (and keeps) the bar of `window`: the top band or the bar itself.
pub fn in_bar_zone(window: Rect, screen: Rect, p: (f64, f64)) -> bool {
    Rect::new(window.x, window.y, window.w, EDGE_BAND.min(window.h)).contains(p) || bar_frame(window, screen).contains(p)
}

/// When the quick-action bar is shown: after the pointer rests in a window's zone for
/// `BAR_DWELL`, until it has been out of every zone for `BAR_GRACE`.
#[derive(Debug, Default)]
pub struct QuickBar {
    over: Option<(u64, f64)>,
    shown: Option<u64>,
    left_at: Option<f64>,
}

impl QuickBar {
    /// `wins` are (id, frame, screen) of the tiled windows. Returns the window the bar is for.
    pub fn step(&mut self, enabled: bool, pointer: Option<(f64, f64)>, pressed: bool, wins: &[(u64, Rect, Rect)], now: f64) -> Option<u64> {
        if !enabled {
            *self = QuickBar::default();
            return None;
        }
        if pressed {
            // a press (a tab click, a drag) never starts the bar and hides it, unless the press is on the bar
            let on_bar = pointer.is_some_and(|p| {
                self.shown.and_then(|id| wins.iter().find(|w| w.0 == id)).is_some_and(|(_, w, s)| bar_frame(*w, *s).contains(p))
            });
            if !on_bar {
                self.over = None;
                self.shown = None;
                self.left_at = None;
            }
            return self.shown;
        }
        let hit = pointer.and_then(|p| wins.iter().find(|(_, w, s)| in_bar_zone(*w, *s, p)).map(|(id, _, _)| *id));
        match hit {
            Some(id) => {
                self.left_at = None;
                if self.shown != Some(id) {
                    match self.over {
                        Some((o, since)) if o == id => {
                            if now - since >= BAR_DWELL {
                                self.shown = Some(id);
                                self.over = None;
                            }
                        }
                        _ => self.over = Some((id, now)),
                    }
                }
            }
            None => {
                self.over = None;
                if self.shown.is_some() {
                    let since = *self.left_at.get_or_insert(now);
                    if now - since >= BAR_GRACE {
                        self.shown = None;
                        self.left_at = None;
                    }
                }
            }
        }
        self.shown
    }
}

/// The buttons of the quick-action bar, left to right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickAction {
    Focus,
    Float,
    NextDisplay,
    Swap,
    Exclude,
}

impl QuickAction {
    pub const ALL: [QuickAction; 5] =
        [QuickAction::Focus, QuickAction::Float, QuickAction::NextDisplay, QuickAction::Swap, QuickAction::Exclude];
    /// The command the button sends: the very one the launcher and the hotkeys use.
    pub fn action(self) -> Action {
        match self {
            QuickAction::Focus => Action::FocusMode,
            QuickAction::Float => Action::FloatToggle,
            QuickAction::NextDisplay => Action::NextDisplay,
            QuickAction::Swap => Action::SwapMain,
            QuickAction::Exclude => Action::ExcludeApp,
        }
    }
}

/// Swap `id` with the main (first) slot, or with the second one when it already is the main.
pub fn swap_with_main(list: &mut [u64], id: u64) -> bool {
    let Some(at) = list.iter().position(|i| *i == id) else { return false };
    let with = if at == 0 { 1 } else { 0 };
    if with >= list.len() {
        return false;
    }
    list.swap(at, with);
    true
}

/// The display after `from` among `n`, wrapping; `None` with a single display.
pub fn next_display(n: usize, from: usize) -> Option<usize> {
    (n > 1).then(|| (from + 1) % n)
}

/// Hotkeys bound out of the box (global-hotkey syntax). They are only registered while tiling is
/// on, except the on/off command itself, which has none.
pub const DEFAULT_HOTKEYS: &[(&str, &str)] = &[
    ("tiling:layout-1", "Control+Alt+Super+Digit1"),
    ("tiling:layout-2", "Control+Alt+Super+Digit2"),
    ("tiling:layout-3", "Control+Alt+Super+Digit3"),
    ("tiling:layout-4", "Control+Alt+Super+Digit4"),
    ("tiling:focus-mode", "Control+Alt+KeyZ"),
    ("tiling:move-left", "Control+Alt+ArrowLeft"),
    ("tiling:move-right", "Control+Alt+ArrowRight"),
    ("tiling:move-up", "Control+Alt+ArrowUp"),
    ("tiling:move-down", "Control+Alt+ArrowDown"),
];

#[cfg(test)]
mod tests {
    use super::*;

    const S: Rect = Rect::new(0.0, 25.0, 1600.0, 900.0);
    fn w(id: u64) -> Win {
        Win { id, min: None }
    }
    fn ws(n: u64) -> Vec<Win> {
        (1..=n).map(w).collect()
    }
    fn frames(p: &[Placed]) -> Vec<Rect> {
        p.iter().map(|p| p.frame).collect()
    }
    fn overlap(a: Rect, b: Rect) -> bool {
        a.x < b.x + b.w - 0.01 && b.x < a.x + a.w - 0.01 && a.y < b.y + b.h - 0.01 && b.y < a.y + a.h - 0.01
    }
    fn assert_disjoint_inside(p: &[Placed], area: Rect) {
        for (i, a) in p.iter().enumerate() {
            assert!(a.frame.x >= area.x - 0.01 && a.frame.y >= area.y - 0.01, "{a:?}");
            assert!(a.frame.x + a.frame.w <= area.x + area.w + 0.01 && a.frame.y + a.frame.h <= area.y + area.h + 0.01, "{a:?}");
            for b in &p[i + 1..] {
                assert!(!overlap(a.frame, b.frame), "{a:?} overlaps {b:?}");
            }
        }
    }

    #[test]
    fn layout_numbers_round_trip() {
        for n in 1..=4 {
            assert_eq!(Layout::from_number(n).unwrap().number(), n);
        }
        assert_eq!(Layout::from_number(0), None);
        assert_eq!(Layout::from_number(5), None);
    }

    #[test]
    fn one_window_fills_the_area_inside_the_gap() {
        for l in Layout::ALL {
            let p = tile(S, 8.0, l, &ws(1));
            assert_eq!(frames(&p), vec![Rect::new(8.0, 33.0, 1584.0, 884.0)], "{l:?}");
            assert_eq!(p[0].kind, Kind::Tiled);
        }
    }

    #[test]
    fn columns_share_the_width_equally_with_gaps() {
        let p = tile(S, 10.0, Layout::Columns, &ws(3));
        // 1600 - 2*10 margin - 2*10 gaps = 1560 → 520 each
        assert_eq!(
            frames(&p),
            vec![Rect::new(10.0, 35.0, 520.0, 880.0), Rect::new(540.0, 35.0, 520.0, 880.0), Rect::new(1070.0, 35.0, 520.0, 880.0)]
        );
    }

    #[test]
    fn main_and_stack_puts_the_first_window_left_and_stacks_the_rest() {
        let p = tile(S, 0.0, Layout::MainStack, &ws(3));
        assert_eq!(p[0].frame, Rect::new(0.0, 25.0, 960.0, 900.0));
        assert_eq!(p[1].frame, Rect::new(960.0, 25.0, 640.0, 450.0));
        assert_eq!(p[2].frame, Rect::new(960.0, 475.0, 640.0, 450.0));
    }

    #[test]
    fn grid_fills_rows_and_stretches_a_short_last_row() {
        let p = tile(S, 0.0, Layout::Grid, &ws(3));
        // 2 columns × 2 rows; the last row has one window across the full width
        assert_eq!(p[0].frame, Rect::new(0.0, 25.0, 800.0, 450.0));
        assert_eq!(p[1].frame, Rect::new(800.0, 25.0, 800.0, 450.0));
        assert_eq!(p[2].frame, Rect::new(0.0, 475.0, 1600.0, 450.0));
        let p4 = tile(S, 0.0, Layout::Grid, &ws(4));
        assert_eq!(p4[3].frame, Rect::new(800.0, 475.0, 800.0, 450.0));
    }

    #[test]
    fn spiral_halves_what_is_left_and_alternates_direction() {
        let p = tile(S, 0.0, Layout::Spiral, &ws(3));
        assert_eq!(p[0].frame, Rect::new(0.0, 25.0, 800.0, 900.0));
        assert_eq!(p[1].frame, Rect::new(800.0, 25.0, 800.0, 450.0));
        assert_eq!(p[2].frame, Rect::new(800.0, 475.0, 800.0, 450.0));
    }

    #[test]
    fn every_layout_gives_disjoint_frames_inside_the_area_for_two_to_four_windows() {
        for l in Layout::ALL {
            for n in 2..=4 {
                let p = tile(S, 8.0, l, &ws(n));
                assert_eq!(p.len(), n as usize);
                assert!(p.iter().all(|p| p.kind == Kind::Tiled), "{l:?} {n}");
                assert_disjoint_inside(&p, S.inset(8.0));
            }
        }
    }

    #[test]
    fn gap_is_the_space_between_neighbours() {
        let p = tile(S, 12.0, Layout::Columns, &ws(2));
        let (a, b) = (p[0].frame, p[1].frame);
        assert!((b.x - (a.x + a.w) - 12.0).abs() < 1e-9);
        assert_eq!(a.x, 12.0);
        assert!((b.x + b.w - (1600.0 - 12.0)).abs() < 1e-9);
    }

    #[test]
    fn distribute_moves_split_lines_to_respect_minimums() {
        assert_eq!(distribute(1000.0, &[1.0, 1.0], &[0.0, 0.0]), vec![500.0, 500.0]);
        assert_eq!(distribute(1000.0, &[1.0, 1.0], &[700.0, 0.0]), vec![700.0, 300.0]);
        let d = distribute(1000.0, &[1.0, 1.0, 1.0], &[450.0, 450.0, 0.0]);
        assert_eq!(d, vec![450.0, 450.0, 100.0]);
    }

    #[test]
    fn a_learned_minimum_moves_the_split_line() {
        let mut v = ws(2);
        v[0].min = Some((1000.0, 300.0));
        let p = tile(S, 0.0, Layout::Columns, &v);
        assert_eq!(p[0].frame.w, 1000.0);
        assert_eq!(p[1].frame.w, 600.0);
        assert_eq!(p[1].frame.x, 1000.0);
        assert!(p.iter().all(|p| p.kind == Kind::Tiled));
    }

    #[test]
    fn a_window_that_cannot_fit_floats_centred_and_the_rest_re_tile() {
        let mut v = ws(3);
        v[1].min = Some((1500.0, 800.0));
        let p = tile(S, 0.0, Layout::Columns, &v);
        assert_eq!(p[1].kind, Kind::Floating);
        assert_eq!(p[1].frame, Rect::new(50.0, 75.0, 1500.0, 800.0));
        assert_eq!((p[0].kind, p[2].kind), (Kind::Tiled, Kind::Tiled));
        assert_eq!(p[0].frame, Rect::new(0.0, 25.0, 800.0, 900.0));
        assert_eq!(p[2].frame, Rect::new(800.0, 25.0, 800.0, 900.0));
    }

    #[test]
    fn a_floating_window_never_exceeds_the_screen() {
        let mut v = ws(1);
        v[0].min = Some((5000.0, 5000.0));
        let p = tile(S, 8.0, Layout::Columns, &v);
        // a single window always has the whole area; it only floats when others also need room
        assert_eq!(p.len(), 1);
        assert!(p[0].frame.w <= 1584.0 + 1e-9 && p[0].frame.h <= 884.0 + 1e-9);
    }

    #[test]
    fn extra_windows_stack_in_the_last_slot_when_the_screen_is_full() {
        // 1600 wide holds 5 default-minimum (320) columns with no gap
        let p = tile(S, 0.0, Layout::Columns, &ws(7));
        assert_eq!(p.len(), 7);
        assert!(p[..4].iter().all(|p| p.kind == Kind::Tiled));
        assert!(p[4..].iter().all(|p| p.kind == Kind::Stacked));
        // the first four have their own columns, the last three cascade inside the fifth
        let slot = Rect::new(1280.0, 25.0, 320.0, 900.0);
        for s in &p[4..] {
            assert!(s.frame.x >= slot.x && s.frame.x + s.frame.w <= slot.x + slot.w + 1e-9, "{s:?}");
            assert!(s.frame.y >= slot.y && s.frame.y + s.frame.h <= slot.y + slot.h + 1e-9, "{s:?}");
        }
        assert!(p[5].frame.x > p[4].frame.x && p[6].frame.x > p[5].frame.x, "cascade offsets");
        for a in &p[..4] {
            for b in &p[4..] {
                assert!(!overlap(a.frame, b.frame));
            }
        }
    }

    #[test]
    fn capacity_counts_default_minimum_slots() {
        assert_eq!(capacity(S, 0.0, Layout::Columns, 10), 5);
        assert_eq!(capacity(S, 0.0, Layout::Columns, 3), 3);
        assert_eq!(capacity(Rect::new(0.0, 0.0, 100.0, 100.0), 0.0, Layout::Grid, 4), 1);
        assert!(capacity(S, 0.0, Layout::Grid, 20) > capacity(S, 0.0, Layout::Columns, 20));
    }

    #[test]
    fn no_windows_no_frames() {
        assert!(tile(S, 8.0, Layout::Grid, &[]).is_empty());
    }

    #[test]
    fn output_keeps_the_input_order_and_ids() {
        let p = tile(S, 8.0, Layout::Spiral, &[w(9), w(4), w(7)]);
        assert_eq!(p.iter().map(|p| p.id).collect::<Vec<_>>(), vec![9, 4, 7]);
    }

    #[test]
    fn displays_are_laid_out_independently() {
        let left = Rect::new(0.0, 25.0, 1600.0, 900.0);
        let right = Rect::new(1600.0, 0.0, 2560.0, 1440.0);
        let a = tile(left, 8.0, Layout::Columns, &ws(2));
        let b = tile(right, 8.0, Layout::Grid, &[w(10), w(11), w(12), w(13)]);
        // the second display's windows stay inside it and do not change the first
        assert_eq!(a, tile(left, 8.0, Layout::Columns, &ws(2)));
        for p in &b {
            assert!(p.frame.x >= right.x && p.frame.x + p.frame.w <= right.x + right.w);
        }
        assert_eq!(b.len(), 4);
        assert_ne!(a[0].frame.w, b[0].frame.w);
    }

    #[test]
    fn focus_mode_saves_every_frame_and_restores_them_exactly() {
        let cur =
            vec![(1, Rect::new(8.5, 33.25, 700.0, 400.5)), (2, Rect::new(10.0, 20.0, 30.0, 40.0)), (3, Rect::new(-5.0, 0.0, 1.0, 1.0))];
        let (saved, full) = focus_enter(&cur, 2, S, 8.0).unwrap();
        assert_eq!(full, Rect::new(8.0, 33.0, 1584.0, 884.0));
        assert_eq!(saved.focused, 2);
        assert_eq!(focus_restore(&saved), cur);
    }

    #[test]
    fn focus_mode_needs_a_known_window() {
        assert_eq!(focus_enter(&[(1, S)], 5, S, 0.0), None);
    }

    #[test]
    fn neighbours_follow_the_direction() {
        let p = tile(S, 0.0, Layout::MainStack, &ws(3));
        let f: Vec<(u64, Rect)> = p.iter().map(|p| (p.id, p.frame)).collect();
        assert_eq!(neighbour(&f, 1, Dir::Right), Some(2));
        assert_eq!(neighbour(&f, 2, Dir::Down), Some(3));
        assert_eq!(neighbour(&f, 3, Dir::Up), Some(2));
        assert_eq!(neighbour(&f, 3, Dir::Left), Some(1));
        assert_eq!(neighbour(&f, 1, Dir::Left), None);
        assert_eq!(neighbour(&f, 2, Dir::Up), None);
        assert_eq!(neighbour(&f, 99, Dir::Up), None);
    }

    #[test]
    fn past_the_edge_the_next_display_is_found() {
        let screens = [Rect::new(0.0, 0.0, 1600.0, 900.0), Rect::new(1600.0, 0.0, 2560.0, 1440.0)];
        assert_eq!(display_in_direction(&screens, 0, Dir::Right), Some(1));
        assert_eq!(display_in_direction(&screens, 1, Dir::Left), Some(0));
        assert_eq!(display_in_direction(&screens, 0, Dir::Left), None);
        assert_eq!(display_in_direction(&screens, 0, Dir::Up), None);
    }

    #[test]
    fn minimum_sizes_are_learned_when_an_app_refuses_a_frame() {
        let mut m = MinSizes::default();
        assert!(!m.learn("com.a", (500.0, 400.0), (500.0, 400.0)), "accepted frame teaches nothing");
        assert!(!m.learn("com.a", (500.0, 400.0), (400.0, 300.0)), "smaller result is not a minimum");
        assert!(m.learn("com.a", (300.0, 400.0), (480.0, 400.0)));
        assert_eq!(m.get("com.a"), Some((480.0, 0.0)));
        assert!(m.learn("com.a", (300.0, 100.0), (300.0, 260.0)));
        assert_eq!(m.get("com.a"), Some((480.0, 260.0)));
        assert!(!m.learn("com.a", (300.0, 100.0), (450.0, 100.0)), "never shrinks");
        assert_eq!(m.get("com.b"), None);
    }

    #[test]
    fn minimum_sizes_round_trip_through_json_without_titles() {
        let mut m = MinSizes::default();
        m.learn("com.a", (1.0, 1.0), (300.0, 200.0));
        let s = serde_json::to_string(&m).unwrap();
        assert_eq!(serde_json::from_str::<MinSizes>(&s).unwrap(), m);
        assert!(s.contains("com.a"));
    }

    #[test]
    fn action_ids_are_unique_and_parse_back() {
        let mut ids: Vec<String> = Action::ALL.iter().map(|a| a.id()).collect();
        for a in Action::ALL {
            assert_eq!(Action::from_id(&a.id()), Some(a));
        }
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), Action::ALL.len());
        assert_eq!(Action::from_id("tiling:nope"), None);
    }

    #[test]
    fn default_hotkeys_cover_layouts_focus_and_arrows() {
        let get = |id: &str| DEFAULT_HOTKEYS.iter().find(|(i, _)| *i == id).map(|(_, h)| *h);
        assert_eq!(get("tiling:layout-3"), Some("Control+Alt+Super+Digit3"));
        assert_eq!(get("tiling:focus-mode"), Some("Control+Alt+KeyZ"));
        assert_eq!(get("tiling:move-left"), Some("Control+Alt+ArrowLeft"));
        for (id, _) in DEFAULT_HOTKEYS {
            assert!(Action::from_id(id).is_some(), "{id}");
        }
    }

    #[test]
    fn gap_is_clamped_and_screens_validated() {
        assert_eq!(clamp_gap(5000.0), 64.0);
        assert_eq!(clamp_gap(-9.0), 0.0);
        assert_eq!(clamp_gap(f64::NAN), 0.0);
        assert_eq!(clamp_gap(8.0), 8.0);
        assert!(!valid_screen(Rect::new(0.0, 0.0, 0.0, 900.0)));
        assert!(!valid_screen(Rect::new(f64::NAN, 0.0, 100.0, 100.0)));
        assert!(!valid_screen(Rect::new(0.0, 0.0, f64::INFINITY, 100.0)));
        assert!(valid_screen(S));
    }

    #[test]
    fn an_absurd_gap_or_a_bad_screen_never_puts_frames_off_screen() {
        for gap in [5000.0, -50.0, f64::NAN] {
            let p = tile(S, gap, Layout::Columns, &ws(3));
            assert_eq!(p.len(), 3, "gap {gap}");
            assert_disjoint_inside(&p, S.inset(clamp_gap(gap)));
        }
        assert!(tile(Rect::new(0.0, 0.0, 0.0, 0.0), 8.0, Layout::Grid, &ws(2)).is_empty());
        assert!(tile(Rect::new(f64::NAN, 0.0, 10.0, 10.0), 8.0, Layout::Grid, &ws(2)).is_empty());
    }

    #[test]
    fn displays_going_away_keep_every_window() {
        let order = vec![vec![1, 2], vec![3, 4], vec![5]];
        let r = reconcile_displays(order.clone(), 1);
        assert_eq!(r, vec![vec![1, 2, 3, 4, 5]]);
        let r2 = reconcile_displays(order.clone(), 2);
        assert_eq!(r2, vec![vec![1, 2, 5], vec![3, 4]]);
        let grown = reconcile_displays(vec![vec![1]], 3);
        assert_eq!(grown, vec![vec![1], vec![], vec![]]);
        let mut all: Vec<u64> = reconcile_displays(order, 1).concat();
        all.sort();
        assert_eq!(all, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn only_a_refusal_of_our_own_frame_teaches_a_minimum() {
        let applied = Rect::new(100.0, 100.0, 400.0, 300.0);
        // the app kept the position and came out larger: a refusal
        assert_eq!(should_learn(false, true, Some(applied), Rect::new(100.0, 100.0, 520.0, 300.0)), Some((520.0, 300.0)));
        // focus mode: the window is full screen, never a minimum
        assert_eq!(should_learn(true, true, Some(applied), Rect::new(8.0, 33.0, 1584.0, 884.0)), None);
        // not the first read after our own apply (the user or focus mode resized it since)
        assert_eq!(should_learn(false, false, Some(applied), Rect::new(8.0, 33.0, 1584.0, 884.0)), None);
        // nothing applied yet
        assert_eq!(should_learn(false, true, None, Rect::new(8.0, 33.0, 1584.0, 884.0)), None);
        // smaller or equal is not a refusal
        assert_eq!(should_learn(false, true, Some(applied), Rect::new(100.0, 100.0, 400.0, 300.0)), None);
        assert_eq!(should_learn(false, true, Some(applied), Rect::new(100.0, 100.0, 300.0, 200.0)), None);
        // moved: not a refusal of the size
        assert_eq!(should_learn(false, true, Some(applied), Rect::new(600.0, 100.0, 520.0, 300.0)), None);
    }

    #[test]
    fn learned_minimums_are_capped_by_the_display_and_decay_when_an_app_accepts_less() {
        let mut m = MinSizes::default();
        assert!(m.learn_capped("a", (300.0, 300.0), (5000.0, 400.0), (1600.0, 900.0)));
        assert_eq!(m.get("a"), Some((1600.0, 400.0)));
        // the app later accepted 500 x 400: the stored width was wrong
        assert!(m.confirm("a", (500.0, 400.0), (500.0, 400.0)));
        assert_eq!(m.get("a"), Some((500.0, 400.0)));
        assert!(!m.confirm("a", (500.0, 400.0), (500.0, 400.0)), "nothing more to lower");
        // a refused frame is not an acceptance
        assert!(!m.confirm("a", (300.0, 300.0), (500.0, 400.0)));
        assert_eq!(m.get("a"), Some((500.0, 400.0)));
    }

    #[test]
    fn a_refused_position_is_not_a_user_move_but_a_real_drag_is() {
        let target = Rect::new(0.0, 25.0, 800.0, 900.0);
        // the app keeps the same off-target frame tick after tick: refusal
        let off = Rect::new(40.0, 25.0, 800.0, 900.0);
        assert!(!user_moved(Some(off), off, target, false));
        // we applied a frame since the last read and the app only half obeyed
        assert!(!user_moved(Some(Rect::new(300.0, 300.0, 500.0, 500.0)), off, target, true));
        // the frame changed since the last read and is not on target: the user
        assert!(user_moved(Some(target), Rect::new(300.0, 300.0, 800.0, 900.0), target, false));
        // on target is never a move
        assert!(!user_moved(Some(off), target, target, false));
        // no previous read: cannot tell
        assert!(!user_moved(None, off, target, false));
    }

    #[test]
    fn hover_raises_after_the_dwell_and_again_after_leaving() {
        let mut h = Hover::default();
        assert_eq!(h.step(Some(1), 0.0, 0.25), None);
        assert_eq!(h.step(Some(1), 0.1, 0.25), None);
        assert_eq!(h.step(Some(1), 0.3, 0.25), Some(1));
        assert_eq!(h.step(Some(1), 0.9, 0.25), None, "already raised");
        assert_eq!(h.step(None, 1.0, 0.25), None);
        assert_eq!(h.step(Some(1), 1.1, 0.25), None);
        assert_eq!(h.step(Some(1), 1.5, 0.25), Some(1), "raised again after the pointer left");
        // moving to another window restarts the dwell
        assert_eq!(h.step(Some(2), 1.6, 0.25), None);
        assert_eq!(h.step(Some(2), 1.9, 0.25), Some(2));
    }

    #[test]
    fn hover_uses_the_window_on_top_not_the_first_that_contains_the_pointer() {
        let a = Rect::new(0.0, 0.0, 500.0, 500.0);
        let b = Rect::new(100.0, 100.0, 200.0, 200.0);
        // front to back: b is on top of a
        let entries = [(2, b, 0), (1, a, 0)];
        assert_eq!(top_window_at(&entries, (150.0, 150.0)), Some(0));
        assert_eq!(top_window_at(&entries, (10.0, 10.0)), Some(1));
        assert_eq!(top_window_at(&entries, (900.0, 10.0)), None);
        // a menu or panel (layer > 0) on top means nothing to raise
        let with_menu = [(9, b, 25), (1, a, 0)];
        assert_eq!(top_window_at(&with_menu, (150.0, 150.0)), None);
    }

    #[test]
    fn a_stacked_window_whose_minimum_does_not_fit_the_cascade_floats() {
        // 7 columns on a 1600 screen: the last slot is 320 wide and holds a 3-window cascade
        let mut v = ws(7);
        v[6].min = Some((320.0, 200.0));
        let plain = tile(S, 0.0, Layout::Columns, &v);
        // a minimum equal to the slot still fits the cascade frame? the cascade shrinks the frame
        let cascaded = plain.iter().find(|p| p.id == 7).unwrap();
        assert!(cascaded.frame.w >= 1.0);
        let mut v = ws(7);
        v[5].min = Some((315.0, 300.0));
        let p = tile(S, 0.0, Layout::Columns, &v);
        let f = p.iter().find(|p| p.id == 6).unwrap();
        assert_eq!(f.kind, Kind::Floating, "{f:?}");
        let area = S;
        let c = area.center();
        assert!((f.frame.center().0 - c.0).abs() < 1e-6 && (f.frame.center().1 - c.1).abs() < 1e-6);
        // the others still stack, and nothing stacked is smaller than its minimum
        for q in p.iter().filter(|q| q.kind == Kind::Stacked) {
            let w = v.iter().find(|w| w.id == q.id).unwrap();
            if let Some(m) = w.min {
                assert!(q.frame.w + 1e-9 >= m.0 && q.frame.h + 1e-9 >= m.1, "{q:?}");
            }
        }
    }

    #[test]
    fn a_window_is_never_listed_twice_after_a_display_goes_away() {
        let mut order = reconcile_displays(vec![vec![1], vec![2], vec![3]], 2);
        assert_eq!(order, vec![vec![1, 3], vec![2]]);
        assert_eq!(display_index(&order, 3), Some(0), "a window of the lost display is on display 0");
        move_to_display(&mut order, 3, 1);
        assert_eq!(order, vec![vec![1], vec![2, 3]]);
        assert_eq!(order.concat().iter().filter(|i| **i == 3).count(), 1);
        move_to_display(&mut order, 3, 1);
        assert_eq!(order[1], vec![2, 3], "moving within a display does not duplicate");
        assert_eq!(display_index(&order, 9), None);
        // a list that already holds an id twice is cleaned by reconcile
        let dup = reconcile_displays(vec![vec![1, 2], vec![2, 3]], 2);
        assert_eq!(dup, vec![vec![1, 2], vec![3]]);
    }

    #[test]
    fn a_refusal_must_repeat_before_it_is_learned() {
        let mut p = None;
        assert_eq!(confirm_refusal(&mut p, Some((520.0, 300.0))), None, "first sight only waits");
        assert_eq!(confirm_refusal(&mut p, Some((521.0, 300.0))), Some((521.0, 300.0)), "same again: learn");
        assert_eq!(p, None);
        // an async resize that settles on the asked size never teaches anything
        assert_eq!(confirm_refusal(&mut p, Some((700.0, 500.0))), None);
        assert_eq!(confirm_refusal(&mut p, None), None);
        assert_eq!(confirm_refusal(&mut p, Some((700.0, 500.0))), None, "the wait started over");
        // a different size restarts the wait
        assert_eq!(confirm_refusal(&mut p, Some((900.0, 500.0))), None);
    }

    #[test]
    fn several_floating_windows_do_not_sit_exactly_on_top_of_each_other() {
        let mut v = ws(4);
        for i in [0, 1, 2] {
            v[i].min = Some((1500.0, 800.0));
        }
        let p = tile(S, 0.0, Layout::Columns, &v);
        let floats: Vec<&Placed> = p.iter().filter(|p| p.kind == Kind::Floating).collect();
        assert!(floats.len() >= 2, "{p:?}");
        assert_eq!(floats[0].frame, Rect::new(50.0, 75.0, 1500.0, 800.0), "the first stays centred");
        for (i, a) in floats.iter().enumerate() {
            assert!(a.frame.x + a.frame.w <= S.x + S.w + 1e-9 && a.frame.y + a.frame.h <= S.y + S.h + 1e-9, "{a:?} inside");
            for b in &floats[i + 1..] {
                assert!(a.frame.x != b.frame.x || a.frame.y != b.frame.y, "{a:?} and {b:?} coincide");
            }
        }
    }

    #[test]
    fn the_border_sits_just_outside_the_window_and_follows_its_radius() {
        let w = Rect::new(100.0, 100.0, 600.0, 400.0);
        let g = border_geometry(w, S, BORDER_WIDTH).unwrap();
        assert_eq!(g.frame, Rect::new(97.0, 97.0, 606.0, 406.0), "entirely outside the window");
        assert_eq!(g.width, 3.0);
        assert_eq!(g.radius, WINDOW_RADIUS + 3.0);
        // off this screen, or nothing to outline
        assert_eq!(border_geometry(Rect::new(5000.0, 0.0, 100.0, 100.0), S, 3.0), None);
        assert_eq!(border_geometry(Rect::new(0.0, 0.0, 0.0, 100.0), S, 3.0), None);
        assert_eq!(border_geometry(Rect::new(f64::NAN, 0.0, 10.0, 10.0), S, 3.0), None);
        // a window partly off the screen is still outlined
        assert!(border_geometry(Rect::new(1500.0, 800.0, 400.0, 300.0), S, 3.0).is_some());
    }

    #[test]
    fn the_border_shows_only_for_a_focused_tiled_window_while_tiling_runs() {
        let on = BorderInputs {
            tiling_on: true,
            border_on: true,
            paused: false,
            stage_manager: false,
            focus_settling: false,
            focused_tiled: true,
            dragging: false,
        };
        assert!(border_visible(&on));
        assert!(!border_visible(&BorderInputs { tiling_on: false, ..on }), "tiling off");
        assert!(!border_visible(&BorderInputs { border_on: false, ..on }), "setting off");
        assert!(!border_visible(&BorderInputs { paused: true, ..on }), "paused");
        assert!(!border_visible(&BorderInputs { stage_manager: true, ..on }), "Stage Manager");
        assert!(!border_visible(&BorderInputs { focus_settling: true, ..on }), "focus mode transition");
        assert!(!border_visible(&BorderInputs { focused_tiled: false, ..on }), "floating or untiled window");
    }

    #[test]
    fn the_bar_goes_above_the_window_or_inside_it_and_stays_on_screen() {
        let w = Rect::new(400.0, 300.0, 600.0, 400.0);
        let b = bar_frame(w, S);
        assert_eq!((b.w, b.h), (BAR_W, BAR_H));
        assert_eq!(b.x, 400.0 + 300.0 - BAR_W / 2.0);
        assert_eq!(b.y, 300.0 - BAR_H - 6.0, "above the top edge");
        // no room above: a pill below the title bar strip
        let top = Rect::new(400.0, 25.0, 600.0, 400.0);
        assert_eq!(bar_frame(top, S).y, 25.0 + TOP_STRIP + 6.0);
        // a narrow window at the screen edge keeps the bar on screen
        let edge = Rect::new(0.0, 300.0, 100.0, 300.0);
        let e = bar_frame(edge, S);
        assert!(e.x >= S.x && e.x + e.w <= S.x + S.w);
        let right = Rect::new(1550.0, 300.0, 50.0, 300.0);
        let r = bar_frame(right, S);
        assert!(r.x + r.w <= S.x + S.w + 1e-9);
    }

    #[test]
    fn the_zone_is_the_top_band_and_the_bar_but_not_the_rest_of_the_window() {
        let w = Rect::new(400.0, 300.0, 600.0, 400.0);
        assert!(in_bar_zone(w, S, (500.0, 310.0)), "top band");
        assert!(in_bar_zone(w, S, (700.0, 300.0 - 20.0)), "the bar itself, above the window");
        assert!(!in_bar_zone(w, S, (500.0, 300.0 + EDGE_BAND + 20.0)), "inside the window, below the band");
        assert!(!in_bar_zone(w, S, (100.0, 310.0)), "beside the window");
    }

    #[test]
    fn the_bar_appears_after_a_dwell_and_hides_after_a_grace_period() {
        let w = Rect::new(400.0, 300.0, 600.0, 400.0);
        let wins = [(1, w, S)];
        let mut q = QuickBar::default();
        let near = Some((500.0, 305.0));
        assert_eq!(q.step(true, near, false, &wins, 0.0), None);
        assert_eq!(q.step(true, near, false, &wins, 0.2), None);
        assert_eq!(q.step(true, near, false, &wins, 0.4), Some(1));
        // moving up onto the bar keeps it
        assert_eq!(q.step(true, Some((700.0, 285.0)), false, &wins, 0.6), Some(1));
        // leaving: still there during the grace period, gone after it
        assert_eq!(q.step(true, Some((100.0, 100.0)), false, &wins, 1.0), Some(1));
        assert_eq!(q.step(true, Some((100.0, 100.0)), false, &wins, 1.6), None);
        // coming back needs a new dwell
        assert_eq!(q.step(true, near, false, &wins, 2.0), None);
        // the setting off hides it at once
        assert_eq!(q.step(true, near, false, &wins, 2.5), Some(1));
        assert_eq!(q.step(false, near, false, &wins, 2.6), None);
        // no pointer: hides after the grace period
        let mut q = QuickBar::default();
        q.step(true, near, false, &wins, 0.0);
        assert_eq!(q.step(true, near, false, &wins, 0.5), Some(1));
        assert_eq!(q.step(true, None, false, &wins, 0.6), Some(1));
        assert_eq!(q.step(true, None, false, &wins, 1.2), None);
    }

    #[test]
    fn the_bar_switches_windows_only_after_a_dwell_on_the_new_one() {
        let a = Rect::new(0.0, 300.0, 700.0, 400.0);
        let b = Rect::new(800.0, 300.0, 700.0, 400.0);
        let wins = [(1, a, S), (2, b, S)];
        let mut q = QuickBar::default();
        q.step(true, Some((100.0, 305.0)), false, &wins, 0.0);
        assert_eq!(q.step(true, Some((100.0, 305.0)), false, &wins, 0.5), Some(1));
        assert_eq!(q.step(true, Some((900.0, 305.0)), false, &wins, 0.6), Some(1));
        assert_eq!(q.step(true, Some((900.0, 305.0)), false, &wins, 1.0), Some(2));
    }

    #[test]
    fn every_quick_action_sends_the_command_the_launcher_and_hotkeys_use() {
        let ids: Vec<String> = QuickAction::ALL.iter().map(|q| q.action().id()).collect();
        assert_eq!(ids, vec!["tiling:focus-mode", "tiling:float-toggle", "tiling:next-display", "tiling:swap-main", "tiling:exclude-app"]);
        for q in QuickAction::ALL {
            assert!(Action::ALL.contains(&q.action()), "{q:?} is a launcher command");
        }
    }

    #[test]
    fn swap_with_main_and_next_display_are_pure_list_moves() {
        let mut l = vec![1, 2, 3];
        assert!(swap_with_main(&mut l, 3));
        assert_eq!(l, vec![3, 2, 1]);
        assert!(swap_with_main(&mut l, 3), "the main window swaps with the next slot");
        assert_eq!(l, vec![2, 3, 1]);
        assert!(!swap_with_main(&mut l, 9));
        let mut single = vec![5];
        assert!(!swap_with_main(&mut single, 5));
        assert_eq!(next_display(3, 0), Some(1));
        assert_eq!(next_display(3, 2), Some(0));
        assert_eq!(next_display(1, 0), None);
        assert_eq!(next_display(0, 0), None);
    }

    #[test]
    fn the_bar_never_covers_the_title_bar_or_tab_strip() {
        // default gap 8 at the top of the screen: no room above, so a compact pill below the strip
        let w = Rect::new(8.0, 33.0, 800.0, 600.0);
        let b = bar_frame(w, S);
        assert!(b.y >= w.y + TOP_STRIP, "{b:?} is below the top strip");
        assert!(b.x + b.w <= w.x + w.w, "inside the window's right edge");
        assert!(b.x > w.x + w.w / 2.0, "at the top-right corner, not centred over the tabs");
        // with room above it still goes above, centred
        let low = Rect::new(400.0, 300.0, 600.0, 400.0);
        assert_eq!(bar_frame(low, S).y, 300.0 - BAR_H - 6.0);
        // every window height keeps it inside the window when below
        let short = Rect::new(8.0, 33.0, 400.0, 120.0);
        let sb = bar_frame(short, S);
        assert!(sb.y >= short.y + TOP_STRIP);
    }

    #[test]
    fn pressing_the_mouse_never_starts_the_bar_and_hides_it_unless_the_press_is_on_the_bar() {
        let w = Rect::new(400.0, 300.0, 600.0, 400.0);
        let wins = [(1, w, S)];
        let near = Some((500.0, 305.0));
        let mut q = QuickBar::default();
        // a press on the title bar (a tab click, a drag) never starts the dwell
        assert_eq!(q.step(true, near, true, &wins, 0.0), None);
        assert_eq!(q.step(true, near, true, &wins, 0.6), None);
        assert_eq!(q.step(true, near, false, &wins, 0.7), None, "the dwell starts only after the release");
        assert_eq!(q.step(true, near, false, &wins, 1.1), Some(1));
        // a press on the band hides it at once
        assert_eq!(q.step(true, near, true, &wins, 1.2), None);
        // a press on the bar itself (a button click) keeps it
        let mut q = QuickBar::default();
        q.step(true, near, false, &wins, 0.0);
        assert_eq!(q.step(true, near, false, &wins, 0.5), Some(1));
        let on_bar = Some((bar_frame(w, S).x + 10.0, bar_frame(w, S).y + 10.0));
        assert_eq!(q.step(true, on_bar, true, &wins, 0.6), Some(1));
    }

    #[test]
    fn the_border_hides_while_the_mouse_is_down() {
        let on = BorderInputs {
            tiling_on: true,
            border_on: true,
            paused: false,
            stage_manager: false,
            focus_settling: false,
            focused_tiled: true,
            dragging: false,
        };
        assert!(border_visible(&on));
        assert!(!border_visible(&BorderInputs { dragging: true, ..on }));
    }

    #[test]
    fn moving_to_a_display_that_does_not_exist_loses_no_window() {
        let mut order = vec![vec![1, 2], vec![3]];
        move_to_display(&mut order, 2, 7);
        let mut all = order.concat();
        all.sort();
        assert_eq!(all, vec![1, 2, 3], "{order:?}");
        assert_eq!(order, vec![vec![1], vec![3, 2]], "clamped to the last display");
        let mut none: Vec<Vec<u64>> = vec![];
        move_to_display(&mut none, 1, 0);
        assert!(none.is_empty());
    }
}
