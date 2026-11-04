//! Pointer logic of the notch player, pure: the cover crop, which text opens the source app, and
//! seeking on the progress bar (hit area, x to position, the scrub gesture).

use crate::nowplaying_view::Expanded;

/// Source rectangle (x, y, w, h in image units) that fills a square with `w` by `h` pixels of image
/// without stretching it: the biggest centred square. All zeros when the size is unusable (draw the
/// whole image then).
pub fn cover_crop(w: f64, h: f64) -> [f64; 4] {
    if !w.is_finite() || !h.is_finite() || w <= 0.0 || h <= 0.0 {
        return [0.0; 4];
    }
    let side = w.min(h);
    [(w - side) / 2.0, (h - side) / 2.0, side, side]
}

/// The bundle id to bring forward when the title is clicked: `None` unless it looks like one
/// (letters, digits, `.`, `-`, `_`; no leading dash), so nothing odd is ever handed to the system.
pub fn open_target(bundle_id: &str) -> Option<&str> {
    let b = bundle_id.trim();
    let ok = !b.is_empty() && !b.starts_with('-') && b.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
    ok.then_some(b)
}

/// The clickable text: the title and artist rows together.
pub fn link_rect(e: &Expanded) -> [f64; 4] {
    [e.title[0] - 6.0, e.title[1] - 2.0, e.title[2] + 12.0, (e.artist[1] + e.artist[3]) - e.title[1] + 4.0]
}

/// Height of the bar's hit area.
pub const BAR_HIT_H: f64 = 16.0;

/// The progress bar's hit area: as wide as the bar and [`BAR_HIT_H`] tall, centred on it.
pub fn bar_hit_rect(bar: [f64; 4]) -> [f64; 4] {
    [bar[0], bar[1] + bar[3] / 2.0 - BAR_HIT_H / 2.0, bar[2], BAR_HIT_H]
}

pub fn contains(r: [f64; 4], p: [f64; 2]) -> bool {
    p[0] >= r[0] && p[0] <= r[0] + r[2] && p[1] >= r[1] && p[1] <= r[1] + r[3]
}

/// A track can be seeked only when it has a duration (a live stream has none).
pub fn seekable(duration: f64) -> bool {
    duration.is_finite() && duration > 0.0
}

/// Position in seconds under pointer `x`: the bar's left end is 0, its right end the duration;
/// outside the bar it clamps.
pub fn position_for_x(bar: [f64; 4], x: f64, duration: f64) -> f64 {
    if !x.is_finite() || !seekable(duration) || bar[2] <= 0.0 {
        return 0.0;
    }
    ((x - bar[0]) / bar[2]).clamp(0.0, 1.0) * duration
}

/// A press on the bar, until release. Moving only updates [`Scrub::position`] (what to show);
/// [`Scrub::end`] consumes it and gives the single position to seek to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scrub {
    bar: [f64; 4],
    duration: f64,
    pos: f64,
}

impl Scrub {
    /// Start at a press at (`x`, `y`); `None` outside the hit area or for a track without duration.
    pub fn begin(bar: [f64; 4], x: f64, y: f64, duration: f64) -> Option<Scrub> {
        (seekable(duration) && contains(bar_hit_rect(bar), [x, y])).then(|| Scrub { bar, duration, pos: position_for_x(bar, x, duration) })
    }

    /// The pointer moved (only its x counts once the press started).
    pub fn drag(&mut self, x: f64) {
        self.pos = position_for_x(self.bar, x, self.duration);
    }

    /// Where the scrub is now, in seconds.
    pub fn position(&self) -> f64 {
        self.pos
    }

    /// The button was released at `x`: the position to seek to.
    pub fn end(self, x: f64) -> f64 {
        position_for_x(self.bar, x, self.duration)
    }
}

#[cfg(test)]
#[path = "nowplaying_input_tests.rs"]
mod tests;
