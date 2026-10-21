use super::*;

/// Hooks that touch nothing: Accessibility is not granted, no tap can start, the selection is
/// empty and nothing is re-posted. Tests that exercise Enter-on-Finder replace them.
fn inert_finder_hooks() -> services::FinderHooks {
    services::FinderHooks {
        trusted: Arc::new(|_| false),
        start: Arc::new(|_, _| Err("inert".into())),
        selection: Arc::new(|| Ok(vec![])),
        repost: Arc::new(|| false),
    }
}

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
        finder: services::FinderEnterState { hooks: inert_finder_hooks(), ..Default::default() },
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
fn actions_wheel_has_the_six_actions_in_order_plus_zip_last() {
    use ferrocast_core::wheel::WheelKind;
    let app = fixture();
    let one = vec![std::path::PathBuf::from("/nonexistent/a.png")];
    let many = vec![one[0].clone(), std::path::PathBuf::from("/nonexistent/b.png")];
    let folder = vec![std::env::temp_dir()];
    let unknown = vec![std::path::PathBuf::from("/nonexistent/data.qzx9")];
    let noext = vec![std::path::PathBuf::from("/nonexistent/Makefile")];
    for files in [one, many, folder, unknown, noext] {
        let segs = app.wheel_segments(WheelKind::Actions, &files);
        let labels: Vec<String> = segs.iter().map(|s| s.label.clone()).collect();
        assert_eq!(labels, ["AirDrop", "Float", "Copy", "Move to…", "Copy Path", "Terminal", "Zip"], "{files:?}");
        match &segs[6].pick {
            WheelPick::Act(Act::ZipFiles(z)) => assert_eq!(z, &files),
            _ => panic!("Zip segment must carry Act::ZipFiles"),
        }
        assert_eq!(segs[6].symbol, "sf:doc.zipper");
        assert!(segs[6].needs.is_none());
    }
    assert!(app.wheel_segments(WheelKind::Actions, &[]).is_empty());
}

#[test]
fn zip_of_a_vanished_file_toasts_and_starts_nothing() {
    let mut app = fixture();
    let gone = std::env::temp_dir().join(format!("fc-zip-gone-{}.txt", uuid::Uuid::new_v4()));
    app.perform(Act::ZipFiles(vec![gone]), true);
    assert!(app.toast.is_some(), "must toast");
    assert!(app.rx.try_recv().is_err(), "no job may start");
}

#[test]
fn zip_target_zips_any_kind_of_item_next_to_it() {
    let dir = std::env::temp_dir().join(format!("fc-zip-real-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let png = dir.join("pic.png");
    std::fs::write(&png, b"\x89PNG\r\n\x1a\nfake").unwrap();
    let txt = dir.join("note.txt");
    std::fs::write(&txt, b"hello").unwrap();
    let noext = dir.join("Makefile");
    std::fs::write(&noext, b"all:").unwrap();
    let acc = dir.join("Informe año ñandú final.txt");
    std::fs::write(&acc, b"hola").unwrap();
    let folder = dir.join("My Folder");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("inner.txt"), b"inner").unwrap();
    let tools = Default::default();
    let t = crate::app::zip_target();
    for (item, member) in [
        (&png, "pic.png"),
        (&txt, "note.txt"),
        (&noext, "Makefile"),
        (&acc, "Informe año ñandú final.txt"),
        (&folder, "My Folder/inner.txt"),
    ] {
        let before = std::fs::read(item).ok();
        let (made, errs) = crate::services::run_conversion(std::slice::from_ref(item), &t, &tools);
        assert!(errs.is_empty(), "{errs:?}");
        // output_path swaps the extension: pic.png -> pic.zip, Makefile -> Makefile.zip
        let name = item.file_stem().unwrap().to_string_lossy().to_string();
        let expect = dir.join(format!("{name}.zip"));
        assert_eq!(made, std::slice::from_ref(&expect));
        assert!(std::fs::read(&expect).unwrap().starts_with(b"PK"));
        let out = std::process::Command::new("unzip").arg("-l").arg(&expect).output().unwrap();
        let listing = String::from_utf8_lossy(&out.stdout).to_string();
        assert!(out.status.success() && listing.contains(member), "{listing}");
        assert_eq!(std::fs::read(item).ok(), before, "original untouched");
        // a second run never overwrites
        let (again, errs) = crate::services::run_conversion(std::slice::from_ref(item), &t, &tools);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(again, [dir.join(format!("{name} 2.zip"))]);
    }
    let _ = std::fs::remove_dir_all(&dir);
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

// ---- wheel animation (B3/B4) -----------------------------------------------------------------

/// Frames recorded by `draw_wheel` in a test build: (highlight of the selected segment, open scale,
/// open alpha). egui's clock advances 1/60 s per `render` call, so the tests drive it deterministically.
fn wheel_probe(app: &App) -> Vec<(f32, f32, f32)> {
    app.ctx.data(|d| d.get_temp(Id::new("wheel-anim-probe"))).unwrap_or_default()
}

fn wheel_with_selection(app: &mut App, kind: ferrocast_core::wheel::WheelKind, n: usize, sel: Option<usize>, reduce: bool) {
    let files = vec![std::path::PathBuf::from("/nonexistent/a.png")];
    let segs: Vec<WheelSeg> = (0..n)
        .map(|i| WheelSeg { label: format!("Option {i}"), symbol: "sf:photo", needs: None, pick: WheelPick::Act(Act::Run) })
        .collect();
    app.ctx.data_mut(|d| {
        d.insert_temp(Id::new("wheel_reduce_motion"), reduce);
        d.remove::<Vec<(f32, f32, f32)>>(Id::new("wheel-anim-probe"));
        d.remove::<super::WheelAnim>(Id::new("wheel_anim"));
    });
    app.stack = vec![View::Root, View::Wheel { files, center: Pos2::new(-9999.0, -9999.0), segs, kind, sel }];
}

#[test]
fn wheel_highlight_eases_in_when_motion_is_allowed() {
    let mut app = fixture();
    wheel_with_selection(&mut app, ferrocast_core::wheel::WheelKind::Tools, 7, Some(2), false);
    for _ in 0..30 {
        render(&mut app, vec![]);
    }
    let frames = wheel_probe(&app);
    assert_eq!(frames.len(), 30);
    let first = frames[0].0;
    assert!(first > 0.0 && first < 1.0, "first frame must be mid-transition, got {first}");
    assert!(frames.windows(2).all(|w| w[1].0 >= w[0].0), "highlight must never go backwards: {frames:?}");
    assert!(frames[4].0 < 1.0, "120 ms is about 7 frames, frame 5 is still moving: {}", frames[4].0);
    assert_eq!(frames[29].0, 1.0, "0.5 s later the highlight is complete");
    // open animation: starts small and transparent, ends exactly at 1
    assert!(frames[0].1 >= 0.92 && frames[0].1 < 1.0 && frames[0].2 < 1.0, "open starts mid-way: {:?}", frames[0]);
    assert_eq!((frames[29].1, frames[29].2), (1.0, 1.0));
}

#[test]
fn wheel_reduce_motion_jumps_to_the_end_state_but_still_highlights() {
    let mut app = fixture();
    wheel_with_selection(&mut app, ferrocast_core::wheel::WheelKind::Tools, 7, Some(2), true);
    render(&mut app, vec![]);
    let frames = wheel_probe(&app);
    assert_eq!(frames[0], (1.0, 1.0, 1.0), "highlight, open scale and open alpha are final on the first frame");
    // with nothing selected the highlight is off, and a selection change is instant
    wheel_with_selection(&mut app, ferrocast_core::wheel::WheelKind::Tools, 7, None, true);
    render(&mut app, vec![]);
    assert_eq!(wheel_probe(&app)[0].0, 0.0);
    if let Some(View::Wheel { sel, .. }) = app.stack.last_mut() {
        *sel = Some(4);
    }
    render(&mut app, vec![]);
    assert_eq!(wheel_probe(&app)[1].0, 1.0, "the highlight still changes, instantly");
}

#[test]
fn wheel_renders_through_open_switch_and_hover_animations_in_both_themes() {
    use ferrocast_core::wheel::WheelKind;
    for reduce in [false, true] {
        for theme in [egui::ThemePreference::Dark, egui::ThemePreference::Light] {
            for n in [1usize, 7, 9, 12] {
                let mut app = fixture();
                app.ctx.set_theme(theme);
                wheel_with_selection(&mut app, WheelKind::Convert, n, None, reduce);
                for f in 0..24 {
                    // step the selection, and tap Command half-way (a switch to the tools wheel)
                    if let Some(View::Wheel { sel, .. }) = app.stack.last_mut() {
                        *sel = Some(f % n);
                    }
                    if f == 12 {
                        render_mods(&mut app, vec![], Modifiers { command: true, ..Modifiers::NONE });
                    } else {
                        render(&mut app, vec![]);
                    }
                }
                assert!(matches!(app.view(), View::Wheel { .. }), "n={n} reduce={reduce}: the wheel must still be open");
                assert_eq!(wheel_probe(&app).len(), 24);
            }
        }
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
    assert_eq!(wheel_state(&app).unwrap().2, 7);
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
    let pos = Pos2::new(325.0, 93.0);
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
    app.stack = vec![View::Root, View::FrameImage { file: f.clone(), size: (100, 80), opts: Default::default(), color: 4, picking: false }];
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
    app.stack = vec![
        View::Root,
        View::RedactImage { file: f.clone(), size: (100, 80), areas: vec![r], sel: Some(0), blur: true, radius: 24, color: 5 },
    ];
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
const COMPRESS_KB_BOX: (f32, f32) = (616.0, 180.0);

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

const COMPRESS_MEDIA_KB_BOX: (f32, f32) = (616.0, 180.0);

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

const ANORM_TARGET_BOX: (f32, f32) = (607.0, 105.0);
const ACHAN_LEFT_BOX: (f32, f32) = (619.0, 158.0);

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

// ---- A13: image backdrop in the background window, blur radius and deselect in the redaction window

fn a13_dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("fc-a13-{}-{}", std::process::id(), name));
    let _ = std::fs::create_dir_all(&d);
    d
}

fn frame_app(backdrop: ferrocast_core::frame::Backdrop) -> App {
    let mut app = fixture();
    let opts = ferrocast_core::frame::FrameOpts { pad: 10, radius: 4, shadow: 6, backdrop };
    app.ctx.data_mut(|d| {
        d.insert_temp(Id::new("frame-base"), std::sync::Arc::new(image::RgbaImage::from_pixel(60, 40, image::Rgba([10, 200, 30, 255]))))
    });
    app.stack = vec![
        View::Root,
        View::FrameImage { file: std::path::PathBuf::from("/nonexistent/a.png"), size: (60, 40), opts, color: 4, picking: false },
    ];
    app
}

fn frame_state(app: &App) -> (ferrocast_core::frame::Backdrop, bool) {
    match app.view() {
        View::FrameImage { opts, picking, .. } => (opts.backdrop.clone(), *picking),
        other => panic!("background window closed unexpectedly: {}", other.title()),
    }
}

#[test]
fn frame_preview_with_an_image_backdrop_renders_in_both_themes_without_panicking() {
    use ferrocast_core::frame::Backdrop;
    let d = a13_dir("render");
    let garbage = d.join("garbage.png");
    std::fs::write(&garbage, "not an image").unwrap();
    let real = d.join("real.png");
    image::RgbaImage::from_pixel(32, 32, image::Rgba([250, 0, 0, 255])).save(&real).unwrap();
    for theme in [egui::ThemePreference::Dark, egui::ThemePreference::Light] {
        for path in
            [String::new(), "/nonexistent/bg.png".to_string(), garbage.to_string_lossy().to_string(), real.to_string_lossy().to_string()]
        {
            let mut app = frame_app(Backdrop::Image(path.clone()));
            app.ctx.set_theme(theme);
            if path.ends_with("real.png") {
                let bg = image::RgbaImage::from_pixel(32, 32, image::Rgba([250, 0, 0, 255]));
                app.ctx.data_mut(|d| d.insert_temp(Id::new("frame-bg"), std::sync::Arc::new((path.clone(), bg))));
            }
            render(&mut app, vec![]);
            render(&mut app, vec![]);
            assert!(matches!(app.view(), View::FrameImage { .. }));
            assert!(app.ctx.data(|d| d.get_temp::<egui::TextureHandle>(Id::new("frame-tex"))).is_some(), "a preview is drawn for {path:?}");
        }
    }
}

#[test]
fn frame_preview_cover_scales_the_chosen_picture_under_the_photo() {
    use ferrocast_core::frame::{Backdrop, FrameOpts};
    let photo = image::RgbaImage::from_pixel(60, 40, image::Rgba([10, 200, 30, 255]));
    let bg = image::RgbaImage::from_pixel(20, 50, image::Rgba([250, 0, 0, 255]));
    let o = FrameOpts { pad: 10, radius: 0, shadow: 0, backdrop: Backdrop::Image("x".into()) };
    let out = services::frame_compose(&photo, Some(&bg), &o);
    assert_eq!((out.width(), out.height()), (80, 60));
    assert_eq!(out.get_pixel(0, 0).0, [250, 0, 0, 255], "the picture covers the whole canvas");
    assert_eq!(out.get_pixel(79, 59).0, [250, 0, 0, 255]);
    assert_eq!(out.get_pixel(40, 30).0, [10, 200, 30, 255], "the photo sits on top");
    let none = services::frame_compose(&photo, None, &o);
    assert_eq!(none.get_pixel(0, 0).0, [255, 255, 255, 255], "no picture yet: white, no panic");
}

#[test]
fn chosen_backdrop_picture_reaches_the_frame_window_and_another_file_is_ignored() {
    use ferrocast_core::frame::Backdrop;
    let mut app = frame_app(Backdrop::Color([1, 2, 3]));
    if let Some(View::FrameImage { picking, .. }) = app.stack.last_mut() {
        *picking = true;
    }
    render(&mut app, vec![]);
    let pic = std::path::PathBuf::from("/nonexistent/other-bg.png");
    app.bus.send(Msg::FramePicked(std::path::PathBuf::from("/nonexistent/other.png"), Some(pic.clone())));
    app.drain();
    assert_eq!(frame_state(&app), (Backdrop::Color([1, 2, 3]), true), "a reply for another file is ignored");
    app.bus.send(Msg::FramePicked(std::path::PathBuf::from("/nonexistent/a.png"), Some(pic.clone())));
    app.drain();
    assert_eq!(frame_state(&app), (Backdrop::Image(pic.to_string_lossy().to_string()), false));
    // a cancelled dialog keeps the earlier choice and ends the picking
    if let Some(View::FrameImage { picking, .. }) = app.stack.last_mut() {
        *picking = true;
    }
    app.bus.send(Msg::FramePicked(std::path::PathBuf::from("/nonexistent/a.png"), None));
    app.drain();
    assert_eq!(frame_state(&app), (Backdrop::Image(pic.to_string_lossy().to_string()), false));
    render(&mut app, vec![]);
}

#[test]
fn unreadable_backdrop_reply_clears_the_choice_with_a_toast() {
    use ferrocast_core::frame::Backdrop;
    let mut app = frame_app(Backdrop::Image("/nonexistent/bg.png".into()));
    app.toast = None;
    app.bus.send(Msg::FrameBgLoaded(std::path::PathBuf::from("/nonexistent/a.png"), "/nonexistent/bg.png".into(), None));
    app.drain();
    assert_eq!(frame_state(&app).0, Backdrop::Image(String::new()));
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("could not read")), "{:?}", app.toast);
    // a late reply for a picture that is no longer the backdrop changes nothing
    app.toast = None;
    app.bus.send(Msg::FrameBgLoaded(std::path::PathBuf::from("/nonexistent/a.png"), "/nonexistent/old.png".into(), None));
    app.drain();
    assert!(app.toast.is_none());
}

