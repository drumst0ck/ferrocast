//! Bugs round 3, group T1: hotkey recording, destructive confirmations and the custom-theme editor.

use super::*;

fn key_with(key: Key, modifiers: Modifiers) -> Event {
    Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }
}

fn recording(id: &str) -> App {
    let mut app = fixture();
    app.stack = vec![View::Root, View::Hotkey { id: id.into(), name: id.into(), err: String::new() }];
    app
}

fn record(app: &mut App, key: Key, m: Modifiers) {
    render_mods(app, vec![key_with(key, m)], m);
}

fn hotkey_err(app: &App) -> String {
    match app.stack.last() {
        Some(View::Hotkey { err, .. }) => err.clone(),
        _ => String::new(),
    }
}

fn cmd_hotkey(app: &App, id: &str) -> Option<String> {
    app.settings.commands.get(id).and_then(|c| c.hotkey.clone())
}

const SHIFT: Modifiers = Modifiers { shift: true, ..Modifiers::NONE };
const ALT: Modifiers = Modifiers { alt: true, ..Modifiers::NONE };

// ---- S1: Shift+letter is typing, not a global hotkey

#[test]
fn shift_plus_a_letter_is_not_accepted_as_a_global_hotkey() {
    for id in ["builtin:x", "__launcher"] {
        let mut app = recording(id);
        let before = (app.settings.launcher_hotkey.clone(), cmd_hotkey(&app, id));
        record(&mut app, Key::A, SHIFT);
        assert_eq!((app.settings.launcher_hotkey.clone(), cmd_hotkey(&app, id)), before, "{id}: nothing is saved");
        assert!(matches!(app.view(), View::Hotkey { .. }), "{id}: still recording");
        assert!(!hotkey_err(&app).is_empty(), "{id}: says why");
        assert!(!toast_text(&app).contains("Hotkey set"), "{id}: {}", toast_text(&app));
    }
}

#[test]
fn shift_with_a_real_modifier_or_an_f_key_is_still_accepted() {
    let mut app = recording("builtin:x");
    record(&mut app, Key::A, Modifiers { shift: true, alt: true, ..Modifiers::NONE });
    assert_eq!(cmd_hotkey(&app, "builtin:x").as_deref(), Some("Alt+Shift+KeyA"));
    let mut app = recording("builtin:y");
    record(&mut app, Key::F5, SHIFT);
    assert_eq!(cmd_hotkey(&app, "builtin:y").as_deref(), Some("Shift+F5"));
}

// ---- S2: a taken shortcut is an error, not a success toast

#[test]
fn a_shortcut_already_bound_is_refused_with_an_error_and_nothing_changes() {
    let mut app = recording("builtin:b");
    app.settings.command_mut("builtin:a").hotkey = Some("Alt+KeyK".into());
    record(&mut app, Key::K, ALT);
    assert_eq!(cmd_hotkey(&app, "builtin:b"), None, "B stays unbound");
    assert_eq!(cmd_hotkey(&app, "builtin:a").as_deref(), Some("Alt+KeyK"), "A keeps it");
    assert!(matches!(app.view(), View::Hotkey { .. }), "still recording so the user can pick another");
    assert!(!hotkey_err(&app).is_empty());
    assert!(!toast_text(&app).contains("Hotkey set"), "{}", toast_text(&app));
}

#[test]
fn the_launcher_shortcut_cannot_be_given_to_a_command_and_a_free_one_succeeds() {
    let mut app = recording("builtin:b");
    let launcher = app.settings.launcher_hotkey.clone();
    app.settings.launcher_hotkey = "Alt+KeyL".into();
    record(&mut app, Key::L, ALT);
    assert_eq!(cmd_hotkey(&app, "builtin:b"), None);
    assert_eq!(app.settings.launcher_hotkey, "Alt+KeyL");
    app.settings.launcher_hotkey = launcher;
    record(&mut app, Key::J, ALT);
    assert_eq!(cmd_hotkey(&app, "builtin:b").as_deref(), Some("Alt+KeyJ"));
    assert!(toast_text(&app).contains("Hotkey set"), "{}", toast_text(&app));
    assert!(!matches!(app.view(), View::Hotkey { .. }));
}

#[test]
fn a_system_registration_error_is_found_by_its_shortcut() {
    let errors = vec!["Alt+KeyK: already registered".to_string(), "Alt+KeyKK: other".to_string()];
    assert_eq!(draw::hotkey_failure(&errors, "Alt+KeyK").as_deref(), Some("already registered"));
    assert_eq!(draw::hotkey_failure(&errors, "Alt+KeyJ"), None);
    assert_eq!(draw::hotkey_failure(&[], "Alt+KeyK"), None);
}

