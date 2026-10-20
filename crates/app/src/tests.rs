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
    app.stack = vec![View::Root, View::VideoCrop { file: f.clone(), size: None, rect: ferrocast_core::croprect::full(2, 2), aspect: None }];
    render(&mut app, vec![]);
    let r = ferrocast_core::croprect::Rect { x: 10, y: 10, w: 100, h: 60 };
    app.stack = vec![View::Root, View::VideoCrop { file: f, size: Some((640, 360)), rect: r, aspect: None }];
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