#[test]
fn enter_with_an_image_backdrop_and_no_readable_picture_refuses_and_starts_nothing() {
    use ferrocast_core::frame::Backdrop;
    let d = a13_dir("enter");
    let garbage = d.join("garbage.png");
    std::fs::write(&garbage, "not an image").unwrap();
    for path in [String::new(), "/nonexistent/bg.png".to_string(), garbage.to_string_lossy().to_string()] {
        let mut app = frame_app(Backdrop::Image(path.clone()));
        render(&mut app, vec![]);
        app.toast = None;
        render(&mut app, vec![key_event(Key::Enter)]);
        assert!(matches!(app.view(), View::FrameImage { .. }), "the window stays open for {path:?}");
        assert!(app.toast.is_some(), "a toast names the problem for {path:?}");
        assert!(app.rx.try_recv().is_err(), "no job starts for {path:?}");
    }
}

#[test]
fn frame_job_builds_the_op_for_every_backdrop_or_names_the_problem() {
    use ferrocast_core::convert::ImageOp;
    use ferrocast_core::frame::{Backdrop, FrameOpts};
    let d = a13_dir("job");
    let real = d.join("bg.png");
    image::RgbaImage::from_pixel(16, 16, image::Rgba([0, 0, 255, 255])).save(&real).unwrap();
    let with = |b| FrameOpts { backdrop: b, ..Default::default() };
    for b in [Backdrop::Color([1, 2, 3]), Backdrop::Blur(30.0), Backdrop::Image(real.to_string_lossy().to_string())] {
        match frame_job(&with(b.clone())) {
            Ok(ImageOp::FrameWith(o)) => assert_eq!(o.backdrop, b),
            other => panic!("{other:?}"),
        }
    }
    assert!(frame_job(&with(Backdrop::Image(String::new()))).unwrap_err().contains("choose a picture"));
    assert!(frame_job(&with(Backdrop::Image("/nonexistent/x.png".into()))).unwrap_err().contains("not found"));
}

#[test]
fn frame_job_with_an_image_backdrop_runs_through_the_executor() {
    use ferrocast_core::frame::{Backdrop, FrameOpts};
    let d = a13_dir("exec");
    let (src, bg) = (d.join("photo.png"), d.join("bg.png"));
    image::RgbaImage::from_pixel(64, 48, image::Rgba([0, 255, 0, 255])).save(&src).unwrap();
    image::RgbaImage::from_pixel(40, 40, image::Rgba([255, 0, 0, 255])).save(&bg).unwrap();
    let opts = FrameOpts { pad: 20, radius: 0, shadow: 0, backdrop: Backdrop::Image(bg.to_string_lossy().to_string()) };
    let op = frame_job(&opts).unwrap();
    let (outs, errs) = services::run_image_op(std::slice::from_ref(&src), &op);
    assert!(errs.is_empty(), "{errs:?}");
    let img = image::open(&outs[0]).unwrap().to_rgba8();
    assert_eq!((img.width(), img.height()), (104, 88));
    assert_eq!(img.get_pixel(2, 2).0, [255, 0, 0, 255], "the chosen picture is the backdrop");
    assert_eq!(img.get_pixel(52, 44).0, [0, 255, 0, 255], "the photo is on top");
}

#[test]
fn frame_window_keeps_colour_and_blur_options() {
    use ferrocast_core::frame::Backdrop;
    for b in [Backdrop::Color([255, 255, 255]), Backdrop::Blur(30.0)] {
        let mut app = frame_app(b.clone());
        render(&mut app, vec![]);
        render(&mut app, vec![]);
        assert_eq!(frame_state(&app), (b, false));
    }
}

// ---- redaction window

fn redact_img_app(blur: bool, radius: u32, areas: Vec<ferrocast_core::croprect::Rect>, sel: Option<usize>) -> App {
    let mut app = fixture();
    app.stack = vec![
        View::Root,
        View::RedactImage { file: std::path::PathBuf::from("/nonexistent/r.png"), size: (1000, 600), areas, sel, blur, radius, color: 5 },
    ];
    render(&mut app, vec![]);
    render(&mut app, vec![]);
    app
}

fn redact_img_state(app: &App) -> (Vec<ferrocast_core::croprect::Rect>, Option<usize>, bool, u32) {
    match app.view() {
        View::RedactImage { areas, sel, blur, radius, .. } => (areas.clone(), *sel, *blur, *radius),
        other => panic!("redact window closed unexpectedly: {}", other.title()),
    }
}

/// Screen position of an image pixel in the redaction window (rect recorded by the drawing code).
fn redact_pos(app: &App, size: (u32, u32), x: f32, y: f32) -> Pos2 {
    let r: Rect = app.ctx.data(|d| d.get_temp(Id::new("redact-img-rect"))).expect("the redact canvas was drawn");
    Pos2::new(r.min.x + x / size.0 as f32 * r.width(), r.min.y + y / size.1 as f32 * r.height())
}

fn two_redact_areas() -> Vec<ferrocast_core::croprect::Rect> {
    vec![
        ferrocast_core::croprect::Rect { x: 100, y: 100, w: 200, h: 150 },
        ferrocast_core::croprect::Rect { x: 600, y: 300, w: 200, h: 150 },
    ]
}

#[test]
fn redact_job_image_carries_the_blur_radius_into_the_spec() {
    use ferrocast_core::convert::ImageOp;
    use ferrocast_core::redact::{Source, Style};
    let areas = two_redact_areas();
    for (radius, want) in [(2, 2.0), (24, 24.0), (80, 80.0), (500, 80.0), (0, 2.0)] {
        match redact_job_image(&areas, true, radius, 5).unwrap() {
            ImageOp::RedactWith(spec) => {
                assert_eq!(spec.style, Style::Blur(want), "radius {radius}");
                assert!(matches!(spec.source, Source::Areas(ref a) if a.len() == 2 && a[0].x == 100 && a[1].w == 200));
            }
            other => panic!("{other:?}"),
        }
    }
    // solid mode ignores the radius
    match redact_job_image(&areas, false, 77, 0).unwrap() {
        ImageOp::RedactWith(spec) => assert!(matches!(spec.style, Style::Solid(_))),
        other => panic!("{other:?}"),
    }
    assert!(redact_job_image(&[], true, 24, 5).unwrap_err().contains("Nothing to redact"));
    // what the typed query `redact ... blur 15` produces is what the slider produces
    let typed = ferrocast_core::redact::parse("redact 100,100 200x150 blur 15").unwrap();
    match redact_job_image(&areas[..1], true, 15, 5).unwrap() {
        ImageOp::RedactWith(spec) => assert_eq!(spec, typed),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_larger_blur_radius_blurs_more_in_a_real_file() {
    let d = a13_dir("blur");
    let src = d.join("edge.png");
    // left half black, right half white: a hard vertical edge at x = 100
    let img =
        image::RgbaImage::from_fn(200, 100, |x, _| if x < 100 { image::Rgba([0, 0, 0, 255]) } else { image::Rgba([255, 255, 255, 255]) });
    img.save(&src).unwrap();
    let area = [ferrocast_core::croprect::Rect { x: 60, y: 20, w: 80, h: 60 }];
    // brightness step between two pixels that straddle the edge, at the area's middle row
    let step = |radius: u32| {
        let op = redact_job_image(&area, true, radius, 5).unwrap();
        let (outs, errs) = services::run_image_op(std::slice::from_ref(&src), &op);
        assert!(errs.is_empty(), "{errs:?}");
        let out = image::open(&outs[0]).unwrap().to_rgba8();
        let _ = std::fs::remove_file(&outs[0]);
        let (a, b) = (out.get_pixel(90, 50).0[0] as i32, out.get_pixel(110, 50).0[0] as i32);
        b - a
    };
    let (small, large) = (step(2), step(30));
    assert!(small > 150, "a small radius keeps the edge sharp ({small})");
    assert!(large < small - 60, "a larger radius flattens the edge more ({large} vs {small})");
}

#[test]
fn redact_view_with_the_blur_slider_renders_in_both_themes() {
    for theme in [egui::ThemePreference::Dark, egui::ThemePreference::Light] {
        for (blur, radius) in [(true, 24), (true, 2), (true, 80), (false, 24)] {
            let mut app = fixture();
            app.ctx.set_theme(theme);
            app.stack = vec![
                View::Root,
                View::RedactImage {
                    file: "/nonexistent/r.png".into(),
                    size: (1000, 600),
                    areas: two_redact_areas(),
                    sel: Some(0),
                    blur,
                    radius,
                    color: 5,
                },
            ];
            render(&mut app, vec![]);
            render(&mut app, vec![]);
            assert_eq!(redact_img_state(&app).2, blur);
        }
    }
}

#[test]
fn clicking_empty_space_deselects_and_clicking_an_area_selects_it() {
    let size = (1000, 600);
    let mut app = redact_img_app(false, 24, two_redact_areas(), Some(0));
    // inside the second area (far from its edges)
    let inside = redact_pos(&app, size, 700.0, 375.0);
    click_at(&mut app, inside);
    assert_eq!(redact_img_state(&app).1, Some(1), "a click on an area selects it");
    // empty picture space
    let empty = redact_pos(&app, size, 450.0, 520.0);
    click_at(&mut app, empty);
    let (areas, sel, ..) = redact_img_state(&app);
    assert_eq!(sel, None, "a click on empty space clears the selection");
    assert_eq!(areas, two_redact_areas(), "and changes no area");
    let again = redact_pos(&app, size, 200.0, 175.0);
    click_at(&mut app, again);
    assert_eq!(redact_img_state(&app).1, Some(0));
}

#[test]
fn dragging_on_empty_space_still_creates_an_area() {
    let size = (1000, 600);
    let mut app = redact_img_app(false, 24, two_redact_areas(), Some(0));
    let (from, to) = (redact_pos(&app, size, 400.0, 480.0), redact_pos(&app, size, 500.0, 560.0));
    drag(&mut app, from, to);
    let (areas, sel, ..) = redact_img_state(&app);
    assert_eq!(areas.len(), 3, "a new area was added");
    assert_eq!(sel, Some(2), "and selected");
    assert!(areas[2].w > 40 && areas[2].h > 30, "{:?}", areas[2]);
}

#[test]
fn deleting_an_area_during_a_drag_does_not_move_another_one() {
    let size = (1000, 600);
    let mut app = redact_img_app(false, 24, two_redact_areas(), None);
    let first = two_redact_areas()[0];
    // start dragging the first area, then delete it while the pointer is still down
    let from = redact_pos(&app, size, 200.0, 175.0);
    let btn = |pos, pressed| Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    render(&mut app, vec![Event::PointerMoved(from)]);
    render(&mut app, vec![btn(from, true)]);
    let mid = from + Vec2::new(10.0, 6.0);
    render(&mut app, vec![Event::PointerMoved(mid)]);
    assert_eq!(redact_img_state(&app).1, Some(0), "the drag grabbed the first area");
    render(&mut app, vec![key_event(Key::Delete)]);
    let (areas, ..) = redact_img_state(&app);
    assert_eq!(areas, vec![two_redact_areas()[1]], "the first area is gone");
    let far = from + Vec2::new(80.0, 50.0);
    render(&mut app, vec![Event::PointerMoved(far)]);
    render(&mut app, vec![btn(far, false)]);
    render(&mut app, vec![]);
    let (areas, ..) = redact_img_state(&app);
    assert_eq!(areas, vec![two_redact_areas()[1]], "the surviving area was not dragged ({first:?} was deleted)");
}

/// Click the blur slider's value box by scanning the strip from the right (guarded by an assertion).
fn click_redact_blur_box(app: &mut App) {
    for x in (200..600).rev().step_by(6) {
        click_at(app, Pos2::new(x as f32, 103.0));
        if app.ctx.egui_wants_keyboard_input() {
            return;
        }
    }
    panic!("the blur value box was not found (layout moved?)");
}

// L74-style: Escape while the blur box is edited ends the edit but keeps the window.
#[test]
fn redact_blur_box_keeps_enter_and_escape_while_editing() {
    let areas = two_redact_areas();
    let mut app = redact_img_app(true, 24, areas.clone(), Some(0));
    click_redact_blur_box(&mut app);
    assert!(app.ctx.egui_wants_keyboard_input());
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::RedactImage { .. }), "Escape ends the edit, it must not close the window");
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::Root), "a second Escape closes the window");
    let mut app = redact_img_app(true, 24, areas.clone(), Some(0));
    click_redact_blur_box(&mut app);
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::RedactImage { .. }), "Enter confirms the number, it must not save");
    assert!(app.rx.try_recv().is_err(), "no job starts while editing");
    // Backspace in the box must not delete the selected area
    let mut app = redact_img_app(true, 24, areas.clone(), Some(0));
    click_redact_blur_box(&mut app);
    render(&mut app, vec![key_event(Key::Backspace)]);
    assert_eq!(redact_img_state(&app).0, areas, "Backspace edits the number, not the areas");
}

