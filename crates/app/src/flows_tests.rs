//! Round 3, flows group (child module of `tests`): editor save failures, late translations, settings writes, brew, recovery.
use super::*;

/// Make `dir` read-only (Unix) so the atomic `project.json` write fails; returns a guard-free undo.
fn set_writable(dir: &std::path::Path, writable: bool) {
    use std::os::unix::fs::PermissionsExt;
    let mode = if writable { 0o755 } else { 0o555 };
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(mode)).unwrap();
}

#[test]
fn leaving_the_editor_when_the_last_save_fails_keeps_the_unsaved_project() {
    let (mut app, _rx, dir) = editor_app_in("r3-s7");
    app.editor.as_mut().unwrap().st.autosave.changed(1.0); // an edit is owed to disk
    set_writable(&dir, false);
    app.pop(); // Esc
    app.editor_gc();
    // the session and its view stay, with the error shown
    assert!(app.editor.is_some(), "the in-memory project is not dropped");
    assert!(app.stack.iter().any(|v| matches!(v, View::Editor { .. })), "the editor view is kept: {:?}", app.stack.len());
    assert!(app.editor.as_ref().unwrap().st.autosave.pending(), "the edit is still owed");
    let toast = app.toast.as_ref().map(|t| t.0.clone()).unwrap_or_default();
    assert!(toast.contains("Could not save"), "the error is toasted: {toast:?}");
    // a second Esc is the explicit discard
    app.pop();
    app.editor_gc();
    assert!(app.editor.is_none(), "the second Esc discards");
    assert!(!app.stack.iter().any(|v| matches!(v, View::Editor { .. })));
    set_writable(&dir, true);
}

#[test]
fn leaving_the_editor_after_a_good_save_closes_it_as_before() {
    let (mut app, _rx, dir) = editor_app_in("r3-s7-ok");
    app.editor.as_mut().unwrap().st.autosave.changed(1.0);
    app.pop();
    app.editor_gc();
    assert!(app.editor.is_none());
    assert!(!app.stack.iter().any(|v| matches!(v, View::Editor { .. })));
    assert!(app.toast.is_none());
    std::fs::remove_dir_all(dir.parent().unwrap()).ok();
}

// ---- S8: a late, older translation -------------------------------------------------------------

fn translate_result(app: &mut App, run: u64, output: ferrocast_core::models::TranslateOutput) {
    app.settings.translate_output = output;
    app.bus.send(Msg::Translated {
        run,
        lang: "Spanish".into(),
        source_pid: Some(7),
        result: Ok("Hola".into()),
        ms: 5,
        target: ferrocast_core::translate::Target::Editable,
    });
    app.drain();
}

#[test]
fn a_late_older_translation_never_replaces_the_newer_selection_in_place() {
    use ferrocast_core::models::TranslateOutput as Out;
    let mut app = fixture();
    FX_FRONT.with(|f| f.set(Some(7)));
    app.translate_run = 2; // the user started a second translation
    fx_taken();
    translate_result(&mut app, 1, Out::Auto);
    assert!(!fx_taken().iter().any(|l| l.starts_with("ax ")), "the old run must not write into the selection");
    assert_shows_hola(&app); // shown (and on the clipboard) instead
                             // the current run still replaces in place
    let mut app = fixture();
    FX_FRONT.with(|f| f.set(Some(7)));
    app.translate_run = 2;
    fx_taken();
    translate_result(&mut app, 2, Out::Auto);
    assert!(fx_taken().iter().any(|l| l == "ax 7"), "the current run replaces in place");
}

// ---- S9: Settings edits and the colour picker are not persisted at frame rate --------------------

fn on_disk_settings(app: &App) -> Settings {
    app.store.load("settings.json")
}

