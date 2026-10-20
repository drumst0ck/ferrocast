use super::*;

fn fixture() -> App {
    let ctx = Context::default();
    let store = Store::new(std::env::temp_dir().join(format!("ferrocast-ui-test-{}", uuid::Uuid::new_v4())));
    let (tx, rx) = std::sync::mpsc::channel();
    let bus = Bus { tx, ctx: ctx.clone() };
    let settings = Settings::default();
    let shared = Arc::new(Shared {
        store: store.clone(),
        clipboard: Mutex::new(vec![]),
        snippets: RwLock::new(vec![]),
        settings: RwLock::new(settings.clone()),
        ignored_clipboard_count: Default::default(),
        visible: Default::default(),
        keywords: Default::default(),
    });
    let mut app = App {
        ctx,
        shared,
        bus,
        rx,
        store: store.clone(),
        settings,
        snippets: vec![],
        quicklinks: vec![],
        notes: vec![],
        frecency: Frecency::default(),
        apps: vec![],
        apps_at: Instant::now(),
        scripts: vec![],
        base: vec![],
        hotkeys: None,
        _tray: None,
        icons: IconCache::new(store.images_dir()),
        stack: vec![View::Root],
        query: String::new(),
        prev_query: String::new(),
        selected: 0,
        scroll_to_sel: false,
        panel: None,
        toast: None,
        prev_pid: None,
        visible: true,
        had_focus: false,
        shown_at: Instant::now(),
        hidden_at: Instant::now(),
        cur_height: ui::HEIGHT,
        files: (String::new(), vec![]),
        files_req: String::new(),
        query_changed_at: Instant::now(),
        win_cache: vec![],
        proc_cache: vec![],
        timers: vec![],
        calc_history: vec![],
        recent_emoji: vec![],
        shelf: vec![],
        app_icon: None,
        wide_font: false,
        show_t: None,
        nav_at: Instant::now(),
        claude: None,
        ai_models: Default::default(),
        tools: Default::default(),
        runs: HashMap::new(),
        next_run: 1,
        inline: HashMap::new(),
        focus_first: false,
        dark: true,
        started: true,
        act_item: None,
        scratch: String::new(),
        key_scratch: String::new(),
        rates: Default::default(),
        rates_checked_at: Instant::now(),
        sec_cache: None,
        sec_dirty: true,
    };
    app.rebuild();
    app
}

fn render(app: &mut App, events: Vec<Event>) {
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(ui::WIDTH, ui::HEIGHT))),
        events,
        ..Default::default()
    };
    let ctx = app.ctx.clone();
    let mut output = ctx.run_ui(input, |ui| app.draw(ui));
    output.textures_delta.clear();
}

#[test]
fn list_forms_and_settings_render_without_native_services() {
    let mut app = fixture();
    for view in [
        View::Root,
        View::Clipboard,
        View::Snippets,
        View::Quicklinks,
        View::Emoji,
        View::Files,
        View::Windows,
        View::Notes,
        View::Settings,
        View::SnippetForm { id: None, name: String::new(), text: String::new(), keyword: String::new() },
        View::QuicklinkForm { id: None, name: String::new(), link: String::new(), open_with: String::new() },
    ] {
        app.stack = vec![View::Root, view];
        render(&mut app, vec![]);
    }
}

#[test]
fn script_with_confirmation_does_not_start_before_approval() {
    let mut app = fixture();
    let source = "# @raycast.schemaVersion 1\n# @raycast.title Example\n# @raycast.mode fullOutput\n# @raycast.needsConfirmation true\n";
    app.scripts = vec![script::parse(std::path::Path::new("/nonexistent-ferrocast-test-script"), source).unwrap()];
    app.run_script_cmd(0, vec!["argument".into()]);
    assert!(matches!(app.view(), View::Confirm { .. }), "must ask before executing a confirmation-marked script");
    assert!(app.runs.is_empty(), "must not start a process yet");
}

#[test]
fn empty_snippet_library_can_create_with_keyboard() {
    let mut app = fixture();
    app.stack.push(View::Snippets);
    render(&mut app, vec![Event::Key { key: Key::N, physical_key: None, pressed: true, repeat: false, modifiers: CMD }]);
    assert!(matches!(app.view(), View::SnippetForm { id: None, .. }));
}

#[test]
fn calculator_is_first_root_result() {
    let mut app = fixture();
    app.query = "52% of 900".into();
    let results = app.root_sections();
    assert!(matches!(results[0].items[0].kind, Kind::Calc(_)));
    assert_eq!(results[0].items[0].title, "468");
}

#[test]
fn quicklink_alias_preserves_argument() {
    let mut app = fixture();
    let q = Quicklink::new("Google", "https://google.com/search?q={argument name=\"q\"}");
    app.settings.command_mut(&format!("quicklink:{}", q.id)).alias = Some("g".into());
    app.quicklinks.push(q);
    app.rebuild();
    app.query = "g rust macos".into();
    let results = app.root_sections();
    assert!(results.iter().flat_map(|s| &s.items).any(|i| matches!(&i.kind,
        Kind::Quicklink { arg: Some(arg), .. } if arg == "rust macos")));
}

#[test]
fn duplicate_snippet_clears_keyword() {
    let mut app = fixture();
    let s = Snippet::new("Signature", "hello", "!sig");
    let id = s.id.clone();
    app.snippets.push(s);
    app.perform(Act::DuplicateSnippet(id), false);
    assert_eq!(app.snippets.len(), 2);
    assert!(app.snippets[1].keyword.is_empty());
    let _ = std::fs::remove_dir_all(&app.store.root);
}

#[test]
fn currency_query_uses_cached_rates() {
    let mut app = fixture();
    app.rates = ferrocast_core::currency::Rates::parse(r#"{"base":"EUR","date":"d","rates":{"USD":1.25}}"#, 0).unwrap();
    app.query = "100 usd in eur".into();
    let results = app.root_sections();
    assert_eq!(results[0].items[0].title, "80 EUR");
}

#[test]
fn force_quit_asks_for_confirmation() {
    let mut app = fixture();
    app.perform(Act::ForceQuitApp(1), false);
    assert!(matches!(app.view(), View::Confirm { act: Act::ForceQuitApp(1), .. }), "must not kill the process before confirming");
}

#[test]
fn destructive_system_commands_confirm_on_every_entry_path() {
    let mut app = fixture();
    let item = app.base.iter().find(|i| matches!(i.kind, Kind::System(ferrocast_macos::system::SystemCommand::ShutDown))).cloned().unwrap();
    // run_item is the single path used by Enter, the action panel and global hotkeys.
    app.run_item(&item);
    assert!(matches!(app.view(), View::Confirm { act: Act::RunSystem(_), .. }));
}

#[test]
fn ai_flow_shows_busy_then_readable_error() {
    let mut app = fixture();
    app.settings.ai_base_url = "http://127.0.0.1:1/v1".into(); // closed port: deterministic failure
    app.run_builtin(Builtin::AskAi);
    if let View::Ai { prompt, .. } = app.stack.last_mut().unwrap() {
        *prompt = "hello".into();
    }
    app.send_ai();
    assert!(matches!(app.view(), View::Ai { busy: true, .. }));
    // a second Enter while busy must not start another request
    let started = app.next_run;
    app.send_ai();
    // send_ai consumes a run id even when ignored, but must leave the in-flight run untouched
    assert!(matches!(app.view(), View::Ai { busy: true, run, .. } if *run == started - 1));
    let msg = app.rx.recv_timeout(Duration::from_secs(20)).expect("ai result");
    let _ = app.bus.tx.send(msg);
    app.drain();
    match app.view() {
        View::Ai { busy: false, answer, .. } => assert!(answer.contains("⚠ Cannot reach"), "{answer}"),
        _ => panic!("expected AI view"),
    }
}

#[test]
fn empty_prompt_is_not_sent() {
    let mut app = fixture();
    app.run_builtin(Builtin::AskAi);
    app.send_ai();
    assert!(matches!(app.view(), View::Ai { busy: false, .. }));
}

#[test]
fn ai_turns_accumulate_as_history_and_failed_turns_do_not() {
    let mut app = fixture();
    app.run_builtin(Builtin::AskAi);
    let set_prompt = |app: &mut App, t: &str| {
        if let View::Ai { prompt, .. } = app.stack.last_mut().unwrap() {
            *prompt = t.into();
        }
    };
    // Simulate turn 1 streaming in, bypassing the network.
    if let View::Ai { asked, answer, busy, run, .. } = app.stack.last_mut().unwrap() {
        *asked = "q1".into();
        *answer = "a1".into();
        *busy = false;
        *run = 7;
    }
    let _ = app.bus.tx.send(Msg::AiChunk { run: 7, text: " more".into() });
    let _ = app.bus.tx.send(Msg::AiDone { run: 7, error: None });
    app.drain();
    assert!(matches!(app.view(), View::Ai { answer, busy: false, .. } if answer == "a1 more"));
    // Turn 2 starts: turn 1 moves into history.
    app.settings.ai_base_url = "http://127.0.0.1:1/v1".into();
    set_prompt(&mut app, "q2");
    app.send_ai();
    match app.view() {
        View::Ai { history, asked, prompt, busy: true, .. } => {
            assert_eq!(history, &[("q1".to_string(), "a1 more".to_string())]);
            assert_eq!((asked.as_str(), prompt.as_str()), ("q2", ""));
        }
        _ => panic!("expected busy AI view"),
    }
    // Chunks for a stale run id are ignored.
    let _ = app.bus.tx.send(Msg::AiChunk { run: 7, text: "STALE".into() });
    app.drain();
    assert!(matches!(app.view(), View::Ai { answer, .. } if !answer.contains("STALE")));
    // The unreachable server fails turn 2; a later turn must not carry the failed one as context.
    let done = app.rx.recv_timeout(Duration::from_secs(20)).expect("chunk or done");
    let _ = app.bus.tx.send(done);
    while !matches!(app.view(), View::Ai { busy: false, .. }) {
        app.drain();
        std::thread::sleep(Duration::from_millis(20));
    }
    set_prompt(&mut app, "q3");
    app.send_ai();
    assert!(matches!(app.view(), View::Ai { history, .. } if history.len() == 1), "failed turn must not become context");
}

#[test]
fn output_view_does_not_leave_confirm_underneath() {
    let mut app = fixture();
    let source = "# @raycast.schemaVersion 1\n# @raycast.title Example\n# @raycast.mode fullOutput\n# @raycast.needsConfirmation true\n";
    app.scripts = vec![script::parse(std::path::Path::new("/nonexistent-ferrocast-test-script"), source).unwrap()];
    app.run_script_cmd(0, vec!["argument".into()]);
    assert!(matches!(app.view(), View::Confirm { .. }));
    app.execute_script_cmd(0, vec!["argument".into()]);
    assert!(matches!(app.view(), View::Output { .. }));
    assert!(!app.stack.iter().any(|v| matches!(v, View::Confirm { .. } | View::Args { .. })));
}

#[test]
fn snippet_keyword_prefix_conflicts_are_rejected() {
    let mut app = fixture();
    app.snippets.push(Snippet::new("Signature", "hello", "!sig"));
    for kw in ["!s", "!sig", "!signature"] {
        app.stack = vec![View::Root, View::SnippetForm { id: None, name: "N".into(), text: "t".into(), keyword: kw.into() }];
        app.save_form();
        assert_eq!(app.snippets.len(), 1, "{kw} must be rejected");
        assert!(app.toast.is_some(), "{kw} must toast");
    }
    app.stack = vec![View::Root, View::SnippetForm { id: None, name: "N".into(), text: "t".into(), keyword: "!other".into() }];
    app.save_form();
    assert_eq!(app.snippets.len(), 2);
    let _ = std::fs::remove_dir_all(&app.store.root);
}

#[test]
fn push_and_pop_scroll_to_first_row() {
    let mut app = fixture();
    app.scroll_to_sel = false;
    app.push(View::Snippets);
    assert!(app.scroll_to_sel);
    app.scroll_to_sel = false;
    app.pop();
    assert!(app.scroll_to_sel);
}

#[test]
fn shelf_adds_once_lists_and_removes() {
    let mut app = fixture();
    let f = std::env::temp_dir().join(format!("fc-shelf-{}.txt", std::process::id()));
    std::fs::write(&f, "x").unwrap();
    app.perform(Act::AddToShelf(vec![f.clone()]), false);
    app.perform(Act::AddToShelf(vec![f.clone()]), false);
    assert_eq!(app.shelf, vec![f.clone()], "no duplicates");
    app.stack = vec![View::Root, View::Shelf];
    app.invalidate_sections();
    let items: Vec<String> = app.sections_cached().into_iter().flat_map(|s| s.items).map(|i| i.title).collect();
    assert_eq!(items, vec![f.file_name().unwrap().to_string_lossy().to_string()]);
    render(&mut app, vec![]);
    app.perform(Act::RemoveFromShelf(f.clone()), false);
    assert!(app.shelf.is_empty());
    assert!(f.exists(), "removing from the shelf never touches the file");
    let _ = std::fs::remove_file(&f);
}

#[test]
fn typed_image_commands_with_parameters_are_listed_in_convert() {
    let mut app = fixture();
    let f = std::env::temp_dir().join(format!("fc-typed-{}.png", std::process::id()));
    std::fs::write(&f, "x").unwrap();
    for (query, title) in [
        ("frame pad 80 radius 24 shadow 30 bg 222222", "Add a background"),
        ("edit brightness 20 grain 30", "Edit photo"),
        ("redact 10,10 200x100", "Redact photo"),
        ("collage row 1600 gap 16 radius 12 bg 000000", "Collage"),
    ] {
        app.stack = vec![View::Root, View::Convert(vec![f.clone()])];
        app.query = query.to_string();
        app.invalidate_sections();
        let titles: Vec<String> = app.sections_cached().into_iter().flat_map(|s| s.items).map(|i| i.title).collect();
        assert!(titles.iter().any(|t| t.starts_with(title)), "{query:?} should offer {title:?}, got {titles:?}");
    }
    let _ = std::fs::remove_file(&f);
}

#[test]
fn pdf_organizer_view_renders_and_enter_without_edits_changes_nothing() {
    let mut app = fixture();
    let f = std::env::temp_dir().join(format!("fc-org-{}.pdf", std::process::id()));
    std::fs::write(&f, "not a pdf").unwrap();
    // a missing/invalid PDF is refused with a toast, no view is pushed
    let depth = app.stack.len();
    app.perform(Act::OrganizePdf(f.clone()), false);
    assert_eq!(app.stack.len(), depth, "an unreadable PDF opens no organizer");
    // the view itself renders and an unchanged plan is not written
    app.stack = vec![View::Root, View::PdfOrganize { file: f.clone(), n: 3, plan: ferrocast_core::pdfplan::PdfPlan::new(3) }];
    render(&mut app, vec![]);
    assert!(matches!(app.view(), View::PdfOrganize { .. }));
    let _ = std::fs::remove_file(&f);
}

#[test]
fn convert_view_offers_the_editors_for_one_video_pdf_and_image() {
    let mut app = fixture();
    let dir = std::env::temp_dir().join(format!("fc-editors-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let titles = |app: &mut App, f: &str| -> Vec<String> {
        let p = dir.join(f);
        std::fs::write(&p, b"x").unwrap();
        app.stack = vec![View::Root, View::Convert(vec![p])];
        app.sections().iter().flat_map(|s| s.items.iter().filter(|i| matches!(i.kind, Kind::Editor(_))).map(|i| i.title.clone())).collect()
    };
    let v = titles(&mut app, "a.mp4");
    for t in ["Trim Video…", "Crop Video…", "Change Speed…", "Video Snapshots…", "Redact Video…"] {
        assert!(v.iter().any(|x| x == t), "{t} missing from {v:?}");
    }
    assert!(titles(&mut app, "a.pdf").iter().any(|x| x == "Organize Pages…"));
    assert!(titles(&mut app, "a.png").iter().any(|x| x == "Crop…"));
    assert!(titles(&mut app, "a.txt").is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn wheel_renders_for_any_segment_count_in_both_themes() {
    use ferrocast_core::wheel::WheelKind;
    let mut app = fixture();
    let files = vec![std::path::PathBuf::from("/nonexistent/a.png")];
    for theme in [egui::ThemePreference::Dark, egui::ThemePreference::Light] {
        app.ctx.set_theme(theme);
        for kind in [WheelKind::Convert, WheelKind::Tools, WheelKind::Actions] {
            for n in [0usize, 1, 8, 9, 12] {
                let segs: Vec<WheelSeg> = (0..n)
                    .map(|i| WheelSeg { label: format!("Option {i}"), symbol: "sf:photo", needs: None, pick: WheelPick::Act(Act::Run) })
                    .collect();
                for sel in [None, n.checked_sub(1)] {
                    app.stack = vec![
                        View::Root,
                        View::Wheel { files: files.clone(), center: Pos2::new(-9999.0, -9999.0), segs: segs.clone(), kind, sel },
                    ];
                    render(&mut app, vec![]);
                }
            }
        }
    }
}

#[test]
fn actions_wheel_has_exactly_the_six_actions_in_order() {
    use ferrocast_core::wheel::WheelKind;
    let app = fixture();
    let one = vec![std::path::PathBuf::from("/nonexistent/a.png")];
    let many = vec![one[0].clone(), std::path::PathBuf::from("/nonexistent/b.png")];
    let folder = vec![std::env::temp_dir()];
    for files in [one, many, folder] {
        let labels: Vec<String> = app.wheel_segments(WheelKind::Actions, &files).into_iter().map(|s| s.label).collect();
        assert_eq!(labels, ["AirDrop", "Float", "Copy", "Move to…", "Copy Path", "Terminal"], "{files:?}");
    }
    assert!(app.wheel_segments(WheelKind::Actions, &[]).is_empty());
}

#[test]
fn convert_wheel_shows_every_output_and_marks_missing_tools() {
    use ferrocast_core::wheel::WheelKind;
    let mut app = fixture();
    app.tools = Default::default(); // no ffmpeg, LibreOffice, cwebp or unrar
    let dir = std::env::temp_dir().join(format!("fc-cw-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mk = |name: &str| {
        let p = dir.join(name);
        std::fs::write(&p, b"x").unwrap();
        vec![p]
    };
    // a PDF has nine outputs: none may be dropped
    let pdf = app.wheel_segments(WheelKind::Convert, &mk("a.pdf"));
    assert_eq!(pdf.len(), 9, "{:?}", pdf.iter().map(|s| s.label.clone()).collect::<Vec<_>>());
    // without ffmpeg every video output is still on the ring, marked as needing it
    let mp4 = app.wheel_segments(WheelKind::Convert, &mk("a.mp4"));
    assert!(mp4.len() >= 7, "{}", mp4.len());
    // MOV is converted with a built-in macOS tool; every other video output needs ffmpeg
    assert!(mp4.iter().filter(|s| s.label != "MOV").all(|s| s.needs.is_some_and(|n| n.contains("ffmpeg"))));
    assert!(mp4.iter().any(|s| s.label == "MOV" && s.needs.is_none()));
    // an image to PNG needs nothing
    let png = app.wheel_segments(WheelKind::Convert, &mk("a.jpg"));
    assert!(png.iter().any(|s| s.label.contains("PNG") && s.needs.is_none()));
    // symbols follow the output, not the input family
    let sym = |segs: &[WheelSeg], label: &str| segs.iter().find(|s| s.label.contains(label)).map(|s| s.symbol);
    assert_ne!(sym(&pdf, "Word"), sym(&pdf, "Markdown"));
    assert_eq!(sym(&mp4, "GIF"), Some("sf:livephoto"));
    let _ = std::fs::remove_dir_all(&dir);
}

fn render_mods(app: &mut App, mut events: Vec<Event>, modifiers: Modifiers) {
    // egui 0.36 takes the held modifiers from a ModifiersChanged event, not from RawInput
    events.insert(0, Event::ModifiersChanged(modifiers));
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(ui::WIDTH, ui::HEIGHT))),
        events,
        ..Default::default()
    };
    let ctx = app.ctx.clone();
    let mut output = ctx.run_ui(input, |ui| app.draw(ui));
    output.textures_delta.clear();
}

fn key_event(key: Key) -> Event {
    Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }
}

fn open_wheel(app: &mut App, kind: ferrocast_core::wheel::WheelKind, n_files: usize) {
    let files: Vec<std::path::PathBuf> = (0..n_files).map(|i| std::path::PathBuf::from(format!("/nonexistent/f{i}.png"))).collect();
    let segs = app.wheel_segments(kind, &files);
    app.stack = vec![View::Root, View::Wheel { files, center: Pos2::new(-9999.0, -9999.0), segs, kind, sel: None }];
}

fn wheel_state(app: &App) -> Option<(ferrocast_core::wheel::WheelKind, Option<usize>, usize)> {
    match app.view() {
        View::Wheel { kind, sel, segs, .. } => Some((*kind, *sel, segs.len())),
        _ => None,
    }
}

// Characterisation tests of the wheel's key handling (the behaviour already existed; no RED).
#[test]
fn wheel_arrows_step_and_wrap_around() {
    use ferrocast_core::wheel::WheelKind;
    let mut app = fixture();
    open_wheel(&mut app, WheelKind::Convert, 1);
    let n = wheel_state(&app).unwrap().2;
    assert!(n >= 3);
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::ArrowRight)]);
    assert_eq!(wheel_state(&app).unwrap().1, Some(0));
    render(&mut app, vec![key_event(Key::ArrowRight)]);
    assert_eq!(wheel_state(&app).unwrap().1, Some(1));
    render(&mut app, vec![key_event(Key::ArrowLeft)]);
    render(&mut app, vec![key_event(Key::ArrowLeft)]);
    assert_eq!(wheel_state(&app).unwrap().1, Some(n - 1), "left from the first segment wraps to the last");
    render(&mut app, vec![key_event(Key::ArrowDown)]);
    assert_eq!(wheel_state(&app).unwrap().1, Some(0), "down steps forward like right");
}