#[test]
fn redact_enter_without_areas_refuses_and_with_areas_saves() {
    let mut app = redact_img_app(true, 40, vec![], None);
    app.toast = None;
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::RedactImage { .. }));
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("Nothing to redact")));
    let mut app = redact_img_app(true, 40, two_redact_areas(), Some(0));
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::Root), "Enter with areas saves and leaves");
}

// ---- A10f: bleep window (several ranges)

fn bleep_ranges3() -> Vec<(f64, f64)> {
    vec![(1.0, 2.0), (3.0, 3.5), (4.5, 5.5)]
}

fn bleep_peaks() -> Option<Arc<Vec<(i16, i16)>>> {
    Some(Arc::new((0..400).map(|i| (-(i % 50) as i16 * 300, (i % 50) as i16 * 300)).collect()))
}

fn bleep_view(ranges: Vec<(f64, f64)>, sel: usize) -> View {
    View::BleepAudio { file: "/nonexistent/fc-bleep.mp3".into(), duration: 6.0, peaks: bleep_peaks(), ranges, sel }
}

fn bleep_app(ranges: Vec<(f64, f64)>, sel: usize) -> App {
    let mut app = fixture();
    app.stack = vec![View::Root, bleep_view(ranges, sel)];
    render(&mut app, vec![]);
    render(&mut app, vec![]);
    app
}

fn bleep_state(app: &App) -> (Vec<(f64, f64)>, usize) {
    match app.view() {
        View::BleepAudio { ranges, sel, .. } => (ranges.clone(), *sel),
        other => panic!("bleep window closed unexpectedly: {}", other.title()),
    }
}

#[test]
fn bleep_job_builds_the_job_or_names_the_problem() {
    use ferrocast_core::convert::FileTool;
    assert!(matches!(bleep_job(&bleep_ranges3(), 6.0), Ok(FileTool::BleepMany(v)) if v == bleep_ranges3()));
    assert!(matches!(bleep_job(&[(2.0, 3.0), (1.0, 1.5)], 6.0), Ok(FileTool::BleepMany(v)) if v == vec![(1.0, 1.5), (2.0, 3.0)]));
    assert!(bleep_job(&[(1.0, 2.0), (4.0, 3.0)], 6.0).unwrap_err().contains("range 2"));
    assert!(bleep_job(&[(1.0, 7.0)], 6.0).unwrap_err().contains("range 1"));
    assert!(bleep_job(&[], 6.0).is_err());
}

#[test]
fn bleep_view_renders_probing_ready_and_with_a_bad_selection_in_both_themes() {
    for theme in [egui::ThemePreference::Dark, egui::ThemePreference::Light] {
        let mut app = fixture();
        app.ctx.set_theme(theme);
        app.stack = vec![
            View::Root,
            View::BleepAudio { file: "/nonexistent/fc-bleep.mp3".into(), duration: 0.0, peaks: None, ranges: vec![], sel: 0 },
        ];
        render(&mut app, vec![]);
        assert_eq!(app.view().title(), "Bleep Audio");
        for (ranges, sel) in [(vec![(1.0, 2.0)], 0), (bleep_ranges3(), 1), (bleep_ranges3(), 2), (bleep_ranges3(), 9), (vec![], 3)] {
            app.stack = vec![View::Root, bleep_view(ranges, sel)];
            render(&mut app, vec![]);
            render(&mut app, vec![]);
            assert!(matches!(app.view(), View::BleepAudio { .. }));
        }
    }
}

#[test]
fn bleep_escape_goes_back_and_starts_no_job() {
    let mut app = bleep_app(bleep_ranges3(), 0);
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::Root));
    assert!(app.rx.try_recv().is_err(), "Escape starts no job");
}

#[test]
fn bleep_enter_with_an_invalid_range_keeps_the_window_and_starts_nothing() {
    let ranges = vec![(1.0, 2.0), (4.0, 3.0)];
    let mut app = bleep_app(ranges.clone(), 0);
    render(&mut app, vec![key_event(Key::Enter)]);
    assert_eq!(bleep_state(&app).0, ranges, "nothing edited");
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("range 2")), "{:?}", app.toast);
    assert!(app.rx.try_recv().is_err(), "an invalid range must not start work");
}

#[test]
fn bleep_add_range_button_adds_a_one_second_range_at_the_centre() {
    let mut app = bleep_app(vec![(1.0, 2.0)], 0);
    click_at(&mut app, Pos2::new(ADD_AREA_POS.0, ADD_AREA_POS.1));
    let (ranges, sel) = bleep_state(&app);
    assert_eq!(ranges, vec![(1.0, 2.0), (2.5, 3.5)], "the click must have hit + Add range (layout moved?)");
    assert_eq!(sel, 1, "the new range is selected");
}

#[test]
fn bleep_default_range_is_clamped_to_short_clips() {
    assert_eq!(new_bleep_range(6.0), (2.5, 3.5));
    assert_eq!(new_bleep_range(0.6), (0.0, 0.6));
    assert_eq!(new_bleep_range(1.0), (0.0, 1.0));
}

#[test]
fn bleep_add_range_stops_at_32() {
    let many: Vec<(f64, f64)> = (0..32).map(|i| (i as f64 * 0.1, i as f64 * 0.1 + 0.05)).collect();
    let mut app = bleep_app(many, 0);
    click_at(&mut app, Pos2::new(ADD_AREA_POS.0, ADD_AREA_POS.1));
    assert_eq!(bleep_state(&app).0.len(), 32);
}

#[test]
fn bleep_delete_removes_one_but_never_the_last() {
    let mut app = bleep_app(bleep_ranges3(), 2);
    click_at(&mut app, delete_pos(0));
    let (ranges, sel) = bleep_state(&app);
    assert_eq!(ranges, bleep_ranges3()[1..].to_vec(), "the first range went away (layout moved?)");
    assert_eq!(sel, 1, "the selection follows its range");
    click_at(&mut app, delete_pos(0));
    assert_eq!(bleep_state(&app).0.len(), 1);
    click_at(&mut app, delete_pos(0));
    assert_eq!(bleep_state(&app).0.len(), 1, "the last range cannot be deleted");
}

#[test]
fn bleep_clicking_a_row_selects_that_range() {
    let mut app = bleep_app(bleep_ranges3(), 0);
    click_at(&mut app, row_pos(2));
    assert_eq!(bleep_state(&app).1, 2, "layout moved?");
}

// L74-style: Escape while the Start box is being edited ends the edit, not the window.
#[test]
fn bleep_start_box_keeps_escape_and_enter_while_editing() {
    let mut app = bleep_app(bleep_ranges3(), 1);
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
    assert!(matches!(app.view(), View::BleepAudio { .. }), "Escape ends the edit, it must not close the window");
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::Root), "a second Escape closes it");
    let mut app = bleep_app(bleep_ranges3(), 1);
    click_at(&mut app, Pos2::new(hit_x, 79.0));
    assert!(app.ctx.egui_wants_keyboard_input());
    render(&mut app, vec![Event::Text("2.5".into())]);
    render(&mut app, vec![key_event(Key::Enter)]);
    render(&mut app, vec![]);
    let (ranges, _) = bleep_state(&app);
    assert_eq!(ranges[1], (2.5, 3.5), "the first number box is Start of the selected range");
    assert!(app.rx.try_recv().is_err(), "Enter in the box starts no job");
}

#[test]
fn convert_view_offers_bleep_for_one_audio_file_only() {
    let mut app = fixture();
    let dir = std::env::temp_dir().join(format!("fc-bleepoffer-{}", std::process::id()));
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
            .any(|i| i.title == "Bleep…" && matches!(i.kind, Kind::Editor(Act::BleepAudio(_))))
    };
    assert!(has(&mut app, &["a.mp3"]));
    assert!(!has(&mut app, &["a.mp3", "b.mp3"]));
    assert!(!has(&mut app, &["a.png"]));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tool_reach_bleep_opens_the_window_for_one_audio_file() {
    use ferrocast_core::tools::{implemented, Impl, ToolId};
    let a = std::path::PathBuf::from("/nonexistent/a.mp3");
    let b = std::path::PathBuf::from("/nonexistent/b.mp3");
    assert!(matches!(tool_reach(ToolId::RedactAudio, std::slice::from_ref(&a)), ToolReach::Act(Act::BleepAudio(_))));
    assert!(matches!(tool_reach(ToolId::RedactAudio, &[a, b]), ToolReach::Open));
    assert_eq!(implemented(ToolId::RedactAudio), Impl::Window);
}

#[test]
fn bleep_without_ffmpeg_shows_the_hint_and_opens_no_view() {
    let mut app = fixture();
    app.tools = Default::default();
    let depth = app.stack.len();
    app.perform(Act::BleepAudio("/nonexistent/fc-bleep.mp3".into()), false);
    assert_eq!(app.stack.len(), depth);
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("ffmpeg")), "{:?}", app.toast);
}

#[test]
fn bleep_enter_with_valid_ranges_dispatches_and_goes_back() {
    let mut app = bleep_app(bleep_ranges3(), 0);
    app.tools.ffmpeg = Some("/nonexistent/ffmpeg");
    render(&mut app, vec![key_event(Key::Enter)]);
    assert!(matches!(app.view(), View::Root), "a valid job closes the window");
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("Bleeping")), "{:?}", app.toast);
}

// ---- A8c: drag to reorder (Choose Order list and collage strip)

fn pointer_button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
}

/// Press at `from`, then walk to `to` in steps (egui needs a drag threshold and a frame after the
/// press before a drag starts). The button stays down on return.
fn drag_begin(app: &mut App, from: Pos2, to: Pos2) {
    render(app, vec![Event::PointerMoved(from)]);
    render(app, vec![pointer_button(from, true)]);
    render(app, vec![]);
    for k in 1..=4 {
        let p = from + (to - from) * (k as f32 / 4.0) + Vec2::new(0.0, if k == 1 { 8.0 } else { 0.0 });
        render(app, vec![Event::PointerMoved(p)]);
    }
    render(app, vec![Event::PointerMoved(to)]);
}

fn drag_release(app: &mut App, at: Pos2) {
    render(app, vec![pointer_button(at, false)]);
    render(app, vec![]);
}

fn dnd_rects(app: &App, id: &str) -> Vec<Rect> {
    app.ctx.data(|d| d.get_temp::<Vec<Rect>>(Id::new(("dnd-rects", id)))).unwrap_or_default()
}

fn reorder_rows(app: &mut App, n: usize) -> Vec<Rect> {
    open_reorder(app, ReorderPurpose::MergePdf, reorder_files(n));
    render(app, vec![]);
    render(app, vec![]);
    dnd_rects(app, "reorder-rows")
}

#[test]
fn drag_reorder_renders_for_empty_1_2_5_files_in_both_themes() {
    let mut app = fixture();
    for theme in [egui::ThemePreference::Dark, egui::ThemePreference::Light] {
        app.ctx.set_theme(theme);
        for n in [0, 1, 2, 5] {
            let rows = reorder_rows(&mut app, n);
            assert_eq!(rows.len(), n);
            let imgs: Vec<image::RgbaImage> = (0..n).map(|_| image::RgbaImage::new(8, 8)).collect();
            app.ctx.data_mut(|d| d.insert_temp(Id::new("collage-base"), Arc::new(imgs)));
            app.stack = vec![View::Root, View::CollageImage { files: reorder_files(n), opts: Default::default(), color: 0 }];
            render(&mut app, vec![]);
            render(&mut app, vec![]);
        }
    }
}

#[test]
fn dragging_a_row_past_another_moves_it_and_the_cursor_follows() {
    let mut app = fixture();
    let f = reorder_files(5);
    let rows = reorder_rows(&mut app, 5);
    let (from, to) = (rows[0].center() - Vec2::new(60.0, 0.0), rows[3].center() + Vec2::new(-60.0, 10.0));
    drag_begin(&mut app, from, to);
    assert_eq!(reorder_state(&app).0, f, "nothing moves until the release");
    drag_release(&mut app, to);
    let (files, cursor) = reorder_state(&app);
    assert_eq!(files, vec![f[1].clone(), f[2].clone(), f[3].clone(), f[0].clone(), f[4].clone()], "the row ends where it was dropped");
    assert_eq!(cursor, 3, "the cursor follows the moved row");
}

#[test]
fn dragging_a_row_to_the_top_and_to_the_end_of_the_list() {
    let mut app = fixture();
    let f = reorder_files(4);
    let rows = reorder_rows(&mut app, 4);
    drag_begin(&mut app, rows[2].center() - Vec2::new(60.0, 0.0), rows[0].min + Vec2::new(100.0, 3.0));
    drag_release(&mut app, rows[0].min + Vec2::new(100.0, 3.0));
    assert_eq!(reorder_state(&app), (vec![f[2].clone(), f[0].clone(), f[1].clone(), f[3].clone()], 0));
    let rows = dnd_rects(&app, "reorder-rows");
    let end = rows[3].max - Vec2::new(100.0, 3.0);
    drag_begin(&mut app, rows[0].center() - Vec2::new(60.0, 0.0), end);
    drag_release(&mut app, end);
    assert_eq!(reorder_state(&app), (vec![f[0].clone(), f[1].clone(), f[3].clone(), f[2].clone()], 3));
}

#[test]
fn releasing_where_the_row_was_changes_nothing() {
    let mut app = fixture();
    let f = reorder_files(4);
    let rows = reorder_rows(&mut app, 4);
    let at = rows[1].center() - Vec2::new(60.0, 0.0);
    drag_begin(&mut app, at, at + Vec2::new(0.0, 12.0));
    drag_release(&mut app, at + Vec2::new(0.0, 6.0));
    assert_eq!(reorder_state(&app), (f, 0));
}

