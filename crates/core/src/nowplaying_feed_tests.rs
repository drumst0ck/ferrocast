use super::*;
use base64::Engine;

const NOW: f64 = 1_800_000_000_000_000.0; // µs since the epoch

fn full(extra: &str) -> String {
    format!(
        r#"{{"type":"data","diff":false,"payload":{{"bundleIdentifier":"com.brave.Browser","title":"Song","artist":"Band","album":"Record","playing":true,"playbackRate":1,"durationMicros":200000000,"elapsedTimeMicros":10000000,"timestampEpochMicros":{NOW}{extra}}}}}"#
    )
}

fn tracker(line: &str) -> Tracker {
    let mut t = Tracker::new();
    assert!(t.feed(line), "{line}");
    t
}

// ---------------------------------------------------------------- parsing

#[test]
fn a_full_line_gives_every_field() {
    let s = tracker(&full("")).snapshot(NOW).unwrap();
    assert_eq!(s.bundle_id, "com.brave.Browser");
    assert_eq!((s.title.as_str(), s.artist.as_str(), s.album.as_str()), ("Song", "Band", "Record"));
    assert_eq!(s.state, PlayState::Playing);
    assert_eq!((s.duration, s.position), (200.0, 10.0));
    assert!(s.artwork.is_none());
}

#[test]
fn optional_keys_may_be_missing() {
    let t = tracker(
        r#"{"type":"data","diff":false,"payload":{"bundleIdentifier":"com.spotify.client","title":"Only a title","playing":false}}"#,
    );
    let s = t.snapshot(NOW).unwrap();
    assert_eq!((s.artist.as_str(), s.album.as_str()), ("", ""));
    assert_eq!((s.duration, s.position, s.state), (0.0, 0.0, PlayState::Paused));
}