#[test]
fn wheel_escape_cancels_and_returns_to_the_root() {
    use ferrocast_core::wheel::WheelKind;
    let mut app = fixture();
    open_wheel(&mut app, WheelKind::Tools, 1);
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(wheel_state(&app).is_none(), "wheel must be gone");
    assert!(matches!(app.view(), View::Root));
}

#[test]
fn tapping_shift_command_or_control_switches_the_open_wheel() {
    use ferrocast_core::wheel::WheelKind;
    let mut app = fixture();
    open_wheel(&mut app, WheelKind::Convert, 1);
    // first frame with no key held records the idle state; the very first frame never switches
    render_mods(&mut app, vec![], Modifiers::NONE);
    assert_eq!(wheel_state(&app).unwrap().0, WheelKind::Convert);
    render_mods(&mut app, vec![], Modifiers::COMMAND);
    assert_eq!(wheel_state(&app).unwrap().0, WheelKind::Tools, "Command opens the tools wheel");
    render_mods(&mut app, vec![], Modifiers::NONE);
    render_mods(&mut app, vec![], Modifiers::CTRL);
    assert_eq!(wheel_state(&app).unwrap().0, WheelKind::Actions, "Control opens the actions wheel");
    assert_eq!(wheel_state(&app).unwrap().2, 6);
    render_mods(&mut app, vec![], Modifiers::NONE);
    render_mods(&mut app, vec![], Modifiers::SHIFT);
    assert_eq!(wheel_state(&app).unwrap().0, WheelKind::Convert, "Shift goes back to conversions");
    // holding the key does not switch again, and a selection is cleared only by a real switch
    render(&mut app, vec![key_event(Key::ArrowRight)]);
    let sel = wheel_state(&app).unwrap().1;
    assert_eq!(sel, Some(0));
    render_mods(&mut app, vec![], Modifiers::SHIFT);
    assert_eq!(wheel_state(&app).unwrap().1, sel, "a held Shift keeps the selection");
}

fn key_mod_event(key: Key, modifiers: Modifiers) -> Event {
    Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }
}

fn organizer_plan(app: &App) -> ferrocast_core::pdfplan::PdfPlan {
    match app.view() {
        View::PdfOrganize { plan, .. } => plan.clone(),
        other => panic!("organizer closed unexpectedly: {}", other.title()),
    }
}

// Characterisation tests of the organizer's keys (behaviour already existed; no RED).
#[test]
fn pdf_organizer_keys_edit_the_plan() {
    let mut app = fixture();
    let f = std::env::temp_dir().join(format!("fc-orgkeys-{}.pdf", std::process::id()));
    app.stack = vec![View::Root, View::PdfOrganize { file: f, n: 4, plan: ferrocast_core::pdfplan::PdfPlan::new(4) }];
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::ArrowRight)]);
    assert_eq!(organizer_plan(&app).cursor, 1);
    render(&mut app, vec![key_event(Key::R)]);
    assert_eq!(organizer_plan(&app).pages[1].turns, 1, "R turns the page clockwise");
    render(&mut app, vec![key_mod_event(Key::R, Modifiers::SHIFT)]);
    assert_eq!(organizer_plan(&app).pages[1].turns, 0, "Shift+R turns it back");
    render(&mut app, vec![key_event(Key::D)]);
    let p = organizer_plan(&app);
    assert_eq!(p.pages.len(), 5, "D duplicates the page");
    assert_eq!(p.pages[1].src, p.pages[2].src);
    render(&mut app, vec![key_event(Key::Delete)]);
    assert_eq!(organizer_plan(&app).pages.len(), 4, "Delete removes the page at the cursor");
    let before: Vec<usize> = organizer_plan(&app).pages.iter().map(|p| p.src).collect();
    render(&mut app, vec![key_mod_event(Key::ArrowRight, Modifiers::COMMAND)]);
    let after: Vec<usize> = organizer_plan(&app).pages.iter().map(|p| p.src).collect();
    assert_ne!(before, after, "Cmd+Right moves the page");
    let mut a = after.clone();
    let mut b = before.clone();
    a.sort();
    b.sort();
    assert_eq!(a, b, "moving never loses or invents a page");
}

#[test]
fn pdf_organizer_enter_without_edits_stays_open_and_starts_nothing() {
    let mut app = fixture();
    let f = std::env::temp_dir().join(format!("fc-orgenter-{}.pdf", std::process::id()));
    app.stack = vec![View::Root, View::PdfOrganize { file: f, n: 3, plan: ferrocast_core::pdfplan::PdfPlan::new(3) }];
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::PdfOrganize { .. }), "an unchanged plan is not written and the view stays");
    assert!(app.rx.try_recv().is_err(), "no job may start");
}

#[test]
fn pdf_organizer_escape_goes_back_without_a_job() {
    let mut app = fixture();
    let f = std::env::temp_dir().join(format!("fc-orgesc-{}.pdf", std::process::id()));
    app.stack = vec![View::Root, View::PdfOrganize { file: f, n: 3, plan: ferrocast_core::pdfplan::PdfPlan::new(3) }];
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::D)]);
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::Root), "Escape leaves the organizer");
    assert!(app.rx.try_recv().is_err(), "Escape starts no job even after edits");
}

fn crop_app() -> App {
    let mut app = fixture();
    let f = std::env::temp_dir().join(format!("fc-cropfocus-{}.png", uuid::Uuid::new_v4()));
    let rect = ferrocast_core::croprect::Rect { x: 0, y: 0, w: 100, h: 100 };
    app.stack = vec![View::Root, View::CropImage { file: f, size: (1000, 600), rect, aspect: None }];
    render(&mut app, vec![]);
    app
}

/// Click the W box of the crop view (position taken from the layout; guarded by an assertion).
fn click_crop_width_box(app: &mut App) {
    let pos = Pos2::new(300.0, 89.0);
    let btn = |pressed| Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    render(app, vec![Event::PointerMoved(pos)]);
    render(app, vec![btn(true)]);
    render(app, vec![btn(false)]);
    render(app, vec![]);
}

// Regression test for L41 B1 (Enter and Escape were taken while a number box was being edited).
#[test]
fn crop_number_box_keeps_enter_and_escape_while_editing() {
    let mut app = crop_app();
    click_crop_width_box(&mut app);
    assert!(app.ctx.egui_wants_keyboard_input(), "the click must have started editing the W box (layout moved?)");
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::CropImage { .. }), "Escape ends the edit, it must not close the crop view");
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::Root), "a second Escape, with nothing being edited, closes the view");
    let mut app = crop_app();
    click_crop_width_box(&mut app);
    assert!(app.ctx.egui_wants_keyboard_input());
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::CropImage { .. }), "Enter confirms the number, it must not save");
    assert!(app.rx.try_recv().is_err(), "no crop job may start while editing");
}

#[test]
fn crop_enter_without_a_focused_field_saves_and_leaves() {
    let mut app = crop_app();
    assert!(!app.ctx.egui_wants_keyboard_input());
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::Root), "Enter on the crop view starts the crop and goes back");
}

fn pointer_events(from: Pos2, to: Pos2) -> Vec<Vec<Event>> {
    let btn = |pos, pressed| Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    let steps = 6;
    let mut frames = vec![vec![Event::PointerMoved(from)], vec![btn(from, true)]];
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        frames.push(vec![Event::PointerMoved(from + (to - from) * t)]);
    }
    frames.push(vec![btn(to, false)]);
    frames
}

fn drag(app: &mut App, from: Pos2, to: Pos2) {
    for f in pointer_events(from, to) {
        render(app, f);
    }
    render(app, vec![]);
}

fn crop_rect_of(app: &App) -> ferrocast_core::croprect::Rect {
    match app.view() {
        View::CropImage { rect, .. } => *rect,
        other => panic!("crop view closed unexpectedly: {}", other.title()),
    }
}

// A-T2: pointer drags through render(); positions follow the layout (header 58 pt, footer 38 pt)
// and the assertions are relative, so a small layout change does not break them.
#[test]
fn dragging_inside_the_crop_rectangle_moves_it_without_resizing() {
    let mut app = fixture();
    let rect = ferrocast_core::croprect::Rect { x: 100, y: 100, w: 300, h: 200 };
    let f = std::env::temp_dir().join("fc-dragmove.png");
    app.stack = vec![View::Root, View::CropImage { file: f, size: (1000, 600), rect, aspect: None }];
    render(&mut app, vec![]);
    drag(&mut app, Pos2::new(250.0, 204.0), Pos2::new(310.0, 234.0));
    let after = crop_rect_of(&app);
    assert_eq!((after.w, after.h), (300, 200), "moving keeps the size");
    assert!(after.x > 100 + 40 && after.y > 100 + 20, "the rectangle followed the pointer: {after:?}");
}

#[test]
fn clicking_the_trim_bar_seeks_proportionally() {
    let mut app = fixture();
    let f = std::env::temp_dir().join("fc-trimbar.mp4");
    let trim = ferrocast_core::videoplan::Trim::new(10.0, 25.0);
    app.stack = vec![View::Root, View::VideoTrim { file: f, trim }];
    render(&mut app, vec![]);
    let pos = Pos2::new(375.0, 358.0);
    let btn = |pressed| Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    render(&mut app, vec![Event::PointerMoved(pos)]);
    render(&mut app, vec![btn(true)]);
    render(&mut app, vec![btn(false)]);
    render(&mut app, vec![]);
    match app.view() {
        View::VideoTrim { trim: Some(t), .. } => assert!((t.cursor - 5.0).abs() < 0.8, "cursor {} should be near the middle", t.cursor),
        _ => panic!("trim view closed"),
    }
}

/// A hand-built 58-byte RAR5 holding in.txt ("inside rar"); the same bytes as the matrix test.
const SYNTHETIC_RAR: [u8; 58] = [
    0x52, 0x61, 0x72, 0x21, 0x1a, 0x07, 0x01, 0x00, 0xc5, 0x1a, 0x33, 0x32, 0x03, 0x01, 0x00, 0x00, 0xdb, 0x87, 0x37, 0x92, 0x13, 0x02,
    0x02, 0x0a, 0x04, 0x0a, 0x00, 0x38, 0x6e, 0x35, 0x91, 0x00, 0x00, 0x06, 0x69, 0x6e, 0x2e, 0x74, 0x78, 0x74, 0x69, 0x6e, 0x73, 0x69,
    0x64, 0x65, 0x20, 0x72, 0x61, 0x72, 0x19, 0xb2, 0x3a, 0x35, 0x03, 0x05, 0x00, 0x00,
];

#[test]
fn rar_extracts_through_tar_when_unrar_is_not_installed() {
    let d = tmp_dir("rar-tar");
    let rar = d.join("pack.rar");
    std::fs::write(&rar, SYNTHETIC_RAR).unwrap();
    let no_unrar = ferrocast_core::convert::Tools::default();
    let out = crate::route::extract_archive(&rar, &no_unrar).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(out, d.join("pack"));
    assert_eq!(std::fs::read_to_string(out.join("in.txt")).unwrap(), "inside rar");
    assert_eq!(std::fs::read(&rar).unwrap(), SYNTHETIC_RAR, "archive untouched");
    // garbage with a .rar name still fails cleanly: no unrar to fall back on, folder removed
    let bad = d.join("bad.rar");
    std::fs::write(&bad, b"not a rar").unwrap();
    let err = crate::route::extract_archive(&bad, &no_unrar).unwrap_err();
    assert!(err.contains("unrar"), "{err}");
    assert!(!d.join("bad").exists(), "no leftover folder");
}

// L74-style regression test for the split window: Escape while the Equal parts box is edited.
#[test]
fn split_equal_parts_box_keeps_escape_while_editing() {
    let mut app = fixture();
    let f = std::env::temp_dir().join("fc-splitfocus.mp4");
    let trim = ferrocast_core::videoplan::Trim::new(10.0, 25.0);
    app.stack = vec![View::Root, View::VideoSplit { file: f, trim, points: vec![], parts: 2 }];
    render(&mut app, vec![]);
    let pos = Pos2::new(181.0, 374.0);
    let btn = |pressed| Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    render(&mut app, vec![Event::PointerMoved(pos)]);
    render(&mut app, vec![btn(true)]);
    render(&mut app, vec![btn(false)]);
    render(&mut app, vec![]);
    assert!(app.ctx.egui_wants_keyboard_input(), "the click must have started editing the Equal parts box (layout moved?)");
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::VideoSplit { .. }), "Escape ends the edit, it must not close the window");
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::Root), "a second Escape, with nothing being edited, closes it");
}

#[test]
fn wheel_log_line_names_the_kind_and_segments_but_no_paths() {
    use ferrocast_core::wheel::WheelKind;
    let seg = |l: &str| WheelSeg { label: l.into(), symbol: "sf:photo", needs: None, pick: WheelPick::Act(Act::Run) };
    let segs = vec![seg("AirDrop"), seg("Float"), seg("Copy"), seg("Move to…")];
    let line = wheel_log_line("open", WheelKind::Actions, &segs, 2);
    assert_eq!(line, "wheel open Actions segs=4 files=2 first=AirDrop,Float,Copy");
    assert_eq!(wheel_log_line("switch", WheelKind::Convert, &[], 0), "wheel switch Convert segs=0 files=0 first=");
    assert!(!line.contains('/'), "no paths in the log");
}

#[test]
fn video_speed_view_renders_while_probing_and_after() {
    let mut app = fixture();
    let f = std::env::temp_dir().join(format!("fc-vspeed-{}.mp4", std::process::id()));
    app.stack = vec![View::Root, View::VideoSpeed { file: f.clone(), duration: None, factor: 2.0 }];
    render(&mut app, vec![]);
    app.stack = vec![View::Root, View::VideoSpeed { file: f, duration: Some(8.0), factor: 0.5 }];
    render(&mut app, vec![]);
    assert!(matches!(app.view(), View::VideoSpeed { duration: Some(_), .. }));
}

#[test]
fn video_crop_view_renders_while_probing_and_after() {
    let mut app = fixture();
    let f = std::env::temp_dir().join(format!("fc-vcrop-{}.mp4", std::process::id()));
    app.stack = vec![
        View::Root,
        View::VideoCrop {
            file: f.clone(),
            size: None,
            rect: ferrocast_core::croprect::full(2, 2),
            aspect: None,
            duration: 0.0,
            ref_t: 0.0,
        },
    ];
    render(&mut app, vec![]);
    let r = ferrocast_core::croprect::Rect { x: 10, y: 10, w: 100, h: 60 };
    app.stack = vec![View::Root, View::VideoCrop { file: f, size: Some((640, 360)), rect: r, aspect: None, duration: 6.0, ref_t: 0.6 }];
    render(&mut app, vec![]);
    assert!(matches!(app.view(), View::VideoCrop { size: Some(_), .. }));
}

#[test]
fn video_trim_view_renders_while_probing_and_after() {
    let mut app = fixture();
    let f = std::env::temp_dir().join(format!("fc-vtrim-{}.mp4", std::process::id()));
    app.stack = vec![View::Root, View::VideoTrim { file: f.clone(), trim: None }];
    render(&mut app, vec![]);
    let t = ferrocast_core::videoplan::Trim::new(12.0, 25.0);
    assert!(t.is_some());
    app.stack = vec![View::Root, View::VideoTrim { file: f, trim: t }];
    render(&mut app, vec![]);
    assert!(matches!(app.view(), View::VideoTrim { trim: Some(_), .. }));
}