#[test]
fn releasing_outside_the_list_cancels_the_drag() {
    let mut app = fixture();
    let f = reorder_files(4);
    for outside in [Pos2::new(ui::WIDTH + 60.0, 200.0), Pos2::new(60.0, 8.0), Pos2::new(60.0, ui::HEIGHT - 2.0)] {
        let rows = reorder_rows(&mut app, 4);
        drag_begin(&mut app, rows[0].center() - Vec2::new(60.0, 0.0), outside);
        drag_release(&mut app, outside);
        assert_eq!(reorder_state(&app), (f.clone(), 0), "released at {outside:?}");
    }
}

#[test]
fn escape_during_a_drag_cancels_it_and_does_not_leave_the_view() {
    let mut app = fixture();
    let f = reorder_files(4);
    let rows = reorder_rows(&mut app, 4);
    let to = rows[3].center() + Vec2::new(-60.0, 10.0);
    drag_begin(&mut app, rows[0].center() - Vec2::new(60.0, 0.0), to);
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::Reorder { .. }), "the first Escape only cancels the drag");
    render(&mut app, vec![Event::PointerMoved(to + Vec2::new(0.0, 3.0))]);
    drag_release(&mut app, to);
    assert_eq!(reorder_state(&app), (f, 0), "the order is unchanged after the cancelled drag");
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::Root), "a second Escape leaves as before");
}

#[test]
fn row_buttons_and_clicks_still_work_next_to_the_drag_source() {
    let mut app = fixture();
    let f = reorder_files(3);
    let rows = reorder_rows(&mut app, 3);
    // click the "down" button of row 0 (right side), then select row 2 by clicking its name
    let down = Pos2::new(rows[0].max.x - 50.0 + 32.0, rows[0].center().y);
    render(&mut app, vec![Event::PointerMoved(down)]);
    render(&mut app, vec![pointer_button(down, true)]);
    render(&mut app, vec![]);
    render(&mut app, vec![pointer_button(down, false)]);
    assert_eq!(reorder_state(&app), (vec![f[1].clone(), f[0].clone(), f[2].clone()], 1));
    let name = rows[2].center() - Vec2::new(60.0, 0.0);
    render(&mut app, vec![Event::PointerMoved(name)]);
    render(&mut app, vec![pointer_button(name, true)]);
    render(&mut app, vec![pointer_button(name, false)]);
    assert_eq!(reorder_state(&app).1, 2, "a plain click selects the row");
}

fn collage_view(app: &mut App, n: usize) {
    let imgs: Vec<image::RgbaImage> = (0..n).map(|i| image::RgbaImage::new(10 + i as u32, 8)).collect();
    app.ctx.data_mut(|d| {
        d.insert_temp(Id::new("collage-base"), Arc::new(imgs));
        d.remove::<Vec<Rect>>(Id::new(("dnd-rects", "collage-strip")));
    });
    app.stack = vec![View::Root, View::CollageImage { files: reorder_files(n), opts: Default::default(), color: 0 }];
    render(app, vec![]);
    render(app, vec![]);
}

fn collage_state(app: &App) -> (Vec<std::path::PathBuf>, Vec<u32>) {
    let widths = app
        .ctx
        .data(|d| d.get_temp::<Arc<Vec<image::RgbaImage>>>(Id::new("collage-base")))
        .map(|b| b.iter().map(|i| i.width()).collect())
        .unwrap_or_default();
    match app.view() {
        View::CollageImage { files, .. } => (files.clone(), widths),
        other => panic!("collage closed unexpectedly: {}", other.title()),
    }
}

#[test]
fn dragging_a_collage_picture_reorders_the_files_and_the_loaded_images() {
    let mut app = fixture();
    let f = reorder_files(3);
    collage_view(&mut app, 3);
    let chips = dnd_rects(&app, "collage-strip");
    assert_eq!(chips.len(), 3);
    let to = chips[2].max - Vec2::new(3.0, 8.0);
    let tex_before = app.ctx.data(|d| d.get_temp::<egui::TextureHandle>(Id::new("collage-tex"))).map(|t| t.id());
    drag_begin(&mut app, chips[0].center(), to);
    drag_release(&mut app, to);
    let (files, widths) = collage_state(&app);
    assert_eq!(files, vec![f[1].clone(), f[2].clone(), f[0].clone()], "the order Enter hands to the collage job");
    assert_eq!(widths, vec![11, 12, 10], "the pictures the preview composes follow the same order");
    let tex_after = app.ctx.data(|d| d.get_temp::<egui::TextureHandle>(Id::new("collage-tex"))).map(|t| t.id());
    assert!(tex_before.is_some() && tex_after.is_some() && tex_before != tex_after, "the preview recomposes");
}

#[test]
fn collage_drag_released_outside_or_escaped_changes_nothing_and_one_picture_has_no_strip() {
    let mut app = fixture();
    let f = reorder_files(3);
    collage_view(&mut app, 3);
    let chips = dnd_rects(&app, "collage-strip");
    drag_begin(&mut app, chips[0].center(), Pos2::new(ui::WIDTH + 40.0, 300.0));
    drag_release(&mut app, Pos2::new(ui::WIDTH + 40.0, 300.0));
    assert_eq!(collage_state(&app), (f.clone(), vec![10, 11, 12]));
    let to = chips[2].max - Vec2::new(3.0, 8.0);
    drag_begin(&mut app, chips[0].center(), to);
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::CollageImage { .. }));
    drag_release(&mut app, to);
    assert_eq!(collage_state(&app), (f, vec![10, 11, 12]));
    collage_view(&mut app, 1);
    assert!(dnd_rects(&app, "collage-strip").is_empty(), "a single picture shows no strip");
}

#[test]
fn collage_thumbnails_that_arrive_after_a_reorder_follow_the_current_order() {
    let p = |s: &str| std::path::PathBuf::from(s);
    let loaded = vec![p("a"), p("b"), p("c")];
    let now = vec![p("c"), p("a"), p("b")];
    assert_eq!(align_to_current(&now, &loaded, vec![1, 2, 3]), Some(vec![3, 1, 2]));
    let dup_now = vec![p("a"), p("a"), p("b")];
    assert_eq!(align_to_current(&dup_now, &[p("a"), p("b"), p("a")], vec![1, 2, 3]), Some(vec![1, 3, 2]));
    assert_eq!(align_to_current(&[p("a"), p("x")], &[p("a"), p("b")], vec![1, 2]), None, "a different set of files is stale");
    assert_eq!(align_to_current(&now, &loaded, vec![1, 2]), None, "an image missing");
}

// ---- A8c part 2: drag to reorder the PDF organizer's thumbnails

fn open_organizer(app: &mut App, plan: ferrocast_core::pdfplan::PdfPlan) -> Vec<Rect> {
    let f = std::env::temp_dir().join(format!("fc-orgdrag-{}.pdf", std::process::id()));
    let n = plan.pages.iter().map(|p| p.src + 1).max().unwrap_or(0);
    let want = plan.pages.len();
    app.ctx.data_mut(|d| d.remove::<Vec<Rect>>(Id::new(("dnd-rects", "pdf-strip"))));
    app.stack = vec![View::Root, View::PdfOrganize { file: f, n, plan }];
    render(app, vec![]);
    render(app, vec![]);
    let rects = dnd_rects(app, "pdf-strip");
    assert_eq!(rects.len(), want, "one draggable cell per page entry");
    rects
}

fn page_srcs(app: &App) -> Vec<usize> {
    organizer_plan(app).pages.iter().map(|p| p.src).collect()
}

/// Where to release to drop the page at `from` on top of the one at `to`: the half of the target
/// that makes the gap after it (moving right) or before it (moving left).
fn drop_on(rects: &[Rect], from: usize, to: usize) -> Pos2 {
    rects[to].center() + Vec2::new(if to > from { 30.0 } else { -30.0 }, 0.0)
}

#[test]
fn pdf_organizer_renders_with_marks_in_both_themes_for_1_3_and_9_pages() {
    use ferrocast_core::pdfplan::PdfPlan;
    let mut app = fixture();
    for theme in [egui::ThemePreference::Dark, egui::ThemePreference::Light] {
        app.ctx.set_theme(theme);
        for n in [1, 3, 9] {
            let mut plan = PdfPlan::new(n);
            plan.pages[0].turns = 1; // a rotated mark
            if n > 1 {
                plan.pages[n - 1].turns = 3;
                plan.cursor = 1;
                plan.remove(); // a removed page leaves a gap in the source numbers
                plan.cursor = 0;
            }
            let want = plan.pages.len();
            let rects = open_organizer(&mut app, plan);
            assert_eq!(rects.len(), want, "{n} pages: one draggable cell per page entry");
            assert!(rects.iter().all(|r| r.width() > 50.0 && r.height() > 50.0));
            assert!(matches!(app.view(), View::PdfOrganize { .. }));
        }
    }
}

#[test]
fn dragging_a_thumbnail_to_the_front_middle_and_end_moves_the_page() {
    use ferrocast_core::pdfplan::PdfPlan;
    let mut app = fixture();
    // to the end
    let rects = open_organizer(&mut app, PdfPlan::new(4));
    let to = drop_on(&rects, 0, 3);
    drag_begin(&mut app, rects[0].center(), to);
    assert_eq!(page_srcs(&app), [0, 1, 2, 3], "nothing moves until the release");
    drag_release(&mut app, to);
    assert_eq!(page_srcs(&app), [1, 2, 3, 0], "first page dropped on the end");
    assert_eq!(organizer_plan(&app).cursor, 3, "the selection follows the moved page");
    // to the front
    let rects = open_organizer(&mut app, PdfPlan::new(4));
    let to = drop_on(&rects, 3, 0);
    drag_begin(&mut app, rects[3].center(), to);
    drag_release(&mut app, to);
    assert_eq!(page_srcs(&app), [3, 0, 1, 2], "last page dropped on the front");
    assert_eq!(organizer_plan(&app).cursor, 0);
    // to the middle
    let rects = open_organizer(&mut app, PdfPlan::new(4));
    let to = drop_on(&rects, 0, 2);
    drag_begin(&mut app, rects[0].center(), to);
    drag_release(&mut app, to);
    assert_eq!(page_srcs(&app), [1, 2, 0, 3], "page 1 dropped after page 3");
    assert_eq!(organizer_plan(&app).cursor, 2);
}

#[test]
fn dragging_a_thumbnail_gives_the_same_plan_as_the_keyboard_move() {
    use ferrocast_core::pdfplan::PdfPlan;
    let mut app = fixture();
    let rects = open_organizer(&mut app, PdfPlan::new(4));
    let to = drop_on(&rects, 0, 2);
    drag_begin(&mut app, rects[0].center(), to);
    drag_release(&mut app, to);
    let dragged = organizer_plan(&app);
    let mut kb = open_organizer(&mut app, PdfPlan::new(4));
    kb.clear();
    render(&mut app, vec![key_mod_event(Key::ArrowRight, Modifiers::COMMAND)]);
    render(&mut app, vec![key_mod_event(Key::ArrowRight, Modifiers::COMMAND)]);
    let keyed = organizer_plan(&app);
    assert_eq!(dragged, keyed, "two ⌘→ presses and a two-place drag are the same edit");
    assert_eq!(dragged.selection(), [(1, 0), (2, 0), (0, 0), (3, 0)], "what the writer gets");
}

#[test]
fn rotation_and_removal_marks_travel_with_the_dragged_page() {
    use ferrocast_core::pdfplan::{Page, PdfPlan};
    let mut app = fixture();
    let mut plan = PdfPlan::new(5);
    plan.pages[1].turns = 1;
    plan.pages[3].turns = 2;
    plan.cursor = 2;
    plan.remove(); // source page 3 is gone: entries are 0,1,3,4
    plan.cursor = 0;
    assert_eq!(plan.selection(), [(0, 0), (1, 1), (3, 2), (4, 0)]);
    let rects = open_organizer(&mut app, plan);
    let to = drop_on(&rects, 1, 3);
    drag_begin(&mut app, rects[1].center(), to);
    drag_release(&mut app, to);
    let p = organizer_plan(&app);
    assert_eq!(p.selection(), [(0, 0), (3, 2), (4, 0), (1, 1)], "each entry keeps its own rotation");
    assert_eq!(p.pages[3], Page { src: 1, turns: 1 });
    assert_eq!(p.pages.len(), 4, "the removed page stays removed");
    assert_eq!(p.cursor, 3);
}

#[test]
fn organizer_release_where_it_was_or_outside_or_after_escape_changes_nothing() {
    use ferrocast_core::pdfplan::PdfPlan;
    let mut app = fixture();
    let rects = open_organizer(&mut app, PdfPlan::new(4));
    let at = rects[1].center();
    // released where it was
    drag_begin(&mut app, at, at + Vec2::new(12.0, 0.0));
    drag_release(&mut app, at + Vec2::new(6.0, 0.0));
    assert_eq!(page_srcs(&app), [0, 1, 2, 3]);
    // released outside the strip
    for outside in [Pos2::new(ui::WIDTH + 60.0, 200.0), Pos2::new(60.0, 8.0), Pos2::new(60.0, ui::HEIGHT - 2.0)] {
        let rects = open_organizer(&mut app, PdfPlan::new(4));
        drag_begin(&mut app, rects[0].center(), outside);
        drag_release(&mut app, outside);
        assert_eq!(page_srcs(&app), [0, 1, 2, 3], "released at {outside:?}");
        assert_eq!(organizer_plan(&app).cursor, 0);
    }
    // Escape mid-drag: the first Escape only cancels the drag, the second leaves
    let rects = open_organizer(&mut app, PdfPlan::new(4));
    let to = drop_on(&rects, 0, 3);
    drag_begin(&mut app, rects[0].center(), to);
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::PdfOrganize { .. }), "the first Escape only cancels the drag");
    render(&mut app, vec![Event::PointerMoved(to + Vec2::new(0.0, 3.0))]);
    drag_release(&mut app, to);
    assert_eq!(page_srcs(&app), [0, 1, 2, 3], "the plan is unchanged after the cancelled drag");
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::Root), "a second Escape leaves as before");
}

