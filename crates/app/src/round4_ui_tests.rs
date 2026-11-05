//! Bugs round 4, group T5: Settings, notes and i18n of the launcher UI (S44-S53).

use super::*;
use ferrocast_core::i18n::{lookup, with_language, Lang};

fn key_with(key: Key, modifiers: Modifiers) -> Event {
    Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }
}

/// Every string the app painted in one frame (labels and painter text alike).
fn painted(app: &mut App) -> Vec<String> {
    fn walk(shape: &egui::epaint::Shape, out: &mut Vec<String>) {
        match shape {
            egui::epaint::Shape::Text(t) => out.push(t.galley.text().to_string()),
            egui::epaint::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let input =
        egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(ui::WIDTH, ui::HEIGHT))), ..Default::default() };
    let ctx = app.ctx.clone();
    let mut output = ctx.run_ui(input, |ui| app.draw(ui));
    output.textures_delta.clear();
    let mut out = vec![];
    for s in &output.shapes {
        walk(&s.shape, &mut out);
    }
    out
}

/// Render a few frames and return what the last one painted.
fn painted_settled(app: &mut App) -> Vec<String> {
    for _ in 0..2 {
        painted(app);
    }
    painted(app)
}

fn row_titles(app: &App) -> Vec<String> {
    settings_cards(app).iter().flat_map(|c| c.rows.iter().map(|r| r.0.clone())).collect()
}

/// (row rect, text fields inside it, small buttons inside it) of the Settings row `title`, from a fresh frame.
fn row_parts_raw(app: &mut App, title: &str) -> (Rect, Vec<Rect>, Vec<Rect>) {
    app.ctx.data_mut(|d| {
        d.remove::<Vec<FormCard>>(Id::new("form-cards"));
        d.remove::<Vec<(String, Rect)>>(Id::new("field-count"));
        d.remove::<Vec<(String, Id, Rect, bool)>>(Id::new("ctl-hits"));
    });
    render(app, vec![]);
    let row = settings_cards(app)
        .iter()
        .flat_map(|c| c.rows.iter())
        .find(|r| r.0 == title)
        .unwrap_or_else(|| panic!("row {title:?} not drawn"))
        .1;
    let fields = drawn_fields(app).into_iter().filter(|(k, r)| k == "text" && row.contains_rect(*r)).map(|f| f.1).collect();
    let hits: Vec<(String, Id, Rect, bool)> = app.ctx.data(|d| d.get_temp(Id::new("ctl-hits"))).unwrap_or_default();
    let buttons = hits.into_iter().filter(|h| h.0 == "small_btn" && row.contains_rect(h.2)).map(|h| h.2).collect();
    (row, fields, buttons)
}

/// Like `row_parts_raw`, after scrolling the pane until the row is above the footer.
fn row_parts(app: &mut App, title: &str) -> (Rect, Vec<Rect>, Vec<Rect>) {
    let visible = |r: Rect| r.max.y < ui::HEIGHT - ui::FOOTER_H - 4.0;
    for _ in 0..40 {
        let parts = row_parts_raw(app, title);
        if visible(parts.0) {
            return parts;
        }
        let at = Pos2::new(500.0, 200.0);
        render(app, vec![Event::PointerMoved(at)]);
        render(
            app,
            vec![Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, -60.0),
                phase: egui::TouchPhase::Move,
                modifiers: Modifiers::NONE,
            }],
        );
        render(app, vec![]);
    }
    panic!("could not scroll {title:?} into view");
}

fn click_field_and_type(app: &mut App, field: Rect, text: &str) {
    click_at(app, field.center());
    assert!(app.ctx.egui_wants_keyboard_input(), "the click at {field:?} focused the field");
    render(app, vec![Event::Text(text.into())]);
}

fn writes(app: &App) -> Vec<String> {
    app.ctx.data(|d| d.get_temp(Id::new("test-keychain-writes"))).unwrap_or_default()
}

