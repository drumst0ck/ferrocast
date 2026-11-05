//! Host side of the Raycast extension runtime: clipboard secrets, HUD / toast notifications and
//! LocalStorage (R2-S11, S12, S13, S15). Driven through `ext_call` / `ext_event` like the sidecar does.

use super::*;
use ferrocast_core::extensions::protocol::RuntimeMsg;
use ferrocast_core::extensions::sidecar::Event;
use ferrocast_core::extensions::storage;
use serde_json::{json, Value};

fn ext_app() -> App {
    let mut app = fixture();
    app.ext.test_install_session(1, false, Duration::ZERO);
    app
}

fn is_err_with(r: &ext_glue::Reply, needle: &str) -> bool {
    matches!(r, ext_glue::Reply::Now(Err(e)) if e.contains(needle))
}

#[test]
fn s11_a_concealed_copy_is_written_as_a_secret_and_never_as_plain_text() {
    let mut app = ext_app();
    notion_io::take_copied();
    ext_glue::take_concealed();
    let r = app.ext_call("e", 1, 1, "clipboard.copy", &json!({"text":"s3cret","concealed":true}));
    assert!(matches!(r, ext_glue::Reply::Now(Ok(Value::Null))));
    assert_eq!(ext_glue::take_concealed(), vec!["s3cret".to_string()]);
    assert!(notion_io::take_copied().is_empty(), "no plain item a clipboard manager would record");
}

#[test]
fn s11_a_normal_copy_stays_a_plain_write_and_only_true_means_concealed() {
    let mut app = ext_app();
    notion_io::take_copied();
    ext_glue::take_concealed();
    for params in [json!({"text":"a","concealed":false}), json!({"text":"b"}), json!({"text":"c","concealed":"yes"})] {
        assert!(matches!(app.ext_call("e", 1, 1, "clipboard.copy", &params), ext_glue::Reply::Now(Ok(_))));
    }
    assert_eq!(notion_io::take_copied(), vec!["a".to_string(), "b".into(), "c".into()]);
    assert!(ext_glue::take_concealed().is_empty());
}

#[test]
fn s12_a_hud_is_shown_as_a_notification_because_the_window_hides_at_once() {
    let mut app = ext_app();
    ext_glue::take_notified();
    app.visible = true;
    app.ext_event(1, Event::Msg(RuntimeMsg::Hud { title: "Copied to Clipboard".into() }));
    assert_eq!(ext_glue::take_notified(), vec!["Copied to Clipboard".to_string()]);
    assert!(!app.visible, "the launcher still hides");
    // a no-view command: the window is already hidden, the HUD must still be seen
    app.ext_event(1, Event::Msg(RuntimeMsg::Hud { title: "Done".into() }));
    assert_eq!(ext_glue::take_notified(), vec!["Done".to_string()]);
}

fn toast(app: &mut App, style: &str, title: &str, message: &str) {
    app.ext_event(1, Event::Msg(RuntimeMsg::Toast { style: style.into(), title: title.into(), message: message.into() }));
}

#[test]
fn s13_a_hidden_launcher_gets_no_notification_for_an_animated_toast() {
    let mut app = ext_app();
    ext_glue::take_notified();
    app.visible = false;
    toast(&mut app, "animated", "Loading", "");
    assert!(ext_glue::take_notified().is_empty(), "a progress toast is not worth a banner");
    toast(&mut app, "success", "Done", "all fine");
    toast(&mut app, "failure", "Oops", "");
    assert_eq!(ext_glue::take_notified(), vec!["Done: all fine".to_string(), "⚠ Oops".into()]);
}

#[test]
fn s13_a_visible_launcher_shows_the_toast_and_sends_no_notification() {
    let mut app = ext_app();
    ext_glue::take_notified();
    app.visible = true;
    toast(&mut app, "animated", "Loading", "");
    assert!(ext_glue::take_notified().is_empty());
    assert!(app.toast_user && app.toast.as_ref().is_some_and(|t| t.0 == "Loading"));
}

fn ls_file(app: &App, ext: &str) -> PathBuf {
    storage::extension_dir(&app.store.root, ext).join("localstorage.json")
}

#[test]
fn s15_local_storage_changes_are_applied_at_once_and_written_off_the_ui_thread() {
    let mut app = ext_app();
    let f = ls_file(&app, "e");
    // 20 unawaited writes are not capped by the in-flight limit that guards the slow host calls
    for i in 0..20 {
        let r = app.ext_call("e", 1, i, "localStorage.setItem", &json!({"key":format!("k{i}"),"value":i}));
        assert!(matches!(r, ext_glue::Reply::Now(Ok(Value::Null))));
    }
    // a read already sees the last change, straight from memory
    match app.ext_call("e", 1, 99, "localStorage.getItem", &json!({"key":"k19"})) {
        ext_glue::Reply::Now(Ok(v)) => assert_eq!(v, json!(19)),
        _ => panic!("reads are answered from memory"),
    }
    // and the file catches up on a background thread
    let read = || std::fs::read(&f).ok().and_then(|b| serde_json::from_slice::<serde_json::Map<String, Value>>(&b).ok());
    let start = Instant::now();
    while read().is_none_or(|m| m.len() < 20) && start.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(read().map(|m| m.len()), Some(20));
}

#[test]
fn s15_a_corrupt_local_storage_file_refuses_writes_and_is_kept() {
    let mut app = ext_app();
    let f = ls_file(&app, "e");
    std::fs::create_dir_all(f.parent().unwrap()).unwrap();
    std::fs::write(&f, b"{not json").unwrap();
    let r = app.ext_call("e", 1, 1, "localStorage.setItem", &json!({"key":"k","value":"v"}));
    assert!(is_err_with(&r, "unreadable"));
    assert_eq!(std::fs::read(&f).unwrap(), b"{not json", "the damaged file is still there to be recovered");
    // the other extensions are unaffected
    let r = app.ext_call("other", 1, 2, "localStorage.setItem", &json!({"key":"k","value":"v"}));
    assert!(matches!(r, ext_glue::Reply::Now(Ok(Value::Null))));
}