#[test]
fn organizer_click_still_selects_a_page_next_to_the_drag_source() {
    use ferrocast_core::pdfplan::PdfPlan;
    let mut app = fixture();
    let rects = open_organizer(&mut app, PdfPlan::new(4));
    let at = rects[2].center();
    render(&mut app, vec![Event::PointerMoved(at)]);
    render(&mut app, vec![pointer_button(at, true)]);
    render(&mut app, vec![pointer_button(at, false)]);
    assert_eq!(organizer_plan(&app).cursor, 2, "a plain click selects the page");
    assert_eq!(page_srcs(&app), [0, 1, 2, 3]);
}

// ---- U2: key hints live in the footer chips, not in a grey sentence in the body

const BACK: (&str, &str) = ("Back", "esc");

/// Every tool window with its expected footer chips (reading order) and the information-only notes
/// its body may still draw.
type Chips = Vec<(&'static str, &'static str)>;

fn u2_windows() -> Vec<(&'static str, View, Chips, Vec<String>)> {
    use crate::media_tools::VisStyle;
    use ferrocast_core::media::Canvas;
    let png = std::path::PathBuf::from("/nonexistent/u2.png");
    let mp4 = std::path::PathBuf::from("/nonexistent/u2.mp4");
    let r = ferrocast_core::croprect::Rect { x: 5, y: 5, w: 20, h: 20 };
    let area = Area { rect: ferrocast_core::croprect::full(64, 36), start: 0.0, end: 2.0, blur: false };
    let trim = ferrocast_core::videoplan::Trim::new(12.0, 25.0);
    let jump_bar = || vec!["click the bar to jump".to_string()];
    let s = |t: &str| vec![t.to_string()];
    let save = ("Save copy", "↵");
    let trim_chips = vec![("Trim", "↵"), ("Start", "I"), ("End", "O"), BACK];
    vec![
        ("compress images", compress_view(vec![png.clone()], Some((400, 300)), 0, 500), vec![("Compress", "↵"), BACK], vec![]),
        ("compress media", media_view(vec![mp4.clone()], true, some_info(true), 2, 500), vec![("Compress", "↵"), BACK], vec![]),
        ("metadata", meta_view("/nonexistent/u2.mp4", Some(vec![]), None), vec![save, BACK], s("edit a value or clear it")),
        (
            "speed",
            View::VideoSpeed { file: mp4.clone(), duration: Some(8.0), factor: 2.0 },
            vec![save, BACK],
            s("pick a preset or drag the slider"),
        ),
        (
            "normalize",
            View::NormalizeAudio { file: anorm_file(), loudness: None, target: -16.0, range: 11.0, peak: -1.5 },
            vec![("Normalize", "↵"), BACK],
            s("pick a target or type one"),
        ),
        (
            "channels",
            View::ChannelsAudio { file: anorm_file(), in_channels: Some(2), stereo: true, left_db: 0.0, right_db: 0.0 },
            vec![save, BACK],
            s("pick Mono or Stereo, set the volumes"),
        ),
        (
            "audio to video",
            View::VisualizeAudio { file: avis_file(), orient: Canvas::Landscape, style: VisStyle::Waveform, image: None, picking: false },
            vec![("Make video", "↵"), BACK],
            s("pick the picture and the shape"),
        ),
        ("trim video", View::VideoTrim { file: mp4.clone(), trim }, trim_chips.clone(), jump_bar()),
        (
            "trim audio",
            View::AudioTrim {
                file: atrim_file(),
                trim: ferrocast_core::videoplan::Trim::new(6.0, 100.0),
                audio: Some(synth_audio(6.0, 1.0, 3.0)),
            },
            trim_chips,
            s("click the waveform to jump"),
        ),
        ("snapshot", View::VideoSnapshot { file: mp4.clone(), trim }, vec![("Save PNG", "↵"), ("Frame", "←/→"), BACK], jump_bar()),
        (
            "split",
            View::VideoSplit { file: mp4.clone(), trim, points: vec![], parts: 2 },
            vec![("Split", "↵"), ("Cut here", "M"), ("Remove last", "⌫"), BACK],
            jump_bar(),
        ),
        ("bleep", bleep_view(bleep_ranges3(), 0), vec![save, BACK], s("click a shaded range to select it · overlapping ranges are merged")),
        (
            "collage",
            View::CollageImage { files: vec![png.clone(), png.clone()], opts: Default::default(), color: 0 },
            vec![("Save collage", "↵"), BACK],
            s("2 pictures · the preview is a reduced copy"),
        ),
        (
            "frame",
            View::FrameImage { file: png.clone(), size: (100, 80), opts: Default::default(), color: 4, picking: false },
            vec![save, BACK],
            s("drag a slider · the preview is a reduced copy"),
        ),
        (
            "photo",
            View::EditPhoto { file: png.clone(), size: (100, 80), opts: Default::default() },
            vec![save, BACK],
            s("drag a slider · the preview is a reduced copy"),
        ),
        (
            "redact image",
            View::RedactImage { file: png.clone(), size: (100, 80), areas: vec![r], sel: Some(0), blur: true, radius: 24, color: 5 },
            vec![save, ("Remove", "⌫"), BACK],
            s("drag to add an area · drag an area to move, corners to resize · the preview only marks areas; the cover is applied on save"),
        ),
        (
            "redact video",
            redact_view("/nonexistent/u2.mp4", 6.0, vec![area], 0),
            vec![save, BACK],
            s("drag to move/resize the selected area · the preview only marks it"),
        ),
        (
            "crop image",
            View::CropImage { file: png.clone(), size: (1000, 600), rect: r, aspect: None },
            vec![("Crop", "↵"), BACK],
            s("drag inside to move · drag an edge or corner to resize · presets lock the ratio"),
        ),
        (
            "crop video",
            View::VideoCrop { file: mp4.clone(), size: Some((640, 360)), rect: r, aspect: None, duration: 6.0, ref_t: 0.6 },
            vec![("Crop", "↵"), BACK],
            s("drag inside to move · drag an edge or corner to resize · presets lock the ratio"),
        ),
        (
            "annotate",
            View::Annotate {
                file: png.clone(),
                size: (100, 80),
                doc: Default::default(),
                tool: ferrocast_core::annotate::Tool::Arrow,
                color: 0,
                sel: None,
            },
            vec![save, ("Undo", "⌘Z"), ("Delete", "⌫"), BACK],
            s("drag to draw · click a Number to place · Select: click and drag to move"),
        ),
        (
            "reorder join",
            View::Reorder { purpose: ReorderPurpose::JoinVideos, files: reorder_files(3), cursor: 0 },
            vec![("Join", "↵"), ("Move", "⌘↑/↓"), BACK],
            s("First clip sets the canvas · drag a row to move"),
        ),
        (
            "reorder pdf from pictures",
            View::Reorder { purpose: ReorderPurpose::CreatePdf, files: reorder_files(3), cursor: 0 },
            vec![("Create PDF", "↵"), ("Move", "⌘↑/↓"), BACK],
            s("One page per picture · drag a row to move"),
        ),
        (
            "reorder merge pdf",
            View::Reorder { purpose: ReorderPurpose::MergePdf, files: reorder_files(3), cursor: 0 },
            vec![("Merge", "↵"), ("Move", "⌘↑/↓"), BACK],
            s("Drag a row to move"),
        ),
        (
            "reorder merge audio",
            View::Reorder { purpose: ReorderPurpose::MergeAudio, files: reorder_files(3), cursor: 0 },
            vec![("Merge", "↵"), ("Move", "⌘↑/↓"), BACK],
            s("Drag a row to move"),
        ),
        (
            "organize pdf",
            View::PdfOrganize { file: std::env::temp_dir().join("fc-u2.pdf"), n: 3, plan: ferrocast_core::pdfplan::PdfPlan::new(3) },
            vec![save, ("Rotate", "R"), ("Remove", "⌫"), BACK],
            vec![],
        ),
    ]
}

/// What the footer drew (chips left to right) and the body notes of `view` after two frames.
fn footer_and_notes(view: View) -> (Vec<(String, String)>, Vec<String>) {
    let mut app = fixture();
    app.ctx.data_mut(|d| d.remove::<Vec<String>>(Id::new("body-notes")));
    app.stack = vec![View::Root, view];
    render(&mut app, vec![]);
    app.ctx.data_mut(|d| d.remove::<Vec<String>>(Id::new("body-notes")));
    render(&mut app, vec![]);
    let chips = app.ctx.data(|d| d.get_temp(Id::new("footer-chips"))).unwrap_or_default();
    let notes = app.ctx.data(|d| d.get_temp(Id::new("body-notes"))).unwrap_or_default();
    (chips, notes)
}

#[test]
fn tool_windows_show_their_keys_in_the_footer_and_only_information_in_the_body() {
    let mut bad = vec![];
    for (name, view, chips, notes) in u2_windows() {
        let (got_chips, got_notes) = footer_and_notes(view);
        let want: Vec<(String, String)> = chips.iter().map(|(l, k)| (l.to_string(), k.to_string())).collect();
        if got_chips != want {
            bad.push(format!("{name}: footer chips {got_chips:?}, want {want:?}"));
        }
        if got_notes != notes {
            bad.push(format!("{name}: body notes {got_notes:?}, want {notes:?}"));
        }
        let glyphs = ["↵", "esc", "⌘", "⇧", "⌫", "←", "→", "↑", "↓"];
        if let Some(n) = got_notes.iter().find(|n| glyphs.iter().any(|g| n.contains(g))) {
            bad.push(format!("{name}: the body still prints key names: {n:?}"));
        }
    }
    assert!(bad.is_empty(), "footer chips / body notes are wrong:\n{}", bad.join("\n"));
}

#[test]
fn footer_hints_are_pure_per_view_and_at_most_four_chips_with_back_last() {
    let windows = u2_windows();
    for (name, view, chips, _) in &windows {
        assert_eq!(&view.footer_hints(), chips, "{name}");
        assert!(chips.len() <= 4, "{name}: at most four chips");
        assert_eq!(chips.last(), Some(&BACK), "{name}: Back esc is last");
        assert_eq!(chips[0].1, "↵", "{name}: the primary action comes first");
    }
    for v in [View::Root, View::Clipboard, View::Snippets, View::Settings, View::Files, View::Notes, View::Welcome] {
        assert!(v.footer_hints().is_empty(), "{} keeps the footer it had", v.title());
    }
}

/// Views whose footer already drew hints keep drawing exactly them.
#[test]
fn list_and_settings_footers_are_unchanged() {
    let mut app = fixture();
    app.stack = vec![View::Root, View::Settings];
    render(&mut app, vec![]);
    let chips: Vec<(String, String)> = app.ctx.data(|d| d.get_temp(Id::new("footer-chips"))).unwrap_or_default();
    assert_eq!(chips, vec![("Back".to_string(), "esc".to_string())]);
    app.stack = vec![View::Root, View::Snippets];
    render(&mut app, vec![]);
    let chips: Vec<(String, String)> = app.ctx.data(|d| d.get_temp(Id::new("footer-chips"))).unwrap_or_default();
    assert!(chips.iter().any(|(l, k)| l == "Actions" && k == "⌘K"), "{chips:?}");
    assert!(!chips.iter().any(|(l, _)| l == "Back"), "list views have no Back chip");
}

// ---- U1: exclusive choices use the Settings segmented control

/// Every segmented bar drawn by `view` after two frames, top to bottom then left to right, as
/// (cell count). Each bar must be ONE container whose cells sit inside it, side by side.
fn segmented_bars(view: View) -> Vec<usize> {
    let mut app = fixture();
    app.ctx.data_mut(|d| d.remove::<Vec<(Rect, Vec<Rect>)>>(Id::new("segmented-count")));
    app.stack = vec![View::Root, view];
    render(&mut app, vec![]);
    render(&mut app, vec![]);
    let mut bars: Vec<(Rect, Vec<Rect>)> = app.ctx.data(|d| d.get_temp(Id::new("segmented-count"))).unwrap_or_default();
    bars.sort_by(|a, b| (a.0.min.y, a.0.min.x).partial_cmp(&(b.0.min.y, b.0.min.x)).unwrap());
    for (container, cells) in &bars {
        assert!(cells.iter().all(|c| container.contains_rect(*c)), "cells stay inside their container");
        assert!(cells.windows(2).all(|w| w[0].max.x <= w[1].min.x + 0.01 && w[0].min.y == w[1].min.y), "cells sit side by side");
        assert!(container.max.x <= ui::WIDTH, "the bar fits the window width ({} > {})", container.max.x, ui::WIDTH);
    }
    bars.iter().map(|(_, c)| c.len()).collect()
}

#[test]
fn exclusive_choices_in_the_tool_windows_are_segmented_bars() {
    use crate::media_tools::VisStyle;
    use ferrocast_core::media::Canvas;
    let png = std::path::PathBuf::from("/nonexistent/u1.png");
    let r = ferrocast_core::croprect::Rect { x: 5, y: 5, w: 20, h: 20 };
    let area = Area { rect: ferrocast_core::croprect::full(64, 36), start: 0.0, end: 2.0, blur: false };
    let mut got: Vec<(&str, Vec<usize>, Vec<usize>)> = vec![];
    let mut check = |name: &'static str, want: Vec<usize>, view: View| got.push((name, want, segmented_bars(view)));
    check("compress goal, size", vec![3], compress_view(vec![png.clone()], Some((400, 300)), 0, 500));
    check("compress goal, preset", vec![3, 3], compress_view(vec![png.clone()], Some((400, 300)), 2, 500));
    check("compress media", vec![3, 3], media_view(vec!["/nonexistent/u1.mp4".into()], true, some_info(true), 2, 500));
    check("video speed", vec![7], View::VideoSpeed { file: "/nonexistent/u1.mp4".into(), duration: Some(8.0), factor: 2.0 });
    check(
        "normalize presets",
        vec![3],
        View::NormalizeAudio { file: anorm_file(), loudness: None, target: -16.0, range: 11.0, peak: -1.5 },
    );
    check("channels", vec![2], View::ChannelsAudio { file: anorm_file(), in_channels: Some(2), stereo: true, left_db: 0.0, right_db: 0.0 });
    check(
        "audio to video",
        vec![2, 3],
        View::VisualizeAudio { file: avis_file(), orient: Canvas::Landscape, style: VisStyle::Waveform, image: None, picking: false },
    );
    check("collage layout", vec![4], View::CollageImage { files: vec![png.clone(), png.clone()], opts: Default::default(), color: 0 });
    check(
        "frame backdrop",
        vec![3],
        View::FrameImage { file: png.clone(), size: (100, 80), opts: Default::default(), color: 4, picking: false },
    );
    check(
        "redact image",
        vec![2],
        View::RedactImage { file: png.clone(), size: (100, 80), areas: vec![r], sel: Some(0), blur: true, radius: 24, color: 5 },
    );
    check(
        "redact video (style bar, then the aspect strip of the shared crop)",
        vec![2, 6],
        redact_view("/nonexistent/u1.mp4", 6.0, vec![area], 0),
    );
    check("crop image aspect", vec![6], View::CropImage { file: png.clone(), size: (1000, 600), rect: r, aspect: None });
    check(
        "crop video aspect",
        vec![6],
        View::VideoCrop { file: "/nonexistent/u1.mp4".into(), size: Some((640, 360)), rect: r, aspect: None, duration: 6.0, ref_t: 0.6 },
    );
    let bad: Vec<String> =
        got.iter().filter(|(_, w, g)| w != g).map(|(n, w, g)| format!("{n}: want bars with {w:?} cells, drew {g:?}")).collect();
    assert!(bad.is_empty(), "exclusive choices must be ONE segmented bar each:\n{}", bad.join("\n"));
}