// ---- S44: 'Disabled commands' names the command, not its internal id

#[test]
fn the_disabled_commands_list_shows_the_command_title_not_its_id() {
    use ferrocast_macos::system::SystemCommand;
    let mut app = settings_app(9, egui::ThemePreference::Dark);
    let item = app.base.iter().find(|i| matches!(i.kind, Kind::System(SystemCommand::ShutDown))).cloned().unwrap();
    assert!(item.id.contains(':'), "an internal id: {}", item.id);
    app.settings.command_mut(&item.id).disabled = true;
    app.rebuild();
    assert!(app.base.iter().all(|i| i.id != item.id), "a disabled command leaves the launcher");
    settle(&mut app);
    let titles = row_titles(&app);
    assert!(titles.contains(&item.title), "the row is named {:?}: {titles:?}", item.title);
    assert!(!titles.contains(&item.id), "the raw id is not shown: {titles:?}");
    // the row carries its Enable button
    let (_, _, buttons) = row_parts(&mut app, &item.title);
    assert_eq!(buttons.len(), 1, "one Enable button");
}

#[test]
fn the_disabled_commands_title_follows_the_interface_language() {
    use ferrocast_macos::system::SystemCommand;
    with_language(Lang::Es, || {
        let mut app = settings_app(9, egui::ThemePreference::Dark);
        app.rebuild();
        let item = app.base.iter().find(|i| matches!(i.kind, Kind::System(SystemCommand::ShutDown))).cloned().unwrap();
        app.settings.command_mut(&item.id).disabled = true;
        app.rebuild();
        settle(&mut app);
        assert!(row_titles(&app).contains(&item.title), "{:?} in {:?}", item.title, row_titles(&app));
    });
}

#[test]
fn a_disabled_command_that_is_no_longer_known_keeps_a_row_named_by_its_id() {
    let mut app = settings_app(9, egui::ThemePreference::Dark);
    app.settings.command_mut("app:com.example.Gone").disabled = true;
    app.rebuild();
    settle(&mut app);
    assert!(row_titles(&app).iter().any(|t| t.contains("com.example.Gone")), "{:?}", row_titles(&app));
}

// ---- S45: Enter saves the AI key and renames a theme

fn ai_tab(provider: ferrocast_core::models::AiProvider) -> App {
    let mut app = settings_app(3, egui::ThemePreference::Dark);
    seed_model_lists(&mut app);
    app.settings.ai_provider = provider;
    app.ctx.data_mut(|d| d.insert_temp(Id::new("ai_has_key"), false));
    settle(&mut app);
    app
}

#[test]
fn enter_in_the_ai_key_field_saves_the_key_like_the_save_button() {
    use ferrocast_core::models::AiProvider as P;
    for provider in [P::Anthropic, P::OpenAiCompatible] {
        let mut app = ai_tab(provider);
        let (_, fields, _) = row_parts(&mut app, "API key");
        assert_eq!(fields.len(), 1, "{provider:?}: the key field");
        click_field_and_type(&mut app, fields[0], "sk-test-123");
        assert!(writes(&app).is_empty(), "typing alone stores nothing");
        render(&mut app, vec![key_event(Key::Enter)]);
        assert_eq!(writes(&app), vec!["sk-test-123".to_string()], "{provider:?}: Enter stored the key once");
        assert_eq!(toast_text(&app), "AI key saved to Keychain");
        assert!(app.key_scratch.is_empty(), "the typed key is not kept after a successful save");
        let _ = std::fs::remove_dir_all(&app.store.root);
    }
}

