//! The data source of the notch player: JSON lines of `ungive/mediaremote-adapter`'s `stream`
//! command, merged into one [`Snapshot`]; the command lines to run; and the restart policy.
//! No process, no clock and no AppKit here, so all of it is tested with plain values.
//!
//! The adapter is run as `/usr/bin/perl <script> <framework> stream --micros --debounce=N`. Each
//! line is `{"type":"data","diff":bool,"payload":{...}}`. With `diff` true only changed keys are
//! present (a key that vanished is `null`); with `diff` false the payload is the whole state. An
//! empty payload means nothing plays. Times arrive in microseconds (`--micros`): `elapsedTimeMicros`
//! is the position at `timestampEpochMicros`, so the position now is computed, not polled.

use std::sync::Arc;

use base64::Engine;
use serde_json::{Map, Value};

use crate::nowplaying::{Command, PlayState, Snapshot};

/// The system perl that is entitled to use MediaRemote.
pub const PERL: &str = "/usr/bin/perl";
/// Upstream merges bursts of updates for this many milliseconds before printing one line.
pub const DEBOUNCE_MS: u32 = 150;
/// After this many exits without a good line in a row the helper is given up on.
pub const MAX_FAILURES: u32 = 5;
/// A run this long counts as healthy even without data (nothing may be playing).
pub const STABLE_SECS: f64 = 30.0;
/// First restart delay; it doubles up to [`MAX_BACKOFF_SECS`].
pub const BASE_BACKOFF_SECS: f64 = 1.0;
pub const MAX_BACKOFF_SECS: f64 = 30.0;

/// A line longer than this (cover art included) is dropped unread.
pub const MAX_LINE_BYTES: usize = 8 * 1024 * 1024;
/// A decoded cover larger than this is ignored.
pub const MAX_ARTWORK_BYTES: usize = 4 * 1024 * 1024;
/// A cover wider or taller than this many pixels is ignored (it would be decoded at full size).
pub const MAX_ARTWORK_SIDE: u32 = 2048;

fn be16(b: &[u8], i: usize) -> Option<u32> {
    Some(u16::from_be_bytes([*b.get(i)?, *b.get(i + 1)?]) as u32)
}

fn le(b: &[u8], i: usize, n: usize) -> Option<u32> {
    let s = b.get(i..i + n)?;
    Some(s.iter().rev().fold(0u32, |a, x| (a << 8) | *x as u32))
}

/// Pixel size (width, height) read from the header of a PNG, JPEG, GIF or WebP; `None` for anything
/// else or a truncated header. Nothing is decoded.
pub fn image_size(b: &[u8]) -> Option<(u32, u32)> {
    if b.starts_with(b"\x89PNG\r\n\x1a\n") && b.get(12..16) == Some(b"IHDR") {
        return Some((u32::from_be_bytes(b.get(16..20)?.try_into().ok()?), u32::from_be_bytes(b.get(20..24)?.try_into().ok()?)));
    }
    if b.starts_with(b"GIF8") {
        return Some((le(b, 6, 2)?, le(b, 8, 2)?));
    }
    if b.starts_with(b"\xFF\xD8") {
        let mut i = 2;
        while i + 4 <= b.len() {
            if b[i] != 0xFF {
                return None;
            }
            let m = b[i + 1];
            match m {
                0xFF => i += 1,
                0xD8 | 0x01 | 0xD0..=0xD7 => i += 2,
                0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF => return Some((be16(b, i + 7)?, be16(b, i + 5)?)),
                _ => i += 2 + be16(b, i + 2)? as usize,
            }
        }
        return None;
    }
    if b.starts_with(b"RIFF") && b.get(8..12) == Some(b"WEBP") {
        return match b.get(12..16)? {
            b"VP8X" => Some((le(b, 24, 3)? + 1, le(b, 27, 3)? + 1)),
            b"VP8L" => {
                let bits = le(b, 21, 4)?;
                Some(((bits & 0x3FFF) + 1, ((bits >> 14) & 0x3FFF) + 1))
            }
            b"VP8 " => Some((le(b, 26, 2)? & 0x3FFF, le(b, 28, 2)? & 0x3FFF)),
            _ => None,
        };
    }
    None
}