/// Cells of the `bar`-th segmented bar (top to bottom, left to right) drawn by the open view.
fn seg_cells(app: &App, bar: usize) -> Vec<Rect> {
    let mut bars: Vec<(Rect, Vec<Rect>)> = app.ctx.data(|d| d.get_temp(Id::new("segmented-count"))).unwrap_or_default();
    bars.sort_by(|a, b| (a.0.min.y, a.0.min.x).partial_cmp(&(b.0.min.y, b.0.min.x)).unwrap());
    bars[bar].1.clone()
}

fn click_cell(app: &mut App, bar: usize, cell: usize) {
    let at = seg_cells(app, bar)[cell].center();
    click_at(app, at);
    render(app, vec![]);
}

#[test]
fn clicking_a_segmented_cell_sets_the_same_state_the_old_chips_did() {
    // usize mode (compress goal) and index preset
    let mut app = compress_app(0, 500);
    click_cell(&mut app, 0, 2);
    assert!(matches!(app.view(), View::CompressImage { mode: 2, .. }));
    click_cell(&mut app, 1, 0);
    assert!(matches!(app.view(), View::CompressImage { mode: 2, preset: 0, .. }));
    click_cell(&mut app, 0, 1);
    assert!(matches!(app.view(), View::CompressImage { mode: 1, preset: 0, .. }));
    // bool (channels)
    let mut app = fixture();
    achan_view(&mut app, Some(2), true);
    render(&mut app, vec![]);
    click_cell(&mut app, 0, 0);
    assert!(matches!(app.view(), View::ChannelsAudio { stereo: false, .. }));
    click_cell(&mut app, 0, 1);
    assert!(matches!(app.view(), View::ChannelsAudio { stereo: true, .. }));
    // float presets: speed (x4 is cell 6) and normalize (-14 is cell 2)
    let mut app = fixture();
    app.stack = vec![View::Root, View::VideoSpeed { file: "/nonexistent/u1.mp4".into(), duration: Some(8.0), factor: 2.0 }];
    render(&mut app, vec![]);
    click_cell(&mut app, 0, 6);
    assert!(matches!(app.view(), View::VideoSpeed { factor, .. } if (*factor - 4.0).abs() < 1e-9));
    app.ctx.data_mut(|d| d.remove::<Vec<(Rect, Vec<Rect>)>>(Id::new("segmented-count")));
    anorm_view(&mut app, None);
    render(&mut app, vec![]);
    click_cell(&mut app, 0, 2);
    assert!(matches!(app.view(), View::NormalizeAudio { target, .. } if (*target + 14.0).abs() < 1e-9));
    // Option aspect (crop): 16:9 locks the ratio and refits the rectangle, Free keeps it
    let mut app = crop_app();
    click_cell(&mut app, 0, 4);
    assert!(
        matches!(app.view(), View::CropImage { aspect: Some((16, 9)), rect, .. } if *rect == ferrocast_core::croprect::fit_aspect(1000, 600, 16, 9))
    );
    click_cell(&mut app, 0, 0);
    assert!(matches!(app.view(), View::CropImage { aspect: None, .. }));
    // enum with side effects (frame backdrop)
    let mut app = fixture();
    let f = std::path::PathBuf::from("/nonexistent/u1.png");
    app.stack = vec![View::Root, View::FrameImage { file: f, size: (100, 80), opts: Default::default(), color: 4, picking: false }];
    render(&mut app, vec![]);
    click_cell(&mut app, 0, 1);
    assert!(matches!(app.view(), View::FrameImage { opts, .. } if matches!(opts.backdrop, ferrocast_core::frame::Backdrop::Blur(_))));
    click_cell(&mut app, 0, 0);
    assert!(matches!(app.view(), View::FrameImage { opts, .. } if matches!(opts.backdrop, ferrocast_core::frame::Backdrop::Color(_))));
}

/// A dropped file for tests: egui 0.36 hands the app a trait object that only knows its path.
#[derive(Debug)]
struct TestDrop(std::path::PathBuf);

impl egui::DroppedFile for TestDrop {
    fn path(&self) -> &std::path::Path {
        &self.0
    }
    fn bytes(&self) -> Result<Vec<u8>, String> {
        std::fs::read(&self.0).map_err(|e| e.to_string())
    }
}

fn dropped(path: &std::path::Path) -> Vec<egui::DroppedFileHandle> {
    vec![std::sync::Arc::new(TestDrop(path.to_path_buf()))]
}

/// The mouse release that picks a wheel segment also ends the drag, so macOS delivers a file DROP
/// to the launcher right after the tool window was shown. That drop must not replace the window.
#[test]
fn the_drop_that_ends_a_wheel_gesture_does_not_replace_the_tool_window() {
    use ferrocast_core::wheel::WheelKind;
    let dir = std::env::temp_dir().join(format!("fc-wheel-drop-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let png = dir.join("pic.png");
    image::RgbImage::new(64, 48).save(&png).unwrap();
    for (label, title) in
        [("Compress", "Compress Images"), ("Edit Photo", "Edit Photo"), ("Crop", "Crop Image"), ("Redact", "Redact Image")]
    {
        let mut app = fixture();
        let files = vec![png.clone()];
        let segs = app.wheel_segments(WheelKind::Tools, &files);
        let i = segs.iter().position(|s| s.label == label).expect(label);
        app.stack = vec![View::Root, View::Wheel { files, center: Pos2::new(-9999.0, -9999.0), segs, kind: WheelKind::Tools, sel: None }];
        app.apply_wheel(Some(i));
        assert_eq!(app.view().title(), title, "{label}: the tool window opens");
        // the drop of the dragged file arrives on the next frame
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(ui::WIDTH, ui::HEIGHT))),
            dropped_files: dropped(&png),
            ..Default::default()
        };
        let ctx = app.ctx.clone();
        let mut output = ctx.run_ui(input, |ui| app.draw(ui));
        output.textures_delta.clear();
        assert_eq!(app.view().title(), title, "{label}: the drop that ended the wheel gesture must not replace it");
    }
}

/// A drop that is NOT the end of a wheel gesture still opens the converter, as before.
#[test]
fn an_ordinary_drop_on_the_visible_launcher_still_opens_the_converter() {
    let dir = std::env::temp_dir().join(format!("fc-plain-drop-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let png = dir.join("pic.png");
    image::RgbImage::new(8, 8).save(&png).unwrap();
    let mut app = fixture();
    app.visible = true;
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(ui::WIDTH, ui::HEIGHT))),
        dropped_files: dropped(&png),
        ..Default::default()
    };
    let ctx = app.ctx.clone();
    let mut output = ctx.run_ui(input, |ui| app.draw(ui));
    output.textures_delta.clear();
    assert!(matches!(app.view(), View::Convert(_)), "got {:?}", app.view().title());
}

/// Characterisation (the function already exists, so there is no RED): the log line for a picked
/// wheel segment names the wheel, the segment, the view that ended on top and whether the launcher
/// is visible, and never a path.
#[test]
fn wheel_pick_log_line_names_the_segment_and_the_resulting_view_but_no_paths() {
    use ferrocast_core::wheel::WheelKind;
    let line = wheel_pick_log_line(WheelKind::Tools, "Crop", "Crop Image", true);
    assert_eq!(line, "wheel pick Tools \"Crop\" -> view \"Crop Image\" launcher_visible=true");
    let hidden = wheel_pick_log_line(WheelKind::Actions, "Copy Path", "Ferrocast", false);
    assert!(hidden.contains("launcher_visible=false") && !hidden.contains('/'), "{hidden}");
}

// ---- U3: form windows draw their controls as rows of a Settings-style card