#[test]
fn enter_in_the_theme_name_field_renames_the_theme() {
    let mut app = fixture();
    seed_model_lists(&mut app);
    let t = ocean();
    app.custom_themes = vec![t.clone()];
    app.settings.custom_theme = Some(t.name.clone());
    app.stack = vec![View::Root, View::Settings];
    app.ctx.data_mut(|d| d.insert_temp(Id::new("settings_tab"), 0usize));
    settle(&mut app);
    // the theme editor's rows are not recorded as cards: its name field is the only 170 pt one
    app.ctx.data_mut(|d| d.remove::<Vec<(String, Rect)>>(Id::new("field-count")));
    render(&mut app, vec![]);
    let fields: Vec<Rect> =
        drawn_fields(&app).into_iter().filter(|(k, r)| k == "text" && (r.width() - 170.0).abs() < 1.0).map(|f| f.1).collect();
    assert_eq!(fields.len(), 1, "{:?}", drawn_fields(&app));
    click_at(&mut app, fields[0].center());
    // replace the text: select all, type the new name, Enter
    render_mods(&mut app, vec![key_with(Key::A, Modifiers::COMMAND)], Modifiers::COMMAND);
    render(&mut app, vec![Event::Text("Sea".into())]);
    assert_eq!(app.custom_themes[0].name, "Ocean", "typing alone renames nothing");
    render(&mut app, vec![key_event(Key::Enter)]);
    assert_eq!(app.custom_themes[0].name, "Sea", "Enter renamed it");
    assert_eq!(app.settings.custom_theme.as_deref(), Some("Sea"));
    let _ = std::fs::remove_dir_all(&app.store.root);
}

// ---- S46: /usr/bin/open never blocks the UI thread

#[test]
fn a_detached_process_does_not_block_the_caller_and_a_missing_program_reports_false() {
    let t = Instant::now();
    assert!(spawn_detached("/bin/sleep", &["2".into()]));
    assert!(t.elapsed() < Duration::from_millis(1000), "returned after {:?}", t.elapsed());
    assert!(!spawn_detached("/nonexistent/ferrocast-no-such-program", &[]));
}

#[test]
fn no_ui_thread_call_site_waits_for_open() {
    for f in ["draw.rs", "app.rs"] {
        let src = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(f)).unwrap();
        let code = &src[..src.find("\n#[cfg(test)]").unwrap_or(src.len())];
        for (n, line) in code.lines().enumerate() {
            assert!(
                !(line.contains("\"/usr/bin/open\"") && line.contains(".status()")),
                "{f}:{} waits for /usr/bin/open on the UI thread: {line}",
                n + 1
            );
        }
    }
}

// ---- S48: the tab tooltip names the real shortcut

#[test]
fn every_settings_tab_tooltip_shows_the_key_that_selects_it() {
    for (i, (name, _)) in SETTINGS_TABS.iter().enumerate() {
        let tip = settings_tab_tooltip(name, i);
        let key = SETTINGS_NUM_KEYS[i].name();
        assert_eq!(tip, format!("{}  ⌘{key}", tr(name)), "tab {i}");
    }
    assert_eq!(settings_tab_tooltip("Advanced", 9), "Advanced  ⌘0", "the tenth tab is ⌘0, not ⌘10");
    assert_eq!(settings_tab_tooltip("General", 0), "General  ⌘1");
}

// ---- S49: Reset this tab can be undone with the keyboard and the toast survives other toasts

fn reset_app() -> App {
    let mut app = fixture();
    app.settings = sd4_scrambled_quiet();
    app.stack = vec![View::Root, View::Settings];
    app.ctx.data_mut(|d| d.insert_temp(Id::new("settings_tab"), 1usize));
    assert!(app.reset_settings_tab(1));
    app
}