/// Decode a cover within the caps: at most [`MAX_ARTWORK_BYTES`] and [`MAX_ARTWORK_SIDE`] pixels a
/// side when the header says (a format whose size cannot be read is bounded by the byte cap alone).
fn bounded_artwork(b64: &str) -> Option<Arc<Vec<u8>>> {
    let b64 = b64.trim();
    if b64.len() > MAX_ARTWORK_BYTES / 3 * 4 + 8 {
        return None;
    }
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
    if bytes.is_empty() || bytes.len() > MAX_ARTWORK_BYTES {
        return None;
    }
    if let Some((w, h)) = image_size(&bytes) {
        if w == 0 || h == 0 || w > MAX_ARTWORK_SIDE || h > MAX_ARTWORK_SIDE {
            return None;
        }
    }
    Some(Arc::new(bytes))
}

/// The adapter's `send` command id for a button (see its README: 2 toggle, 4 next, 5 previous).
pub fn command_id(c: Command) -> u8 {
    match c {
        Command::PlayPause => 2,
        Command::Next => 4,
        Command::Previous => 5,
    }
}

/// Arguments (after the program) that start the stream.
pub fn stream_args(script: &str, framework: &str) -> Vec<String> {
    [script, framework, "stream", "--micros", &format!("--debounce={DEBOUNCE_MS}")].map(String::from).to_vec()
}

/// Arguments (after the program) that send a transport button.
pub fn send_args(script: &str, framework: &str, c: Command) -> Vec<String> {
    [script, framework, "send", &command_id(c).to_string()].map(String::from).to_vec()
}

/// Arguments (after the program) that seek to `secs`: the adapter's `seek` takes whole microseconds.
pub fn seek_args(script: &str, framework: &str, secs: f64) -> Vec<String> {
    let micros = if secs.is_finite() && secs > 0.0 { (secs * 1e6).round() as u64 } else { 0 };
    [script, framework, "seek", &micros.to_string()].map(String::from).to_vec()
}

/// Whatever the lines said so far.
#[derive(Debug, Default)]
pub struct Tracker {
    bundle_id: Option<String>,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    item_id: Option<String>,
    playing: bool,
    rate: f64,
    duration_micros: f64,
    elapsed_micros: f64,
    timestamp_epoch_micros: Option<f64>,
    artwork: Option<Arc<Vec<u8>>>,
}

fn text(v: &Value) -> Option<String> {
    v.as_str().map(str::to_string)
}

fn micros(v: &Value) -> f64 {
    v.as_f64().filter(|n| n.is_finite() && *n >= 0.0).unwrap_or(0.0)
}