#[test]
fn collage_view_renders_and_needs_two_images() {
    let mut app = fixture();
    let a = std::env::temp_dir().join(format!("fc-collview-a-{}.png", std::process::id()));
    std::fs::write(&a, "x").unwrap();
    let depth = app.stack.len();
    app.perform(Act::CollageImages(vec![a.clone()]), false);
    assert_eq!(app.stack.len(), depth, "one picture is not a collage");
    app.stack = vec![View::Root, View::CollageImage { files: vec![a.clone(), a.clone()], opts: Default::default(), color: 0 }];
    render(&mut app, vec![]);
    assert!(matches!(app.view(), View::CollageImage { .. }));
    let _ = std::fs::remove_file(&a);
}

#[test]
fn frame_view_renders_and_refuses_unreadable_images() {
    let mut app = fixture();
    let f = std::env::temp_dir().join(format!("fc-frameview-{}.png", std::process::id()));
    std::fs::write(&f, "not an image").unwrap();
    let depth = app.stack.len();
    app.perform(Act::FrameImage(f.clone()), false);
    assert_eq!(app.stack.len(), depth, "an unreadable image opens no editor");
    app.stack = vec![View::Root, View::FrameImage { file: f.clone(), size: (100, 80), opts: Default::default(), color: 4 }];
    render(&mut app, vec![]);
    assert!(matches!(app.view(), View::FrameImage { .. }));
    let _ = std::fs::remove_file(&f);
}

#[test]
fn redact_view_renders_and_refuses_unreadable_images() {
    let mut app = fixture();
    let f = std::env::temp_dir().join(format!("fc-redview-{}.png", std::process::id()));
    std::fs::write(&f, "not an image").unwrap();
    let depth = app.stack.len();
    app.perform(Act::RedactImage(f.clone()), false);
    assert_eq!(app.stack.len(), depth, "an unreadable image opens no editor");
    let r = ferrocast_core::croprect::Rect { x: 5, y: 5, w: 20, h: 20 };
    app.stack =
        vec![View::Root, View::RedactImage { file: f.clone(), size: (100, 80), areas: vec![r], sel: Some(0), blur: true, color: 5 }];
    render(&mut app, vec![]);
    assert!(matches!(app.view(), View::RedactImage { .. }));
    let _ = std::fs::remove_file(&f);
}

#[test]
fn photo_editor_view_renders_and_refuses_unreadable_images() {
    let mut app = fixture();
    let f = std::env::temp_dir().join(format!("fc-editview-{}.png", std::process::id()));
    std::fs::write(&f, "not an image").unwrap();
    let depth = app.stack.len();
    app.perform(Act::EditPhoto(f.clone()), false);
    assert_eq!(app.stack.len(), depth, "an unreadable image opens no editor");
    app.stack = vec![View::Root, View::EditPhoto { file: f.clone(), size: (100, 80), opts: Default::default() }];
    render(&mut app, vec![]);
    assert!(matches!(app.view(), View::EditPhoto { .. }));
    let _ = std::fs::remove_file(&f);
}

#[test]
fn annotate_view_renders_and_empty_drawing_is_not_saved() {
    let mut app = fixture();
    let f = std::env::temp_dir().join(format!("fc-annot-{}.png", std::process::id()));
    std::fs::write(&f, "not an image").unwrap();
    let depth = app.stack.len();
    app.perform(Act::AnnotateCanvas(f.clone()), false);
    assert_eq!(app.stack.len(), depth, "an unreadable image opens no canvas");
    app.stack = vec![
        View::Root,
        View::Annotate {
            file: f.clone(),
            size: (100, 80),
            doc: Default::default(),
            tool: ferrocast_core::annotate::Tool::Arrow,
            color: 0,
            sel: None,
        },
    ];
    render(&mut app, vec![]);
    assert!(matches!(app.view(), View::Annotate { .. }));
    let _ = std::fs::remove_file(&f);
}

#[test]
fn snapshot_view_steps_without_changing_trim_markers() {
    let mut app = fixture();
    let file = std::path::PathBuf::from("/nonexistent-snapshot.mp4");
    let t = ferrocast_core::videoplan::Trim::new(8.0, 25.0).unwrap();
    app.stack = vec![View::Root, View::VideoSnapshot { file, trim: Some(t) }];
    render(&mut app, vec![]);
    let key = |key| Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
    render(&mut app, vec![key(Key::ArrowRight)]);
    match app.view() {
        View::VideoSnapshot { trim: Some(after), .. } => {
            assert!((after.cursor - 0.04).abs() < 1e-8);
            assert_eq!((after.start, after.end), (t.start, t.end));
        }
        _ => panic!("snapshot viewer closed unexpectedly"),
    }
    render(&mut app, vec![key(Key::I), key(Key::O)]);
    match app.view() {
        View::VideoSnapshot { trim: Some(after), .. } => assert_eq!((after.start, after.end), (t.start, t.end)),
        _ => panic!("snapshot viewer closed unexpectedly"),
    }
}

#[test]
fn trim_view_still_steps_and_sets_start_marker() {
    let mut app = fixture();
    app.stack = vec![
        View::Root,
        View::VideoTrim { file: std::path::PathBuf::from("/nonexistent-trim.mp4"), trim: ferrocast_core::videoplan::Trim::new(8.0, 25.0) },
    ];
    render(&mut app, vec![]);
    let key = |key| Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
    render(&mut app, vec![key(Key::ArrowRight)]);
    render(&mut app, vec![key(Key::I)]);
    match app.view() {
        View::VideoTrim { trim: Some(t), .. } => {
            assert!((t.cursor - 0.04).abs() < 1e-8);
            assert!((t.start - 0.04).abs() < 1e-8);
            assert_eq!(t.end, 8.0);
        }
        _ => panic!("trim viewer closed unexpectedly"),
    }
}

#[test]
fn video_redact_rejects_reversed_range_without_closing() {
    let mut app = fixture();
    app.stack = vec![
        View::Root,
        redact_view(
            "/nonexistent-redact.mp4",
            8.0,
            vec![Area { rect: ferrocast_core::croprect::full(640, 360), start: 5.0, end: 2.0, blur: false }],
            0,
        ),
    ];
    render(&mut app, vec![]);
    render(&mut app, vec![Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }]);
    assert!(matches!(app.view(), View::VideoRedact { .. }));
    assert!(app.rx.try_recv().is_err(), "invalid range must not start work");
}

fn tools_wheel_titles(app: &App, names: &[&str]) -> Vec<(String, Option<&'static str>)> {
    let files: Vec<std::path::PathBuf> = names.iter().map(|n| std::path::PathBuf::from(format!("/nonexistent/{n}"))).collect();
    app.wheel_segments(ferrocast_core::wheel::WheelKind::Tools, &files).into_iter().map(|s| (s.label, s.needs)).collect()
}

#[test]
fn tools_wheel_follows_tools_for_per_format() {
    let app = fixture();
    let labels = |names: &[&str]| -> Vec<String> { tools_wheel_titles(&app, names).into_iter().map(|(l, _)| l).collect() };
    assert_eq!(labels(&["a.mp4"]), ["Compress", "Metadata", "Mute", "Trim", "Crop Video", "Speed", "Snapshots", "Split", "Redact Video"]);
    assert_eq!(labels(&["a.png"]), ["Compress", "Metadata", "Edit Photo", "Annotate", "Background", "Crop", "Redact"]);
    assert_eq!(labels(&["a.png", "b.jpeg"]), ["Compress", "Metadata", "Crop", "Create PDF", "Collage"]);
    assert_eq!(labels(&["a.pdf"]), ["Compress", "Metadata", "Split PDF", "Organize"]);
    assert_eq!(labels(&["a.gif"]), ["Metadata"]);
    // an archive's only tool is Extract, now implemented: no "Not available yet" mark
    assert_eq!(tools_wheel_titles(&app, &["a.zip"]), [("Extract".to_string(), None)]);
    // the generic mapping still marks a tool whose implemented() is Missing (synthetic value)
    let zip = vec![std::path::PathBuf::from("/nonexistent/a.zip")];
    assert_eq!(tool_segment_with(ferrocast_core::tools::ToolId::ExtractArchive, &zip, true).needs, Some("Not available yet"));
    assert_eq!(tool_segment_with(ferrocast_core::tools::ToolId::ExtractArchive, &zip, false).needs, None);
    // no tools for text: the single fallback segment, not marked missing
    assert_eq!(tools_wheel_titles(&app, &["a.txt"]), [("Open tools…".to_string(), None)]);
    // mixed families fall back too
    assert_eq!(labels(&["a.png", "b.mp4"]), ["Open tools…"]);
}

#[test]
fn tool_reach_maps_tools_to_windows_and_presets() {
    use ferrocast_core::convert::{FileTool, ImageOp};
    use ferrocast_core::tools::ToolId;
    let p = std::path::PathBuf::from("/nonexistent/a.png");
    assert!(matches!(tool_reach(ToolId::AnnotateImage, std::slice::from_ref(&p)), ToolReach::Act(Act::AnnotateCanvas(_))));
    assert!(matches!(tool_reach(ToolId::EditImage, std::slice::from_ref(&p)), ToolReach::Act(Act::EditPhoto(_))));
    assert!(matches!(tool_reach(ToolId::Compress, std::slice::from_ref(&p)), ToolReach::Act(Act::CompressImages(_))));
    assert!(matches!(tool_reach(ToolId::RemoveMetadata, std::slice::from_ref(&p)), ToolReach::Image(ImageOp::StripMetadata)));
    let two = vec![p.clone(), std::path::PathBuf::from("/nonexistent/b.png")];
    assert!(matches!(tool_reach(ToolId::CropImage, &two), ToolReach::Image(ImageOp::Crop(1, 1))));
    assert!(matches!(tool_reach(ToolId::CreatePdf, &two), ToolReach::Image(ImageOp::CombinePdf)));
    assert!(matches!(tool_reach(ToolId::CreateCollage, &two), ToolReach::Act(Act::CollageImages(_))));
    let v = std::path::PathBuf::from("/nonexistent/a.mp4");
    assert!(matches!(tool_reach(ToolId::RedactAudio, std::slice::from_ref(&v)), ToolReach::Open));
    assert!(matches!(tool_reach(ToolId::MuteVideo, std::slice::from_ref(&v)), ToolReach::File(FileTool::Mute)));
    assert!(matches!(tool_reach(ToolId::TrimVideo, std::slice::from_ref(&v)), ToolReach::Act(Act::TrimVideo(_))));
    assert!(matches!(tool_reach(ToolId::ExtractArchive, &[]), ToolReach::Open));
    let z = std::path::PathBuf::from("/nonexistent/a.zip");
    assert!(matches!(tool_reach(ToolId::ExtractArchive, std::slice::from_ref(&z)), ToolReach::File(FileTool::Extract)));
    assert!(matches!(tool_reach(ToolId::ExtractArchive, &[z.clone(), z]), ToolReach::File(FileTool::Extract)));
}

#[test]
fn nine_segment_tools_wheel_renders_in_both_themes() {
    use ferrocast_core::wheel::WheelKind;
    let mut app = fixture();
    let files = vec![std::path::PathBuf::from("/nonexistent/a.mp4")];
    let segs = app.wheel_segments(WheelKind::Tools, &files);
    assert_eq!(segs.len(), 9);
    for theme in [egui::ThemePreference::Dark, egui::ThemePreference::Light] {
        app.ctx.set_theme(theme);
        for sel in [None, Some(0), Some(8)] {
            app.stack = vec![
                View::Root,
                View::Wheel { files: files.clone(), center: Pos2::new(-9999.0, -9999.0), segs: segs.clone(), kind: WheelKind::Tools, sel },
            ];
            render(&mut app, vec![]);
        }
    }
}

#[test]
fn convert_view_lists_a_missing_tool_instead_of_hiding_it() {
    let mut app = fixture();
    let zip = vec![std::path::PathBuf::from("/nonexistent/a.zip")];
    // the generic mechanism, with a synthetic "everything is missing" predicate
    let sec = app.tools_section_where(&zip, "", "sub", |_| true).expect("a Tools section");
    assert!(sec.items.iter().any(|i| i.title == "Extract" && i.accessory == "Not available yet"));
    // real table: nothing is Missing any more, so no format gets such a row
    let mut titles = |f: &str| -> Vec<(String, String)> {
        app.stack = vec![View::Root, View::Convert(vec![std::path::PathBuf::from(format!("/nonexistent/{f}"))])];
        app.invalidate_sections();
        app.sections_cached().into_iter().flat_map(|s| s.items).map(|i| (i.title, i.accessory)).collect()
    };
    for f in ["a.zip", "a.mp4"] {
        assert!(!titles(f).iter().any(|(_, a)| a == "Not available yet"), "{f}");
    }
}

#[test]
fn convert_view_offers_extract_for_an_archive() {
    let mut app = fixture();
    let dir = tmp_dir("offer");
    let z = dir.join("a.zip");
    std::fs::write(&z, b"x").unwrap();
    app.stack = vec![View::Root, View::Convert(vec![z.clone()])];
    app.invalidate_sections();
    let items: Vec<_> = app.sections_cached().into_iter().flat_map(|s| s.items).collect();
    let row = items.iter().find(|i| i.title == "Extract").expect("Extract row for an archive");
    assert!(matches!(&row.kind, Kind::FileTool(_, ferrocast_core::convert::FileTool::Extract)));
    // not offered for a non-archive
    let t = dir.join("a.txt");
    std::fs::write(&t, b"x").unwrap();
    app.stack = vec![View::Root, View::Convert(vec![t])];
    app.invalidate_sections();
    assert!(!app.sections_cached().into_iter().flat_map(|s| s.items).any(|i| i.title == "Extract"));
    let _ = std::fs::remove_dir_all(&dir);
}

// ------------------------------------------------------------ extractArchive (A14)

fn tmp_dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("ferrocast-extract-{tag}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn sh(cwd: &std::path::Path, prog: &str, args: &[&str]) {
    let st = std::process::Command::new(prog).args(args).current_dir(cwd).output().unwrap();
    assert!(st.status.success(), "{prog} {args:?}: {}", String::from_utf8_lossy(&st.stderr));
}

fn names(d: &std::path::Path) -> Vec<String> {
    let mut v: Vec<String> =
        std::fs::read_dir(d).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).collect();
    v.sort();
    v
}

fn extract(f: &std::path::Path) -> Result<std::path::PathBuf, String> {
    crate::route::extract_archive(f, &ferrocast_core::convert::Tools::detect())
}

/// A folder `src/` with `a.txt` and `sub/b.txt`, in a fresh temp dir.
fn tree(tag: &str) -> std::path::PathBuf {
    let d = tmp_dir(tag);
    std::fs::create_dir_all(d.join("src/sub")).unwrap();
    std::fs::write(d.join("src/a.txt"), "alpha").unwrap();
    std::fs::write(d.join("src/sub/b.txt"), "beta").unwrap();
    d
}

fn check_tree(out: &std::path::Path) {
    assert_eq!(std::fs::read_to_string(out.join("a.txt")).unwrap(), "alpha");
    assert_eq!(std::fs::read_to_string(out.join("sub/b.txt")).unwrap(), "beta");
}