#[test]
fn command_z_in_settings_undoes_the_pending_reset() {
    let mut app = reset_app();
    let reset = app.settings.clone();
    let scrambled = sd4_scrambled_quiet();
    assert_ne!(scrambled.clipboard_retention, reset.clipboard_retention);
    for _ in 0..2 {
        render(&mut app, vec![]);
    }
    render_mods(&mut app, vec![key_with(Key::Z, Modifiers::COMMAND)], Modifiers::COMMAND);
    assert_eq!(app.settings.clipboard_retention, scrambled.clipboard_retention, "the tab's fields are back");
    assert_eq!(app.settings.clipboard_enabled, scrambled.clipboard_enabled);
    assert!(app.settings_undo.is_none());
    assert_eq!(toast_text(&app), "Previous settings restored");
    // a second ⌘Z has nothing left to undo
    let after = app.settings.clone();
    render_mods(&mut app, vec![key_with(Key::Z, Modifiers::COMMAND)], Modifiers::COMMAND);
    assert_eq!(app.settings, after);
    let _ = std::fs::remove_dir_all(&app.store.root);
}

#[test]
fn command_z_without_a_pending_reset_changes_nothing() {
    let mut app = fixture();
    app.stack = vec![View::Root, View::Settings];
    let before = app.settings.clone();
    render_mods(&mut app, vec![key_with(Key::Z, Modifiers::COMMAND)], Modifiers::COMMAND);
    assert_eq!(app.settings, before);
    assert_eq!(toast_text(&app), "");
}

#[test]
fn another_toast_does_not_remove_the_undo_while_it_is_pending() {
    let mut app = reset_app();
    app.toast("Could not change login item: denied");
    assert_eq!(toast_text(&app), "Tab reset · Undo", "the Undo toast stays");
    assert!(app.settings_undo.is_some(), "and so does the undo state");
    // the other message is not lost: it is shown when the Undo window closes
    app.toast = app.toast.take().map(|(t, _)| (t, Instant::now() - Duration::from_secs(9)));
    app.expire_toasts();
    assert!(app.settings_undo.is_none());
    assert_eq!(toast_text(&app), "Could not change login item: denied");
    let _ = std::fs::remove_dir_all(&app.store.root);
}

#[test]
fn undoing_still_works_after_another_toast_arrived_and_later_toasts_are_normal() {
    let mut app = reset_app();
    let scrambled = sd4_scrambled_quiet();
    app.toast("Synced");
    assert!(app.undo_settings_reset());
    assert_eq!(app.settings.clipboard_retention, scrambled.clipboard_retention);
    assert_eq!(toast_text(&app), "Previous settings restored");
    app.toast("Synced");
    assert_eq!(toast_text(&app), "Synced", "with no undo pending a toast shows at once");
    let _ = std::fs::remove_dir_all(&app.store.root);
}

// ---- S50: the Translate 'Languages' field follows changes made elsewhere

#[test]
fn the_languages_field_shows_a_synced_change_and_the_next_keystroke_extends_it() {
    let mut app = translate_tab_app();
    app.settings.translate_languages = vec!["English".into(), "Spanish".into()];
    settle(&mut app);
    let (_, fields, _) = row_parts(&mut app, "Languages");
    assert_eq!(fields.len(), 1);
    click_field_and_type(&mut app, fields[0], "X");
    assert_eq!(app.settings.translate_languages.last().map(String::as_str), Some("SpanishX"), "typing edits the list");
    // an iCloud merge / import replaces the list while the field is focused
    app.settings.translate_languages = vec!["German".into()];
    for _ in 0..2 {
        render(&mut app, vec![]);
    }
    render(&mut app, vec![Event::Text("Y".into())]);
    assert_eq!(app.settings.translate_languages, vec!["GermanY".to_string()], "the field showed the synced text, not the old one");
    let _ = std::fs::remove_dir_all(&app.store.root);
}

#[test]
fn typing_a_trailing_comma_keeps_it_while_nothing_changes_elsewhere() {
    let mut app = translate_tab_app();
    app.settings.translate_languages = vec!["English".into()];
    settle(&mut app);
    let (_, fields, _) = row_parts(&mut app, "Languages");
    click_field_and_type(&mut app, fields[0], ", ");
    for _ in 0..2 {
        render(&mut app, vec![]);
    }
    render(&mut app, vec![Event::Text("French".into())]);
    assert_eq!(app.settings.translate_languages, vec!["English".to_string(), "French".to_string()]);
    let _ = std::fs::remove_dir_all(&app.store.root);
}