/// The cards (with their rows) and the segmented bars `view` drew in its second frame.
fn form_cards(view: View) -> (Vec<FormCard>, Vec<Rect>) {
    let mut app = fixture();
    app.ctx.data_mut(|d| {
        d.remove::<Vec<FormCard>>(Id::new("form-cards"));
        d.remove::<Vec<(Rect, Vec<Rect>)>>(Id::new("segmented-count"));
    });
    app.stack = vec![View::Root, view];
    render(&mut app, vec![]);
    render(&mut app, vec![]);
    let cards: Vec<FormCard> = app.ctx.data(|d| d.get_temp(Id::new("form-cards"))).unwrap_or_default();
    let bars: Vec<(Rect, Vec<Rect>)> = app.ctx.data(|d| d.get_temp(Id::new("segmented-count"))).unwrap_or_default();
    (cards, bars.into_iter().map(|b| b.0).collect())
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum FormPlace {
    /// the card is centred in the window
    Centred,
    /// preview on the left, the card is a side panel on the right half
    SidePanel,
}

fn u3_windows() -> Vec<(&'static str, FormPlace, View)> {
    use crate::media_tools::VisStyle;
    use ferrocast_core::media::Canvas;
    use FormPlace::*;
    let png = std::path::PathBuf::from("/nonexistent/u3.png");
    let mp4 = std::path::PathBuf::from("/nonexistent/u3.mp4");
    let few = {
        use ferrocast_core::metaform::Group;
        vec![
            meta_field(Group::File, "title", "Hello", true),
            meta_field(Group::File, "major_brand", "isom", false),
            meta_field(Group::Track { index: 0, kind: "audio".into() }, "language", "eng", true),
        ]
    };
    vec![
        (
            "audio to video",
            Centred,
            View::VisualizeAudio { file: avis_file(), orient: Canvas::Landscape, style: VisStyle::Waveform, image: None, picking: false },
        ),
        (
            "audio to video, still",
            Centred,
            View::VisualizeAudio { file: avis_file(), orient: Canvas::Square, style: VisStyle::Still, image: None, picking: false },
        ),
        ("normalize", Centred, View::NormalizeAudio { file: anorm_file(), loudness: Some(-20.0), target: -16.0, range: 11.0, peak: -1.5 }),
        ("channels", Centred, View::ChannelsAudio { file: anorm_file(), in_channels: Some(2), stereo: true, left_db: 0.0, right_db: 0.0 }),
        ("compress image, size", Centred, compress_view(vec![png.clone()], Some((400, 300)), 0, 500)),
        ("compress image, preset", Centred, compress_view(vec![png.clone()], Some((400, 300)), 2, 500)),
        ("compress video", Centred, media_view(vec![mp4.clone()], true, some_info(true), 1, 500)),
        ("compress audio", Centred, media_view(vec!["/nonexistent/u3.mp3".into()], false, some_info(false), 0, 500)),
        ("metadata", Centred, meta_view("/x/a.mp4", Some(few), None)),
        ("video speed", Centred, View::VideoSpeed { file: mp4.clone(), duration: Some(8.0), factor: 2.0 }),
        (
            "redact image options",
            Centred,
            View::RedactImage { file: png.clone(), size: (100, 80), areas: vec![], sel: None, blur: false, radius: 24, color: 5 },
        ),
        (
            "add background",
            SidePanel,
            View::FrameImage { file: png.clone(), size: (100, 80), opts: Default::default(), color: 4, picking: false },
        ),
        ("collage", SidePanel, View::CollageImage { files: vec![png.clone(), png.clone()], opts: Default::default(), color: 0 }),
        ("edit photo", SidePanel, View::EditPhoto { file: png, size: (100, 80), opts: Default::default() }),
    ]
}

#[test]
fn form_windows_group_their_controls_in_a_card_inside_the_window() {
    let mut bad = vec![];
    for (name, place, view) in u3_windows() {
        let (cards, bars) = form_cards(view);
        if cards.is_empty() {
            bad.push(format!("{name}: drew no card (want at least one)"));
            continue;
        }
        let content = Rect::from_min_max(Pos2::new(0.0, ui::SEARCH_H), Pos2::new(ui::WIDTH, ui::HEIGHT - ui::FOOTER_H));
        for (i, c) in cards.iter().enumerate() {
            if !content.contains_rect(c.rect) {
                bad.push(format!("{name}: card {i} {:?} is not inside the window content {content:?}", c.rect));
            }
            if c.rect.width() > 600.0 {
                bad.push(format!("{name}: card {i} is {} pt wide (max 600)", c.rect.width()));
            }
            match place {
                FormPlace::Centred if (c.rect.center().x - ui::WIDTH / 2.0).abs() >= 4.0 => {
                    bad.push(format!("{name}: card {i} is centred at {} not {}", c.rect.center().x, ui::WIDTH / 2.0))
                }
                FormPlace::SidePanel if c.rect.min.x < ui::WIDTH / 2.0 => {
                    bad.push(format!("{name}: side card {i} starts at {}", c.rect.min.x))
                }
                _ => {}
            }
            if c.rows.is_empty() {
                bad.push(format!("{name}: card {i} has no row"));
            }
            for (title, row, control) in &c.rows {
                if title.trim().is_empty() {
                    bad.push(format!("{name}: card {i} has a row without a title"));
                }
                if !c.rect.contains_rect(*row) {
                    bad.push(format!("{name}: row {title:?} {row:?} sticks out of its card {:?}", c.rect));
                }
                if !c.rect.contains_rect(*control) {
                    bad.push(format!("{name}: the control of {title:?} {control:?} sticks out of its card {:?}", c.rect));
                }
            }
        }
        for b in bars {
            if !cards.iter().any(|c| c.rect.contains_rect(b)) {
                bad.push(format!("{name}: a segmented bar {b:?} lies outside every card"));
            }
        }
    }
    assert!(bad.is_empty(), "form windows must use cards:\n{}", bad.join("\n"));
}

#[test]
fn annotate_tool_strip_fits_the_window_width() {
    let mut app = fixture();
    app.ctx.data_mut(|d| d.remove::<Vec<Rect>>(Id::new("annot-strip")));
    app.stack = vec![
        View::Root,
        View::Annotate {
            file: "/nonexistent/u3.png".into(),
            size: (100, 80),
            doc: Default::default(),
            tool: ferrocast_core::annotate::Tool::Arrow,
            color: 0,
            sel: None,
        },
    ];
    render(&mut app, vec![]);
    app.ctx.data_mut(|d| d.remove::<Vec<Rect>>(Id::new("annot-strip")));
    render(&mut app, vec![]);
    let strip: Vec<Rect> = app.ctx.data(|d| d.get_temp(Id::new("annot-strip"))).unwrap_or_default();
    assert!(strip.len() >= 18, "12 tools and 6 colours, got {}", strip.len());
    let right = strip.iter().map(|r| r.max.x).fold(0.0, f32::max);
    assert!(right <= ui::WIDTH - 12.0, "the tool strip ends at x={right}, the window is {} wide", ui::WIDTH);
}

// ---- U4a: the new windows read in both themes

/// Source-over of `top` on an opaque `under`, as floats 0..255. `Color32` is premultiplied, so its
/// channels are added as they are.
fn over(under: [f32; 3], top: Color32) -> [f32; 3] {
    let a = top.a() as f32 / 255.0;
    let t = [top.r() as f32, top.g() as f32, top.b() as f32];
    [0, 1, 2].map(|i| t[i] + under[i] * (1.0 - a))
}

/// WCAG relative luminance of an sRGB colour (0..255 channels).
fn luminance(c: [f32; 3]) -> f32 {
    let lin = |v: f32| {
        let v = v / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
}

/// WCAG contrast ratio of two opaque colours.
fn contrast(a: [f32; 3], b: [f32; 3]) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// The window fill as the user sees it. It is translucent, so the worst of a white and a black
/// backdrop is used by the callers.
fn window_over(pal: &Palette, backdrop: f32) -> [f32; 3] {
    over([backdrop; 3], pal.bg)
}

#[test]
fn palette_text_and_controls_keep_their_contrast_in_both_themes() {
    let mut bad = vec![];
    for dark in [false, true] {
        let pal = palette(dark);
        for backdrop in [0.0_f32, 255.0] {
            let bg = window_over(&pal, backdrop);
            let card = over(bg, pal.field);
            let container = over(card, pal.field.gamma_multiply(1.3));
            let segment = over(container, pal.sel.gamma_multiply(1.8));
            let t = |c: Color32| [c.r() as f32, c.g() as f32, c.b() as f32];
            let rows = [
                ("text on bg", contrast(t(pal.text), bg), 4.5),
                ("text on card", contrast(t(pal.text), card), 4.5),
                ("sub on bg", contrast(t(pal.sub), bg), 3.0),
                ("sub on card", contrast(t(pal.sub), card), 3.0),
                ("selected segment vs its container", contrast(segment, container), 1.3),
            ];
            for (what, got, want) in rows {
                eprintln!("U4a contrast dark={dark} backdrop={backdrop}: {what} = {got:.2} (want >= {want})");
                if got < want {
                    bad.push(format!("dark={dark} backdrop={backdrop}: {what} is {got:.2}, want >= {want}"));
                }
            }
        }
    }
    assert!(bad.is_empty(), "contrast too low:\n{}", bad.join("\n"));
}

#[test]
fn the_last_row_of_a_form_card_draws_no_separator() {
    let mut bad = vec![];
    for (name, _, view) in u3_windows() {
        let (cards, _) = form_cards(view);
        for (i, c) in cards.iter().enumerate() {
            let Some((title, last, _)) = c.rows.last() else { continue };
            if let Some(y) = c.seps.iter().find(|y| (**y - last.max.y).abs() < 0.5) {
                bad.push(format!("{name}: card {i} draws a separator at y={y} under its last row {title:?} (row ends at {})", last.max.y));
            }
            if c.seps.len() + 1 != c.rows.len() {
                bad.push(format!("{name}: card {i} has {} rows but {} separators (want rows - 1)", c.rows.len(), c.seps.len()));
            }
        }
    }
    assert!(bad.is_empty(), "separator under the last row:\n{}", bad.join("\n"));
}

/// The slider rail is `widgets.inactive.bg_fill`; on a card it must show in both themes.
#[test]
fn slider_rail_stands_out_from_the_card_in_both_themes() {
    let mut bad = vec![];
    for dark in [false, true] {
        let mut app = fixture();
        app.ctx.set_theme(if dark { egui::ThemePreference::Dark } else { egui::ThemePreference::Light });
        app.stack = vec![View::Root, View::VideoSpeed { file: "/nonexistent/u4.mp4".into(), duration: Some(8.0), factor: 2.0 }];
        render(&mut app, vec![]);
        render(&mut app, vec![]);
        let theme = if dark { egui::Theme::Dark } else { egui::Theme::Light };
        let rail = app.ctx.style_of(theme).visuals.widgets.inactive.bg_fill;
        let pal = palette(dark);
        for backdrop in [0.0_f32, 255.0] {
            let card = over(window_over(&pal, backdrop), pal.field);
            let got = contrast(over(card, rail), card);
            eprintln!("U4a slider rail dark={dark} backdrop={backdrop}: {got:.2}");
            if got < 1.3 {
                bad.push(format!("dark={dark} backdrop={backdrop}: the slider rail is {got:.2} against its card, want >= 1.3"));
            }
        }
    }
    assert!(bad.is_empty(), "invisible slider rail:\n{}", bad.join("\n"));
}

/// Every filled circle of radius 8 (the colour swatches) a view drew in its second frame.
fn swatch_circles(view: View, dark: bool) -> Vec<egui::epaint::CircleShape> {
    fn walk(shape: &egui::Shape, out: &mut Vec<egui::epaint::CircleShape>) {
        match shape {
            egui::Shape::Circle(c) if (c.radius - 8.0).abs() < 0.01 && c.fill != Color32::TRANSPARENT => out.push(*c),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut app = fixture();
    app.ctx.set_theme(if dark { egui::ThemePreference::Dark } else { egui::ThemePreference::Light });
    app.stack = vec![View::Root, view];
    render(&mut app, vec![]);
    let input =
        egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(ui::WIDTH, ui::HEIGHT))), ..Default::default() };
    let ctx = app.ctx.clone();
    let output = ctx.run_ui(input, |ui| app.draw(ui));
    let mut found = vec![];
    for s in &output.shapes {
        walk(&s.shape, &mut found);
    }
    found
}

/// A swatch is as dark as a dark card or as light as a light one for some colours (black, white),
/// so it needs an edge of its own.
#[test]
fn colour_swatches_have_an_edge_that_shows_on_the_card() {
    use ferrocast_core::annotate::Tool;
    let png = std::path::PathBuf::from("/nonexistent/u4.png");
    let views = || {
        vec![
            (
                "annotate",
                View::Annotate { file: png.clone(), size: (100, 80), doc: Default::default(), tool: Tool::Arrow, color: 0, sel: None },
            ),
            ("collage", View::CollageImage { files: vec![png.clone(), png.clone()], opts: Default::default(), color: 0 }),
            ("add background", View::FrameImage { file: png.clone(), size: (100, 80), opts: Default::default(), color: 4, picking: false }),
            (
                "redact image",
                View::RedactImage { file: png.clone(), size: (100, 80), areas: vec![], sel: None, blur: false, radius: 24, color: 5 },
            ),
        ]
    };
    let mut bad = vec![];
    for dark in [false, true] {
        let pal = palette(dark);
        let card = over(window_over(&pal, if dark { 0.0 } else { 255.0 }), pal.field);
        for (name, view) in views() {
            let circles = swatch_circles(view, dark);
            if circles.len() < 6 {
                bad.push(format!("{name} dark={dark}: found only {} swatches", circles.len()));
            }
            for c in circles {
                let ring = if c.stroke.width >= 1.0 { contrast(over(card, c.stroke.color), card) } else { 0.0 };
                if ring < 1.3 {
                    bad.push(format!(
                        "{name} dark={dark}: swatch {:?} has no edge that shows on the card (edge contrast {ring:.2}, stroke width {})",
                        c.fill, c.stroke.width
                    ));
                }
            }
        }
    }
    assert!(bad.is_empty(), "swatches without an edge:\n{}", bad.join("\n"));
}

// ---------------------------------------------------------------------------
// Enter on a Finder selection (opt-in). The tap itself needs a real session and is NOT tested
// here: these tests drive the app side through injected hooks and never post real key events.

#[derive(Default)]
struct FinderProbe {
    prompts: Mutex<Vec<bool>>,
    starts: std::sync::atomic::AtomicUsize,
    drops: Arc<std::sync::atomic::AtomicUsize>,
    reads: std::sync::atomic::AtomicUsize,
    reposts: std::sync::atomic::AtomicUsize,
}

struct DropCount(Arc<std::sync::atomic::AtomicUsize>);
impl Drop for DropCount {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

type Selection = Arc<dyn Fn() -> Result<Vec<PathBuf>, String> + Send + Sync>;

fn finder_hooks(probe: &Arc<FinderProbe>, trusted: bool, start_ok: bool, selection: Selection) -> services::FinderHooks {
    let (p1, p2, p3, p4) = (probe.clone(), probe.clone(), probe.clone(), probe.clone());
    services::FinderHooks {
        trusted: Arc::new(move |prompt| {
            p1.prompts.lock().unwrap().push(prompt);
            trusted
        }),
        start: Arc::new(move |_, _| {
            p2.starts.fetch_add(1, Ordering::SeqCst);
            if start_ok {
                Ok(Box::new(DropCount(p2.drops.clone())) as Box<dyn std::any::Any>)
            } else {
                Err("no tap".into())
            }
        }),
        selection: Arc::new(move || {
            p3.reads.fetch_add(1, Ordering::SeqCst);
            selection()
        }),
        repost: Arc::new(move || {
            p4.reposts.fetch_add(1, Ordering::SeqCst);
            true
        }),
    }
}

fn finder_app(probe: &Arc<FinderProbe>, trusted: bool, selection: Selection) -> App {
    let mut app = fixture();
    app.finder.hooks = finder_hooks(probe, trusted, true, selection);
    app
}

/// Drain the bus until `done` holds (the selection read runs on a worker thread).
fn pump(app: &mut App, done: impl Fn(&App) -> bool) {
    let t = Instant::now();
    while !done(app) && t.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(5));
        app.drain();
    }
    assert!(done(app), "timed out waiting for the Finder-Enter handler");
}

/// Scroll the Settings pane until the Enter-on-Finder toggle is on screen; returns its centre.
fn scroll_to_enter_toggle(app: &mut App) -> Option<Pos2> {
    let at = |app: &App| app.ctx.data(|d| d.get_temp::<Rect>(Id::new("enter-on-finder-toggle")));
    let pane = Pos2::new(500.0, 200.0);
    render(app, vec![Event::PointerMoved(pane)]);
    for _ in 0..8 {
        let r = at(app)?;
        if r.center().y < ui::HEIGHT - 40.0 {
            // let the scroll animation settle, then read the final place
            for _ in 0..30 {
                render(app, vec![]);
            }
            return at(app).map(|r| r.center());
        }
        render(
            app,
            vec![Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, -150.0),
                phase: egui::TouchPhase::Move,
                modifiers: Modifiers::NONE,
            }],
        );
        render(app, vec![]);
    }
    at(app).map(|r| r.center())
}

fn count(n: &std::sync::atomic::AtomicUsize) -> usize {
    n.load(Ordering::SeqCst)
}

#[test]
fn with_the_setting_off_no_tap_is_made_and_nothing_is_asked() {
    let probe = Arc::new(FinderProbe::default());
    let mut app = finder_app(&probe, true, Arc::new(|| Ok(vec![])));
    assert!(!app.settings.enter_on_finder, "off by default");
    app.set_finder_enter(false, true);
    render(&mut app, vec![]);
    assert!(app.finder.tap.is_none());
    assert!(probe.prompts.lock().unwrap().is_empty(), "no Accessibility prompt while the feature is off");
    assert_eq!(count(&probe.starts), 0);
}

#[test]
fn enabling_without_accessibility_makes_no_tap_asks_once_and_toasts() {
    let probe = Arc::new(FinderProbe::default());
    let mut app = finder_app(&probe, false, Arc::new(|| Ok(vec![])));
    app.set_finder_enter(true, true);
    assert!(app.finder.tap.is_none(), "no tap without Accessibility");
    assert_eq!(count(&probe.starts), 0, "the tap is not even attempted");
    assert_eq!(*probe.prompts.lock().unwrap(), vec![true], "asked exactly once, with the system prompt");
    assert_eq!(app.toast.as_ref().map(|t| t.0.as_str()), Some("Enter on Finder needs Accessibility access"));
    render(&mut app, vec![]);
    render(&mut app, vec![]);
    assert_eq!(probe.prompts.lock().unwrap().len(), 1, "no retry loop");
}

#[test]
fn a_tap_that_cannot_be_created_takes_the_same_graceful_path() {
    let probe = Arc::new(FinderProbe::default());
    let mut app = fixture();
    app.finder.hooks = finder_hooks(&probe, true, false, Arc::new(|| Ok(vec![])));
    app.set_finder_enter(true, true);
    assert_eq!(count(&probe.starts), 1);
    assert!(app.finder.tap.is_none());
    assert_eq!(app.toast.as_ref().map(|t| t.0.as_str()), Some("Enter on Finder needs Accessibility access"));
}