#[test]
fn extract_zip_tar_and_targz_make_a_named_sibling_folder() {
    let d = tree("kinds");
    sh(&d.join("src"), "/usr/bin/zip", &["-qr", "../photos.zip", "."]);
    sh(&d.join("src"), "/usr/bin/tar", &["-cf", "../pack.tar", "."]);
    sh(&d.join("src"), "/usr/bin/tar", &["-czf", "../deep.tar.gz", "."]);
    sh(&d.join("src"), "/usr/bin/tar", &["-czf", "../short.tgz", "."]);
    let before = std::fs::read(d.join("photos.zip")).unwrap();
    for (archive, folder) in [("photos.zip", "photos"), ("pack.tar", "pack"), ("deep.tar.gz", "deep"), ("short.tgz", "short")] {
        let out = extract(&d.join(archive)).unwrap_or_else(|e| panic!("{archive}: {e}"));
        assert_eq!(out, d.join(folder), "{archive}");
        check_tree(&out);
    }
    assert_eq!(std::fs::read(d.join("photos.zip")).unwrap(), before, "archive untouched");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn extract_plain_gz_writes_the_single_file_inside_the_folder() {
    let d = tmp_dir("gz");
    std::fs::write(d.join("notes.txt"), "plain text").unwrap();
    sh(&d, "/usr/bin/gzip", &["-k", "notes.txt"]);
    std::fs::remove_file(d.join("notes.txt")).unwrap();
    let out = extract(&d.join("notes.txt.gz")).unwrap();
    assert_eq!(out, d.join("notes.txt"), "folder named after the archive without .gz");
    assert!(out.is_dir());
    assert_eq!(std::fs::read_to_string(out.join("notes.txt")).unwrap(), "plain text");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn extracting_twice_makes_name_2_and_leaves_the_first_alone() {
    let d = tree("twice");
    sh(&d.join("src"), "/usr/bin/zip", &["-qr", "../photos.zip", "."]);
    let first = extract(&d.join("photos.zip")).unwrap();
    std::fs::write(first.join("mine.txt"), "keep").unwrap();
    let second = extract(&d.join("photos.zip")).unwrap();
    let third = extract(&d.join("photos.zip")).unwrap();
    assert_eq!(second, d.join("photos 2"));
    assert_eq!(third, d.join("photos 3"));
    assert_eq!(std::fs::read_to_string(first.join("mine.txt")).unwrap(), "keep", "first folder untouched");
    assert!(!second.join("mine.txt").exists(), "never merged");
    check_tree(&second);
    // a plain file with the folder's name also forces the next suffix
    std::fs::write(d.join("pack"), "file").unwrap();
    sh(&d.join("src"), "/usr/bin/tar", &["-cf", "../pack.tar", "."]);
    assert_eq!(extract(&d.join("pack.tar")).unwrap(), d.join("pack 2"));
    assert_eq!(std::fs::read_to_string(d.join("pack")).unwrap(), "file");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn extract_refuses_a_traversal_archive_and_creates_nothing() {
    let d = tmp_dir("evil");
    let work = d.join("work");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(d.join("escape.txt"), "outside").unwrap();
    sh(&work, "/usr/bin/tar", &["-cPf", "../evil.tar", "../escape.txt"]);
    sh(&work, "/usr/bin/zip", &["-q", "../evil.zip", "../escape.txt"]);
    sh(&work, "/usr/bin/tar", &["-cPf", "../abs.tar", &d.join("escape.txt").to_string_lossy()]);
    sh(&work, "/usr/bin/tar", &["-czPf", "../evil.tgz", "../escape.txt"]);
    let listing = names(&d);
    for a in ["evil.tar", "evil.zip", "abs.tar", "evil.tgz"] {
        let before = std::fs::read(d.join(a)).unwrap();
        let err = extract(&d.join(a)).unwrap_err();
        assert!(err.starts_with("unsafe path in archive: "), "{a}: {err}");
        assert_eq!(names(&d), listing, "{a}: nothing created beside the archives");
        assert_eq!(names(&work), Vec::<String>::new(), "{a}: nothing created in work");
        assert_eq!(std::fs::read(d.join(a)).unwrap(), before);
    }
    assert_eq!(std::fs::read_to_string(d.join("escape.txt")).unwrap(), "outside");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn unsafe_entry_check_covers_dotdot_absolute_backslash_and_drive() {
    use crate::route::unsafe_entry;
    for bad in ["../x", "a/../../x", "/etc/passwd", "a\\..\\b", "\\server\\x", "C:\\x", "c:/x", "a/..", "a\0b"] {
        assert!(unsafe_entry(bad), "{bad:?}");
    }
    for ok in ["a.txt", "sub/b.txt", "./a", "dir/", "a..b/c", "..hidden", "x:y/z"] {
        assert!(!unsafe_entry(ok), "{ok:?}");
    }
}

#[test]
fn extracting_a_corrupt_archive_fails_cleanly_and_removes_the_folder() {
    let d = tmp_dir("corrupt");
    let before_dir = |name: &str, bytes: &[u8]| {
        std::fs::write(d.join(name), bytes).unwrap();
    };
    before_dir("bad.zip", b"this is not a zip at all");
    before_dir("bad.tar", b"this is not a tar at all");
    before_dir("bad.tar.gz", b"this is not gzip at all");
    before_dir("bad.gz", b"this is not gzip at all");
    let listing = names(&d);
    for a in ["bad.zip", "bad.tar", "bad.tar.gz", "bad.gz"] {
        let err = extract(&d.join(a)).unwrap_err();
        assert!(!err.is_empty(), "{a}");
        assert_eq!(names(&d), listing, "{a}: no folder left behind");
        assert_eq!(std::fs::read(d.join(a)).unwrap().len(), if a == "bad.zip" || a == "bad.tar" { 24 } else { 23 }, "{a} unchanged");
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn extract_tool_runs_through_run_file_tool_for_several_archives() {
    use ferrocast_core::convert::FileTool;
    let d = tree("several");
    sh(&d.join("src"), "/usr/bin/zip", &["-qr", "../one.zip", "."]);
    sh(&d.join("src"), "/usr/bin/tar", &["-cf", "../two.tar", "."]);
    let (outs, errs) = crate::services::run_file_tool(&[d.join("one.zip"), d.join("two.tar")], &FileTool::Extract, None);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(outs, vec![d.join("one"), d.join("two")]);
    check_tree(&outs[0]);
    check_tree(&outs[1]);
    // an unsafe one fails alone; the good one still extracts
    let work = d.join("src");
    std::fs::write(d.join("escape.txt"), "o").unwrap();
    sh(&work, "/usr/bin/tar", &["-cPf", "../evil.tar", "../escape.txt"]);
    let (outs, errs) = crate::services::run_file_tool(&[d.join("evil.tar"), d.join("one.zip")], &FileTool::Extract, None);
    assert_eq!(outs, vec![d.join("one 2")]);
    assert_eq!(errs.len(), 1);
    assert!(errs[0].contains("unsafe path in archive: ../escape.txt"), "{errs:?}");
    assert!(!d.join("evil").exists());
    let _ = std::fs::remove_dir_all(&d);
}

// ---- A8b: reorder window

fn reorder_files(n: usize) -> Vec<std::path::PathBuf> {
    (0..n).map(|i| std::path::PathBuf::from(format!("/nonexistent/clip {i}.mp4"))).collect()
}

fn reorder_state(app: &App) -> (Vec<std::path::PathBuf>, usize) {
    match app.view() {
        View::Reorder { files, cursor, .. } => (files.clone(), *cursor),
        other => panic!("reorder view closed unexpectedly: {}", other.title()),
    }
}

fn open_reorder(app: &mut App, purpose: ReorderPurpose, files: Vec<std::path::PathBuf>) {
    app.stack = vec![View::Root, View::Reorder { purpose, files, cursor: 0 }];
}

#[test]
fn reorder_job_maps_each_purpose_to_its_tool_and_keeps_the_order() {
    use ferrocast_core::convert::{FileTool, ImageOp};
    let f = reorder_files(3);
    let mut rev = f.clone();
    rev.reverse();
    assert_eq!(reorder_job(ReorderPurpose::MergePdf, rev.clone()), ReorderJob::Tool(FileTool::Merge, rev.clone()));
    assert_eq!(reorder_job(ReorderPurpose::MergeAudio, f.clone()), ReorderJob::Tool(FileTool::Merge, f.clone()));
    assert_eq!(reorder_job(ReorderPurpose::JoinVideos, rev.clone()), ReorderJob::Tool(FileTool::Join, rev.clone()));
    assert_eq!(reorder_job(ReorderPurpose::CreatePdf, f.clone()), ReorderJob::Image(ImageOp::CombinePdf, f));
}

#[test]
fn reorder_view_renders_for_2_3_and_8_files_in_both_themes() {
    let mut app = fixture();
    for theme in [egui::ThemePreference::Dark, egui::ThemePreference::Light] {
        app.ctx.set_theme(theme);
        for n in [2, 3, 8] {
            for purpose in [ReorderPurpose::MergePdf, ReorderPurpose::CreatePdf, ReorderPurpose::JoinVideos, ReorderPurpose::MergeAudio] {
                open_reorder(&mut app, purpose, reorder_files(n));
                render(&mut app, vec![]);
                render(&mut app, vec![]);
            }
        }
        let long = (0..3).map(|i| std::path::PathBuf::from(format!("/nonexistent/{}{i}.pdf", "very long file name ".repeat(12)))).collect();
        open_reorder(&mut app, ReorderPurpose::MergePdf, long);
        render(&mut app, vec![]);
    }
}

#[test]
fn reorder_arrow_down_moves_only_the_cursor() {
    let mut app = fixture();
    open_reorder(&mut app, ReorderPurpose::JoinVideos, reorder_files(3));
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::ArrowDown)]);
    assert_eq!(reorder_state(&app), (reorder_files(3), 1));
    render(&mut app, vec![key_event(Key::ArrowUp)]);
    assert_eq!(reorder_state(&app).1, 0);
}

#[test]
fn reorder_cmd_arrows_move_the_row_and_the_cursor_follows() {
    let mut app = fixture();
    let f = reorder_files(3);
    open_reorder(&mut app, ReorderPurpose::MergePdf, f.clone());
    render(&mut app, vec![]);
    render_mods(&mut app, vec![key_mod_event(Key::ArrowDown, CMD)], CMD);
    let (files, cursor) = reorder_state(&app);
    assert_eq!(files, vec![f[1].clone(), f[0].clone(), f[2].clone()]);
    assert_eq!(cursor, 1, "the cursor follows the moved row");
    render_mods(&mut app, vec![key_mod_event(Key::ArrowUp, CMD)], CMD);
    assert_eq!(reorder_state(&app), (f.clone(), 0));
    render_mods(&mut app, vec![key_mod_event(Key::ArrowUp, CMD)], CMD);
    assert_eq!(reorder_state(&app), (f, 0), "Cmd+Up on the first row does nothing");
}

#[test]
fn reorder_escape_goes_back_without_starting_a_job() {
    let mut app = fixture();
    open_reorder(&mut app, ReorderPurpose::MergePdf, reorder_files(2));
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::Root));
    assert!(app.rx.try_recv().is_err(), "Esc must not start a job");
}

#[test]
fn reorder_enter_with_one_file_refuses_with_a_toast() {
    let mut app = fixture();
    open_reorder(&mut app, ReorderPurpose::MergePdf, reorder_files(1));
    render(&mut app, vec![]);
    app.toast = None;
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(app.toast.is_some(), "one file is refused with a toast");
    assert!(matches!(app.view(), View::Reorder { .. }), "the view stays");
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert!(app.rx.try_recv().is_err(), "no job started");
}

#[test]
fn convert_view_offers_choose_order_only_for_two_or_more_files_of_one_family() {
    let mut app = fixture();
    let dir = std::env::temp_dir().join(format!("fc-order-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let purposes = |app: &mut App, names: &[&str]| -> Vec<ReorderPurpose> {
        let files: Vec<_> = names
            .iter()
            .map(|n| {
                let p = dir.join(n);
                std::fs::write(&p, b"x").unwrap();
                p
            })
            .collect();
        app.stack = vec![View::Root, View::Convert(files)];
        app.sections()
            .iter()
            .flat_map(|s| s.items.iter())
            .filter(|i| i.title == "Choose order…")
            .map(|i| match &i.kind {
                Kind::Editor(Act::ReorderFiles(p, files)) => {
                    assert!(files.len() >= 2);
                    *p
                }
                _ => panic!("wrong kind"),
            })
            .collect()
    };
    assert_eq!(purposes(&mut app, &["a.pdf", "b.pdf"]), vec![ReorderPurpose::MergePdf]);
    assert_eq!(purposes(&mut app, &["a.mp4", "b.mp4"]), vec![ReorderPurpose::JoinVideos]);
    assert_eq!(purposes(&mut app, &["a.mp3", "b.mp3"]), vec![ReorderPurpose::MergeAudio]);
    assert_eq!(purposes(&mut app, &["a.png", "b.png"]), vec![ReorderPurpose::CreatePdf]);
    assert!(purposes(&mut app, &["a.pdf"]).is_empty());
    assert!(purposes(&mut app, &["a.png", "b.mp4"]).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- A9c split window ----

fn split_view(app: &mut App, name: &str, points: Vec<f64>, parts: usize, cursor: f64) {
    let f = std::env::temp_dir().join(name);
    let mut t = ferrocast_core::videoplan::Trim::new(10.0, 25.0).unwrap();
    t.seek(cursor);
    app.stack = vec![View::Root, View::VideoSplit { file: f, trim: Some(t), points, parts }];
}

fn split_state(app: &App) -> (Vec<f64>, usize, f64) {
    match app.view() {
        View::VideoSplit { points, parts, trim: Some(t), .. } => (points.clone(), *parts, t.cursor),
        other => panic!("split view closed unexpectedly: {}", other.title()),
    }
}

#[test]
fn split_view_renders_probing_and_ready_in_both_themes() {
    let mut app = fixture();
    for theme in [egui::ThemePreference::Dark, egui::ThemePreference::Light] {
        app.ctx.set_theme(theme);
        let f = std::env::temp_dir().join("fc-split-render.mp4");
        app.stack = vec![View::Root, View::VideoSplit { file: f, trim: None, points: vec![], parts: 2 }];
        render(&mut app, vec![]);
        split_view(&mut app, "fc-split-render.mp4", vec![2.0, 6.0], 2, 3.0);
        render(&mut app, vec![]);
        assert!(matches!(app.view(), View::VideoSplit { .. }));
    }
}

#[test]
fn split_m_adds_a_point_at_the_cursor_and_dedupes() {
    let mut app = fixture();
    split_view(&mut app, "fc-split-m.mp4", vec![], 2, 2.0);
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::M)]);
    assert_eq!(split_state(&app).0, vec![2.0]);
    render(&mut app, vec![key_event(Key::M)]);
    assert_eq!(split_state(&app).0, vec![2.0], "a second M on the same frame adds nothing");
    render(&mut app, vec![key_event(Key::ArrowRight)]);
    render(&mut app, vec![key_event(Key::M)]);
    let (p, _, c) = split_state(&app);
    assert_eq!(p.len(), 2, "{p:?}");
    assert!((p[1] - c).abs() < 1e-9 && p[1] > p[0], "{p:?} cursor {c}");
}

#[test]
fn split_backspace_removes_the_last_added_point() {
    let mut app = fixture();
    split_view(&mut app, "fc-split-bs.mp4", vec![], 2, 2.0);
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::M)]);
    render(&mut app, vec![key_event(Key::ArrowRight)]);
    render(&mut app, vec![key_event(Key::M)]);
    render(&mut app, vec![key_event(Key::Backspace)]);
    assert_eq!(split_state(&app).0, vec![2.0]);
    render(&mut app, vec![key_event(Key::Backspace)]);
    render(&mut app, vec![key_event(Key::Backspace)]);
    assert!(split_state(&app).0.is_empty(), "removing from an empty list is harmless and the view stays");
}

#[test]
fn split_escape_goes_back_without_a_job() {
    let mut app = fixture();
    split_view(&mut app, "fc-split-esc.mp4", vec![3.0], 2, 2.0);
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::Root));
    assert!(app.rx.try_recv().is_err(), "no job may start");
}

#[test]
fn split_enter_refuses_fewer_than_two_clips_and_starts_nothing() {
    let mut app = fixture();
    // a lone point that would leave a tiny piece leaves one clip (the equal-parts box cannot go
    // below 2, so a count of 1 is covered by the pure split_job test)
    split_view(&mut app, "fc-split-enter.mp4", vec![0.01], 2, 2.0);
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::VideoSplit { .. }), "one clip is refused and the view stays");
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("Nothing to split")), "{:?}", app.toast);
    assert!(app.rx.try_recv().is_err(), "no job may start");
}

#[test]
fn split_job_builds_the_exact_ranges() {
    use ferrocast_core::convert::FileTool;
    match split_job(10.0, &[], 2) {
        Ok(FileTool::SplitPoints(r, true)) => assert_eq!(r, vec![(0.0, 5.0), (5.0, 10.0)]),
        other => panic!("{other:?}"),
    }
    match split_job(10.0, &[3.0, 7.0], 2) {
        Ok(FileTool::SplitPoints(r, true)) => assert_eq!(r.len(), 3, "points override parts"),
        other => panic!("{other:?}"),
    }
    assert!(split_job(10.0, &[], 1).is_err());
    assert!(split_job(f64::NAN, &[], 2).is_err());
}

#[test]
fn split_enter_with_ready_ranges_does_not_run_without_ffmpeg() {
    let mut app = fixture();
    app.tools = Default::default();
    split_view(&mut app, "fc-split-noff.mp4", vec![], 2, 2.0);
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::VideoSplit { .. }), "without ffmpeg the view stays");
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("ffmpeg")), "{:?}", app.toast);
}

#[test]
fn convert_view_offers_split_video_for_one_mp4_and_not_for_two() {
    let mut app = fixture();
    let dir = std::env::temp_dir().join(format!("fc-splitoffer-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let has = |app: &mut App, names: &[&str]| -> bool {
        let files: Vec<_> = names
            .iter()
            .map(|n| {
                let p = dir.join(n);
                std::fs::write(&p, b"x").unwrap();
                p
            })
            .collect();
        app.stack = vec![View::Root, View::Convert(files)];
        app.sections()
            .iter()
            .flat_map(|s| s.items.iter())
            .any(|i| i.title == "Split Video…" && matches!(i.kind, Kind::Editor(Act::SplitVideo(_))))
    };
    assert!(has(&mut app, &["a.mp4"]));
    assert!(!has(&mut app, &["a.mp4", "b.mp4"]));
    assert!(!has(&mut app, &["a.png"]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tool_reach_split_video_opens_the_window_for_one_file_and_keeps_the_preset_for_several() {
    use ferrocast_core::convert::FileTool;
    use ferrocast_core::tools::{implemented, Impl, ToolId};
    let a = std::path::PathBuf::from("/nonexistent/a.mp4");
    let b = std::path::PathBuf::from("/nonexistent/b.mp4");
    assert!(matches!(tool_reach(ToolId::SplitVideo, std::slice::from_ref(&a)), ToolReach::Act(Act::SplitVideo(_))));
    assert!(matches!(tool_reach(ToolId::SplitVideo, &[a, b]), ToolReach::File(FileTool::SplitEvery(60))));
    assert_eq!(implemented(ToolId::SplitVideo), Impl::Window);
}

// ---- A6b: image Compress window

fn compress_view(files: Vec<std::path::PathBuf>, size: Option<(u32, u32)>, mode: usize, kb: u32) -> View {
    View::CompressImage { files, orig_bytes: 2_000_000, size, mode, kb, percent: 40, preset: 1, max_edge: None, resize: false, media: None }
}

fn compress_app(mode: usize, kb: u32) -> App {
    let mut app = fixture();
    let f = std::path::PathBuf::from(format!("/nonexistent/fc-compress-{}.png", uuid::Uuid::new_v4()));
    app.stack = vec![View::Root, compress_view(vec![f], Some((4000, 3000)), mode, kb)];
    render(&mut app, vec![]);
    app
}

#[test]
fn compress_view_renders_in_both_themes_with_and_without_size() {
    let mut app = fixture();
    let f = vec![std::path::PathBuf::from("/nonexistent/a.png")];
    for theme in [egui::ThemePreference::Dark, egui::ThemePreference::Light] {
        app.ctx.set_theme(theme);
        for size in [None, Some((4000, 3000))] {
            for mode in 0..3 {
                app.stack = vec![View::Root, compress_view(f.clone(), size, mode, 500)];
                render(&mut app, vec![]);
                render(&mut app, vec![]);
                assert_eq!(app.view().title(), "Compress Images");
            }
        }
    }
    // no files at all still draws
    app.stack = vec![View::Root, compress_view(vec![], None, 0, 500)];
    render(&mut app, vec![]);
}

#[test]
fn compress_escape_goes_back_without_a_job() {
    let mut app = compress_app(0, 500);
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::Root), "Escape goes back");
    assert!(app.rx.try_recv().is_err(), "no job may start");
}

#[test]
fn compress_enter_with_invalid_input_starts_nothing_and_stays() {
    // KB 0
    let mut app = compress_app(0, 0);
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::CompressImage { .. }), "an invalid goal keeps the window open");
    assert!(app.rx.try_recv().is_err(), "no job may start for KB 0");
    assert!(app.toast.is_some(), "the refusal is explained");
    // no files
    let mut app = fixture();
    app.stack = vec![View::Root, compress_view(vec![], None, 0, 500)];
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::CompressImage { .. }));
    assert!(app.rx.try_recv().is_err());
    assert!(app.toast.is_some());
}