#[test]
fn nothing_playing_is_an_empty_payload() {
    let mut t = tracker(&full(""));
    assert!(t.feed(r#"{"type":"data","diff":false,"payload":{}}"#));
    assert_eq!(t.snapshot(NOW), None);
}

#[test]
fn null_in_a_diff_removes_the_key_and_a_diff_updates_the_rest() {
    let mut t = tracker(&full(r#","genre":"x""#));
    assert!(t.feed(r#"{"type":"data","diff":true,"payload":{"playing":false,"album":null}}"#));
    let s = t.snapshot(NOW).unwrap();
    assert_eq!(s.state, PlayState::Paused);
    assert_eq!(s.album, "");
    assert_eq!(s.title, "Song", "untouched keys stay");
}

#[test]
fn a_title_is_required_and_the_app_too() {
    let mut t = tracker(&full(""));
    t.feed(r#"{"type":"data","diff":true,"payload":{"title":null}}"#);
    assert_eq!(t.snapshot(NOW), None);
    let t = tracker(r#"{"type":"data","diff":false,"payload":{"title":"x","playing":true}}"#);
    assert_eq!(t.snapshot(NOW), None, "no bundle id");
}

#[test]
fn artwork_is_decoded_from_base64_in_memory() {
    let b64 = base64::engine::general_purpose::STANDARD.encode([1u8, 2, 3, 255]);
    let s = tracker(&full(&format!(r#","artworkData":"{b64}","artworkMimeType":"image/png""#))).snapshot(NOW).unwrap();
    assert_eq!(s.artwork.as_deref().map(Vec::as_slice), Some(&[1u8, 2, 3, 255][..]));
    // not base64: no cover, the rest still shows
    let s = tracker(&full(r#","artworkData":"%%%not base64""#)).snapshot(NOW).unwrap();
    assert!(s.artwork.is_none());
    assert_eq!(s.title, "Song");
}

#[test]
fn a_new_track_drops_the_old_cover_until_a_new_one_arrives() {
    let b64 = base64::engine::general_purpose::STANDARD.encode([9u8; 8]);
    let mut t = tracker(&full(&format!(r#","artworkData":"{b64}""#)));
    // an update of the same track keeps it
    t.feed(r#"{"type":"data","diff":true,"payload":{"playing":false}}"#);
    assert!(t.snapshot(NOW).unwrap().artwork.is_some());
    // a new title without a cover: the old one goes
    t.feed(r#"{"type":"data","diff":true,"payload":{"title":"Next"}}"#);
    assert!(t.snapshot(NOW).unwrap().artwork.is_none());
    // the new cover arriving later is kept
    t.feed(&format!(r#"{{"type":"data","diff":true,"payload":{{"artworkData":"{b64}"}}}}"#));
    assert!(t.snapshot(NOW).unwrap().artwork.is_some());
}

#[test]
fn malformed_and_foreign_lines_are_ignored_and_change_nothing() {
    let mut t = tracker(&full(""));
    for bad in [
        "",
        "not json",
        "{\"type\":\"data\"",
        "[1,2]",
        "null",
        r#"{"type":"error","payload":{"title":"x"}}"#,
        r#"{"type":"data","diff":false}"#,
        r#"{"type":"data","diff":false,"payload":5}"#,
    ] {
        assert!(!t.feed(bad), "{bad:?}");
    }
    assert_eq!(t.snapshot(NOW).unwrap().title, "Song");
}

#[test]
fn wrong_typed_fields_do_not_panic() {
    let t = tracker(
        r#"{"type":"data","diff":false,"payload":{"bundleIdentifier":"a","title":"t","playing":"yes","durationMicros":"x","elapsedTimeMicros":-5}}"#,
    );
    let s = t.snapshot(NOW).unwrap();
    assert_eq!((s.state, s.duration, s.position), (PlayState::Paused, 0.0, 0.0));
}

// ---------------------------------------------------------------- elapsed time

#[test]
fn playing_position_advances_from_the_timestamp() {
    let t = tracker(&full(""));
    assert_eq!(t.snapshot(NOW).unwrap().position, 10.0);
    assert_eq!(t.snapshot(NOW + 5_500_000.0).unwrap().position, 15.5);
    assert_eq!(t.snapshot(NOW - 3_000_000.0).unwrap().position, 10.0, "a clock behind the stamp never rewinds");
    assert_eq!(t.snapshot(NOW + 900_000_000_000.0).unwrap().position, 200.0, "clamped to the duration");
}

#[test]
fn a_paused_position_stays_and_the_rate_scales_playing_time() {
    let mut t = tracker(&full(""));
    t.feed(r#"{"type":"data","diff":true,"payload":{"playing":false}}"#);
    assert_eq!(t.snapshot(NOW + 60_000_000.0).unwrap().position, 10.0);
    t.feed(r#"{"type":"data","diff":true,"payload":{"playing":true,"playbackRate":2}}"#);
    assert_eq!(t.snapshot(NOW + 5_000_000.0).unwrap().position, 20.0);
    t.feed(r#"{"type":"data","diff":true,"payload":{"playbackRate":0}}"#);
    assert_eq!(t.snapshot(NOW + 5_000_000.0).unwrap().position, 15.0, "rate 0 while playing counts as normal speed");
}

// ---------------------------------------------------------------- commands

#[test]
fn buttons_map_to_the_adapters_send_ids() {
    assert_eq!(command_id(Command::PlayPause), 2); // kMRTogglePlayPause
    assert_eq!(command_id(Command::Next), 4); // kMRNextTrack
    assert_eq!(command_id(Command::Previous), 5); // kMRPreviousTrack
}

#[test]
fn the_command_lines_are_argv_for_the_system_perl() {
    assert_eq!(PERL, "/usr/bin/perl");
    assert_eq!(send_args("/r/a.pl", "/f/M.framework", Command::Next), ["/r/a.pl", "/f/M.framework", "send", "4"]);
    let s = stream_args("/r/a.pl", "/f/M.framework");
    assert_eq!(s[..3], ["/r/a.pl", "/f/M.framework", "stream"]);
    assert!(s.contains(&"--micros".to_string()) && s.contains(&format!("--debounce={DEBOUNCE_MS}")));
    assert!(!s.contains(&"--no-artwork".to_string()), "the cover is wanted");
}

// ---------------------------------------------------------------- restart policy

#[test]
fn failures_back_off_exponentially_then_give_up() {
    let mut s = Supervisor::new();
    assert_eq!(s.on_exit(0.1, false), Next::Restart(1.0));
    assert_eq!(s.on_exit(0.1, false), Next::Restart(2.0));
    assert_eq!(s.on_exit(0.1, false), Next::Restart(4.0));
    assert_eq!(s.on_exit(0.1, false), Next::Restart(8.0));
    assert_eq!(s.on_exit(0.1, false), Next::GiveUp);
}

#[test]
fn a_good_run_resets_the_count() {
    let mut s = Supervisor::new();
    for _ in 0..4 {
        s.on_exit(0.1, false);
    }
    assert_eq!(s.on_exit(1.0, true), Next::Restart(1.0), "it printed data: healthy");
    assert_eq!(s.on_exit(0.1, false), Next::Restart(1.0), "the count started over");
    for _ in 0..3 {
        s.on_exit(0.1, false);
    }
    assert_eq!(s.on_exit(STABLE_SECS, false), Next::Restart(1.0), "a long run is healthy too");
}

#[test]
fn the_backoff_is_capped() {
    assert!(BASE_BACKOFF_SECS * 2f64.powi(MAX_FAILURES as i32 - 2) <= MAX_BACKOFF_SECS, "the schedule stays under the cap");
}

// ---------------------------------------------------------------- the cover survives a pause

fn cover() -> String {
    base64::engine::general_purpose::STANDARD.encode([7u8; 16])
}

#[test]
fn a_pause_diff_without_artwork_keeps_the_cover() {
    let mut t = tracker(&full(&format!(r#","artworkData":"{}""#, cover())));
    t.feed(r#"{"type":"data","diff":true,"payload":{"playing":false,"elapsedTimeMicros":12000000}}"#);
    assert!(t.snapshot(NOW).unwrap().artwork.is_some());
}

#[test]
fn a_null_artwork_on_the_same_track_keeps_the_cover() {
    // macOS unloads the artwork when a player pauses: the diff says artworkData is null
    let mut t = tracker(&full(&format!(r#","artworkData":"{}""#, cover())));
    t.feed(r#"{"type":"data","diff":true,"payload":{"playing":false,"artworkData":null,"artworkMimeType":null}}"#);
    assert!(t.snapshot(NOW).unwrap().artwork.is_some());
}

#[test]
fn a_full_payload_without_artwork_for_the_same_track_keeps_the_cover() {
    let mut t = tracker(&full(&format!(r#","artworkData":"{}""#, cover())));
    // same title, artist and app, no item id
    assert!(t.feed(&full("").replace(r#""playing":true"#, r#""playing":false"#)));
    let s = t.snapshot(NOW).unwrap();
    assert_eq!(s.state, PlayState::Paused);
    assert!(s.artwork.is_some());
    // the same item id also counts as the same track, even with another title
    let mut t = tracker(&full(&format!(r#","contentItemIdentifier":"X1","artworkData":"{}""#, cover())));
    t.feed(&full(r#","contentItemIdentifier":"X1""#).replace("Song", "Song (remaster)"));
    assert!(t.snapshot(NOW).unwrap().artwork.is_some());
}

#[test]
fn a_new_track_clears_the_cover_even_through_a_full_payload() {
    let mut t = tracker(&full(&format!(r#","contentItemIdentifier":"X1","artworkData":"{}""#, cover())));
    t.feed(&full(r#","contentItemIdentifier":"X2""#).replace("Song", "Other").replace("Band", "Else"));
    assert!(t.snapshot(NOW).unwrap().artwork.is_none());
    // another app with the same title and artist is not the same track
    let mut t = tracker(&full(&format!(r#","artworkData":"{}""#, cover())));
    t.feed(&full("").replace("com.brave.Browser", "com.spotify.client"));
    assert!(t.snapshot(NOW).unwrap().artwork.is_none());
}

#[test]
fn a_new_non_empty_cover_replaces_the_old_one() {
    let mut t = tracker(&full(&format!(r#","artworkData":"{}""#, cover())));
    let newer = base64::engine::general_purpose::STANDARD.encode([1u8, 2, 3]);
    t.feed(&format!(r#"{{"type":"data","diff":true,"payload":{{"artworkData":"{newer}"}}}}"#));
    assert_eq!(t.snapshot(NOW).unwrap().artwork.as_deref().map(Vec::as_slice), Some(&[1u8, 2, 3][..]));
}

// ---------------------------------------------------------------- bounds on what the adapter sends

fn png(w: u32, h: u32, total: usize) -> Vec<u8> {
    let mut b = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    b.extend(w.to_be_bytes());
    b.extend(h.to_be_bytes());
    b.resize(total.max(b.len()), 0);
    b
}

fn jpeg(w: u16, h: u16) -> Vec<u8> {
    let mut b = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x4A, 0x46]; // SOI + a 4-byte APP0
    b.extend([0xFF, 0xC0, 0x00, 0x11, 0x08]);
    b.extend(h.to_be_bytes());
    b.extend(w.to_be_bytes());
    b.extend([3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
    b
}

fn with_art(bytes: &[u8]) -> Option<Snapshot> {
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    tracker(&full(&format!(r#","artworkData":"{b64}""#))).snapshot(NOW)
}

#[test]
fn image_sizes_are_read_from_the_headers() {
    assert_eq!(image_size(&png(640, 480, 0)), Some((640, 480)));
    assert_eq!(image_size(&jpeg(1200, 900)), Some((1200, 900)));
    let mut gif = b"GIF89a".to_vec();
    gif.extend([0x40, 0x01, 0xF0, 0x00]);
    assert_eq!(image_size(&gif), Some((320, 240)));
    let mut webp = b"RIFF\0\0\0\0WEBPVP8X\x0a\0\0\0\0\0\0\0".to_vec();
    webp.extend([0x7F, 0x02, 0x00, 0xDF, 0x01, 0x00]); // 640 x 480, stored minus one
    assert_eq!(image_size(&webp), Some((640, 480)));
    assert_eq!(image_size(&[1, 2, 3]), None);
    assert_eq!(image_size(&[]), None);
    assert_eq!(image_size(&png(1, 1, 0)[..12]), None, "a truncated header");
}

#[test]
fn a_cover_within_the_pixel_cap_is_kept_and_a_bigger_one_is_ignored() {
    assert!(with_art(&png(MAX_ARTWORK_SIDE, MAX_ARTWORK_SIDE, 100)).unwrap().artwork.is_some());
    assert!(with_art(&jpeg(1200, 1200)).unwrap().artwork.is_some());
    let big = with_art(&png(MAX_ARTWORK_SIDE + 1, 100, 100)).unwrap();
    assert!(big.artwork.is_none(), "too wide");
    assert!(with_art(&png(100, 60_000, 100)).unwrap().artwork.is_none(), "too tall");
    assert!(with_art(&jpeg(9000, 9000)).unwrap().artwork.is_none());
    assert_eq!(big.title, "Song", "the rest of the track still shows");
}

#[test]
fn a_cover_above_the_byte_cap_is_ignored_without_decoding_it() {
    assert!(with_art(&png(10, 10, MAX_ARTWORK_BYTES)).unwrap().artwork.is_some(), "exactly at the cap");
    assert!(with_art(&png(10, 10, MAX_ARTWORK_BYTES + 1)).unwrap().artwork.is_none());
    // an oversized base64 string is dropped by length alone: it is not even valid base64 here
    let huge = "A".repeat(MAX_ARTWORK_BYTES * 4 / 3 + 16);
    let s = tracker(&full(&format!(r#","artworkData":"{huge}""#))).snapshot(NOW).unwrap();
    assert!(s.artwork.is_none());
}

#[test]
fn an_oversized_cover_does_not_replace_the_one_of_the_same_track() {
    let mut t = tracker(&full(&format!(r#","artworkData":"{}""#, cover())));
    let b64 = base64::engine::general_purpose::STANDARD.encode(png(10, 10, MAX_ARTWORK_BYTES + 1));
    t.feed(&format!(r#"{{"type":"data","diff":true,"payload":{{"artworkData":"{b64}"}}}}"#));
    assert!(t.snapshot(NOW).unwrap().artwork.is_some(), "the good cover stays");
}

#[test]
fn a_line_longer_than_the_cap_is_ignored() {
    let mut t = tracker(&full(""));
    let long = format!(r#"{{"type":"data","diff":true,"payload":{{"title":"{}"}}}}"#, "x".repeat(MAX_LINE_BYTES));
    assert!(!t.feed(&long));
    assert_eq!(t.snapshot(NOW).unwrap().title, "Song");
}

#[test]
fn seek_uses_whole_microseconds() {
    // the adapter's `seek` takes a position in microseconds (seek.m divides by 1e6)
    assert_eq!(seek_args("/r/a.pl", "/f/M.framework", 12.5), ["/r/a.pl", "/f/M.framework", "seek", "12500000"]);
    assert_eq!(seek_args("s", "f", 0.0)[3], "0");
    assert_eq!(seek_args("s", "f", -3.0)[3], "0", "never negative");
    assert_eq!(seek_args("s", "f", f64::NAN)[3], "0");
    assert_eq!(seek_args("s", "f", 200.0)[3], "200000000");
}
