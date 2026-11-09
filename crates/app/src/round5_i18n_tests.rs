//! Bugs round 5, i18n group (child module of `tests`): texts that reached the user in English
//! although the interface language was another one.
//!
//! The oracle is what a control does on display: `tr` of the text the code produced. A text that
//! is still English there either has no row (a miss is recorded) or was built with `format!`
//! instead of `trf`, so `tr` finds its template row only when the English text is unchanged.

use super::*;
use ferrocast_core::i18n::{take_misses, tr, with_language, Lang};

/// `produce()` in English and as shown in German; the German text must not be the English one.
fn shown_in_german(label: &str, mut produce: impl FnMut() -> String) -> String {
    let en = with_language(Lang::En, &mut produce);
    let de = with_language(Lang::De, || {
        let s = produce();
        // what a control does with it: a text already in German stays, an English one is looked up
        tr(&s).into_owned()
    });
    assert_ne!(de, en, "{label}: still English ({en:?})");
    de
}

fn de_misses(keys: &[&str]) -> Vec<String> {
    with_language(Lang::De, || {
        let _ = take_misses();
        for k in keys {
            let _ = tr(k);
        }
        take_misses()
    })
}

// ---- F3: the sync status line -----------------------------------------------------------------

#[test]
fn the_sync_status_line_follows_the_language() {
    let mut app = fixture();
    app.settings.sync_enabled = true;
    let ago = services::now_secs() - 300;
    for (last, peers, down) in [(Some(ago), 0, 0), (Some(ago), 1, 0), (Some(ago), 3, 1), (Some(ago), 2, 4), (None, 0, 2)] {
        app.sync.last_sync = last;
        app.sync.peers = peers;
        app.sync.downloading = down;
        let label = format!("sync status {last:?} {peers} {down}");
        shown_in_german(&label, || app.sync_status());
    }
    app.sync.last_sync = Some(ago);
    app.sync.downloading = 0;
    app.sync.restored.insert("notes");
    shown_in_german("sync status with a restored file", || app.sync_status());
}

#[test]
fn a_failed_sync_run_is_reported_in_the_language() {
    let mut app = fixture();
    app.settings.sync_enabled = true;
    let done = || crate::app::SyncDone { results: vec![("notes", vec![], Err("disk full".into()))], unavailable: None };
    let shown = shown_in_german("Sync failed", || {
        app.sync.in_flight = true;
        app.sync_done(done());
        app.sync_status()
    });
    assert!(shown.contains("disk full"), "{shown}");
}

// ---- F4: Notion relative times ----------------------------------------------------------------

#[test]
fn notion_relative_times_follow_the_language() {
    let now = chrono::DateTime::parse_from_rfc3339("2026-06-15T12:00:00Z").unwrap().with_timezone(&chrono::Utc);
    for iso in ["2026-06-15T11:59:30.000Z", "2026-06-15T11:30:00.000Z", "2026-06-15T09:00:00.000Z", "2026-06-14T11:00:00.000Z"] {
        shown_in_german(iso, || notion_io::relative_time(iso, now));
    }
}

// ---- F2: recording messages --------------------------------------------------------------------

#[test]
fn recording_messages_follow_the_language() {
    use crate::record_ctl::*;
    shown_in_german("saved", || saved_toast("Demo", 65.0, false, None, false));
    shown_in_german("low disk", || saved_toast("Demo", 65.0, false, None, true));
    shown_in_german("early", || saved_toast("Demo", 3.0, true, None, false));
    shown_in_german("early with reason", || saved_toast("Demo", 3.0, true, Some("display lost"), false));
    shown_in_german("camera", || warn_message("camera-lost", None).unwrap());
    shown_in_german("source", || warn_message("source-failed", Some("no space")).unwrap());
    shown_in_german("source bare", || warn_message("source-failed", None).unwrap());
    shown_in_german("not connected", || device_shown(&[], "usb-1", true));
    shown_in_german("failed", || match start_failure(&ferrocast_macos::capture::RecordError::Timeout) {
        StartFailure::Message(m) => m,
        other => panic!("{other:?}"),
    });
}

// ---- F5: script run messages -------------------------------------------------------------------