#[test]
fn compress_enter_with_a_valid_goal_starts_the_job_and_goes_back() {
    let mut app = compress_app(0, 500);
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::Root));
    // the job runs on a worker and reports through the bus (the file does not exist: a failure toast)
    let msg = app.rx.recv_timeout(std::time::Duration::from_secs(20)).expect("the job reports");
    assert!(matches!(msg, Msg::Toast(t) if t.contains("Compress")));
}

/// Click the KB box of the compress window (position taken from the layout; guarded by an assertion).
fn click_compress_kb_box(app: &mut App) {
    let pos = Pos2::new(COMPRESS_KB_BOX.0, COMPRESS_KB_BOX.1);
    let btn = |pressed| Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    render(app, vec![Event::PointerMoved(pos)]);
    render(app, vec![btn(true)]);
    render(app, vec![btn(false)]);
    render(app, vec![]);
}

// centre of the KB box in points (found by probing the layout; breaks if the controls move)
const COMPRESS_KB_BOX: (f32, f32) = (210.0, 156.0);

// L74 style: Escape while the number box is edited ends the edit but keeps the window.
#[test]
fn compress_number_box_keeps_escape_and_enter_while_editing() {
    let mut app = compress_app(0, 500);
    click_compress_kb_box(&mut app);
    assert!(app.ctx.egui_wants_keyboard_input(), "the click must have started editing the KB box (layout moved?)");
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::CompressImage { .. }), "Escape ends the edit, it must not close the window");
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::Root), "a second Escape closes it");
    let mut app = compress_app(0, 500);
    click_compress_kb_box(&mut app);
    assert!(app.ctx.egui_wants_keyboard_input());
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::CompressImage { .. }), "Enter confirms the number, it must not start the job");
    assert!(app.rx.try_recv().is_err());
}

#[test]
fn convert_view_offers_compress_for_one_or_more_images_only() {
    let mut app = fixture();
    let dir = std::env::temp_dir().join(format!("fc-compress-entry-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let rows = |app: &mut App, names: &[&str]| -> Vec<Vec<std::path::PathBuf>> {
        let files: Vec<_> = names
            .iter()
            .map(|n| {
                let p = dir.join(n);
                std::fs::write(&p, b"x").unwrap();
                p
            })
            .collect();
        app.stack = vec![View::Root, View::Convert(files)];
        app.sections()
            .iter()
            .flat_map(|s| s.items.iter())
            .filter(|i| i.title == "Compress…")
            .map(|i| match &i.kind {
                Kind::Editor(Act::CompressImages(f)) => f.clone(),
                _ => panic!("wrong kind"),
            })
            .collect()
    };
    assert_eq!(rows(&mut app, &["a.png"]).len(), 1);
    assert_eq!(rows(&mut app, &["a.png", "b.png"])[0].len(), 2);
    assert!(rows(&mut app, &["a.txt"]).is_empty());
    assert!(rows(&mut app, &["a.png", "b.mp4"]).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tool_reach_compress_opens_the_window_for_images_and_keeps_other_mappings() {
    use ferrocast_core::tools::ToolId;
    let imgs = |n: &[&str]| n.iter().map(std::path::PathBuf::from).collect::<Vec<_>>();
    match tool_reach(ToolId::Compress, &imgs(&["/x/a.png"])) {
        ToolReach::Act(Act::CompressImages(f)) => assert_eq!(f.len(), 1),
        _ => panic!("one image opens the window"),
    }
    match tool_reach(ToolId::Compress, &imgs(&["/x/a.png", "/x/b.jpg"])) {
        ToolReach::Act(Act::CompressImages(f)) => assert_eq!(f.len(), 2),
        _ => panic!("several images open the window"),
    }
    assert!(!matches!(tool_reach(ToolId::Compress, &imgs(&["/x/a.mp4"])), ToolReach::Act(Act::CompressImages(_))));
    assert!(!matches!(tool_reach(ToolId::Compress, &imgs(&["/x/a.pdf"])), ToolReach::Act(Act::CompressImages(_))));
}

#[test]
fn compress_goal_maps_fields_and_rejects_out_of_range() {
    use ferrocast_core::compress::{Goal, Preset};
    assert_eq!(compress_goal(0, 500, 40, 1, None, false).unwrap().0, Goal::TargetBytes(500 * 1024));
    assert_eq!(compress_goal(1, 500, 40, 1, None, false).unwrap().0, Goal::Percent(40));
    assert_eq!(compress_goal(2, 0, 0, 2, None, false).unwrap().0, Goal::Preset(Preset::Strong));
    assert_eq!(compress_goal(2, 0, 0, 0, Some(800), true).unwrap().1.max_edge, Some(800));
    assert_eq!(compress_goal(2, 0, 0, 0, Some(800), false).unwrap().1.max_edge, None);
    assert_eq!(compress_goal(2, 0, 0, 0, None, true).unwrap().1.max_edge, Some(1600));
    assert!(compress_goal(0, 0, 40, 1, None, false).is_err());
    assert!(compress_goal(0, 100_001, 40, 1, None, false).is_err());
    assert!(compress_goal(1, 500, 0, 1, None, false).is_err());
    assert!(compress_goal(1, 500, 96, 1, None, false).is_err());
    assert!(compress_goal(2, 0, 0, 3, None, false).is_err());
    assert!(compress_goal(2, 0, 0, 0, Some(63), true).is_err());
    assert!(compress_goal(3, 500, 40, 1, None, false).is_err());
}

#[test]
fn compress_with_labels() {
    use ferrocast_core::compress::{Goal, Preset, Resize};
    use ferrocast_core::convert::ImageOp;
    let none = Resize::default();
    assert_eq!(ImageOp::CompressWith(Goal::TargetBytes(500 * 1024), none).label(), "Compress · 500 KB");
    assert_eq!(ImageOp::CompressWith(Goal::Percent(40), none).label(), "Compress · 40 % smaller");
    assert_eq!(
        ImageOp::CompressWith(Goal::Preset(Preset::Balanced), Resize { max_edge: Some(1600), percent: None }).label(),
        "Compress · balanced · max 1600 px"
    );
    assert_eq!(ImageOp::CompressWith(Goal::Percent(40), none).suffix(), "compressed");
}

// ---- A6c: audio and video Compress (same window, media mode)

fn media_view(
    files: Vec<std::path::PathBuf>,
    video: bool,
    info: Option<Option<ferrocast_core::compress::MediaInfo>>,
    mode: usize,
    kb: u32,
) -> View {
    let (loaded, info) = match info {
        Some(i) => (true, i),
        None => (false, None),
    };
    View::CompressImage {
        files,
        orig_bytes: 9_000_000,
        size: None,
        mode,
        kb,
        percent: 40,
        preset: 1,
        max_edge: None,
        resize: false,
        media: Some(MediaWin { video, loaded, info }),
    }
}

fn some_info(video: bool) -> Option<Option<ferrocast_core::compress::MediaInfo>> {
    Some(Some(ferrocast_core::compress::MediaInfo { duration: 20.0, has_audio: true, dims: video.then_some((1280, 720)) }))
}

fn media_app(video: bool, mode: usize, kb: u32) -> App {
    let mut app = fixture();
    app.tools.ffmpeg = Some("/nonexistent/ffmpeg");
    let f = std::path::PathBuf::from(format!("/nonexistent/fc-compress-{}.{}", uuid::Uuid::new_v4(), if video { "mp4" } else { "mp3" }));
    app.stack = vec![View::Root, media_view(vec![f], video, some_info(video), mode, kb)];
    render(&mut app, vec![]);
    app
}

#[test]
fn compress_media_view_renders_in_both_themes() {
    let mut app = fixture();
    for theme in [egui::ThemePreference::Dark, egui::ThemePreference::Light] {
        app.ctx.set_theme(theme);
        for video in [false, true] {
            for info in [None, Some(None), some_info(video)] {
                for mode in 0..3 {
                    let f = vec![std::path::PathBuf::from(if video { "/nonexistent/a.mp4" } else { "/nonexistent/a.mp3" })];
                    // 10 KB over 20 s is an impossible target: the line says so instead of panicking
                    app.stack = vec![View::Root, media_view(f, video, info, mode, 10)];
                    render(&mut app, vec![]);
                    render(&mut app, vec![]);
                    assert_eq!(app.view().title(), "Compress Audio and Video");
                }
            }
        }
    }
}

#[test]
fn compress_media_job_maps_goals_and_refuses_the_impossible() {
    use ferrocast_core::compress::{Goal, MediaInfo, Preset};
    use ferrocast_core::convert::FileTool;
    let info = MediaInfo { duration: 20.0, has_audio: true, dims: Some((1280, 720)) };
    let job = |video, i: Option<&MediaInfo>, mode, kb, resize| compress_media_job(video, 9_000_000, i, mode, kb, 40, 1, Some(320), resize);
    match job(true, Some(&info), 0, 2048, false) {
        Ok(FileTool::Compress(Goal::TargetBytes(b), r)) => assert_eq!((b, r.max_edge), (2048 * 1024, None)),
        other => panic!("{other:?}"),
    }
    // resize is passed for video only
    assert_eq!(
        job(true, Some(&info), 2, 0, true).map(|t| t
            == FileTool::Compress(Goal::Preset(Preset::Balanced), ferrocast_core::compress::Resize { max_edge: Some(320), percent: None })),
        Ok(true)
    );
    match job(false, Some(&info), 2, 0, true) {
        Ok(FileTool::Compress(_, r)) => assert_eq!(r.max_edge, None, "audio ignores resize"),
        other => panic!("{other:?}"),
    }
    // media targets may exceed the 100000 KB of the image window
    assert!(job(true, Some(&info), 0, 500_000, false).is_ok());
    assert!(job(true, Some(&info), 0, 5_000_000, false).is_err());
    assert!(job(true, Some(&info), 0, 0, false).is_err());
    assert!(job(true, Some(&info), 0, 10, false).unwrap_err().contains("too small"));
    assert!(job(true, None, 0, 2048, false).is_err(), "not probed yet");
}

#[test]
fn compress_media_escape_goes_back_without_a_job() {
    let mut app = media_app(true, 0, 500);
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::Root));
    assert!(app.rx.try_recv().is_err());
}

#[test]
fn compress_media_enter_invalid_starts_nothing() {
    for (mode, kb) in [(0, 0), (0, 10)] {
        let mut app = media_app(true, mode, kb);
        render(&mut app, vec![key_event(Key::Enter)]);
        assert!(matches!(app.view(), View::CompressImage { .. }), "kb {kb}");
        assert!(app.rx.try_recv().is_err(), "no job may start for kb {kb}");
        assert!(app.toast.is_some());
    }
    // not probed yet
    let mut app = fixture();
    app.tools.ffmpeg = Some("/nonexistent/ffmpeg");
    app.stack = vec![View::Root, media_view(vec!["/nonexistent/a.mp4".into()], true, None, 2, 0)];
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::CompressImage { .. }));
    assert!(app.rx.try_recv().is_err());
}

#[test]
fn compress_media_enter_valid_starts_the_job_and_goes_back() {
    let mut app = media_app(true, 2, 0);
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::Root));
    let msg = app.rx.recv_timeout(std::time::Duration::from_secs(20)).expect("the job reports");
    assert!(matches!(msg, Msg::Toast(t) if t.contains("Compress")));
}

#[test]
fn media_probe_result_reaches_the_window() {
    let mut app = fixture();
    let f = std::path::PathBuf::from("/nonexistent/a.mp4");
    app.stack = vec![View::Root, media_view(vec![f.clone()], true, None, 0, 500)];
    let info = ferrocast_core::compress::MediaInfo { duration: 5.0, has_audio: false, dims: Some((640, 360)) };
    app.bus.send(Msg::MediaProbed(f, Some(info)));
    app.drain();
    match app.view() {
        View::CompressImage { media: Some(m), .. } => assert_eq!((m.loaded, m.info), (true, Some(info))),
        _ => panic!("window gone"),
    }
}

const COMPRESS_MEDIA_KB_BOX: (f32, f32) = (210.0, 156.0);

// L74 style: Escape and Enter inside the number box of the media window keep it open.
#[test]
fn compress_media_number_box_keeps_escape_and_enter_while_editing() {
    let click = |app: &mut App| {
        let pos = Pos2::new(COMPRESS_MEDIA_KB_BOX.0, COMPRESS_MEDIA_KB_BOX.1);
        let btn = |pressed| Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        render(app, vec![Event::PointerMoved(pos)]);
        render(app, vec![btn(true)]);
        render(app, vec![btn(false)]);
        render(app, vec![]);
    };
    let mut app = media_app(true, 0, 500);
    click(&mut app);
    assert!(app.ctx.egui_wants_keyboard_input(), "the click must have started editing the KB box (layout moved?)");
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::CompressImage { .. }), "Escape ends the edit, it must not close the window");
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::Root), "a second Escape closes it");
    let mut app = media_app(true, 0, 500);
    click(&mut app);
    assert!(app.ctx.egui_wants_keyboard_input());
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::CompressImage { .. }), "Enter confirms the number, it must not start the job");
    assert!(app.rx.try_recv().is_err());
}

