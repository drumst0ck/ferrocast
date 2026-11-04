//! Geometry and motion of the notch player ("Vinyl / ambient", design/notch-player/03-vinyl-ambient.html):
//! pure numbers, no AppKit. The AppKit layer draws what these functions say, so the layout and the
//! motion are tested without a screen.
//!
//! Coordinates inside the player are top-left points (x right, y down) relative to the card. The
//! content lives in a frame 430 pt wide that is centred on the card, so while the pill is narrower
//! the content is clipped to it, as in the artboard.

/// Width of each side of the collapsed pill next to the notch (cover left, equalizer right).
pub const WING: f64 = 40.0;
/// Expanded card: 430 x 216.
pub const CARD_W: f64 = 430.0;
pub const CARD_H: f64 = 216.0;
const PLAIN_W: f64 = 190.0;
const PLAIN_H: f64 = 28.0;
/// Bottom corner radius of the pill and of the card.
pub const RC: f64 = 16.0;
pub const RE: f64 = 36.0;
/// Room below the card for the tinted light that bleeds out under it.
pub const BLEED_PAD: f64 = 44.0;
/// Width of the title column and height of one title line (28 pt, line height 1.03).
pub const TITLE_W: f64 = 214.0;
const TITLE_LINE: f64 = 28.84;
const ARTIST_H: f64 = 19.0;

/// The camera housing of a MacBook display.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Notch {
    pub width: f64,
    pub height: f64,
}

/// The notch of a display from `NSScreen`: width of the screen, `safeAreaInsets.top` and the widths
/// of `auxiliaryTopLeftArea` / `auxiliaryTopRightArea` (the menu bar on each side of it).
pub fn notch_from_screen(screen_w: f64, safe_top: f64, left_w: f64, right_w: f64) -> Option<Notch> {
    if safe_top <= 0.0 || left_w <= 0.0 || right_w <= 0.0 {
        return None;
    }
    let width = screen_w - left_w - right_w;
    (width > 0.0 && width <= screen_w * 0.5).then_some(Notch { width, height: safe_top })
}

/// Size of the resting pill: the notch plus a wing on each side, or a plain capsule.
pub fn collapsed_size(n: Option<Notch>) -> (f64, f64) {
    match n {
        Some(n) => (n.width + 2.0 * WING, n.height),
        None => (PLAIN_W, PLAIN_H),
    }
}

pub fn expanded_size() -> (f64, f64) {
    (CARD_W, CARD_H)
}

/// Cocoa frame (x, y of the bottom-left, w, h) of a panel hanging from the top centre of `screen`
/// (a Cocoa display frame): the card of `size` plus [`BLEED_PAD`] below it.
pub fn panel_frame(screen: [f64; 4], size: (f64, f64)) -> [f64; 4] {
    [screen[0] + (screen[2] - size.0) / 2.0, screen[1] + screen[3] - size.1 - BLEED_PAD, size.0, size.1 + BLEED_PAD]
}

pub fn lerp_rect(a: [f64; 4], b: [f64; 4], p: f64) -> [f64; 4] {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * p)
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Size and bottom radius of the card at width progress `pw` and height progress `ph` (the springs
/// may be a little past 1 or below 0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape {
    pub w: f64,
    pub h: f64,
    pub radius: f64,
}

pub fn shape(pw: f64, ph: f64, coll: (f64, f64)) -> Shape {
    let w = lerp(coll.0, CARD_W, pw).max(60.0);
    let h = lerp(coll.1, CARD_H, ph).max(coll.1 * 0.92);
    let e = pw.max(ph).clamp(0.0, 1.0);
    Shape { w, h, radius: lerp(RC, RE, e).min(h / 2.0) }
}

/// The cover disc, the progress ring and the equalizer at expansion `ex` (the width spring, not
/// clamped: they overshoot with the card), in the 430-wide content frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parts {
    pub disc: [f64; 4],
    pub ring: [f64; 4],
    pub eq: [f64; 4],
}

pub fn parts_at(ex: f64, coll: (f64, f64)) -> Parts {
    let (cw, ch) = coll;
    let o = (CARD_W - cw) / 2.0;
    let c = Parts {
        disc: [9.0 + o, (ch - 22.0) / 2.0, 22.0, 22.0],
        ring: [8.0 + o, (ch - 24.0) / 2.0, 24.0, 24.0],
        eq: [o + cw - 9.0 - 22.0, (ch - 14.0) / 2.0, 22.0, 14.0],
    };
    let e = expanded_layout(1);
    Parts { disc: lerp_rect(c.disc, e.disc, ex), ring: lerp_rect(c.ring, e.ring, ex), eq: lerp_rect(c.eq, e.eq, ex) }
}

