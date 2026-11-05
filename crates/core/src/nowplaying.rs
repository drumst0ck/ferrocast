//! Notch "now playing" player (S3): the state behind it, with no AppKit and no process.
//!
//! * Source of truth: macOS's own Now Playing, whatever app reports to it (browsers, Spotify,
//!   Music, Podcasts, VLC...), read through the bundled `ungive/mediaremote-adapter`
//!   (see `nowplaying_feed`). There is no other source.
//! * [`Ctl`] is the state machine the app drives: media updates, the pointer and the setting go in;
//!   what to draw comes out ([`Model`]), and only when it changed.
//!
//! Time is a plain `f64` of seconds on a monotonic clock ([`mono_secs`]) so every rule is a pure
//! function that tests can step through.

use std::sync::OnceLock;

/// Paused or idle for this long (and not under the pointer) and the player goes away.
pub const IDLE_HIDE_SECS: f64 = 45.0;
/// After a seek, updates still carrying the old position are ignored for this long.
pub const SEEK_HOLD_SECS: f64 = 3.0;
/// A position this far from where the last sample predicted it counts as a seek (a redraw).
pub const SEEK_JUMP_SECS: f64 = 2.0;

use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayState {
    Playing,
    Paused,
}

/// A transport button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Previous,
    PlayPause,
    Next,
}

/// What the system reported at one moment.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    /// Bundle id of the app that plays (its icon and name are shown); may be empty in tests.
    pub bundle_id: String,
    pub state: PlayState,
    pub title: String,
    pub artist: String,
    pub album: String,
    /// Seconds; 0 when unknown (streams).
    pub duration: f64,
    /// Seconds into the track when sampled.
    pub position: f64,
    /// Encoded image (JPEG/PNG...) decoded from the adapter's base64; `None` shows a placeholder.
    pub artwork: Option<Arc<Vec<u8>>>,
}

/// Position `elapsed` seconds after the snapshot was taken: advances only while playing and never
/// passes a known duration.
pub fn position_at(s: &Snapshot, elapsed: f64) -> f64 {
    if s.state != PlayState::Playing {
        return s.position;
    }
    let p = s.position + elapsed.max(0.0);
    if s.duration > 0.0 {
        p.min(s.duration)
    } else {
        p
    }
}

/// 0..=1 progress; 0 when the duration is unknown.
pub fn fraction(position: f64, duration: f64) -> f64 {
    if !position.is_finite() || !duration.is_finite() || duration <= 0.0 {
        return 0.0;
    }
    (position / duration).clamp(0.0, 1.0)
}

/// `m:ss`, or `h:mm:ss` from one hour on.
pub fn format_clock(secs: f64) -> String {
    let total = if secs.is_finite() && secs > 0.0 { secs as u64 } else { 0 };
    let (h, m, s) = (total / 3600, total % 3600 / 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// Seconds on a monotonic clock that starts at the first call.
pub fn mono_secs() -> f64 {
    static START: OnceLock<std::time::Instant> = OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_secs_f64()
}

/// What the feed tells the state machine.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// What plays now (`None`: nothing).
    Media(Option<Snapshot>),
    /// An app covers the whole main display.
    Fullscreen(bool),
    /// The idle deadline from [`Ctl::hide_deadline`] passed: look again whether the pill still belongs.
    Tick,
    /// The helper kept failing and was given up on.
    Unavailable,
}

/// What the notch shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    pub visible: bool,
    pub expanded: bool,
    pub snapshot: Option<Snapshot>,
    /// [`mono_secs`] when `snapshot` was sampled; progress is `position_at(snapshot, now - sampled_at)`.
    pub sampled_at: f64,
}

impl Model {
    fn hidden() -> Model {
        Model { visible: false, expanded: false, snapshot: None, sampled_at: 0.0 }
    }