// ---- S3: deleting a custom theme asks first

#[test]
fn deleting_a_custom_theme_needs_a_second_press() {
    let mut app = fixture();
    let t = ocean();
    app.custom_themes = vec![t.clone()];
    app.settings.custom_theme = Some(t.name.clone());
    assert!(!app.request_delete_theme(), "first press only asks");
    assert_eq!(app.custom_themes.len(), 1, "theme still there");
    assert_eq!(app.settings.custom_theme.as_deref(), Some(t.name.as_str()));
    assert!(toast_text(&app).contains(&t.name), "{}", toast_text(&app));
    assert!(!app.request_delete_theme(), "an immediate second press (double-click) does not delete");
    assert_eq!(app.custom_themes.len(), 1, "theme survives a double-click");
    let back = std::time::Instant::now() - std::time::Duration::from_secs(1);
    app.ctx.data_mut(|d| d.insert_temp(egui::Id::new("theme-delete-armed"), (t.id.clone(), back)));
    assert!(app.request_delete_theme(), "second press deletes");
    assert!(app.custom_themes.is_empty());
    assert_eq!(app.settings.custom_theme, None);
    assert!(!app.request_delete_theme(), "nothing selected: nothing to delete");
    let _ = std::fs::remove_dir_all(&app.store.root);
}

#[test]
fn the_delete_confirmation_belongs_to_one_theme_and_expires() {
    let mut app = fixture();
    let (mut a, mut b) = (ocean(), ocean());
    (a.id, b.id, b.name) = ("t-a".into(), "t-b".into(), "Sea".into());
    app.custom_themes = vec![a, b];
    app.settings.custom_theme = Some("Ocean".into());
    app.request_delete_theme();
    app.settings.custom_theme = Some("Sea".into());
    assert!(!app.request_delete_theme(), "arming one theme does not confirm another");
    assert_eq!(app.custom_themes.len(), 2);
    app.ctx.data_mut(|d| d.insert_temp(Id::new("theme-delete-armed"), ("t-b".to_string(), Instant::now() - Duration::from_secs(60))));
    assert!(!app.request_delete_theme(), "an old confirmation is not valid");
    assert_eq!(app.custom_themes.len(), 2);
    let _ = std::fs::remove_dir_all(&app.store.root);
}

// ---- S4: destructive actions ask first

#[test]
fn force_kill_clear_history_and_clear_shelf_ask_before_running() {
    let mut app = fixture();
    app.calc_history = vec![ferrocast_core::models::CalcEntry {
        question: "1+1".into(),
        display: "2".into(),
        raw: "2".into(),
        value: 2.0,
        unit: None,
        at: 0,
    }];
    app.shelf = vec![PathBuf::from("/tmp/ferrocast-shelf-a")];
    // an impossible pid: never a real process
    let pid = i32::MAX;
    for act in [Act::KillProcess(pid, true), Act::ClearCalcHistory, Act::ClearShelf] {
        let name = format!("{act:?}");
        app.stack = vec![View::Root];
        app.toast = None;
        app.perform(act.clone(), false);
        assert!(matches!(app.view(), View::Confirm { .. }), "{name}: asks first");
        assert!(app.toast.is_none(), "{name}: nothing ran");
    }
    assert_eq!(app.calc_history.len(), 1, "history untouched");
    assert_eq!(app.shelf.len(), 1, "shelf untouched");
    app.stack = vec![View::Root];
    app.perform(Act::ClearCalcHistory, true);
    app.perform(Act::ClearShelf, true);
    assert!(app.calc_history.is_empty() && app.shelf.is_empty(), "confirmed: they clear");
    let _ = std::fs::remove_dir_all(&app.store.root);
}

// ---- S5: system confirmation prompts are translated

#[test]
fn system_confirmation_prompts_follow_the_language() {
    use ferrocast_macos::system::SystemCommand;
    let mut app = fixture();
    let shut = app.base.iter().find(|i| matches!(i.kind, Kind::System(SystemCommand::ShutDown))).cloned().unwrap();
    let msg = ferrocast_core::i18n::with_language(ferrocast_core::i18n::Lang::De, || {
        app.run_item(&shut);
        match app.view() {
            View::Confirm { msg, .. } => msg.clone(),
            _ => panic!("no confirmation"),
        }
    });
    assert_eq!(msg, "Ausschalten?");
    assert_eq!(trf("{command}?", &[("command", "Restart")]), "Restart?");
    let _ = std::fs::remove_dir_all(&app.store.root);
}