#[test]
fn a_stream_of_settings_edits_writes_once_after_the_pause() {
    let mut app = fixture();
    app.persist("settings.json", &app.settings.clone()); // known starting file
    let before = on_disk_settings(&app).tiling_gap;
    let mut last = before;
    for gap in (1..=12).map(f64::from) {
        let mut s = app.settings.clone();
        s.tiling_gap = gap;
        last = gap;
        app.commit_settings_debounced(s);
    }
    assert_eq!(app.settings.tiling_gap, last, "the edit applies in memory at once");
    assert_eq!(app.shared.settings.read().unwrap().tiling_gap, last, "and for the worker threads");
    assert_eq!(on_disk_settings(&app).tiling_gap, before, "nothing was written while the edits went on");
    app.flush_pending_saves(Instant::now() + Duration::from_secs(5));
    assert_eq!(on_disk_settings(&app).tiling_gap, last, "the last value is written after the pause");
    std::fs::remove_dir_all(&app.store.root).ok();
}

#[test]
fn pending_settings_are_written_before_the_pause_is_over_only_when_forced_by_leaving() {
    let mut app = fixture();
    app.persist("settings.json", &app.settings.clone());
    let mut s = app.settings.clone();
    s.tiling_gap = 9.0;
    app.commit_settings_debounced(s);
    app.flush_pending_saves(Instant::now()); // too early: still waiting
    assert_ne!(on_disk_settings(&app).tiling_gap, 9.0, "not yet due");
    app.hide();
    assert_eq!(on_disk_settings(&app).tiling_gap, 9.0, "hiding the window writes what is owed");
    std::fs::remove_dir_all(&app.store.root).ok();
}

#[test]
fn a_settings_edit_rebuilds_the_index_and_resends_tiling_only_when_related_fields_change() {
    let mut app = fixture();
    app.settings.tiling_enabled = true;
    *app.shared.settings.write().unwrap() = app.settings.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    app.tiling = Some(ferrocast_macos::tiling::Handle::from_sender(tx));
    app.base.clear();
    let mut s = app.settings.clone();
    s.translate_effect = !s.translate_effect;
    app.commit_settings_debounced(s);
    assert!(app.base.is_empty(), "an unrelated setting does not rebuild the root index");
    let configs = |rx: &std::sync::mpsc::Receiver<ferrocast_macos::tiling::Cmd>| {
        rx.try_iter().filter(|c| matches!(c, ferrocast_macos::tiling::Cmd::Config(_))).count()
    };
    assert_eq!(configs(&rx), 0, "nor does it push the tiling config");
    let mut s = app.settings.clone();
    s.tiling_gap += 3.0;
    app.commit_settings_debounced(s);
    assert_eq!(configs(&rx), 1, "a tiling field does");
    let mut s = app.settings.clone();
    s.commands.entry("system:lock-screen".into()).or_default().disabled = true;
    app.commit_settings_debounced(s);
    assert!(!app.base.is_empty(), "disabling a command rebuilds the index");
    std::fs::remove_dir_all(&app.store.root).ok();
}

#[test]
fn dragging_the_theme_colour_picker_writes_themes_json_once() {
    let mut app = fixture();
    let t = ferrocast_core::themes::builtin(ferrocast_core::themes::ThemeBase::Dark);
    app.custom_themes = vec![ferrocast_core::themes::CustomTheme { id: "t1".into(), name: "Mine".into(), ..t }];
    app.settings.custom_theme = Some("Mine".into());
    app.save_themes();
    let disk = |app: &App| app.store.load::<ferrocast_core::themes::ThemeFile>("themes.json").themes[0].accent.clone();
    let before = disk(&app);
    let mut last = String::new();
    for n in 0..10u8 {
        let mut e = app.active_theme().unwrap().clone();
        e.accent = format!("#1122{:02X}", 0x30 + n);
        last = e.accent.clone();
        app.update_active_theme(e);
    }
    assert_eq!(app.active_theme().unwrap().accent, last, "repaints from memory");
    assert_eq!(disk(&app), before, "no write per frame");
    app.flush_pending_saves(Instant::now() + Duration::from_secs(5));
    assert_eq!(disk(&app), last);
    std::fs::remove_dir_all(&app.store.root).ok();
}