    /// True when drawing `other` would look the same as drawing `self` at `now`.
    fn same_view(&self, other: &Model, now: f64) -> bool {
        if self.visible != other.visible || self.expanded != other.expanded {
            return false;
        }
        match (&self.snapshot, &other.snapshot) {
            (None, None) => true,
            (Some(a), Some(b)) => {
                a.bundle_id == b.bundle_id
                    && a.state == b.state
                    && a.title == b.title
                    && a.artist == b.artist
                    && a.album == b.album
                    && a.duration == b.duration
                    && a.artwork == b.artwork
                    && (position_at(a, now - self.sampled_at) - position_at(b, now - other.sampled_at)).abs() <= SEEK_JUMP_SECS
            }
            _ => false,
        }
    }
}

/// What a transition asks the caller to do.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Out {
    /// Draw this (`None`: nothing changed, do not redraw).
    pub render: Option<Model>,
    /// The feed gave up: tell the user once (until the setting is turned off and on).
    pub unavailable: bool,
}

/// The player's state machine.
#[derive(Debug)]
pub struct Ctl {
    enabled: bool,
    hover: bool,
    fullscreen: bool,
    snap: Option<Snapshot>,
    sampled_at: f64,
    was_playing: bool,
    /// Last moment something was playing (or the player was paused, or the pointer left).
    last_active: f64,
    told_unavailable: bool,
    /// A seek the player has not confirmed yet: target seconds and when it was made.
    pending_seek: Option<(f64, f64)>,
    shown: Model,
}

impl Default for Ctl {
    fn default() -> Self {
        Self::new()
    }
}

impl Ctl {
    /// Off, showing nothing.
    pub fn new() -> Self {
        Ctl {
            enabled: false,
            hover: false,
            fullscreen: false,
            snap: None,
            sampled_at: 0.0,
            was_playing: false,
            last_active: 0.0,
            told_unavailable: false,
            pending_seek: None,
            shown: Model::hidden(),
        }
    }

    /// The setting changed. Turning it off hides at once; turning it on starts from a clean slate
    /// (and lets "unavailable" be reported again).
    pub fn set_enabled(&mut self, on: bool, now: f64) -> Out {
        if on == self.enabled {
            return Out::default();
        }
        self.enabled = on;
        self.hover = false;
        self.fullscreen = false;
        self.snap = None;
        self.was_playing = false;
        self.told_unavailable = false;
        self.pending_seek = None;
        self.last_active = now;
        Out { render: self.refresh(now), unavailable: false }
    }

    /// The system reported what plays now (`None`: nothing).
    pub fn on_media(&mut self, now: f64, snap: Option<Snapshot>) -> Out {
        if !self.enabled {
            return Out::default();
        }
        let mut snap = snap;
        self.hold_seek(now, &mut snap);
        let playing = snap.as_ref().is_some_and(|s| s.state == PlayState::Playing);
        // the update that notices a pause is the pause: the idle clock starts there
        if playing || self.was_playing {
            self.last_active = now;
        }
        self.was_playing = playing;
        self.snap = snap;
        self.sampled_at = now;
        Out { render: self.refresh(now), unavailable: false }
    }

    /// Keep the position of a seek the player has not reported yet: for [`SEEK_HOLD_SECS`], an update
    /// of the same track far from where the seek put us is shown at the expected position instead; one
    /// close to it confirms the seek and ends the hold.
    fn hold_seek(&mut self, now: f64, snap: &mut Option<Snapshot>) {
        let Some((target, at)) = self.pending_seek else { return };
        let (Some(s), Some(old)) = (snap.as_mut(), self.snap.as_ref()) else {
            self.pending_seek = None;
            return;
        };
        let same = s.bundle_id == old.bundle_id && s.title == old.title && s.artist == old.artist;
        if !same || now - at > SEEK_HOLD_SECS {
            self.pending_seek = None;
            return;
        }
        let mut expected = if s.state == PlayState::Playing { target + (now - at) } else { target };
        if s.duration > 0.0 {
            expected = expected.min(s.duration);
        }
        if (s.position - expected).abs() <= 1.5 {
            self.pending_seek = None;
        } else {
            s.position = expected;
        }
    }