#[test]
fn convert_view_offers_compress_for_audio_and_video_and_keeps_images() {
    let mut app = fixture();
    let dir = std::env::temp_dir().join(format!("fc-compress-media-entry-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // (media files, image files) offered by the "Compress…" rows
    let rows = |app: &mut App, names: &[&str]| -> (Vec<usize>, Vec<usize>) {
        let files: Vec<_> = names
            .iter()
            .map(|n| {
                let p = dir.join(n);
                std::fs::write(&p, b"x").unwrap();
                p
            })
            .collect();
        app.stack = vec![View::Root, View::Convert(files)];
        let (mut media, mut images) = (vec![], vec![]);
        for i in app.sections().iter().flat_map(|s| s.items.iter()).filter(|i| i.title == "Compress…") {
            match &i.kind {
                Kind::Editor(Act::CompressMedia(f)) => media.push(f.len()),
                Kind::Editor(Act::CompressImages(f)) => images.push(f.len()),
                _ => panic!("wrong kind"),
            }
        }
        (media, images)
    };
    assert_eq!(rows(&mut app, &["a.mp4"]), (vec![1], vec![]));
    assert_eq!(rows(&mut app, &["a.mp4", "b.mp4"]), (vec![2], vec![]));
    assert_eq!(rows(&mut app, &["a.mp3"]), (vec![1], vec![]));
    assert_eq!(rows(&mut app, &["a.mp3", "b.mp3"]), (vec![2], vec![]));
    assert_eq!(rows(&mut app, &["a.png", "b.png"]), (vec![], vec![2]));
    assert_eq!(rows(&mut app, &["a.png"]), (vec![], vec![1]));
    assert_eq!(rows(&mut app, &["a.mp3", "b.mp4"]), (vec![], vec![]), "mixed families get no Compress row");
    assert_eq!(rows(&mut app, &["a.txt"]), (vec![], vec![]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tool_reach_compress_opens_the_media_window_for_audio_and_video() {
    use ferrocast_core::tools::ToolId;
    let f = |n: &[&str]| n.iter().map(std::path::PathBuf::from).collect::<Vec<_>>();
    for names in [&["/x/a.mp4"][..], &["/x/a.mp4", "/x/b.mov"], &["/x/a.mp3"], &["/x/a.wav", "/x/b.flac"]] {
        assert!(
            matches!(tool_reach(ToolId::Compress, &f(names)), ToolReach::Act(Act::CompressMedia(v)) if v.len() == names.len()),
            "{names:?}"
        );
    }
    assert!(!matches!(tool_reach(ToolId::Compress, &f(&["/x/a.mp3", "/x/b.mp4"])), ToolReach::Act(Act::CompressMedia(_))));
    assert!(!matches!(tool_reach(ToolId::Compress, &f(&["/x/a.pdf"])), ToolReach::Act(Act::CompressMedia(_))), "PDF keeps its mapping");
    assert!(matches!(tool_reach(ToolId::Compress, &f(&["/x/a.png"])), ToolReach::Act(Act::CompressImages(_))));
}

#[test]
fn compress_media_without_ffmpeg_toasts_the_hint_and_opens_nothing() {
    let mut app = fixture();
    app.stack = vec![View::Root];
    app.perform(Act::CompressMedia(vec!["/x/a.mp4".into()]), false);
    assert!(matches!(app.view(), View::Root));
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("ffmpeg")), "{:?}", app.toast);
}

// ---- A6d: PDF compress rows and mapping
#[test]
fn convert_view_offers_three_compress_pdf_rows_for_one_or_two_pdfs() {
    use ferrocast_core::convert::FileTool;
    let mut app = fixture();
    let dir = tmp_dir("pdfc");
    let (a, b, t) = (dir.join("a.pdf"), dir.join("b.pdf"), dir.join("a.txt"));
    for f in [&a, &b, &t] {
        std::fs::write(f, b"x").unwrap();
    }
    for files in [vec![a.clone()], vec![a.clone(), b.clone()]] {
        app.stack = vec![View::Root, View::Convert(files)];
        app.invalidate_sections();
        let items: Vec<_> = app.sections_cached().into_iter().flat_map(|s| s.items).collect();
        let rows: Vec<_> = items.iter().filter(|i| i.title.starts_with("Compress PDF")).collect();
        let titles: Vec<_> = rows.iter().map(|r| r.title.as_str()).collect();
        assert_eq!(titles, ["Compress PDF · light", "Compress PDF · balanced", "Compress PDF · strong"]);
        assert!(rows.iter().all(|r| matches!(&r.kind, Kind::FileTool(_, FileTool::CompressPdf(_)))));
    }
    app.stack = vec![View::Root, View::Convert(vec![t])];
    app.invalidate_sections();
    assert!(!app.sections_cached().into_iter().flat_map(|s| s.items).any(|i| i.title.starts_with("Compress PDF")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn compress_pdf_rows_say_when_ghostscript_is_missing() {
    let mut app = fixture();
    let dir = tmp_dir("pdfg");
    let a = dir.join("a.pdf");
    std::fs::write(&a, b"x").unwrap();
    app.stack = vec![View::Root, View::Convert(vec![a])];
    app.tools.gs = None;
    app.invalidate_sections();
    let items: Vec<_> = app.sections_cached().into_iter().flat_map(|s| s.items).collect();
    assert!(items.iter().filter(|i| i.title.starts_with("Compress PDF")).all(|i| i.accessory == "Needs Ghostscript"));
    app.tools.gs = Some("/gs");
    app.invalidate_sections();
    let items: Vec<_> = app.sections_cached().into_iter().flat_map(|s| s.items).collect();
    assert!(items.iter().filter(|i| i.title.starts_with("Compress PDF")).all(|i| i.accessory == "File tool"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tool_reach_compress_for_pdf_is_balanced_compress_pdf() {
    use ferrocast_core::compress::Preset;
    use ferrocast_core::convert::FileTool;
    use ferrocast_core::tools::ToolId;
    let p = vec![std::path::PathBuf::from("/nonexistent/a.pdf")];
    assert!(matches!(tool_reach(ToolId::Compress, &p), ToolReach::File(FileTool::CompressPdf(Preset::Balanced))));
    let two = vec![p[0].clone(), std::path::PathBuf::from("/nonexistent/b.pdf")];
    assert!(matches!(tool_reach(ToolId::Compress, &two), ToolReach::File(FileTool::CompressPdf(Preset::Balanced))));
}

#[test]
fn pdf_size_report_names_the_outcome() {
    assert_eq!(crate::services::pdf_size_report(1000, 400), "1000 B → 400 B");
    assert_eq!(crate::services::pdf_size_report(1000, 1000), "no smaller: the PDF was already compact");
    assert_eq!(crate::services::pdf_size_report(1000, 1200), "no smaller: the PDF was already compact");
}

// ---- A7d: Edit metadata window
fn meta_field(group: ferrocast_core::metaform::Group, key: &str, value: &str, editable: bool) -> ferrocast_core::metaform::Field {
    ferrocast_core::metaform::Field { group, key: key.into(), value: value.into(), editable }
}

fn meta_fields() -> Vec<ferrocast_core::metaform::Field> {
    use ferrocast_core::metaform::Group;
    vec![
        meta_field(Group::File, "title", "Hello", true),
        meta_field(Group::File, "artist", "Me", true),
        meta_field(Group::File, "major_brand", "isom", false),
        meta_field(Group::Track { index: 0, kind: "video".into() }, "language", "und", true),
        meta_field(Group::Track { index: 0, kind: "video".into() }, "handler_name", "VideoHandler", false),
        meta_field(Group::Track { index: 1, kind: "audio".into() }, "language", "eng", true),
        meta_field(Group::Chapter(0), "title", "Intro", false),
    ]
}

fn meta_view(file: &str, fields: Option<Vec<ferrocast_core::metaform::Field>>, error: Option<&str>) -> View {
    let kind = if file.ends_with(".pdf") { MetaKind::Pdf } else { MetaKind::Media };
    View::EditMetadata {
        file: file.into(),
        kind,
        working: fields.clone().unwrap_or_default(),
        original: fields,
        remove_all: false,
        error: error.map(String::from),
    }
}

#[test]
fn edit_metadata_view_renders_every_state_in_both_themes() {
    let long = "x".repeat(600);
    let mut many = meta_fields();
    many[0].value = long;
    for theme in [egui::ThemePreference::Dark, egui::ThemePreference::Light] {
        let mut app = fixture();
        app.ctx.set_theme(theme);
        for v in [
            meta_view("/x/a.mp4", None, None),
            meta_view("/x/a.mp4", Some(meta_fields()), None),
            meta_view("/x/a.mp4", Some(many.clone()), None),
            meta_view("/x/a.mp4", None, Some("media inspection timed out or failed")),
            meta_view("/x/a.gif", Some(meta_fields()), None),
            meta_view("/x/a.pdf", Some(ferrocast_core::metaform::pdf_fields(&[])), None),
        ] {
            app.stack = vec![View::Root, v];
            render(&mut app, vec![]);
            render(&mut app, vec![]);
            assert!(matches!(app.view(), View::EditMetadata { .. }));
        }
    }
}

#[test]
fn metadata_loaded_reaches_the_window_and_other_files_are_ignored() {
    let mut app = fixture();
    app.stack = vec![View::Root, meta_view("/x/a.mp4", None, None)];
    app.bus.send(Msg::MetadataLoaded("/x/other.mp4".into(), Ok(meta_fields())));
    app.drain();
    assert!(matches!(app.view(), View::EditMetadata { original: None, error: None, .. }), "a reply for another file is ignored");
    app.bus.send(Msg::MetadataLoaded("/x/a.mp4".into(), Ok(meta_fields())));
    app.drain();
    match app.view() {
        View::EditMetadata { original: Some(o), working, .. } => assert_eq!((o.len(), working.len()), (7, 7)),
        _ => panic!("window gone"),
    }
    app.stack = vec![View::Root, meta_view("/x/a.mp4", None, None)];
    app.bus.send(Msg::MetadataLoaded("/x/a.mp4".into(), Err("boom".into())));
    app.drain();
    assert!(matches!(app.view(), View::EditMetadata { original: None, error: Some(e), .. } if e == "boom"));
}

#[test]
fn edit_metadata_escape_goes_back_without_a_job() {
    let mut app = fixture();
    app.stack = vec![View::Root, meta_view("/x/a.mp4", Some(meta_fields()), None)];
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::Root));
    assert!(app.rx.try_recv().is_err());
}

#[test]
fn edit_metadata_enter_without_changes_stays_and_starts_nothing() {
    let mut app = fixture();
    app.stack = vec![View::Root, meta_view("/x/a.mp4", Some(meta_fields()), None)];
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::EditMetadata { .. }));
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("Nothing changed")), "{:?}", app.toast);
    assert!(app.rx.try_recv().is_err());
}

#[test]
fn edit_metadata_enter_with_a_change_saves_and_closes() {
    let mut app = fixture();
    app.stack = vec![View::Root, meta_view("/x/a.mp4", Some(meta_fields()), None)];
    if let Some(View::EditMetadata { working, .. }) = app.stack.last_mut() {
        working[0].value = "Changed".into();
    }
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::Root), "the save pops the window");
}

#[test]
fn edit_metadata_remove_all_counts_as_a_change_for_enter() {
    let mut app = fixture();
    let mut v = meta_view("/x/a.mp4", Some(meta_fields()), None);
    if let View::EditMetadata { remove_all, .. } = &mut v {
        *remove_all = true;
    }
    app.stack = vec![View::Root, v];
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::Root));
}

fn meta_click_first_box(app: &mut App) {
    app.stack = vec![View::Root, meta_view("/x/a.mp4", Some(meta_fields()), None)];
    render(app, vec![]);
    render(app, vec![]);
    let r = app.ctx.data(|d| d.get_temp::<Rect>(egui::Id::new("meta-first-edit"))).expect("the window records its first text box");
    let pos = r.center();
    let btn = |pressed| Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    render(app, vec![Event::PointerMoved(pos)]);
    render(app, vec![btn(true)]);
    render(app, vec![btn(false)]);
    render(app, vec![]);
    assert!(app.ctx.egui_wants_keyboard_input(), "the click must have started editing the first row (layout moved?)");
}

#[test]
fn edit_metadata_escape_while_editing_a_box_keeps_the_view() {
    let mut app = fixture();
    meta_click_first_box(&mut app);
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::EditMetadata { .. }), "Escape ends the edit, it must not close the window");
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::Root), "a second Escape closes it");
    assert!(app.rx.try_recv().is_err());
}

#[test]
fn edit_metadata_enter_while_editing_a_box_does_not_save() {
    let mut app = fixture();
    meta_click_first_box(&mut app);
    if let Some(View::EditMetadata { working, .. }) = app.stack.last_mut() {
        working[0].value = "Changed".into();
    }
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::EditMetadata { .. }), "Enter belongs to the text box");
    assert!(app.rx.try_recv().is_err());
}

#[test]
fn convert_view_offers_edit_metadata_for_one_audio_video_gif_or_pdf() {
    let mut app = fixture();
    let dir = tmp_dir("meta");
    let mut has = |names: &[&str]| {
        let files: Vec<_> = names
            .iter()
            .map(|n| {
                let p = dir.join(n);
                std::fs::write(&p, b"x").unwrap();
                p
            })
            .collect();
        app.stack = vec![View::Root, View::Convert(files)];
        app.invalidate_sections();
        app.sections_cached()
            .iter()
            .flat_map(|s| s.items.iter())
            .any(|i| i.title == "Edit metadata…" && matches!(i.kind, Kind::Editor(Act::EditMetadata(_))))
    };
    for one in ["a.mp4", "a.mp3", "a.gif", "a.pdf"] {
        assert!(has(&[one]), "{one}");
    }
    assert!(!has(&["a.mp4", "b.mp4"]));
    assert!(!has(&["a.pdf", "b.pdf"]));
    assert!(!has(&["a.png"]));
    assert!(!has(&["a.txt"]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tool_reach_remove_metadata_opens_the_window_for_one_non_image_file() {
    use ferrocast_core::tools::ToolId;
    let f = |n: &[&str]| n.iter().map(std::path::PathBuf::from).collect::<Vec<_>>();
    for one in ["/x/a.mp4", "/x/a.mp3", "/x/a.gif", "/x/a.pdf"] {
        assert!(matches!(tool_reach(ToolId::RemoveMetadata, &f(&[one])), ToolReach::Act(Act::EditMetadata(_))), "{one}");
    }
    assert!(matches!(tool_reach(ToolId::RemoveMetadata, &f(&["/x/a.png"])), ToolReach::Image(_)), "images keep StripMetadata");
    assert!(!matches!(tool_reach(ToolId::RemoveMetadata, &f(&["/x/a.mp4", "/x/b.mp4"])), ToolReach::Act(Act::EditMetadata(_))));
    assert!(!matches!(tool_reach(ToolId::RemoveMetadata, &f(&["/x/a.txt"])), ToolReach::Act(Act::EditMetadata(_))));
}

#[test]
fn edit_metadata_media_without_ffmpeg_toasts_the_hint_and_opens_nothing() {
    let mut app = fixture();
    app.tools.ffmpeg = None;
    app.perform(Act::EditMetadata("/x/a.mp4".into()), false);
    assert!(matches!(app.view(), View::Root));
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("ffmpeg")), "{:?}", app.toast);
}

// ---- A10c: audio trim window

fn synth_audio(secs: f64, lead: f64, tone: f64) -> Arc<crate::media_tools::AudioSamples> {
    let rate = 8000usize;
    let n = (secs * rate as f64) as usize;
    let (a, b) = ((lead * rate as f64) as usize, ((lead + tone) * rate as f64) as usize);
    let samples = (0..n)
        .map(|i| {
            if i >= a && i < b {
                if (i / 9) % 2 == 0 {
                    16000
                } else {
                    -16000
                }
            } else {
                0
            }
        })
        .collect();
    Arc::new(crate::media_tools::AudioSamples { samples, rate: rate as u32, duration: secs, truncated: false })
}

fn atrim_file() -> std::path::PathBuf {
    std::env::temp_dir().join("fc-atrim-view.mp3")
}

fn atrim_view(app: &mut App, audio: Arc<crate::media_tools::AudioSamples>) {
    let trim = ferrocast_core::videoplan::Trim::new(audio.duration, 100.0);
    app.stack = vec![View::Root, View::AudioTrim { file: atrim_file(), trim, audio: Some(audio) }];
}

fn atrim_state(app: &App) -> ferrocast_core::videoplan::Trim {
    match app.view() {
        View::AudioTrim { trim: Some(t), .. } => *t,
        other => panic!("audio trim view closed or not loaded: {}", other.title()),
    }
}

const ATRIM_START_BOX: (f32, f32) = (108.0, 312.0);
const ATRIM_END_BOX: (f32, f32) = (195.0, 312.0);
const ATRIM_SILENCE_BTN: (f32, f32) = (260.0, 312.0);

#[test]
fn audio_trim_view_renders_loading_loaded_and_long_in_both_themes() {
    let mut app = fixture();
    for theme in [egui::ThemePreference::Dark, egui::ThemePreference::Light] {
        app.ctx.set_theme(theme);
        app.stack = vec![View::Root, View::AudioTrim { file: atrim_file(), trim: None, audio: None }];
        render(&mut app, vec![]);
        assert_eq!(app.view().title(), "Trim Audio");
        atrim_view(&mut app, synth_audio(6.0, 1.0, 3.0));
        render(&mut app, vec![]);
        render(&mut app, vec![]);
        // ten minutes of samples: still drawn in at most 800 buckets, and cached between frames
        atrim_view(&mut app, synth_audio(600.0, 100.0, 300.0));
        render(&mut app, vec![]);
        render(&mut app, vec![]);
        assert!(matches!(app.view(), View::AudioTrim { trim: Some(_), .. }));
    }
}

#[test]
fn audio_loaded_fills_the_window_and_ignores_another_file() {
    let mut app = fixture();
    app.stack = vec![View::Root, View::AudioTrim { file: atrim_file(), trim: None, audio: None }];
    render(&mut app, vec![]);
    app.bus.send(Msg::AudioLoaded(std::path::PathBuf::from("/nonexistent/other.mp3"), Ok((*synth_audio(6.0, 1.0, 3.0)).clone())));
    app.drain();
    assert!(matches!(app.view(), View::AudioTrim { trim: None, audio: None, .. }), "a reply for another file is ignored");
    app.bus.send(Msg::AudioLoaded(atrim_file(), Ok((*synth_audio(6.0, 1.0, 3.0)).clone())));
    app.drain();
    let t = atrim_state(&app);
    assert!(t.is_whole() && (t.duration - 6.0).abs() < 1e-9 && (t.fps - 100.0).abs() < 1e-9);
    assert!(matches!(app.view(), View::AudioTrim { audio: Some(_), .. }));
}

#[test]
fn audio_loaded_error_for_the_window_closes_it_with_a_toast() {
    let mut app = fixture();
    app.stack = vec![View::Root, View::AudioTrim { file: atrim_file(), trim: None, audio: None }];
    app.bus.send(Msg::AudioLoaded(atrim_file(), Err("no audio in this file".into())));
    app.drain();
    assert!(matches!(app.view(), View::Root));
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("no audio")), "{:?}", app.toast);
}

#[test]
fn audio_trim_arrows_move_the_cursor_by_ten_ms_and_one_second() {
    let mut app = fixture();
    atrim_view(&mut app, synth_audio(6.0, 1.0, 3.0));
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::ArrowRight)]);
    assert!((atrim_state(&app).cursor - 0.01).abs() < 1e-9, "{}", atrim_state(&app).cursor);
    render_mods(&mut app, vec![key_mod_event(Key::ArrowRight, Modifiers::SHIFT)], Modifiers::SHIFT);
    assert!((atrim_state(&app).cursor - 1.01).abs() < 1e-9, "Shift moves a second, not ten ms: {}", atrim_state(&app).cursor);
    render_mods(&mut app, vec![key_mod_event(Key::ArrowLeft, Modifiers::SHIFT)], Modifiers::SHIFT);
    render(&mut app, vec![key_event(Key::ArrowLeft)]);
    assert!(atrim_state(&app).cursor.abs() < 1e-9, "{}", atrim_state(&app).cursor);
}

#[test]
fn audio_trim_i_and_o_set_markers_and_home_end_jump_to_them() {
    let mut app = fixture();
    atrim_view(&mut app, synth_audio(6.0, 1.0, 3.0));
    render(&mut app, vec![]);
    render_mods(&mut app, vec![key_mod_event(Key::ArrowRight, Modifiers::SHIFT)], Modifiers::SHIFT);
    render(&mut app, vec![key_event(Key::I)]);
    render_mods(&mut app, vec![key_mod_event(Key::ArrowRight, Modifiers::SHIFT)], Modifiers::SHIFT);
    render_mods(&mut app, vec![key_mod_event(Key::ArrowRight, Modifiers::SHIFT)], Modifiers::SHIFT);
    render(&mut app, vec![key_event(Key::O)]);
    let t = atrim_state(&app);
    assert!((t.start - 1.0).abs() < 1e-9 && (t.end - 3.0).abs() < 1e-9, "{t:?}");
    render(&mut app, vec![key_event(Key::Home)]);
    assert!((atrim_state(&app).cursor - 1.0).abs() < 1e-9);
    render(&mut app, vec![key_event(Key::End)]);
    assert!((atrim_state(&app).cursor - 2.99).abs() < 1e-9, "{}", atrim_state(&app).cursor);
}

#[test]
fn audio_trim_clicking_the_waveform_seeks() {
    let mut app = fixture();
    atrim_view(&mut app, synth_audio(6.0, 1.0, 3.0));
    render(&mut app, vec![]);
    click_at(&mut app, Pos2::new(375.0, 200.0));
    let c = atrim_state(&app).cursor;
    assert!((c - 3.0).abs() < 0.6, "cursor {c} should be near the middle");
}

#[test]
fn audio_trim_escape_goes_back_and_starts_no_job() {
    let mut app = fixture();
    atrim_view(&mut app, synth_audio(6.0, 1.0, 3.0));
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::Root));
    assert!(app.rx.try_recv().is_err(), "Escape starts no job");
}

#[test]
fn audio_trim_enter_with_the_whole_clip_refuses() {
    let mut app = fixture();
    app.tools.ffmpeg = Some("/nonexistent/ffmpeg-fc-test");
    atrim_view(&mut app, synth_audio(6.0, 1.0, 3.0));
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::AudioTrim { .. }), "a whole-clip selection stays");
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("Nothing to trim")), "{:?}", app.toast);
    assert!(app.rx.try_recv().is_err(), "no job may start");
}

#[test]
fn audio_trim_enter_with_a_range_dispatches_and_pops() {
    let mut app = fixture();
    app.tools.ffmpeg = Some("/nonexistent/ffmpeg-fc-test");
    atrim_view(&mut app, synth_audio(6.0, 1.0, 3.0));
    if let Some(View::AudioTrim { trim: Some(t), .. }) = app.stack.last_mut() {
        assert!(t.set_range(1.0, 4.0));
    }
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::Root), "the window closes once the job is dispatched");
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("Trimming")), "{:?}", app.toast);
}

// L74-style: Escape while the Start box is edited ends the edit but keeps the window.
#[test]
fn audio_trim_start_box_keeps_escape_while_editing() {
    let mut app = fixture();
    atrim_view(&mut app, synth_audio(6.0, 1.0, 3.0));
    render(&mut app, vec![]);
    click_at(&mut app, Pos2::new(ATRIM_START_BOX.0, ATRIM_START_BOX.1));
    assert!(app.ctx.egui_wants_keyboard_input(), "the click must have started editing the Start box (layout moved?)");
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::AudioTrim { .. }), "Escape ends the edit, it must not close the window");
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::Root), "a second Escape closes it");
}

