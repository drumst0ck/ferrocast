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
            let group = &live[k - 1..];
            let slot = slots[k - 1];
            let step = CASCADE_STEP.min(slot.w.min(slot.h) / (2.0 * group.len() as f64));
            let total = step * (group.len() - 1) as f64;
            for (j, i) in group.iter().enumerate() {
                let off = step * j as f64;
                let frame = Rect::new(slot.x + off, slot.y + off, (slot.w - total).max(1.0), (slot.h - total).max(1.0));
                result[*i] = Some(Placed { id: wins[*i].id, frame, kind: Kind::Stacked });
            }
        }
        break;
    }
    result.into_iter().flatten().collect()
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
    pub fn learn(&mut self, app: &str, asked: (f64, f64), got: (f64, f64)) -> bool {
        const SLACK: f64 = 2.0;
        let (gw, gh) = (got.0 > asked.0 + SLACK, got.1 > asked.1 + SLACK);
        if !gw && !gh {
            return false;
        }
        let old = self.get(app).unwrap_or((0.0, 0.0));
        let new = (if gw { old.0.max(got.0) } else { old.0 }, if gh { old.1.max(got.1) } else { old.1 });
        let changed = new != old;
        if changed {
            self.0.insert(app.to_string(), new);
        }
        changed
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
}

impl Action {
    pub const ALL: [Action; 11] = [
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
    ];
    pub fn id(self) -> String {
        match self {
            Action::Toggle => "tiling:toggle".into(),
            Action::Pause => "tiling:pause".into(),
            Action::Layout(n) => format!("tiling:layout-{n}"),
            Action::FocusMode => "tiling:focus-mode".into(),
            Action::Move(d) => format!("tiling:move-{}", d.name().to_lowercase()),
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
        }
    }
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
}