// ---- S52: a failed Keychain write is not reported as a stored key

#[test]
fn a_failed_keychain_write_keeps_the_typed_key_and_does_not_claim_one_is_stored() {
    use ferrocast_core::models::AiProvider as P;
    let mut app = ai_tab(P::Anthropic);
    app.ctx.data_mut(|d| d.insert_temp(Id::new("test-keychain-ok"), false));
    app.key_scratch = "sk-ant-typed".into();
    let (_, _, buttons) = row_parts(&mut app, "API key");
    assert_eq!(buttons.len(), 1, "the Save button");
    click_at(&mut app, buttons[0].center());
    assert_eq!(writes(&app), vec!["sk-ant-typed".to_string()], "a write was attempted");
    assert_eq!(toast_text(&app), "Could not update Keychain");
    assert_eq!(app.key_scratch, "sk-ant-typed", "the pasted key is still there to retry");
    assert_ne!(app.ctx.data(|d| d.get_temp::<bool>(Id::new("ai_has_key"))), Some(true), "the row must not say a key is stored");
    let _ = std::fs::remove_dir_all(&app.store.root);
}

#[test]
fn a_successful_keychain_write_marks_the_key_stored_and_clears_the_field() {
    use ferrocast_core::models::AiProvider as P;
    let mut app = ai_tab(P::Anthropic);
    app.key_scratch = "sk-ant-typed".into();
    let (_, _, buttons) = row_parts(&mut app, "API key");
    click_at(&mut app, buttons[0].center());
    assert_eq!(toast_text(&app), "AI key saved to Keychain");
    assert!(app.key_scratch.is_empty());
    assert_eq!(app.ctx.data(|d| d.get_temp::<bool>(Id::new("ai_has_key"))), Some(true));
    // saving an empty field removes the key
    click_at(&mut app, buttons[0].center());
    assert_eq!(toast_text(&app), "AI key removed");
    assert_eq!(app.ctx.data(|d| d.get_temp::<bool>(Id::new("ai_has_key"))), Some(false));
    let _ = std::fs::remove_dir_all(&app.store.root);
}

// ---- S51: text built with format! follows the interface language

fn texts_in_ja(view: View) -> Vec<String> {
    with_language(Lang::Ja, || {
        let mut app = fixture();
        app.stack = vec![View::Root, view];
        let t = painted_settled(&mut app);
        let _ = std::fs::remove_dir_all(&app.store.root);
        t
    })
}

fn assert_none_contains(texts: &[String], english: &[&str]) {
    for e in english {
        assert!(!texts.iter().any(|t| t.contains(e)), "English {e:?} is on screen in ja: {texts:?}");
    }
}

fn en_ja(en: impl Fn() -> String) -> (String, String) {
    (with_language(Lang::En, &en), with_language(Lang::Ja, &en))
}

#[test]
fn the_metadata_window_reads_in_the_interface_language() {
    let view = View::EditMetadata {
        file: "/tmp/x.mp4".into(),
        kind: MetaKind::Media,
        original: None,
        working: vec![],
        remove_all: false,
        error: Some("boom".into()),
    };
    let t = texts_in_ja(view);
    assert_none_contains(&t, &["Could not read the metadata"]);
    assert!(t.iter().any(|s| s.contains("boom")), "the reason is still shown: {t:?}");
    // the summary of the edits
    use ferrocast_core::metaform::{Field, Group};
    let f = |k: &str, v: &str| Field { group: Group::File, key: k.into(), value: v.into(), editable: true };
    let original = vec![f("title", "a"), f("artist", "b")];
    let working = vec![f("title", "A"), f("artist", "B")];
    let view = View::EditMetadata {
        file: "/tmp/x.mp4".into(),
        kind: MetaKind::Media,
        original: Some(original),
        working,
        remove_all: false,
        error: None,
    };
    let t = texts_in_ja(view);
    assert_none_contains(&t, &["2 changes."]);
}