impl Tracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply one output line. Returns false for a line that is not a data line (malformed JSON,
    /// another `type`, no object payload): it is ignored and changes nothing. True when it was a
    /// well-formed data line, whether or not the visible state changed.
    pub fn feed(&mut self, line: &str) -> bool {
        if line.len() > MAX_LINE_BYTES {
            return false;
        }
        let Ok(Value::Object(top)) = serde_json::from_str::<Value>(line.trim()) else { return false };
        if top.get("type").and_then(Value::as_str).is_some_and(|t| t != "data") {
            return false;
        }
        let Some(Value::Object(payload)) = top.get("payload") else { return false };
        let diff = top.get("diff").and_then(Value::as_bool).unwrap_or(false);
        self.apply(payload, diff);
        true
    }

    fn apply(&mut self, p: &Map<String, Value>, diff: bool) {
        let before = (self.bundle_id.clone(), self.title.clone(), self.artist.clone(), self.item_id.clone());
        let old_art = self.artwork.take();
        if !diff {
            *self = Tracker::default();
        }
        self.artwork = old_art;
        let mut new_art = None;
        for (k, v) in p {
            match k.as_str() {
                "bundleIdentifier" => self.bundle_id = text(v),
                "title" => self.title = text(v),
                "artist" => self.artist = text(v),
                "album" => self.album = text(v),
                "contentItemIdentifier" => self.item_id = text(v),
                "playing" => self.playing = v.as_bool().unwrap_or(false),
                "playbackRate" => self.rate = v.as_f64().unwrap_or(0.0),
                "durationMicros" => self.duration_micros = micros(v),
                "elapsedTimeMicros" => self.elapsed_micros = micros(v),
                "timestampEpochMicros" => self.timestamp_epoch_micros = v.as_f64().filter(|n| n.is_finite()),
                // null or undecodable: no new cover (macOS unloads it on pause)
                "artworkData" => new_art = v.as_str().and_then(bounded_artwork),
                _ => {}
            }
        }
        // The cover is replaced only by a new non-empty one or by a different track. The same
        // track (same item id, or same app + title + artist) keeps its cover through a pause.
        let same_track = (self.item_id.is_some() && self.item_id == before.3)
            || (self.bundle_id.is_some() && (&self.bundle_id, &self.title, &self.artist) == (&before.0, &before.1, &before.2));
        if new_art.is_some() {
            self.artwork = new_art;
        } else if !same_track {
            self.artwork = None;
        }
    }

    /// What plays at `now_epoch_micros` (microseconds since the Unix epoch); `None` when nothing
    /// does: no app, or no title (the adapter drops untitled media itself, this is the second line).
    pub fn snapshot(&self, now_epoch_micros: f64) -> Option<Snapshot> {
        let bundle_id = self.bundle_id.clone()?;
        let title = self.title.clone().filter(|t| !t.trim().is_empty())?;
        let mut elapsed = self.elapsed_micros;
        if self.playing {
            if let Some(ts) = self.timestamp_epoch_micros {
                let rate = if self.rate > 0.0 { self.rate } else { 1.0 };
                elapsed += (now_epoch_micros - ts).max(0.0) * rate;
            }
        }
        let duration = self.duration_micros / 1e6;
        let mut position = elapsed / 1e6;
        if duration > 0.0 {
            position = position.min(duration);
        }
        Some(Snapshot {
            bundle_id,
            state: if self.playing { PlayState::Playing } else { PlayState::Paused },
            title,
            artist: self.artist.clone().unwrap_or_default(),
            album: self.album.clone().unwrap_or_default(),
            duration,
            position,
            artwork: self.artwork.clone(),
        })
    }
}

/// What to do after the helper process exited.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Next {
    /// Start it again after this many seconds.
    Restart(f64),
    /// Stop trying: it keeps failing (a macOS update may have broken the adapter).
    GiveUp,
}

/// Restart policy: exponential backoff, and give up after [`MAX_FAILURES`] failures in a row.
#[derive(Debug, Default)]
pub struct Supervisor {
    failures: u32,
}

impl Supervisor {
    pub fn new() -> Self {
        Self::default()
    }

    /// The process exited after `ran_for` seconds; `got_data` says whether it printed a valid line.
    pub fn on_exit(&mut self, ran_for: f64, got_data: bool) -> Next {
        if got_data || ran_for >= STABLE_SECS {
            self.failures = 0;
            return Next::Restart(BASE_BACKOFF_SECS);
        }
        self.failures += 1;
        if self.failures >= MAX_FAILURES {
            return Next::GiveUp;
        }
        Next::Restart((BASE_BACKOFF_SECS * 2f64.powi(self.failures as i32 - 1)).min(MAX_BACKOFF_SECS))
    }
}

#[cfg(test)]
#[path = "nowplaying_feed_tests.rs"]
mod tests;