    /// The user seeked to `secs` on the bar. `None` when nothing seekable is shown; otherwise the
    /// position to send (clamped to the track) and a redraw that already shows it.
    pub fn seek_to(&mut self, now: f64, secs: f64) -> Option<(f64, Out)> {
        if !self.enabled || !self.shown.visible {
            return None;
        }
        let snap = self.snap.as_mut()?;
        if !crate::nowplaying_input::seekable(snap.duration) {
            return None;
        }
        let secs = if secs.is_finite() { secs.clamp(0.0, snap.duration) } else { 0.0 };
        snap.position = secs;
        self.sampled_at = now;
        self.pending_seek = Some((secs, now));
        self.last_active = now;
        let next = Model { visible: true, expanded: self.hover, snapshot: self.snap.clone(), sampled_at: now };
        self.shown = next.clone();
        Some((secs, Out { render: Some(next), unavailable: false }))
    }

    /// Bundle id of the app whose track is on screen (what a click on the title opens).
    pub fn source_bundle(&self) -> Option<String> {
        if !self.shown.visible {
            return None;
        }
        self.shown.snapshot.as_ref().map(|s| s.bundle_id.clone())
    }

    /// An app covers (or no longer covers) the whole main display.
    pub fn set_fullscreen(&mut self, now: f64, on: bool) -> Out {
        if !self.enabled || on == self.fullscreen {
            return Out::default();
        }
        self.fullscreen = on;
        Out { render: self.refresh(now), unavailable: false }
    }

    /// The feed gave up for good: nothing is shown and the user is told, once.
    pub fn on_unavailable(&mut self, now: f64) -> Out {
        if !self.enabled {
            return Out::default();
        }
        self.snap = None;
        self.was_playing = false;
        let first = !std::mem::replace(&mut self.told_unavailable, true);
        Out { render: self.refresh(now), unavailable: first }
    }

    /// Route an [`Event`].
    pub fn on_event(&mut self, now: f64, e: Event) -> Out {
        match e {
            Event::Media(s) => self.on_media(now, s),
            Event::Fullscreen(f) => self.set_fullscreen(now, f),
            Event::Tick => self.tick(now),
            Event::Unavailable => self.on_unavailable(now),
        }
    }

    /// Time passed with no event (a paused player's snapshot stops changing, so none comes): hide
    /// the pill once it has been idle for [`IDLE_HIDE_SECS`]. A playing player is never idle.
    pub fn tick(&mut self, now: f64) -> Out {
        if !self.enabled {
            return Out::default();
        }
        if self.snap.as_ref().is_some_and(|s| s.state == PlayState::Playing) {
            self.last_active = now;
        }
        Out { render: self.refresh(now), unavailable: false }
    }

    /// When [`Ctl::tick`] next has something to do (a [`mono_secs`] time), `None` when no idle
    /// hide is pending: nothing shown, playing, or the pointer is on the player.
    pub fn hide_deadline(&self) -> Option<f64> {
        let paused = self.snap.as_ref().is_some_and(|s| s.state != PlayState::Playing);
        (self.enabled && self.shown.visible && !self.hover && paused).then_some(self.last_active + IDLE_HIDE_SECS)
    }

    /// The pointer entered or left the player. Ignored while nothing is shown.
    pub fn set_hover(&mut self, hover: bool, now: f64) -> Out {
        if !self.enabled {
            return Out::default();
        }
        let hover = hover && self.shown.visible;
        if self.hover && !hover {
            self.last_active = now;
        }
        self.hover = hover;
        Out { render: self.refresh(now), unavailable: false }
    }

    /// True while the pill is on screen: its buttons act on whatever the system is playing.
    pub fn can_command(&self) -> bool {
        self.shown.visible
    }

    /// Recompute what is shown; `Some` only when it would look different.
    fn refresh(&mut self, now: f64) -> Option<Model> {
        let visible = self.enabled && !self.fullscreen && self.snap.is_some() && (self.hover || now - self.last_active <= IDLE_HIDE_SECS);
        if !visible {
            self.hover = false;
        }
        let next = if visible {
            Model { visible, expanded: self.hover, snapshot: self.snap.clone(), sampled_at: self.sampled_at }
        } else {
            Model::hidden()
        };
        if self.shown.same_view(&next, now) {
            return None;
        }
        self.shown = next.clone();
        Some(next)
    }
}

#[cfg(test)]
#[path = "nowplaying_tests.rs"]
mod tests;