#[test]
fn the_audio_visualizer_picture_row_reads_in_the_interface_language() {
    let view = |image: Option<PathBuf>| View::VisualizeAudio {
        file: "/tmp/a.mp3".into(),
        orient: ferrocast_core::media::Canvas::Landscape,
        style: crate::media_tools::VisStyle::Still,
        image,
        picking: false,
    };
    assert_none_contains(&texts_in_ja(view(None)), &["none yet"]);
    assert_none_contains(&texts_in_ja(view(Some("/nonexistent/cover.png".into()))), &["not found"]);
}

#[test]
fn the_audio_trim_status_line_and_truncation_note_read_in_the_interface_language() {
    let trim = ferrocast_core::videoplan::Trim::new(10.0, 30.0).unwrap();
    let audio = crate::media_tools::AudioSamples { samples: vec![0; 8000], rate: 8000, duration: 10.0, truncated: true };
    let view = View::AudioTrim { file: "/tmp/a.mp3".into(), trim: Some(trim), audio: Some(Arc::new(audio)) };
    let t = texts_in_ja(view);
    assert_none_contains(&t, &["waveform covers", "start ", "end ", "clip "]);
}

#[test]
fn editor_status_texts_read_in_the_interface_language() {
    use crate::editor_panels as ep;
    // each text differs from English in ja and keeps its numbers
    type Case<'a> = (&'a str, Box<dyn Fn() -> String>, &'a [&'a str]);
    let cases: Vec<Case> = vec![
        ("cut_report 1", Box::new(|| ep::cut_report(1, 1.4)), &["1", "1.4"]),
        ("cut_report 3", Box::new(|| ep::cut_report(3, 2.5)), &["3", "2.5"]),
        ("face_found", Box::new(|| ep::face_found(3, 10).unwrap()), &["3", "10"]),
        ("map_face_error", Box::new(|| ep::map_face_error(&ferrocast_macos::capture::AnalyzeFaceError::Invalid("bad".into()))), &["bad"]),
        ("analyze progress", Box::new(|| ep::analyze_progress_text(false, 10.0, 0.5)), &["50"]),
        ("analyze cancelling", Box::new(|| ep::analyze_progress_text(true, 10.0, 0.5)), &[]),
        ("analyze starting", Box::new(|| ep::analyze_progress_text(false, 0.0, 0.0)), &[]),
        ("transcribe progress", Box::new(|| ep::transcribe_progress_text(false, 10.0, 0.25)), &["25"]),
        ("transcribe cancelling", Box::new(|| ep::transcribe_progress_text(true, 10.0, 0.25)), &[]),
        ("focus fixed", Box::new(|| ep::focus_note(false, Some((0.25, 0.5)))), &["25", "50"]),
        ("one word", Box::new(|| ep::words_selected(1, "0:01", "0:02")), &["1", "0:01", "0:02"]),
        ("many words", Box::new(|| ep::words_selected(4, "0:01", "0:09")), &["4", "0:01", "0:09"]),
    ];
    for (name, f, numbers) in cases {
        let (en, ja) = en_ja(&f);
        assert_ne!(en, ja, "{name}: the ja text is the English one: {ja:?}");
        for n in numbers {
            assert!(ja.contains(n), "{name}: {ja:?} lost {n:?}");
        }
    }
    // English is unchanged
    assert_eq!(with_language(Lang::En, || ep::cut_report(1, 1.4)), "Removed 1 cut · 1.4 s");
    assert_eq!(with_language(Lang::En, || ep::cut_report(2, 1.4)), "Removed 2 cuts · 1.4 s");
    assert_eq!(with_language(Lang::En, || ep::face_found(3, 10).unwrap()), "Found a face in 3 of 10 samples.");
    assert_eq!(with_language(Lang::En, || ep::analyze_progress_text(false, 10.0, 0.5)), "Analyzing… 50 %");
    assert_eq!(with_language(Lang::En, || ep::transcribe_progress_text(false, 10.0, 0.25)), "Transcribing… 25 %");
    assert_eq!(with_language(Lang::En, || ep::focus_note(false, Some((0.25, 0.5)))), "Focus at 25 %, 50 %.");
    assert_eq!(with_language(Lang::En, || ep::words_selected(1, "0:01", "0:02")), "1 word selected · 0:01–0:02");
    assert_eq!(with_language(Lang::En, || ep::words_selected(4, "0:01", "0:09")), "4 words selected · 0:01–0:09");
}