fn output_text(app: &App) -> String {
    app.stack
        .iter()
        .find_map(|v| match v {
            View::Output { text, .. } => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

fn script_output(code: i32) -> App {
    let mut app = fixture();
    app.runs.insert(5, RunKind::Output);
    app.stack = vec![View::Root, View::Output { run: 5, title: "T".into(), text: String::new(), done: false }];
    app.bus.send(Msg::Script { run: 5, text: "boom".into(), code });
    app.drain();
    app
}

#[test]
fn a_script_exit_code_note_follows_the_language_and_a_kill_shows_none() {
    let de = with_language(Lang::De, || output_text(&script_output(3)));
    assert!(de.starts_with("boom\n"), "{de}");
    assert!(!de.contains("exit code"), "the note is still English: {de}");
    assert!(de.contains('3'), "{de}");
    // a run stopped after its deadline reports -1 and already says why: no "[exit code -1]" on top
    let killed = output_text(&script_output(-1));
    assert_eq!(killed, "boom", "{killed}");
}

#[test]
fn a_script_that_cannot_start_says_so_in_the_language() {
    shown_in_german("Failed to run", || services::run_failed_text("No such file"));
}

// ---- S3: media tools line, trim buttons --------------------------------------------------------

#[test]
fn the_audio_to_video_size_line_follows_the_language() {
    use crate::media_tools::{visualize_line, VisStyle};
    use ferrocast_core::media::Canvas;
    shown_in_german("waveform", || visualize_line(Canvas::Landscape, VisStyle::Waveform));
    shown_in_german("still", || visualize_line(Canvas::Square, VisStyle::Still));
}

#[test]
fn the_trim_buttons_have_a_row_in_every_language() {
    use ferrocast_core::videoplan::TrimCmd;
    let mut keys: Vec<&str> = TrimCmd::ALL.iter().map(|c| c.label()).collect();
    keys.extend(["Cut here", "Remove last"]);
    let miss: Vec<String> = de_misses(&keys).into_iter().filter(|k| k.chars().filter(|c| c.is_alphabetic()).count() > 1).collect();
    assert!(miss.is_empty(), "{miss:?}");
    for c in TrimCmd::ALL {
        if c.label().chars().filter(|c| c.is_alphabetic()).count() > 1 {
            assert_ne!(with_language(Lang::De, || tr(c.label()).into_owned()), c.label());
        }
    }
}

// ---- U3: studio editor labels ------------------------------------------------------------------

#[test]
fn the_studio_editor_labels_have_a_row_in_every_language() {
    use crate::editor_panels::*;
    let mut keys: Vec<String> = look_chip_labels().iter().map(|s| s.to_string()).collect();
    keys.extend(SIDES.iter().map(|s| s.1.to_string()));
    keys.extend(WALLPAPERS.iter().map(|w| w.0.to_string()));
    keys.extend(LOCALES.iter().map(|l| l.1.to_string()));
    keys.extend(CURSOR_STYLES.iter().map(|c| c.1.to_string()));
    keys.extend(
        [
            Section::Zoom,
            Section::Loupe,
            Section::Cursor,
            Section::Style,
            Section::Clips,
            Section::Camera,
            Section::Audio,
            Section::Transcript,
        ]
        .map(|s| s.label().to_string()),
    );
    let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
    let miss: Vec<String> = de_misses(&refs).into_iter().filter(|k| k.chars().any(|c| c.is_alphabetic())).collect();
    assert!(miss.is_empty(), "{miss:?}");
    // the language picker draws through the same lookup
    assert_ne!(with_language(Lang::De, || locale_label(2)), LOCALES[2].1);
}

// ---- U4: theme rename and import errors --------------------------------------------------------

#[test]
fn theme_rename_and_import_errors_follow_the_language() {
    let mut app = fixture();
    let mut other = ocean();
    other.id = "t-sea".into();
    other.name = "Sea".into();
    app.custom_themes = vec![ocean(), other];
    app.settings.custom_theme = Some("Ocean".into());
    let shown = shown_in_german("duplicate name", || app.rename_active_theme("sea").unwrap_err());
    assert!(shown.contains("sea"), "{shown}");
    let missing = std::env::temp_dir().join(format!("ferrocast-r5-missing-{}.ferrocast-theme", std::process::id()));
    shown_in_german("unreadable file", || app.import_theme_file(&missing).unwrap_err());
    let bad = std::env::temp_dir().join(format!("ferrocast-r5-bad-{}.ferrocast-theme", std::process::id()));
    std::fs::write(&bad, "{\"hello\": 1}").unwrap();
    shown_in_german("not a theme", || app.import_theme_file(&bad).unwrap_err());
    let _ = std::fs::remove_file(&bad);
}

// ---- B6: titles the runtime writes itself ------------------------------------------------------

#[test]
fn titles_written_by_the_runtime_are_translated_and_extension_titles_are_not() {
    use ferrocast_core::extensions::protocol::RuntimeMsg;
    use ferrocast_core::extensions::sidecar::Event;
    let mut app = fixture();
    app.ext.test_install_session(1, false, Duration::ZERO);
    app.visible = false;
    ext_glue::take_notified();
    with_language(Lang::De, || {
        app.ext_event(
            1,
            Event::Msg(RuntimeMsg::Toast {
                style: "failure".into(),
                title: "Extension error".into(),
                message: "oops".into(),
                own_title: true,
            }),
        );
        app.ext_event(
            1,
            Event::Msg(RuntimeMsg::Toast { style: "success".into(), title: "Settings".into(), message: String::new(), own_title: false }),
        );
        app.visible = true;
        app.ext_event(1, Event::Msg(RuntimeMsg::Hud { title: "Copied to Clipboard".into(), own_title: true }));
        app.ext_event(1, Event::Msg(RuntimeMsg::Hud { title: "Settings".into(), own_title: false }));
    });
    let n = ext_glue::take_notified();
    assert_eq!(n.len(), 4, "{n:?}");
    assert!(!n[0].contains("Extension error") && n[0].starts_with('⚠') && n[0].ends_with(": oops"), "{n:?}");
    assert_eq!(n[1], "Settings", "an extension's own title is shown as written");
    assert_ne!(n[2], "Copied to Clipboard", "{n:?}");
    assert_eq!(n[3], "Settings");
}