#[test]
fn audio_trim_enter_while_a_box_is_edited_does_not_dispatch() {
    let mut app = fixture();
    app.tools.ffmpeg = Some("/nonexistent/ffmpeg-fc-test");
    atrim_view(&mut app, synth_audio(6.0, 1.0, 3.0));
    if let Some(View::AudioTrim { trim: Some(t), .. }) = app.stack.last_mut() {
        t.set_range(1.0, 4.0);
    }
    render(&mut app, vec![]);
    click_at(&mut app, Pos2::new(ATRIM_END_BOX.0, ATRIM_END_BOX.1));
    assert!(app.ctx.egui_wants_keyboard_input(), "the click must have started editing the End box (layout moved?)");
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::AudioTrim { .. }), "Enter confirms the number, it does not start the trim");
}

#[test]
fn audio_trim_silence_button_sets_start_and_end_then_reports_no_change() {
    let mut app = fixture();
    atrim_view(&mut app, synth_audio(6.0, 1.0, 3.0));
    render(&mut app, vec![]);
    click_at(&mut app, Pos2::new(ATRIM_SILENCE_BTN.0, ATRIM_SILENCE_BTN.1));
    let t = atrim_state(&app);
    assert!((t.start - 1.0).abs() < 0.05 && (t.end - 4.0).abs() < 0.05, "{t:?}");
    app.toast = None;
    click_at(&mut app, Pos2::new(ATRIM_SILENCE_BTN.0, ATRIM_SILENCE_BTN.1));
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("No silence to trim")), "{:?}", app.toast);
}

#[test]
fn audio_trim_silence_button_on_pure_silence_toasts() {
    let mut app = fixture();
    atrim_view(&mut app, synth_audio(6.0, 0.0, 0.0));
    render(&mut app, vec![]);
    click_at(&mut app, Pos2::new(ATRIM_SILENCE_BTN.0, ATRIM_SILENCE_BTN.1));
    assert!(atrim_state(&app).is_whole());
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("No silence to trim")), "{:?}", app.toast);
}

#[test]
fn convert_view_offers_trim_audio_for_one_mp3_only() {
    let mut app = fixture();
    let dir = std::env::temp_dir().join(format!("fc-atrimoffer-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let has = |app: &mut App, names: &[&str]| -> bool {
        let files: Vec<_> = names
            .iter()
            .map(|n| {
                let p = dir.join(n);
                std::fs::write(&p, b"x").unwrap();
                p
            })
            .collect();
        app.stack = vec![View::Root, View::Convert(files)];
        app.sections()
            .iter()
            .flat_map(|s| s.items.iter())
            .any(|i| i.title == "Trim Audio…" && matches!(i.kind, Kind::Editor(Act::TrimAudio(_))))
    };
    assert!(has(&mut app, &["a.mp3"]));
    assert!(!has(&mut app, &["a.mp3", "b.mp3"]));
    assert!(!has(&mut app, &["a.mp4"]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tool_reach_trim_audio_opens_the_window_for_one_file_and_keeps_the_preset_for_several() {
    use ferrocast_core::convert::FileTool;
    use ferrocast_core::tools::{implemented, Impl, ToolId};
    let a = std::path::PathBuf::from("/nonexistent/a.mp3");
    let b = std::path::PathBuf::from("/nonexistent/b.mp3");
    assert!(matches!(tool_reach(ToolId::TrimAudio, std::slice::from_ref(&a)), ToolReach::Act(Act::TrimAudio(_))));
    assert!(matches!(tool_reach(ToolId::TrimAudio, &[a, b]), ToolReach::File(FileTool::TrimSilence)));
    assert_eq!(implemented(ToolId::TrimAudio), Impl::Window);
}

#[test]
fn trim_audio_without_ffmpeg_shows_the_hint_and_opens_no_view() {
    let mut app = fixture();
    app.tools = Default::default();
    let depth = app.stack.len();
    app.perform(Act::TrimAudio(atrim_file()), false);
    assert_eq!(app.stack.len(), depth);
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("ffmpeg")), "{:?}", app.toast);
}

// ---- A10d/A10e: normalize and channels windows

fn anorm_file() -> std::path::PathBuf {
    std::env::temp_dir().join("fc-anorm-view.mp3")
}

fn anorm_view(app: &mut App, loudness: Option<f64>) {
    app.stack = vec![View::Root, View::NormalizeAudio { file: anorm_file(), loudness, target: -16.0, range: 11.0, peak: -1.5 }];
}

fn achan_view(app: &mut App, in_channels: Option<u32>, stereo: bool) {
    app.stack = vec![View::Root, View::ChannelsAudio { file: anorm_file(), in_channels, stereo, left_db: 0.0, right_db: 0.0 }];
}

const ANORM_TARGET_BOX: (f32, f32) = (249.0, 173.0);
const ACHAN_LEFT_BOX: (f32, f32) = (214.0, 168.0);

#[test]
fn normalize_and_channels_views_render_in_both_themes() {
    let mut app = fixture();
    for theme in [egui::ThemePreference::Dark, egui::ThemePreference::Light] {
        app.ctx.set_theme(theme);
        for l in [None, Some(-31.0), Some(-9.0), Some(f64::NAN)] {
            anorm_view(&mut app, l);
            render(&mut app, vec![]);
            render(&mut app, vec![]);
            assert_eq!(app.view().title(), "Normalize Audio");
        }
        for (c, s) in [(None, false), (Some(1), false), (Some(1), true), (Some(2), true), (Some(2), false), (Some(6), true)] {
            achan_view(&mut app, c, s);
            render(&mut app, vec![]);
            render(&mut app, vec![]);
            assert_eq!(app.view().title(), "Audio Channels");
        }
    }
}

#[test]
fn measured_loudness_reaches_the_window_and_another_file_is_ignored() {
    let mut app = fixture();
    anorm_view(&mut app, None);
    render(&mut app, vec![]);
    app.bus.send(Msg::LoudnessMeasured(std::path::PathBuf::from("/nonexistent/other.mp3"), Some(-20.0)));
    app.drain();
    assert!(matches!(app.view(), View::NormalizeAudio { loudness: None, .. }), "a reply for another file is ignored");
    app.bus.send(Msg::LoudnessMeasured(anorm_file(), Some(-31.0)));
    app.drain();
    assert!(matches!(app.view(), View::NormalizeAudio { loudness: Some(l), .. } if *l == -31.0));
    render(&mut app, vec![]);
    // a failed measurement is shown, the window stays usable
    anorm_view(&mut app, None);
    app.bus.send(Msg::LoudnessMeasured(anorm_file(), None));
    app.drain();
    assert!(matches!(app.view(), View::NormalizeAudio { loudness: Some(l), .. } if l.is_nan()));
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("measure")), "{:?}", app.toast);
    render(&mut app, vec![]);
}

#[test]
fn probed_channel_count_reaches_the_window_and_another_file_is_ignored() {
    let mut app = fixture();
    achan_view(&mut app, None, true);
    app.bus.send(Msg::ChannelsProbed(std::path::PathBuf::from("/nonexistent/other.mp3"), Some(1)));
    app.drain();
    assert!(matches!(app.view(), View::ChannelsAudio { in_channels: None, .. }));
    app.bus.send(Msg::ChannelsProbed(anorm_file(), Some(1)));
    app.drain();
    assert!(matches!(app.view(), View::ChannelsAudio { in_channels: Some(1), .. }));
    render(&mut app, vec![]);
}

#[test]
fn normalize_and_channels_escape_goes_back_without_a_job() {
    let mut app = fixture();
    anorm_view(&mut app, Some(-31.0));
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::Root));
    assert!(app.rx.try_recv().is_err(), "Escape starts no job");
    achan_view(&mut app, Some(2), true);
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::Root));
    assert!(app.rx.try_recv().is_err(), "Escape starts no job");
}

#[test]
fn normalize_and_channels_enter_with_valid_input_dispatches_and_goes_back() {
    let mut app = fixture();
    app.tools.ffmpeg = Some("/nonexistent/ffmpeg-fc-test");
    anorm_view(&mut app, Some(-31.0));
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::Root), "the window closes once the job is dispatched");
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("Normaliz")), "{:?}", app.toast);
    app.toast = None;
    achan_view(&mut app, Some(2), false);
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::Root));
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("channel")), "{:?}", app.toast);
}

#[test]
fn normalize_and_channels_enter_with_invalid_input_starts_nothing() {
    let mut app = fixture();
    app.tools.ffmpeg = Some("/nonexistent/ffmpeg-fc-test");
    app.stack = vec![View::Root, View::NormalizeAudio { file: anorm_file(), loudness: None, target: -2.0, range: 11.0, peak: -1.5 }];
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::NormalizeAudio { .. }));
    assert!(app.rx.try_recv().is_err());
    app.stack =
        vec![View::Root, View::ChannelsAudio { file: anorm_file(), in_channels: Some(2), stereo: true, left_db: f64::NAN, right_db: 0.0 }];
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::ChannelsAudio { .. }));
    assert!(app.rx.try_recv().is_err());
}

// L74-style: Escape while a number box is edited ends the edit but keeps the window; Enter
// confirms the number and does not dispatch.
#[test]
fn normalize_target_box_keeps_escape_and_enter_while_editing() {
    let mut app = fixture();
    app.tools.ffmpeg = Some("/nonexistent/ffmpeg-fc-test");
    anorm_view(&mut app, Some(-31.0));
    render(&mut app, vec![]);
    click_at(&mut app, Pos2::new(ANORM_TARGET_BOX.0, ANORM_TARGET_BOX.1));
    assert!(app.ctx.egui_wants_keyboard_input(), "the click must have started editing the Target box (layout moved?)");
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::NormalizeAudio { .. }), "Enter confirms the number, it does not start the job");
    assert!(app.rx.try_recv().is_err());
    click_at(&mut app, Pos2::new(ANORM_TARGET_BOX.0, ANORM_TARGET_BOX.1));
    assert!(app.ctx.egui_wants_keyboard_input());
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::NormalizeAudio { .. }), "Escape ends the edit, it must not close the window");
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::Root), "a second Escape closes it");
}

#[test]
fn channels_left_box_keeps_escape_and_enter_while_editing() {
    let mut app = fixture();
    app.tools.ffmpeg = Some("/nonexistent/ffmpeg-fc-test");
    achan_view(&mut app, Some(2), true);
    render(&mut app, vec![]);
    click_at(&mut app, Pos2::new(ACHAN_LEFT_BOX.0, ACHAN_LEFT_BOX.1));
    assert!(app.ctx.egui_wants_keyboard_input(), "the click must have started editing the Left box (layout moved?)");
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::ChannelsAudio { .. }), "Enter confirms the number, it does not start the job");
    assert!(app.rx.try_recv().is_err());
    click_at(&mut app, Pos2::new(ACHAN_LEFT_BOX.0, ACHAN_LEFT_BOX.1));
    assert!(app.ctx.egui_wants_keyboard_input());
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::ChannelsAudio { .. }), "Escape ends the edit, it must not close the window");
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::Root), "a second Escape closes it");
}

#[test]
fn convert_view_offers_normalize_and_channels_for_one_audio_file_only() {
    let mut app = fixture();
    let dir = std::env::temp_dir().join(format!("fc-anormoffer-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let has = |app: &mut App, names: &[&str], title: &str| -> bool {
        let files: Vec<_> = names
            .iter()
            .map(|n| {
                let p = dir.join(n);
                std::fs::write(&p, b"x").unwrap();
                p
            })
            .collect();
        app.stack = vec![View::Root, View::Convert(files)];
        app.sections().iter().flat_map(|s| s.items.iter()).any(|i| {
            i.title == title
                && matches!(
                    (title, &i.kind),
                    ("Normalize…", Kind::Editor(Act::NormalizeAudio(_))) | ("Channels…", Kind::Editor(Act::AudioChannels(_)))
                )
        })
    };
    for title in ["Normalize…", "Channels…"] {
        assert!(has(&mut app, &["a.mp3"], title), "{title}");
        assert!(!has(&mut app, &["a.mp3", "b.mp3"], title), "{title}");
        assert!(!has(&mut app, &["a.mp4"], title), "{title}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tool_reach_normalize_and_channels_open_windows_for_one_file_and_keep_presets_for_several() {
    use ferrocast_core::convert::FileTool;
    use ferrocast_core::tools::{implemented, Impl, ToolId};
    let a = std::path::PathBuf::from("/nonexistent/a.mp3");
    let b = std::path::PathBuf::from("/nonexistent/b.mp3");
    assert!(matches!(tool_reach(ToolId::NormalizeAudio, std::slice::from_ref(&a)), ToolReach::Act(Act::NormalizeAudio(_))));
    assert!(matches!(tool_reach(ToolId::AudioChannels, std::slice::from_ref(&a)), ToolReach::Act(Act::AudioChannels(_))));
    assert!(matches!(tool_reach(ToolId::NormalizeAudio, &[a.clone(), b.clone()]), ToolReach::File(FileTool::Normalize(..))));
    assert!(matches!(tool_reach(ToolId::AudioChannels, &[a, b]), ToolReach::File(FileTool::Channels(..))));
    assert_eq!(implemented(ToolId::NormalizeAudio), Impl::Window);
    assert_eq!(implemented(ToolId::AudioChannels), Impl::Window);
}

#[test]
fn normalize_and_channels_without_ffmpeg_show_the_hint_and_open_no_view() {
    let mut app = fixture();
    app.tools = Default::default();
    let depth = app.stack.len();
    app.perform(Act::NormalizeAudio(anorm_file()), false);
    app.perform(Act::AudioChannels(anorm_file()), false);
    assert_eq!(app.stack.len(), depth);
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("ffmpeg")), "{:?}", app.toast);
}

#[test]
fn normalize_line_shows_measuring_the_delta_and_failure() {
    use crate::media_tools::normalize_line;
    assert_eq!(normalize_line(None, -16.0), "measuring…");
    assert_eq!(normalize_line(Some(-31.0), -16.0), "now -31.0 LUFS → target -16 LUFS (+15.0 dB)");
    assert_eq!(normalize_line(Some(-9.0), -14.5), "now -9.0 LUFS → target -14.5 LUFS (-5.5 dB)");
    assert!(normalize_line(Some(f64::NAN), -16.0).contains("could not be measured"));
}

// ---- A11c / A12a: redact window with several areas, reference frame ----

fn area(x: u32, y: u32, w: u32, h: u32, start: f64, end: f64, blur: bool) -> Area {
    Area { rect: ferrocast_core::croprect::Rect { x, y, w, h }, start, end, blur }
}

fn three_areas() -> Vec<Area> {
    vec![area(40, 30, 200, 120, 0.0, 6.0, false), area(300, 100, 150, 90, 1.0, 3.0, true), area(100, 200, 120, 80, 2.0, 5.5, false)]
}

fn redact_view(file: &str, duration: f64, areas: Vec<Area>, sel: usize) -> View {
    View::VideoRedact { file: file.into(), size: Some((640, 360)), aspect: None, duration, areas, sel, ref_t: 0.6 }
}

fn redact_app(areas: Vec<Area>, sel: usize) -> App {
    let mut app = fixture();
    app.stack = vec![View::Root, redact_view("/nonexistent/fc-redact.mp4", 6.0, areas, sel)];
    render(&mut app, vec![]);
    render(&mut app, vec![]);
    app
}

fn redact_areas(app: &App) -> (Vec<Area>, usize) {
    match app.view() {
        View::VideoRedact { areas, sel, .. } => (areas.clone(), *sel),
        other => panic!("redact window closed unexpectedly: {}", other.title()),
    }
}

fn click_at(app: &mut App, pos: Pos2) {
    let btn = |pressed| Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    render(app, vec![Event::PointerMoved(pos)]);
    render(app, vec![btn(true)]);
    render(app, vec![btn(false)]);
    render(app, vec![]);
}

// Layout of the redact window (content starts at y 58 pt): "+ Add area" is the first element of
// the left column, rows follow every 30 pt, each with a delete button at the right of the column.
const ADD_AREA_POS: (f32, f32) = (110.0, 79.0);
fn row_pos(i: usize) -> Pos2 {
    Pos2::new(90.0, 118.0 + 30.0 * i as f32)
}
fn delete_pos(i: usize) -> Pos2 {
    Pos2::new(row_pos(i).x + 126.0, row_pos(i).y)
}

#[test]
fn frame_time_for_the_reference_frame_never_leaves_the_clip() {
    assert_eq!(ferrocast_core::videoplan::frame_time(6.0, 99.0), 5.96);
}

#[test]
fn redact_job_builds_the_job_or_names_the_problem() {
    use ferrocast_core::convert::FileTool;
    let ok = redact_job(&three_areas(), 6.0, (640, 360)).unwrap();
    match ok {
        FileTool::RedactVideoMany(v) => {
            assert_eq!(v.len(), 3);
            assert_eq!((v[1].x, v[1].y, v[1].w, v[1].h), (300, 100, 150, 90));
            assert_eq!((v[1].start, v[1].end, v[1].blur), (1.0, 3.0, true));
        }
        other => panic!("{other:?}"),
    }
    assert!(
        matches!(redact_job(&[area(0, 0, 100, 100, 0.0, 6.0, false)], 6.0, (640, 360)), Ok(FileTool::RedactVideoMany(v)) if v.len() == 1)
    );
    let err = |a: Vec<Area>| redact_job(&a, 6.0, (640, 360)).unwrap_err();
    assert!(err(vec![area(0, 0, 100, 100, 5.0, 2.0, false)]).contains("area 1"), "reversed range");
    assert!(
        err(vec![area(0, 0, 100, 100, 0.0, 6.0, false), area(0, 0, 100, 100, 1.0, 9.0, false)]).contains("area 2"),
        "past the duration"
    );
    assert!(err(vec![area(600, 0, 100, 100, 0.0, 6.0, false)]).contains("outside"), "out of the frame");
    assert!(err(vec![area(0, 0, 100, 100, 0.0, 6.0, false); 65]).contains("too many"), "65 areas");
    assert!(redact_job(&vec![area(0, 0, 100, 100, 0.0, 6.0, false); 64], 6.0, (640, 360)).is_ok(), "64 areas are allowed");
    assert!(err(vec![]).contains("at least one"));
}

#[test]
fn redact_view_renders_probing_ready_and_with_a_bad_selection_in_both_themes() {
    for theme in [egui::ThemePreference::Dark, egui::ThemePreference::Light] {
        let mut app = fixture();
        app.ctx.set_theme(theme);
        let f = "/nonexistent/fc-redact-render.mp4";
        app.stack = vec![
            View::Root,
            View::VideoRedact { file: f.into(), size: None, aspect: None, duration: 0.0, areas: vec![], sel: 0, ref_t: 0.0 },
        ];
        render(&mut app, vec![]);
        for (areas, sel) in [(three_areas()[..1].to_vec(), 0), (three_areas(), 1), (three_areas(), 2), (three_areas(), 7), (vec![], 3)] {
            app.stack = vec![View::Root, redact_view(f, 6.0, areas, sel)];
            render(&mut app, vec![]);
            render(&mut app, vec![]);
            assert!(matches!(app.view(), View::VideoRedact { .. }));
        }
    }
}

#[test]
fn redact_escape_goes_back_and_starts_no_job() {
    let mut app = redact_app(three_areas(), 0);
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::Root));
    assert!(app.rx.try_recv().is_err(), "Escape starts no job");
}