#[test]
fn no_user_facing_message_is_left_as_a_bare_format_in_the_editor_and_media_panels() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let ep = std::fs::read_to_string(dir.join("editor_panels.rs")).unwrap();
    let ep = &ep[..ep.find("\n#[cfg(test)]").unwrap_or(ep.len())];
    for needle in [
        "format!(\"Could not start the transcriber",
        "format!(\"The transcript could not be read",
        "format!(\"Could not start the face analysis",
        "format!(\"Could not save project.json",
        "format!(\"This camera recording cannot be analyzed",
        "format!(\"Found a face in",
        "format!(\"Removed {cuts}",
    ] {
        assert!(!ep.contains(needle), "editor_panels.rs still builds {needle} with format!");
    }
    let draw = std::fs::read_to_string(dir.join("draw.rs")).unwrap();
    let draw = &draw[..draw.find("\n#[cfg(test)]").unwrap_or(draw.len())];
    for needle in
        ["format!(\"Unsupported combination", "format!(\"Could not read the metadata", "format!(\"{n} changes.", "(not found)\", p.display"]
    {
        assert!(!draw.contains(needle), "draw.rs still builds {needle} with format!");
    }
}

// ---- S53: extension launch errors follow the language

const LAUNCH_EXT: &str = r#"{"name":"e","title":"Tools","preferences":[
    {"name":"token","type":"password","title":"API token","required":true}],
  "commands":[{"name":"x","title":"X","mode":"view"}]}"#;

fn launch_app() -> App {
    let mut app = fixture();
    app.settings.run_raycast_extensions = true;
    app.ext.test_set_manifests(vec![ext_manifest(LAUNCH_EXT)]);
    app
}

fn launch_error(app: &mut App, ext: &str, cmd: &str) -> String {
    app.ext_launch_with(ext, cmd, Default::default()).expect_err("the launch must fail")
}

fn node() -> ferrocast_core::extensions::node::NodeInfo {
    ferrocast_core::extensions::node::NodeInfo { path: "/nonexistent/ferrocast-node".into(), version: (22, 1, 0) }
}