#[test]
fn enabling_creates_the_tap_once_and_disabling_drops_it_without_a_restart() {
    let probe = Arc::new(FinderProbe::default());
    let mut app = finder_app(&probe, true, Arc::new(|| Ok(vec![])));
    app.set_finder_enter(true, true);
    app.set_finder_enter(true, true);
    assert!(app.finder.tap.is_some());
    assert_eq!(count(&probe.starts), 1, "a second enable does not create a second tap");
    assert_eq!(count(&probe.drops), 0);
    app.set_finder_enter(false, true);
    assert!(app.finder.tap.is_none());
    assert_eq!(count(&probe.drops), 1, "turning it off drops (removes) the tap");
    app.set_finder_enter(true, true);
    assert_eq!(count(&probe.starts), 2, "and it can be turned on again");
}

#[test]
fn an_empty_selection_gives_the_return_back_to_finder() {
    let probe = Arc::new(FinderProbe::default());
    let mut app = finder_app(&probe, true, Arc::new(|| Ok(vec![])));
    app.set_finder_enter(true, true);
    app.bus.send(Msg::FinderEnter);
    pump(&mut app, |a| count(&probe.reposts) == 1 && !a.finder.busy);
    assert_eq!(count(&probe.reads), 1);
    assert!(!matches!(app.view(), View::Wheel { .. }), "no wheel for an empty selection");
}

#[test]
fn a_failed_selection_read_also_gives_the_return_back() {
    let probe = Arc::new(FinderProbe::default());
    let mut app = finder_app(&probe, true, Arc::new(|| Err("Automation denied".into())));
    app.set_finder_enter(true, true);
    app.bus.send(Msg::FinderEnter);
    pump(&mut app, |a| count(&probe.reposts) == 1 && !a.finder.busy);
    assert!(!matches!(app.view(), View::Wheel { .. }));
}

#[test]
fn a_selection_opens_the_convert_wheel_and_reposts_nothing() {
    let probe = Arc::new(FinderProbe::default());
    let mut app = finder_app(&probe, true, Arc::new(|| Ok(vec![PathBuf::from("/nonexistent/a.png")])));
    app.set_finder_enter(true, true);
    app.bus.send(Msg::FinderEnter);
    pump(&mut app, |a| matches!(a.view(), View::Wheel { .. }));
    assert_eq!(wheel_state(&app).map(|w| w.0), Some(ferrocast_core::wheel::WheelKind::Convert));
    assert_eq!(count(&probe.reposts), 0, "the Return stays consumed when the wheel opens");
    assert!(!app.finder.busy);
}

// The `wheel_segments(..).is_empty() => repost` guard in `on_finder_enter_done` is not testable:
// `convert::targets` always yields at least a ZIP target for any non-empty selection, and an empty
// selection is already turned into a re-post by `outcome`.
#[test]
fn a_repost_frees_the_busy_flag_and_starts_the_300ms_pass_through_window() {
    let probe = Arc::new(FinderProbe::default());
    let mut app = finder_app(&probe, true, Arc::new(|| Ok(vec![])));
    app.set_finder_enter(true, true);
    assert!(app.finder.inflight.try_begin(), "the tap swallowed a Return");
    app.bus.send(Msg::FinderEnter);
    pump(&mut app, |a| count(&probe.reposts) == 1 && !a.finder.busy);
    assert!(app.finder.inflight.in_hold(Instant::now()), "re-post starts the pass-through window");
    assert!(!app.finder.inflight.is_busy(), "the busy flag is free again");
    assert!(!app.finder.inflight.try_begin(), "a Return right after the re-post passes through");
}

#[test]
fn a_second_return_while_one_is_handled_is_passed_through_not_queued() {
    let probe = Arc::new(FinderProbe::default());
    let (gate_tx, gate_rx) = std::sync::mpsc::channel::<()>();
    let gate_rx = Mutex::new(gate_rx);
    let sel: Selection = Arc::new(move || {
        let _ = gate_rx.lock().unwrap().recv_timeout(Duration::from_secs(5));
        Ok(vec![PathBuf::from("/nonexistent/a.png")])
    });
    let mut app = finder_app(&probe, true, sel);
    app.set_finder_enter(true, true);
    app.bus.send(Msg::FinderEnter);
    app.bus.send(Msg::FinderEnter);
    app.drain();
    assert!(app.finder.busy, "the first is still being handled");
    assert_eq!(count(&probe.reposts), 1, "the extra Return is given back at once");
    let _ = gate_tx.send(());
    pump(&mut app, |a| matches!(a.view(), View::Wheel { .. }));
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(count(&probe.reads), 1, "only one selection read ran");
    assert_eq!(count(&probe.reposts), 1, "the wheel of the first one opened, nothing else was posted");
}

#[test]
fn a_return_that_arrives_after_the_tap_was_removed_is_given_back() {
    let probe = Arc::new(FinderProbe::default());
    let mut app = finder_app(&probe, true, Arc::new(|| Ok(vec![PathBuf::from("/nonexistent/a.png")])));
    app.bus.send(Msg::FinderEnter); // no tap: a late message from a tap that was just turned off
    app.drain();
    assert_eq!(count(&probe.reposts), 1);
    assert_eq!(count(&probe.reads), 0, "no selection read, no wheel");
    assert!(!matches!(app.view(), View::Wheel { .. }));
}

#[test]
fn the_settings_toggle_renders_in_both_themes_and_flips_the_setting() {
    for dark in [true, false] {
        let probe = Arc::new(FinderProbe::default());
        let mut app = finder_app(&probe, true, Arc::new(|| Ok(vec![])));
        app.ctx.set_theme(if dark { egui::ThemePreference::Dark } else { egui::ThemePreference::Light });
        app.stack = vec![View::Root, View::Settings];
        render(&mut app, vec![]);
        render(&mut app, vec![]);
        let pos = scroll_to_enter_toggle(&mut app);
        assert!(pos.is_some(), "dark={dark}: the 'Enter on Finder selection' toggle was not drawn");
        let pos = pos.unwrap();
        assert!(pos.y < ui::HEIGHT && pos.x < ui::WIDTH, "dark={dark}: toggle off screen at {pos:?}");
        assert!(!app.settings.enter_on_finder, "dark={dark}: off by default");
        let click = |app: &mut App| {
            let btn = |pressed| Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
            render(app, vec![Event::PointerMoved(pos)]);
            render(app, vec![btn(true)]);
            render(app, vec![btn(false)]);
            render(app, vec![]);
        };
        click(&mut app);
        assert!(app.settings.enter_on_finder, "dark={dark}: a click turns it on");
        assert!(app.finder.tap.is_some(), "dark={dark}: turning it on creates the tap");
        assert_eq!(*probe.prompts.lock().unwrap(), vec![true]);
        click(&mut app);
        assert!(!app.settings.enter_on_finder, "dark={dark}: a second click turns it off");
        assert!(app.finder.tap.is_none(), "dark={dark}: turning it off removes the tap");
        assert_eq!(count(&probe.drops), 1);
    }
}

#[test]
fn the_toggle_with_accessibility_missing_keeps_the_setting_and_toasts() {
    let probe = Arc::new(FinderProbe::default());
    let mut app = finder_app(&probe, false, Arc::new(|| Ok(vec![])));
    app.stack = vec![View::Root, View::Settings];
    render(&mut app, vec![]);
    render(&mut app, vec![]);
    let pos = scroll_to_enter_toggle(&mut app);
    assert!(pos.is_some(), "toggle not drawn");
    let pos = pos.unwrap();
    assert!(pos.y < ui::HEIGHT, "toggle still off screen at {pos:?}");
    let btn = |pressed| Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    render(&mut app, vec![Event::PointerMoved(pos)]);
    render(&mut app, vec![btn(true)]);
    render(&mut app, vec![btn(false)]);
    render(&mut app, vec![]);
    assert!(app.settings.enter_on_finder, "the setting stays as the user set it");
    assert!(app.finder.tap.is_none());
    assert_eq!(app.toast.as_ref().map(|t| t.0.as_str()), Some("Enter on Finder needs Accessibility access"));
    assert_eq!(probe.prompts.lock().unwrap().len(), 1, "no retry loop");
}

/// A wheel opened WITHOUT a drag (Enter on a Finder selection) has no mouse-release event to pick
/// with, so it must answer to clicks: a segment is chosen, a click outside the ring cancels and a
/// click on the hub does nothing.
fn wheel_click_fixture(png: &std::path::Path) -> (App, Vec<ferrocast_core::wheel::WheelKind>, Pos2) {
    use ferrocast_core::wheel::WheelKind;
    let mut app = fixture();
    let files = vec![png.to_path_buf()];
    let segs = app.wheel_segments(WheelKind::Tools, &files);
    app.stack = vec![View::Root, View::Wheel { files, center: Pos2::new(-9999.0, -9999.0), segs, kind: WheelKind::Tools, sel: None }];
    render(&mut app, vec![]);
    let centre = Pos2::new(ui::WIDTH / 2.0, ui::HEIGHT / 2.0);
    (app, vec![WheelKind::Tools], centre)
}

fn wheel_click_at(app: &mut App, at: Pos2) {
    render(app, vec![Event::PointerMoved(at)]);
    render(app, vec![pointer_button(at, true)]);
    render(app, vec![pointer_button(at, false)]);
}

#[test]
fn clicking_a_wheel_segment_picks_it_when_there_was_no_drag() {
    use ferrocast_core::wheel::{segment_center, Geometry};
    let dir = std::env::temp_dir().join(format!("fc-wheel-click-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let png = dir.join("pic.png");
    image::RgbImage::new(64, 48).save(&png).unwrap();
    let (mut app, _, centre) = wheel_click_fixture(&png);
    let segs = match app.view() {
        View::Wheel { segs, .. } => segs.clone(),
        _ => panic!("wheel expected"),
    };
    let i = segs.iter().position(|s| s.label == "Crop").expect("a Crop segment");
    let g = Geometry::DEFAULT;
    let [dx, dy] = segment_center(i, segs.len(), (g.r_in + g.r_out) / 2.0);
    wheel_click_at(&mut app, centre + Vec2::new(dx, dy));
    assert_eq!(
        app.view().title(),
        "Crop Image",
        "the click on Crop opens the crop window (wheel still open: {})",
        matches!(app.view(), View::Wheel { .. })
    );
}

#[test]
fn clicking_outside_the_ring_cancels_the_wheel_and_the_hub_does_nothing() {
    let dir = std::env::temp_dir().join(format!("fc-wheel-click2-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let png = dir.join("pic.png");
    image::RgbImage::new(64, 48).save(&png).unwrap();
    let (mut app, _, centre) = wheel_click_fixture(&png);
    wheel_click_at(&mut app, centre);
    assert!(matches!(app.view(), View::Wheel { .. }), "a click on the hub changes nothing");
    wheel_click_at(&mut app, centre + Vec2::new(ferrocast_core::wheel::Geometry::DEFAULT.r_out + 24.0, 0.0));
    assert!(!matches!(app.view(), View::Wheel { .. }), "a click outside the ring cancels the wheel");
    assert_eq!(app.stack.len(), 1, "back at the root, no tool window opened");
}

/// A wheel opened without a drag (Enter on a Finder selection, or "Format Wheel") has its centre
/// parked off screen, so the global mouse position never lands on it: hovering must use the
/// pointer position inside the window instead, as the click already does.
#[test]
fn hovering_a_segment_highlights_it_on_a_wheel_opened_without_a_drag() {
    use ferrocast_core::wheel::{segment_center, Geometry};
    let dir = std::env::temp_dir().join(format!("fc-wheel-hover-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let png = dir.join("pic.png");
    image::RgbImage::new(64, 48).save(&png).unwrap();
    let (mut app, _, centre) = wheel_click_fixture(&png);
    if let Some(View::Wheel { sel, .. }) = app.stack.last_mut() {
        *sel = Some(0); // what Act::OpenWheel does
    }
    render(&mut app, vec![]);
    let hover = |app: &App| app.ctx.data(|d| d.get_temp::<Option<usize>>(Id::new("wheel-hover"))).flatten();
    assert_eq!(hover(&app), Some(0), "with the pointer away the keyboard selection shows");
    let n = match app.view() {
        View::Wheel { segs, .. } => segs.len(),
        _ => panic!("wheel expected"),
    };
    let g = Geometry::DEFAULT;
    for target in [2usize, 4, n - 1] {
        let [dx, dy] = segment_center(target, n, (g.r_in + g.r_out) / 2.0);
        render(&mut app, vec![Event::PointerMoved(centre + Vec2::new(dx, dy))]);
        assert_eq!(hover(&app), Some(target), "the segment under the pointer is the highlighted one");
    }
    // pointer on the hub: back to the keyboard selection
    render(&mut app, vec![Event::PointerMoved(centre)]);
    assert_eq!(hover(&app), Some(0), "the hub falls back to the keyboard selection");
}

/// The drag-opened wheel (a real centre) must keep following the GLOBAL mouse: a pointer position
/// egui happens to hold inside the window must not hijack its highlight.
#[test]
fn a_wheel_with_a_real_centre_ignores_the_pointer_inside_the_window() {
    use ferrocast_core::wheel::{segment_center, Geometry};
    let dir = std::env::temp_dir().join(format!("fc-wheel-hover2-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let png = dir.join("pic.png");
    image::RgbImage::new(64, 48).save(&png).unwrap();
    let (mut app, _, centre) = wheel_click_fixture(&png);
    if let Some(View::Wheel { sel, center, .. }) = app.stack.last_mut() {
        *sel = Some(0);
        *center = Pos2::new(400.0, 300.0); // a real, on-screen centre: the drag-opened wheel
    }
    render(&mut app, vec![]);
    let n = match app.view() {
        View::Wheel { segs, .. } => segs.len(),
        _ => panic!("wheel expected"),
    };
    let g = Geometry::DEFAULT;
    let [dx, dy] = segment_center(3, n, (g.r_in + g.r_out) / 2.0);
    render(&mut app, vec![Event::PointerMoved(centre + Vec2::new(dx, dy))]);
    let hover = app.ctx.data(|d| d.get_temp::<Option<usize>>(Id::new("wheel-hover"))).flatten();
    assert_eq!(hover, Some(0), "no global mouse in tests: the keyboard selection stays, the in-window pointer is ignored");
}