#[test]
fn redact_enter_with_an_invalid_area_keeps_the_window_and_starts_nothing() {
    let mut areas = three_areas();
    areas[1].start = 4.0;
    areas[1].end = 1.0;
    let mut app = redact_app(areas.clone(), 0);
    render(&mut app, vec![key_event(Key::Enter)]);
    assert_eq!(redact_areas(&app).0, areas, "nothing edited");
    assert!(app.rx.try_recv().is_err(), "an invalid area must not start work");
}

#[test]
fn redact_add_area_button_adds_a_centred_third_covering_the_whole_clip() {
    let mut app = redact_app(three_areas()[..1].to_vec(), 0);
    click_at(&mut app, Pos2::new(ADD_AREA_POS.0, ADD_AREA_POS.1));
    let (areas, sel) = redact_areas(&app);
    assert_eq!(areas.len(), 2, "the click must have hit + Add area (layout moved?)");
    assert_eq!(sel, 1, "the new area is selected");
    assert_eq!(areas[1].rect, ferrocast_core::croprect::Rect { x: 214, y: 120, w: 213, h: 120 });
    assert_eq!((areas[1].start, areas[1].end, areas[1].blur), (0.0, 6.0, false));
}

#[test]
fn redact_add_area_stops_at_64() {
    let mut app = redact_app(vec![area(0, 0, 100, 100, 0.0, 6.0, false); 64], 0);
    click_at(&mut app, Pos2::new(ADD_AREA_POS.0, ADD_AREA_POS.1));
    assert_eq!(redact_areas(&app).0.len(), 64);
}

#[test]
fn redact_delete_removes_one_but_never_the_last() {
    let mut app = redact_app(three_areas(), 2);
    click_at(&mut app, delete_pos(0));
    let (areas, sel) = redact_areas(&app);
    assert_eq!(areas, three_areas()[1..].to_vec(), "the first area went away (layout moved?)");
    assert_eq!(sel, 1, "the selection follows its area");
    click_at(&mut app, delete_pos(0));
    assert_eq!(redact_areas(&app).0.len(), 1);
    click_at(&mut app, delete_pos(0));
    assert_eq!(redact_areas(&app).0.len(), 1, "the last area cannot be deleted");
}

#[test]
fn redact_clicking_a_row_selects_that_area() {
    let mut app = redact_app(three_areas(), 0);
    click_at(&mut app, row_pos(2));
    assert_eq!(redact_areas(&app).1, 2, "layout moved?");
}

// L74-style: Escape while the Start box is being edited ends the edit, not the window.
#[test]
fn redact_start_box_keeps_escape_while_editing() {
    let mut app = redact_app(three_areas(), 0);
    // the Start box is the first number box of the strip above the picture: find it by clicking
    // along the strip until the first click that starts an edit
    let (mut hit, mut hit_x) = (false, 0.0);
    for x in (260..560).step_by(4) {
        click_at(&mut app, Pos2::new(x as f32, 79.0));
        if app.ctx.egui_wants_keyboard_input() {
            (hit, hit_x) = (true, x as f32);
            break;
        }
    }
    assert!(hit, "a click must have started editing a number box (layout moved?)");
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::VideoRedact { .. }), "Escape ends the edit, it must not close the window");
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::Root), "a second Escape closes it");
    // that box is Start: typing a number into it and pressing Enter sets the selected area's start
    let mut app = redact_app(three_areas(), 1);
    click_at(&mut app, Pos2::new(hit_x, 79.0));
    assert!(app.ctx.egui_wants_keyboard_input());
    render(&mut app, vec![Event::Text("2.5".into())]);
    render(&mut app, vec![key_event(Key::Enter)]);
    render(&mut app, vec![]);
    let (areas, _) = redact_areas(&app);
    assert_eq!((areas[1].start, areas[1].end), (2.5, 3.0), "the first number box is Start of the selected area");
    assert_eq!((areas[0], areas[2]), (three_areas()[0], three_areas()[2]));
    assert!(app.rx.try_recv().is_err(), "Enter in the box starts no job");
}

#[test]
fn dragging_inside_the_selected_area_moves_only_that_area() {
    let mut app = redact_app(three_areas(), 1);
    let before = redact_areas(&app).0;
    // the 640x360 frame is drawn at about (307,182)-(675,389) pt (scale 0.575); the centre of area 2
    // (300,100 150x90 -> frame point 375,145) is at about (522,265): inside, far from every handle
    drag(&mut app, Pos2::new(522.0, 265.0), Pos2::new(552.0, 285.0));
    let after = redact_areas(&app).0;
    assert_ne!(after[1].rect, before[1].rect, "a drag inside the selected area must move it (layout moved?)");
    assert_eq!((after[1].rect.w, after[1].rect.h), (150, 90), "moving keeps the size");
    assert!(after[1].rect.x > 300 + 30 && after[1].rect.y > 100 + 15, "followed the pointer: {:?}", after[1].rect);
    assert_eq!((after[0], after[2]), (before[0], before[2]), "other areas untouched");
    assert_eq!(redact_areas(&app).1, 1, "the selection stays");
}

fn png_bytes() -> Vec<u8> {
    let mut v = Vec::new();
    image::DynamicImage::ImageRgba8(image::RgbaImage::new(8, 8))
        .write_to(&mut std::io::Cursor::new(&mut v), image::ImageFormat::Png)
        .unwrap();
    v
}

fn crop_tex_id(app: &App) -> Option<egui::TextureId> {
    app.ctx.data(|d| d.get_temp::<egui::TextureHandle>(Id::new("crop-tex"))).map(|t| t.id())
}

#[test]
fn reference_frame_reply_replaces_the_texture_only_for_this_file_and_the_newest_time() {
    let mut app = redact_app(three_areas(), 0);
    let file = std::path::PathBuf::from("/nonexistent/fc-redact.mp4");
    assert_eq!(crop_tex_id(&app), None);
    app.ctx.data_mut(|d| d.insert_temp(Id::new("ref-asked"), (file.clone(), 2.0f64)));
    // another file
    app.bus.send(Msg::RefFrame("/nonexistent/other.mp4".into(), 2.0, Some(png_bytes())));
    app.drain();
    assert_eq!(crop_tex_id(&app), None, "a reply for another file is ignored");
    // an older time
    app.bus.send(Msg::RefFrame(file.clone(), 1.0, Some(png_bytes())));
    app.drain();
    assert_eq!(crop_tex_id(&app), None, "a reply for an older time is ignored");
    // the one asked for
    app.bus.send(Msg::RefFrame(file, 2.0, Some(png_bytes())));
    app.drain();
    assert!(crop_tex_id(&app).is_some(), "the newest reply replaces the texture");
    let (areas, sel) = redact_areas(&app);
    assert_eq!((areas, sel), (three_areas(), 0), "rectangles are untouched");
}

#[test]
fn crop_ready_carries_the_duration_and_the_reference_frame_control_renders() {
    let mut app = fixture();
    let f = std::path::PathBuf::from("/nonexistent/fc-vcrop-ref.mp4");
    app.stack = vec![
        View::Root,
        View::VideoCrop {
            file: f.clone(),
            size: None,
            rect: ferrocast_core::croprect::full(2, 2),
            aspect: None,
            duration: 0.0,
            ref_t: 0.0,
        },
    ];
    app.bus.send(Msg::VideoCropReady("/nonexistent/other.mp4".into(), Some(((640, 360), 6.0, png_bytes()))));
    app.drain();
    assert!(matches!(app.view(), View::VideoCrop { size: None, .. }), "another file is ignored");
    app.bus.send(Msg::VideoCropReady(f, Some(((640, 360), 6.0, png_bytes()))));
    app.drain();
    match app.view() {
        View::VideoCrop { size: Some((640, 360)), duration, ref_t, .. } => {
            assert_eq!(*duration, 6.0);
            assert!((*ref_t - 0.6).abs() < 1e-9, "the first frame is at 10% of the clip");
        }
        _ => panic!("crop window not ready"),
    }
    for theme in [egui::ThemePreference::Dark, egui::ThemePreference::Light] {
        app.ctx.set_theme(theme);
        render(&mut app, vec![]);
    }
}

// ---- A10g: audio to video window

fn avis_file() -> std::path::PathBuf {
    std::env::temp_dir().join("fc-avis-view.mp3")
}

fn avis_view(app: &mut App, orient: ferrocast_core::media::Canvas, style: crate::media_tools::VisStyle, image: Option<std::path::PathBuf>) {
    app.stack = vec![View::Root, View::VisualizeAudio { file: avis_file(), orient, style, image, picking: false }];
}

fn avis_state(app: &App) -> (ferrocast_core::media::Canvas, crate::media_tools::VisStyle, Option<std::path::PathBuf>, bool) {
    match app.view() {
        View::VisualizeAudio { orient, style, image, picking, .. } => (*orient, *style, image.clone(), *picking),
        other => panic!("visualizer closed unexpectedly: {}", other.title()),
    }
}

/// A real file with a picture extension (the job only checks that it is a file with that extension).
fn avis_picture(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("fc-avis-pic-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, b"not really a picture").unwrap();
    p
}

/// Centres of the five segmented buttons, recorded by the window while it draws:
/// Waveform, Still image, Landscape, Portrait, Square.
fn avis_buttons(app: &App) -> [Pos2; 5] {
    let r: [Rect; 5] = app.ctx.data(|d| d.get_temp(Id::new("avis-rects"))).expect("the window records its buttons");
    r.map(|r| r.center())
}

#[test]
fn visualizer_view_renders_in_both_themes_with_and_without_a_picture() {
    use crate::media_tools::VisStyle;
    use ferrocast_core::media::Canvas;
    let pic = avis_picture("cover.png");
    let mut app = fixture();
    for theme in [egui::ThemePreference::Dark, egui::ThemePreference::Light] {
        app.ctx.set_theme(theme);
        for (style, image, picking) in [
            (VisStyle::Waveform, None, false),
            (VisStyle::Still, None, false),
            (VisStyle::Still, Some(pic.clone()), false),
            (VisStyle::Still, Some(std::path::PathBuf::from("/nonexistent/gone.png")), false),
            (VisStyle::Still, None, true),
        ] {
            for c in [Canvas::Landscape, Canvas::Portrait, Canvas::Square] {
                app.stack = vec![View::Root, View::VisualizeAudio { file: avis_file(), orient: c, style, image: image.clone(), picking }];
                render(&mut app, vec![]);
                render(&mut app, vec![]);
                assert_eq!(app.view().title(), "Audio to Video");
            }
        }
    }
}

#[test]
fn chosen_picture_reaches_the_window_and_another_file_is_ignored() {
    use crate::media_tools::VisStyle;
    use ferrocast_core::media::Canvas;
    let mut app = fixture();
    avis_view(&mut app, Canvas::Landscape, VisStyle::Still, None);
    if let Some(View::VisualizeAudio { picking, .. }) = app.stack.last_mut() {
        *picking = true;
    }
    render(&mut app, vec![]);
    let pic = std::path::PathBuf::from("/nonexistent/other-cover.png");
    app.bus.send(Msg::VisualPicked(std::path::PathBuf::from("/nonexistent/other.mp3"), Some(pic.clone())));
    app.drain();
    assert_eq!(avis_state(&app), (Canvas::Landscape, VisStyle::Still, None, true), "a reply for another file is ignored");
    app.bus.send(Msg::VisualPicked(avis_file(), Some(pic.clone())));
    app.drain();
    assert_eq!(avis_state(&app), (Canvas::Landscape, VisStyle::Still, Some(pic.clone()), false));
    // a cancelled dialog keeps the earlier choice and ends the picking
    if let Some(View::VisualizeAudio { picking, .. }) = app.stack.last_mut() {
        *picking = true;
    }
    app.bus.send(Msg::VisualPicked(avis_file(), None));
    app.drain();
    assert_eq!(avis_state(&app), (Canvas::Landscape, VisStyle::Still, Some(pic), false));
    render(&mut app, vec![]);
}

#[test]
fn visualizer_escape_goes_back_without_a_job() {
    use crate::media_tools::VisStyle;
    let mut app = fixture();
    avis_view(&mut app, ferrocast_core::media::Canvas::Square, VisStyle::Waveform, None);
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::Root));
    assert!(app.rx.try_recv().is_err(), "Escape starts no job");
}

#[test]
fn visualizer_enter_with_still_image_and_no_picture_refuses_and_starts_nothing() {
    use crate::media_tools::VisStyle;
    use ferrocast_core::media::Canvas;
    let mut app = fixture();
    app.tools.ffmpeg = Some("/nonexistent/ffmpeg-fc-test");
    avis_view(&mut app, Canvas::Portrait, VisStyle::Still, None);
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::VisualizeAudio { .. }), "the window stays");
    assert!(app.toast.as_ref().is_some_and(|t| t.0.to_lowercase().contains("picture")), "{:?}", app.toast);
    assert!(app.rx.try_recv().is_err(), "nothing started");
    // a chosen picture that no longer exists refuses too
    avis_view(&mut app, Canvas::Portrait, VisStyle::Still, Some("/nonexistent/gone.png".into()));
    app.toast = None;
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::VisualizeAudio { .. }));
    assert!(app.toast.is_some());
    assert!(app.rx.try_recv().is_err());
}

#[test]
fn visualizer_enter_with_a_valid_job_dispatches_and_goes_back() {
    use crate::media_tools::VisStyle;
    use ferrocast_core::media::Canvas;
    let mut app = fixture();
    app.tools.ffmpeg = Some("/nonexistent/ffmpeg-fc-test");
    avis_view(&mut app, Canvas::Portrait, VisStyle::Waveform, None);
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::Root), "the window closes once the job is dispatched");
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("visualiz") || t.0.contains("video")), "{:?}", app.toast);
    // Still image with a real picture file
    let pic = avis_picture("cover2.jpg");
    avis_view(&mut app, Canvas::Square, VisStyle::Still, Some(pic));
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::Root));
    // without ffmpeg the window stays and says why (the hint)
    app.tools.ffmpeg = None;
    avis_view(&mut app, Canvas::Landscape, VisStyle::Waveform, None);
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::VisualizeAudio { .. }));
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("ffmpeg")), "{:?}", app.toast);
}

#[test]
fn visualizer_buttons_change_orientation_and_style() {
    use crate::media_tools::VisStyle;
    use ferrocast_core::media::Canvas;
    let mut app = fixture();
    avis_view(&mut app, Canvas::Landscape, VisStyle::Waveform, None);
    render(&mut app, vec![]);
    render(&mut app, vec![]);
    let b = avis_buttons(&app);
    click_at(&mut app, b[3]);
    assert_eq!(avis_state(&app).0, Canvas::Portrait, "Portrait button");
    click_at(&mut app, b[4]);
    assert_eq!(avis_state(&app).0, Canvas::Square, "Square button");
    click_at(&mut app, b[2]);
    assert_eq!(avis_state(&app).0, Canvas::Landscape, "Landscape button");
    click_at(&mut app, b[1]);
    assert_eq!(avis_state(&app).1, VisStyle::Still, "Still image button");
    // the layout may shift when the picture row appears: read the positions again
    render(&mut app, vec![]);
    let b = avis_buttons(&app);
    click_at(&mut app, b[0]);
    assert_eq!(avis_state(&app).1, VisStyle::Waveform, "Waveform button");
    assert!(app.rx.try_recv().is_err(), "clicking a segment starts nothing");
}

#[test]
fn convert_view_offers_audio_to_video_for_one_audio_file_only() {
    let mut app = fixture();
    let dir = std::env::temp_dir().join(format!("fc-avisoffer-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let has = |app: &mut App, names: &[&str]| -> bool {
        let files: Vec<_> = names
            .iter()
            .map(|n| {
                let p = dir.join(n);
                std::fs::write(&p, b"x").unwrap();
                p
            })
            .collect();
        app.stack = vec![View::Root, View::Convert(files)];
        app.sections()
            .iter()
            .flat_map(|s| s.items.iter())
            .any(|i| i.title == "Audio to Video…" && matches!(&i.kind, Kind::Editor(Act::VisualizeAudio(_))))
    };
    assert!(has(&mut app, &["a.mp3"]));
    assert!(has(&mut app, &["a.wav"]));
    assert!(!has(&mut app, &["a.mp3", "b.mp3"]));
    assert!(!has(&mut app, &["a.mp4"]));
    assert!(!has(&mut app, &["a.png"]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tool_reach_audio_to_video_opens_the_window_for_one_audio_file() {
    use ferrocast_core::convert::FileTool;
    use ferrocast_core::tools::{implemented, Impl, ToolId};
    let a = std::path::PathBuf::from("/nonexistent/a.mp3");
    let b = std::path::PathBuf::from("/nonexistent/b.mp3");
    assert!(matches!(tool_reach(ToolId::AudioToVideo, std::slice::from_ref(&a)), ToolReach::Act(Act::VisualizeAudio(_))));
    assert!(matches!(tool_reach(ToolId::AudioToVideo, &[a, b]), ToolReach::File(FileTool::Visualize(_))));
    assert_eq!(implemented(ToolId::AudioToVideo), Impl::Window);
}

#[test]
fn visualizer_without_ffmpeg_shows_the_hint_and_opens_no_view() {
    let mut app = fixture();
    app.tools = Default::default();
    let depth = app.stack.len();
    app.perform(Act::VisualizeAudio(avis_file()), false);
    assert_eq!(app.stack.len(), depth);
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("ffmpeg")), "{:?}", app.toast);
    app.tools.ffmpeg = Some("/nonexistent/ffmpeg-fc-test");
    app.perform(Act::VisualizeAudio(avis_file()), false);
    assert!(matches!(app.view(), View::VisualizeAudio { picking: false, image: None, .. }));
}
