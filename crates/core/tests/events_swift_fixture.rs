//! `fixtures/events_swift.json` is written by the Swift `EventLog` (crates/macos/swift/capture/Events.swift)
//! through the scratch harness `target/capture-spike/events-spike/build.sh`. The Rust decoder
//! must accept the Swift output as is.

use ferrocast_core::capture::events::{Events, VERSION};

const SWIFT_OUTPUT: &str = include_str!("../fixtures/events_swift.json");

#[test]
fn swift_output_decodes_and_validates() {
    let e = Events::decode(SWIFT_OUTPUT).expect("decode");
    assert_eq!(e.version, VERSION);
    assert_eq!(e.screen.size, [3456, 2234]);
    assert_eq!(e.screen.display_frame, [0.0, 0.0, 1728.0, 1117.0]);
    assert!(e.validate().is_empty(), "{:?}", e.validate());
    assert_eq!(e.windows[0].app, "Sa\"fari\\é");
    assert_eq!(e.windows[0].id, 4231);
}

#[test]
fn swift_idle_pointer_is_collapsed_to_its_endpoints() {
    let e = Events::decode(SWIFT_OUTPUT).unwrap();
    // 5 moving samples + hold start + hold end + the move away
    assert_eq!(e.cursor.len(), 8);
    assert_eq!(e.cursor[5].t(), 0.0833);
    assert_eq!(e.cursor[6].t(), 0.9833);
    assert_eq!(e.cursor[7].3, "ibeam");
}

#[test]
fn swift_clicks_use_down_and_up_flags() {
    let e = Events::decode(SWIFT_OUTPUT).unwrap();
    assert_eq!(e.clicks.len(), 2);
    assert!(e.clicks[0].is_down() && !e.clicks[1].is_down());
}

#[test]
fn swift_keys_hold_shortcuts_only() {
    let e = Events::decode(SWIFT_OUTPUT).unwrap();
    // the harness fed a bare "p" and shift+"a": neither was stored
    assert_eq!(e.keys.len(), 3);
    assert!(e.keys.iter().all(|k| k.is_shortcut()));
    assert!(e.keys.iter().all(|k| k.code.is_some()));
    assert!(!SWIFT_OUTPUT.contains("\"char\":\"p\"") && !SWIFT_OUTPUT.contains("\"char\":\"a\""));
    assert_eq!(e.keys[1].mods, vec!["cmd", "shift"]);
}

#[test]
fn swift_extra_screen_field_is_tolerated_and_times_sorted() {
    assert!(SWIFT_OUTPUT.contains("capture_rect"));
    let e = Events::decode(SWIFT_OUTPUT).unwrap();
    assert_eq!(e.scroll.len(), 1, "the zero scroll is not stored");
    assert_eq!(e.duration(), 4.2);
}