/// Radius of the disc described by `p`.
pub fn disc_radius(p: &Parts) -> f64 {
    p.disc[2].max(0.0) / 2.0
}

/// Where everything sits in the expanded card.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Expanded {
    pub disc: [f64; 4],
    pub ring: [f64; 4],
    pub eq: [f64; 4],
    /// Icon and name of the source app, top left (the notch strip).
    pub chip_icon: [f64; 4],
    pub chip: [f64; 4],
    pub title: [f64; 4],
    pub artist: [f64; 4],
    pub prev: [f64; 4],
    pub play: [f64; 4],
    pub next: [f64; 4],
    /// The artboard's time row: elapsed, the seek bar (added), remaining.
    pub elapsed: [f64; 4],
    pub bar: [f64; 4],
    pub remaining: [f64; 4],
}

/// The artboard's layout for a title of `title_lines` lines (one or two).
pub fn expanded_layout(title_lines: usize) -> Expanded {
    let lines = title_lines.clamp(1, 2) as f64;
    let th = lines * TITLE_LINE;
    Expanded {
        disc: [28.0, 56.0, 128.0, 128.0],
        ring: [16.0, 44.0, 152.0, 152.0],
        eq: [382.0, 9.0, 30.0, 14.0],
        chip_icon: [18.0, 9.0, 14.0, 14.0],
        chip: [38.0, 7.0, 64.0, 18.0],
        title: [192.0, 48.0, TITLE_W, th],
        artist: [192.0, 48.0 + th + 7.0, TITLE_W, ARTIST_H],
        prev: [184.0, 146.0, 40.0, 40.0],
        play: [228.0, 140.0, 52.0, 52.0],
        next: [284.0, 146.0, 40.0, 40.0],
        elapsed: [192.0, 196.0, 46.0, 11.0],
        bar: [244.0, 199.0, 110.0, 4.0],
        remaining: [360.0, 196.0, 46.0, 11.0],
    }
}

/// `r` scaled by `s` about `origin`, then moved `dy` down: how a content item arrives.
pub fn xf_rect(r: [f64; 4], origin: [f64; 2], s: f64, dy: f64) -> [f64; 4] {
    [origin[0] + (r[0] - origin[0]) * s, origin[1] + (r[1] - origin[1]) * s + dy, r[2] * s, r[3] * s]
}