#[test]
fn extension_launch_errors_are_in_english_by_default_and_translated_otherwise() {
    let dir = std::env::temp_dir().join(format!("fc-ext-launch-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut cases: Vec<(String, &str, String)> = vec![];
    for lang in [Lang::En, Lang::Es] {
        with_language(lang, || {
            let mut app = launch_app();
            let mut out = vec![];
            out.push(launch_error(&mut app, "nope", "x"));
            app.settings.raycast_extensions_disabled = vec!["e".into()];
            out.push(launch_error(&mut app, "e", "x"));
            app.settings.raycast_extensions_disabled.clear();
            out.push(launch_error(&mut app, "e", "missing"));
            out.push(launch_error(&mut app, "e", "x")); // no Node.js
            app.ext.test_set_node(Some(node()));
            out.push(launch_error(&mut app, "e", "x")); // the command file is not there
            let mut m = ext_manifest(LAUNCH_EXT);
            m.dir = dir.clone();
            std::fs::write(dir.join("x.js"), "// built").unwrap();
            app.ext.test_set_manifests(vec![m]);
            out.push(launch_error(&mut app, "e", "x")); // required preference missing
            let mut prefs = serde_json::Map::new();
            prefs.insert("token".into(), serde_json::json!("t"));
            ferrocast_core::extensions::storage::save_preferences(&app.store.root, "e", &prefs).unwrap();
            out.push(launch_error(&mut app, "e", "x")); // Node cannot start
            cases.push((format!("{lang:?}"), "", out.join("\n")));
            let _ = std::fs::remove_dir_all(&app.store.root);
        });
    }
    let en: Vec<String> = cases[0].2.split('\n').map(String::from).collect();
    let es: Vec<String> = cases[1].2.split('\n').map(String::from).collect();
    assert_eq!(en.len(), 7);
    assert_eq!(en[0], "unknown extension nope");
    assert_eq!(en[1], "Tools is turned off in Settings → Integrations");
    assert_eq!(en[2], "Tools has no command missing");
    assert_eq!(en[3], "Node.js 22 or newer is needed: install it or download it in Settings → Integrations");
    assert_eq!(en[4], "Tools is not built: x.js is missing");
    assert_eq!(en[5], "Required preferences are not set (API token): fill them in at Settings → Integrations");
    assert!(en[6].starts_with("Cannot start Node.js: "), "{}", en[6]);
    for (i, (e, s)) in en.iter().zip(&es).enumerate() {
        assert_ne!(e, s, "message {i} stays English in es: {s:?}");
    }
    assert!(es[1].contains("Tools"), "{}", es[1]);
    assert!(es[4].contains("x.js"), "{}", es[4]);
    assert!(es[5].contains("API token"), "{}", es[5]);
    for key in [
        "unknown extension {ext}",
        "{title} is turned off in Settings → Integrations",
        "{title} has no command {cmd}",
        "{title} is not built: {cmd}.js is missing",
        "Required preferences are not set ({names}): fill them in at Settings → Integrations",
        "Cannot start Node.js: {e}",
        "Cannot talk to Node.js: {e}",
        "Cannot write the extensions runtime: {e}",
        "{title} needs a value for: {names}",
        "Node.js 22 or newer is needed: install it or download it in Settings → Integrations",
    ] {
        for lang in [Lang::Es, Lang::De, Lang::Fr, Lang::PtBr, Lang::Ja, Lang::ZhHans] {
            assert!(lookup(key, lang).is_some(), "{key:?} has no {lang:?} text");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_node_still_sends_the_user_to_settings_in_any_language() {
    for lang in [Lang::En, Lang::Es] {
        with_language(lang, || {
            let mut app = launch_app();
            // an extension without required preferences, so the launch reaches the Node check
            app.ext.test_set_manifests(vec![ext_manifest(
                r#"{"name":"e","title":"Tools","commands":[{"name":"x","title":"X","mode":"view"}]}"#,
            )]);
            app.ext_launch("e", "x");
            assert!(matches!(app.view(), View::Settings), "{lang:?}: Settings opens");
            let _ = std::fs::remove_dir_all(&app.store.root);
        });
    }
}

#[test]
fn the_missing_preferences_notice_is_translated() {
    let notice = |lang| {
        with_language(lang, || {
            let mut app = launch_app();
            app.ext_launch("e", "x");
            let n = app.ext.prefs_edit().expect("the form opens").notice.clone();
            let _ = std::fs::remove_dir_all(&app.store.root);
            n
        })
    };
    assert_eq!(notice(Lang::En), "Tools needs a value for: API token");
    let es = notice(Lang::Es);
    assert_ne!(es, "Tools needs a value for: API token");
    assert!(es.contains("Tools") && es.contains("API token"), "{es}");
}