/// Greedy word wrap of `text` into at most `max_lines` lines no wider than `max_w` (as `measure`
/// says); a word too long for a line is broken, and what does not fit ends the last line with `…`.
pub fn wrap_lines(text: &str, max_w: f64, max_lines: usize, measure: &dyn Fn(&str) -> f64) -> Vec<String> {
    let mut words: Vec<String> = vec![];
    for w in text.split_whitespace() {
        let mut cur = String::new();
        for ch in w.chars() {
            cur.push(ch);
            if measure(&cur) > max_w && cur.chars().count() > 1 {
                cur.pop();
                words.push(std::mem::take(&mut cur));
                cur.push(ch);
            }
        }
        words.push(cur);
    }
    let mut lines: Vec<String> = vec![];
    let mut cur = String::new();
    let mut i = 0;
    while i < words.len() && max_lines > 0 {
        if lines.len() + 1 == max_lines {
            // the last line takes everything that is left
            let rest = words[i..].join(" ");
            let full = if cur.is_empty() { rest } else { format!("{cur} {rest}") };
            lines.push(truncate_to_width(&full, max_w, measure));
            return lines;
        }
        let cand = if cur.is_empty() { words[i].clone() } else { format!("{cur} {}", words[i]) };
        if measure(&cand) <= max_w {
            cur = cand;
            i += 1;
        } else {
            lines.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// `text` cut to fit `max_w` with a trailing `…` when it does not.
pub fn truncate_to_width(text: &str, max_w: f64, measure: &dyn Fn(&str) -> f64) -> String {
    if measure(text) <= max_w {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    for n in (0..chars.len()).rev() {
        let cand: String = chars[..n].iter().collect::<String>().trim_end().to_string() + "…";
        if measure(&cand) <= max_w {
            return cand;
        }
    }
    "…".to_string()
}

/// Opacity (0 to .17) of the disc's fixed gloss at `deg` degrees: the artboard's conic gradient
/// (from 30 deg: clear to 30 %, .17 white at 38 %, clear 46 % to 80 %, .1 at 88 %, clear from 95 %).
pub fn gloss_alpha(deg: f64) -> f64 {
    let t = ((deg - 30.0) / 360.0).rem_euclid(1.0);
    let tri = |t: f64, a: f64, peak: f64, b: f64, h: f64| {
        if t <= a || t >= b {
            0.0
        } else if t <= peak {
            h * (t - a) / (peak - a)
        } else {
            h * (b - t) / (b - peak)
        }
    };
    tri(t, 0.30, 0.38, 0.46, 0.17) + tri(t, 0.80, 0.88, 0.95, 0.10)
}

// ---------------------------------------------------------------- motion

/// A spring with unit mass: `x` follows `t` with stiffness `k` and damping `c`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sp {
    pub x: f64,
    pub v: f64,
    pub t: f64,
}

impl Sp {
    pub fn new(x: f64) -> Self {
        Sp { x, v: 0.0, t: x }
    }

    pub fn at_rest(&self) -> bool {
        (self.x - self.t).abs() < 0.0015 && self.v.abs() < 0.02
    }

    /// Advance by `dt` seconds, sub-stepped at 240 Hz so the result does not depend on the frame
    /// rate. At rest it snaps exactly onto the target; with `rm` (Reduce Motion) it jumps there.
    pub fn step(&mut self, dt: f64, k: f64, c: f64, rm: bool) {
        if rm {
            *self = Sp::new(self.t);
            return;
        }
        let dt = dt.clamp(0.0, 0.05);
        let n = (dt * 240.0).ceil().max(1.0);
        let h = dt / n;
        for _ in 0..n as usize {
            self.v += (k * (self.t - self.x) - c * self.v) * h;
            self.x += self.v * h;
        }
        if self.at_rest() {
            *self = Sp::new(self.t);
        }
    }
}

/// The artboard's motion values (its panel defaults).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionCfg {
    pub k: f64,
    pub c: f64,
    /// Velocity kick when opening (times 4 on the width, times 3 on the height).
    pub ov: f64,
    /// Largest squash/stretch.
    pub sq: f64,
    /// Milliseconds between content items.
    pub stag_ms: f64,
    /// Milliseconds the height lags the width.
    pub lead_ms: f64,
    /// Milliseconds the card stays open after the pointer leaves.
    pub hold_ms: f64,
    pub pulse: f64,
}

impl Default for MotionCfg {
    fn default() -> Self {
        MotionCfg { k: 150.0, c: 14.0, ov: 0.8, sq: 0.035, stag_ms: 70.0, lead_ms: 90.0, hold_ms: 400.0, pulse: 1.0 }
    }
}

/// Content items that arrive one after another: 0 source chip, 1 title and artist, 2 buttons,
/// 3 time row with the seek bar.
pub const ITEMS: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Act {
    PhOpen,
    ItemOpen(usize),
    ItemClose(usize),
    PwClose,
    Collapse,
}

/// All the springs of the player. Time only moves through [`Motion::step`].
#[derive(Clone, Debug)]
pub struct Motion {
    pub cfg: MotionCfg,
    /// Width and height progress of the card (0 pill, 1 card).
    pub pw: Sp,
    pub ph: Sp,
    pub items: [Sp; ITEMS],
    /// Play/pause morph (1 = pause bars).
    pub pm: Sp,
    /// Equalizer amplitude.
    pub am: Sp,
    /// New-track swell of the pill.
    pub pulse: Sp,
    /// Disc speed in degrees per second, and its angle.
    pub om: Sp,
    pub angle: f64,
    playing: bool,
    open: bool,
    queue: Vec<(f64, Act)>,
}

impl Default for Motion {
    fn default() -> Self {
        Self::new()
    }
}

impl Motion {
    /// Start already settled as playing or paused (a pill that appears on a paused track).
    pub fn with_playing(playing: bool) -> Self {
        let mut m = Motion::new();
        m.playing = playing;
        if !playing {
            m.pm = Sp::new(0.0);
            m.am = Sp::new(0.0);
            m.om = Sp::new(0.0);
        }
        m
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn new() -> Self {
        Motion {
            cfg: MotionCfg::default(),
            pw: Sp::new(0.0),
            ph: Sp::new(0.0),
            items: [Sp::new(0.0); ITEMS],
            pm: Sp::new(1.0),
            am: Sp::new(1.0),
            pulse: Sp::new(0.0),
            om: Sp::new(60.0),
            angle: 0.0,
            playing: true,
            open: false,
            queue: vec![],
        }
    }

    fn later(&mut self, ms: f64, act: Act, rm: bool) {
        if rm || ms <= 0.0 {
            self.run(act, rm);
        } else {
            self.queue.push((ms / 1000.0, act));
        }
    }

    fn run(&mut self, act: Act, rm: bool) {
        let ov = self.cfg.ov;
        match act {
            Act::PhOpen => {
                self.ph.t = 1.0;
                self.ph.v += ov * 3.0;
            }
            Act::ItemOpen(i) => {
                self.items[i].t = 1.0;
                self.items[i].v += 2.5;
            }
            Act::ItemClose(i) => self.items[i].t = 0.0,
            Act::PwClose => self.pw.t = 0.0,
            Act::Collapse => self.set_open(false, rm),
        }
    }

    /// Open or close the card now (the artboard's `set`).
    pub fn set_open(&mut self, open: bool, rm: bool) {
        if self.open == open {
            return;
        }
        self.open = open;
        self.queue.clear();
        let (lead, stag) = (self.cfg.lead_ms, self.cfg.stag_ms);
        if open {
            self.pw.t = 1.0;
            self.pw.v += self.cfg.ov * 4.0;
            self.later(lead, Act::PhOpen, rm);
            for i in 0..ITEMS {
                self.later(lead * 0.6 + i as f64 * stag, Act::ItemOpen(i), rm);
            }
        } else {
            for i in 0..ITEMS {
                self.later((ITEMS - i) as f64 * 10.0, Act::ItemClose(i), rm);
            }
            self.ph.t = 0.0;
            self.later(lead, Act::PwClose, rm);
        }
        if rm {
            // nothing waits: everything is already where it is going
            self.step(0.0, true);
        }
    }

    /// Follow the pointer: it opens at once and, after leaving, closes once the hold has passed (and
    /// not at all if it comes back).
    pub fn set_expanded(&mut self, want: bool, rm: bool) {
        if want {
            self.queue.retain(|(_, a)| *a != Act::Collapse);
            self.set_open(true, rm);
        } else if self.open && !self.queue.iter().any(|(_, a)| *a == Act::Collapse) {
            let hold = self.cfg.hold_ms / 1000.0;
            self.queue.push((hold, Act::Collapse));
        }
    }

    /// A new track: the pill swells, the title slides in again if the card is open, and with `flick`
    /// the disc is thrown to seven times its speed.
    pub fn track_changed(&mut self, flick: bool) {
        self.pulse.v += 6.0 * self.cfg.pulse;
        if self.open {
            self.items[1].x = 0.1;
            self.items[1].v = 0.0;
        }
        if flick {
            self.om.x = 420.0;
        }
    }

    pub fn set_playing(&mut self, playing: bool) {
        self.playing = playing;
        let t = if playing { 1.0 } else { 0.0 };
        self.pm.t = t;
        self.am.t = t;
    }

    /// Advance everything by `dt` seconds.
    pub fn step(&mut self, dt: f64, rm: bool) {
        let mut due = vec![];
        self.queue.retain_mut(|(left, act)| {
            *left -= dt;
            if *left <= 0.0 {
                due.push(*act);
                false
            } else {
                true
            }
        });
        for a in due {
            self.run(a, rm);
        }
        let (k, c) = (self.cfg.k, self.cfg.c);
        self.pw.step(dt, k, c, rm);
        self.ph.step(dt, k, c, rm);
        self.pm.step(dt, 320.0, 26.0, rm);
        self.pulse.t = 0.0;
        self.pulse.step(dt, k * 1.3, c * 0.8, rm);
        self.am.step(dt, 60.0, 14.0, rm);
        for it in &mut self.items {
            it.step(dt, k * 1.5, c * 1.2, rm);
        }
        self.om.t = if self.playing { 60.0 } else { 0.0 };
        // the speed springs on its own (soft: it coasts) and is never snapped by the rest rule
        let dt = dt.clamp(0.0, 0.05);
        let n = (dt * 240.0).ceil().max(1.0);
        let h = dt / n;
        for _ in 0..n as usize {
            self.om.v += (5.0 * (self.om.t - self.om.x) - 4.0 * self.om.v) * h;
            self.om.x += self.om.v * h;
        }
        if !rm {
            self.angle = (self.angle + self.om.x * dt).rem_euclid(360.0);
        }
    }

    /// True when every spring is on its target and nothing is queued (the disc does not count).
    pub fn at_rest(&self) -> bool {
        self.queue.is_empty()
            && self.pw.at_rest()
            && self.ph.at_rest()
            && self.pm.at_rest()
            && self.am.at_rest()
            && self.pulse.at_rest()
            && self.items.iter().all(Sp::at_rest)
    }

    /// True while the disc is turning (it needs frames even when nothing else moves).
    pub fn spinning(&self, rm: bool) -> bool {
        !rm && self.om.x.abs() > 0.5
    }

    /// Opacity, scale and downward offset (points) of content item `i`.
    pub fn item(&self, i: usize, rm: bool) -> (f64, f64, f64) {
        let x = self.items[i].x;
        let o = x.clamp(0.0, 1.0);
        if rm {
            (o, 1.0, 0.0)
        } else {
            (o, 0.9 + 0.1 * x, (1.0 - x) * 10.0)
        }
    }

    /// Width and height factors of the whole card: squash and stretch from the width's velocity,
    /// plus the new-track swell.
    pub fn squash(&self, rm: bool) -> (f64, f64) {
        if rm {
            return (1.0, 1.0);
        }
        let sq = self.cfg.sq;
        let s = (self.pw.v * sq * 0.4).clamp(-sq, sq);
        let pu = self.pulse.x * 0.025 * self.cfg.pulse;
        (1.0 + s + pu, 1.0 - s + pu * 0.6)
    }

    /// The card's shape now.
    pub fn shape(&self, coll: (f64, f64)) -> Shape {
        shape(self.pw.x, self.ph.x, coll)
    }
}

/// End angle in degrees of the progress ring (clockwise from the top) for `frac` of the track.
pub fn ring_end_degrees(frac: f64) -> f64 {
    if frac.is_finite() {
        frac.clamp(0.0, 1.0) * 360.0
    } else {
        0.0
    }
}

/// Track and arc stroke widths of a ring of `diameter` points (2.4 % and 3 % of it).
pub fn ring_stroke(diameter: f64) -> (f64, f64) {
    (diameter * 0.024, diameter * 0.03)
}

/// The play triangle (two triangles' worth of points, as the artboard) and the two pause bars, as 8
/// x/y pairs on a 24 x 24 grid: [`pp_points`] morphs between them.
pub const PP_PLAY: [f64; 16] = [7.0, 5.0, 12.0, 8.0, 12.0, 16.0, 7.0, 19.0, 12.0, 8.0, 18.0, 12.0, 18.0, 12.0, 12.0, 16.0];
pub const PP_PAUSE: [f64; 16] = [6.0, 5.0, 10.0, 5.0, 10.0, 19.0, 6.0, 19.0, 14.0, 5.0, 18.0, 5.0, 18.0, 19.0, 14.0, 19.0];

/// The glyph at morph `t` (0 play, 1 pause; the spring may be a little outside, clamped to -0.1..1.1).
pub fn pp_points(t: f64) -> [f64; 16] {
    let t = t.clamp(-0.1, 1.1);
    std::array::from_fn(|i| PP_PLAY[i] + (PP_PAUSE[i] - PP_PLAY[i]) * t)
}

/// Height (0.16..=1) of equalizer bar `i` at time `tm` seconds with amplitude `am` (0 flat, 1
/// dancing). Decorative: macOS does not let an app read another app's audio level. With Reduce
/// Motion the bars are still.
pub fn eq_scale(tm: f64, i: usize, am: f64, rm: bool) -> f64 {
    let a = am.clamp(0.0, 1.0);
    if rm {
        return 0.16 + 0.44 * a;
    }
    let i = i as f64;
    let w = (tm * (5.0 + i * 1.7) + i * 2.0).sin() * (tm * (2.3 + i * 0.8)).cos() * 0.5 + 0.5;
    lerp(0.16, 0.22 + 0.78 * w, a)
}

/// Seconds until the next redraw, or `None` when nothing on screen changes (the timer stops):
/// the display's own rate (60 to 120 Hz) while springs move, a calmer one while only the disc turns
/// or the equalizer dances, and a slow one for the progress bar alone.
pub fn tick_interval(animating: bool, spinning: bool, playing: bool, expanded: bool, reduce_motion: bool, fps: f64) -> Option<f64> {
    if animating {
        Some(1.0 / fps.clamp(60.0, 120.0))
    } else if spinning || (playing && !reduce_motion) {
        Some(1.0 / 30.0)
    } else if playing && expanded {
        Some(0.25)
    } else {
        None
    }
}

#[cfg(test)]
#[path = "nowplaying_view_tests.rs"]
mod tests;
