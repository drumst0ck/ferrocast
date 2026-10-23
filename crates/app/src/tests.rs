use super::*;

/// Hooks that touch nothing: Accessibility is not granted, no tap can start, the selection is
/// empty and nothing is re-posted. Tests that exercise Enter-on-Finder replace them.
fn inert_finder_hooks() -> services::FinderHooks {
    services::FinderHooks {
        trusted: Arc::new(|_| false),
        start: Arc::new(|_, _, _| Err("inert".into())),
        selection: Arc::new(|| Ok(vec![])),
        repost: Arc::new(|| false),
        log: Arc::new(|_, _| {}),
        notify: Arc::new(|_, _| {}),
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
        update_poller: None,
        ctx,
        shared,
        bus,
        rx,
        store: store.clone(),
        settings,
        snippets: vec![],
        quicklinks: vec![],
        notes: vec![],
        notes_ctl: crate::notes_ctl::NotesCtl::new(Box::new(crate::notes_ctl::fake::FakeFx::new().0)),
        frecency: Frecency::default(),
        update_available: None,
        apps: vec![],
        apps_at: Instant::now(),
        scripts: vec![],
        base: vec![],
        hotkeys: None,
        capture: crate::capture_ctl::CaptureState::with(std::path::PathBuf::new(), crate::capture_ctl::Availability::Available),
        selection: Default::default(),
        finder: services::FinderEnterState { hooks: inert_finder_hooks(), ..Default::default() },
        _tray: None,
        record: Default::default(),
        tray_rec_shown: false,
        rt: Default::default(),
        editor: None,
        notes_tray: None,
        icons: IconCache::new(store.images_dir()),
        stack: vec![View::Root],
        query: String::new(),
        prev_query: String::new(),
        selected: 0,
        scroll_to_sel: false,
        panel: None,
        toast: None,
        settings_undo: None,
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
        wheel_text: None,
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
        notion: Default::default(),
        nv: Default::default(),
        ov: Default::default(),
        na: Default::default(),
        oc: Default::default(),
        obsidian: Default::default(),
        rates: Default::default(),
        rates_checked_at: Instant::now(),
        sec_cache: None,
        sec_dirty: true,
        tour: tour::Tour::new(),
        intro: None,
        fx: Effects::recording(),
        translate_run: 0,
        translated_run: None,
        translating_effect: None,
        translate_started: Instant::now(),
        apple_fallback_told: false,
        apple_langs: Default::default(),
        web_pending: None,
    };
    app.rebuild();
    app
}

/// (source pid, drop point, run) of an `effect_probe` call.
type ProbeCall = (Option<i32>, Option<[f64; 2]>, u64);

thread_local! {
    /// Side effects the translate delivery asked for in this test thread.
    static FX_LOG: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
    /// What `apple_available` answers in this test thread.
    static APPLE_AVAILABLE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static FX_FRONT: std::cell::Cell<Option<i32>> = const { std::cell::Cell::new(None) };
    /// Scripts the web effect asked a browser to run (replace and unmark), in order.
    static FX_SCRIPTS: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
    /// Arguments of every `effect_probe` call: (source pid, drop point, run).
    static FX_PROBE: std::cell::RefCell<Vec<ProbeCall>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn fx_scripts() -> Vec<String> {
    FX_SCRIPTS.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

fn fx_probes() -> Vec<ProbeCall> {
    FX_PROBE.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

fn fx_taken() -> Vec<String> {
    FX_LOG.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

/// The Notes menu-bar item of the tests: only records that it was created and removed.
struct FakeNotesTray;

impl Drop for FakeNotesTray {
    fn drop(&mut self) {
        FX_LOG.with(|l| l.borrow_mut().push("notes_tray off".into()));
    }
}

impl Effects {
    /// No process, AX or keyboard: every side effect is only recorded in `FX_LOG`.
    fn recording() -> Effects {
        FX_LOG.with(|l| l.borrow_mut().clear());
        FX_FRONT.with(|f| f.set(None));
        FX_SCRIPTS.with(|l| l.borrow_mut().clear());
        FX_PROBE.with(|l| l.borrow_mut().clear());
        Effects {
            frontmost: || FX_FRONT.with(|f| f.get()),
            ax_replace: |pid, _lang, _text, _bus| FX_LOG.with(|l| l.borrow_mut().push(format!("ax {pid}"))),
            web_replace: |b, _lang, _text, script, _bus| {
                FX_SCRIPTS.with(|l| l.borrow_mut().push(script.to_string()));
                FX_LOG.with(|l| l.borrow_mut().push(format!("web {b:?}")))
            },
            effect_probe: |pid, drop, run, _bus| FX_PROBE.with(|l| l.borrow_mut().push((pid, drop, run))),
            glow_show: |_t| FX_LOG.with(|l| l.borrow_mut().push("glow_show".into())),
            glow_hide: || FX_LOG.with(|l| l.borrow_mut().push("glow_hide".into())),
            notes_tray: |_bus| {
                FX_LOG.with(|l| l.borrow_mut().push("notes_tray on".into()));
                Some(Box::new(FakeNotesTray))
            },
            notes_appearance: |a| FX_LOG.with(|l| l.borrow_mut().push(format!("appearance {a:?}"))),
            web_script: |_b, js| FX_SCRIPTS.with(|l| l.borrow_mut().push(js.to_string())),
            apple_available: || APPLE_AVAILABLE.with(|a| a.get()),
            translate_start: |_s, _text, _lang, _pid, _out, run, _bus| FX_LOG.with(|l| l.borrow_mut().push(format!("translate {run}"))),
            paste: |_app, _text| FX_LOG.with(|l| l.borrow_mut().push("paste".into())),
        }
    }
}

fn translated(
    app: &mut App,
    output: ferrocast_core::models::TranslateOutput,
    target: ferrocast_core::translate::Target,
    source_pid: Option<i32>,
) {
    app.settings.translate_output = output;
    app.bus.send(Msg::Translated { run: 0, lang: "Spanish".into(), source_pid, result: Ok("Hola".into()), ms: 5, target });
    app.drain();
}

fn assert_shows_hola(app: &App) {
    match app.view() {
        View::Translation { lang, text } => assert_eq!((lang.as_str(), text.as_str()), ("Spanish", "Hola")),
        _ => panic!("expected View::Translation"),
    }
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
    for v in [View::Root, View::Clipboard, View::Snippets, View::Files, View::Notes, View::Welcome] {
        assert!(v.footer_hints().is_empty(), "{} keeps the footer it had", v.title());
    }
    assert_eq!(View::Settings.footer_hints(), vec![("Tabs", "⌘1–0"), ("Find", "⌘F"), BACK], "Settings hints (SD2, SD4 adds Find)");
}

/// Views whose footer already drew hints keep drawing exactly them.
#[test]
fn list_and_settings_footers_are_unchanged() {
    let mut app = fixture();
    app.stack = vec![View::Root, View::Settings];
    render(&mut app, vec![]);
    let chips: Vec<(String, String)> = app.ctx.data(|d| d.get_temp(Id::new("footer-chips"))).unwrap_or_default();
    // SD2: Settings now shows its own hint chips instead of only Back
    assert_eq!(SETTINGS_CHIPS.map(|(a, b)| (a.to_string(), b.to_string())).to_vec(), chips);
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
    /// Lines passed to the injected log hook.
    lines: Mutex<Vec<String>>,
    /// (title, message) passed to the injected notify hook.
    notes: Mutex<Vec<(String, String)>>,
}

struct DropCount(Arc<std::sync::atomic::AtomicUsize>);
impl Drop for DropCount {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

type Selection = Arc<dyn Fn() -> Result<Vec<PathBuf>, String> + Send + Sync>;

fn finder_hooks(probe: &Arc<FinderProbe>, trusted: bool, start_ok: bool, selection: Selection) -> services::FinderHooks {
    let (p1, p2, p3, p4, p5, p6) = (probe.clone(), probe.clone(), probe.clone(), probe.clone(), probe.clone(), probe.clone());
    services::FinderHooks {
        trusted: Arc::new(move |prompt| {
            p1.prompts.lock().unwrap().push(prompt);
            trusted
        }),
        start: Arc::new(move |_, _, tap_log| {
            p2.starts.fetch_add(1, Ordering::SeqCst);
            tap_log("finder-enter: probe line from the tap thread");
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
        log: Arc::new(move |_, line| p5.lines.lock().unwrap().push(line.to_string())),
        notify: Arc::new(move |t, m| p6.notes.lock().unwrap().push((t.to_string(), m.to_string()))),
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

fn lines(p: &FinderProbe) -> Vec<String> {
    p.lines.lock().unwrap().clone()
}

fn notes(p: &FinderProbe) -> Vec<(String, String)> {
    p.notes.lock().unwrap().clone()
}

const AUTOMATION_TEXT: &str =
    "Enter on Finder: Ferrocast is not allowed to control Finder. Allow it in System Settings > Privacy & Security > Automation.";

#[test]
fn an_empty_selection_logs_a_line_and_posts_no_notification() {
    let probe = Arc::new(FinderProbe::default());
    let mut app = finder_app(&probe, true, Arc::new(|| Ok(vec![])));
    app.set_finder_enter(true, true);
    app.bus.send(Msg::FinderEnter);
    pump(&mut app, |a| count(&probe.reposts) == 1 && !a.finder.busy);
    let l = lines(&probe);
    assert!(l.contains(&"finder-enter: Return intercepted, reading the Finder selection".to_string()), "{l:?}");
    assert!(l.contains(&"finder-enter: selection read ok, empty -> giving the Return back".to_string()), "{l:?}");
    assert!(notes(&probe).is_empty(), "an empty selection is not an error");
}

#[test]
fn a_denied_automation_read_logs_the_error_and_notifies_once_per_minute() {
    let probe = Arc::new(FinderProbe::default());
    let err = "execution error: Not authorized to send Apple events to Finder. (-1743)";
    let mut app = finder_app(&probe, true, Arc::new(move || Err(err.into())));
    app.set_finder_enter(true, true);
    app.bus.send(Msg::FinderEnter);
    pump(&mut app, |a| count(&probe.reposts) == 1 && !a.finder.busy);
    let l = lines(&probe);
    assert!(l.contains(&format!("finder-enter: selection read FAILED: {err} -> giving the Return back")), "{l:?}");
    assert_eq!(notes(&probe), vec![("Ferrocast".to_string(), AUTOMATION_TEXT.to_string())]);
    // a second failure within 60 s: logged again, but no second notification
    app.bus.send(Msg::FinderEnter);
    pump(&mut app, |a| count(&probe.reposts) == 2 && !a.finder.busy);
    assert_eq!(notes(&probe).len(), 1, "rate limited");
    assert_eq!(lines(&probe).iter().filter(|x| x.contains("selection read FAILED")).count(), 2);
    // after the minute has passed a new one is posted
    app.finder.last_failure_notice = Some(Instant::now() - Duration::from_secs(61));
    app.bus.send(Msg::FinderEnter);
    pump(&mut app, |a| count(&probe.reposts) == 3 && !a.finder.busy);
    assert_eq!(notes(&probe).len(), 2);
}

#[test]
fn another_read_error_notifies_with_the_generic_text() {
    let probe = Arc::new(FinderProbe::default());
    let mut app = finder_app(&probe, true, Arc::new(|| Err("timed out after 15 s".into())));
    app.set_finder_enter(true, true);
    app.bus.send(Msg::FinderEnter);
    pump(&mut app, |a| count(&probe.reposts) == 1 && !a.finder.busy);
    assert_eq!(
        notes(&probe),
        vec![("Ferrocast".to_string(), "Enter on Finder could not read the Finder selection: timed out after 15 s".to_string())]
    );
}

#[test]
fn a_selection_logs_the_item_count_and_notifies_nothing() {
    let probe = Arc::new(FinderProbe::default());
    let mut app = finder_app(&probe, true, Arc::new(|| Ok(vec![PathBuf::from("/nonexistent/a.png"), PathBuf::from("/nonexistent/b.png")])));
    app.set_finder_enter(true, true);
    app.bus.send(Msg::FinderEnter);
    pump(&mut app, |a| matches!(a.view(), View::Wheel { .. }));
    let l = lines(&probe);
    assert!(l.contains(&"finder-enter: selection read ok, 2 item(s) -> opening the wheel".to_string()), "{l:?}");
    assert!(!l.iter().any(|x| x.contains("nonexistent")), "no file paths in the log: {l:?}");
    assert!(notes(&probe).is_empty());
}

#[test]
fn the_tap_started_and_not_started_lines_are_logged() {
    let probe = Arc::new(FinderProbe::default());
    let mut app = finder_app(&probe, true, Arc::new(|| Ok(vec![])));
    app.set_finder_enter(true, false);
    let l = lines(&probe);
    assert!(l.contains(&"finder-enter: tap started".to_string()), "{l:?}");
    assert!(l.contains(&"finder-enter: probe line from the tap thread".to_string()), "the tap's log closure reaches the log hook: {l:?}");

    let probe = Arc::new(FinderProbe::default());
    let mut app = finder_app(&probe, false, Arc::new(|| Ok(vec![])));
    app.set_finder_enter(true, false);
    assert_eq!(lines(&probe), vec!["finder-enter: tap NOT started: Accessibility is not granted".to_string()]);

    let probe = Arc::new(FinderProbe::default());
    let mut app = fixture();
    app.finder.hooks = finder_hooks(&probe, true, false, Arc::new(|| Ok(vec![])));
    app.set_finder_enter(true, false);
    assert_eq!(lines(&probe).last().map(String::as_str), Some("finder-enter: tap NOT started: no tap"));
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

/// Timing probe for odd/plans/performance-plan.md (P0), not a pass/fail gate:
/// `cargo test --release -p ferrocast clipboard_sections_timing -- --ignored --nocapture`
#[test]
#[ignore = "timing probe; prints microseconds"]
fn clipboard_sections_timing_1000_entries() {
    let mut app = fixture();
    let now = now_secs();
    *app.shared.clipboard.lock().unwrap() = (0..1000)
        .map(|i| ClipEntry {
            id: format!("clip-{i}"),
            kind: ClipKind::Text,
            content: format!("line {i} of a fairly typical clipboard text entry with some words ").repeat(4),
            title: None,
            copied_at: now - i * 60,
            pinned: false,
            source_app: Some("Safari".into()),
            times_copied: 1,
            width: 0,
            height: 0,
        })
        .collect();
    app.push(View::Clipboard);
    for q in ["", "line 5", "zzz"] {
        app.query = q.into();
        app.invalidate_sections();
        let t = std::time::Instant::now();
        let n: usize = app.sections_cached().iter().map(|s| s.items.len()).sum();
        eprintln!("clipboard sections q={q:?}: {n} items, {:?}", t.elapsed());
    }
}

#[test]
fn a_hidden_launcher_paints_nothing_and_requests_no_repaint_a_visible_one_does() {
    let input =
        || egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(ui::WIDTH, ui::HEIGHT))), ..Default::default() };
    let mut app = fixture();
    let ctx = app.ctx.clone();
    // the search box holds focus, as it does when the launcher was hidden with Escape or by losing focus
    let mut visible_out = None;
    for _ in 0..3 {
        let mut o = ctx.run_ui(input(), |ui| app.draw_if_visible(ui));
        o.textures_delta.clear();
        visible_out = Some(o);
    }
    let visible_out = visible_out.unwrap();
    assert!(!visible_out.shapes.is_empty(), "a visible launcher paints");

    app.visible = false;
    let mut hidden_out = None;
    for _ in 0..3 {
        let mut o = ctx.run_ui(input(), |ui| app.draw_if_visible(ui));
        o.textures_delta.clear();
        hidden_out = Some(o);
    }
    let hidden_out = hidden_out.unwrap();
    assert!(hidden_out.shapes.is_empty(), "a hidden launcher paints nothing: {} shapes", hidden_out.shapes.len());
    let delay = hidden_out.viewport_output.values().map(|v| v.repaint_delay).min().unwrap();
    assert!(delay >= Duration::from_secs(3600), "a hidden launcher asks for no repaint, asked in {delay:?}");

    // showing it again fades in from transparent, as before
    app.visible = true;
    let fade = ctx.animate_bool_with_time(Id::new("appear"), true, 0.14);
    assert!(fade < 0.01, "the next show() starts transparent, got {fade}");
}

#[test]
fn only_messages_that_can_change_the_list_mark_the_sections_dirty() {
    let mut app = fixture();
    render(&mut app, vec![]);
    let _ = app.sections_cached();
    assert!(!app.sec_dirty, "a computed list is clean");

    // up to 40 of these arrive after every query; they only feed the icon cache
    for i in 0..40 {
        app.bus.send(Msg::IconBytes(format!("app:/Applications/A{i}.app"), vec![1, 2, 3]));
    }
    app.drain();
    assert!(!app.sec_dirty, "icon bytes must not rebuild the list");
    assert_eq!(app.icons.prefetched.len(), 40, "the bytes still reach the icon cache");

    // everything else keeps invalidating (clipboard change, apps, files, toasts...)
    for (label, msg) in
        [("Wake", Msg::Wake), ("Apps", Msg::Apps(vec![])), ("Files", Msg::Files("q".into(), vec![])), ("Toast", Msg::Toast("t".into()))]
    {
        app.bus.send(msg);
        app.drain();
        assert!(app.sec_dirty, "{label} marks the list dirty");
        let _ = app.sections_cached();
    }
}

/// Steady-state frame cost of a long list, for odd/plans/performance-plan.md (P4):
/// `cargo test --release -p ferrocast list_frame_timing -- --ignored --nocapture`
#[test]
#[ignore = "timing probe; prints milliseconds per frame"]
fn list_frame_timing_clipboard_1000_and_emoji() {
    let mut app = fixture();
    let now = now_secs();
    *app.shared.clipboard.lock().unwrap() = (0..1000)
        .map(|i| ClipEntry {
            id: format!("clip-{i}"),
            kind: ClipKind::Text,
            content: format!("line {i} of a fairly typical clipboard text entry with some words ").repeat(4),
            title: None,
            copied_at: now - i * 60,
            pinned: false,
            source_app: Some("Safari".into()),
            times_copied: 1,
            width: 0,
            height: 0,
        })
        .collect();
    for (name, view) in [("clipboard 1000", View::Clipboard), ("emoji", View::Emoji), ("root", View::Root)] {
        app.stack = vec![View::Root, view];
        if name == "root" {
            app.stack = vec![View::Root];
        }
        for _ in 0..5 {
            render(&mut app, vec![]);
        }
        let n = 30;
        let t = std::time::Instant::now();
        for _ in 0..n {
            render(&mut app, vec![]);
        }
        eprintln!("{name}: {:?} per frame (mean of {n}, after warm-up)", t.elapsed() / n);
    }
}

fn write_png(dir: &std::path::Path, name: &str, w: u32, h: u32) {
    std::fs::create_dir_all(dir).unwrap();
    image::RgbaImage::from_pixel(w, h, image::Rgba([200, 80, 40, 255])).save(dir.join(name)).unwrap();
}

#[test]
fn a_big_clipboard_image_is_decoded_off_the_ui_thread_and_downscaled() {
    let mut app = fixture();
    write_png(&app.store.images_dir(), "big.png", 2400, 1800);
    app.icons = IconCache::new(app.store.images_dir()).with_bus(app.bus.clone());
    let ctx = app.ctx.clone();
    let icon = Icon::ClipImage("big.png".into());
    app.icons.new_frame();
    assert!(app.icons.texture(&ctx, &icon).is_none(), "the UI thread shows a placeholder, it does not decode the file");
    // the worker answers with PNG bytes through the bus; drain() hands them to the cache
    let t = Instant::now();
    let tex = loop {
        app.drain();
        app.icons.new_frame();
        if let Some(t) = app.icons.texture(&ctx, &icon) {
            break t;
        }
        assert!(t.elapsed() < Duration::from_secs(20), "the image never arrived");
        std::thread::sleep(Duration::from_millis(20));
    };
    let [w, h] = tex.size();
    assert!(w.max(h) <= ui::CLIP_IMAGE_PX as usize, "{w}x{h} must be at most {} px", ui::CLIP_IMAGE_PX);
    assert_eq!(w * 3, h * 4, "the aspect ratio is kept: {w}x{h}");
    // a file that cannot be read settles as "no picture" instead of retrying every frame
    let bad = Icon::ClipImage("missing.png".into());
    let t = Instant::now();
    while {
        app.drain();
        app.icons.new_frame();
        app.icons.texture(&ctx, &bad);
        !app.icons.map.contains_key("clip:missing.png")
    } {
        assert!(t.elapsed() < Duration::from_secs(20), "a missing file never settled");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(app.icons.map["clip:missing.png"].is_none());
}

#[test]
fn clipboard_image_textures_are_capped_least_recently_used_first() {
    let mut app = fixture();
    let dir = app.store.images_dir();
    let total = ui::CLIP_TEXTURES + 8;
    for i in 0..total {
        write_png(&dir, &format!("i{i}.png"), 8, 8);
    }
    let ctx = app.ctx.clone();
    for i in 0..total {
        app.icons.new_frame();
        assert!(app.icons.texture(&ctx, &Icon::ClipImage(format!("i{i}.png"))).is_some(), "i{i}");
        // keep image 0 in use: it must survive the eviction of older, untouched ones
        app.icons.texture(&ctx, &Icon::ClipImage("i0.png".into()));
    }
    let clip_keys = app.icons.map.keys().filter(|k| k.starts_with("clip:")).count();
    assert_eq!(clip_keys, ui::CLIP_TEXTURES, "the cache never holds more than the cap");
    assert!(app.icons.map.contains_key("clip:i0.png"), "the recently used image stays");
    assert!(!app.icons.map.contains_key("clip:i1.png"), "the least recently used one was dropped");
    assert!(app.icons.map.contains_key(&format!("clip:i{}.png", total - 1)), "the newest stays");
    // an evicted image loads again when it is shown again
    app.icons.new_frame();
    assert!(app.icons.texture(&ctx, &Icon::ClipImage("i1.png".into())).is_some());
    // other icon kinds are never evicted
    app.icons.new_frame();
    assert!(app.icons.texture(&ctx, &Icon::Glyph("sf:command", [1, 2, 3])).is_some());
    assert!(app.icons.map.keys().any(|k| k.starts_with("glyph:")));
}

// ---- translate wheel (TW4) --------------------------------------------------------------------

fn open_translate_wheel(app: &mut App) {
    app.bus.send(Msg::WheelOpenText("Hello world".into()));
    app.drain();
}

#[test]
fn translate_wheel_lists_the_configured_languages_in_order() {
    use ferrocast_core::wheel::WheelKind;
    let mut app = fixture();
    let labels = |app: &App| app.wheel_segments(WheelKind::Translate, &[]).into_iter().map(|s| s.label).collect::<Vec<_>>();
    assert_eq!(labels(&app), ferrocast_core::translate::DEFAULT_LANGUAGES);
    app.settings.translate_languages = vec!["Catalan".into(), "Korean".into(), "catalan".into()];
    assert_eq!(labels(&app), ["Catalan", "Korean"]);
    app.settings.translate_languages = (0..15).map(|i| format!("L{i}")).collect();
    assert_eq!(labels(&app).len(), 12);
    assert!(app.wheel_segments(WheelKind::Translate, &[]).iter().all(|s| matches!(s.pick, WheelPick::Translate(_))));
}

#[test]
fn dragged_text_opens_the_translate_wheel_and_a_huge_selection_does_not() {
    use ferrocast_core::wheel::WheelKind;
    let mut app = fixture();
    open_translate_wheel(&mut app);
    assert_eq!(wheel_state(&app).map(|s| (s.0, s.2)), Some((WheelKind::Translate, 8)));
    let mut app = fixture();
    app.bus.send(Msg::WheelOpenText("x".repeat(ferrocast_core::translate::MAX_CHARS + 1)));
    app.drain();
    assert!(wheel_state(&app).is_none(), "no wheel for a selection that is too long");
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("too long")), "{:?}", app.toast);
}

#[test]
fn translate_wheel_renders_in_both_themes() {
    let mut app = fixture();
    for theme in [egui::ThemePreference::Dark, egui::ThemePreference::Light] {
        app.ctx.set_theme(theme);
        open_translate_wheel(&mut app);
        render(&mut app, vec![]);
        render(&mut app, vec![key_event(Key::ArrowRight)]);
        assert!(wheel_state(&app).is_some());
    }
}

#[test]
fn modifier_taps_do_not_switch_a_translate_wheel() {
    use ferrocast_core::wheel::WheelKind;
    let mut app = fixture();
    open_translate_wheel(&mut app);
    render_mods(&mut app, vec![], Modifiers::NONE);
    for m in [Modifiers::SHIFT, Modifiers::NONE, Modifiers::COMMAND, Modifiers::NONE, Modifiers::CTRL] {
        render_mods(&mut app, vec![], m);
        assert_eq!(wheel_state(&app).unwrap().0, WheelKind::Translate);
    }
    app.switch_wheel(WheelKind::Actions);
    assert_eq!(wheel_state(&app).unwrap().0, WheelKind::Translate);
}

#[test]
fn a_failed_translation_toasts_and_does_not_touch_the_view() {
    let mut app = fixture();
    app.bus.send(Msg::Translated {
        run: 0,
        lang: "Spanish".into(),
        source_pid: None,
        result: Err("Claude Code is not installed.".into()),
        ms: 0,
        target: ferrocast_core::translate::Target::Other,
    });
    app.drain();
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("Claude Code is not installed")), "{:?}", app.toast);
    assert!(matches!(app.view(), View::Root));
}

#[test]
fn a_shown_translation_opens_the_translation_view() {
    let mut app = fixture();
    app.settings.translate_output = ferrocast_core::models::TranslateOutput::Show;
    app.bus.send(Msg::Translated {
        run: 0,
        lang: "Spanish".into(),
        source_pid: None,
        result: Ok("Hola".into()),
        ms: 5,
        target: ferrocast_core::translate::Target::Other,
    });
    app.drain();
    assert_shows_hola(&app);
}

#[test]
fn auto_is_the_default_and_old_settings_load_as_auto() {
    use ferrocast_core::models::TranslateOutput;
    assert_eq!(Settings::default().translate_output, TranslateOutput::Auto);
    let s: Settings = serde_json::from_str("{\"theme\":\"Dark\"}").unwrap();
    assert_eq!(s.translate_output, TranslateOutput::Auto);
    let s: Settings = serde_json::from_str("{\"translate_output\":\"Show\"}").unwrap();
    assert_eq!(s.translate_output, TranslateOutput::Show, "an explicit choice is kept");
}

#[test]
fn auto_with_an_unscriptable_app_shows_the_window() {
    use ferrocast_core::{models::TranslateOutput::Auto, translate::Target};
    let mut app = fixture();
    translated(&mut app, Auto, Target::Other, Some(7));
    assert_shows_hola(&app);
    assert!(fx_taken().is_empty(), "no side effect");
}

#[test]
fn auto_in_a_text_field_replaces_through_accessibility_and_does_not_open_the_window() {
    use ferrocast_core::{models::TranslateOutput::Auto, translate::Target};
    let mut app = fixture();
    FX_FRONT.with(|f| f.set(Some(7)));
    translated(&mut app, Auto, Target::Editable, Some(7));
    assert_eq!(fx_taken(), vec!["ax 7".to_string()]);
    assert!(matches!(app.view(), View::Root));
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("Replacing")), "{:?}", app.toast);
}

#[test]
fn auto_in_a_browser_page_goes_to_the_web_replace_not_the_window() {
    use ferrocast_core::translate::{Browser, Target};
    let mut app = fixture();
    FX_FRONT.with(|f| f.set(Some(7)));
    translated(&mut app, ferrocast_core::models::TranslateOutput::Auto, Target::Browser(Browser::Safari), Some(7));
    assert_eq!(fx_taken(), vec!["web Safari".to_string()]);
    assert!(matches!(app.view(), View::Root), "the window waits for the result");
}

#[test]
fn auto_downgrades_to_show_when_another_app_is_in_front() {
    use ferrocast_core::translate::{Browser, Target};
    for target in [Target::Editable, Target::Browser(Browser::Safari)] {
        let mut app = fixture();
        FX_FRONT.with(|f| f.set(Some(99)));
        translated(&mut app, ferrocast_core::models::TranslateOutput::Auto, target, Some(7));
        assert_shows_hola(&app);
        assert!(fx_taken().is_empty(), "never write into another app");
    }
}

#[test]
fn the_other_output_settings_do_not_use_the_auto_path() {
    use ferrocast_core::{models::TranslateOutput as O, translate::Target};
    let mut app = fixture();
    translated(&mut app, O::Show, Target::Editable, Some(7));
    assert_shows_hola(&app);
    assert!(fx_taken().is_empty());
}

#[test]
fn a_web_replace_that_worked_toasts_and_leaves_the_view() {
    use ferrocast_core::translate::WebResult;
    let mut app = fixture();
    app.bus.send(Msg::WebReplaced { lang: "Spanish".into(), text: "Hola".into(), result: WebResult::Replaced });
    app.drain();
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("Replaced in the page")), "{:?}", app.toast);
    assert!(matches!(app.view(), View::Root));
}

#[test]
fn a_web_replace_without_selection_falls_back_to_the_window() {
    use ferrocast_core::translate::WebResult;
    let mut app = fixture();
    app.bus.send(Msg::WebReplaced { lang: "Spanish".into(), text: "Hola".into(), result: WebResult::NoSelection });
    app.drain();
    assert_shows_hola(&app);
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("selection was lost")), "{:?}", app.toast);
}

#[test]
fn a_refused_web_replace_shows_the_window_and_explains_the_permission() {
    use ferrocast_core::translate::{WebResult, JS_PERMISSION_HINT};
    let mut app = fixture();
    let err = "execution error: Safari got an error: You must enable 'Allow JavaScript from Apple Events' in the Developer section. (8)";
    app.bus.send(Msg::WebReplaced { lang: "Spanish".into(), text: "Hola".into(), result: WebResult::Failed(err.into()) });
    app.drain();
    assert_shows_hola(&app);
    assert_eq!(app.toast.as_ref().map(|t| t.0.as_str()), Some(JS_PERMISSION_HINT));
}

#[test]
fn another_web_failure_shows_the_window_and_the_first_error_line() {
    use ferrocast_core::translate::WebResult;
    let mut app = fixture();
    app.bus.send(Msg::WebReplaced { lang: "Spanish".into(), text: "Hola".into(), result: WebResult::Failed("weird\nsecond".into()) });
    app.drain();
    assert_shows_hola(&app);
    let toast = app.toast.as_ref().map(|t| t.0.clone()).unwrap_or_default();
    assert!(toast.contains("weird") && !toast.contains("second"), "{toast}");
}

#[test]
fn a_failed_ax_replace_pastes_once_and_a_good_one_does_not() {
    let mut app = fixture();
    app.bus.send(Msg::AxReplaced { lang: "Spanish".into(), text: "Hola".into(), ok: true });
    app.drain();
    assert!(fx_taken().is_empty());
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("Replaced")), "{:?}", app.toast);
    app.bus.send(Msg::AxReplaced { lang: "Spanish".into(), text: "Hola".into(), ok: false });
    app.drain();
    assert_eq!(fx_taken(), vec!["paste".to_string()]);
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("Pasted")), "{:?}", app.toast);
}

#[test]
fn the_after_translating_row_renders_with_and_without_the_javascript_note() {
    use ferrocast_core::models::TranslateOutput as O;
    let mut app = fixture();
    for theme in [egui::ThemePreference::Dark, egui::ThemePreference::Light] {
        app.ctx.set_theme(theme);
        for out in [O::Auto, O::Show] {
            app.settings.translate_output = out;
            app.stack = vec![View::Root, View::Settings];
            app.ctx.data_mut(|d| d.insert_temp(Id::new("settings_tab"), 4usize));
            render(&mut app, vec![]);
            render(&mut app, vec![]);
            assert!(matches!(app.view(), View::Settings));
            let shown = app.ctx.data(|d| d.get_temp::<bool>(Id::new("auto-js-note"))).unwrap_or(false);
            assert_eq!(shown, out == O::Auto, "{out:?} {theme:?}");
        }
    }
}

#[test]
fn the_javascript_note_text_is_defined_once_and_names_both_browsers() {
    let n = draw::AUTO_JS_NOTE;
    assert!(n.contains("Allow JavaScript from Apple Events") && n.contains("Safari") && n.contains("Chrome"), "{n}");
}

#[test]
fn translation_view_renders_in_both_themes() {
    let mut app = fixture();
    for theme in [egui::ThemePreference::Dark, egui::ThemePreference::Light] {
        app.ctx.set_theme(theme);
        app.stack = vec![View::Root, View::Translation { lang: "Spanish".into(), text: "Hola\nmundo ".repeat(200) }];
        render(&mut app, vec![]);
        render(&mut app, vec![]);
        assert!(matches!(app.view(), View::Translation { .. }));
        render(&mut app, vec![key_event(Key::Escape)]);
        assert!(matches!(app.view(), View::Root), "escape closes");
    }
}

#[test]
fn the_translate_log_line_has_language_and_count_but_not_the_text() {
    let ok = translate_log_line("Spanish", "Claude Code", &Ok("zanahoria secreta".into()), 3700);
    assert!(ok.contains("Spanish") && ok.contains("chars=17") && ok.contains("ms=3700") && ok.contains("Claude Code"), "{ok}");
    assert!(!ok.contains("zanahoria"), "{ok}");
    let bad = translate_log_line("Spanish", "Claude Code", &Err("boom\nsecond line zanahoria".into()), 10);
    assert!(bad.contains("failed: boom") && !bad.contains("zanahoria"), "{bad}");
}

#[test]
fn the_log_line_of_a_translate_wheel_has_no_text() {
    use ferrocast_core::wheel::WheelKind;
    let app = fixture();
    let segs = app.wheel_segments(WheelKind::Translate, &[]);
    let line = wheel_log_line("open", WheelKind::Translate, &segs, 0);
    assert!(line.contains("Translate") && line.contains("segs=8"), "{line}");
    let pick = wheel_pick_log_line(WheelKind::Translate, "Spanish", "Root", false);
    assert!(pick.contains("Spanish"), "{pick}");
}

#[test]
fn settings_show_the_translation_controls_and_keep_other_settings_untouched() {
    let mut app = fixture();
    app.stack = vec![View::Root, View::Settings];
    for tab in [0usize, 3, 4] {
        app.ctx.data_mut(|d| d.insert_temp(Id::new("settings_tab"), tab));
        for theme in [egui::ThemePreference::Dark, egui::ThemePreference::Light] {
            app.ctx.set_theme(theme);
            render(&mut app, vec![]);
            render(&mut app, vec![]);
        }
    }
    // drawing alone changes nothing: defaults stay, chat provider untouched
    let d = Settings::default();
    assert_eq!(app.settings.translate_languages, d.translate_languages);
    assert_eq!(app.settings.translate_provider, None);
    assert_eq!(app.settings.ai_provider, d.ai_provider);
}

// ---------------------------------------------------------------- updates (no network, no threads)

fn fake_release(ver: &str) -> ferrocast_core::update::Release {
    use ferrocast_core::update::{Release, Version};
    Release {
        version: Version::parse(ver).unwrap(),
        tag: format!("v{ver}"),
        zip_url: format!("https://example.invalid/Ferrocast-{ver}.zip"),
        sums_url: "https://example.invalid/SHA256SUMS.txt".into(),
        notes: "- faster\n- fixes".into(),
        html_url: "https://example.invalid/release".into(),
    }
}

fn toast_text(app: &App) -> String {
    app.toast.as_ref().map(|t| t.0.clone()).unwrap_or_default()
}

#[test]
fn manual_check_without_release_toasts_up_to_date() {
    use ferrocast_core::update::Channel;
    let mut app = fixture();
    app.handle_update_checked(true, Ok(None), Channel::Direct, true);
    assert!(toast_text(&app).contains("up to date"), "{}", toast_text(&app));
    assert!(app.settings.update_last_check.is_some());
}

#[test]
fn available_in_ask_first_opens_the_update_view() {
    use ferrocast_core::update::Channel;
    let mut app = fixture();
    app.settings.update_install = UpdateInstall::AskFirst;
    app.handle_update_checked(false, Ok(Some(fake_release("99.0.0"))), Channel::Direct, true);
    match app.view() {
        View::Update { release } => assert_eq!(release.tag, "v99.0.0"),
        _ => panic!("expected the update view"),
    }
}

#[test]
fn skipped_release_is_silent_on_automatic_and_shown_on_manual() {
    use ferrocast_core::update::Channel;
    let mut app = fixture();
    app.settings.update_skipped = Some("99.0.0".into());
    app.handle_update_checked(false, Ok(Some(fake_release("99.0.0"))), Channel::Direct, true);
    assert!(matches!(app.view(), View::Root));
    assert!(toast_text(&app).is_empty());
    app.handle_update_checked(true, Ok(Some(fake_release("99.0.0"))), Channel::Direct, true);
    assert!(matches!(app.view(), View::Update { .. }));
}

#[test]
fn automatic_install_on_dev_channel_toasts_refusal_without_view() {
    use ferrocast_core::update::Channel;
    let mut app = fixture();
    app.settings.update_install = UpdateInstall::Automatic;
    app.handle_update_checked(false, Ok(Some(fake_release("99.0.0"))), Channel::Dev, true);
    assert!(matches!(app.view(), View::Root));
    assert!(toast_text(&app).contains("Development build"), "{}", toast_text(&app));
}

#[test]
fn update_failed_toasts_and_keeps_the_view() {
    let mut app = fixture();
    app.stack.push(View::Settings);
    app.bus.send(Msg::UpdateFailed("checksum mismatch".into()));
    app.drain();
    assert!(matches!(app.view(), View::Settings));
    assert!(toast_text(&app).contains("checksum mismatch"));
}

#[test]
fn automatic_check_error_only_logs_and_manual_error_toasts() {
    use ferrocast_core::update::Channel;
    let mut app = fixture();
    app.handle_update_checked(false, Err("offline".into()), Channel::Direct, true);
    assert!(toast_text(&app).is_empty());
    app.handle_update_checked(true, Err("offline".into()), Channel::Direct, true);
    assert!(toast_text(&app).contains("offline"));
}

#[test]
fn poller_is_not_started_when_update_check_is_off() {
    let mut app = fixture();
    app.settings.update_check = false;
    *app.shared.settings.write().unwrap() = app.settings.clone();
    assert!(services::start_update_poller(app.shared.clone(), app.bus.clone()).is_none());
}

// ---- update advisories (L20)

#[test]
fn poller_starts_once_when_the_toggle_is_turned_on() {
    let mut app = fixture();
    app.settings.update_check = false;
    *app.shared.settings.write().unwrap() = app.settings.clone();
    assert!(app.update_poller.is_none());
    // an unrelated change while off starts nothing
    let mut s = app.settings.clone();
    s.compact_mode = !s.compact_mode;
    app.commit_settings(s);
    assert!(app.update_poller.is_none());
    // off -> on through the settings commit path starts the poller
    let mut s = app.settings.clone();
    s.update_check = true;
    app.commit_settings(s);
    assert!(app.update_poller.is_some());
    let id = app.update_poller.as_ref().unwrap().thread().id();
    // off and on again keeps the same single poller
    let mut s = app.settings.clone();
    s.update_check = false;
    app.commit_settings(s);
    let mut s = app.settings.clone();
    s.update_check = true;
    app.commit_settings(s);
    assert_eq!(app.update_poller.as_ref().unwrap().thread().id(), id);
}

static INSTALL_GUARD_TESTS: Mutex<()> = Mutex::new(());

#[test]
fn install_guard_is_exclusive_and_released_on_drop() {
    let _l = INSTALL_GUARD_TESTS.lock().unwrap_or_else(|e| e.into_inner());
    let flag = std::sync::atomic::AtomicBool::new(false);
    let first = services::InstallGuard::take(&flag).expect("free");
    assert!(services::InstallGuard::take(&flag).is_none());
    drop(first);
    assert!(services::InstallGuard::take(&flag).is_some());
    // released on panic as well
    let r = std::panic::catch_unwind(|| {
        let f = std::sync::atomic::AtomicBool::new(false);
        let _g = services::InstallGuard::take(&f).unwrap();
        panic!("boom");
    });
    assert!(r.is_err());
}

#[test]
fn second_install_or_brew_upgrade_is_refused_while_one_runs() {
    let _l = INSTALL_GUARD_TESTS.lock().unwrap_or_else(|e| e.into_inner());
    let app = fixture();
    let held = services::InstallGuard::take(services::install_flag()).expect("free");
    services::update_install(fake_release("99.0.0"), app.store.root.clone(), app.bus.clone());
    services::brew_upgrade(app.bus.clone());
    for _ in 0..2 {
        match app.rx.recv_timeout(std::time::Duration::from_secs(5)).expect("a message") {
            Msg::UpdateFailed(m) => assert_eq!(m, "An update is already being installed"),
            _ => panic!("expected UpdateFailed"),
        }
    }
    assert!(app.rx.try_recv().is_err());
    drop(held);
}

#[test]
fn pick_exe_prefers_the_resolved_path() {
    let raw = PathBuf::from("/Applications/Ferrocast.app/Contents/MacOS/ferrocast");
    let real = PathBuf::from("/opt/homebrew/Cellar/ferrocast/0.1.0/Ferrocast.app/Contents/MacOS/ferrocast");
    assert_eq!(instance::pick_exe(raw.clone(), Some(real.clone())), real);
    assert_eq!(instance::pick_exe(raw.clone(), None), raw);
}

#[test]
fn symlinked_applications_bundle_into_the_cellar_is_brew() {
    use ferrocast_core::update::{channel_of, Channel};
    let root = std::env::temp_dir().join(format!("ferrocast-sym-{}", uuid::Uuid::new_v4()));
    let macos = root.join("Cellar/ferrocast/0.1.0/Ferrocast.app/Contents/MacOS");
    std::fs::create_dir_all(&macos).unwrap();
    std::fs::write(macos.join("ferrocast"), b"bin").unwrap();
    let apps = root.join("Applications");
    std::fs::create_dir_all(&apps).unwrap();
    std::os::unix::fs::symlink(root.join("Cellar/ferrocast/0.1.0/Ferrocast.app"), apps.join("Ferrocast.app")).unwrap();
    let raw = apps.join("Ferrocast.app/Contents/MacOS/ferrocast");
    assert_ne!(channel_of(&raw), Channel::Brew, "the raw path hides the Cellar");
    let resolved = instance::resolve_exe(raw);
    assert_eq!(channel_of(&resolved), Channel::Brew);
    assert!(instance::bundle_of(&resolved).unwrap().to_string_lossy().contains("/Cellar/"));
    // a path that does not exist falls back to itself
    let ghost = root.join("nope/Ferrocast.app/Contents/MacOS/ferrocast");
    assert_eq!(instance::resolve_exe(ghost.clone()), ghost);
    let _ = std::fs::remove_dir_all(&root);
}

fn swap_dirs(tag: &str) -> (PathBuf, PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!("ferrocast-swap-{tag}-{}", uuid::Uuid::new_v4()));
    let (bundle, new_app) = (root.join("apps/Ferrocast.app"), root.join("extract/Ferrocast.app"));
    std::fs::create_dir_all(bundle.join("Contents")).unwrap();
    std::fs::write(bundle.join("Contents/v"), "old").unwrap();
    (root, bundle, new_app)
}

#[test]
fn swap_bundle_places_the_new_app_and_keeps_the_old_as_previous() {
    let (root, bundle, new_app) = swap_dirs("ok");
    std::fs::create_dir_all(new_app.join("Contents")).unwrap();
    std::fs::write(new_app.join("Contents/v"), "new").unwrap();
    services::swap_bundle(&new_app, &bundle).unwrap();
    assert_eq!(std::fs::read_to_string(bundle.join("Contents/v")).unwrap(), "new");
    let previous = root.join("apps/Ferrocast.app.previous");
    assert_eq!(std::fs::read_to_string(previous.join("Contents/v")).unwrap(), "old");
    assert!(!new_app.exists());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn swap_bundle_replaces_a_stale_previous() {
    let (root, bundle, new_app) = swap_dirs("stale");
    let previous = root.join("apps/Ferrocast.app.previous");
    std::fs::create_dir_all(&previous).unwrap();
    std::fs::write(previous.join("stale.txt"), "stale").unwrap();
    std::fs::create_dir_all(new_app.join("Contents")).unwrap();
    std::fs::write(new_app.join("Contents/v"), "new").unwrap();
    services::swap_bundle(&new_app, &bundle).unwrap();
    assert!(!previous.join("stale.txt").exists());
    assert_eq!(std::fs::read_to_string(previous.join("Contents/v")).unwrap(), "old");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn swap_bundle_rolls_back_when_placing_fails() {
    let (root, bundle, new_app) = swap_dirs("fail"); // new_app was never created
    let err = services::swap_bundle(&new_app, &bundle).unwrap_err();
    assert!(err.contains("old version was restored"), "{err}");
    assert_eq!(std::fs::read_to_string(bundle.join("Contents/v")).unwrap(), "old");
    assert!(!root.join("apps/Ferrocast.app.previous").exists());
    let entries: Vec<_> = std::fs::read_dir(root.join("apps")).unwrap().flatten().collect();
    assert_eq!(entries.len(), 1, "no partial copy left behind");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn skipping_a_version_persists_it() {
    let mut app = fixture();
    app.skip_update(&fake_release("99.0.0"));
    assert_eq!(app.settings.update_skipped.as_deref(), Some("99.0.0"));
    let saved: Settings = app.store.load("settings.json");
    assert_eq!(saved.update_skipped.as_deref(), Some("99.0.0"));
}

#[test]
fn update_view_renders_in_both_themes() {
    let mut app = fixture();
    app.stack = vec![View::Root, View::Update { release: fake_release("99.0.0") }];
    for dark in [true, false] {
        app.dark = dark;
        render(&mut app, vec![]);
    }
}

#[test]
fn bundle_of_finds_the_app_directory() {
    let exe = std::path::Path::new("/Applications/Ferrocast.app/Contents/MacOS/ferrocast");
    assert_eq!(instance::bundle_of(exe), Some(PathBuf::from("/Applications/Ferrocast.app")));
    assert_eq!(instance::bundle_of(std::path::Path::new("/usr/bin/ferrocast")), None);
}

// ---- RU4: Updates controls in Settings, About and the tray

#[test]
fn drawing_general_tab_and_about_changes_no_update_setting() {
    for dark in [true, false] {
        let mut app = fixture();
        app.settings.theme = if dark { Theme::Dark } else { Theme::Light };
        app.settings.update_check = true;
        app.stack = vec![View::Root, View::Settings];
        app.ctx.data_mut(|d| d.insert_temp(Id::new("settings_tab"), 0usize));
        render(&mut app, vec![]);
        render(&mut app, vec![]);
        app.stack = vec![View::Root, View::About];
        render(&mut app, vec![]);
        render(&mut app, vec![]);
        assert!(app.settings.update_check);
        assert_eq!(app.settings.update_install, UpdateInstall::AskFirst);
    }
}

#[test]
fn general_tab_shows_available_release_without_changing_settings() {
    let mut app = fixture();
    app.update_available = Some(fake_release("99.0.0"));
    app.stack = vec![View::Root, View::Settings];
    app.ctx.data_mut(|d| d.insert_temp(Id::new("settings_tab"), 0usize));
    render(&mut app, vec![]);
    assert!(app.settings.update_check);
    assert!(matches!(app.view(), View::Settings));
}

#[test]
fn tray_menu_has_check_for_updates_before_the_separator() {
    let items = services::tray_items();
    let at = items.iter().position(|(id, label)| *id == "check_updates" && *label == "Check for Updates…");
    assert!(at.is_some(), "{items:?}");
    assert!(items.iter().any(|(id, _)| *id == "settings"));
}

#[test]
fn tray_menu_offers_stop_recording_first_only_while_recording() {
    assert_eq!(services::tray_menu_items(false), services::tray_items().to_vec());
    let rec = services::tray_menu_items(true);
    assert_eq!(rec[0], ("stop_recording", "Stop Recording"));
    assert_eq!(rec[1..], services::tray_items());
}

#[test]
fn the_tray_stop_recording_item_requests_a_stop_only_while_recording() {
    let mut app = fixture();
    app.bus.send(Msg::Tray("stop_recording".into()));
    app.drain();
    assert!(app.record.is_idle(), "nothing to stop");
    app.record.begin(0);
    app.bus.send(Msg::Tray("stop_recording".into()));
    app.drain();
    assert_eq!(app.record.phase(), crate::record_ctl::Phase::Stopping);
}

#[test]
fn check_now_entry_point_toasts_and_requests_a_manual_check() {
    let mut app = fixture();
    let exe = std::path::PathBuf::from("/Applications/Ferrocast.app/Contents/MacOS/ferrocast");
    let calls = std::cell::RefCell::new(vec![]);
    app.check_updates_via(exe.clone(), |_bus, manual, e| calls.borrow_mut().push((manual, e)));
    assert_eq!(toast_text(&app), "Checking for updates…");
    assert_eq!(*calls.borrow(), vec![(true, exe)]);
}

#[test]
fn update_checks_record_the_available_release() {
    use ferrocast_core::update::Channel;
    let mut app = fixture();
    app.handle_update_checked(false, Ok(Some(fake_release("99.0.0"))), Channel::Direct, true);
    assert!(app.update_available.is_some());
    app.handle_update_checked(true, Ok(None), Channel::Direct, true);
    assert!(app.update_available.is_none());
}

/// Scroll the settings pane the way a user would until the segmented bar with `n` cells (the
/// lowest one when several) sits on screen, and return its cells.
fn reach_bar(app: &mut App, n: usize) -> Option<Vec<Rect>> {
    let find = |app: &App| -> Option<Vec<Rect>> {
        let bars: Vec<(Rect, Vec<Rect>)> = app.ctx.data(|d| d.get_temp(Id::new("segmented-count"))).unwrap_or_default();
        bars.into_iter().filter(|(_, c)| c.len() == n).max_by(|a, b| a.0.min.y.total_cmp(&b.0.min.y)).map(|(_, c)| c)
    };
    scroll_until(app, |app| {
        app.ctx.data_mut(|d| d.remove::<Vec<(Rect, Vec<Rect>)>>(Id::new("segmented-count")));
        render(app, vec![]);
        find(app).map(|c| c[0].center())
    })?;
    for _ in 0..30 {
        render(app, vec![]);
    }
    app.ctx.data_mut(|d| d.remove::<Vec<(Rect, Vec<Rect>)>>(Id::new("segmented-count")));
    render(app, vec![]);
    find(app)
}

/// Scroll down in steps until `probe` reports a point inside the visible pane.
fn scroll_until(app: &mut App, mut probe: impl FnMut(&mut App) -> Option<Pos2>) -> Option<Pos2> {
    render(app, vec![Event::PointerMoved(Pos2::new(500.0, 200.0))]);
    for _ in 0..14 {
        let p = probe(app)?;
        if p.y < ui::HEIGHT - 60.0 && p.y > 80.0 {
            return Some(p);
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
    None
}

/// The four-cell "After translating" bar is the only four-cell bar of the Translate tab.
fn after_translating_cells(app: &mut App) -> Option<Vec<Rect>> {
    reach_bar(app, 4)
}

#[test]
fn clicking_each_after_translating_cell_selects_that_option() {
    use ferrocast_core::models::TranslateOutput as T;
    for (cell, want) in [(0usize, T::Auto), (1, T::Show), (2, T::Replace), (3, T::Copy)] {
        let mut app = fixture();
        app.stack = vec![View::Root, View::Settings];
        app.ctx.data_mut(|d| d.insert_temp(Id::new("settings_tab"), 4usize));
        // start from a different value than the one clicked
        app.settings.translate_output = if want == T::Copy { T::Auto } else { T::Copy };
        let cells = after_translating_cells(&mut app).expect("the After translating bar is reachable");
        click_at(&mut app, cells[cell].center());
        render(&mut app, vec![]);
        assert_eq!(app.settings.translate_output, want, "cell {cell}");
    }
}

// ---- Translate settings tab (provider and model)

const TRANSLATE_TAB: usize = 4;

/// Cache an empty model list for every provider so drawing never starts a fetch (no network).
fn seed_model_lists(app: &mut App) {
    use ferrocast_core::models::AiProvider as P;
    for p in [P::OpenAiCompatible, P::Anthropic, P::ChatGpt, P::ClaudeCode] {
        let key = services::models_key(&Settings { ai_provider: p, ..app.settings.clone() });
        app.ai_models.insert(key, Some(Ok(vec![])));
    }
}

fn translate_tab_app() -> App {
    let mut app = fixture();
    seed_model_lists(&mut app);
    app.stack = vec![View::Root, View::Settings];
    app.ctx.data_mut(|d| d.insert_temp(Id::new("settings_tab"), TRANSLATE_TAB));
    app
}

fn tab_bars(tab: usize) -> Vec<usize> {
    let mut app = fixture();
    seed_model_lists(&mut app);
    app.settings.translate_output = ferrocast_core::models::TranslateOutput::Auto;
    app.stack = vec![View::Root, View::Settings];
    app.ctx.data_mut(|d| d.insert_temp(Id::new("settings_tab"), tab));
    // rows learn their control width in the first frames (a wide control moves under its text):
    // read the bars of the settled frame only
    render(&mut app, vec![]);
    render(&mut app, vec![]);
    app.ctx.data_mut(|d| d.remove::<Vec<(Rect, Vec<Rect>)>>(Id::new("segmented-count")));
    render(&mut app, vec![]);
    let mut bars: Vec<(Rect, Vec<Rect>)> = app.ctx.data(|d| d.get_temp(Id::new("segmented-count"))).unwrap_or_default();
    bars.sort_by(|a, b| (a.0.min.y, a.0.min.x).partial_cmp(&(b.0.min.y, b.0.min.x)).unwrap());
    bars.iter().map(|(_, c)| c.len()).collect()
}

fn auto_js_note_shown(app: &App) -> bool {
    app.ctx.data(|d| d.get_temp::<bool>(Id::new("auto-js-note"))).unwrap_or(false)
}

#[test]
fn the_translate_tab_renders_with_every_provider_in_both_themes_and_changes_nothing() {
    use ferrocast_core::models::AiProvider as P;
    for provider in [None, Some(P::OpenAiCompatible), Some(P::ChatGpt), Some(P::Anthropic), Some(P::ClaudeCode)] {
        for theme in [egui::ThemePreference::Dark, egui::ThemePreference::Light] {
            let mut app = translate_tab_app();
            app.ctx.set_theme(theme);
            app.settings.translate_provider = provider;
            app.settings.translate_model = if provider.is_some() { "some-model".into() } else { String::new() };
            let before = app.settings.clone();
            render(&mut app, vec![]);
            render(&mut app, vec![]);
            assert!(matches!(app.view(), View::Settings));
            assert!(auto_js_note_shown(&app), "the Translate tab draws the Auto note ({provider:?})");
            assert_eq!(app.settings, before, "drawing alone changes no setting ({provider:?} {theme:?})");
        }
    }
}

#[test]
fn general_and_ai_tabs_no_longer_draw_the_translation_rows() {
    // The Auto note only exists on the Translate tab now.
    for tab in [0usize, 3, 5, 6, 7, INTEGRATIONS_TAB, 9] {
        let mut app = translate_tab_app();
        app.ctx.data_mut(|d| d.insert_temp(Id::new("settings_tab"), tab));
        render(&mut app, vec![]);
        render(&mut app, vec![]);
        assert!(!auto_js_note_shown(&app), "tab {tab} must not draw After translating");
    }
    // Segmented bars, top to bottom. General lost the 3-cell Translate key and the 4-cell After
    // translating bars; AI lost the 5-cell Translation engine bar.
    assert_eq!(tab_bars(0), vec![3, 4, 4, 4, 2, 3], "General: theme, convert, tools, actions, update install, sensitivity");
    assert_eq!(tab_bars(3), vec![4], "AI: only the chat provider bar");
    assert_eq!(tab_bars(4), vec![3, 4, 5], "Translate: wheel key, after translating, provider");
}

#[test]
fn scripts_and_advanced_tabs_keep_rendering_at_their_new_numbers() {
    for tab in [6usize, 7, INTEGRATIONS_TAB, 9] {
        for theme in [egui::ThemePreference::Dark, egui::ThemePreference::Light] {
            let mut app = translate_tab_app();
            app.ctx.set_theme(theme);
            app.ctx.data_mut(|d| d.insert_temp(Id::new("settings_tab"), tab));
            render(&mut app, vec![]);
            render(&mut app, vec![]);
            assert!(matches!(app.view(), View::Settings));
        }
    }
    // Scripts body has the "Add directory" row, Advanced the units: they are on different tabs
    assert_eq!(tab_bars(6), Vec::<usize>::new());
    assert_eq!(tab_bars(7), Vec::<usize>::new());
    assert_eq!(tab_bars(INTEGRATIONS_TAB), Vec::<usize>::new());
    assert_eq!(tab_bars(9), Vec::<usize>::new());
}

#[test]
fn clicking_each_provider_cell_selects_it() {
    use ferrocast_core::models::AiProvider as P;
    let want = [None, Some(P::OpenAiCompatible), Some(P::ChatGpt), Some(P::Anthropic), Some(P::ClaudeCode)];
    for (cell, want) in want.into_iter().enumerate() {
        let mut app = translate_tab_app();
        app.settings.translate_provider = if want == Some(P::ChatGpt) { None } else { Some(P::ChatGpt) };
        let cells = reach_bar(&mut app, 5).expect("the provider bar is reachable");
        click_at(&mut app, cells[cell].center());
        assert_eq!(app.settings.translate_provider, want, "cell {cell}");
    }
}

/// Open the combo `id` on the Translate tab and return the recorded (label, rect) of its entries.
fn open_model_combo(app: &mut App, id: &str) -> Vec<(String, Rect)> {
    let probe_id = id.to_string();
    let p = scroll_until(app, |app| {
        render(app, vec![]);
        let combos: Vec<(String, Rect)> = app.ctx.data(|d| d.get_temp(Id::new("model-combos"))).unwrap_or_default();
        combos.into_iter().find(|(i, _)| *i == probe_id).map(|(_, r)| r.center())
    })
    .expect("the model combo is reachable");
    // let the scroll settle, then read the combo again: its place moves while the pane scrolls
    for _ in 0..30 {
        render(app, vec![]);
    }
    let combos: Vec<(String, Rect)> = app.ctx.data(|d| d.get_temp(Id::new("model-combos"))).unwrap_or_default();
    let p = combos.into_iter().find(|(i, _)| *i == id).map(|(_, r)| r.center()).unwrap_or(p);
    click_at(app, p);
    render(app, vec![]);
    app.ctx.data(|d| d.get_temp(Id::new("model-items"))).unwrap_or_default()
}

#[test]
fn clicking_a_model_sets_translate_model_and_provider_default_clears_it() {
    use ferrocast_core::models::AiProvider as P;
    let cases = [(P::ChatGpt, services::CHATGPT_MODELS[2]), (P::ClaudeCode, services::CLAUDE_CODE_MODELS[1])];
    for (provider, model) in cases {
        let mut app = translate_tab_app();
        app.settings.translate_provider = Some(provider);
        app.settings.translate_model = String::new();
        let items = open_model_combo(&mut app, "translate");
        assert_eq!(items.first().map(|(l, _)| l.as_str()), Some("Provider default"), "{provider:?}: {items:?}");
        assert!(items.iter().any(|(l, _)| l == "Custom…"), "{items:?}");
        let (_, r) = items.iter().find(|(l, _)| l == model).unwrap_or_else(|| panic!("{model} listed: {items:?}"));
        click_at(&mut app, r.center());
        assert_eq!(app.settings.translate_model, model, "{provider:?}");
        assert_eq!(app.settings.translate_provider, Some(provider));
        assert_eq!(app.settings.ai_model, Settings::default().ai_model, "the chat model is untouched");
        // back to the provider default
        let items = open_model_combo(&mut app, "translate");
        let (_, r) = items.iter().find(|(l, _)| l == "Provider default").unwrap();
        click_at(&mut app, r.center());
        assert_eq!(app.settings.translate_model, "", "{provider:?}");
    }
}

#[test]
fn model_lists_of_fetched_providers_come_from_that_providers_cache() {
    use ferrocast_core::models::AiProvider as P;
    let mut app = translate_tab_app();
    app.settings.ai_provider = P::ChatGpt; // chat uses another provider than the translation
    app.settings.translate_provider = Some(P::Anthropic);
    let key = services::models_key(&Settings { ai_provider: P::Anthropic, ..app.settings.clone() });
    app.ai_models.insert(key, Some(Ok(vec!["claude-a".into(), "claude-b".into()])));
    let items = open_model_combo(&mut app, "translate");
    let labels: Vec<&str> = items.iter().map(|(l, _)| l.as_str()).collect();
    assert_eq!(labels, vec!["Provider default", "claude-a", "claude-b", "Custom…"]);
    let (_, r) = items.iter().find(|(l, _)| l == "claude-b").unwrap();
    click_at(&mut app, r.center());
    assert_eq!(app.settings.translate_model, "claude-b");
    assert_eq!(app.settings.anthropic_model, "", "the chat configuration is not edited");
}

#[test]
fn switching_the_translation_provider_resets_the_model() {
    use ferrocast_core::models::AiProvider as P;
    let mut app = translate_tab_app();
    app.settings.translate_provider = Some(P::ChatGpt);
    app.settings.translate_model = "gpt-6-sol".into();
    let cells = reach_bar(&mut app, 5).unwrap();
    click_at(&mut app, cells[4].center()); // Claude Code
    assert_eq!(app.settings.translate_provider, Some(P::ClaudeCode));
    assert_eq!(app.settings.translate_model, "", "a stale model name is never sent to another provider");
    // clicking the already selected provider keeps a chosen model
    app.settings.translate_model = "opus".into();
    let cells = reach_bar(&mut app, 5).unwrap();
    click_at(&mut app, cells[4].center());
    assert_eq!(app.settings.translate_model, "opus");
}

// ---- SD1: Settings row and control system

fn settings_app(tab: usize, theme: egui::ThemePreference) -> App {
    let app = translate_tab_app();
    app.ctx.set_theme(theme);
    app.ctx.data_mut(|d| d.insert_temp(Id::new("settings_tab"), tab));
    app
}

/// Render three frames: the rows learn their control width in the first one.
fn settle(app: &mut App) {
    for _ in 0..2 {
        render(app, vec![]);
    }
    // the markers of the last frame only
    app.ctx.data_mut(|d| {
        d.remove::<Vec<FormCard>>(Id::new("form-cards"));
        d.remove::<Vec<(String, Rect)>>(Id::new("field-count"));
        d.remove::<Vec<(String, bool)>>(Id::new("row-stacked"));
    });
    render(app, vec![]);
}

fn settings_cards(app: &App) -> Vec<FormCard> {
    app.ctx.data(|d| d.get_temp(Id::new("form-cards"))).unwrap_or_default()
}

fn drawn_fields(app: &App) -> Vec<(String, Rect)> {
    app.ctx.data(|d| d.get_temp(Id::new("field-count"))).unwrap_or_default()
}

const BOTH_THEMES: [egui::ThemePreference; 2] = [egui::ThemePreference::Dark, egui::ThemePreference::Light];

#[test]
fn every_settings_tab_renders_in_both_themes() {
    for tab in 0..SETTINGS_TABS.len() {
        for theme in BOTH_THEMES {
            let mut app = settings_app(tab, theme);
            let before = app.settings.clone();
            settle(&mut app);
            assert!(matches!(app.view(), View::Settings), "tab {tab} {theme:?}");
            assert_eq!(app.settings, before, "drawing alone changes nothing (tab {tab})");
            assert!(!settings_cards(&app).is_empty() || tab == 9 || tab == 0, "tab {tab} drew no card");
        }
    }
}

#[test]
fn the_last_row_of_a_settings_card_has_no_separator_under_it() {
    for tab in 0..SETTINGS_TABS.len() {
        let mut app = settings_app(tab, egui::ThemePreference::Dark);
        settle(&mut app);
        let cards = settings_cards(&app);
        assert!(!cards.is_empty(), "tab {tab} records its cards");
        for c in cards {
            assert!(!c.rows.is_empty(), "tab {tab}: a card without rows");
            assert_eq!(c.seps.len(), c.rows.len() - 1, "tab {tab}: one line between rows, none after the last: {:?}", c.seps);
            let last = &c.rows.last().unwrap().1;
            assert!(c.seps.iter().all(|y| *y < last.min.y + 0.5), "tab {tab}: a line at or under the last row");
            // (the Translate result card ends with a note under its row)
            assert!(tab == 4 || c.rect.max.y - last.max.y < 6.0, "tab {tab}: the card ends right under its last row");
        }
    }
}

#[test]
fn every_settings_text_field_and_combo_goes_through_the_field_helper() {
    use ferrocast_core::models::AiProvider as P;
    // (tab, chat provider, translate provider) -> expected (text fields, combos, drag fields)
    let cases: [(usize, P, Option<P>, usize, usize, usize); 13] = [
        (0, P::OpenAiCompatible, None, 1, 0, 0),
        (1, P::OpenAiCompatible, None, 0, 1, 0),
        (2, P::OpenAiCompatible, None, 0, 0, 0),
        (3, P::OpenAiCompatible, None, 2, 1, 0),
        (3, P::Anthropic, None, 1, 1, 0),
        (3, P::ChatGpt, None, 0, 1, 0),
        (3, P::ClaudeCode, None, 0, 1, 0),
        (4, P::OpenAiCompatible, Some(P::ChatGpt), 1, 1, 0),
        (5, P::OpenAiCompatible, None, 1, 2, 0),
        (6, P::OpenAiCompatible, None, 0, 1, 0),
        (7, P::OpenAiCompatible, None, 1, 0, 0),
        (8, P::OpenAiCompatible, None, 2, 1, 0),
        (9, P::OpenAiCompatible, None, 0, 0, 2),
    ];
    for (tab, chat, tr, texts, combos, drags) in cases {
        for theme in BOTH_THEMES {
            let mut app = settings_app(tab, theme);
            app.settings.ai_provider = chat;
            app.settings.translate_provider = tr;
            settle(&mut app);
            let f = drawn_fields(&app);
            let n = |k: &str| f.iter().filter(|(kind, _)| kind == k).count();
            assert_eq!((n("text"), n("combo"), n("drag")), (texts, combos, drags), "tab {tab} {chat:?} {tr:?}");
            for (_, r) in &f {
                assert!(r.height() >= 28.0 - 0.5, "tab {tab}: a field is at least 28 pt tall: {r:?}");
            }
        }
    }
    // no raw TextEdit / ComboBox / DragValue in the Settings drawing code
    let src = include_str!("draw.rs");
    let start = src.find("    fn draw_settings(").expect("draw_settings");
    let end = src.find("// ---- field helpers").expect("field helpers marker");
    let body = &src[start..end];
    for raw in ["TextEdit::", "ComboBox::", "DragValue::"] {
        assert!(!body.contains(raw), "Settings code draws a raw {raw}: use the field helpers");
    }
}

#[test]
fn settings_rows_are_dense_enough_for_five_rows_on_the_first_screen() {
    let mut app = settings_app(0, egui::ThemePreference::Dark);
    settle(&mut app);
    let rows: Vec<Rect> = settings_cards(&app).iter().flat_map(|c| c.rows.iter().map(|r| r.1)).collect();
    let bottom = ui::HEIGHT - ui::FOOTER_H;
    let visible = rows.iter().filter(|r| r.max.y <= bottom).count();
    assert!(visible >= 5, "{visible} rows fully visible above y={bottom}: {rows:?}");
    // a one-line description row is about 48 pt
    let h: Vec<f32> = rows.iter().map(|r| r.height()).collect();
    assert!(h.iter().any(|h| *h < 56.0), "rows are trimmed: {h:?}");
}

#[test]
fn rows_with_wide_controls_stack_and_toggle_rows_do_not() {
    let stacked = |tab: usize| -> Vec<(String, bool)> {
        let mut app = settings_app(tab, egui::ThemePreference::Dark);
        settle(&mut app);
        let flags: Vec<(String, bool)> = app.ctx.data(|d| d.get_temp(Id::new("row-stacked"))).unwrap_or_default();
        let cards = settings_cards(&app);
        for (title, row, ctl) in cards.iter().flat_map(|c| c.rows.iter()) {
            let is_stacked = flags.iter().find(|(t, _)| t == title).map(|f| f.1).unwrap_or(false);
            if is_stacked {
                assert!(ctl.min.y > row.min.y + 34.0, "{title}: control sits below the text ({ctl:?} in {row:?})");
            } else {
                assert!(ctl.center().y < row.max.y && ctl.center().y > row.min.y, "{title}: control beside the text");
            }
        }
        flags
    };
    let t4 = stacked(4);
    let get = |v: &Vec<(String, bool)>, t: &str| v.iter().find(|(n, _)| n == t).map(|f| f.1);
    assert_eq!(get(&t4, "After translating"), Some(true));
    assert_eq!(get(&t4, "Languages"), Some(true));
    let t0 = stacked(0);
    assert_eq!(get(&t0, "Compact mode"), Some(false));
    assert_eq!(get(&t0, "Launch at login"), Some(false));
}

// ---- SD2: Settings sidebar, header and footer

const TAB_NAMES: [&str; 10] =
    ["General", "Clipboard", "Snippets", "AI", "Translate", "Capture", "Notes", "Scripts", "Integrations", "Advanced"];
const NUM_KEYS: [Key; 10] = [Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9, Key::Num0];

fn cmd_key(key: Key) -> Event {
    Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: CMD }
}

fn tab_of(app: &App) -> Option<usize> {
    app.ctx.data(|d| d.get_temp(Id::new("settings_tab")))
}

/// A Settings view on `tab`, rendered until the sidebar markers are those of the last frame.
fn sd2_app(tab: usize) -> App {
    let mut app = settings_app(tab, egui::ThemePreference::Dark);
    render(&mut app, vec![]);
    render(&mut app, vec![]);
    app
}

type SidebarMarks = (Vec<(String, Rect)>, Vec<Rect>);

fn sidebar_marks(app: &App) -> SidebarMarks {
    app.ctx.data(|d| d.get_temp(Id::new("settings-sidebar"))).unwrap_or_default()
}

#[test]
fn sd2_cmd_number_jumps_to_that_tab_in_settings() {
    for n in 1..=SETTINGS_TABS.len() {
        let mut app = sd2_app((n + 2) % SETTINGS_TABS.len());
        render(&mut app, vec![cmd_key(NUM_KEYS[n - 1])]);
        assert_eq!(tab_of(&app), Some(n - 1), "Cmd+{n}");
        render(&mut app, vec![]);
        assert!(matches!(app.view(), View::Settings));
    }
}

#[test]
fn sd2_cmd_number_does_nothing_outside_settings() {
    for view in [View::Root, View::Clipboard, View::Snippets, View::Notes] {
        let mut app = fixture();
        app.stack = vec![View::Root];
        if !matches!(view, View::Root) {
            app.stack.push(view);
        }
        let depth = app.stack.len();
        let query = app.query.clone();
        for key in NUM_KEYS {
            render(&mut app, vec![cmd_key(key)]);
        }
        assert_eq!(app.stack.len(), depth);
        assert_eq!(app.query, query);
        assert_eq!(tab_of(&app), None, "no Settings state is touched");
    }
}

fn press_over(app: &mut App, pos: Pos2, key: Key) {
    render(app, vec![Event::PointerMoved(pos)]);
    render(app, vec![Event::PointerMoved(pos), key_event(key)]);
}

#[test]
fn sd2_arrows_move_tabs_only_over_the_sidebar() {
    let mut app = sd2_app(2);
    let (items, _) = sidebar_marks(&app);
    let over = items[2].1.center();
    press_over(&mut app, over, Key::ArrowDown);
    assert_eq!(tab_of(&app), Some(3), "Down over the sidebar");
    press_over(&mut app, over, Key::ArrowUp);
    press_over(&mut app, over, Key::ArrowUp);
    assert_eq!(tab_of(&app), Some(1), "Up over the sidebar");
    // over the content pane the arrows belong to the pane
    let pane = Pos2::new(500.0, 250.0);
    press_over(&mut app, pane, Key::ArrowDown);
    press_over(&mut app, pane, Key::ArrowUp);
    assert_eq!(tab_of(&app), Some(1), "arrows over the pane do not switch tabs");
    // the ends do not wrap
    let mut app = sd2_app(0);
    press_over(&mut app, over, Key::ArrowUp);
    assert_eq!(tab_of(&app), Some(0));
}

#[test]
fn sd2_arrows_do_not_steal_from_a_focused_text_field() {
    let mut app = sd2_app(3);
    let fields: Vec<(String, Rect)> = app.ctx.data(|d| d.get_temp(Id::new("field-count"))).unwrap_or_default();
    let field = fields.first().expect("the AI tab has a text field").1;
    click_at(&mut app, field.center());
    assert!(app.ctx.egui_wants_keyboard_input(), "the click focused the field");
    let (items, _) = sidebar_marks(&app);
    press_over(&mut app, items[3].1.center(), Key::ArrowDown);
    assert_eq!(tab_of(&app), Some(3), "a focused text field keeps the arrows");
}

#[test]
fn sd2_sidebar_has_ten_items_in_four_groups_with_three_dividers() {
    let app = sd2_app(0);
    let (items, dividers) = sidebar_marks(&app);
    let names: Vec<&str> = items.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, TAB_NAMES);
    assert_eq!(dividers.len(), 3);
    // dividers sit after Snippets, after Translate and after Capture (its own group), between the neighbouring items
    for (d, after) in dividers.iter().zip([2usize, 4, 5]) {
        assert!(d.center().y > items[after].1.max.y && d.center().y < items[after + 1].1.min.y, "{d:?} after {}", names[after]);
        assert!(d.height() <= 1.5, "a thin line");
        assert!(items[after + 1].1.min.y - items[after].1.max.y >= 8.0, "room around the divider");
    }
}

#[test]
fn sd2_sidebar_items_are_large_hit_targets() {
    let app = sd2_app(0);
    let (items, _) = sidebar_marks(&app);
    assert_eq!(items.len(), SETTINGS_TABS.len());
    for (name, r) in &items {
        assert!(r.height() >= 32.0, "{name}: {r:?} is shorter than 32 pt");
        assert!(r.width() >= 38.0 + 7.0 * name.chars().count() as f32, "{name}: {r:?} is too narrow for its label");
    }
}

#[test]
fn sd2_the_current_tab_is_always_in_view_even_when_the_tabs_do_not_fit_the_window() {
    // nine tabs are taller than the window: the sidebar scrolls to the current tab instead of
    // drawing the last ones over the footer
    for tab in [0, SETTINGS_TABS.len() - 1] {
        let mut app = sd2_app(tab);
        for _ in 0..40 {
            render(&mut app, vec![]); // the scroll to the current tab is animated
        }
        let (items, _) = sidebar_marks(&app);
        let (name, r) = &items[tab];
        assert!(r.min.y >= 0.0 && r.max.y <= ui::HEIGHT - 40.0, "{name} (tab {tab}) at {r:?} is outside the area above the footer");
    }
}

#[test]
fn sd2_breadcrumb_and_footer_chips() {
    for (i, name) in TAB_NAMES.iter().enumerate() {
        let mut app = sd2_app(i);
        let crumb: String = app.ctx.data(|d| d.get_temp(Id::new("settings-breadcrumb"))).unwrap_or_default();
        assert_eq!(crumb, format!("Settings / {name}"));
        render(&mut app, vec![cmd_key(NUM_KEYS[(i + 1) % SETTINGS_TABS.len()])]);
        render(&mut app, vec![]);
        let crumb: String = app.ctx.data(|d| d.get_temp(Id::new("settings-breadcrumb"))).unwrap_or_default();
        assert_eq!(crumb, format!("Settings / {}", TAB_NAMES[(i + 1) % SETTINGS_TABS.len()]), "the header follows a shortcut");
    }
    let app = sd2_app(0);
    let chips: Vec<(String, String)> = app.ctx.data(|d| d.get_temp(Id::new("footer-chips"))).unwrap_or_default();
    assert_eq!(SETTINGS_CHIPS.map(|(a, b)| (a.to_string(), b.to_string())).to_vec(), chips);
}

#[test]
fn sd2_every_tab_renders_in_both_themes() {
    for theme in [egui::ThemePreference::Dark, egui::ThemePreference::Light] {
        for tab in 0..SETTINGS_TABS.len() {
            let mut app = settings_app(tab, theme);
            render(&mut app, vec![]);
            render(&mut app, vec![]);
            assert_eq!(sidebar_marks(&app).0.len(), SETTINGS_TABS.len(), "tab {tab}");
        }
    }
}

// ---- SD3: Settings search

fn text_event(s: &str) -> Event {
    Event::Text(s.to_string())
}

fn search_id() -> Id {
    Id::new("settings-search")
}

fn search_has_focus(app: &App) -> bool {
    app.ctx.memory(|m| m.has_focus(search_id()))
}

fn settings_query(app: &App) -> String {
    app.ctx.data(|d| d.get_temp(Id::new("settings_query"))).unwrap_or_default()
}

/// Titles of the result rows drawn by the last frame.
fn result_titles(app: &App) -> Vec<String> {
    app.ctx.data(|d| d.get_temp(Id::new("settings-results"))).unwrap_or_default()
}

/// Titles of the tab rows drawn by the last frame.
fn drawn_rows(app: &App) -> Vec<(usize, String)> {
    app.ctx.data(|d| d.get_temp(Id::new("settings-row-titles"))).unwrap_or_default()
}

fn flash_of(app: &App) -> Option<String> {
    app.ctx.data(|d| d.get_temp(Id::new("settings-flash")))
}

fn empty_state(app: &App) -> Option<String> {
    app.ctx.data(|d| d.get_temp(Id::new("settings-empty-state")))
}

/// A Settings view on `tab` with `q` typed into the search field (focused through Cmd+F).
fn searching(tab: usize, q: &str) -> App {
    let mut app = sd2_app(tab);
    render(&mut app, vec![cmd_key(Key::F)]);
    render(&mut app, vec![]);
    render(&mut app, vec![text_event(q)]);
    render(&mut app, vec![]);
    app
}

#[test]
fn sd3_typing_a_query_shows_results_and_hides_the_tab() {
    let app = sd2_app(0);
    assert!(!drawn_rows(&app).is_empty(), "without a query the tab rows are drawn");
    assert!(result_titles(&app).is_empty());
    let app = searching(0, "hot");
    assert_eq!(settings_query(&app), "hot");
    let titles = result_titles(&app);
    assert_eq!(titles.first().map(String::as_str), Some("Launcher hotkey"), "{titles:?}");
    assert!(titles.len() <= 8);
    assert!(drawn_rows(&app).is_empty(), "the tab content is hidden while results show");
    assert!(matches!(app.view(), View::Settings));
}

#[test]
fn sd3_enter_opens_the_first_result_clears_the_query_and_flashes_the_row() {
    let mut app = searching(0, "model");
    assert_eq!(result_titles(&app).first().map(String::as_str), Some("Model"));
    render(&mut app, vec![key_event(Key::Enter)]);
    render(&mut app, vec![]);
    assert_eq!(tab_of(&app), Some(3), "the AI tab");
    assert_eq!(settings_query(&app), "");
    assert_eq!(flash_of(&app).as_deref(), Some("Model"));
    assert!(drawn_rows(&app).iter().any(|(t, n)| *t == 3 && n == "Model"), "the tab shows again");
    assert!(matches!(app.view(), View::Settings));
}

#[test]
fn sd3_arrows_move_the_selection_and_a_click_opens_a_result() {
    let mut app = searching(0, "wheel");
    let n = result_titles(&app).len();
    assert!(n >= 3, "wheel matches several rows");
    let sel = |app: &App| app.ctx.data(|d| d.get_temp::<usize>(Id::new("settings-selected"))).unwrap_or(99);
    assert_eq!(sel(&app), 0);
    render(&mut app, vec![key_event(Key::ArrowDown)]);
    assert_eq!(sel(&app), 1);
    render(&mut app, vec![key_event(Key::ArrowUp)]);
    render(&mut app, vec![key_event(Key::ArrowUp)]);
    assert_eq!(sel(&app), 0, "no wrap at the top");
    for _ in 0..12 {
        render(&mut app, vec![key_event(Key::ArrowDown)]);
    }
    assert_eq!(sel(&app), n - 1, "no wrap at the bottom");
    // click the second result
    let rects: Vec<(String, Rect)> = app.ctx.data(|d| d.get_temp(Id::new("settings-result-rects"))).unwrap_or_default();
    assert_eq!(rects.len(), n);
    let (title, r) = rects[1].clone();
    let want = ferrocast_core::settings_index::search("wheel")[1];
    assert_eq!(title, want.title);
    click_at(&mut app, r.center());
    assert_eq!(tab_of(&app), Some(want.tab));
    assert_eq!(settings_query(&app), "");
    assert_eq!(flash_of(&app).as_deref(), Some(want.title));
}

#[test]
fn sd3_clicking_a_tab_clears_the_query() {
    let mut app = searching(0, "hot");
    let (items, _) = sidebar_marks(&app);
    click_at(&mut app, items[5].1.center());
    assert_eq!(tab_of(&app), Some(5));
    assert_eq!(settings_query(&app), "");
    assert!(result_titles(&app).is_empty());
    assert!(!drawn_rows(&app).is_empty());
}

#[test]
fn sd3_escape_clears_the_query_first_and_then_leaves_settings() {
    let mut app = searching(0, "hot");
    render(&mut app, vec![key_event(Key::Escape)]);
    render(&mut app, vec![]);
    assert_eq!(settings_query(&app), "");
    assert!(matches!(app.view(), View::Settings), "the first Escape only clears");
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(!matches!(app.view(), View::Settings), "the second Escape leaves Settings");
}

#[test]
fn sd3_cmd_f_focuses_the_search_field_only_inside_settings() {
    let mut app = sd2_app(2);
    assert!(!search_has_focus(&app));
    render(&mut app, vec![cmd_key(Key::F)]);
    render(&mut app, vec![]);
    assert!(search_has_focus(&app));
    for view in [View::Root, View::Clipboard, View::Snippets] {
        let mut app = fixture();
        app.stack = vec![View::Root];
        if !matches!(view, View::Root) {
            app.stack.push(view);
        }
        let query = app.query.clone();
        render(&mut app, vec![cmd_key(Key::F)]);
        render(&mut app, vec![]);
        assert!(!search_has_focus(&app));
        assert_eq!(app.query, query);
        assert_eq!(settings_query(&app), "");
    }
}

#[test]
fn sd3_an_unknown_query_shows_the_empty_state() {
    let app = searching(0, "zzzqq");
    assert!(result_titles(&app).is_empty());
    assert_eq!(empty_state(&app).as_deref(), Some("No setting matches \u{201c}zzzqq\u{201d}"));
    assert!(drawn_rows(&app).is_empty());
    // a query that matches shows no empty state
    let app = searching(0, "hot");
    assert_eq!(empty_state(&app), None);
}

#[test]
fn sd3_every_drawn_row_is_indexed_and_every_entry_is_drawn() {
    use ferrocast_core::models::AiProvider as P;
    use ferrocast_core::settings_index::ENTRIES;
    let all = [P::OpenAiCompatible, P::ChatGpt, P::Anthropic, P::ClaudeCode];
    let mut drawn: Vec<(usize, String)> = vec![];
    let mut collect = |app: &mut App| {
        seed_model_lists(app);
        render(app, vec![]);
        render(app, vec![]);
        drawn.extend(drawn_rows(app));
    };
    for tab in [0usize, 1, 2, 5, 6, 7, 8, 9] {
        collect(&mut settings_app(tab, egui::ThemePreference::Dark));
    }
    for p in all {
        let mut app = settings_app(3, egui::ThemePreference::Dark);
        app.settings.ai_provider = p;
        collect(&mut app);
    }
    for tp in [None, Some(P::OpenAiCompatible), Some(P::ChatGpt), Some(P::Anthropic), Some(P::ClaudeCode)] {
        let mut app = settings_app(4, egui::ThemePreference::Dark);
        app.settings.translate_provider = tp;
        collect(&mut app);
    }
    // the Apple engine adds a row, only while it is chosen and can run
    APPLE_AVAILABLE.with(|a| a.set(true));
    let mut app = settings_app(4, egui::ThemePreference::Dark);
    app.settings.translate_apple = true;
    for l in ferrocast_core::translate::DEFAULT_LANGUAGES {
        app.apple_langs.insert(l.to_string(), ferrocast_core::translate::AppleLang::Installed);
    }
    // the download row only exists while some language needs a download
    app.apple_langs.insert("Chinese".into(), ferrocast_core::translate::AppleLang::NeedsDownload);
    collect(&mut app);
    APPLE_AVAILABLE.with(|a| a.set(false));
    assert!(drawn.len() > 20, "rows were recorded: {drawn:?}");
    for (tab, title) in &drawn {
        assert!(ENTRIES.iter().any(|e| e.tab == *tab && e.title == title), "tab {tab} draws {title:?} but the index has no entry");
    }
    for e in ENTRIES {
        assert!(drawn.iter().any(|(t, n)| *t == e.tab && n == e.title), "index entry {:?} (tab {}) is never drawn", e.title, e.tab);
    }
}

#[test]
fn sd3_all_tabs_render_in_both_themes_with_and_without_a_query() {
    for theme in [egui::ThemePreference::Dark, egui::ThemePreference::Light] {
        for tab in 0..SETTINGS_TABS.len() {
            let mut app = settings_app(tab, theme);
            render(&mut app, vec![]);
            render(&mut app, vec![]);
            assert_eq!(sidebar_marks(&app).0.len(), SETTINGS_TABS.len(), "tab {tab}");
            render(&mut app, vec![cmd_key(Key::F)]);
            render(&mut app, vec![text_event("model")]);
            render(&mut app, vec![]);
            assert!(!result_titles(&app).is_empty(), "tab {tab} {theme:?}");
            render(&mut app, vec![key_event(Key::Escape)]);
            render(&mut app, vec![]);
            assert!(!drawn_rows(&app).is_empty(), "tab {tab} {theme:?} back on the tab");
        }
    }
}

// ---- SD4: per-tab polish

/// Footer chips of Settings: Tabs, Find (SD4) and Back, in reading order.
const SETTINGS_CHIPS: [(&str, &str); 3] = [("Tabs", "⌘1–0"), ("Find", "⌘F"), ("Back", "esc")];

fn sd4_render(app: &mut App) {
    for _ in 0..3 {
        render(app, vec![]);
    }
}

fn sd4_app(tab: usize, theme: egui::ThemePreference) -> App {
    let mut app = settings_app(tab, theme);
    sd4_render(&mut app);
    app
}

fn sd4_chips(app: &App) -> Vec<(String, String)> {
    app.ctx.data(|d| d.get_temp(Id::new("status-chips"))).unwrap_or_default()
}

fn sd4_chip(kind: &str, text: &str) -> (String, String) {
    (kind.to_string(), text.to_string())
}

fn sd4_disabled(app: &App) -> Vec<String> {
    app.ctx.data(|d| d.get_temp(Id::new("settings-row-disabled"))).unwrap_or_default()
}

fn sd4_active(app: &App) -> Vec<String> {
    app.ctx.data(|d| d.get_temp(Id::new("settings-sidebar-active"))).unwrap_or_default()
}

fn sd4_rect(app: &App, key: &str) -> Option<Rect> {
    app.ctx.data(|d| d.get_temp(Id::new(key)))
}

// (a) search follow-ups

#[test]
fn sd4_no_tab_is_active_while_searching_and_the_breadcrumb_says_search() {
    let app = sd2_app(2);
    assert_eq!(sd4_active(&app), vec!["Snippets".to_string()]);
    let mut app = searching(2, "hot");
    assert!(sd4_active(&app).is_empty(), "no tab is highlighted while results show: {:?}", sd4_active(&app));
    let crumb: String = app.ctx.data(|d| d.get_temp(Id::new("settings-breadcrumb"))).unwrap_or_default();
    assert_eq!(crumb, "Settings / Search");
    render(&mut app, vec![key_event(Key::Escape)]);
    render(&mut app, vec![]);
    render(&mut app, vec![]);
    assert_eq!(sd4_active(&app), vec!["Snippets".to_string()], "the tab is active again once the query is cleared");
    let crumb: String = app.ctx.data(|d| d.get_temp(Id::new("settings-breadcrumb"))).unwrap_or_default();
    assert_eq!(crumb, "Settings / Snippets");
}

#[test]
fn sd4_a_result_accents_only_a_proper_substring_of_its_title() {
    let accents = |app: &App| -> Vec<(String, Option<String>)> {
        app.ctx.data(|d| d.get_temp(Id::new("settings-result-accent"))).unwrap_or_default()
    };
    let app = searching(0, "hot");
    assert!(accents(&app).contains(&("Launcher hotkey".to_string(), Some("hot".to_string()))), "{:?}", accents(&app));
    // the whole title typed: normal colour, nothing accented
    let app = searching(0, "appearance");
    assert!(accents(&app).contains(&("Appearance".to_string(), None)), "{:?}", accents(&app));
    let app = searching(0, "Model");
    assert!(accents(&app).contains(&("Model".to_string(), None)), "{:?}", accents(&app));
    // a longer query accents its first word, a proper part of the title
    let app = searching(0, "compact mode");
    assert!(accents(&app).contains(&("Compact mode".to_string(), Some("Compact".to_string()))), "{:?}", accents(&app));
}

// (b) dependent rows

const WHEEL_ROWS: [&str; 3] = ["Convert wheel key", "Tools wheel key", "Actions wheel key"];

#[test]
fn sd4_wheel_rows_are_disabled_while_drop_wheel_is_off() {
    let mut app = settings_app(0, egui::ThemePreference::Dark);
    app.settings.drop_wheel = false;
    sd4_render(&mut app);
    let off = sd4_disabled(&app);
    for t in WHEEL_ROWS {
        assert!(off.iter().any(|d| d == t), "{t} is disabled: {off:?}");
    }
    assert_eq!(off.len(), 3, "only the dependents: {off:?}");
    // the rows are still drawn (dimmed, not hidden)
    let drawn = drawn_rows(&app);
    for t in WHEEL_ROWS {
        assert!(drawn.iter().any(|(_, n)| n == t), "{t} is drawn");
    }
    app.settings.drop_wheel = true;
    sd4_render(&mut app);
    assert!(sd4_disabled(&app).is_empty(), "{:?}", sd4_disabled(&app));
}

#[test]
fn sd4_the_translate_key_row_follows_drop_wheel() {
    let mut app = settings_app(TRANSLATE_TAB, egui::ThemePreference::Dark);
    app.settings.drop_wheel = false;
    sd4_render(&mut app);
    assert_eq!(sd4_disabled(&app), vec!["Translate wheel key".to_string()]);
    app.settings.drop_wheel = true;
    sd4_render(&mut app);
    assert!(sd4_disabled(&app).is_empty());
}

#[test]
fn sd4_install_updates_follows_the_automatic_check() {
    let mut app = settings_app(0, egui::ThemePreference::Dark);
    app.settings.update_check = false;
    sd4_render(&mut app);
    assert!(sd4_disabled(&app).iter().any(|d| d == "Install updates"), "{:?}", sd4_disabled(&app));
    assert!(!sd4_disabled(&app).iter().any(|d| d == "Version" || d == "Launcher hotkey"), "unrelated rows stay enabled");
    app.settings.update_check = true;
    sd4_render(&mut app);
    assert!(!sd4_disabled(&app).iter().any(|d| d == "Install updates"));
}

#[test]
fn sd4_history_retention_follows_record_history() {
    let mut app = settings_app(1, egui::ThemePreference::Dark);
    app.settings.clipboard_enabled = false;
    sd4_render(&mut app);
    assert_eq!(sd4_disabled(&app), vec!["Keep history for".to_string()]);
    app.settings.clipboard_enabled = true;
    sd4_render(&mut app);
    assert!(sd4_disabled(&app).is_empty());
}

/// The centre of cell `i` of the segmented control inside the row `title`, as of the last frame.
fn sd4_cell_now(app: &App, title: &str, i: usize) -> Option<Pos2> {
    let row = settings_cards(app).iter().flat_map(|c| c.rows.iter().filter(|r| r.0 == title).map(|r| r.1)).next()?;
    let bars: Vec<(Rect, Vec<Rect>)> = app.ctx.data(|d| d.get_temp(Id::new("segmented-count"))).unwrap_or_default();
    bars.iter().find(|(c, _)| row.contains_rect(*c)).map(|(_, cells)| cells[i].center())
}

/// Scroll the pane like a user until that cell is on screen; return where it is.
fn sd4_reach_cell(app: &mut App, title: &str, i: usize) -> Pos2 {
    scroll_until(app, |app| {
        app.ctx.data_mut(|d| d.remove::<Vec<(Rect, Vec<Rect>)>>(Id::new("segmented-count")));
        render(app, vec![]);
        sd4_cell_now(app, title, i)
    })
    .unwrap_or_else(|| panic!("{title} could not be scrolled into view"));
    for _ in 0..30 {
        render(app, vec![]);
    }
    app.ctx.data_mut(|d| d.remove::<Vec<(Rect, Vec<Rect>)>>(Id::new("segmented-count")));
    render(app, vec![]);
    sd4_cell_now(app, title, i).expect("cell")
}

#[test]
fn sd4_a_disabled_row_ignores_clicks_and_an_enabled_one_does_not() {
    let mut app = settings_app(0, egui::ThemePreference::Dark);
    app.settings.drop_wheel = false;
    sd4_render(&mut app);
    let p = sd4_reach_cell(&mut app, "Convert wheel key", 2);
    click_at(&mut app, p);
    assert_eq!(app.settings.wheel_convert_key, "Shift", "disabled: the click does nothing");
    app.settings.drop_wheel = true;
    sd4_render(&mut app);
    click_at(&mut app, p);
    assert_eq!(app.settings.wheel_convert_key, "Option", "enabled: the click works");
}

// Apple engine in Settings → Translate

fn apple_tab(available: bool) -> App {
    APPLE_AVAILABLE.with(|a| a.set(available));
    let mut app = settings_app(TRANSLATE_TAB, egui::ThemePreference::Dark);
    // answers the helper would give, so no test starts it
    for l in ferrocast_core::translate::DEFAULT_LANGUAGES {
        app.apple_langs.insert(l.to_string(), ferrocast_core::translate::AppleLang::Installed);
    }
    app.apple_langs.insert("Chinese".into(), ferrocast_core::translate::AppleLang::NeedsDownload);
    app.apple_langs.insert("Japanese".into(), ferrocast_core::translate::AppleLang::Unsupported);
    app
}

#[test]
fn the_apple_segment_exists_only_where_apple_translation_can_run() {
    let provider_bar = |bars: &[usize]| bars.iter().copied().filter(|n| *n >= 5).collect::<Vec<_>>();
    APPLE_AVAILABLE.with(|a| a.set(false));
    assert_eq!(provider_bar(&tab_bars(TRANSLATE_TAB)), vec![5], "Same as chat plus four providers");
    APPLE_AVAILABLE.with(|a| a.set(true));
    assert_eq!(provider_bar(&tab_bars(TRANSLATE_TAB)), vec![6], "Apple joins them");
}

#[test]
fn choosing_apple_keeps_the_provider_and_choosing_another_engine_replaces_it() {
    use ferrocast_core::models::AiProvider as P;
    let mut app = apple_tab(true);
    app.settings.translate_provider = Some(P::Anthropic);
    app.settings.translate_model = "claude-x".into();
    sd4_render(&mut app);
    // cells: Same as chat, Apple, OpenAI API, ChatGPT, Anthropic, Claude Code
    let apple = sd4_reach_cell(&mut app, "Provider", 1);
    click_at(&mut app, apple);
    assert!(app.settings.translate_apple);
    assert_eq!(app.settings.translate_provider, Some(P::Anthropic), "the provider is remembered while Apple translates");
    let chatgpt = sd4_reach_cell(&mut app, "Provider", 3);
    click_at(&mut app, chatgpt);
    assert!(!app.settings.translate_apple);
    assert_eq!(app.settings.translate_provider, Some(P::ChatGpt));
    assert_eq!(app.settings.translate_model, "", "a model name of one engine is never sent to another");
    let apple = sd4_reach_cell(&mut app, "Provider", 1);
    click_at(&mut app, apple);
    let same = sd4_reach_cell(&mut app, "Provider", 0);
    click_at(&mut app, same);
    assert!(!app.settings.translate_apple);
    assert_eq!(app.settings.translate_provider, None);
}

#[test]
fn the_languages_card_shows_one_chip_per_wheel_language_and_the_model_row_goes_away() {
    let mut app = apple_tab(true);
    app.settings.translate_apple = true;
    app.settings.translate_provider = Some(ferrocast_core::models::AiProvider::Anthropic);
    sd4_render(&mut app);
    let chips = sd4_chips(&app);
    assert!(chips.contains(&sd4_chip("ok", "On this Mac")), "the engine chip: {chips:?}");
    let languages: Vec<_> = chips
        .iter()
        .filter(|(_, t)| ["Installed", "Needs download", "Not supported by Apple", "Checking…"].contains(&t.as_str()))
        .cloned()
        .collect();
    assert_eq!(languages.len(), 8, "one per default wheel language: {chips:?}");
    assert_eq!(languages[0], sd4_chip("ok", "Installed"));
    assert!(languages.contains(&sd4_chip("warn", "Needs download")), "Chinese");
    assert!(languages.contains(&sd4_chip("error", "Not supported by Apple")), "Japanese");
    let rows = app.ctx.data(|d| d.get_temp::<Vec<(String, bool)>>(Id::new("row-stacked"))).unwrap_or_default();
    assert!(!rows.iter().any(|(t, _)| t == "Model"), "no model for the on-device engine");
}

#[test]
fn the_download_row_only_exists_while_a_language_needs_a_download() {
    use ferrocast_core::translate::AppleLang;
    let drawn = |app: &App| drawn_rows(app).iter().any(|(_, t)| t == "Download languages");
    // Apple reports every language ready (its own model is on the Mac): no download row, an explaining note instead
    let mut app = apple_tab(true);
    app.settings.translate_apple = true;
    for l in ferrocast_core::translate::DEFAULT_LANGUAGES {
        app.apple_langs.insert(l.to_string(), AppleLang::Installed);
    }
    settle(&mut app);
    assert!(!drawn(&app), "nothing to download, so no download row");
    // Apple says one language still needs a pack: the row (with its System Settings button) is back
    let mut app = apple_tab(true);
    app.settings.translate_apple = true;
    app.apple_langs.insert("German".into(), AppleLang::NeedsDownload);
    settle(&mut app);
    assert!(drawn(&app), "a language needs a download, so the row is drawn");
}

#[test]
fn every_card_of_the_translate_tab_has_the_same_width_with_the_apple_engine_on() {
    for chosen in [false, true] {
        let mut app = apple_tab(true);
        app.settings.translate_apple = chosen;
        settle(&mut app);
        let cards: Vec<FormCard> = app.ctx.data(|d| d.get_temp(Id::new("form-cards"))).unwrap_or_default();
        assert!(cards.len() >= 3, "the tab draws its cards ({chosen})");
        let (l, r) = (cards[0].rect.min.x, cards[0].rect.max.x);
        for c in &cards {
            let rows: Vec<_> = c.rows.iter().map(|r| r.0.as_str()).collect();
            assert!(
                (c.rect.min.x - l).abs() < 0.5 && (c.rect.max.x - r).abs() < 0.5,
                "card {rows:?} is {:?}, the others {l}..{r} (apple {chosen})",
                c.rect
            );
        }
    }
}

#[test]
fn a_stored_apple_choice_on_a_mac_that_cannot_run_it_shows_the_chat_provider_note() {
    let mut app = apple_tab(false);
    app.settings.translate_apple = true;
    sd4_render(&mut app);
    let chips = sd4_chips(&app);
    assert!(!chips.contains(&sd4_chip("ok", "On this Mac")));
    assert!(!chips.iter().any(|(_, t)| t == "Installed"), "no languages card without Apple");
}

// (c) status chips

#[test]
fn sd4_ai_readiness_texts() {
    use ferrocast_core::models::AiProvider as P;
    use StatusKind as K;
    assert_eq!(ai_readiness(P::OpenAiCompatible, true, false, false), (K::Ok, "Key stored"));
    assert_eq!(ai_readiness(P::OpenAiCompatible, false, false, false), (K::Neutral, "No key set"));
    assert_eq!(ai_readiness(P::Anthropic, true, false, false), (K::Ok, "Key stored"));
    assert_eq!(ai_readiness(P::Anthropic, false, true, true), (K::Warn, "Not configured"));
    assert_eq!(ai_readiness(P::ChatGpt, false, true, false), (K::Ok, "Signed in"));
    assert_eq!(ai_readiness(P::ChatGpt, true, false, true), (K::Warn, "Not signed in"));
    assert_eq!(ai_readiness(P::ClaudeCode, false, false, true), (K::Ok, "Claude Code found"));
    assert_eq!(ai_readiness(P::ClaudeCode, true, true, false), (K::Error, "Claude Code not found"));
    assert_eq!(StatusKind::Ok.name(), "ok");
}

fn sd4_ai_chips(provider: ferrocast_core::models::AiProvider, has_key: bool, signed: bool) -> Vec<(String, String)> {
    let mut app = settings_app(3, egui::ThemePreference::Dark);
    app.settings.ai_provider = provider;
    app.ctx.data_mut(|d| {
        d.insert_temp(Id::new("ai_has_key"), has_key);
        d.insert_temp(Id::new("chatgpt_signed"), signed);
    });
    sd4_render(&mut app);
    sd4_chips(&app)
}

#[test]
fn sd4_the_ai_tab_shows_one_readiness_chip_per_provider() {
    use ferrocast_core::models::AiProvider as P;
    assert_eq!(sd4_ai_chips(P::OpenAiCompatible, true, false), vec![sd4_chip("ok", "Key stored")]);
    assert_eq!(sd4_ai_chips(P::OpenAiCompatible, false, false), vec![sd4_chip("neutral", "No key set")]);
    assert_eq!(sd4_ai_chips(P::Anthropic, true, false), vec![sd4_chip("ok", "Key stored")]);
    assert_eq!(sd4_ai_chips(P::Anthropic, false, false), vec![sd4_chip("warn", "Not configured")]);
    assert_eq!(sd4_ai_chips(P::ChatGpt, false, true), vec![sd4_chip("ok", "Signed in")]);
    assert_eq!(sd4_ai_chips(P::ChatGpt, false, false), vec![sd4_chip("warn", "Not signed in")]);
    let claude = if services::claude_code_path().is_some() {
        sd4_chip("ok", "Claude Code found")
    } else {
        sd4_chip("error", "Claude Code not found")
    };
    assert_eq!(sd4_ai_chips(P::ClaudeCode, false, false), vec![claude]);
}

#[test]
fn sd4_the_translate_engine_shows_readiness_as_a_chip() {
    use ferrocast_core::models::AiProvider as P;
    let chips = |tp: Option<P>, seed: &dyn Fn(&mut App)| {
        let mut app = settings_app(TRANSLATE_TAB, egui::ThemePreference::Dark);
        app.settings.translate_provider = tp;
        seed(&mut app);
        sd4_render(&mut app);
        sd4_chips(&app).into_iter().filter(|(_, t)| !t.contains("language") && t != "Default languages").collect::<Vec<_>>()
    };
    let signed = |a: &mut App| {
        a.ctx.data_mut(|d| {
            d.insert_temp(Id::new("chatgpt_signed"), true);
        })
    };
    assert_eq!(chips(Some(P::ChatGpt), &signed), vec![sd4_chip("ok", "Signed in")]);
    let claude = if services::claude_code_path().is_some() {
        sd4_chip("ok", "Claude Code found")
    } else {
        sd4_chip("error", "Claude Code not found")
    };
    assert_eq!(chips(Some(P::ClaudeCode), &|_| {}), vec![claude]);
    // "Same as chat" reports the chat provider
    let key = |a: &mut App| {
        a.ctx.data_mut(|d| {
            d.insert_temp(Id::new("ai_has_key"), true);
        })
    };
    assert_eq!(chips(None, &key), vec![sd4_chip("ok", "Key stored")]);
    // another provider than the chat's: its own cached key flag
    let other = |a: &mut App| {
        a.ctx.data_mut(|d| {
            d.insert_temp(Id::new(("ai_has_key_for", "Anthropic")), false);
        })
    };
    assert_eq!(chips(Some(P::Anthropic), &other), vec![sd4_chip("warn", "Not configured")]);
}

#[test]
fn sd4_accessibility_shows_granted_or_not_granted() {
    let app = sd4_app(9, egui::ThemePreference::Dark);
    let want = if windows::accessibility_trusted(false) { sd4_chip("ok", "Granted") } else { sd4_chip("warn", "Not granted") };
    assert_eq!(sd4_chips(&app), vec![want]);
}

// (d) validation

#[test]
fn sd4_server_url_problems() {
    for ok in ["http://localhost:11434/v1", "https://api.openai.com/v1", "  https://x.y/v1/  ", "HTTP://LOCALHOST:1/v1"] {
        assert_eq!(url_problem(ok), None, "{ok}");
    }
    for bad in ["", "   ", "localhost:11434", "ftp://x.y", "http://", "https://", "api.openai.com/v1", "http:/x"] {
        assert_eq!(url_problem(bad), Some("Should start with http:// or https://"), "{bad:?}");
    }
}

#[test]
fn sd4_the_server_chip_shows_only_for_a_bad_address() {
    let chips = |url: &str| {
        let mut app = settings_app(3, egui::ThemePreference::Dark);
        app.settings.ai_base_url = url.into();
        app.ctx.data_mut(|d| d.insert_temp(Id::new("ai_has_key"), true));
        sd4_render(&mut app);
        sd4_chips(&app)
    };
    assert!(chips("localhost:11434").contains(&sd4_chip("warn", "Should start with http:// or https://")));
    assert!(!chips("http://localhost:11434/v1").iter().any(|(k, _)| k == "warn"));
}

#[test]
fn sd4_the_languages_chip_follows_what_is_typed() {
    let chips = |typed: Option<String>| {
        let mut app = settings_app(TRANSLATE_TAB, egui::ThemePreference::Dark);
        if let Some(t) = typed {
            app.ctx.data_mut(|d| d.insert_temp(Id::new("translate_langs_buf"), t));
        }
        sd4_render(&mut app);
        sd4_chips(&app)
    };
    let many = |n: usize| (0..n).map(|i| format!("L{i}")).collect::<Vec<_>>().join(", ");
    assert!(chips(None).contains(&sd4_chip("neutral", "8 languages")));
    assert!(chips(Some(String::new())).contains(&sd4_chip("neutral", "Default languages")));
    assert!(chips(Some("A, B, a".into())).contains(&sd4_chip("warn", "Duplicates removed")));
    assert!(chips(Some(many(12))).contains(&sd4_chip("neutral", "12 languages (maximum)")));
    assert!(chips(Some(many(14))).contains(&sd4_chip("warn", "Using the first 12")));
}

// (e) reset and undo

fn sd4_json(s: &Settings) -> serde_json::Map<String, serde_json::Value> {
    match serde_json::to_value(s).unwrap() {
        serde_json::Value::Object(m) => m,
        _ => unreachable!(),
    }
}

/// Every setting of the four resettable tabs changed, and some of the others too.
#[allow(clippy::field_reassign_with_default)]
fn sd4_scrambled() -> Settings {
    use ferrocast_core::models::{Retention, Theme, TranslateOutput, UpdateInstall};
    let mut s = Settings::default();
    s.launcher_hotkey = "Ctrl+Shift+K".into();
    s.theme = Theme::Dark;
    s.compact_mode = true;
    s.launch_at_login = true;
    s.drop_wheel = false;
    s.wheel_convert_key = "Option".into();
    s.wheel_tools_key = "Shift".into();
    s.wheel_actions_key = "Command".into();
    s.enter_on_finder = true;
    s.show_menu_bar_icon = false;
    s.update_check = false;
    s.update_install = UpdateInstall::Automatic;
    s.sensitivity = ferrocast_core::fuzzy::Sensitivity::High;
    s.file_search_in_root = false;
    s.fallback_search_url = "https://example.invalid/?q={query}".into();
    s.clipboard_enabled = false;
    s.clipboard_retention = Retention::Week;
    s.wheel_translate_key = "Control".into();
    s.translate_languages = vec!["Latin".into()];
    s.translate_output = TranslateOutput::Copy;
    s.translate_effect = false;
    s.translate_provider = Some(ferrocast_core::models::AiProvider::Anthropic);
    s.translate_model = "some-model".into();
    s.translate_apple = true;
    s.rem_px = 20.0;
    s.window_gap = 5.0;
    // not on any resettable tab
    s.ai_model = "other-model".into();
    s.script_directories = vec!["~/scripts".into()];
    s.snippet_expansion = false;
    s.favorites = vec!["app:x".into()];
    s.clipboard_ignored_apps.clear();
    s.onboarded = true;
    s.update_last_check = Some(5);
    s.command_mut("calc").disabled = true;
    s
}

/// `sd4_scrambled` without the two settings whose change touches the system (login item, Finder tap).
fn sd4_scrambled_quiet() -> Settings {
    Settings { launch_at_login: false, enter_on_finder: false, ..sd4_scrambled() }
}

#[test]
fn sd4_the_reset_table_names_exact_stored_fields() {
    let d = sd4_json(&Settings::default());
    let sc = sd4_json(&sd4_scrambled());
    let mut seen = std::collections::HashSet::new();
    assert_eq!(RESET_FIELDS.iter().map(|(t, _)| *t).collect::<Vec<_>>(), vec![0, 1, 4, 9], "General, Clipboard, Translate, Advanced");
    for (tab, fields) in RESET_FIELDS {
        for f in fields {
            assert!(d.contains_key(*f), "tab {tab}: {f} is not a stored setting");
            assert_ne!(d[*f], sc[*f], "tab {tab}: the scrambled settings must change {f}");
            assert!(seen.insert(*f), "{f} is listed twice");
        }
    }
    // user data is never reset
    for keep in ["commands", "favorites", "script_directories", "snippet_expansion", "ai_model", "onboarded", "update_last_check"] {
        assert!(!seen.contains(keep), "{keep} must not be in the reset table");
    }
}

#[test]
fn sd4_reset_tab_restores_exactly_that_tabs_fields() {
    let d = sd4_json(&Settings::default());
    let sc = sd4_json(&sd4_scrambled());
    for (tab, fields) in RESET_FIELDS {
        let out = sd4_json(&reset_tab(&sd4_scrambled(), tab));
        for (k, v) in &out {
            if fields.contains(&k.as_str()) {
                assert_eq!(v, &d[k], "tab {tab}: {k} is back to its default");
            } else {
                assert_eq!(v, &sc[k], "tab {tab}: {k} is untouched");
            }
        }
        assert_eq!(reset_tab(&Settings::default(), tab), Settings::default(), "tab {tab}: resetting defaults changes nothing");
    }
    for tab in [2usize, 3, 5, 6, 7, INTEGRATIONS_TAB] {
        assert_eq!(reset_tab(&sd4_scrambled(), tab), sd4_scrambled(), "tab {tab} holds user data: no reset");
    }
}

#[test]
fn sd4_reset_and_undo_on_the_app() {
    let mut app = fixture();
    app.settings = sd4_scrambled_quiet();
    let before = app.settings.clone();
    assert!(app.reset_settings_tab(4));
    assert_eq!(app.settings.translate_languages, Settings::default().translate_languages);
    assert_eq!(app.settings.translate_provider, None);
    assert_eq!(app.settings.translate_output, Settings::default().translate_output);
    assert_eq!(app.settings.compact_mode, before.compact_mode, "other tabs stay");
    assert_eq!(app.settings.rem_px, before.rem_px);
    assert_eq!(app.settings.translate_provider, None);
    assert_eq!(toast_text(&app), "Tab reset · Undo");
    assert!(app.settings_undo.is_some());
    // a change elsewhere after the reset survives the Undo
    app.settings.window_gap = 9.0;
    assert!(app.undo_settings_reset());
    assert_eq!(app.settings.translate_languages, before.translate_languages);
    assert_eq!(app.settings.translate_provider, before.translate_provider);
    assert_eq!(app.settings.translate_model, before.translate_model);
    assert_eq!(app.settings.window_gap, 9.0, "the later change stays");
    assert!(app.settings_undo.is_none());
    assert!(!app.undo_settings_reset(), "nothing left to undo");
    // the file on disk follows
    let saved: Settings = app.store.load("settings.json");
    assert_eq!(saved.translate_languages, before.translate_languages);
}

#[test]
fn sd4_resetting_a_default_tab_changes_nothing() {
    let mut app = fixture();
    app.settings = Settings { compact_mode: true, ..Settings::default() };
    let before = app.settings.clone();
    assert!(!app.reset_settings_tab(9), "Advanced already has its defaults");
    assert_eq!(app.settings, before);
    assert!(app.settings_undo.is_none(), "no undo for a no-op");
    assert_ne!(toast_text(&app), "Tab reset · Undo");
    // a tab with no reset at all
    assert!(!app.reset_settings_tab(7));
    assert_eq!(app.settings, before);
}

#[test]
fn sd4_the_reset_button_exists_on_four_tabs_and_undo_is_clickable() {
    for tab in 0..SETTINGS_TABS.len() {
        let app = sd4_app(tab, egui::ThemePreference::Dark);
        assert_eq!(sd4_rect(&app, "settings-reset-button").is_some(), [0, 1, 4, 9].contains(&tab), "tab {tab}");
    }
    let mut app = settings_app(1, egui::ThemePreference::Dark);
    app.settings.clipboard_enabled = false;
    app.settings.clipboard_retention = ferrocast_core::models::Retention::Week;
    sd4_render(&mut app);
    let r = sd4_rect(&app, "settings-reset-button").expect("reset button");
    assert!(r.max.y < ui::HEIGHT - ui::FOOTER_H, "reachable without scrolling on the short tab: {r:?}");
    click_at(&mut app, r.center());
    assert!(app.settings.clipboard_enabled);
    assert_eq!(app.settings.clipboard_retention, ferrocast_core::models::Retention::Month);
    assert_eq!(toast_text(&app), "Tab reset · Undo");
    sd4_render(&mut app);
    let u = sd4_rect(&app, "settings-undo-rect").expect("the footer offers Undo");
    click_at(&mut app, u.center());
    assert!(!app.settings.clipboard_enabled);
    assert_eq!(app.settings.clipboard_retention, ferrocast_core::models::Retention::Week);
    assert!(app.settings_undo.is_none());
}

#[test]
fn sd4_the_reset_toast_stays_for_eight_seconds() {
    let mut app = fixture();
    app.settings = sd4_scrambled_quiet();
    app.reset_settings_tab(1);
    app.toast = app.toast.take().map(|(t, _)| (t, Instant::now() - Duration::from_secs(5)));
    app.expire_toasts();
    assert_eq!(toast_text(&app), "Tab reset · Undo", "still offered after 5 s");
    app.toast = app.toast.take().map(|(t, _)| (t, Instant::now() - Duration::from_secs(9)));
    app.expire_toasts();
    assert_eq!(toast_text(&app), "", "gone after 9 s");
    assert!(app.settings_undo.is_none(), "and the undo state with it");
}

// (f) empty states

fn sd4_empty(app: &App) -> Vec<(usize, String)> {
    app.ctx.data(|d| d.get_temp(Id::new("settings-pane-empty"))).unwrap_or_default()
}

#[test]
fn sd4_snippets_and_scripts_tabs_compose_an_empty_state() {
    let app = sd4_app(2, egui::ThemePreference::Dark);
    assert_eq!(sd4_empty(&app), vec![(2, "No snippets yet".to_string())]);
    let app = sd4_app(7, egui::ThemePreference::Dark);
    assert_eq!(sd4_empty(&app), vec![(7, "No script commands yet".to_string())]);
    for tab in [0usize, 1, 3, 4, 5, 6, 8, 9] {
        assert!(sd4_empty(&sd4_app(tab, egui::ThemePreference::Dark)).is_empty(), "tab {tab}");
    }
    // data hides the empty state
    let mut app = settings_app(2, egui::ThemePreference::Dark);
    app.snippets.push(ferrocast_core::models::Snippet::new("Sig", "Best", "!sig"));
    sd4_render(&mut app);
    assert!(sd4_empty(&app).is_empty());
    let mut app = settings_app(7, egui::ThemePreference::Dark);
    app.settings.script_directories.push("~/scripts".into());
    sd4_render(&mut app);
    assert!(sd4_empty(&app).is_empty());
}

#[test]
fn sd4_empty_state_buttons_use_the_existing_actions() {
    let mut app = sd4_app(2, egui::ThemePreference::Dark);
    let r = sd4_rect(&app, "settings-empty-button").expect("a button");
    assert!(r.max.y < ui::HEIGHT - ui::FOOTER_H, "{r:?}");
    click_at(&mut app, r.center());
    assert!(matches!(app.view(), View::SnippetForm { id: None, .. }), "New snippet opens the snippet form");
    let mut app = sd4_app(7, egui::ThemePreference::Dark);
    let r = sd4_rect(&app, "settings-empty-button").expect("a button");
    assert!(r.max.y < ui::HEIGHT - ui::FOOTER_H, "{r:?}");
    click_at(&mut app, r.center());
    assert!(matches!(app.view(), View::CreateScript { .. }), "Create script opens the script form");
}

// (g) everything renders

#[test]
fn sd4_every_tab_renders_in_both_themes_in_every_state() {
    use ferrocast_core::models::AiProvider as P;
    for theme in BOTH_THEMES {
        for tab in 0..SETTINGS_TABS.len() {
            for dep_off in [false, true] {
                let mut app = settings_app(tab, theme);
                seed_model_lists(&mut app);
                app.settings.drop_wheel = !dep_off;
                app.settings.update_check = !dep_off;
                app.settings.clipboard_enabled = !dep_off;
                app.settings.translate_provider = if dep_off { Some(P::ClaudeCode) } else { None };
                app.settings.ai_provider = if dep_off { P::ChatGpt } else { P::OpenAiCompatible };
                let before = app.settings.clone();
                sd4_render(&mut app);
                assert!(matches!(app.view(), View::Settings), "tab {tab} {theme:?}");
                assert_eq!(app.settings, before, "drawing alone changes nothing (tab {tab} {theme:?})");
            }
        }
    }
}

// ---- SD5: contrast, hit targets, keyboard focus

/// WCAG relative luminance of an sRGB colour given as 0..=255 channels.
fn wcag_luminance(rgb: [f64; 3]) -> f64 {
    let lin = |c: f64| {
        let c = c / 255.0;
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(rgb[0]) + 0.7152 * lin(rgb[1]) + 0.0722 * lin(rgb[2])
}

/// WCAG contrast ratio of two sRGB colours (0..=255 channels).
fn wcag_contrast(a: [f64; 3], b: [f64; 3]) -> f64 {
    let (la, lb) = (wcag_luminance(a), wcag_luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// `fg` (with extra `alpha`) over the opaque colour `under`, in sRGB space like egui does.
fn over_rgb(fg: Color32, alpha: f32, under: [f64; 3]) -> [f64; 3] {
    let [r, g, b, a] = fg.to_srgba_unmultiplied();
    let a = a as f64 / 255.0 * alpha as f64;
    let c = [r as f64, g as f64, b as f64];
    [0, 1, 2].map(|i| c[i] * a + under[i] * (1.0 - a))
}

#[test]
fn wcag_helpers_match_known_pairs() {
    assert!((wcag_contrast([0.0; 3], [255.0; 3]) - 21.0).abs() < 1e-9);
    assert!((wcag_contrast([255.0; 3], [255.0; 3]) - 1.0).abs() < 1e-9);
    let c = wcag_contrast([119.0; 3], [255.0; 3]);
    assert!((c - 4.48).abs() < 0.01, "#777 on white is about 4.48, got {c}");
    // order does not matter
    assert_eq!(wcag_contrast([10.0; 3], [200.0; 3]), wcag_contrast([200.0; 3], [10.0; 3]));
}

/// Contrast of a recorded role: the text colour (at `alpha`) over the fills in order, the window
/// background first (drawn over black in dark, white in light).
fn role_contrast(fg: Color32, layers: &[Color32], alpha: f32, dark: bool) -> f64 {
    let mut under = if dark { [0.0; 3] } else { [255.0; 3] };
    for l in layers {
        under = over_rgb(*l, 1.0, under);
    }
    wcag_contrast(over_rgb(fg, alpha, under), under)
}

/// Required ratio per role: disabled text and placeholder hints 3:1, icons and the white-on-accent
/// button label 3:1 (large or non-text), everything else 4.5:1.
fn required_ratio(role: &str) -> f64 {
    let relaxed = role.ends_with("-disabled")
        || role.starts_with("hint")
        || role.contains("icon")
        || matches!(role, "search-clear" | "back-arrow" | "accent-button");
    if relaxed {
        3.0
    } else {
        4.5
    }
}

type RoleRec = (String, Color32, Vec<Color32>, f32);

fn sd5_clear(app: &App) {
    app.ctx.data_mut(|d| {
        d.remove::<Vec<RoleRec>>(Id::new("text-roles"));
        d.remove::<Vec<(String, Id, Rect, bool)>>(Id::new("ctl-hits"));
        d.remove::<Vec<Id>>(Id::new("focus-rings"));
    });
}

/// Settle the layout, then draw one clean frame whose records are the only ones kept.
fn sd5_frame(app: &mut App) {
    for _ in 0..3 {
        render(app, vec![]);
    }
    sd5_clear(app);
    render(app, vec![]);
}

fn sd5_roles(app: &App) -> Vec<RoleRec> {
    app.ctx.data(|d| d.get_temp(Id::new("text-roles"))).unwrap_or_default()
}

fn sd5_hits(app: &App) -> Vec<(String, Id, Rect, bool)> {
    app.ctx.data(|d| d.get_temp(Id::new("ctl-hits"))).unwrap_or_default()
}

/// Every Settings state the SD5 checks walk: all tabs in both themes, dependent rows on and off,
/// the search results and its empty state, and the footer toast with its Undo link.
fn sd5_states() -> Vec<(String, bool, App)> {
    use ferrocast_core::models::AiProvider as P;
    let mut out = vec![];
    for theme in BOTH_THEMES {
        let dark = theme == egui::ThemePreference::Dark;
        for tab in 0..SETTINGS_TABS.len() {
            for dep_off in [false, true] {
                let mut app = settings_app(tab, theme);
                seed_model_lists(&mut app);
                app.settings.drop_wheel = !dep_off;
                app.settings.update_check = !dep_off;
                app.settings.clipboard_enabled = !dep_off;
                app.settings.translate_provider = if dep_off { Some(P::ClaudeCode) } else { None };
                app.settings.ai_provider = if dep_off { P::ChatGpt } else { P::OpenAiCompatible };
                sd5_frame(&mut app);
                out.push((format!("tab {tab} dep_off={dep_off} dark={dark}"), dark, app));
            }
        }
        for q in ["hot", "zzzzqq"] {
            let mut app = settings_app(0, theme);
            render(&mut app, vec![cmd_key(Key::F)]);
            render(&mut app, vec![]);
            render(&mut app, vec![text_event(q)]);
            sd5_frame(&mut app);
            out.push((format!("search {q} dark={dark}"), dark, app));
        }
        let mut app = settings_app(1, theme);
        app.settings = sd4_scrambled_quiet();
        app.reset_settings_tab(1);
        sd5_frame(&mut app);
        out.push((format!("reset toast dark={dark}"), dark, app));
    }
    out
}

#[test]
fn sd5_every_settings_text_role_meets_its_contrast() {
    let mut worst: std::collections::BTreeMap<(String, bool), f64> = Default::default();
    let mut failures = vec![];
    for (name, dark, app) in sd5_states() {
        for (role, fg, layers, alpha) in sd5_roles(&app) {
            let ratio = role_contrast(fg, &layers, alpha, dark);
            let w = worst.entry((role.clone(), dark)).or_insert(f64::MAX);
            *w = w.min(ratio);
            if ratio + 1e-9 < required_ratio(&role) {
                failures.push(format!("{name}: {role} {ratio:.2} < {}", required_ratio(&role)));
            }
        }
    }
    // the four status chip kinds, whether or not a tab shows them now
    for dark in [true, false] {
        let pal = ui::palette(dark);
        for kind in [StatusKind::Ok, StatusKind::Warn, StatusKind::Error, StatusKind::Neutral] {
            let c = status_color(&pal, kind);
            let ratio = role_contrast(c, &[pal.field, c.gamma_multiply(0.16)], 1.0, dark);
            let w = worst.entry((format!("chip-{}", kind.name()), dark)).or_insert(f64::MAX);
            *w = w.min(ratio);
            if ratio < 4.5 {
                failures.push(format!("chip {kind:?} dark={dark}: {ratio:.2} < 4.5"));
            }
        }
    }
    for ((role, dark), r) in &worst {
        eprintln!("CONTRAST {} {role:<18} {r:.2} (needs {})", if *dark { "dark " } else { "light" }, required_ratio(role));
    }
    for role in [
        "title",
        "desc",
        "title-disabled",
        "desc-disabled",
        "hint-card",
        "hint-sidebar",
        "side-label",
        "side-icon",
        "side-label-active",
        "side-icon-active",
        "segment",
        "segment-active",
        "button",
        "text-btn",
        "text-btn-hover",
        "footer-label",
        "footer-key",
        "footer-left",
        "footer-toast",
        "crumb-parent",
        "crumb-current",
        "search-icon",
        "result-chip",
        "result-title",
        "section",
        "back-arrow",
    ] {
        assert!(worst.keys().any(|(r, _)| r == role), "role {role} was never recorded");
    }
    failures.sort();
    failures.dedup();
    assert!(failures.is_empty(), "{} contrast failures:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn sd5_the_title_stays_brighter_than_the_description() {
    for dark in [true, false] {
        let pal = ui::palette(dark);
        let card = [pal.field];
        let t = role_contrast(pal.text, &card, 1.0, dark);
        let d = role_contrast(pal.sub_strong, &card, 1.0, dark);
        assert!(t > d + 1.0, "dark={dark}: title {t:.2} vs description {d:.2}");
    }
}

#[test]
fn sd5_every_interactive_settings_rect_is_at_least_28_pt() {
    let mut kinds = std::collections::BTreeSet::new();
    let mut failures = vec![];
    for (name, _, app) in sd5_states() {
        for (kind, _, r, _) in sd5_hits(&app) {
            kinds.insert(kind.clone());
            if r.width() < 28.0 - 0.01 || r.height() < 28.0 - 0.01 {
                failures.push(format!("{name}: {kind} {:.1}x{:.1} at {r:?}", r.width(), r.height()));
            }
        }
    }
    for k in [
        "sidebar",
        "search",
        "search-clear",
        "toggle",
        "segment",
        "small_btn",
        "text_btn",
        "result",
        "field",
        "combo",
        "drag",
        "back",
        "undo",
    ] {
        assert!(kinds.contains(k), "no {k} control was recorded: {kinds:?}");
    }
    failures.sort();
    failures.dedup();
    assert!(failures.is_empty(), "{} small hit targets:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn sd5_every_focusable_settings_control_draws_a_focus_ring() {
    let mut checked = std::collections::BTreeSet::new();
    let mut failures = vec![];
    for tab in 0..SETTINGS_TABS.len() {
        let mut app = settings_app(tab, egui::ThemePreference::Dark);
        seed_model_lists(&mut app);
        sd5_frame(&mut app);
        let hits: Vec<_> = sd5_hits(&app).into_iter().filter(|h| h.3 && h.0 != "undo" && h.0 != "result").collect();
        for (kind, id, _, _) in hits {
            app.ctx.memory_mut(|m| m.request_focus(id));
            app.ctx.data_mut(|d| d.remove::<Vec<Id>>(Id::new("focus-rings")));
            render(&mut app, vec![]);
            let rings: Vec<Id> = app.ctx.data(|d| d.get_temp(Id::new("focus-rings"))).unwrap_or_default();
            checked.insert(kind.clone());
            if !rings.contains(&id) {
                failures.push(format!("tab {tab}: no focus ring on {kind} {id:?}"));
            }
            app.ctx.memory_mut(|m| m.surrender_focus(id));
        }
    }
    // result rows (search), and a real Tab key press reaching a control
    let mut app = settings_app(0, egui::ThemePreference::Dark);
    render(&mut app, vec![cmd_key(Key::F)]);
    render(&mut app, vec![text_event("hot")]);
    sd5_frame(&mut app);
    let rows: Vec<_> = sd5_hits(&app).into_iter().filter(|h| h.0 == "result").collect();
    assert!(!rows.is_empty());
    for (_, id, _, _) in rows {
        app.ctx.memory_mut(|m| m.request_focus(id));
        app.ctx.data_mut(|d| d.remove::<Vec<Id>>(Id::new("focus-rings")));
        render(&mut app, vec![]);
        let rings: Vec<Id> = app.ctx.data(|d| d.get_temp(Id::new("focus-rings"))).unwrap_or_default();
        checked.insert("result".to_string());
        if !rings.contains(&id) {
            failures.push(format!("no focus ring on a result row {id:?}"));
        }
        app.ctx.memory_mut(|m| m.surrender_focus(id));
    }
    let mut app = settings_app(0, egui::ThemePreference::Dark);
    sd5_frame(&mut app);
    app.ctx.memory_mut(|m| m.surrender_focus(Id::new("search")));
    app.ctx.data_mut(|d| d.remove::<Vec<Id>>(Id::new("focus-rings")));
    let tab = Event::Key { key: Key::Tab, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
    render(&mut app, vec![tab]);
    render(&mut app, vec![]);
    let rings: Vec<Id> = app.ctx.data(|d| d.get_temp(Id::new("focus-rings"))).unwrap_or_default();
    assert!(!rings.is_empty(), "pressing Tab focuses a Settings control and draws its ring");
    for k in ["sidebar", "search", "toggle", "segment", "small_btn", "text_btn", "field"] {
        assert!(checked.contains(k), "{k} was not exercised: {checked:?}");
    }
    failures.sort();
    failures.dedup();
    assert!(failures.is_empty(), "{} controls without a ring:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn sd5_reset_link_dividers_and_footer() {
    let app = sd5_state_tab(0);
    // "Reset this tab" sits at the right end of the tab content
    let r = sd4_rect(&app, "settings-reset-button").expect("reset button");
    assert!(r.max.x > ui::WIDTH - 40.0, "right-aligned: {r:?}");
    // group dividers are as wide as the sidebar items
    let (items, dividers) = sidebar_marks(&app);
    assert_eq!(dividers.len(), 3);
    for d in &dividers {
        for (_, it) in &items {
            assert_eq!((d.min.x, d.max.x), (it.min.x, it.max.x), "divider {d:?} vs item {it:?}");
        }
    }
    // the footer's left side names the app
    let roles = sd5_roles(&app);
    assert!(roles.iter().any(|r| r.0 == "footer-left"), "footer shows the app name in Settings");
    // other views keep their own footer text (no Ferrocast name next to the icon)
    let mut other = fixture();
    other.stack = vec![View::Root, View::Snippets];
    sd5_clear(&other);
    render(&mut other, vec![]);
    assert!(!sd5_roles(&other).iter().any(|r| r.0 == "footer-left"));
}

fn sd5_state_tab(tab: usize) -> App {
    let mut app = settings_app(tab, egui::ThemePreference::Dark);
    sd5_frame(&mut app);
    app
}

// ---- translating effect (T3) ------------------------------------------------------------------

use ferrocast_core::models::TranslateOutput as Out;
use ferrocast_core::translate::{web_replace_marked_js, web_unmark_js, Browser, Target as WebTarget, WebResult, FX_ID};
use services::EffectKind;

/// A fixture whose data directory exists, so the log lines can be read back.
fn effect_app() -> App {
    let app = fixture();
    let _ = std::fs::create_dir_all(&app.store.root);
    app
}

fn effect_log(app: &App) -> String {
    std::fs::read_to_string(instance::log_path(&app.store.root)).unwrap_or_default()
}

fn pick_first_language(app: &mut App, effect: bool) {
    app.settings.translate_effect = effect;
    open_translate_wheel(app);
    app.prev_pid = Some(2_000_000_000);
    if let Some(View::Wheel { center, .. }) = app.stack.last_mut() {
        *center = Pos2::new(300.0, 200.0);
    }
    app.apply_wheel(Some(0));
}

fn started(app: &mut App, run: u64, kind: EffectKind) {
    app.bus.send(Msg::EffectStarted { run, kind });
    app.drain();
}

fn translated_run(app: &mut App, run: u64, result: Result<String, String>, target: WebTarget) {
    app.bus.send(Msg::Translated { run, lang: "Spanish".into(), source_pid: Some(7), result, ms: 5, target });
    app.drain();
}

fn overlay_point() -> EffectKind {
    EffectKind::Overlay(ferrocast_macos::glow::Target::Point([1.0, 2.0]))
}

#[test]
fn apple_engine_translates_with_apple_when_it_is_available() {
    APPLE_AVAILABLE.with(|a| a.set(true));
    let mut app = effect_app();
    app.settings.translate_apple = true;
    app.settings.translate_provider = Some(ferrocast_core::models::AiProvider::Anthropic);
    pick_first_language(&mut app, false);
    assert_eq!(fx_taken(), vec!["translate 1".to_string()]);
    assert_eq!(toast_text(&app), "Translating to English with Apple…");
    assert!(!app.apple_fallback_told);
}

#[test]
fn apple_engine_falls_back_to_the_chat_provider_and_says_so_once_per_session() {
    APPLE_AVAILABLE.with(|a| a.set(false));
    let mut app = effect_app();
    app.settings.translate_apple = true;
    pick_first_language(&mut app, false);
    assert_eq!(fx_taken(), vec!["translate 1".to_string()], "the translation still starts");
    assert_eq!(toast_text(&app), "Apple Translation needs macOS 26; using the chat provider.");
    assert!(app.settings.translate_apple, "the stored choice is kept for a Mac that can use it");
    open_translate_wheel(&mut app);
    app.apply_wheel(Some(0));
    assert_eq!(toast_text(&app), "Translating to English with Same as chat (OpenAI API)…", "the notice is not repeated");
}

#[test]
fn a_effect_probe_runs_at_pick_time_only_when_the_setting_is_on() {
    let mut app = effect_app();
    pick_first_language(&mut app, true);
    assert_eq!(fx_probes(), vec![(Some(2_000_000_000), Some([300.0, 200.0]), 1)]);
    assert_eq!(fx_taken(), vec!["translate 1".to_string()]);
    let mut app = effect_app();
    pick_first_language(&mut app, false);
    assert!(fx_probes().is_empty(), "no probe with the effect off");
    assert_eq!(fx_taken(), vec!["translate 1".to_string()], "the translation still starts");
}

#[test]
fn b_overlay_is_shown_then_hidden_once_before_the_window() {
    let mut app = effect_app();
    app.settings.translate_output = Out::Show;
    started(&mut app, 0, overlay_point());
    assert_eq!(fx_taken(), vec!["glow_show".to_string()]);
    assert!(app.translating_effect.is_some());
    translated_run(&mut app, 0, Ok("Hola".into()), WebTarget::Other);
    assert_eq!(fx_taken(), vec!["glow_hide".to_string()]);
    assert_shows_hola(&app);
    assert!(app.translating_effect.is_none());
    assert!(effect_log(&app).contains("translate effect overlay point -> ended: hidden"));
}

#[test]
fn c_web_effect_replaces_through_the_marked_script_without_unmarking() {
    let mut app = effect_app();
    FX_FRONT.with(|f| f.set(Some(7)));
    app.settings.translate_output = Out::Auto;
    started(&mut app, 0, EffectKind::Web(Browser::Safari));
    assert!(fx_taken().is_empty() && fx_scripts().is_empty(), "starting a web effect only records it");
    translated_run(&mut app, 0, Ok("Hola".into()), WebTarget::Browser(Browser::Safari));
    let scripts = fx_scripts();
    assert_eq!(scripts, vec![web_replace_marked_js("Hola")]);
    assert!(scripts[0].contains(FX_ID) && scripts[0].contains("\"Hola\""));
    assert!(!scripts.contains(&web_unmark_js()));
    assert!(app.translating_effect.is_none());
    assert!(effect_log(&app).contains("translate effect web Safari -> ended: replaced"));
}

#[test]
fn d_web_effect_with_a_failed_translation_unmarks_and_toasts() {
    let mut app = effect_app();
    app.settings.translate_output = Out::Auto;
    started(&mut app, 0, EffectKind::Web(Browser::Safari));
    translated_run(&mut app, 0, Err("boom".into()), WebTarget::Other);
    assert_eq!(fx_scripts(), vec![web_unmark_js()]);
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("boom")), "{:?}", app.toast);
    assert!(fx_taken().is_empty(), "nothing is replaced");
    assert!(effect_log(&app).contains("-> ended: unmarked"));
    assert!(!effect_log(&app).contains("Hola"));
}

#[test]
fn e_web_effect_whose_replace_finds_no_selection_shows_the_window_and_unmarks() {
    let mut app = effect_app();
    FX_FRONT.with(|f| f.set(Some(7)));
    app.settings.translate_output = Out::Auto;
    started(&mut app, 0, EffectKind::Web(Browser::Safari));
    translated_run(&mut app, 0, Ok("Hola".into()), WebTarget::Browser(Browser::Safari));
    fx_scripts();
    app.bus.send(Msg::WebReplaced { lang: "Spanish".into(), text: "Hola".into(), result: WebResult::NoSelection });
    app.drain();
    assert_shows_hola(&app);
    assert_eq!(fx_scripts(), vec![web_unmark_js()]);
}

#[test]
fn e2_web_effect_shown_in_the_window_is_unmarked() {
    let mut app = effect_app();
    app.settings.translate_output = Out::Show;
    started(&mut app, 0, EffectKind::Web(Browser::Safari));
    translated_run(&mut app, 0, Ok("Hola".into()), WebTarget::Other);
    assert_shows_hola(&app);
    assert_eq!(fx_scripts(), vec![web_unmark_js()]);
}

#[test]
fn f_a_late_effect_start_is_ended_at_once_and_an_old_run_is_ignored() {
    let mut app = effect_app();
    app.settings.translate_output = Out::Show;
    app.translate_run = 3;
    translated_run(&mut app, 3, Ok("Hola".into()), WebTarget::Other);
    fx_taken();
    started(&mut app, 3, overlay_point());
    assert_eq!(fx_taken(), vec!["glow_hide".to_string()], "never shown, ended at once");
    assert!(app.translating_effect.is_none());
    started(&mut app, 3, EffectKind::Web(Browser::Safari));
    assert_eq!(fx_scripts(), vec![web_unmark_js()]);
    // an old run
    let mut app = effect_app();
    app.translate_run = 5;
    started(&mut app, 4, overlay_point());
    started(&mut app, 4, EffectKind::Web(Browser::Safari));
    assert!(fx_taken().is_empty() && fx_scripts().is_empty());
    assert!(app.translating_effect.is_none());
}

#[test]
fn g_the_effect_ends_after_sixty_seconds_without_a_translation() {
    let mut app = effect_app();
    started(&mut app, 0, overlay_point());
    fx_taken();
    let t0 = app.translate_started;
    app.effect_tick(t0 + Duration::from_secs(59));
    assert!(fx_taken().is_empty() && app.translating_effect.is_some());
    app.effect_tick(t0 + Duration::from_secs(61));
    assert_eq!(fx_taken(), vec!["glow_hide".to_string()]);
    assert!(app.translating_effect.is_none());
    assert!(effect_log(&app).contains("translate effect overlay point -> ended: timeout"));
    app.effect_tick(t0 + Duration::from_secs(120));
    assert!(fx_taken().is_empty(), "ends exactly once");
}

#[test]
fn a_new_translation_ends_a_running_effect_first() {
    let mut app = effect_app();
    started(&mut app, 0, overlay_point());
    fx_taken();
    pick_first_language(&mut app, true);
    assert_eq!(fx_taken(), vec!["glow_hide".to_string(), "translate 1".to_string()]);
}

#[test]
fn h_the_translating_effect_row_renders_and_its_toggle_updates_the_setting() {
    assert!(Settings::default().translate_effect);
    let old: Settings = serde_json::from_str("{}").unwrap();
    assert!(old.translate_effect, "missing in old files means on");
    for theme in [egui::ThemePreference::Dark, egui::ThemePreference::Light] {
        let mut app = translate_tab_app();
        app.ctx.set_theme(theme);
        render(&mut app, vec![]);
        render(&mut app, vec![]);
        assert!(drawn_rows(&app).contains(&(4, "Translating effect".to_string())), "{theme:?}");
    }
    let mut app = translate_tab_app();
    let pos = scroll_until(&mut app, |app| {
        app.ctx.data_mut(|d| d.remove::<Vec<(String, Id, Rect, bool)>>(Id::new("ctl-hits")));
        render(app, vec![]);
        sd5_hits(app).into_iter().find(|h| h.0 == "toggle").map(|h| h.2.center())
    })
    .expect("the toggle is reachable");
    for _ in 0..30 {
        render(&mut app, vec![]);
    }
    app.ctx.data_mut(|d| d.remove::<Vec<(String, Id, Rect, bool)>>(Id::new("ctl-hits")));
    render(&mut app, vec![]);
    let pos = sd5_hits(&app).into_iter().find(|h| h.0 == "toggle").map(|h| h.2.center()).unwrap_or(pos);
    click_at(&mut app, pos);
    assert!(!app.settings.translate_effect);
    click_at(&mut app, pos);
    assert!(app.settings.translate_effect);
}

// ---- Notes: launcher commands, Search Notes and the controller wiring (T6)

fn notes_note(id: &str, text: &str, modified: i64) -> Note {
    Note { id: id.into(), text: text.into(), created: 0, modified, pinned_at: None, deleted_at: None }
}

/// The app with a fake Notes window; the handle shows what the controller asked of it.
fn notes_app(notes: Vec<Note>) -> (App, std::rc::Rc<std::cell::RefCell<crate::notes_ctl::fake::St>>) {
    let mut app = fixture();
    let (fx, st) = crate::notes_ctl::fake::FakeFx::new();
    app.notes_ctl = crate::notes_ctl::NotesCtl::new(Box::new(fx));
    app.notes = notes;
    (app, st)
}

#[test]
fn notes_commands_exist_and_share_the_notes_icon() {
    let app = fixture();
    let find = |id: &str| app.base.iter().find(|i| i.id == id).unwrap_or_else(|| panic!("{id} missing"));
    let notes = find("builtin:notes");
    assert_eq!((notes.title.as_str(), notes.subtitle.as_str()), ("Notes", ""));
    assert!(matches!(notes.kind, Kind::Builtin(Builtin::Notes)));
    for id in ["builtin:search-notes", "builtin:create-note"] {
        assert_eq!(find(id).icon, notes.icon, "{id}");
    }
    assert!(notes.keywords.iter().any(|k| k == "floating notes") && notes.keywords.iter().any(|k| k == "raycast notes"));
}

#[test]
fn floating_notes_alias_finds_the_notes_command() {
    let mut app = fixture();
    for q in ["floating notes", "raycast notes"] {
        app.query = q.into();
        let results = app.root_sections();
        let first = results.iter().flat_map(|s| &s.items).next().unwrap_or_else(|| panic!("no result for {q}"));
        assert_eq!(first.id, "builtin:notes", "{q}");
    }
}

#[test]
fn notes_commands_take_a_hotkey_like_other_builtins_and_have_none_by_default() {
    let mut app = fixture();
    for id in ["builtin:notes", "builtin:create-note", "builtin:search-notes"] {
        let item = app.base.iter().find(|i| i.id == id).unwrap();
        assert!(item.is_command(), "{id} is configurable in Root Search");
        assert_eq!(app.settings.hotkey(id), None, "no default hotkey for {id}");
        assert!(!ferrocast_core::models::DEFAULT_HOTKEYS.iter().any(|(c, _)| *c == id));
        app.settings.command_mut(id).hotkey = Some("Alt+KeyN".into());
        app.settings.command_mut(id).alias = Some("nn".into());
        assert_eq!(app.settings.hotkey(id), Some("Alt+KeyN"));
        assert_eq!(app.settings.alias(id), Some("nn"));
    }
}

#[test]
fn search_notes_lists_pinned_first_with_pin_glyph_and_slot() {
    let (mut app, _) = notes_app(vec![notes_note("a", "Alpha", 1), notes_note("b", "Beta apple", 9), notes_note("c", "Gamma", 5)]);
    ferrocast_core::notes::set_pinned(&mut app.notes, "a", true, 10);
    app.stack = vec![View::Root, View::Notes];
    let s = app.sections();
    let titles: Vec<(&str, Vec<&str>)> = s.iter().map(|x| (x.title.as_str(), x.items.iter().map(|i| i.title.as_str()).collect())).collect();
    assert_eq!(titles, vec![("Pinned", vec!["Alpha"]), ("Notes", vec!["Beta apple", "Gamma"])]);
    let pinned = &s[0].items[0];
    assert_eq!(pinned.icon, Icon::Glyph("sf:pin.fill", [255, 197, 49]));
    assert_eq!(pinned.accessory, "⌘1");
    assert_ne!(s[1].items[0].icon, pinned.icon);
    app.query = "apple".into();
    let s = app.sections();
    assert_eq!(s.iter().flat_map(|x| &x.items).map(|i| i.title.as_str()).collect::<Vec<_>>(), ["Beta apple"], "content is searched");
    app.notes[1].deleted_at = Some(3);
    assert!(app.sections().iter().all(|x| x.items.is_empty()), "deleted notes are not listed");
}

#[test]
fn search_notes_actions_pin_delete_and_create() {
    let (mut app, _) = notes_app(vec![notes_note("a", "Alpha", 1)]);
    let item = Item {
        id: "note:a".into(),
        title: "Alpha".into(),
        subtitle: String::new(),
        icon: Icon::Emoji(String::new()),
        accessory: String::new(),
        keywords: vec![],
        kind: Kind::Note("a".into()),
    };
    let acts = app.actions_for(&item);
    let by = |t: &str| {
        acts.iter().find(|a| a.title == t).unwrap_or_else(|| panic!("{t} in {:?}", acts.iter().map(|a| &a.title).collect::<Vec<_>>()))
    };
    assert!(matches!(by("Open Note").kind, Act::Run));
    assert_eq!(by("Pin Note").shortcut, Some((CMD, Key::Period)));
    assert_eq!(shortcut_label(CMD, Key::Period), "⌘.");
    assert_eq!(by("Create Note").shortcut, Some((CMD, Key::N)));
    assert!(by("Delete Note").destructive);
    ferrocast_core::notes::set_pinned(&mut app.notes, "a", true, 1);
    assert!(app.actions_for(&item).iter().any(|a| a.title == "Unpin Note"));
}

#[test]
fn enter_on_a_note_opens_it_in_the_floating_window_and_hides_the_launcher() {
    let (mut app, st) = notes_app(vec![notes_note("a", "Alpha", 1), notes_note("b", "Beta", 2)]);
    app.stack = vec![View::Root, View::Notes];
    app.visible = true;
    app.run_item(&Item {
        id: "note:b".into(),
        title: "Beta".into(),
        subtitle: String::new(),
        icon: Icon::Emoji(String::new()),
        accessory: String::new(),
        keywords: vec![],
        kind: Kind::Note("b".into()),
    });
    assert!(!app.visible, "the launcher steps aside first");
    assert!(matches!(app.view(), View::Root), "and goes back to the root");
    assert_eq!(app.notes_ctl.current.as_deref(), Some("b"));
    assert!(st.borrow().visible);
    assert_eq!(st.borrow().markdown, "Beta");
    assert_eq!(app.settings.notes_last.as_deref(), Some("b"));
    let _ = std::fs::remove_dir_all(&app.store.root);
}

#[test]
fn create_note_command_opens_the_window_instead_of_a_form() {
    let (mut app, st) = notes_app(vec![]);
    app.run_builtin(Builtin::CreateNote);
    assert_eq!(app.notes.len(), 1);
    assert!(matches!(app.view(), View::Root), "no note form is pushed");
    assert!(st.borrow().visible);
    assert_eq!(app.notes_ctl.current.as_deref(), Some(app.notes[0].id.as_str()));
    let saved: Vec<Note> = app.store.load("notes.json");
    assert_eq!(saved.len(), 1, "persisted");
    let _ = std::fs::remove_dir_all(&app.store.root);
}

#[test]
fn notes_command_toggles_the_window() {
    let (mut app, st) = notes_app(vec![notes_note("a", "Alpha", 1)]);
    app.run_builtin(Builtin::Notes);
    assert!(st.borrow().visible);
    assert_eq!(app.notes_ctl.current.as_deref(), Some("a"));
    app.run_builtin(Builtin::Notes);
    assert!(!st.borrow().visible);
    let _ = std::fs::remove_dir_all(&app.store.root);
}

#[test]
fn create_dictated_note_command_exists_with_its_keywords_and_the_notes_icon() {
    let app = fixture();
    let item = app.base.iter().find(|i| i.id == "builtin:create-dictated-note").expect("command");
    assert_eq!(item.title, "Create Dictated Note");
    assert!(item.keywords.iter().any(|k| k == "dictate") && item.keywords.iter().any(|k| k == "voice note"));
    assert_eq!(item.icon, app.base.iter().find(|i| i.id == "builtin:notes").unwrap().icon);
    assert!(item.is_command(), "it takes a hotkey like the other Notes commands");
}

#[test]
fn create_dictated_note_command_opens_a_note_and_starts_dictation_through_the_fake_window() {
    let (mut app, st) = notes_app(vec![notes_note("a", "Alpha", 1)]);
    app.run_builtin(Builtin::CreateDictatedNote);
    assert_eq!(app.notes.len(), 2);
    assert!(st.borrow().visible && st.borrow().dict_active);
    assert!(matches!(app.view(), View::Root));
    let _ = std::fs::remove_dir_all(&app.store.root);
}

#[test]
fn a_ferrocast_url_message_hides_the_launcher_and_opens_the_note() {
    use crate::notes_ctl::NMsg;
    use ferrocast_macos::url_scheme::Route;
    let (mut app, st) = notes_app(vec![notes_note("a", "Alpha", 1), notes_note("b", "Beta", 2)]);
    app.visible = true;
    app.bus.send(Msg::Notes(NMsg::Url(Route::Note("b".into()))));
    app.drain();
    assert!(!app.visible, "the launcher steps aside first");
    assert_eq!(app.notes_ctl.current.as_deref(), Some("b"));
    assert!(st.borrow().visible);
    app.bus.send(Msg::Notes(NMsg::Url(Route::Note("zzz".into()))));
    app.drain();
    assert_eq!(app.notes.len(), 2, "an unknown id creates nothing");
    assert_eq!(app.notes_ctl.current.as_deref(), Some("b"));
    let _ = std::fs::remove_dir_all(&app.store.root);
}

#[test]
fn note_pin_and_delete_from_search_notes_need_no_confirmation() {
    let (mut app, _) = notes_app(vec![notes_note("a", "Alpha", 1)]);
    app.stack = vec![View::Root, View::Notes];
    app.perform(Act::NotePin("a".into()), false);
    assert!(app.notes[0].pinned_at.is_some());
    app.perform(Act::NoteDelete("a".into()), false);
    assert!(!matches!(app.view(), View::Confirm { .. }), "it is recoverable from Recently Deleted");
    assert!(app.notes[0].deleted_at.is_some() && app.notes[0].pinned_at.is_none());
    let saved: Vec<Note> = app.store.load("notes.json");
    assert!(saved[0].deleted_at.is_some());
    let _ = std::fs::remove_dir_all(&app.store.root);
}

#[test]
fn native_notes_messages_are_applied_by_drain() {
    use crate::notes_ctl::NMsg;
    let (mut app, st) = notes_app(vec![notes_note("a", "Alpha", 1)]);
    app.run_builtin(Builtin::Notes);
    app.bus.send(Msg::Notes(NMsg::Changed("Alpha edited".into())));
    app.bus.send(Msg::Notes(NMsg::Zoom(1.3)));
    app.drain();
    assert_eq!(app.notes[0].text, "Alpha edited");
    assert_eq!(app.settings.notes_zoom, 1.3);
    let saved: Vec<Note> = app.store.load("notes.json");
    assert_eq!(saved[0].text, "Alpha edited", "the first change after a quiet period is written at once");
    assert!(st.borrow().visible);
    let _ = std::fs::remove_dir_all(&app.store.root);
}

// ---- Notes settings tab and the menu-bar item (S14, S15)

const NOTES_TAB_TITLES: [&str; 3] = ["Show Notes Toggle Button in Menu Bar", "Escape Key Loses Focus", "Zoom"];

/// The app on the Notes tab with a fake Notes window; the menu-bar log is cleared.
fn notes_tab_app(theme: egui::ThemePreference) -> (App, std::rc::Rc<std::cell::RefCell<crate::notes_ctl::fake::St>>) {
    let (mut app, st) = notes_app(vec![]);
    seed_model_lists(&mut app);
    app.stack = vec![View::Root, View::Settings];
    app.ctx.set_theme(theme);
    app.ctx.data_mut(|d| d.insert_temp(Id::new("settings_tab"), NOTES_TAB));
    fx_taken();
    for _ in 0..3 {
        render(&mut app, vec![]);
    }
    (app, st)
}

fn notes_tab_hits(app: &mut App, kind: &str) -> Vec<Rect> {
    app.ctx.data_mut(|d| d.remove::<Vec<(String, Id, Rect, bool)>>(Id::new("ctl-hits")));
    render(app, vec![]);
    let mut hits: Vec<Rect> = sd5_hits(app).into_iter().filter(|h| h.0 == kind).map(|h| h.2).collect();
    hits.sort_by(|a, b| a.min.y.total_cmp(&b.min.y));
    hits
}

#[test]
fn notes_tab_sits_after_translate_and_lists_the_three_controls() {
    assert_eq!(SETTINGS_TABS[TRANSLATE_TAB].0, "Translate");
    assert_eq!(SETTINGS_TABS[NOTES_TAB].0, "Notes");
    assert_eq!(NOTES_TAB, CAPTURE_TAB + 1, "Capture is its own group between Translate and Notes");
    for theme in BOTH_THEMES {
        let (app, _) = notes_tab_app(theme);
        let rows: Vec<String> = drawn_rows(&app).into_iter().filter(|(t, _)| *t == NOTES_TAB).map(|(_, n)| n).collect();
        assert_eq!(rows, NOTES_TAB_TITLES.map(String::from).to_vec(), "{theme:?}");
        assert_eq!(notes_tab_hits(&mut { app }, "toggle").len(), 2, "two switches");
    }
}

#[test]
fn notes_settings_are_found_by_the_settings_search() {
    use ferrocast_core::settings_index::search;
    for (q, title) in [
        ("menu bar button", NOTES_TAB_TITLES[0]),
        ("toggle button", NOTES_TAB_TITLES[0]),
        ("floating notes", NOTES_TAB_TITLES[0]),
        ("escape", NOTES_TAB_TITLES[1]),
        ("focus", NOTES_TAB_TITLES[1]),
        ("zoom", NOTES_TAB_TITLES[2]),
        ("text size", NOTES_TAB_TITLES[2]),
    ] {
        assert!(search(q).iter().any(|e| e.tab == NOTES_TAB && e.title == title), "{q:?} finds {title:?}");
    }
    // and the UI jumps to the row
    let (mut app, _) = notes_tab_app(egui::ThemePreference::Dark);
    app.ctx.data_mut(|d| d.insert_temp(Id::new("settings_tab"), 0usize));
    render(&mut app, vec![cmd_key(Key::F)]);
    render(&mut app, vec![text_event("escape key")]);
    render(&mut app, vec![]);
    assert!(result_titles(&app).contains(&"Escape Key Loses Focus".to_string()));
    render(&mut app, vec![key_event(Key::Enter)]);
    render(&mut app, vec![]);
    assert_eq!(tab_of(&app), Some(NOTES_TAB));
    assert_eq!(flash_of(&app).as_deref(), Some("Escape Key Loses Focus"));
}

#[test]
fn the_menu_bar_switch_updates_the_setting_and_the_menu_bar_item() {
    let (mut app, _) = notes_tab_app(egui::ThemePreference::Dark);
    assert!(!app.settings.notes_menu_bar_button && app.notes_tray.is_none());
    let sw = notes_tab_hits(&mut app, "toggle")[0].center();
    click_at(&mut app, sw);
    assert!(app.settings.notes_menu_bar_button);
    assert!(app.notes_tray.is_some());
    assert_eq!(fx_taken(), vec!["notes_tray on".to_string()]);
    let saved: Settings = app.store.load("settings.json");
    assert!(saved.notes_menu_bar_button, "saved with the other settings");
    let sw = notes_tab_hits(&mut app, "toggle")[0].center();
    click_at(&mut app, sw);
    assert!(!app.settings.notes_menu_bar_button && app.notes_tray.is_none());
    assert_eq!(fx_taken(), vec!["notes_tray off".to_string()]);
}

#[test]
fn the_escape_switch_updates_the_setting_and_the_window() {
    let (mut app, st) = notes_tab_app(egui::ThemePreference::Light);
    let sw = notes_tab_hits(&mut app, "toggle")[1].center();
    click_at(&mut app, sw);
    assert!(app.settings.notes_escape_loses_focus);
    assert!(st.borrow().log.contains(&"esc:true".to_string()), "the controller hook ran: {:?}", st.borrow().log);
    let saved: Settings = app.store.load("settings.json");
    assert!(saved.notes_escape_loses_focus);
    let sw = notes_tab_hits(&mut app, "toggle")[1].center();
    click_at(&mut app, sw);
    assert!(!app.settings.notes_escape_loses_focus);
    assert_eq!(st.borrow().log.iter().rev().find(|l| l.starts_with("esc:")).map(String::as_str), Some("esc:false"));
}

#[test]
fn zoom_steps_are_fifty_to_three_hundred_percent_like_the_editor() {
    let steps = notes_zoom_steps();
    assert_eq!(steps.len(), 26);
    assert_eq!((steps[0], steps[25]), (0.5, 3.0));
    assert!(steps.windows(2).all(|w| ((w[1] - w[0]) - 0.1).abs() < 1e-9), "10 % apart");
    for w in steps.windows(2) {
        assert_eq!(ferrocast_macos::notes_editor::step_zoom(w[0], 1), w[1], "same as the editor's step from {}", w[0]);
    }
    assert_eq!(notes_zoom_label(1.0), "100%");
    assert_eq!(notes_zoom_label(0.5), "50%");
    assert_eq!(notes_zoom_label(3.0), "300%");
    assert_eq!(notes_zoom_label(1.5), "150%");
    assert_eq!(notes_zoom_label(1.1 + 0.2), "130%", "float noise is rounded away");
}

fn notes_zoom_shown(app: &App) -> String {
    app.ctx.data(|d| d.get_temp(Id::new("notes-zoom-shown"))).unwrap_or_default()
}

#[test]
fn the_zoom_control_follows_the_window_and_changes_it() {
    let (mut app, st) = notes_tab_app(egui::ThemePreference::Dark);
    assert_eq!(notes_zoom_shown(&app), "100%");
    // ⌘+ inside the Notes window: the controller persists it, the tab shows the live value
    app.bus.send(Msg::Notes(crate::notes_ctl::NMsg::Zoom(1.3)));
    app.drain();
    render(&mut app, vec![]);
    assert_eq!(notes_zoom_shown(&app), "130%");
    // choose 60 % in the drop-down (the list scrolls: later entries start out of view)
    let combo = notes_tab_hits(&mut app, "combo")[0].center();
    click_at(&mut app, combo);
    for _ in 0..30 {
        render(&mut app, vec![]);
    }
    let items: Vec<(String, Rect)> = app.ctx.data(|d| d.get_temp(Id::new("notes-zoom-items"))).unwrap_or_default();
    assert_eq!(items.len(), 26);
    let (_, r) = items.iter().find(|(l, _)| l == "60%").cloned().expect("a 60% entry");
    click_at(&mut app, r.center());
    assert_eq!(app.settings.notes_zoom, 0.6);
    assert_eq!(st.borrow().zoom, 0.6, "the open window follows");
    let saved: Settings = app.store.load("settings.json");
    assert_eq!(saved.notes_zoom, 0.6);
    render(&mut app, vec![]);
    assert_eq!(notes_zoom_shown(&app), "60%");
    // Reset goes back to 100 %
    let reset = notes_tab_hits(&mut app, "small_btn")[0].center();
    click_at(&mut app, reset);
    assert_eq!(app.settings.notes_zoom, 1.0);
    assert_eq!(st.borrow().zoom, 1.0);
}

#[test]
fn the_notes_tab_switches_are_large_hit_targets_in_both_themes() {
    for theme in BOTH_THEMES {
        let (mut app, _) = notes_tab_app(theme);
        let toggles = notes_tab_hits(&mut app, "toggle");
        assert!(toggles.iter().all(|r| r.height() >= 24.0 && r.width() >= 36.0), "{toggles:?}");
    }
}

// ---- onboarding tour

fn tour_app(slide: usize) -> App {
    let mut app = fixture();
    app.settings.onboarded = true;
    app.stack = vec![View::Root, View::Welcome];
    app.tour = tour::Tour::new();
    app.tour.slide = slide;
    app.tour.injected = Some(tour::Perms { accessibility: false, input_monitoring: false });
    app
}

fn on_tour(app: &App) -> bool {
    matches!(app.view(), View::Welcome)
}

#[test]
fn tour_navigation_is_bounded() {
    let mut t = tour::Tour::new();
    assert_eq!(t.slide, 0);
    assert!(!t.prev(), "no slide before the first");
    for want in 1..tour::SLIDES {
        assert!(t.next());
        assert_eq!(t.slide, want);
    }
    assert!(!t.next(), "no slide after the last");
    assert_eq!(t.slide, tour::SLIDES - 1);
    assert!(t.go(2));
    assert_eq!(t.slide, 2);
    assert!(!t.go(2), "going to the current slide is not a move");
    assert!(!t.go(tour::SLIDES), "out of range is ignored");
    assert_eq!(t.slide, 2);
}

fn release(key: Key) -> Event {
    Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::NONE }
}

/// A full key tap: egui ignores a second press of a key it still sees held.
fn tap(app: &mut App, key: Key) {
    render(app, vec![key_event(key)]);
    render(app, vec![release(key)]);
}

#[test]
fn tour_arrow_keys_enter_and_escape() {
    let mut app = tour_app(0);
    tap(&mut app, Key::ArrowRight);
    assert_eq!(app.tour.slide, 1);
    tap(&mut app, Key::ArrowLeft);
    tap(&mut app, Key::ArrowLeft);
    assert_eq!(app.tour.slide, 0, "left stops at the first slide");
    assert!(on_tour(&app));
    tap(&mut app, Key::Enter);
    assert_eq!(app.tour.slide, 1, "Enter goes to the next slide");
    // a held Enter must not run through the deck
    let held = Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: true, modifiers: Modifiers::NONE };
    render(&mut app, vec![key_event(Key::Enter)]);
    assert_eq!(app.tour.slide, 2, "the first press moves");
    render(&mut app, vec![held.clone(), held]);
    assert_eq!(app.tour.slide, 2, "its auto-repeats do not");
    render(&mut app, vec![release(Key::Enter)]);
    // Enter on the permissions slide only moves on (the grant is a button)
    app.tour.slide = tour::PERMS_SLIDE;
    tap(&mut app, Key::Enter);
    assert_eq!(app.tour.slide, tour::PERMS_SLIDE + 1);
    // right at the last slide stays; Enter there finishes
    tap(&mut app, Key::ArrowRight);
    assert_eq!(app.tour.slide, tour::SLIDES - 1);
    assert!(on_tour(&app));
    tap(&mut app, Key::Enter);
    assert!(!on_tour(&app), "Enter on the last slide closes the tour");
}

#[test]
fn tour_escape_closes_from_any_slide_and_stays_onboarded() {
    for slide in 0..tour::SLIDES {
        let mut app = tour_app(slide);
        render(&mut app, vec![key_event(Key::Escape)]);
        assert!(!on_tour(&app), "Esc closes slide {slide}");
        assert!(app.settings.onboarded);
    }
}

#[test]
fn the_menu_bar_item_follows_the_saved_setting_at_startup_and_every_change() {
    let (mut app, _) = notes_app(vec![]);
    fx_taken();
    app.settings.notes_menu_bar_button = true;
    app.sync_notes_tray();
    app.sync_notes_tray();
    assert_eq!(fx_taken(), vec!["notes_tray on".to_string()], "created once, not per call");
    // commit_settings (the Settings screen) turns it off again
    let mut s = app.settings.clone();
    s.notes_menu_bar_button = false;
    app.commit_settings(s);
    assert!(app.notes_tray.is_none());
    assert_eq!(fx_taken(), vec!["notes_tray off".to_string()]);
    // reset/undo paths go through commit_settings too
    let mut s = app.settings.clone();
    s.notes_menu_bar_button = true;
    app.commit_settings(s);
    assert!(app.notes_tray.is_some());
}

#[test]
fn a_menu_bar_click_toggles_the_notes_window() {
    let (mut app, st) = notes_app(vec![notes_note("a", "Alpha", 1)]);
    app.bus.send(Msg::Tray("notes_toggle".into()));
    app.drain();
    assert!(st.borrow().visible, "first click shows");
    app.bus.send(Msg::Tray("notes_toggle".into()));
    app.drain();
    assert!(!st.borrow().visible, "second click hides");
    let _ = std::fs::remove_dir_all(&app.store.root);
}

#[test]
fn the_notes_tab_has_no_reset_and_the_format_bar_has_no_switch() {
    assert!(!RESET_FIELDS.iter().any(|(t, _)| *t == NOTES_TAB));
    let (app, _) = notes_tab_app(egui::ThemePreference::Dark);
    assert!(!drawn_rows(&app).iter().any(|(_, n)| n.to_lowercase().contains("format bar")), "the Format Bar toggle stays in the ⌘K panel");
}

/// Every text painted by one frame (the Keyboard Shortcuts screen is a scroll area, so callers scroll between frames).
fn painted_texts(app: &mut App, events: Vec<Event>) -> Vec<String> {
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(ui::WIDTH, ui::HEIGHT))),
        events,
        ..Default::default()
    };
    let ctx = app.ctx.clone();
    let mut output = ctx.run_ui(input, |ui| app.draw(ui));
    output.textures_delta.clear();
    output
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) => Some(t.galley.text().to_string()),
            _ => None,
        })
        .collect()
}

#[test]
fn keyboard_shortcuts_screen_has_a_notes_group_with_the_window_shortcuts() {
    let mut app = fixture();
    app.stack = vec![View::Root, View::Shortcuts];
    let mut seen: Vec<String> = vec![];
    let centre = Pos2::new(ui::WIDTH / 2.0, ui::HEIGHT / 2.0);
    for i in 0..14 {
        let mut events = vec![Event::PointerMoved(centre)];
        if i > 0 {
            events.push(Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, -120.0),
                phase: egui::TouchPhase::Move,
                modifiers: Modifiers::NONE,
            });
        }
        seen.extend(painted_texts(&mut app, events));
    }
    assert!(seen.iter().any(|t| t == "NOTES" || t == "Notes"), "no Notes group heading, saw {seen:?}");
    for what in [
        "New note",
        "Browse Notes",
        "Open the action panel",
        "Pin or unpin the note",
        "Go back to the previous note",
        "Go forward to the next note",
    ] {
        assert!(seen.iter().any(|t| t == what), "missing Notes row {what:?}");
    }
    // the key caps of the six rows
    for keys in ["N", "P", "K", "[", "]"] {
        assert!(seen.iter().any(|t| t == keys), "missing key cap {keys:?}");
    }
}

#[test]
fn first_run_opens_the_tour_once_and_hidden_does_not() {
    let mut s = Settings::default();
    assert!(!s.onboarded);
    assert!(!tour::first_run(&mut s, true), "--hidden never opens it");
    assert!(!s.onboarded, "and does not use up the first run");
    assert!(tour::first_run(&mut s, false), "first visible launch opens it");
    assert!(s.onboarded);
    assert!(!tour::first_run(&mut s, false), "only once");
}

#[test]
fn reopening_the_tour_starts_on_slide_zero() {
    let mut app = tour_app(4);
    app.stack = vec![View::Root];
    app.push(View::Welcome);
    assert_eq!(app.tour.slide, 0);
}

#[test]
fn every_tour_slide_renders_at_the_window_size() {
    for dark in [true, false] {
        for slide in 0..tour::SLIDES {
            let mut app = tour_app(slide);
            app.dark = dark;
            for _ in 0..3 {
                render(&mut app, vec![]);
            }
            // mid-transition too
            app.tour.go((slide + 1) % tour::SLIDES);
            render(&mut app, vec![]);
        }
    }
}

#[test]
fn tour_footer_matches_the_deck() {
    let mut app = tour_app(0);
    assert_eq!(app.view().footer_hints().len(), 0, "the tour state lives in App, not in the view");
    render(&mut app, vec![]);
    let chips: Vec<(String, String)> = app.ctx.data(|d| d.get_temp(Id::new("footer-chips"))).unwrap_or_default();
    let has = |l: &str, k: &str| chips.iter().any(|(a, b)| a == l && b == k);
    assert!(has("Next", "↵") && has("Skip", "esc"), "{chips:?}");
    let mut app = tour_app(tour::SLIDES - 1);
    render(&mut app, vec![]);
    let chips: Vec<(String, String)> = app.ctx.data(|d| d.get_temp(Id::new("footer-chips"))).unwrap_or_default();
    assert!(chips.iter().any(|(l, k)| l == "Start" && k == "↵"), "{chips:?}");
}

fn perm_lines(app: &App) -> Vec<String> {
    app.ctx.data(|d| d.get_temp(Id::new("tour-perm-status"))).unwrap_or_default()
}

#[test]
fn permissions_slide_status_follows_the_checked_value() {
    let mut app = tour_app(tour::PERMS_SLIDE);
    app.tour.injected = Some(tour::Perms { accessibility: true, input_monitoring: false });
    render(&mut app, vec![]);
    let l = perm_lines(&app);
    assert!(l.contains(&"Accessibility: Granted".to_string()), "{l:?}");
    assert!(l.contains(&"Input Monitoring: Not granted".to_string()), "{l:?}");
    app.tour.injected = Some(tour::Perms { accessibility: false, input_monitoring: true });
    render(&mut app, vec![]);
    let l = perm_lines(&app);
    assert!(l.contains(&"Accessibility: Not granted".to_string()), "{l:?}");
    assert!(l.contains(&"Input Monitoring: Granted".to_string()), "{l:?}");
    // Automation cannot be checked silently: it is never reported as granted
    assert!(l.iter().any(|s| s.starts_with("Automation") && !s.contains("Granted")), "{l:?}");
}

#[test]
fn progress_dots_jump_to_a_slide() {
    let mut app = tour_app(0);
    render(&mut app, vec![]);
    let dots: Vec<Rect> = app.ctx.data(|d| d.get_temp(Id::new("tour-dots"))).unwrap_or_default();
    assert_eq!(dots.len(), tour::SLIDES);
    let at = dots[3].center();
    render(&mut app, vec![Event::PointerMoved(at)]);
    let click = |pressed| Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    render(&mut app, vec![click(true)]);
    render(&mut app, vec![click(false)]);
    assert_eq!(app.tour.slide, 3);
}

// ---- first-launch intro (sunrise)

fn intro_app() -> App {
    let mut app = tour_app(0);
    app.begin_intro(false);
    app.intro.as_mut().unwrap().clock = Some(0.0);
    app
}

fn at(app: &mut App, t: f64) {
    app.intro.as_mut().unwrap().clock = Some(t);
    render(app, vec![]);
}

#[test]
fn intro_timeline_is_ordered_and_bounded() {
    let p0 = intro::phase(0.0);
    assert!(p0.rise < 0.01 && p0.logo == 0.0 && p0.word == 0.0 && p0.alpha == 1.0 && !p0.done);
    let mid = intro::phase(1.4);
    assert!(mid.rise > 0.3 && mid.logo > 0.0 && mid.word == 0.0, "the logo comes before the wordmark");
    let late = intro::phase(2.4);
    assert!(late.logo == 1.0 && late.word > 0.0 && late.word < 1.0, "the wordmark arrives after the logo settles");
    let held = intro::phase(intro::HANDOFF_AT - 0.01);
    assert!(held.rise == 1.0 && held.logo == 1.0 && held.word == 1.0 && held.alpha == 1.0);
    let fading = intro::phase(intro::HANDOFF_AT + intro::HANDOFF / 2.0);
    assert!(fading.alpha > 0.0 && fading.alpha < 1.0 && !fading.done);
    assert!(intro::phase(intro::DURATION).done);
    let total = std::hint::black_box(intro::DURATION);
    assert!((3.0..=4.5).contains(&total), "about 3-4 s");
}

#[test]
fn first_launch_plays_the_intro_over_the_tour_then_ends_by_itself() {
    let mut app = intro_app();
    at(&mut app, 0.0);
    assert!(on_tour(&app) && app.intro.is_some());
    assert!(app.intro.as_ref().unwrap().covers(), "the intro covers the tour");
    at(&mut app, intro::DURATION as f64 - 0.1);
    assert!(app.intro.is_some(), "still playing just before the end");
    at(&mut app, intro::DURATION as f64 + 0.05);
    assert!(app.intro.is_none(), "it ends by itself");
    assert!(on_tour(&app) && app.tour.slide == 0, "and leaves the tour on slide 0");
}

#[test]
fn a_key_press_or_click_skips_the_intro_without_driving_the_tour() {
    for ev in [key_event(Key::ArrowRight), key_event(Key::Escape), key_event(Key::Enter), key_event(Key::A)] {
        let mut app = intro_app();
        at(&mut app, 0.5);
        app.intro.as_mut().unwrap().clock = Some(0.6);
        render(&mut app, vec![ev]);
        let i = app.intro.as_ref().expect("hand-off still fading");
        assert!(!i.covers(), "the key skips to the hand-off");
        assert_eq!(app.tour.slide, 0, "and is not also a tour key");
        assert!(on_tour(&app), "Esc does not close the tour either");
        at(&mut app, 0.6 + intro::HANDOFF as f64 + 0.05);
        assert!(app.intro.is_none());
    }
    let mut app = intro_app();
    at(&mut app, 0.5);
    click_at(&mut app, Pos2::new(300.0, 200.0));
    assert!(app.intro.as_ref().is_none_or(|i| !i.covers()), "a click skips too");
}

#[test]
fn reduce_motion_shows_the_settled_frame_briefly() {
    let mut app = tour_app(0);
    app.begin_intro(true);
    let i = app.intro.as_mut().unwrap();
    i.clock = Some(0.0);
    let p = intro::phase(i.elapsed(0.0));
    assert!(p.rise == 1.0 && p.logo == 1.0 && p.word == 1.0 && p.alpha == 1.0, "static final frame, no motion");
    let hold = std::hint::black_box(intro::DURATION - intro::REDUCED_START);
    assert!(hold < 2.0, "and short");
}

#[test]
fn reopening_the_tour_does_not_replay_the_intro() {
    let mut app = tour_app(0);
    app.pop();
    app.push(View::Welcome);
    assert!(app.intro.is_none());
    render(&mut app, vec![]);
    assert!(app.intro.is_none() && on_tour(&app));
    // after a played intro, closing and reopening does not bring it back either
    let mut app = intro_app();
    at(&mut app, 0.0);
    at(&mut app, 20.0);
    app.pop();
    app.push(View::Welcome);
    render(&mut app, vec![]);
    assert!(app.intro.is_none());
}

#[test]
fn intro_renders_every_frame_of_its_timeline_and_then_requests_nothing() {
    let mut app = intro_app();
    for n in 0..=40 {
        if app.intro.is_none() {
            break;
        }
        at(&mut app, n as f64 * 0.1);
    }
    assert!(app.intro.is_none());
    let ctx = app.ctx.clone();
    render(&mut app, vec![]);
    let again = ctx.run_ui(egui::RawInput::default(), |_| {});
    assert!(again.viewport_output.values().all(|v| v.repaint_delay == std::time::Duration::MAX), "no repaints once the intro is over");
}

#[test]
fn intro_sound_plays_once_and_only_on_the_real_animated_intro() {
    let mut live = intro::Intro::new(false);
    assert!(live.take_sound(), "the animated intro plays its sound");
    assert!(!live.take_sound(), "only once");
    assert!(!intro::Intro::new(true).take_sound(), "Reduce Motion starts at the settled frame: no sound out of sync");
    let mut tested = intro::Intro::new(false);
    tested.clock = Some(0.0);
    assert!(!tested.take_sound(), "tests drive the clock and must stay silent");
}

// ---- Settings → Integrations (Notion)

const NOTION_TAB: usize = INTEGRATIONS_TAB;
const ME_JSON: &str = r#"{"object":"user","type":"bot","name":"Ferrocast","bot":{"workspace_name":"Acme"}}"#;
const SEARCH_JSON: &str = r#"{"object":"list","results":[
  {"object":"page","id":"p1","url":"https://app.notion.com/p/p1","properties":{"title":{"type":"title","title":[{"plain_text":"Inbox"}]}}},
  {"object":"data_source","id":"d1","url":"https://app.notion.com/d1","title":[{"plain_text":"Tasks"}]}
],"has_more":false,"next_cursor":null}"#;

/// Settings on the Integrations tab with the fake Keychain and base URL reset for this test thread.
fn notion_app(token: Option<&str>, base: Option<String>) -> App {
    notion_io::set_test_token(token);
    notion_io::set_test_base(base);
    let mut app = settings_app(NOTION_TAB, egui::ThemePreference::Dark);
    app.ctx.data_mut(|d| d.remove::<bool>(Id::new("notion_has_token")));
    settle(&mut app);
    app
}

/// Click the `n`th small button of the pane (Save, Test, [Clear], Search, Setup guide, in drawing order).
fn click_small_btn(app: &mut App, n: usize) {
    app.ctx.data_mut(|d| d.remove::<Vec<(String, Id, Rect, bool)>>(Id::new("ctl-hits")));
    render(app, vec![]);
    let hits: Vec<(String, Id, Rect, bool)> = app.ctx.data(|d| d.get_temp(Id::new("ctl-hits"))).unwrap_or_default();
    let btns: Vec<_> = hits.iter().filter(|h| h.0 == "small_btn").collect();
    click_at(app, btns[n].2.center());
}

/// Drain the bus until `done` holds (worker threads answer asynchronously).
fn drain_until(app: &mut App, done: impl Fn(&App) -> bool) {
    for _ in 0..200 {
        app.drain();
        if done(app) {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("no answer from the worker, toast: {:?}", toast_text(app));
}

#[test]
fn saving_the_notion_token_trims_it_stores_it_in_the_keychain_and_clears_the_field() {
    let mut app = notion_app(None, None);
    app.notion.token = "  ntn_secret  ".into();
    click_small_btn(&mut app, 0);
    assert_eq!(notion_io::test_token().as_deref(), Some("ntn_secret"));
    assert_eq!(toast_text(&app), "Notion token saved to Keychain");
    assert!(app.notion.token.is_empty(), "the typed secret is cleared after saving");
    // the secret is nowhere in the settings file
    app.settings.notion_target = None;
    app.save_settings();
    let file = std::fs::read_to_string(app.store.root.join("settings.json")).unwrap();
    assert!(!file.contains("ntn_secret"), "{file}");
    // an empty value removes it
    click_small_btn(&mut app, 0);
    assert_eq!(notion_io::test_token(), None);
    assert_eq!(toast_text(&app), "Notion token removed");
}

#[test]
fn test_connection_without_a_token_says_to_paste_one() {
    let mut app = notion_app(None, Some("http://127.0.0.1:9/v1".into()));
    click_small_btn(&mut app, 1);
    drain_until(&mut app, |a| toast_text(a) == "Paste your Notion token first.");
}

#[test]
fn test_connection_success_names_the_bot_and_workspace() {
    let (base, h) = notion_io::testing::serve(200, ME_JSON);
    let mut app = notion_app(Some("ntn_tok"), Some(base));
    click_small_btn(&mut app, 1);
    drain_until(&mut app, |a| toast_text(a).starts_with("Connected"));
    assert_eq!(toast_text(&app), "Connected to Notion as Ferrocast · Acme");
    let req = h.join().unwrap();
    assert!(req.starts_with("GET /v1/users/me"), "{req}");
    assert!(req.to_lowercase().contains("authorization: bearer ntn_tok"), "{req}");
}

#[test]
fn test_connection_with_a_rejected_token_toasts_the_exact_message() {
    let (base, h) = notion_io::testing::serve(401, r#"{"object":"error","status":401,"code":"unauthorized","message":"bad"}"#);
    let mut app = notion_app(Some("ntn_bad"), Some(base));
    click_small_btn(&mut app, 1);
    let want = ferrocast_core::notion::NotionError::Unauthorized.user_message();
    drain_until(&mut app, |a| toast_text(a) == want);
    let _ = h.join();
}

#[test]
fn searching_fills_up_to_eight_results_and_a_stale_answer_is_dropped() {
    let (base, h) = notion_io::testing::serve(200, SEARCH_JSON);
    let mut app = notion_app(Some("ntn_tok"), Some(base));
    app.notion.query = "inbox".into();
    app.notion_search();
    assert!(app.notion.searching);
    drain_until(&mut app, |a| !a.notion.searching);
    let titles: Vec<_> = app.notion.results.iter().map(|i| i.title.as_str()).collect();
    assert_eq!(titles, ["Inbox", "Tasks"]);
    let req = h.join().unwrap();
    assert!(req.starts_with("POST /v1/search") && req.contains(r#""query":"inbox""#) && req.contains(r#""page_size":8"#), "{req}");
    // a newer search is in flight: an older answer changes nothing
    app.notion.request += 1;
    app.notion.searching = true;
    let stale = ferrocast_core::notion::parse_search(r#"{"results":[{"object":"page","id":"zz","url":"u","properties":{"title":{"type":"title","title":[{"plain_text":"Stale"}]}}}],"has_more":false}"#).unwrap();
    let old = app.notion.request - 1;
    app.bus.send(Msg::NotionSearched { request: old, result: Ok(stale) });
    app.drain();
    assert_eq!(app.notion.results.len(), 2, "stale results are ignored");
    assert!(app.notion.searching, "still waiting for the newest answer");
}

#[test]
fn a_failed_search_toasts_and_keeps_the_previous_results() {
    let (base, h) = notion_io::testing::serve(403, r#"{"object":"error","status":403,"code":"restricted_resource","message":"x"}"#);
    let mut app = notion_app(Some("ntn_tok"), Some(base));
    app.notion.results = ferrocast_core::notion::parse_search(SEARCH_JSON).unwrap().items;
    app.notion_search();
    drain_until(&mut app, |a| !a.notion.searching);
    assert_eq!(toast_text(&app), ferrocast_core::notion::NotionError::from_status(403, r#"{"code":"restricted_resource"}"#).user_message());
    assert_eq!(app.notion.results.len(), 2);
    let _ = h.join();
}

#[test]
fn choosing_a_result_stores_the_target_and_it_survives_a_reload_and_clear_unsets_it() {
    let mut app = notion_app(Some("ntn_tok"), None);
    app.notion.results = ferrocast_core::notion::parse_search(SEARCH_JSON).unwrap().items;
    // draw the results, then click the second one (Tasks, a database)
    app.ctx.data_mut(|d| d.remove::<Vec<(String, Id, Rect, bool)>>(Id::new("ctl-hits")));
    render(&mut app, vec![]);
    let hits: Vec<(String, Id, Rect, bool)> = app.ctx.data(|d| d.get_temp(Id::new("ctl-hits"))).unwrap_or_default();
    let rows: Vec<_> = hits.iter().filter(|h| h.0 == "notion_result").collect();
    assert_eq!(rows.len(), 2);
    click_at(&mut app, rows[1].2.center());
    assert_eq!(toast_text(&app), "Capture target set to Tasks");
    let saved: Settings = app.store.load("settings.json");
    let t = saved.notion_target.expect("saved to settings.json");
    assert_eq!((t.id.as_str(), t.kind.as_str(), t.title.as_str()), ("d1", "data_source", "Tasks"));
    let file = std::fs::read_to_string(app.store.root.join("settings.json")).unwrap();
    assert!(!file.contains("ntn_tok"), "no token in settings.json");
    // Clear (the second small button after Save/Test is Clear once a target exists)
    click_small_btn(&mut app, 2);
    assert_eq!(toast_text(&app), "Capture target cleared");
    let saved: Settings = app.store.load("settings.json");
    assert_eq!(saved.notion_target, None);
}

#[test]
fn without_a_token_the_target_and_search_rows_are_inert_but_visible() {
    let mut app = notion_app(None, None);
    app.ctx.data_mut(|d| d.remove::<Vec<(String, Id, Rect, bool)>>(Id::new("ctl-hits")));
    render(&mut app, vec![]);
    let rows: Vec<String> = drawn_rows(&app).into_iter().filter(|(t, _)| *t == NOTION_TAB).map(|r| r.1).collect();
    for want in ["Notion token", "Test connection", "Capture target", "Find a page or database", "Setup guide"] {
        assert!(rows.iter().any(|r| r == want), "{want} stays visible: {rows:?}");
    }
    let hits: Vec<(String, Id, Rect, bool)> = app.ctx.data(|d| d.get_temp(Id::new("ctl-hits"))).unwrap_or_default();
    let small: Vec<bool> = hits.iter().filter(|h| h.0 == "small_btn").map(|h| h.3).collect();
    // Save, Test, Search, Setup: only Search is inert
    assert_eq!(small[..4], [true, true, false, true], "{small:?}");
}

#[test]
fn every_index_entry_is_drawn_on_the_tab_it_names() {
    use ferrocast_core::models::AiProvider as P;
    use ferrocast_core::settings_index::ENTRIES;
    for e in ENTRIES {
        let mut found = false;
        'variants: for chat in [P::OpenAiCompatible, P::ChatGpt, P::Anthropic, P::ClaudeCode] {
            // the last variant is the Apple engine (its language card draws a row of its own)
            for (tr, apple) in
                [(None, false), (Some(P::OpenAiCompatible), false), (Some(P::ChatGpt), false), (Some(P::Anthropic), false), (None, true)]
            {
                APPLE_AVAILABLE.with(|a| a.set(apple));
                let mut app = settings_app(e.tab, egui::ThemePreference::Dark);
                app.settings.ai_provider = chat;
                app.settings.translate_provider = tr;
                app.settings.translate_apple = apple;
                if apple {
                    // the download row only exists while some language needs a download
                    app.apple_langs.insert("English".into(), ferrocast_core::translate::AppleLang::NeedsDownload);
                }
                seed_model_lists(&mut app);
                render(&mut app, vec![]);
                render(&mut app, vec![]);
                if drawn_rows(&app).iter().any(|(t, n)| *t == e.tab && n == e.title) {
                    found = true;
                    break 'variants;
                }
            }
        }
        assert!(found, "{:?} is indexed on tab {} ({}) but that tab never draws it", e.title, e.tab, TAB_NAMES[e.tab]);
    }
    APPLE_AVAILABLE.with(|a| a.set(false));
    let tab_of_title = |t: &str| ENTRIES.iter().find(|e| e.title == t).map(|e| TAB_NAMES[e.tab]);
    assert_eq!(tab_of_title("Notion token"), Some("Integrations"));
    assert_eq!(tab_of_title("Capture target"), Some("Integrations"));
    assert_eq!(tab_of_title("Accessibility"), Some("Advanced"));
    assert_eq!(tab_of_title("Data folder"), Some("Advanced"));
    assert_eq!(tab_of_title("Default folder"), Some("Scripts"));
}

// ---- Search Notion (command)

use crate::notion_io::testing::{serve_seq, Loopback};

const PAGE2_JSON: &str = r#"{"object":"list","results":[
  {"object":"page","id":"p9","url":"https://app.notion.com/p/p9","properties":{"title":{"type":"title","title":[{"plain_text":"Roadmap"}]}}}
],"has_more":false,"next_cursor":null}"#;
const MORE_JSON: &str = r#"{"object":"list","results":[
  {"object":"page","id":"p1","url":"https://app.notion.com/p/p1","last_edited_time":"2026-03-11T10:00:00.000Z","icon":{"type":"emoji","emoji":"📥"},"properties":{"title":{"type":"title","title":[{"plain_text":"Inbox"}]}}},
  {"object":"data_source","id":"d1","url":"https://app.notion.com/d1","title":[{"plain_text":"Tasks"}]}
],"has_more":true,"next_cursor":"cur-1"}"#;
const UNAUTH_JSON: &str = r#"{"object":"error","status":401,"code":"unauthorized","message":"API token is invalid."}"#;

/// The launcher on Search Notion with the fake Keychain and loopback base URL of this test thread.
fn nview(token: Option<&str>, lb: Option<&Loopback>) -> App {
    notion_io::set_test_token(token);
    notion_io::set_test_base(lb.map(|l| l.base.clone()));
    let _ = notion_io::take_opened();
    let _ = notion_io::take_copied();
    let mut app = fixture();
    app.run_builtin(Builtin::SearchNotion);
    app
}

fn nv_titles(app: &App) -> Vec<String> {
    app.sections().iter().flat_map(|s| s.items.iter().map(|i| i.title.clone())).collect()
}

/// The JSON body of a raw request captured by the loopback server.
fn body_of(req: &str) -> serde_json::Value {
    serde_json::from_str(req.split("\r\n\r\n").nth(1).unwrap()).unwrap()
}

#[test]
fn the_command_exists_with_its_keywords_and_opens_its_own_view() {
    let item = Builtin::ALL.iter().find(|b| b.title() == "Search Notion").expect("registered");
    for k in ["notion", "pages", "wiki", "docs", "notes"] {
        assert!(item.keywords().iter().any(|w| w == k), "{k}");
    }
    assert_eq!(item.id(), "builtin:search-notion");
    let app = nview(None, None);
    assert!(matches!(app.view(), View::NotionSearch));
    assert_eq!(app.view().placeholder(), "Search Notion…");
    assert_eq!(app.view().title(), "Search Notion");
    assert!(app.view().shape() == Shape::List);
}

#[test]
fn without_a_token_nothing_is_sent_and_the_only_row_opens_settings_on_integrations() {
    let lb = serve_seq(vec![(200, MORE_JSON)]);
    let mut app = nview(None, Some(&lb));
    app.tick_notion_view(0.0);
    drain_until(&mut app, |a| a.nv.connected == Some(false));
    // typing does not start requests either
    app.query = "plan".into();
    app.tick_notion_view(1.0);
    app.tick_notion_view(2.0);
    std::thread::sleep(Duration::from_millis(150));
    app.drain();
    assert!(lb.requests().is_empty(), "no token, no request: {:?}", lb.requests());
    let secs = app.sections();
    assert_eq!(secs.len(), 1);
    assert_eq!(secs[0].title, "Connect Notion to search your pages");
    assert_eq!(secs[0].items.len(), 1);
    assert_eq!(secs[0].items[0].title, "Open Settings → Integrations");
    let item = secs[0].items[0].clone();
    app.run_item(&item);
    assert!(matches!(app.view(), View::Settings));
    assert_eq!(app.ctx.data(|d| d.get_temp::<usize>(Id::new("settings_tab"))), Some(NOTION_TAB));
}

#[test]
fn opening_the_view_lists_the_twenty_latest_pages_and_shows_kind_icon_and_time() {
    let lb = serve_seq(vec![(200, MORE_JSON)]);
    let mut app = nview(Some("ntn_tok"), Some(&lb));
    app.tick_notion_view(0.0);
    assert!(app.nv.searching, "the request is in flight");
    drain_until(&mut app, |a| !a.nv.searching);
    let reqs = lb.requests();
    assert_eq!(reqs.len(), 1);
    assert!(reqs[0].starts_with("POST /v1/search"), "{}", reqs[0]);
    let want: serde_json::Value = serde_json::from_str(&ferrocast_core::notion::search_body("", None, 20)).unwrap();
    assert_eq!(body_of(&reqs[0]), want);
    assert_eq!(want["page_size"], 20);
    assert_eq!(want["sort"]["timestamp"], "last_edited_time");
    let secs = app.sections();
    assert_eq!(secs[0].title, "Recently edited");
    let rows = &secs[0].items;
    assert_eq!((rows[0].title.as_str(), rows[0].subtitle.as_str(), &rows[0].icon), ("Inbox", "Page", &Icon::Emoji("📥".into())));
    assert_eq!((rows[1].title.as_str(), rows[1].subtitle.as_str()), ("Tasks", "Database"));
    assert!(matches!(rows[1].icon, Icon::Glyph(..)), "no emoji: a generic glyph");
    assert!(!rows[0].accessory.is_empty(), "relative edit time on rows that have one");
    assert_eq!(rows[1].accessory, "", "none when the API gave no time");
    // a render with the results does not panic and the footer shows no busy state any more
    render(&mut app, vec![]);
}

#[test]
fn typing_waits_250_ms_after_the_last_key_then_sends_exactly_one_request() {
    let lb = serve_seq(vec![(200, MORE_JSON), (200, PAGE2_JSON)]);
    let mut app = nview(Some("ntn_tok"), Some(&lb));
    app.tick_notion_view(0.0);
    drain_until(&mut app, |a| !a.nv.searching);
    app.query = "r".into();
    app.tick_notion_view(10.0);
    app.query = "road".into();
    app.tick_notion_view(10.2);
    app.tick_notion_view(10.3);
    app.tick_notion_view(10.44);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(lb.requests().len(), 1, "still inside the debounce window of the last key");
    app.tick_notion_view(10.46);
    drain_until(&mut app, |a| !a.nv.searching);
    for t in [10.5, 11.0, 12.0] {
        app.tick_notion_view(t);
    }
    std::thread::sleep(Duration::from_millis(100));
    let reqs = lb.requests();
    assert_eq!(reqs.len(), 2, "exactly one request for the typed word: {reqs:?}");
    assert_eq!(body_of(&reqs[1])["query"], "road");
    assert!(body_of(&reqs[1]).get("sort").is_none());
    assert_eq!(nv_titles(&app), ["Roadmap"]);
    assert_eq!(app.sections()[0].title, "Results");
}

#[test]
fn an_answer_from_an_older_request_is_dropped() {
    let lb = serve_seq(vec![(200, MORE_JSON)]);
    let mut app = nview(Some("ntn_tok"), Some(&lb));
    app.tick_notion_view(0.0);
    drain_until(&mut app, |a| !a.nv.searching);
    let first = app.nv.request;
    app.query = "x".into();
    app.tick_notion_view(5.0);
    app.tick_notion_view(5.3);
    assert_eq!(app.nv.request, first + 1, "each request has a higher id");
    let stale = ferrocast_core::notion::parse_search(PAGE2_JSON).unwrap();
    app.bus.send(Msg::NotionViewSearched { request: first, append: false, result: Ok(stale) });
    app.drain();
    assert_eq!(nv_titles(&app).first().map(String::as_str), Some("Inbox"), "stale rows ignored");
    assert!(app.nv.searching, "still waiting for the newest answer");
}

#[test]
fn has_more_adds_a_load_more_row_that_appends_the_next_page_with_the_cursor() {
    let lb = serve_seq(vec![(200, MORE_JSON), (200, PAGE2_JSON)]);
    let mut app = nview(Some("ntn_tok"), Some(&lb));
    app.tick_notion_view(0.0);
    drain_until(&mut app, |a| !a.nv.searching);
    assert_eq!(nv_titles(&app), ["Inbox", "Tasks", "Load more"]);
    let more = app.sections()[0].items[2].clone();
    assert!(matches!(more.kind, Kind::NotionLoadMore));
    app.run_item(&more);
    assert!(app.nv.searching);
    drain_until(&mut app, |a| !a.nv.searching);
    let reqs = lb.requests();
    assert_eq!(body_of(&reqs[1])["start_cursor"], "cur-1");
    assert_eq!(body_of(&reqs[1])["page_size"], 20);
    assert_eq!(nv_titles(&app), ["Inbox", "Tasks", "Roadmap"], "appended, and no Load more once has_more is false");
}

#[test]
fn a_401_toasts_the_exact_message_and_keeps_the_previous_rows() {
    let lb = serve_seq(vec![(200, MORE_JSON), (401, UNAUTH_JSON)]);
    let mut app = nview(Some("ntn_tok"), Some(&lb));
    app.tick_notion_view(0.0);
    drain_until(&mut app, |a| !a.nv.searching);
    app.query = "zzz".into();
    app.tick_notion_view(3.0);
    app.tick_notion_view(3.3);
    drain_until(&mut app, |a| !a.nv.searching);
    assert_eq!(toast_text(&app), ferrocast_core::notion::NotionError::Unauthorized.user_message());
    assert_eq!(nv_titles(&app), ["Inbox", "Tasks", "Load more"], "the list stays as it was");
}

#[test]
fn enter_opens_the_url_and_the_actions_panel_offers_open_copy_link_and_capture_target() {
    let lb = serve_seq(vec![(200, MORE_JSON)]);
    let mut app = nview(Some("ntn_tok"), Some(&lb));
    app.tick_notion_view(0.0);
    drain_until(&mut app, |a| !a.nv.searching);
    let page = app.sections()[0].items[0].clone();
    let acts = app.actions_for(&page);
    let labels: Vec<_> = acts.iter().map(|a| a.title.as_str()).collect();
    assert_eq!(labels, ["Open", "Copy Link", "Set as Capture Target", "Add Text Here…"]);
    assert_eq!(app.primary_label(&page), "Open");
    // Enter in the list
    app.nav_at = Instant::now() - Duration::from_secs(1);
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Enter)]);
    assert_eq!(notion_io::take_opened(), ["https://app.notion.com/p/p1"]);
    // Copy Link
    let mut app = nview(Some("ntn_tok"), Some(&lb));
    app.perform(Act::CopyText("https://app.notion.com/p/p1".into()), false);
    assert_eq!(notion_io::take_copied(), ["https://app.notion.com/p/p1"]);
    // a link that is not http(s) is never handed to `open`
    app.notion_open_url("-a Calculator");
    assert!(notion_io::take_opened().is_empty());
    assert_eq!(toast_text(&app), "This Notion result has no link to open.");
}

#[test]
fn set_as_capture_target_persists_to_settings_json_and_toasts() {
    let lb = serve_seq(vec![(200, MORE_JSON)]);
    let mut app = nview(Some("ntn_tok"), Some(&lb));
    app.tick_notion_view(0.0);
    drain_until(&mut app, |a| !a.nv.searching);
    let db = app.sections()[0].items[1].clone();
    let act = app.actions_for(&db).into_iter().find(|a| a.title == "Set as Capture Target").unwrap();
    app.perform(act.kind, false);
    assert_eq!(toast_text(&app), "Capture target set to Tasks");
    let saved: Settings = app.store.load("settings.json");
    let t = saved.notion_target.expect("saved");
    assert_eq!((t.id.as_str(), t.kind.as_str(), t.title.as_str()), ("d1", "data_source", "Tasks"));
    let file = std::fs::read_to_string(app.store.root.join("settings.json")).unwrap();
    assert!(!file.contains("ntn_tok"));
}

#[test]
fn typing_is_not_blocked_while_a_request_is_in_flight() {
    let lb = serve_seq(vec![(200, MORE_JSON)]);
    let mut app = nview(Some("ntn_tok"), Some(&lb));
    app.tick_notion_view(0.0);
    assert!(app.nv.searching);
    render(&mut app, vec![]);
    render(&mut app, vec![]);
    render(&mut app, vec![Event::Text("a".into())]);
    assert_eq!(app.query, "a", "typing is not blocked by the request");
    drain_until(&mut app, |a| !a.nv.searching);
}

// ---- Add to Notion / Add Clipboard to Notion

const OK_JSON: &str = r#"{"object":"page_markdown","id":"p1"}"#;
const CREATED_JSON: &str = r#"{"object":"page","id":"new1","url":"https://app.notion.com/p/new1"}"#;
const SCHEMA_JSON: &str = r#"{"object":"data_source","id":"d1","properties":{"Tag":{"type":"select"},"Task name":{"type":"title"}}}"#;
const RATE_JSON: &str = r#"{"object":"error","status":429,"code":"rate_limited","message":"slow down"}"#;
const FORBIDDEN_JSON: &str = r#"{"object":"error","status":403,"code":"restricted_resource","message":"no"}"#;
const NOT_FOUND_JSON: &str = r#"{"object":"error","status":404,"code":"object_not_found","message":"gone"}"#;

fn target_of(kind: &str) -> Option<ferrocast_core::models::NotionTarget> {
    let (id, title) = if kind == "page" { ("p1", "Inbox") } else { ("d1", "Tasks") };
    Some(ferrocast_core::models::NotionTarget { id: id.into(), kind: kind.into(), title: title.into() })
}

/// The launcher with the fake Keychain, loopback base URL, fake clipboard and capture target of this test thread.
fn nadd(token: Option<&str>, lb: Option<&Loopback>, kind: Option<&str>) -> App {
    notion_io::set_test_token(token);
    notion_io::set_test_base(lb.map(|l| l.base.clone()));
    notion_io::set_test_clipboard(None);
    let _ = notion_io::take_opened();
    let mut app = fixture();
    app.settings.notion_target = kind.and_then(target_of);
    app
}

fn fill(app: &mut App, title: &str, text: &str) {
    match app.stack.last_mut().unwrap() {
        View::NotionAdd { title: t, text: x, .. } => {
            *t = title.into();
            *x = text.into();
        }
        _ => panic!("not on the Add to Notion form"),
    }
}

fn form(app: &App) -> (String, String) {
    match app.view() {
        View::NotionAdd { title, text, .. } => (title.clone(), text.clone()),
        _ => panic!("not on the Add to Notion form"),
    }
}

/// Open the form, type, send with the form action and wait for the worker.
fn send_form(app: &mut App, title: &str, text: &str) {
    app.run_builtin(Builtin::AddToNotion);
    fill(app, title, text);
    app.notion_add_submit();
    assert!(app.na.sending, "the request is in flight");
    drain_until(app, |a| !a.na.sending);
}

fn md_of(req: &str) -> String {
    body_of(req)["insert_content"]["content"].as_str().unwrap().to_string()
}

#[test]
fn the_two_commands_exist_with_their_keywords() {
    let add = Builtin::ALL.iter().find(|b| b.title() == "Add to Notion").expect("registered");
    for k in ["notion", "capture", "save", "note", "clip"] {
        assert!(add.keywords().iter().any(|w| w == k), "{k}");
    }
    let clip = Builtin::ALL.iter().find(|b| b.title() == "Add Clipboard to Notion").expect("registered");
    for k in ["notion", "clipboard", "save"] {
        assert!(clip.keywords().iter().any(|w| w == k), "{k}");
    }
    assert_ne!(add.id(), clip.id());
}

#[test]
fn without_a_token_both_commands_toast_open_settings_integrations_and_send_nothing() {
    for b in [Builtin::AddToNotion, Builtin::AddClipboardToNotion] {
        let lb = serve_seq(vec![(200, OK_JSON)]);
        let mut app = nadd(None, Some(&lb), Some("page"));
        notion_io::set_test_clipboard(Some("hello"));
        app.run_builtin(b);
        assert_eq!(toast_text(&app), "Connect Notion first: Settings → Integrations.");
        assert!(matches!(app.view(), View::Settings), "{b:?}");
        assert_eq!(app.ctx.data(|d| d.get_temp::<usize>(Id::new("settings_tab"))), Some(NOTION_TAB));
        std::thread::sleep(Duration::from_millis(100));
        assert!(lb.requests().is_empty(), "no token, no request");
    }
}

#[test]
fn with_a_token_but_no_target_both_commands_toast_open_settings_integrations_and_send_nothing() {
    for b in [Builtin::AddToNotion, Builtin::AddClipboardToNotion] {
        let lb = serve_seq(vec![(200, OK_JSON)]);
        let mut app = nadd(Some("ntn_tok"), Some(&lb), None);
        notion_io::set_test_clipboard(Some("hello"));
        app.run_builtin(b);
        assert_eq!(toast_text(&app), "Choose a capture target in Settings → Integrations.");
        assert!(matches!(app.view(), View::Settings), "{b:?}");
        assert_eq!(app.ctx.data(|d| d.get_temp::<usize>(Id::new("settings_tab"))), Some(NOTION_TAB));
        std::thread::sleep(Duration::from_millis(100));
        assert!(lb.requests().is_empty(), "no target, no request");
    }
}

#[test]
fn the_form_opens_with_two_empty_fields_a_send_hint_and_renders() {
    let mut app = nadd(Some("ntn_tok"), None, Some("page"));
    app.run_builtin(Builtin::AddToNotion);
    assert!(matches!(app.view(), View::NotionAdd { .. }));
    assert_eq!(app.view().title(), "Add to Notion");
    assert!(app.view().shape() == Shape::Form);
    assert_eq!(form(&app), (String::new(), String::new()));
    assert!(app.view().footer_hints().contains(&("Send", "⌘↵")));
    render(&mut app, vec![]);
    render(&mut app, vec![]);
}

#[test]
fn a_page_target_sends_exactly_one_patch_that_inserts_the_text_at_the_end() {
    let lb = serve_seq(vec![(200, OK_JSON)]);
    let mut app = nadd(Some("ntn_tok"), Some(&lb), Some("page"));
    send_form(&mut app, "", "Buy milk\n- eggs");
    let reqs = lb.requests();
    assert_eq!(reqs.len(), 1, "{reqs:?}");
    assert!(reqs[0].starts_with("PATCH /v1/pages/p1/markdown"), "{}", reqs[0]);
    let want: serde_json::Value = serde_json::from_str(&ferrocast_core::notion::append_markdown_body("Buy milk\n- eggs")).unwrap();
    assert_eq!(body_of(&reqs[0]), want);
    assert_eq!(body_of(&reqs[0])["insert_content"]["position"]["type"], "end");
    assert_eq!(toast_text(&app), "Added to Inbox");
    assert_eq!(form(&app), (String::new(), String::new()), "success clears the form");
}

#[test]
fn a_title_becomes_a_heading_before_the_text_on_a_page_target() {
    let lb = serve_seq(vec![(200, OK_JSON)]);
    let mut app = nadd(Some("ntn_tok"), Some(&lb), Some("page"));
    send_form(&mut app, "Groceries", "milk");
    assert_eq!(md_of(&lb.requests()[0]), "## Groceries\n\nmilk");
}

#[test]
fn a_data_source_target_reads_the_schema_once_then_creates_pages_with_the_title_property() {
    let lb = serve_seq(vec![(200, SCHEMA_JSON), (200, CREATED_JSON), (200, CREATED_JSON)]);
    let mut app = nadd(Some("ntn_tok"), Some(&lb), Some("data_source"));
    send_form(&mut app, "Call Ana", "about the offer");
    let reqs = lb.requests();
    assert_eq!(reqs.len(), 2, "{reqs:?}");
    assert!(reqs[0].starts_with("GET /v1/data_sources/d1"), "{}", reqs[0]);
    assert!(reqs[1].starts_with("POST /v1/pages"), "{}", reqs[1]);
    let b = body_of(&reqs[1]);
    assert_eq!(b["parent"], serde_json::json!({"data_source_id":"d1"}));
    assert_eq!(b["properties"], serde_json::json!({"Task name":{"title":[{"text":{"content":"Call Ana"}}]}}));
    assert_eq!(b["markdown"], "about the offer");
    assert_eq!(toast_text(&app), "Added to Tasks");
    // the second send reuses the cached schema
    send_form(&mut app, "Second", "x");
    let reqs = lb.requests();
    assert_eq!(reqs.len(), 3, "no second schema request: {reqs:?}");
    assert!(reqs[2].starts_with("POST /v1/pages"));
    assert!(body_of(&reqs[2])["properties"].get("Task name").is_some());
}

#[test]
fn without_a_title_a_database_page_takes_the_first_line_and_the_body_is_the_rest() {
    let lb = serve_seq(vec![(200, SCHEMA_JSON), (200, CREATED_JSON)]);
    let mut app = nadd(Some("ntn_tok"), Some(&lb), Some("data_source"));
    send_form(&mut app, "  ", "\n\nFirst line\nsecond\nthird");
    let b = body_of(&lb.requests()[1]);
    assert_eq!(b["properties"]["Task name"]["title"][0]["text"]["content"], "First line");
    assert_eq!(b["markdown"], "second\nthird");
}

#[test]
fn a_long_unicode_first_line_is_cut_at_100_characters_and_stays_whole_in_the_body() {
    let line = "é日".repeat(60); // 120 chars, 240 bytes: a byte cut would split a character
    let lb = serve_seq(vec![(200, SCHEMA_JSON), (200, CREATED_JSON)]);
    let mut app = nadd(Some("ntn_tok"), Some(&lb), Some("data_source"));
    send_form(&mut app, "", &format!("{line}\nrest"));
    let b = body_of(&lb.requests()[1]);
    let title = b["properties"]["Task name"]["title"][0]["text"]["content"].as_str().unwrap();
    assert_eq!(title.chars().count(), 100);
    assert!(line.starts_with(title));
    assert_eq!(b["markdown"], format!("{line}\nrest"), "nothing the user typed is lost to the cut");
}

#[test]
fn the_clipboard_command_sends_the_clipboard_text_without_a_form() {
    let lb = serve_seq(vec![(200, OK_JSON)]);
    let mut app = nadd(Some("ntn_tok"), Some(&lb), Some("page"));
    notion_io::set_test_clipboard(Some("copied words"));
    app.run_builtin(Builtin::AddClipboardToNotion);
    assert!(!matches!(app.view(), View::NotionAdd { .. }), "no form");
    drain_until(&mut app, |a| !a.na.sending);
    let reqs = lb.requests();
    assert_eq!(reqs.len(), 1);
    assert_eq!(md_of(&reqs[0]), "copied words");
    assert_eq!(toast_text(&app), "Added to Inbox");
}

#[test]
fn an_empty_or_non_text_clipboard_toasts_and_sends_nothing() {
    for clip in [None, Some(""), Some("  \n ")] {
        let lb = serve_seq(vec![(200, OK_JSON)]);
        let mut app = nadd(Some("ntn_tok"), Some(&lb), Some("page"));
        notion_io::set_test_clipboard(clip);
        app.run_builtin(Builtin::AddClipboardToNotion);
        assert_eq!(toast_text(&app), "The clipboard has no text.");
        std::thread::sleep(Duration::from_millis(100));
        assert!(lb.requests().is_empty());
    }
}

#[test]
fn rejections_toast_the_exact_message_and_keep_the_typed_text() {
    use ferrocast_core::notion::NotionError as E;
    for (status, body, err) in
        [(401, UNAUTH_JSON, E::Unauthorized), (403, FORBIDDEN_JSON, E::Forbidden), (404, NOT_FOUND_JSON, E::NotFound)]
    {
        let lb = serve_seq(vec![(status, body)]);
        let mut app = nadd(Some("ntn_tok"), Some(&lb), Some("page"));
        send_form(&mut app, "T", "typed words");
        assert_eq!(toast_text(&app), err.user_message(), "{status}");
        assert_eq!(form(&app), ("T".into(), "typed words".into()), "the form keeps the text on {status}");
        assert_eq!(lb.requests().len(), 1);
    }
}

#[test]
fn text_of_250000_chars_is_sent_as_three_sequential_appends_split_at_blank_lines() {
    let para = "x".repeat(998);
    let text = (0..250).map(|i| format!("{i:03}{para}")).collect::<Vec<_>>().join("\n\n");
    assert!(text.chars().count() > 250_000);
    let lb = serve_seq(vec![(200, OK_JSON)]);
    let mut app = nadd(Some("ntn_tok"), Some(&lb), Some("page"));
    send_form(&mut app, "", &text);
    let reqs = lb.requests();
    assert_eq!(reqs.len(), 3, "3 calls");
    let parts: Vec<String> = reqs.iter().map(|r| md_of(r)).collect();
    assert!(parts.iter().all(|p| p.chars().count() <= 100_000));
    assert!(parts.iter().all(|p| p.starts_with(|c: char| c.is_ascii_digit())), "each part starts on a paragraph");
    assert_eq!(parts.concat(), text, "nothing lost, nothing added");
    assert_eq!(toast_text(&app), "Added to Inbox");
}

#[test]
fn a_failure_on_part_two_says_one_of_three_were_added_and_keeps_the_text() {
    let para = "y".repeat(998);
    let text = (0..250).map(|i| format!("{i:03}{para}")).collect::<Vec<_>>().join("\n\n");
    let lb = serve_seq(vec![(200, OK_JSON), (500, "{}"), (200, OK_JSON)]);
    let mut app = nadd(Some("ntn_tok"), Some(&lb), Some("page"));
    send_form(&mut app, "", &text);
    let t = toast_text(&app);
    assert!(t.contains("1 of 3"), "{t}");
    assert!(t.contains("the rest were not sent"), "{t}");
    assert!(t.contains(&ferrocast_core::notion::NotionError::Server(500).user_message()), "{t}");
    assert_eq!(lb.requests().len(), 2, "stops at the failure");
    assert_eq!(form(&app).1, text, "the typed text stays in the form");
}

#[test]
fn a_429_is_retried_once_after_one_second_and_then_succeeds() {
    let lb = serve_seq(vec![(429, RATE_JSON), (200, OK_JSON)]);
    let mut app = nadd(Some("ntn_tok"), Some(&lb), Some("page"));
    let t0 = Instant::now();
    send_form(&mut app, "", "retry me");
    assert!(t0.elapsed() >= Duration::from_millis(1000), "waited for the retry: {:?}", t0.elapsed());
    let reqs = lb.requests();
    assert_eq!(reqs.len(), 2);
    assert_eq!(md_of(&reqs[0]), md_of(&reqs[1]));
    assert_eq!(toast_text(&app), "Added to Inbox");
}

#[test]
fn a_second_429_is_not_retried_again_and_reports_the_rate_limit() {
    let lb = serve_seq(vec![(429, RATE_JSON)]);
    let mut app = nadd(Some("ntn_tok"), Some(&lb), Some("page"));
    send_form(&mut app, "", "keep me");
    assert_eq!(lb.requests().len(), 2, "one retry only");
    assert_eq!(toast_text(&app), ferrocast_core::notion::NotionError::RateLimited { retry_after_secs: None }.user_message());
    assert_eq!(form(&app).1, "keep me");
}

#[test]
fn command_enter_in_the_form_sends() {
    let lb = serve_seq(vec![(200, OK_JSON)]);
    let mut app = nadd(Some("ntn_tok"), Some(&lb), Some("page"));
    app.run_builtin(Builtin::AddToNotion);
    fill(&mut app, "", "via keys");
    render(&mut app, vec![]);
    let ev = Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND };
    render(&mut app, vec![ev]);
    drain_until(&mut app, |a| !a.na.sending);
    assert_eq!(md_of(&lb.requests()[0]), "via keys");
}

#[test]
fn an_empty_form_sends_nothing() {
    let lb = serve_seq(vec![(200, OK_JSON)]);
    let mut app = nadd(Some("ntn_tok"), Some(&lb), Some("page"));
    app.run_builtin(Builtin::AddToNotion);
    app.notion_add_submit();
    assert_eq!(toast_text(&app), "Write something to send first.");
    assert!(!app.na.sending);
    std::thread::sleep(Duration::from_millis(100));
    assert!(lb.requests().is_empty());
}

#[test]
fn add_text_here_opens_the_form_for_that_result_without_changing_the_stored_target() {
    let lb = serve_seq(vec![(200, MORE_JSON), (200, SCHEMA_JSON), (200, CREATED_JSON)]);
    let mut app = nadd(Some("ntn_tok"), Some(&lb), Some("page"));
    app.run_builtin(Builtin::SearchNotion);
    app.tick_notion_view(0.0);
    drain_until(&mut app, |a| !a.nv.searching);
    let db = app.sections()[0].items[1].clone();
    let act = app.actions_for(&db).into_iter().find(|a| a.title == "Add Text Here…").expect("in the actions panel");
    app.perform(act.kind, false);
    match app.view() {
        View::NotionAdd { target, .. } => {
            assert_eq!((target.id.as_str(), target.kind.as_str(), target.title.as_str()), ("d1", "data_source", "Tasks"))
        }
        _ => panic!("the form did not open"),
    }
    assert_eq!(app.settings.notion_target, target_of("page"), "stored target untouched");
    let saved: Settings = app.store.load("settings.json");
    assert_eq!(saved.notion_target, None, "nothing was written to settings.json");
    fill(&mut app, "One-off", "text");
    app.notion_add_submit();
    drain_until(&mut app, |a| !a.na.sending);
    let reqs = lb.requests();
    assert!(reqs[1].starts_with("GET /v1/data_sources/d1"), "{}", reqs[1]);
    assert!(reqs[2].starts_with("POST /v1/pages"), "{}", reqs[2]);
    assert_eq!(app.settings.notion_target, target_of("page"));
}

#[test]
fn add_text_here_without_a_token_opens_settings_instead() {
    let mut app = nadd(None, None, Some("page"));
    let item = ferrocast_core::notion::parse_search(MORE_JSON).unwrap().items.remove(0);
    app.perform(Act::NotionAddHere(item), false);
    assert_eq!(toast_text(&app), "Connect Notion first: Settings → Integrations.");
    assert!(matches!(app.view(), View::Settings));
}

// ---- Settings → Integrations → Obsidian

use std::path::{Path, PathBuf};

const OBS_CONFIG: &str = r#"{"vaults":{
  "a1":{"path":"/tmp/fc-vaults/Work Notes","ts":100},
  "b2":{"path":"/tmp/fc-vaults/Personal","ts":200,"open":true}
}}"#;

fn obs_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ferrocast-obs-{tag}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn obs_app() -> App {
    settings_app(NOTION_TAB, egui::ThemePreference::Dark)
}

/// Scan a fixture `obsidian.json` (never the user's) with an injected app detection.
fn obs_scan(app: &mut App, json: Option<&str>, installed: bool) {
    let dir = obs_dir("cfg");
    let cfg = dir.join("obsidian.json");
    if let Some(j) = json {
        std::fs::write(&cfg, j).unwrap();
    }
    app.obsidian_scan_with(cfg, move || installed);
}

#[test]
fn scanning_goes_through_the_bus_and_fills_the_vault_list_and_the_open_vault() {
    let mut app = obs_app();
    assert_eq!(app.obsidian_vaults(), None, "nothing known before the scan");
    obs_scan(&mut app, Some(OBS_CONFIG), true);
    // the scan was queued on a thread: only draining the bus applies the result
    assert!(app.obsidian.scanning, "scanning flag set synchronously");
    drain_until(&mut app, |a| !a.obsidian.scanning);
    let names: Vec<String> = app.obsidian_vaults().unwrap().iter().map(|v| v.name.clone()).collect();
    assert_eq!(names, ["Personal", "Work Notes"], "open vault first");
    assert_eq!(app.obsidian.open, Some(PathBuf::from("/tmp/fc-vaults/Personal")));
    assert_eq!(app.obsidian_installed(), Some(true));
}

#[test]
fn a_missing_obsidian_json_means_no_vaults_not_an_error() {
    let mut app = obs_app();
    obs_scan(&mut app, None, false);
    drain_until(&mut app, |a| !a.obsidian.scanning);
    assert_eq!(app.obsidian_vaults(), Some(vec![]));
    assert_eq!(app.obsidian_installed(), Some(false));
}

#[test]
fn picking_a_vault_stores_its_path_in_settings_json_and_toasts() {
    let mut app = obs_app();
    obs_scan(&mut app, Some(OBS_CONFIG), true);
    drain_until(&mut app, |a| !a.obsidian.scanning);
    app.obsidian_pick(1);
    assert_eq!(toast_text(&app), "Obsidian vault set to Work Notes");
    let saved: Settings = app.store.load("settings.json");
    assert_eq!(saved.obsidian_vault, Some(PathBuf::from("/tmp/fc-vaults/Work Notes")));
    let v = app.obsidian_vault().expect("selected vault");
    assert_eq!((v.name.as_str(), v.path.as_path()), ("Work Notes", Path::new("/tmp/fc-vaults/Work Notes")));
    // an index past the list changes nothing
    app.obsidian_pick(9);
    assert_eq!(app.obsidian_vault().unwrap().name, "Work Notes");
}

#[test]
fn choose_folder_stores_an_existing_directory_and_strips_the_trailing_slash() {
    let mut app = obs_app();
    let dir = obs_dir("pick");
    let vault = dir.join("My Vault");
    std::fs::create_dir_all(&vault).unwrap();
    let picked = PathBuf::from(format!("{}/", vault.display()));
    app.obsidian_choose_folder_with(move || Some(picked));
    assert!(app.obsidian.picking, "the picker runs on a thread");
    drain_until(&mut app, |a| !a.obsidian.picking);
    assert_eq!(toast_text(&app), "Obsidian vault set to My Vault");
    assert_eq!(app.settings.obsidian_vault, Some(vault.clone()));
    let saved: Settings = app.store.load("settings.json");
    assert_eq!(saved.obsidian_vault, Some(vault));
}

#[test]
fn choose_folder_rejects_a_missing_path_and_a_file_and_stores_nothing() {
    let mut app = obs_app();
    let dir = obs_dir("bad");
    let file = dir.join("note.md");
    std::fs::write(&file, "x").unwrap();
    for bad in [dir.join("nope"), file] {
        app.toast = None;
        let b = bad.clone();
        app.obsidian_choose_folder_with(move || Some(b));
        drain_until(&mut app, |a| !a.obsidian.picking);
        assert_eq!(toast_text(&app), "That is not a folder. Choose your Obsidian vault folder.", "{bad:?}");
        assert_eq!(app.settings.obsidian_vault, None);
        let saved: Settings = app.store.load("settings.json");
        assert_eq!(saved.obsidian_vault, None);
    }
    // cancelling the dialog is silent
    app.toast = None;
    app.obsidian_choose_folder_with(|| None);
    drain_until(&mut app, |a| !a.obsidian.picking);
    assert_eq!(toast_text(&app), "");
}

#[test]
fn clear_unsets_the_vault() {
    let mut app = obs_app();
    app.settings.obsidian_vault = Some(PathBuf::from("/tmp/fc-vaults/Personal"));
    app.obsidian_clear();
    assert_eq!(toast_text(&app), "Obsidian vault cleared");
    assert_eq!(app.obsidian_vault(), None);
    let saved: Settings = app.store.load("settings.json");
    assert_eq!(saved.obsidian_vault, None);
}

#[test]
fn the_status_line_follows_the_detection_result() {
    assert_eq!(obsidian_status(None), "Checking for Obsidian…");
    assert_eq!(obsidian_status(Some(true)), "Obsidian is installed");
    assert_eq!(obsidian_status(Some(false)), "Obsidian is not installed. Download it from obsidian.md.");
    let mut app = obs_app();
    assert_eq!(app.obsidian_installed(), None);
    obs_scan(&mut app, None, false);
    drain_until(&mut app, |a| !a.obsidian.scanning);
    assert_eq!(app.obsidian_installed(), Some(false));
}

#[test]
fn the_obsidian_rows_are_drawn_and_a_bare_render_never_scans() {
    let mut app = obs_app();
    settle(&mut app);
    let rows: Vec<String> = drawn_rows(&app).into_iter().filter(|(t, _)| *t == NOTION_TAB).map(|r| r.1).collect();
    for want in ["Vault", "Vault folder", "Obsidian app"] {
        assert!(rows.iter().any(|r| r == want), "{want} is drawn: {rows:?}");
    }
    assert!(!app.obsidian.scanning, "tests never touch the real obsidian.json or mdfind");
}

// ---- Search Obsidian (command)

use ferrocast_core::obsidian::{self, Block};

/// A throwaway vault on disk: `(relative path, content)`; the returned path is the vault folder.
fn ov_vault(files: &[(&str, &str)]) -> PathBuf {
    let root = std::env::temp_dir().join(format!("ferrocast-ov-{}", uuid::Uuid::new_v4())).join("My Vault");
    for (rel, body) in files {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn ov_set_mtime(vault: &std::path::Path, rel: &str, secs_ago: u64) {
    let t = std::time::SystemTime::now() - Duration::from_secs(secs_ago);
    std::fs::File::options().write(true).open(vault.join(rel)).unwrap().set_modified(t).unwrap();
}

/// The launcher on Search Obsidian; `vault` is stored in the settings (no obsidian.json is read).
fn ov_app(vault: Option<&std::path::Path>) -> App {
    let _ = notion_io::take_opened();
    let _ = notion_io::take_copied();
    let mut app = fixture();
    app.settings.obsidian_vault = vault.map(|p| p.to_path_buf());
    app.run_builtin(Builtin::SearchObsidian);
    app
}

/// Open the view and wait until the index is in.
fn ov_ready(vault: &std::path::Path) -> App {
    let mut app = ov_app(Some(vault));
    drain_until(&mut app, |a| !a.ov.indexing);
    app
}

/// Run a ⌘K action the way the panel does.
fn ov_perform(app: &mut App, item: &Item, act: Act) {
    app.act_item = Some(item.clone());
    app.perform(act, false);
}

fn ov_titles(app: &App) -> Vec<String> {
    app.sections().iter().flat_map(|s| s.items.iter().map(|i| i.title.clone())).collect()
}

fn ov_sections(app: &App) -> Vec<(String, Vec<String>)> {
    app.sections().into_iter().map(|s| (s.title, s.items.into_iter().map(|i| i.title).collect())).collect()
}

/// Every text the last frame painted, joined by newlines.
fn painted(app: &mut App) -> String {
    fn walk(s: &egui::Shape, out: &mut Vec<String>) {
        match s {
            egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let input =
        egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(ui::WIDTH, ui::HEIGHT))), ..Default::default() };
    let ctx = app.ctx.clone();
    let mut output = ctx.run_ui(input, |ui| app.draw(ui));
    output.textures_delta.clear();
    let mut out = vec![];
    for c in &output.shapes {
        walk(&c.shape, &mut out);
    }
    out.join("\n")
}

#[test]
fn search_obsidian_is_a_command_with_its_keywords_and_its_own_view() {
    let b = Builtin::ALL.iter().find(|b| b.title() == "Search Obsidian").expect("registered");
    for k in ["obsidian", "vault", "notes", "markdown", "zettelkasten", "second brain"] {
        assert!(b.keywords().iter().any(|w| w == k), "{k}");
    }
    assert_eq!(b.id(), "builtin:search-obsidian");
    let app = ov_app(None);
    assert!(matches!(app.view(), View::ObsidianSearch));
    assert_eq!(app.view().placeholder(), "Search your vault…");
    assert_eq!(app.view().title(), "Search Obsidian");
    assert!(app.view().shape() == Shape::List);
}

#[test]
fn without_a_vault_the_only_row_opens_settings_on_integrations_and_nothing_is_indexed() {
    let mut app = ov_app(None);
    let secs = app.sections();
    assert_eq!(secs.len(), 1);
    assert_eq!(secs[0].title, "Choose your Obsidian vault");
    assert_eq!(secs[0].items.len(), 1);
    assert_eq!(secs[0].items[0].title, "Open Settings → Integrations");
    assert!(!app.ov.indexing && app.ov.index_request == 0, "no vault, no walk");
    let item = secs[0].items[0].clone();
    app.run_item(&item);
    assert!(matches!(app.view(), View::Settings));
    let tab: usize = app.ctx.data(|d| d.get_temp(Id::new("settings_tab"))).unwrap();
    assert_eq!(tab, draw::INTEGRATIONS_TAB);
}

#[test]
fn a_vault_missing_on_disk_says_so_with_the_same_row() {
    let gone = std::env::temp_dir().join(format!("ferrocast-ov-gone-{}", uuid::Uuid::new_v4()));
    let mut app = ov_app(Some(&gone));
    drain_until(&mut app, |a| !a.ov.indexing);
    let secs = app.sections();
    assert_eq!(secs.len(), 1);
    assert_eq!(secs[0].title, "The vault folder was not found");
    assert_eq!(secs[0].items[0].title, "Open Settings → Integrations");
    let item = secs[0].items[0].clone();
    app.run_item(&item);
    assert!(matches!(app.view(), View::Settings));
}

#[test]
fn the_index_arrives_through_the_bus_never_during_the_draw() {
    let vault = ov_vault(&[("Alpha.md", "# Alpha\n")]);
    let mut app = ov_app(Some(&vault));
    assert!(app.ov.indexing, "the walk runs on a thread");
    assert!(app.sections().is_empty(), "nothing is listed before the Bus delivers");
    let text = painted(&mut app);
    assert!(text.contains("Indexing your vault…"), "{text}");
    // frames alone never fill the list; only draining the Bus does
    assert!(app.ov.cache.is_none() || !app.ov.indexing);
    drain_until(&mut app, |a| !a.ov.indexing);
    assert_eq!(ov_titles(&app), vec!["Alpha"]);
    let text = painted(&mut app);
    assert!(!text.contains("Indexing your vault…"), "{text}");
}

#[test]
fn a_stale_index_answer_is_dropped_after_reopening_the_view() {
    let vault = ov_vault(&[("Alpha.md", "x")]);
    let mut app = ov_ready(&vault);
    let first = app.ov.index_request;
    app.pop();
    app.run_builtin(Builtin::SearchObsidian);
    assert_eq!(app.ov.index_request, first + 1, "each open has a higher id");
    app.bus.send(Msg::ObsidianIndexed {
        request: first,
        vault: vault.clone(),
        notes: Some(vec![obsidian::NoteEntry {
            rel_path: "Stale.md".into(),
            title: "Stale".into(),
            aliases: vec![],
            modified: 1,
            size: 1,
        }]),
    });
    app.drain();
    assert!(app.ov.indexing, "the stale answer did not end the newest request");
    assert!(!ov_titles(&app).contains(&"Stale".to_string()));
    drain_until(&mut app, |a| !a.ov.indexing);
    assert_eq!(ov_titles(&app), vec!["Alpha"]);
}

#[test]
fn reopening_shows_the_cached_index_at_once_while_a_refresh_runs() {
    let vault = ov_vault(&[("Alpha.md", "x")]);
    let mut app = ov_ready(&vault);
    app.pop();
    std::fs::write(vault.join("Beta.md"), "y").unwrap();
    app.run_builtin(Builtin::SearchObsidian);
    assert!(app.ov.indexing, "the refresh is in flight");
    assert_eq!(ov_titles(&app), vec!["Alpha"], "the last index shows immediately");
    drain_until(&mut app, |a| !a.ov.indexing);
    let mut t = ov_titles(&app);
    t.sort();
    assert_eq!(t, vec!["Alpha", "Beta"], "the refresh adds the new note");
}

#[test]
fn an_empty_query_lists_the_30_most_recent_notes_newest_first() {
    let names: Vec<String> = (0..35).map(|i| format!("Note {i:02}.md")).collect();
    let files: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), "body")).collect();
    let vault = ov_vault(&files);
    for (i, n) in names.iter().enumerate() {
        ov_set_mtime(&vault, n, 1000 + (35 - i as u64) * 60); // Note 34 is the newest
    }
    let app = ov_ready(&vault);
    let secs = ov_sections(&app);
    assert_eq!(secs.len(), 1);
    assert_eq!(secs[0].0, "Recent");
    assert_eq!(secs[0].1.len(), 30);
    assert_eq!(secs[0].1[0], "Note 34");
    assert_eq!(secs[0].1[29], "Note 05");
}

#[test]
fn a_query_lists_what_obsidian_search_returns_in_the_same_order() {
    let vault = ov_vault(&[
        ("Meeting notes.md", "x"),
        ("Projects/Meeting plan.md", "x"),
        ("Recipes/Pasta.md", "x"),
        ("Daily/2026-01-02.md", "# Standup meeting\n"),
    ]);
    let mut app = ov_ready(&vault);
    app.query = "meet".into();
    let entries = obsidian::index_vault(&vault);
    let want: Vec<String> = obsidian::search(&entries, "meet", 30).into_iter().map(|i| entries[i].title.clone()).collect();
    assert!(want.len() >= 2, "{want:?}");
    let secs = ov_sections(&app);
    assert_eq!(secs[0].0, "Notes");
    assert_eq!(secs[0].1, want);
}

#[test]
fn rows_show_the_title_the_folder_and_the_relative_modified_time() {
    let vault = ov_vault(&[("Projects/Deep/Kickoff.md", "# Project kickoff\n"), ("Root.md", "x")]);
    ov_set_mtime(&vault, "Projects/Deep/Kickoff.md", 2 * 3600 + 30);
    let app = ov_ready(&vault);
    let items: Vec<Item> = app.sections().into_iter().flat_map(|s| s.items).collect();
    let k = items.iter().find(|i| i.title == "Project kickoff").unwrap();
    assert_eq!(k.subtitle, "Projects/Deep");
    assert_eq!(k.accessory, "2h ago");
    let r = items.iter().find(|i| i.title == "Root").unwrap();
    assert_eq!(r.subtitle, "", "a note at the vault root has no folder");
}

#[test]
fn content_hits_are_appended_under_in_notes_without_repeating_title_matches() {
    let vault =
        ov_vault(&[("Zebra.md", "stripes"), ("Budget plan.md", "the quarterly zebra review is on Friday"), ("Other.md", "nothing here")]);
    let mut app = ov_ready(&vault);
    app.query = "zebra".into();
    app.tick_obsidian_view(0.0);
    assert_eq!(ov_sections(&app), vec![("Notes".to_string(), vec!["Zebra".to_string()])], "title matches first, content not asked yet");
    app.tick_obsidian_view(0.3);
    drain_until(&mut app, |a| a.ov.content.is_some());
    assert_eq!(
        ov_sections(&app),
        vec![("Notes".to_string(), vec!["Zebra".to_string()]), ("In notes".to_string(), vec!["Budget plan".to_string()])]
    );
}

#[test]
fn the_content_scan_waits_250_ms_and_skips_queries_under_three_characters() {
    let vault = ov_vault(&[("Budget plan.md", "the quarterly zebra review")]);
    let mut app = ov_ready(&vault);
    let base = app.ov.content_request;
    app.query = "ze".into();
    app.tick_obsidian_view(0.0);
    app.tick_obsidian_view(5.0);
    assert_eq!(app.ov.content_request, base, "two characters never scan contents");
    app.query = "zeb".into();
    app.tick_obsidian_view(10.0);
    app.tick_obsidian_view(10.2);
    assert_eq!(app.ov.content_request, base, "inside the debounce window");
    app.tick_obsidian_view(10.26);
    assert_eq!(app.ov.content_request, base + 1, "250 ms after the last keystroke");
    drain_until(&mut app, |a| a.ov.content.is_some());
    // typing again restarts the wait
    app.query = "zebr".into();
    app.tick_obsidian_view(20.0);
    app.tick_obsidian_view(20.2);
    assert_eq!(app.ov.content_request, base + 1);
    app.tick_obsidian_view(20.3);
    assert_eq!(app.ov.content_request, base + 2);
}

#[test]
fn a_stale_content_answer_is_dropped() {
    let vault = ov_vault(&[("Budget plan.md", "the quarterly zebra review"), ("Other.md", "zebra too")]);
    let mut app = ov_ready(&vault);
    app.query = "zebra".into();
    app.tick_obsidian_view(0.0);
    app.tick_obsidian_view(0.3);
    let id = app.ov.content_request;
    app.bus.send(Msg::ObsidianContent { request: id - 1, query: "zebra".into(), hits: vec!["Other.md".into()] });
    app.drain();
    assert!(app.ov.content.is_none(), "an older id is ignored");
    drain_until(&mut app, |a| a.ov.content.is_some());
    let (_, hits) = app.ov.content.clone().unwrap();
    assert_eq!(hits.len(), 2, "the newest answer stands: {hits:?}");
}

#[test]
fn clearing_the_query_below_three_characters_drops_in_flight_content_answers() {
    let vault = ov_vault(&[("Budget plan.md", "the quarterly zebra review")]);
    let mut app = ov_ready(&vault);
    app.query = "zebra".into();
    app.tick_obsidian_view(0.0);
    app.tick_obsidian_view(0.3);
    app.query = "ze".into();
    app.tick_obsidian_view(0.4);
    std::thread::sleep(Duration::from_millis(200));
    app.drain();
    assert!(app.ov.content.is_none());
}

#[test]
fn the_content_scan_skips_cloud_placeholders_through_the_injected_predicate() {
    let vault = ov_vault(&[("Local.md", "a zebra here"), ("Cloud.md", "a zebra there"), ("Plain.md", "nothing")]);
    let entries = obsidian::index_vault(&vault);
    let none = |_: &std::path::Path| false;
    let mut all = services::content_scan(&vault, &entries, "zebra", 30, &none);
    all.sort();
    assert_eq!(all, vec!["Cloud.md", "Local.md"]);
    let cloud = |p: &std::path::Path| p.file_name().is_some_and(|n| n == "Cloud.md");
    assert_eq!(services::content_scan(&vault, &entries, "zebra", 30, &cloud), vec!["Local.md"]);
    // the real predicate: ordinary files are not placeholders, a missing file is not one either
    assert!(!services::is_dataless(&vault.join("Local.md")));
    assert!(!services::is_dataless(&vault.join("nope.md")));
}

#[test]
fn enter_opens_the_exact_obsidian_uri_for_unicode_and_spaces() {
    let vault = ov_vault(&[("Café/Ünï note.md", "# Ünï note\n")]);
    let mut app = ov_ready(&vault);
    let item = app.sections().remove(0).items.remove(0);
    assert_eq!(app.actions_for(&item)[0].title, "Open in Obsidian");
    app.run_item(&item);
    assert_eq!(notion_io::take_opened(), vec!["obsidian://open?vault=My%20Vault&file=Caf%C3%A9%2F%C3%9Cn%C3%AF%20note.md".to_string()]);
    assert_eq!(notion_io::take_opened().len(), 0);
}

#[test]
fn obsidian_not_installed_toasts_and_opens_nothing_while_unknown_lets_the_os_answer() {
    let vault = ov_vault(&[("Alpha.md", "x")]);
    let mut app = ov_ready(&vault);
    let item = app.sections().remove(0).items.remove(0);
    app.obsidian.installed = Some(false);
    app.run_item(&item);
    assert_eq!(toast_text(&app), "Obsidian is not installed. Download it from obsidian.md.");
    assert!(notion_io::take_opened().is_empty());
    assert!(matches!(app.view(), View::ObsidianSearch), "the launcher stays where it was");
    app.obsidian.installed = None;
    app.run_item(&item);
    assert_eq!(notion_io::take_opened(), vec!["obsidian://open?vault=My%20Vault&file=Alpha.md".to_string()]);
    app.obsidian.installed = Some(true);
    app.run_item(&item);
    assert_eq!(notion_io::take_opened().len(), 1);
}

#[test]
fn the_actions_panel_lists_open_reveal_wikilink_path_quick_look_and_search() {
    let vault = ov_vault(&[("Alpha.md", "x")]);
    let app = ov_ready(&vault);
    let item = app.sections().remove(0).items.remove(0);
    let labels: Vec<String> = app.actions_for(&item).into_iter().map(|a| a.title.to_string()).collect();
    assert_eq!(labels, ["Open in Obsidian", "Reveal in Finder", "Copy Wikilink", "Copy Path", "Quick Look", "Search in Obsidian"]);
}

#[test]
fn copy_wikilink_and_copy_path_put_the_exact_text_on_the_clipboard() {
    let vault = ov_vault(&[("Plain.md", "x"), ("Projects/Deep/Heading note.md", "# A different title\n")]);
    let mut app = ov_ready(&vault);
    let items: Vec<Item> = app.sections().into_iter().flat_map(|s| s.items).collect();
    let by = |t: &str| items.iter().find(|i| i.title == t).unwrap().clone();
    let act = |app: &App, item: &Item, label: &str| app.actions_for(item).into_iter().find(|a| a.title == label).unwrap().kind;
    // title equals the file stem
    let plain = by("Plain");
    let a = act(&app, &plain, "Copy Wikilink");
    ov_perform(&mut app, &plain, a);
    assert_eq!(notion_io::take_copied(), vec!["[[Plain]]".to_string()]);
    // a note in a subfolder whose title differs: the link must still resolve by file name
    let deep = by("A different title");
    let a = act(&app, &deep, "Copy Wikilink");
    ov_perform(&mut app, &deep, a);
    assert_eq!(notion_io::take_copied(), vec!["[[Heading note|A different title]]".to_string()]);
    let a = act(&app, &deep, "Copy Path");
    ov_perform(&mut app, &deep, a);
    assert_eq!(notion_io::take_copied(), vec![vault.join("Projects/Deep/Heading note.md").to_string_lossy().into_owned()]);
}

#[test]
fn search_in_obsidian_opens_the_search_uri_with_the_typed_query() {
    let vault = ov_vault(&[("Alpha.md", "x")]);
    let mut app = ov_ready(&vault);
    let item = app.sections().remove(0).items.remove(0);
    app.query = "Alpha plan/2".into();
    let a = app.actions_for(&item).into_iter().find(|a| a.title == "Search in Obsidian").unwrap().kind;
    ov_perform(&mut app, &item, a);
    assert_eq!(notion_io::take_opened(), vec![obsidian::uri_search("My Vault", "Alpha plan/2")]);
    assert_eq!(notion_io::take_opened().len(), 0);
}

#[test]
fn a_uri_over_8000_characters_is_refused_with_a_toast_instead_of_truncated() {
    let vault = ov_vault(&[("Alpha.md", "x")]);
    let mut app = ov_ready(&vault);
    let item = app.sections().remove(0).items.remove(0);
    app.query = "x".repeat(9000);
    let q = app.query.clone();
    ov_perform(&mut app, &item, Act::ObsidianSearch(q));
    assert_eq!(toast_text(&app), "That link is too long for Obsidian (over 8,000 characters).");
    assert!(notion_io::take_opened().is_empty());
    // and a long note path through Enter
    let long = Item { kind: Kind::ObsidianNote("a".repeat(9000) + ".md"), ..item.clone() };
    app.run_item(&long);
    assert_eq!(toast_text(&app), "That link is too long for Obsidian (over 8,000 characters).");
    assert!(notion_io::take_opened().is_empty());
}

#[test]
fn command_y_toggles_the_preview_pane() {
    let vault = ov_vault(&[("Alpha.md", "# Alpha\n")]);
    let mut app = ov_ready(&vault);
    assert!(!app.ov.hide_preview, "the pane is shown by default");
    render_mods(&mut app, vec![key_mod_event(Key::Y, Modifiers::COMMAND)], Modifiers::COMMAND);
    assert!(app.ov.hide_preview);
    render_mods(&mut app, vec![key_mod_event(Key::Y, Modifiers::COMMAND)], Modifiers::COMMAND);
    assert!(!app.ov.hide_preview);
}

#[test]
fn the_preview_is_read_on_a_thread_after_a_short_pause_and_matches_parse_preview() {
    let body = "---\ntitle: Hidden\n---\n# Heading\n\n- bullet\n- [ ] todo\n> quote\n";
    let vault = ov_vault(&[("Alpha.md", body)]);
    let mut app = ov_ready(&vault);
    app.tick_obsidian_view(0.0);
    assert_eq!(app.ov.preview_request, 0, "the selection must rest before the file is read");
    app.tick_obsidian_view(0.2);
    assert_eq!(app.ov.preview_request, 1);
    assert!(app.ov.preview.is_none(), "the answer comes through the Bus");
    drain_until(&mut app, |a| a.ov.preview.is_some());
    let (rel, result) = app.ov.preview.clone().unwrap();
    assert_eq!(rel, "Alpha.md");
    assert_eq!(result, Ok(obsidian::parse_preview(body)));
    // no new read while the selection stays
    app.tick_obsidian_view(5.0);
    assert_eq!(app.ov.preview_request, 1);
}

#[test]
fn the_preview_reads_nothing_while_the_pane_is_hidden() {
    let vault = ov_vault(&[("Alpha.md", "x")]);
    let mut app = ov_ready(&vault);
    app.ov.hide_preview = true;
    app.tick_obsidian_view(0.0);
    app.tick_obsidian_view(1.0);
    assert_eq!(app.ov.preview_request, 0);
}

#[test]
fn a_preview_answer_for_an_older_selection_is_dropped() {
    let vault = ov_vault(&[("A.md", "# A\n"), ("B.md", "# B\n")]);
    ov_set_mtime(&vault, "A.md", 10);
    ov_set_mtime(&vault, "B.md", 1000);
    let mut app = ov_ready(&vault);
    app.tick_obsidian_view(0.0);
    app.tick_obsidian_view(0.2);
    let first = app.ov.preview_request;
    app.selected = 1;
    app.tick_obsidian_view(1.0);
    app.tick_obsidian_view(1.2);
    assert_eq!(app.ov.preview_request, first + 1);
    app.bus.send(Msg::ObsidianPreview { request: first, result: Ok(vec![Block::Paragraph("stale".into())]) });
    app.drain();
    assert!(!matches!(&app.ov.preview, Some((_, Ok(b))) if b == &vec![Block::Paragraph("stale".into())]), "the old answer must not show");
    drain_until(&mut app, |a| a.ov.preview.is_some());
    assert_eq!(app.ov.preview.clone().unwrap().0, "B.md");
}

#[test]
fn an_unreadable_note_shows_could_not_read_this_note_in_the_pane() {
    let vault = ov_vault(&[("Alpha.md", "# Alpha\n")]);
    let mut app = ov_ready(&vault);
    std::fs::remove_file(vault.join("Alpha.md")).unwrap();
    app.tick_obsidian_view(0.0);
    app.tick_obsidian_view(0.2);
    drain_until(&mut app, |a| a.ov.preview.is_some());
    assert_eq!(app.ov.preview.clone().unwrap().1, Err(()));
    let text = painted(&mut app);
    assert!(text.contains("Could not read this note."), "{text}");
}

#[test]
fn every_preview_block_renders_without_panicking_and_the_title_sits_above() {
    let vault = ov_vault(&[("Alpha.md", "x")]);
    let mut app = ov_ready(&vault);
    let blocks = vec![
        Block::Heading(1, "Heading one".into()),
        Block::Heading(6, "Heading six".into()),
        Block::Paragraph("A plain paragraph".into()),
        Block::Bullet(0, "bullet zero".into()),
        Block::Bullet(3, "bullet deep".into()),
        Block::Ordered(1, 12, "ordered twelve".into()),
        Block::Task(0, false, "open task".into()),
        Block::Task(2, true, "done task".into()),
        Block::Quote("a quote".into()),
        Block::Code(Some("rust".into()), "fn main() {}".into()),
        Block::Code(None, "".into()),
        Block::Rule,
    ];
    app.ov.preview_for = Some("Alpha.md".into());
    app.ov.preview = Some(("Alpha.md".into(), Ok(blocks)));
    render(&mut app, vec![]);
    let text = painted(&mut app);
    for want in [
        "Alpha",
        "Heading one",
        "Heading six",
        "A plain paragraph",
        "bullet zero",
        "bullet deep",
        "ordered twelve",
        "open task",
        "done task",
        "a quote",
        "fn main() {}",
    ] {
        assert!(text.contains(want), "{want} is drawn: {text}");
    }
    // the title comes before the first block
    assert!(text.find("Alpha").unwrap() < text.find("Heading one").unwrap());
    // hidden pane: the blocks are not drawn
    app.ov.hide_preview = true;
    let text = painted(&mut app);
    assert!(!text.contains("Heading one"), "{text}");
}

// ---------------------------------------------------------------- New Obsidian Note / Append to Daily Note (T9)

fn oc_app(vault: Option<&std::path::Path>) -> App {
    let _ = notion_io::take_opened();
    notion_io::set_test_clipboard(None);
    let mut app = fixture();
    app.settings.obsidian_vault = vault.map(|p| p.to_path_buf());
    app.obsidian.installed = Some(true);
    app
}

fn oc_vault_with_plugins(json: Option<&str>) -> PathBuf {
    let v = ov_vault(&[("Alpha.md", "x")]);
    if let Some(j) = json {
        std::fs::create_dir_all(v.join(".obsidian")).unwrap();
        std::fs::write(v.join(".obsidian/core-plugins.json"), j).unwrap();
    }
    v
}

fn oc_new_form(app: &mut App, title: &str, text: &str) {
    app.run_builtin(Builtin::NewObsidianNote);
    match app.stack.last_mut().unwrap() {
        View::ObsidianNew { title: t, text: x } => {
            *t = title.into();
            *x = text.into();
        }
        _ => panic!("not on the New Obsidian Note form"),
    }
}

fn oc_new_fields(app: &App) -> (String, String) {
    match app.view() {
        View::ObsidianNew { title, text } => (title.clone(), text.clone()),
        _ => panic!("not on the New Obsidian Note form"),
    }
}

fn oc_daily_form(app: &mut App, text: &str) {
    app.run_builtin(Builtin::AppendToDailyNote);
    match app.stack.last_mut().unwrap() {
        View::ObsidianDaily { text: x } => *x = text.into(),
        _ => panic!("not on the Append to Daily Note form"),
    }
}

fn oc_daily_text(app: &App) -> String {
    match app.view() {
        View::ObsidianDaily { text } => text.clone(),
        _ => panic!("not on the Append to Daily Note form"),
    }
}

fn oc_send_new(app: &mut App) {
    app.obsidian_new_submit();
    drain_until(app, |a| !a.oc.sending);
}

fn oc_send_daily(app: &mut App) {
    app.obsidian_daily_submit();
    drain_until(app, |a| !a.oc.sending);
}

fn settings_tab_of(app: &App) -> Option<usize> {
    app.ctx.data(|d| d.get_temp(Id::new("settings_tab")))
}

fn clock(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> chrono::NaiveDateTime {
    chrono::NaiveDate::from_ymd_opt(y, mo, d).unwrap().and_hms_opt(h, mi, 7).unwrap()
}

#[test]
fn the_new_command_builds_the_exact_uri_with_unicode_spaces_amp_hash_and_newlines() {
    let vault = oc_vault_with_plugins(None);
    let mut app = oc_app(Some(&vault));
    oc_new_form(&mut app, "Café plan", "a & b #tag\nline 2 é");
    oc_send_new(&mut app);
    assert_eq!(
        notion_io::take_opened(),
        vec!["obsidian://new?vault=My%20Vault&name=Caf%C3%A9%20plan&content=a%20%26%20b%20%23tag%0Aline%202%20%C3%A9".to_string()],
        "no silent flag: Obsidian opens the new note"
    );
    assert_eq!(toast_text(&app), "Sent to Obsidian");
    assert_eq!(oc_new_fields(&app), (String::new(), String::new()), "the form is cleared after a send");
}

#[test]
fn titles_are_trimmed_cleaned_and_fall_back_to_the_first_line_then_the_clock() {
    let now = clock(2026, 3, 9, 14, 5);
    assert_eq!(services::note_title("  Plan  ", "body", now), "Plan");
    assert_eq!(services::note_title("a/b\\c:d*e?f\"g<h>i|j", "", now), "a b c d e f g h i j");
    assert_eq!(services::note_title("x #y ^z [w] ]", "", now), "x y z w");
    assert_eq!(services::note_title("a  /  b", "", now), "a b", "runs of spaces collapse");
    assert_eq!(services::note_title("", "\n\n  First line here \nsecond", now), "First line here");
    assert_eq!(services::note_title("   ", "A/B", now), "A B");
    assert_eq!(services::note_title("", "", now), "2026-03-09 14.05");
    assert_eq!(services::note_title("", "  \n ", now), "2026-03-09 14.05");
    assert_eq!(services::note_title("///", "body", now), "2026-03-09 14.05", "empty after cleaning: the date-time");
    assert_eq!(services::note_title("", "#^[]", now), "2026-03-09 14.05");
}

#[test]
fn the_first_line_fallback_is_cut_at_80_characters_on_a_character_boundary() {
    let now = clock(2026, 1, 2, 3, 4);
    let line = "é日".repeat(60); // 120 chars
    let t = services::note_title("", &format!("{line}\nrest"), now);
    assert_eq!(t.chars().count(), 80);
    assert!(line.starts_with(&t));
    // an explicit title is not cut
    assert_eq!(services::note_title(&line, "", now).chars().count(), 120);
}

#[test]
fn an_empty_title_in_the_form_uses_the_first_line_and_with_nothing_the_date_time_shape() {
    let vault = oc_vault_with_plugins(None);
    let mut app = oc_app(Some(&vault));
    oc_new_form(&mut app, "", "Meeting: notes\nmore");
    oc_send_new(&mut app);
    let u = notion_io::take_opened().remove(0);
    assert!(u.starts_with("obsidian://new?vault=My%20Vault&name=Meeting%20notes&content="), "{u}");
    oc_new_form(&mut app, "", "");
    app.obsidian_new_submit();
    assert_eq!(toast_text(&app), "Write something to send first.");
    assert!(notion_io::take_opened().is_empty(), "nothing at all to send");
    // text of only a character that cleans away: the date-time (yyyy-MM-dd HH.mm)
    oc_new_form(&mut app, "", "***");
    oc_send_new(&mut app);
    let u = notion_io::take_opened().remove(0);
    let name = u.split("&name=").nth(1).unwrap().split('&').next().unwrap().replace("%20", " ");
    assert!(chrono::NaiveDateTime::parse_from_str(&format!("{name}.00"), "%Y-%m-%d %H.%M.%S").is_ok(), "{name}");
}

#[test]
fn the_daily_command_appends_silently_and_adds_the_missing_trailing_newline() {
    let vault = oc_vault_with_plugins(None);
    let mut app = oc_app(Some(&vault));
    oc_daily_form(&mut app, "buy milk & eggs");
    oc_send_daily(&mut app);
    assert_eq!(
        notion_io::take_opened(),
        vec!["obsidian://daily?vault=My%20Vault&content=buy%20milk%20%26%20eggs%0A&append=true&silent=true".to_string()]
    );
    assert_eq!(toast_text(&app), "Added to today's daily note");
    assert_eq!(oc_daily_text(&app), "", "cleared after the send");
    // text that already ends in a newline is sent as is
    oc_daily_form(&mut app, "line\n");
    oc_send_daily(&mut app);
    assert_eq!(notion_io::take_opened(), vec![obsidian::uri_daily("My Vault", "line\n", true, true)]);
    assert_eq!(services::ensure_newline("a"), "a\n");
    assert_eq!(services::ensure_newline("a\n"), "a\n");
    assert_eq!(services::ensure_newline("a\r\n"), "a\r\n");
}

#[test]
fn the_daily_form_refuses_empty_text() {
    let vault = oc_vault_with_plugins(None);
    let mut app = oc_app(Some(&vault));
    oc_daily_form(&mut app, " \n ");
    app.obsidian_daily_submit();
    assert_eq!(toast_text(&app), "Write something to send first.");
    assert!(notion_io::take_opened().is_empty());
}

#[test]
fn the_clipboard_command_sends_the_clipboard_text_with_no_form() {
    let vault = oc_vault_with_plugins(None);
    let mut app = oc_app(Some(&vault));
    notion_io::set_test_clipboard(Some("copied é"));
    app.run_builtin(Builtin::AddClipboardToDailyNote);
    assert!(matches!(app.view(), View::Root), "no form");
    drain_until(&mut app, |a| !a.oc.sending);
    assert_eq!(notion_io::take_opened(), vec![obsidian::uri_daily("My Vault", "copied é\n", true, true)]);
    assert_eq!(toast_text(&app), "Added to today's daily note");
}

#[test]
fn an_empty_clipboard_toasts_and_opens_nothing() {
    let vault = oc_vault_with_plugins(None);
    for clip in [None, Some(""), Some(" \n ")] {
        let mut app = oc_app(Some(&vault));
        notion_io::set_test_clipboard(clip);
        app.run_builtin(Builtin::AddClipboardToDailyNote);
        assert_eq!(toast_text(&app), "The clipboard has no text.");
        std::thread::sleep(Duration::from_millis(50));
        app.drain();
        assert!(notion_io::take_opened().is_empty());
    }
}

const NO_VAULT: &str = "Choose your Obsidian vault in Settings → Integrations.";
const GONE_VAULT: &str = "The vault folder was not found. Choose it again in Settings → Integrations.";
const NOT_INSTALLED: &str = "Obsidian is not installed. Download it from obsidian.md.";
const PLUGIN_OFF: &str = "Turn on the Daily notes plugin in Obsidian (Settings → Core plugins).";

#[test]
fn no_vault_toasts_opens_the_integrations_tab_and_opens_nothing_for_all_three_commands() {
    for b in [Builtin::NewObsidianNote, Builtin::AppendToDailyNote, Builtin::AddClipboardToDailyNote] {
        let mut app = oc_app(None);
        notion_io::set_test_clipboard(Some("text"));
        app.run_builtin(b);
        std::thread::sleep(Duration::from_millis(50));
        app.drain();
        assert_eq!(toast_text(&app), NO_VAULT, "{b:?}");
        assert!(matches!(app.view(), View::Settings), "{b:?}");
        assert_eq!(settings_tab_of(&app), Some(draw::INTEGRATIONS_TAB));
        assert!(notion_io::take_opened().is_empty());
    }
}

#[test]
fn a_vault_folder_gone_from_disk_toasts_jumps_to_settings_keeps_the_form_and_opens_nothing() {
    let gone = std::env::temp_dir().join(format!("ferrocast-oc-gone-{}", uuid::Uuid::new_v4()));
    let mut app = oc_app(Some(&gone));
    oc_new_form(&mut app, "T", "typed");
    oc_send_new(&mut app);
    assert_eq!(toast_text(&app), GONE_VAULT);
    assert!(matches!(app.view(), View::Settings));
    assert_eq!(settings_tab_of(&app), Some(draw::INTEGRATIONS_TAB));
    app.pop();
    assert_eq!(oc_new_fields(&app), ("T".into(), "typed".into()), "the typed text is kept");
    app.pop();
    // the daily form and the clipboard command check it too
    oc_daily_form(&mut app, "typed");
    oc_send_daily(&mut app);
    assert_eq!(toast_text(&app), GONE_VAULT);
    let mut app = oc_app(Some(&gone));
    notion_io::set_test_clipboard(Some("c"));
    app.run_builtin(Builtin::AddClipboardToDailyNote);
    drain_until(&mut app, |a| !a.oc.sending);
    assert_eq!(toast_text(&app), GONE_VAULT);
    assert!(matches!(app.view(), View::Settings));
    assert!(notion_io::take_opened().is_empty());
}

#[test]
fn obsidian_known_missing_toasts_and_opens_nothing_while_unknown_proceeds() {
    let vault = oc_vault_with_plugins(None);
    let mut app = oc_app(Some(&vault));
    app.obsidian.installed = Some(false);
    oc_new_form(&mut app, "T", "typed");
    oc_send_new(&mut app);
    assert_eq!(toast_text(&app), NOT_INSTALLED);
    assert!(notion_io::take_opened().is_empty());
    assert!(matches!(app.view(), View::ObsidianNew { .. }), "stays on the form");
    assert_eq!(oc_new_fields(&app).1, "typed");
    app.pop();
    oc_daily_form(&mut app, "x");
    oc_send_daily(&mut app);
    assert_eq!(toast_text(&app), NOT_INSTALLED);
    assert!(notion_io::take_opened().is_empty());
    app.obsidian.installed = None;
    oc_send_daily(&mut app);
    assert_eq!(notion_io::take_opened().len(), 1, "not scanned yet: let the system answer");
}

#[test]
fn the_precondition_order_is_vault_then_folder_then_installed_then_plugin() {
    let gone = std::env::temp_dir().join(format!("ferrocast-oc-gone-{}", uuid::Uuid::new_v4()));
    let mut app = oc_app(Some(&gone));
    app.obsidian.installed = Some(false);
    oc_new_form(&mut app, "T", "x");
    oc_send_new(&mut app);
    assert_eq!(toast_text(&app), GONE_VAULT, "folder before installed");
    let vault = oc_vault_with_plugins(Some(r#"["file-explorer"]"#));
    let mut app = oc_app(Some(&vault));
    app.obsidian.installed = Some(false);
    oc_daily_form(&mut app, "x");
    oc_send_daily(&mut app);
    assert_eq!(toast_text(&app), NOT_INSTALLED, "installed before the plugin");
}

#[test]
fn the_daily_plugin_is_read_from_core_plugins_in_array_and_object_forms() {
    let off_array = oc_vault_with_plugins(Some(r#"["file-explorer","graph"]"#));
    let on_array = oc_vault_with_plugins(Some(r#"["file-explorer","daily-notes"]"#));
    let off_obj = oc_vault_with_plugins(Some(r#"{"file-explorer":true,"daily-notes":false}"#));
    let absent_obj = oc_vault_with_plugins(Some(r#"{"file-explorer":true}"#));
    let on_obj = oc_vault_with_plugins(Some(r#"{"daily-notes":true}"#));
    assert_eq!(services::daily_plugin_enabled(&off_array), Some(false));
    assert_eq!(services::daily_plugin_enabled(&on_array), Some(true));
    assert_eq!(services::daily_plugin_enabled(&off_obj), Some(false));
    assert_eq!(services::daily_plugin_enabled(&absent_obj), Some(false));
    assert_eq!(services::daily_plugin_enabled(&on_obj), Some(true));
    // missing, unreadable (a directory in its place) and malformed: unknown, so Obsidian answers
    assert_eq!(services::daily_plugin_enabled(&oc_vault_with_plugins(None)), None);
    assert_eq!(services::daily_plugin_enabled(&oc_vault_with_plugins(Some("not json"))), None);
    assert_eq!(services::daily_plugin_enabled(&oc_vault_with_plugins(Some("42"))), None);
    let dir = oc_vault_with_plugins(None);
    std::fs::create_dir_all(dir.join(".obsidian/core-plugins.json")).unwrap();
    assert_eq!(services::daily_plugin_enabled(&dir), None);
}

#[test]
fn the_daily_plugin_off_toasts_and_opens_nothing_but_the_new_note_does_not_care() {
    let vault = oc_vault_with_plugins(Some(r#"["file-explorer"]"#));
    let mut app = oc_app(Some(&vault));
    oc_daily_form(&mut app, "typed");
    oc_send_daily(&mut app);
    assert_eq!(toast_text(&app), PLUGIN_OFF);
    assert!(notion_io::take_opened().is_empty());
    assert_eq!(oc_daily_text(&app), "typed", "kept");
    let mut app = oc_app(Some(&vault));
    notion_io::set_test_clipboard(Some("clip"));
    app.run_builtin(Builtin::AddClipboardToDailyNote);
    drain_until(&mut app, |a| !a.oc.sending);
    assert_eq!(toast_text(&app), PLUGIN_OFF);
    assert!(notion_io::take_opened().is_empty());
    let mut app = oc_app(Some(&vault));
    oc_new_form(&mut app, "T", "x");
    oc_send_new(&mut app);
    assert_eq!(notion_io::take_opened().len(), 1);
}

#[test]
fn a_missing_or_unreadable_plugin_file_lets_the_daily_note_through() {
    let vault = oc_vault_with_plugins(None);
    let mut app = oc_app(Some(&vault));
    oc_daily_form(&mut app, "x");
    oc_send_daily(&mut app);
    assert_eq!(notion_io::take_opened().len(), 1);
    let on = oc_vault_with_plugins(Some(r#"{"daily-notes":true}"#));
    let mut app = oc_app(Some(&on));
    oc_daily_form(&mut app, "x");
    oc_send_daily(&mut app);
    assert_eq!(notion_io::take_opened().len(), 1);
}

#[test]
fn a_uri_over_8000_characters_is_refused_with_the_form_kept_and_never_truncated() {
    let vault = oc_vault_with_plugins(None);
    let mut app = oc_app(Some(&vault));
    let big = "é".repeat(2_000); // 12,000 encoded characters
    oc_new_form(&mut app, "T", &big);
    oc_send_new(&mut app);
    let msg = "That is too long to send through Obsidian. Shorten it or paste it in Obsidian directly.";
    assert_eq!(toast_text(&app), msg);
    assert!(notion_io::take_opened().is_empty());
    assert_eq!(oc_new_fields(&app).1, big);
    app.pop();
    oc_daily_form(&mut app, &big);
    oc_send_daily(&mut app);
    assert_eq!(toast_text(&app), msg);
    assert_eq!(oc_daily_text(&app), big);
    // clipboard too
    notion_io::set_test_clipboard(Some(&big));
    app.run_builtin(Builtin::AddClipboardToDailyNote);
    drain_until(&mut app, |a| !a.oc.sending);
    assert_eq!(toast_text(&app), msg);
    assert!(notion_io::take_opened().is_empty());
    // just under the limit goes through
    let ok = "a".repeat(7_900);
    oc_daily_form(&mut app, &ok);
    oc_send_daily(&mut app);
    assert_eq!(notion_io::take_opened().len(), 1);
}

#[test]
fn the_form_commands_with_no_vault_do_not_open_a_form_and_the_commands_are_listed() {
    let mut app = oc_app(None);
    app.run_builtin(Builtin::NewObsidianNote);
    assert!(matches!(app.view(), View::Settings));
    assert_eq!(Builtin::NewObsidianNote.title(), "New Obsidian Note");
    assert_eq!(Builtin::AppendToDailyNote.title(), "Append to Daily Note");
    assert_eq!(Builtin::AddClipboardToDailyNote.title(), "Add Clipboard to Daily Note");
    for b in [Builtin::NewObsidianNote, Builtin::AppendToDailyNote, Builtin::AddClipboardToDailyNote] {
        assert!(Builtin::ALL.contains(&b));
        assert!(b.keywords().contains(&"obsidian".to_string()));
    }
    assert!(Builtin::NewObsidianNote.keywords().iter().any(|k| k == "capture"));
    assert!(Builtin::AppendToDailyNote.keywords().iter().any(|k| k == "journal"));
}

#[test]
fn the_forms_paint_and_send_with_cmd_enter() {
    let vault = oc_vault_with_plugins(None);
    let mut app = oc_app(Some(&vault));
    oc_new_form(&mut app, "Hi", "body");
    let text = painted(&mut app);
    assert!(text.contains("Title") && text.contains("Text") && text.contains("Send"), "{text}");
    assert_eq!(app.view().footer_hints().first().copied(), Some(("Send", "⌘↵")));
    app.pop();
    oc_daily_form(&mut app, "x");
    let text = painted(&mut app);
    assert!(!text.contains("Title"), "the daily form has only the text: {text}");
}

fn capture_ids(app: &App) -> Vec<String> {
    app.base.iter().filter(|i| i.id.starts_with("builtin:capture-")).map(|i| i.id.clone()).collect()
}

#[test]
fn capture_commands_are_listed_only_when_capture_is_available() {
    use crate::capture_ctl::{Availability, CaptureState};
    let mut app = fixture();
    app.rebuild();
    assert_eq!(
        capture_ids(&app),
        ["builtin:capture-area", "builtin:capture-window", "builtin:capture-screen", "builtin:capture-scrolling"]
    );
    let item = app.base.iter().find(|i| i.id == "builtin:capture-area").unwrap();
    assert_eq!((item.title.as_str(), item.subtitle.as_str(), item.is_command()), ("Capture Area", "Capture", true));
    let item = app.base.iter().find(|i| i.id == "builtin:capture-scrolling").unwrap();
    assert_eq!((item.title.as_str(), item.subtitle.as_str(), item.is_command()), ("Capture Scrolling", "Capture", true));
    for availability in [Availability::NeedsMacOs14(13), Availability::NoHelper] {
        app.capture = CaptureState::with(std::path::PathBuf::new(), availability);
        app.rebuild();
        assert!(capture_ids(&app).is_empty(), "{availability:?}");
    }
}

#[test]
fn stored_capture_shortcut_on_macos_13_toasts_once_per_session() {
    use crate::capture_ctl::{Availability, CaptureState};
    let mut app = fixture();
    app.capture = CaptureState::with(std::path::PathBuf::new(), Availability::NeedsMacOs14(13));
    app.rebuild();
    app.visible = true;
    assert!(!app.capture_gate());
    assert_eq!(app.toast.as_ref().map(|t| t.0.as_str()), Some("Screen capture needs macOS 14. This Mac runs macOS 13."));
    app.toast = None;
    assert!(!app.capture_gate());
    assert!(app.toast.is_none(), "second press stays quiet");
    app.capture = CaptureState::with(std::path::PathBuf::new(), Availability::NoHelper);
    assert!(!app.capture_gate());
    assert_eq!(app.toast.as_ref().map(|t| t.0.as_str()), Some("Screen capture is not available in this build."));
}

#[test]
fn capture_permission_denied_shows_the_exact_toast_and_cancel_is_silent() {
    use crate::capture_ctl::Outcome;
    let mut app = fixture();
    app.visible = true;
    app.finish_capture(Outcome::Cancelled);
    assert!(app.toast.is_none(), "Esc: no toast");
    app.finish_capture(Outcome::PermissionDenied);
    assert_eq!(
        app.toast.as_ref().map(|t| t.0.as_str()),
        Some("Screen Recording is off for Ferrocast. Turn it on in System Settings → Privacy & Security → Screen & System Audio Recording, then try again.")
    );
}

#[test]
fn capture_saved_with_save_only_does_nothing_more() {
    use crate::capture_ctl::Outcome;
    let mut app = fixture();
    app.visible = true;
    app.settings.capture.after = ferrocast_core::capture::settings::AfterCapture::Save;
    app.finish_capture(Outcome::Saved(std::path::PathBuf::from("/nonexistent/Capture.png")));
    assert!(app.toast.is_none());
}

// ---- T8: the Capture window (S7) ----

fn capture_png(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("fc-capture-result-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    let f = d.join("Capture 2026-10-23 at 14.05.22.png");
    image::RgbaImage::from_pixel(120, 80, image::Rgba([20, 120, 220, 255])).save(&f).unwrap();
    f
}

fn capture_app(file: &std::path::Path) -> App {
    let mut app = fixture();
    app.stack = vec![View::Root, View::CaptureResult { file: file.to_path_buf(), size: (120, 80) }];
    app
}

fn capture_buttons_drawn(app: &mut App) -> Vec<(Pos2, bool)> {
    app.ctx.data_mut(|d| d.remove::<Vec<(String, Id, Rect, bool)>>(Id::new("ctl-hits")));
    app.ctx.data_mut(|d| d.remove::<Vec<String>>(Id::new("body-notes")));
    render(app, vec![]);
    let hits: Vec<(String, Id, Rect, bool)> = app.ctx.data(|d| d.get_temp(Id::new("ctl-hits"))).unwrap_or_default();
    hits.into_iter().filter(|h| h.0 == "small_btn").map(|h| (h.2.center(), h.3)).collect()
}

#[test]
fn capture_window_renders_six_enabled_buttons_and_the_shortcuts() {
    let f = capture_png("render");
    let mut app = capture_app(&f);
    let btns = capture_buttons_drawn(&mut app);
    assert_eq!(btns.len(), 6, "Copy, Save, Annotate…, Background…, Copy Text, Pin");
    assert!(btns.iter().all(|b| b.1), "all enabled while the file exists");
    let notes: Vec<String> = app.ctx.data(|d| d.get_temp(Id::new("body-notes"))).unwrap_or_default();
    let line = notes.join(" ");
    for hint in ["↵", "⌘S", "⌘⇧A", "⌘⇧B", "⌘T", "⌘P"] {
        assert!(line.contains(hint), "{hint} missing from {line:?}");
    }
    assert!(
        app.ctx.data(|d| d.get_temp::<std::sync::Arc<(std::path::PathBuf, egui::TextureHandle)>>(Id::new("capture-tex"))).is_some(),
        "preview drawn"
    );
    let chips = app.view().footer_hints();
    assert!(chips.contains(&("Back", "esc")) && chips.iter().any(|c| c.1 == "↵"), "{chips:?}");
    assert_eq!(app.view().title(), "Capture");
}

#[test]
fn capture_window_buttons_are_disabled_when_the_file_is_gone() {
    let mut app = capture_app(std::path::Path::new("/nonexistent/capture.png"));
    let btns = capture_buttons_drawn(&mut app);
    assert_eq!(btns.len(), 6);
    assert!(btns.iter().all(|b| !b.1));
    app.toast = None;
    app.capture_result_run(crate::capture_ctl::ResultButton::Copy);
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("no longer exists")), "{:?}", app.toast);
    assert!(matches!(app.view(), View::CaptureResult { .. }));
}

#[test]
fn capture_window_annotate_and_background_open_the_existing_editors_on_the_saved_file() {
    let f = capture_png("editors");
    let mut app = capture_app(&f);
    let btns = capture_buttons_drawn(&mut app);
    click_at(&mut app, btns[2].0);
    assert!(matches!(app.view(), View::Annotate { file, .. } if *file == f), "Annotate… acts on the saved path");
    let mut app = capture_app(&f);
    let btns = capture_buttons_drawn(&mut app);
    click_at(&mut app, btns[3].0);
    assert!(matches!(app.view(), View::FrameImage { file, .. } if *file == f), "Background… acts on the saved path");
    let mut app = capture_app(&f);
    render(&mut app, vec![key_mod_event(Key::A, Modifiers { shift: true, ..Modifiers::COMMAND })]);
    assert!(matches!(app.view(), View::Annotate { file, .. } if *file == f), "⌘⇧A");
    let mut app = capture_app(&f);
    render(&mut app, vec![key_mod_event(Key::B, Modifiers { shift: true, ..Modifiers::COMMAND })]);
    assert!(matches!(app.view(), View::FrameImage { file, .. } if *file == f), "⌘⇧B");
}

#[test]
fn capture_window_copy_text_runs_ocr_off_thread_and_closes_the_window() {
    let f = capture_png("ocr");
    for via_key in [false, true] {
        let mut app = capture_app(&f);
        if via_key {
            render(&mut app, vec![key_mod_event(Key::T, Modifiers::COMMAND)]);
        } else {
            let btns = capture_buttons_drawn(&mut app);
            click_at(&mut app, btns[4].0);
        }
        assert!(app.toast.as_ref().is_none_or(|t| t.0 != "Not available yet"), "{:?}", app.toast);
        assert!(matches!(app.view(), View::Root), "the window closes while the text is read");
        // the blank fixture holds no text (or Vision refuses here): either way the clipboard is left alone
        // Other messages (clipboard polling, notices) may arrive first under load: skip them until OcrDone.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(90);
        let mut seen = vec![];
        loop {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            match app.rx.recv_timeout(left) {
                Ok(Msg::OcrDone(text, toast)) => {
                    assert!(text.is_none(), "{text:?}");
                    assert!(!toast.is_empty());
                    break;
                }
                Ok(_) => seen.push("other message"),
                Err(e) => panic!("OcrDone expected, got {e:?} after {} other messages", seen.len()),
            }
        }
    }
}

#[test]
fn capture_window_pin_is_no_longer_a_stub() {
    let f = capture_png("pin");
    let mut app = capture_app(&f);
    let btns = capture_buttons_drawn(&mut app);
    click_at(&mut app, btns[5].0);
    // tests run off the main thread, so no panel can open; the old stub toast must be gone
    assert!(app.toast.as_ref().is_none_or(|t| t.0 != "Not available yet"), "{:?}", app.toast);
}

#[test]
fn capture_window_escape_closes_it() {
    let f = capture_png("esc");
    let mut app = capture_app(&f);
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::Root));
}

#[test]
fn saved_capture_with_after_show_opens_the_capture_window() {
    let f = capture_png("finish");
    let mut app = fixture();
    app.visible = true;
    assert_eq!(app.settings.capture.after, ferrocast_core::capture::settings::AfterCapture::Show);
    app.finish_capture(crate::capture_ctl::Outcome::Saved(f.clone()));
    assert!(matches!(app.view(), View::CaptureResult { file, size } if *file == f && *size == (120, 80)), "{}", app.view().title());
}

// ---- T9: Settings → Capture tab

/// Titles of the Capture tab rows, in drawing order.
const CAPTURE_TAB_TITLES: [&str; 18] = [
    "Capture folder",
    "Format",
    "After capture",
    "Include cursor",
    "Recording frame rate",
    "Recording countdown",
    "Camera",
    "Camera device",
    "Microphone",
    "Microphone device",
    "System audio",
    "Capture Area shortcut",
    "Capture Window shortcut",
    "Capture Screen shortcut",
    "Capture Scrolling shortcut",
    "Record Screen shortcut",
    "Record Area shortcut",
    "Record Window shortcut",
];

#[test]
fn capture_tab_is_its_own_group_between_translate_and_notes() {
    assert_eq!(SETTINGS_TABS.len(), 10);
    assert_eq!(SETTINGS_TABS.map(|(n, _)| n).to_vec(), TAB_NAMES.to_vec());
    assert_eq!((TRANSLATE_TAB, CAPTURE_TAB, NOTES_TAB), (4, 5, 6));
    assert_eq!(SETTINGS_GROUP_STARTS, [3, 5, 6], "Capture is a group of its own before Power");
    assert_eq!(SETTINGS_NUM_KEYS.len(), SETTINGS_TABS.len());
    assert!(!RESET_FIELDS.iter().any(|(t, _)| *t == CAPTURE_TAB), "folders and shortcuts are user data: no reset");
}

#[test]
fn capture_tab_rows_are_drawn_in_order_and_indexed_on_that_tab() {
    use ferrocast_core::settings_index::ENTRIES;
    let app = sd2_app(CAPTURE_TAB);
    let rows: Vec<String> = drawn_rows(&app).into_iter().filter(|(t, _)| *t == CAPTURE_TAB).map(|(_, n)| n).collect();
    assert_eq!(rows, CAPTURE_TAB_TITLES.map(String::from).to_vec());
    let indexed: Vec<&str> = ENTRIES.iter().filter(|e| e.tab == CAPTURE_TAB).map(|e| e.title).collect();
    assert_eq!(indexed, CAPTURE_TAB_TITLES.to_vec());
}

#[test]
fn capture_settings_are_found_by_the_settings_search() {
    use ferrocast_core::settings_index::search;
    for (q, title) in [
        ("screenshot folder", "Capture folder"),
        ("png", "Format"),
        ("after capture", "After capture"),
        ("clipboard", "After capture"),
        ("pointer", "Include cursor"),
        ("fps", "Recording frame rate"),
        ("countdown", "Recording countdown"),
        ("webcam", "Camera"),
        ("facecam", "Camera device"),
        ("narration", "Microphone"),
        ("mic", "Microphone device"),
        ("computer audio", "System audio"),
        ("capture area", "Capture Area shortcut"),
        ("window shortcut", "Capture Window shortcut"),
        ("screenshot hotkey", "Capture Screen shortcut"),
        ("long screenshot", "Capture Scrolling shortcut"),
        ("record screen", "Record Screen shortcut"),
        ("record area", "Record Area shortcut"),
        ("video hotkey", "Record Window shortcut"),
    ] {
        assert!(search(q).iter().any(|e| e.tab == CAPTURE_TAB && e.title == title), "{q:?} finds {title:?}");
    }
    // and the UI jumps to the row
    let mut app = searching(0, "countdown");
    assert!(result_titles(&app).contains(&"Recording countdown".to_string()));
    render(&mut app, vec![key_event(Key::Enter)]);
    render(&mut app, vec![]);
    assert_eq!(tab_of(&app), Some(CAPTURE_TAB));
    assert_eq!(flash_of(&app).as_deref(), Some("Recording countdown"));
}

#[test]
fn capture_tab_shows_the_s2_defaults_and_no_jpg_choice() {
    let mut app = settings_app(CAPTURE_TAB, egui::ThemePreference::Dark);
    assert_eq!(app.settings.capture, ferrocast_core::capture::CaptureSettings::default());
    settle(&mut app);
    let texts = painted_texts(&mut app, vec![]);
    let has = |t: &str| texts.iter().any(|x| x == t);
    assert!(has("PNG"), "the format is shown as PNG: {texts:?}");
    assert!(!texts.iter().any(|x| x.to_uppercase().contains("JPG") && x.len() < 6), "no JPG choice: {texts:?}");
    for t in ["Show", "Copy", "Save", "30 fps", "60 fps", "Off", "3 s", "5 s"] {
        assert!(has(t), "{t:?} is a choice: {texts:?}");
    }
    assert!(texts.iter().any(|x| x.contains("Ferrocast Captures")), "the empty folder shows the default: {texts:?}");
    assert_eq!(tab_bars(CAPTURE_TAB), vec![3, 2, 3], "after, fps, countdown");
}

#[test]
fn capture_tab_controls_write_to_settings_capture() {
    use ferrocast_core::capture::AfterCapture;
    let mut app = settings_app(CAPTURE_TAB, egui::ThemePreference::Dark);
    settle(&mut app);
    // After capture: the first three-cell bar, on screen at the top of the tab
    app.ctx.data_mut(|d| d.remove::<Vec<(Rect, Vec<Rect>)>>(Id::new("segmented-count")));
    render(&mut app, vec![]);
    let mut bars: Vec<(Rect, Vec<Rect>)> = app.ctx.data(|d| d.get_temp(Id::new("segmented-count"))).unwrap_or_default();
    bars.sort_by(|a, b| a.0.min.y.total_cmp(&b.0.min.y));
    click_at(&mut app, bars[0].1[2].center());
    render(&mut app, vec![]);
    assert_eq!(app.settings.capture.after, AfterCapture::Save);
    // frame rate (the only two-cell bar) and countdown (the lowest three-cell bar): scroll to them
    let fps = reach_bar(&mut app, 2).expect("the frame rate bar is reachable");
    click_at(&mut app, fps[0].center());
    render(&mut app, vec![]);
    assert_eq!(app.settings.capture.fps, 30);
    let cd = reach_bar(&mut app, 3).expect("the countdown bar is reachable");
    click_at(&mut app, cd[0].center());
    render(&mut app, vec![]);
    assert_eq!(app.settings.capture.countdown, 0);
    let saved: Settings = app.store.load("settings.json");
    assert_eq!((saved.capture.after, saved.capture.fps, saved.capture.countdown), (AfterCapture::Save, 30, 0), "persisted");
}

#[test]
fn capture_shortcut_rows_follow_the_builtin_ids_and_have_no_default() {
    use crate::items::Builtin;
    assert_eq!(
        CAPTURE_SHORTCUTS.map(|(_, b)| b.id()),
        [
            Builtin::CaptureArea.id(),
            Builtin::CaptureWindow.id(),
            Builtin::CaptureScreen.id(),
            Builtin::CaptureScrolling.id(),
            Builtin::RecordScreen.id(),
            Builtin::RecordArea.id(),
            Builtin::RecordWindow.id(),
        ]
    );
    let s = Settings::default();
    for (_, b) in CAPTURE_SHORTCUTS {
        assert_eq!(s.hotkey(&b.id()), None, "no default shortcut for {}", b.title());
    }
}

#[test]
fn capture_shortcut_record_opens_the_hotkey_view_for_that_command() {
    let mut app = settings_app(CAPTURE_TAB, egui::ThemePreference::Dark);
    settle(&mut app);
    // the Record… button of the second shortcut row (Capture Window; the Refresh button of the Sources card comes first): scroll it into view
    let p = scroll_until(&mut app, |app| {
        app.ctx.data_mut(|d| d.remove::<Vec<(String, Id, Rect, bool)>>(Id::new("ctl-hits")));
        render(app, vec![]);
        let mut v: Vec<Rect> = sd5_hits(app).into_iter().filter(|h| h.0 == "small_btn").map(|h| h.2).collect();
        v.sort_by(|a, b| a.min.y.total_cmp(&b.min.y));
        assert_eq!(v.len(), 8, "one Refresh button (devices) and a Record… button per shortcut");
        Some(v[2].center())
    })
    .expect("reachable");
    for _ in 0..30 {
        render(&mut app, vec![]);
    }
    let _ = p;
    app.ctx.data_mut(|d| d.remove::<Vec<(String, Id, Rect, bool)>>(Id::new("ctl-hits")));
    render(&mut app, vec![]);
    let mut v: Vec<Rect> = sd5_hits(&app).into_iter().filter(|h| h.0 == "small_btn").map(|h| h.2).collect();
    v.sort_by(|a, b| a.min.y.total_cmp(&b.min.y));
    click_at(&mut app, v[2].center());
    render(&mut app, vec![]);
    match app.view() {
        View::Hotkey { id, .. } => assert_eq!(id, "builtin:capture-window"),
        _ => panic!("the Hotkey view opens"),
    }
}

// ---- P4: Settings > Capture sources, record flow reactions, Camera and Audio tabs ----

/// Hits of one kind on the Capture tab, top to bottom.
fn capture_tab_hits(app: &mut App, kind: &str) -> Vec<Rect> {
    app.ctx.data_mut(|d| d.remove::<Vec<(String, Id, Rect, bool)>>(Id::new("ctl-hits")));
    render(app, vec![]);
    let mut v: Vec<Rect> = sd5_hits(app).into_iter().filter(|h| h.0 == kind).map(|h| h.2).collect();
    v.sort_by(|a, b| a.min.y.total_cmp(&b.min.y));
    v
}

/// Scroll the Settings body down by about `dy` points and let the animation finish.
fn scroll_settings(app: &mut App, dy: f32) {
    render(app, vec![Event::PointerMoved(Pos2::new(500.0, 200.0))]);
    for _ in 0..6 {
        let wheel = Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(0.0, -dy / 6.0),
            phase: egui::TouchPhase::Move,
            modifiers: Modifiers::NONE,
        };
        render(app, vec![wheel]);
    }
    for _ in 0..30 {
        render(app, vec![]);
    }
}

#[test]
fn capture_tab_sources_show_default_devices_and_the_reason_when_none_are_listed() {
    use crate::record_ctl::DevicesState;
    use ferrocast_core::capture::protocol::{CameraDevice, Devices, MicrophoneDevice};
    let mut app = settings_app(CAPTURE_TAB, egui::ThemePreference::Dark);
    app.rt.devices = DevicesState::Failed("Screen capture is not available in this build.".into());
    settle(&mut app);
    scroll_settings(&mut app, 450.0);
    let texts = painted_texts(&mut app, vec![]);
    let defaults = texts.iter().filter(|t| *t == "Default").count();
    assert_eq!(defaults, 2, "both pickers show Default when empty: {texts:?}");
    for t in ["Camera", "Camera device", "Microphone", "Microphone device", "System audio", "Refresh"] {
        assert!(texts.iter().any(|x| x == t), "{t:?} on the tab: {texts:?}");
    }
    assert!(
        texts.iter().any(|t| t == "Devices are not listed: Screen capture is not available in this build."),
        "the reason is shown, not hidden: {texts:?}"
    );
    // a stored device is shown by its name once the list is known, by its id before
    app.settings.capture.camera_id = "0x1".into();
    app.settings.capture.mic_id = "gone".into();
    app.rt.devices = DevicesState::Ready(Devices {
        cameras: vec![CameraDevice { id: "0x1".into(), name: "FaceTime HD".into(), position: "unspecified".into(), default: true }],
        microphones: vec![MicrophoneDevice { id: "m1".into(), name: "Studio Mic".into(), default: false }],
    });
    for _ in 0..3 {
        render(&mut app, vec![]);
    }
    let texts = painted_texts(&mut app, vec![]);
    assert!(texts.iter().any(|t| t == "FaceTime HD"), "{texts:?}");
    assert!(texts.iter().any(|t| t == "gone (not connected)"), "{texts:?}");
    assert!(!texts.iter().any(|t| t.starts_with("Devices are not listed")));
}

#[test]
fn capture_tab_toggles_write_the_source_settings_and_nothing_else_changes() {
    let mut app = settings_app(CAPTURE_TAB, egui::ThemePreference::Dark);
    app.rt.devices = crate::record_ctl::DevicesState::Failed("x".into());
    settle(&mut app);
    let before = app.settings.clone();
    assert!(!before.capture.camera && !before.capture.microphone && !before.capture.system_audio);
    // toggles top to bottom: Include cursor, Camera, Microphone, System audio (the switch may sit below the fold)
    for (i, check) in [
        (1usize, (|a: &App| a.settings.capture.camera) as fn(&App) -> bool),
        (2, |a| a.settings.capture.microphone),
        (3, |a| a.settings.capture.system_audio),
    ] {
        scroll_until(&mut app, |app| capture_tab_hits(app, "toggle").get(i).map(|r| r.center())).expect("reachable");
        for _ in 0..30 {
            render(&mut app, vec![]);
        }
        let p = capture_tab_hits(&mut app, "toggle")[i].center();
        click_at(&mut app, p);
        assert!(check(&app), "toggle {i} turned its source on");
    }
    assert_eq!(app.settings.capture.fps, before.capture.fps);
    assert!(!app.settings.capture.include_cursor);
}

#[test]
fn opening_the_capture_tab_loads_devices_off_the_main_thread_and_the_answer_fills_the_lists() {
    use crate::record_ctl::{DevicesState, RecordMsg};
    use ferrocast_core::capture::protocol::{Devices, MicrophoneDevice};
    let mut app = settings_app(CAPTURE_TAB, egui::ThemePreference::Dark);
    assert_eq!(app.rt.devices, DevicesState::Idle);
    settle(&mut app);
    // the tab asked for the lists: the helper call is on a worker thread, the UI only shows Loading
    assert_eq!(app.rt.devices, DevicesState::Loading);
    // and asks once, not on every frame
    render(&mut app, vec![]);
    assert_eq!(app.rt.devices, DevicesState::Loading);
    // without a helper the reason is there at once: nothing is spawned and nothing blocks
    let mut bare = settings_app(CAPTURE_TAB, egui::ThemePreference::Dark);
    bare.capture = crate::capture_ctl::CaptureState::with(std::path::PathBuf::new(), crate::capture_ctl::Availability::NoHelper);
    settle(&mut bare);
    assert_eq!(bare.rt.devices, DevicesState::Failed("Screen capture is not available in this build.".into()));
    // a worker's answer replaces the state
    app.handle_record(RecordMsg::Devices(Ok(Devices {
        cameras: vec![],
        microphones: vec![MicrophoneDevice { id: "m1".into(), name: "Studio Mic".into(), default: true }],
    })));
    assert!(matches!(&app.rt.devices, DevicesState::Ready(d) if d.microphones.len() == 1));
    app.handle_record(RecordMsg::Devices(Err("boom".into())));
    assert_eq!(app.rt.devices, DevicesState::Failed("boom".into()));
}

#[test]
fn camera_and_microphone_denials_toast_the_s12_text_and_leave_the_recorder_idle() {
    use crate::record_ctl::{RecordMsg, StartFailure};
    let mut app = fixture();
    app.visible = true;
    app.rt.gen = 2;
    let dir = std::env::temp_dir().join(format!("fc-p4-deny-{}", std::process::id())).join("Half.ferrocast");
    std::fs::create_dir_all(&dir).unwrap();
    for (failure, text) in [
        (StartFailure::PermissionCamera, "Camera is off for Ferrocast. Turn it on in System Settings → Privacy & Security → Camera."),
        (
            StartFailure::PermissionMicrophone,
            "Microphone is off for Ferrocast. Turn it on in System Settings → Privacy & Security → Microphone.",
        ),
    ] {
        std::fs::create_dir_all(&dir).unwrap();
        app.record.begin(0);
        app.rt.project = Some(dir.clone());
        app.handle_record(RecordMsg::Started(2, Err(failure)));
        assert!(app.record.is_idle());
        assert_eq!(toast_text(&app), text);
        assert!(!dir.exists(), "the half-made project folder is removed, as for Screen Recording");
    }
    let _ = std::fs::remove_dir_all(dir.parent().unwrap());
}

#[test]
fn source_warnings_during_a_recording_become_toasts() {
    use crate::record_ctl::RecordMsg;
    use ferrocast_core::capture::protocol::RecordEvent;
    let mut app = fixture();
    app.visible = true;
    app.rt.gen = 4;
    app.record.begin(0);
    let warn = |code: &str, m: Option<&str>| RecordEvent::Warn { code: code.into(), message: m.map(String::from), free_bytes: None };
    app.handle_record(RecordMsg::Event(4, warn("camera-lost", Some("unplugged"))));
    assert_eq!(toast_text(&app), "The camera stopped. The recording continues without it.");
    app.handle_record(RecordMsg::Event(4, warn("mic-lost", None)));
    assert_eq!(toast_text(&app), "The microphone stopped. The recording continues without it.");
    app.handle_record(RecordMsg::Event(4, warn("source-failed", Some("system audio refused"))));
    assert_eq!(toast_text(&app), "A source could not be recorded: system audio refused");
    app.handle_record(RecordMsg::Event(4, warn("input-monitoring-off", None)));
    assert_eq!(toast_text(&app), "Keyboard shortcuts will not be shown: Input Monitoring is off.");
}

fn p4_project() -> ferrocast_core::capture::project::Project {
    use ferrocast_core::capture::project::Project;
    Project::from_json(include_str!("../../core/fixtures/camera_project.json")).unwrap()
}

/// An editor on a real folder holding `project` (autosave writes there); `files` are created empty-ish on disk.
fn editor_app_for(
    tag: &str,
    project: ferrocast_core::capture::project::Project,
    files: &[&str],
) -> (App, std::path::PathBuf, std::sync::mpsc::Receiver<crate::editor_ctl::WorkerCmd>) {
    use crate::editor_ctl::{Editor, Opened};
    let dir = std::env::temp_dir().join(format!("fc-editor-p4-{}-{tag}", std::process::id())).join("Take.ferrocast");
    std::fs::create_dir_all(&dir).unwrap();
    for f in files {
        std::fs::write(dir.join(f), vec![0u8; 4096]).unwrap();
    }
    project.save(&dir).unwrap();
    let mut app = fixture();
    let (tx, rx) = std::sync::mpsc::channel();
    let mut ed = Editor::new(dir.clone(), Some(tx));
    ed.opened(Ok(Opened { project, auto_zoomed: false, note: None, events: None }));
    app.editor = Some(ed);
    app.stack = vec![View::Root, View::Editor { project: dir.clone() }];
    (app, dir, rx)
}

/// Every text of the active panel tab: the panel is a small scroll area, so read it top to bottom
/// in steps and come back to the top.
fn tab_texts_scrolling(app: &mut App) -> Vec<String> {
    let mut all = tab_labels(app);
    for _ in 0..8 {
        scroll_panel(app, -90.0);
        all.extend(tab_labels(app));
    }
    scroll_panel(app, 2000.0);
    all
}

fn tab_labels(app: &mut App) -> Vec<String> {
    render_texts(app, vec![]).into_iter().map(|(t, _)| t).collect()
}

#[test]
fn a_project_without_camera_or_audio_shows_neither_tab() {
    let (mut app, _rx, _dir) = editor_app_in("p4-plain");
    let texts = tab_labels(&mut app);
    for t in ["Zoom", "Cursor", "Style", "Clips"] {
        assert!(texts.iter().any(|x| x == t), "{t} tab: {texts:?}");
    }
    assert!(!texts.iter().any(|x| x == "Camera" || x == "Audio"), "no Camera/Audio tab without sources: {texts:?}");
    // a leftover Camera tab selection (another project) falls back instead of drawing an empty panel
    set_tab(&mut app, Tab::Camera);
    let texts = tab_labels(&mut app);
    assert_eq!(app.editor.as_ref().unwrap().st.tab, Tab::Zoom);
    assert!(texts.iter().any(|x| x == "Add at playhead"), "{texts:?}");
}

#[test]
fn the_camera_tab_draws_its_controls_and_edits_through_the_history() {
    use ferrocast_core::capture::camera::{CameraShape, Corner};
    let (mut app, dir, _rx) = editor_app_for("p4-cam", p4_project(), &["camera.mov", "mic.m4a"]);
    let texts = tab_labels(&mut app);
    assert!(texts.iter().any(|x| x == "Camera") && texts.iter().any(|x| x == "Audio"), "both tabs: {texts:?}");
    click_label(&mut app, "Camera");
    assert_eq!(app.editor.as_ref().unwrap().st.tab, Tab::Camera);
    // the lower part of the tab is below the fold: the wheel scrolls the panel through it
    let texts = tab_texts_scrolling(&mut app);
    for want in [
        "Show camera",
        "Shape",
        "Circle",
        "Rounded",
        "Square",
        "Corner",
        "Top left",
        "Top right",
        "Bottom left",
        "Bottom right",
        "Size",
        "Margin",
        "Mirror",
    ] {
        assert!(texts.iter().any(|t| t == want), "Camera tab lacks `{want}`: {texts:?}");
    }
    assert!(texts.iter().any(|t| t.starts_with("camera.mov · 0:10") && t.contains("+0.10 s")), "source info: {texts:?}");
    assert_eq!(editor_proj(&app).camera.as_ref().unwrap().shape, CameraShape::Rounded, "the fixture starts rounded");
    click_label(&mut app, "Square");
    click_label(&mut app, "Top left");
    let c = editor_proj(&app).camera.unwrap();
    assert_eq!((c.shape, c.corner), (CameraShape::Square, Corner::TopLeft));
    app.editor.as_mut().unwrap().flush(1e6);
    let saved = ferrocast_core::capture::project::Project::load(&dir).unwrap().camera.unwrap();
    assert_eq!((saved.shape, saved.corner), (CameraShape::Square, Corner::TopLeft), "autosave wrote the fields");
    render(&mut app, vec![key_mod_event(Key::Z, Modifiers::COMMAND)]);
    assert_eq!(editor_proj(&app).camera.as_ref().unwrap().corner, Corner::TopRight, "one undo step per click");
    // hide
    click_label(&mut app, "Show camera");
    assert!(!editor_proj(&app).camera.as_ref().unwrap().visible);
}

#[test]
fn the_audio_tab_shows_the_mix_and_a_missing_file_never_crashes() {
    use ferrocast_core::capture::project::AudioTrack;
    let (mut app, _dir, _rx) = editor_app_for("p4-audio", p4_project(), &["mic.m4a"]);
    set_tab(&mut app, Tab::Audio);
    let texts = tab_texts_scrolling(&mut app);
    for want in ["Microphone", "System audio", "Gain", "Mute", "Muted", "Master"] {
        assert!(texts.iter().any(|t| t == want), "Audio tab lacks `{want}`: {texts:?}");
    }
    // the fixture's system track is muted; its numeric gain is on screen
    assert!(["-6", "3", "-1.5"].iter().all(|v| texts.iter().any(|t| t == v || *t == format!("{v}.0"))), "gains in dB: {texts:?}");
    // system.m4a is not on disk: said clearly
    assert!(texts.iter().any(|t| t.starts_with("system.m4a is missing")), "{texts:?}");
    assert!(texts.iter().any(|t| t.starts_with("mic.m4a · 0:10")), "{texts:?}");
    // mute buttons edit the project: the fixture has the mic live and the system track muted
    assert!(!editor_proj(&app).audio.mic.unwrap().mute && editor_proj(&app).audio.system.unwrap().mute);
    click_label(&mut app, "Muted");
    assert!(!editor_proj(&app).audio.system.unwrap().mute, "the system track is live again");
    click_label(&mut app, "Mute");
    assert!(editor_proj(&app).audio.mic.unwrap().mute, "the first Mute button is the microphone's");
    assert_eq!(crate::editor_panels::track_mix(&editor_proj(&app), AudioTrack::Mic).gain_db, 3.0, "muting keeps the gain");
}

#[test]
fn an_audio_only_project_has_the_audio_tab_and_no_camera_tab() {
    let mut p = p4_project();
    p.sources.camera = None;
    p.camera = None;
    let (mut app, _dir, _rx) = editor_app_for("p4-audio-only", p, &["mic.m4a", "system.m4a"]);
    let texts = tab_labels(&mut app);
    assert!(texts.iter().any(|x| x == "Audio") && !texts.iter().any(|x| x == "Camera"), "{texts:?}");
}

// ---- T19: recording commands, the flow's reactions and the Recordings browser ----

fn record_ids(app: &App) -> Vec<String> {
    app.base.iter().filter(|i| i.id.starts_with("builtin:record") || i.id == "builtin:open-last-recording").map(|i| i.id.clone()).collect()
}

#[test]
fn record_commands_are_listed_with_the_capture_gating_and_subtitle() {
    use crate::capture_ctl::{Availability, CaptureState};
    let mut app = fixture();
    app.rebuild();
    let mut ids = record_ids(&app);
    ids.sort();
    assert_eq!(
        ids,
        ["builtin:open-last-recording", "builtin:record-area", "builtin:record-screen", "builtin:record-window", "builtin:recordings"]
    );
    for (id, title) in [
        ("builtin:record-screen", "Record Screen"),
        ("builtin:record-area", "Record Area"),
        ("builtin:record-window", "Record Window"),
        ("builtin:open-last-recording", "Open Last Recording"),
        ("builtin:recordings", "Recordings"),
    ] {
        let item = app.base.iter().find(|i| i.id == id).unwrap_or_else(|| panic!("{id}"));
        assert_eq!((item.title.as_str(), item.subtitle.as_str(), item.is_command()), (title, "Capture", true));
    }
    for availability in [Availability::NeedsMacOs14(13), Availability::NoHelper] {
        app.capture = CaptureState::with(std::path::PathBuf::new(), availability);
        app.rebuild();
        assert!(record_ids(&app).is_empty(), "{availability:?}");
    }
}

#[test]
fn a_second_recording_is_refused_while_one_runs() {
    use crate::record_ctl::{Phase, RecordKind};
    let mut app = fixture();
    app.visible = true;
    app.record.begin(0);
    app.run_record(RecordKind::Screen);
    assert_eq!(app.toast.as_ref().map(|t| t.0.as_str()), Some("A recording is already running. Stop it from the menu bar."));
    assert_eq!(app.record.phase(), Phase::Recording, "the running recording is untouched");
    assert!(app.visible, "nothing else happened");
}

// ---- live bug #1: the selection is non-blocking, one flow at a time ----

#[test]
fn a_second_capture_command_while_a_selection_is_pending_is_refused() {
    use crate::capture_ctl::{CaptureKind, Flow, SELECTION_BUSY};
    use crate::record_ctl::RecordKind;
    let mut app = fixture();
    app.visible = true;
    let first = Flow::Capture(CaptureKind::Area);
    app.selection.begin(first).unwrap();
    app.run_capture(CaptureKind::Window);
    assert_eq!(app.toast.as_ref().map(|t| t.0.as_str()), Some(SELECTION_BUSY));
    app.toast = None;
    app.run_record(RecordKind::Area);
    assert_eq!(app.toast.as_ref().map(|t| t.0.as_str()), Some(SELECTION_BUSY), "also for the recording flow");
    assert!(app.visible, "a refused command does not hide the launcher");
    assert!(app.selection.begin(Flow::Capture(CaptureKind::Screen)).is_err(), "the first flow still owns the slot");
    assert!(app.record.is_idle(), "no recording was started");
}

#[test]
fn esc_in_the_overlay_leaves_no_toast_and_frees_the_slot() {
    use crate::capture_ctl::{CaptureKind, Flow};
    use ferrocast_core::capture::protocol::Shareable;
    use ferrocast_macos::selector::Selection;
    let mut app = fixture();
    app.visible = true;
    for flow in [Flow::Capture(CaptureKind::Area), Flow::Capture(CaptureKind::Scrolling), Flow::Record(crate::record_ctl::RecordKind::Area)]
    {
        app.selection.begin(flow).unwrap();
        assert!(app.selection.listed(flow));
        app.capture_selected(flow, Some(Selection::Cancelled), Shareable::default());
        assert!(app.toast.is_none(), "{flow:?}: Esc says nothing");
        assert!(app.record.is_idle(), "{flow:?}: nothing was started");
        assert!(app.selection.begin(flow).is_ok(), "{flow:?}: the slot is free again");
        app.selection.cancel();
    }
}

#[test]
fn a_selection_that_arrives_after_the_flow_was_cancelled_is_ignored() {
    use crate::capture_ctl::{CaptureKind, Flow};
    use ferrocast_core::capture::protocol::Shareable;
    let mut app = fixture();
    app.visible = true;
    let other = Flow::Capture(CaptureKind::Window);
    app.selection.begin(other).unwrap();
    app.capture_selected(Flow::Capture(CaptureKind::Area), None, Shareable::default());
    assert!(app.selection.begin(other).is_err(), "the stale message did not touch the pending flow");
    app.capture_listed(Flow::Capture(CaptureKind::Area), Ok(Shareable::default()));
    assert!(app.toast.is_none());
}

#[test]
fn a_listing_that_fails_clears_the_slot_and_says_why() {
    use crate::capture_ctl::{CaptureKind, Flow, Outcome};
    use crate::record_ctl::RecordKind;
    let mut app = fixture();
    app.visible = true;
    let cap = Flow::Capture(CaptureKind::Area);
    app.selection.begin(cap).unwrap();
    app.capture_listed(cap, Err(Outcome::Failed("helper gone".into())));
    assert_eq!(app.toast.as_ref().map(|t| t.0.as_str()), Some("Capture failed: helper gone"));
    assert!(app.selection.begin(cap).is_ok());
    app.selection.cancel();
    app.toast = None;
    let rec = Flow::Record(RecordKind::Area);
    app.selection.begin(rec).unwrap();
    app.capture_listed(rec, Err(Outcome::PermissionDenied));
    assert_eq!(app.toast.as_ref().map(|t| t.0.as_str()), Some(crate::capture_ctl::PERMISSION_TOAST));
    app.toast = None;
    app.selection.begin(rec).unwrap();
    app.capture_listed(rec, Err(Outcome::Cancelled));
    assert!(app.toast.is_none());
    assert!(app.selection.begin(rec).is_ok(), "cancelled also frees the slot");
}

#[test]
fn the_record_command_during_the_countdown_cancels_the_start() {
    use crate::record_ctl::{Phase, RecordKind};
    let mut app = fixture();
    app.record.begin(3);
    app.run_record(RecordKind::Area);
    assert_eq!(app.record.phase(), Phase::Idle);
}

#[test]
fn hud_and_tray_stop_send_record_stop_to_the_session() {
    use crate::record_ctl::{Cmd, Phase, RecordMsg};
    let mut app = fixture();
    let (tx, rx) = std::sync::mpsc::channel();
    app.rt.cmds = Some(tx);
    app.record.begin(0);
    app.handle_record(RecordMsg::TogglePause);
    assert_eq!((app.record.phase(), rx.try_recv().ok()), (Phase::Paused, Some(Cmd::Pause)));
    app.handle_record(RecordMsg::TogglePause);
    assert_eq!((app.record.phase(), rx.try_recv().ok()), (Phase::Recording, Some(Cmd::Resume)));
    app.bus.send(Msg::Tray("stop_recording".into()));
    app.drain();
    assert_eq!((app.record.phase(), rx.try_recv().ok()), (Phase::Stopping, Some(Cmd::Stop)));
    app.handle_record(RecordMsg::Stop);
    assert!(rx.try_recv().is_err(), "a second Stop is not sent twice");
}

#[test]
fn helper_events_drive_the_warnings_and_the_disk_guard() {
    use crate::record_ctl::{Cmd, Phase, RecordMsg};
    use ferrocast_core::capture::protocol::RecordEvent;
    let mut app = fixture();
    app.visible = true;
    let (tx, rx) = std::sync::mpsc::channel();
    app.rt.cmds = Some(tx);
    app.rt.gen = 4;
    app.record.begin(0);
    let warn = |code: &str, free| RecordEvent::Warn { code: code.into(), message: None, free_bytes: free };
    app.handle_record(RecordMsg::Event(4, warn("input-monitoring-off", None)));
    assert_eq!(toast_text(&app), "Keyboard shortcuts will not be shown: Input Monitoring is off.");
    assert_eq!(app.record.phase(), Phase::Recording, "it records anyway");
    app.handle_record(RecordMsg::Event(4, warn("low-disk", Some(1_500_000_000))));
    assert_eq!(toast_text(&app), "Low disk space: 1.5 GB left");
    assert!(rx.try_recv().is_err());
    app.handle_record(RecordMsg::Event(3, warn("low-disk", Some(100_000_000))));
    assert_eq!(app.record.phase(), Phase::Recording, "events of an older session are ignored");
    app.handle_record(RecordMsg::Event(4, warn("low-disk", Some(100_000_000))));
    assert_eq!((app.record.phase(), rx.try_recv().ok()), (Phase::Stopping, Some(Cmd::Stop)));
}

#[test]
fn a_denied_start_shows_the_s4_toast_and_leaves_the_recorder_idle() {
    use crate::record_ctl::{RecordMsg, StartFailure};
    let mut app = fixture();
    app.visible = true;
    app.rt.gen = 2;
    app.record.begin(0);
    app.handle_record(RecordMsg::Started(2, Err(StartFailure::Permission)));
    assert!(app.record.is_idle());
    assert_eq!(
        toast_text(&app),
        "Screen Recording is off for Ferrocast. Turn it on in System Settings → Privacy & Security → Screen & System Audio Recording, then try again."
    );
    app.record.begin(0);
    app.handle_record(RecordMsg::Started(2, Err(StartFailure::Message("Recording failed: boom".into()))));
    assert!(app.record.is_idle());
    assert_eq!(toast_text(&app), "Recording failed: boom");
}

#[test]
fn a_finished_recording_ends_the_flow_and_says_how_long_it_was() {
    use crate::record_ctl::{RecordMsg, Saved};
    let mut app = fixture();
    app.visible = true;
    app.rt.gen = 1;
    app.record.begin(0);
    let saved = Saved { project: "/r/Recording 1.ferrocast".into(), duration: 65.4, interrupted: false, message: None };
    app.handle_record(RecordMsg::Finished(1, Ok(saved)));
    assert!(app.record.is_idle(), "a helper that sent no stopped event still ends the flow");
    assert_eq!(toast_text(&app), "Recording saved: Recording 1 (1:05)");
    app.handle_record(RecordMsg::Finished(1, Err("writer failed".into())));
    assert_eq!(toast_text(&app), "Recording failed: writer failed");
}

#[test]
fn the_recordings_browser_lists_newest_first_with_reveal_and_trash() {
    use crate::record_ctl::RecordingEntry;
    let mut app = fixture();
    let entry = |name: &str, created| RecordingEntry {
        path: format!("/r/{name}.ferrocast").into(),
        name: name.into(),
        duration: Some(65.0),
        bytes: 48_000_000,
        created,
    };
    app.rt.recordings = vec![entry("New", 1_800_000_000), entry("Old", 1_700_000_000)];
    app.stack.push(View::Recordings);
    let sections = app.sections();
    let items: Vec<_> = sections.iter().flat_map(|s| s.items.iter()).collect();
    assert_eq!(items.iter().map(|i| i.title.as_str()).collect::<Vec<_>>(), ["New", "Old"]);
    assert_eq!(items[0].subtitle, "1:05 · 48 MB");
    assert!(!items[0].accessory.is_empty(), "the date");
    let actions = app.actions_for(items[0]);
    assert_eq!(actions[0].title, "Open in Editor");
    let reveal = actions.iter().find(|a| a.title == "Show in Finder").expect("reveal");
    assert_eq!(reveal.shortcut, Some((crate::items::CMD, egui::Key::Enter)));
    let trash = actions.iter().find(|a| a.title == "Move to Trash").expect("trash");
    assert!(trash.destructive);
    assert_eq!(trash.shortcut, Some((crate::items::CMD, egui::Key::Backspace)));
    assert_eq!(app.primary_label(items[0]), "Open in Editor");
    app.query = "old".into();
    let titles: Vec<String> = app.sections().iter().flat_map(|s| s.items.iter().map(|i| i.title.clone())).collect();
    assert_eq!(titles, ["Old"]);
}

// ---- T26: the studio editor view (S15) ----
// The render worker is replaced by a channel the test reads: it plays the "fake client".

fn editor_project() -> ferrocast_core::capture::project::Project {
    let mut p = ferrocast_core::capture::project::Project::new("Demo", "2026-10-23T14:05:22Z");
    p.duration = 10.0;
    p.zooms = vec![serde_json::from_value(serde_json::json!({"id":"z1","start":2.0,"end":4.0,"mode":"auto","scale":2.0})).unwrap()];
    p
}

/// An app showing the editor on `/r/Demo.ferrocast`, plus the requests the view sends to its worker.
fn editor_app() -> (App, std::sync::mpsc::Receiver<crate::editor_ctl::WorkerCmd>) {
    use crate::editor_ctl::{Editor, Opened};
    let mut app = fixture();
    let dir = std::path::PathBuf::from("/r/Demo.ferrocast");
    let (tx, rx) = std::sync::mpsc::channel();
    let mut ed = Editor::new(dir.clone(), Some(tx));
    ed.opened(Ok(Opened { project: editor_project(), auto_zoomed: true, note: None, events: None }));
    app.editor = Some(ed);
    app.stack = vec![View::Root, View::Editor { project: dir }];
    (app, rx)
}

fn editor_requests(rx: &std::sync::mpsc::Receiver<crate::editor_ctl::WorkerCmd>) -> Vec<crate::editor_ctl::Req> {
    rx.try_iter().filter_map(|c| if let crate::editor_ctl::WorkerCmd::Frame(r) = c { Some(r) } else { None }).collect()
}

fn editor_frame(app: &mut App, seq: u64, t: f64) {
    use crate::editor_ctl::{EditorMsg, FrameOut};
    let dir = std::path::PathBuf::from("/r/Demo.ferrocast");
    let out = FrameOut { image: egui::ColorImage::example(), t, helper_ms: 2.0, total_ms: 5.0 };
    app.handle_editor(EditorMsg::Frame { dir, seq, result: Ok(out) });
}

fn editor_t(app: &App) -> f64 {
    app.editor.as_ref().unwrap().t
}

#[test]
fn the_editor_renders_loading_error_and_frame_states() {
    // loading: the project is not there yet
    let mut app = fixture();
    let dir = std::path::PathBuf::from("/r/Demo.ferrocast");
    app.editor = Some(crate::editor_ctl::Editor::new(dir.clone(), None));
    app.stack = vec![View::Root, View::Editor { project: dir }];
    render(&mut app, vec![]);
    // the helper's message is shown when opening fails
    app.editor.as_mut().unwrap().opened(Err("The recording's video (screen.mov) is missing.".into()));
    app.ctx.data_mut(|d| d.remove::<Vec<String>>(Id::new("body-notes")));
    render(&mut app, vec![]);
    let notes: Vec<String> = app.ctx.data(|d| d.get_temp(Id::new("body-notes"))).unwrap_or_default();
    assert!(notes.iter().any(|n| n.contains("esc closes the editor")), "{notes:?}");
    assert!(app.editor.as_ref().unwrap().error.as_deref().is_some_and(|e| e.contains("screen.mov")));
    // a frame
    let (mut app, rx) = editor_app();
    render(&mut app, vec![]);
    let reqs = editor_requests(&rx);
    assert_eq!(reqs.len(), 1, "the first frame is asked for as soon as the view draws");
    assert!(app.editor.as_ref().unwrap().tex.is_none());
    editor_frame(&mut app, reqs[0].seq, reqs[0].key.t);
    render(&mut app, vec![]);
    let ed = app.editor.as_ref().unwrap();
    assert!(ed.tex.is_some() && ed.shown_t == Some(0.0), "the decoded frame became the preview texture");
    // a failing frame shows the helper's message and the last frame stays
    let mut app = editor_app().0;
    render(&mut app, vec![]);
    app.handle_editor(crate::editor_ctl::EditorMsg::Frame {
        dir: "/r/Demo.ferrocast".into(),
        seq: 1,
        result: Err("take frame timed out".into()),
    });
    assert_eq!(app.editor.as_ref().unwrap().note.as_deref(), Some("take frame timed out"));
    render(&mut app, vec![]);
}

#[test]
fn the_preview_asks_for_its_on_screen_width_capped_at_960() {
    let (mut app, rx) = editor_app();
    render(&mut app, vec![]);
    let r = editor_requests(&rx);
    let w = r[0].key.width;
    assert!((64..=crate::editor_ctl::PREVIEW_MAX_WIDTH).contains(&w) && w.is_multiple_of(32), "{w}");
    assert_eq!(crate::editor_ctl::PREVIEW_MAX_WIDTH, 960);
}

#[test]
fn editor_keys_step_frames_seconds_and_jump() {
    let (mut app, _rx) = editor_app();
    render(&mut app, vec![]);
    let fps = f64::from(app.settings.capture.fps);
    render(&mut app, vec![key_event(Key::ArrowRight)]);
    assert!((editor_t(&app) - 1.0 / fps).abs() < 1e-9, "one frame forward: {}", editor_t(&app));
    render(&mut app, vec![key_event(Key::ArrowRight)]);
    render(&mut app, vec![key_event(Key::ArrowLeft)]);
    assert!((editor_t(&app) - 1.0 / fps).abs() < 1e-9);
    render(&mut app, vec![key_mod_event(Key::ArrowRight, Modifiers::SHIFT)]);
    assert!((editor_t(&app) - (1.0 + 1.0 / fps)).abs() < 1e-9, "one second forward");
    render(&mut app, vec![key_mod_event(Key::ArrowLeft, Modifiers::SHIFT)]);
    render(&mut app, vec![key_event(Key::End)]);
    assert!((editor_t(&app) - (10.0 - 1.0 / fps)).abs() < 1e-9, "End is the last frame");
    render(&mut app, vec![key_event(Key::Home)]);
    assert_eq!(editor_t(&app), 0.0);
    render(&mut app, vec![key_event(Key::ArrowLeft)]);
    assert_eq!(editor_t(&app), 0.0, "never before the start");
}

#[test]
fn space_toggles_playback_and_stepping_pauses_it() {
    let (mut app, _rx) = editor_app();
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Space)]);
    assert!(app.editor.as_ref().unwrap().playing());
    render(&mut app, vec![key_event(Key::Space)]);
    assert!(!app.editor.as_ref().unwrap().playing());
    render(&mut app, vec![key_event(Key::Space)]);
    render(&mut app, vec![key_event(Key::ArrowRight)]);
    assert!(!app.editor.as_ref().unwrap().playing(), "stepping a frame pauses");
}

#[test]
fn clicking_and_dragging_the_timeline_seeks_and_requests_coalesce() {
    let (mut app, rx) = editor_app();
    render(&mut app, vec![]);
    let first = editor_requests(&rx);
    assert_eq!(first.len(), 1);
    let tl: Rect = app.ctx.data(|d| d.get_temp(Id::new("editor-timeline-rect"))).expect("timeline drawn");
    let at = |f: f32| Pos2::new(tl.min.x + tl.width() * f, tl.center().y);
    click_at(&mut app, at(0.5));
    assert!((editor_t(&app) - 5.0).abs() < 0.2, "click in the middle seeks to half: {}", editor_t(&app));
    // drag across the strip while the first render is still in flight: nothing is sent
    let btn = |pos, pressed| Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    render(&mut app, vec![Event::PointerMoved(at(0.1)), btn(at(0.1), true)]);
    for i in 2..=9 {
        render(&mut app, vec![Event::PointerMoved(at(i as f32 / 10.0))]);
    }
    render(&mut app, vec![btn(at(0.9), false)]);
    assert!(editor_requests(&rx).is_empty(), "no request flood while a render is in flight");
    assert!((editor_t(&app) - 9.0).abs() < 0.3, "the playhead followed the drag: {}", editor_t(&app));
    // the first frame comes back: only the latest time is asked for
    editor_frame(&mut app, first[0].seq, first[0].key.t);
    let next = editor_requests(&rx);
    assert_eq!(next.len(), 1, "{next:?}");
    assert!((next[0].key.t - editor_t(&app)).abs() < 1e-9, "the pending request is the latest playhead time");
}

#[test]
fn esc_closes_the_editor_and_ends_its_session() {
    let (mut app, _rx) = editor_app();
    render(&mut app, vec![]);
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(matches!(app.view(), View::Root));
    app.drain();
    assert!(app.editor.is_none(), "the worker and the helper go with the view");
}

#[test]
fn recordings_enter_opens_the_editor_and_the_footer_names_both_keys() {
    use crate::record_ctl::RecordingEntry;
    let mut app = fixture();
    let path: std::path::PathBuf = "/r/Take.ferrocast".into();
    app.rt.recordings =
        vec![RecordingEntry { path: path.clone(), name: "Take".into(), duration: Some(5.0), bytes: 1, created: 1_800_000_000 }];
    app.stack.push(View::Recordings);
    let hints = app.view().footer_hints();
    assert_eq!(hints[0], ("Open", "↵"));
    assert!(hints.contains(&("Show in Finder", "⌘↵")), "{hints:?}");
    let item = app.sections().into_iter().flat_map(|s| s.items).next().unwrap();
    app.run_item(&item);
    assert!(matches!(app.view(), View::Editor { project } if *project == path), "{}", app.view().title());
    assert_eq!(app.editor.as_ref().map(|e| e.dir.clone()), Some(path));
    assert_eq!(app.view().title(), "Editor");
    assert_eq!(app.stack.len(), 3, "Esc returns to the Recordings browser");
}

#[test]
fn open_last_recording_opens_the_editor_without_the_not_available_toast() {
    let root = std::env::temp_dir().join(format!("fc-editor-last-{}", std::process::id()));
    let project = root.join("Newest.ferrocast");
    ferrocast_core::capture::project::Project::create(&project, "Newest", "2026-10-23T14:05:22Z").unwrap();
    let mut app = fixture();
    app.visible = true;
    app.settings.capture.recordings_folder = root.to_string_lossy().to_string();
    app.open_last_recording();
    assert!(matches!(app.view(), View::Editor { project: p } if *p == project), "{}", app.view().title());
    assert!(app.toast.as_ref().is_none_or(|t| !t.0.contains("not available yet")), "{:?}", app.toast);
    // no recordings: the existing message, no editor
    let mut app = fixture();
    app.visible = true;
    app.settings.capture.recordings_folder = root.join("empty").to_string_lossy().to_string();
    app.open_last_recording();
    assert!(matches!(app.view(), View::Root) && app.editor.is_none());
}

// ---- T27: editor panels, undo/redo and autosave (egui harness; the worker is a channel the test reads)

use crate::editor_panels::Tab;

/// Every text the frame painted with its rectangle: find a label, click where it is.
fn render_texts(app: &mut App, events: Vec<Event>) -> Vec<(String, Rect)> {
    fn walk(s: &egui::Shape, out: &mut Vec<(String, Rect)>) {
        match s {
            egui::Shape::Text(t) => out.push((t.galley.text().to_string(), Rect::from_min_size(t.pos, t.galley.size()))),
            egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(ui::WIDTH, ui::HEIGHT))),
        events,
        ..Default::default()
    };
    let ctx = app.ctx.clone();
    let mut output = ctx.run_ui(input, |ui| app.draw(ui));
    output.textures_delta.clear();
    let mut out = vec![];
    output.shapes.iter().for_each(|c| walk(&c.shape, &mut out));
    out
}

fn label_rect(app: &mut App, text: &str) -> Rect {
    let texts = render_texts(app, vec![]);
    let body_bottom = ui::HEIGHT - ui::FOOTER_H;
    texts
        .iter()
        .filter(|(t, r)| t == text && r.max.y < body_bottom)
        .map(|(_, r)| *r)
        .next()
        .unwrap_or_else(|| panic!("no `{text}` on screen: {:?}", texts.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>()))
}

/// Wheel over the right-hand panel by `dy` points (negative scrolls down).
fn scroll_panel(app: &mut App, dy: f32) {
    render(app, vec![Event::PointerMoved(Pos2::new(ui::WIDTH - 100.0, 220.0))]);
    for _ in 0..6 {
        let wheel = Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(0.0, dy / 6.0),
            phase: egui::TouchPhase::Move,
            modifiers: Modifiers::NONE,
        };
        render(app, vec![wheel]);
    }
    render(app, vec![]);
}

fn click_label(app: &mut App, text: &str) {
    let r = label_rect(app, text);
    click_at(app, r.center());
}

fn screen_has(app: &mut App, text: &str) -> bool {
    render_texts(app, vec![]).iter().any(|(t, _)| t.contains(text))
}

/// An editor on a real folder (autosave writes there) and the channel the worker would read.
fn editor_app_in(tag: &str) -> (App, std::sync::mpsc::Receiver<crate::editor_ctl::WorkerCmd>, std::path::PathBuf) {
    use crate::editor_ctl::{Editor, Opened};
    let dir = std::env::temp_dir().join(format!("fc-editor-ui-{}-{tag}", std::process::id())).join("Take.ferrocast");
    std::fs::create_dir_all(&dir).unwrap();
    let project = editor_project();
    project.save(&dir).unwrap();
    let mut app = fixture();
    let (tx, rx) = std::sync::mpsc::channel();
    let mut ed = Editor::new(dir.clone(), Some(tx));
    ed.opened(Ok(Opened { project, auto_zoomed: false, note: None, events: None }));
    app.editor = Some(ed);
    app.stack = vec![View::Root, View::Editor { project: dir.clone() }];
    (app, rx, dir)
}

fn editor_proj(app: &App) -> ferrocast_core::capture::project::Project {
    app.editor.as_ref().unwrap().project.clone().unwrap()
}

fn set_tab(app: &mut App, tab: Tab) {
    app.editor.as_mut().unwrap().st.tab = tab;
}

#[test]
fn each_editor_tab_draws_its_controls() {
    let (mut app, _rx, _dir) = editor_app_in("tabs");
    for (tab, expect) in [
        (Tab::Zoom, &["Add at playhead", "Auto zoom", "Auto  0:02.00–0:04.00  ·  2.0×"][..]),
        (Tab::Cursor, &["Default", "Hidden", "Size", "Smoothing", "Click ring", "Motion blur"][..]),
        (Tab::Style, &["Aspect", "Background", "Angle", "Aurora"][..]),
        (Tab::Clips, &["Trim in", "Trim out", "Split", "Delete segment", "Apply to segment", "Segments (click to move the playhead)"][..]),
    ] {
        set_tab(&mut app, tab);
        let texts: Vec<String> = render_texts(&mut app, vec![]).into_iter().map(|(t, _)| t).collect();
        for want in expect {
            assert!(texts.iter().any(|t| t == want), "{tab:?} lacks `{want}`: {texts:?}");
        }
    }
    // the rest of the Style tab is below the fold: the wheel scrolls the panel
    set_tab(&mut app, Tab::Style);
    let panel = Pos2::new(ui::WIDTH - 100.0, 150.0);
    render(&mut app, vec![Event::PointerMoved(panel)]);
    let mut seen = vec![];
    for _ in 0..6 {
        let wheel = Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(0.0, -200.0),
            phase: egui::TouchPhase::Move,
            modifiers: Modifiers::NONE,
        };
        seen.extend(render_texts(&mut app, vec![wheel]).into_iter().map(|(t, _)| t));
    }
    for want in ["Padding", "Corners", "Shadow", "Opacity", "Offset"] {
        assert!(seen.iter().any(|t| t == want), "Style lacks `{want}` after scrolling: {seen:?}");
    }
    // the transport row has Undo/Redo and the footer says how
    assert!(screen_has(&mut app, "Saved"));
    assert!(app.view().footer_hints().contains(&("Undo", "⌘Z")) && app.view().footer_hints().contains(&("Redo", "⇧⌘Z")));
    // the tabs switch by click
    click_label(&mut app, "Cursor");
    assert_eq!(app.editor.as_ref().unwrap().st.tab, Tab::Cursor);
}

#[test]
fn clicking_trim_in_edits_and_cmd_z_and_shift_cmd_z_undo_and_redo_it() {
    let (mut app, rx, _dir) = editor_app_in("keys");
    set_tab(&mut app, Tab::Clips);
    render(&mut app, vec![]);
    app.editor.as_mut().unwrap().t = 2.0;
    click_label(&mut app, "Trim in");
    assert_eq!(editor_proj(&app).trims.len(), 1);
    assert_eq!(editor_proj(&app).trims[0].start, 2.0, "cut before the playhead");
    assert!(rx.try_iter().any(|c| matches!(c, crate::editor_ctl::WorkerCmd::Project(p) if p.trims.len() == 1)), "the worker got the edit");
    assert!(app.editor.as_ref().unwrap().st.history.can_undo());
    render(&mut app, vec![key_mod_event(Key::Z, Modifiers::COMMAND)]);
    assert!(editor_proj(&app).trims.is_empty(), "⌘Z undid the trim");
    render(&mut app, vec![key_mod_event(Key::Z, Modifiers::COMMAND | Modifiers::SHIFT)]);
    assert_eq!(editor_proj(&app).trims[0].start, 2.0, "⇧⌘Z redid it");
    // the toolbar buttons do the same
    click_label(&mut app, "Undo");
    assert!(editor_proj(&app).trims.is_empty());
    click_label(&mut app, "Redo");
    assert_eq!(editor_proj(&app).trims.len(), 1);
}

#[test]
fn undo_and_redo_buttons_are_disabled_without_history() {
    let (mut app, _rx, _dir) = editor_app_in("disabled");
    let hits_of =
        |app: &App| -> Vec<(String, egui::Id, Rect, bool)> { app.ctx.data(|d| d.get_temp(Id::new("ctl-hits"))).unwrap_or_default() };
    app.ctx.data_mut(|d| d.remove::<Vec<(String, egui::Id, Rect, bool)>>(Id::new("ctl-hits")));
    render(&mut app, vec![]);
    let flags: Vec<bool> = hits_of(&app).iter().filter(|h| h.0 == "small_btn").map(|h| h.3).collect();
    assert_eq!(flags, vec![true, false, false, true], "Play enabled, Undo and Redo disabled, Export… enabled: {flags:?}");
    // a click on a disabled Undo does nothing
    click_label(&mut app, "Undo");
    assert!(!app.editor.as_ref().unwrap().st.history.can_undo());
    app.editor.as_mut().unwrap().apply(ferrocast_core::capture::timeline::EditKind::Style, 1.0, |p| {
        p.style.radius = 3.0;
        Ok(())
    });
    app.ctx.data_mut(|d| d.remove::<Vec<(String, egui::Id, Rect, bool)>>(Id::new("ctl-hits")));
    render(&mut app, vec![]);
    let flags: Vec<bool> = hits_of(&app).iter().filter(|h| h.0 == "small_btn").map(|h| h.3).collect();
    assert_eq!(flags, vec![true, true, false, true], "Undo enabled after an edit: {flags:?}");
}

#[test]
fn split_speed_and_delete_segment_buttons_edit_the_clips() {
    let (mut app, _rx, _dir) = editor_app_in("clips");
    set_tab(&mut app, Tab::Clips);
    render(&mut app, vec![]);
    app.editor.as_mut().unwrap().t = 4.0;
    click_label(&mut app, "Split");
    assert_eq!(editor_proj(&app).trims.len(), 2);
    app.editor.as_mut().unwrap().t = 6.0;
    click_label(&mut app, "4×");
    click_label(&mut app, "Apply to segment");
    let p = editor_proj(&app);
    assert_eq!((p.speeds[0].start, p.speeds[0].end, p.speeds[0].factor), (4.0, 10.0, 4.0));
    let tl = app.editor.as_ref().unwrap().timeline.clone().unwrap();
    assert!((tl.output_duration() - (4.0 + 6.0 / 4.0)).abs() < 1e-9, "the timeline shows the speed: {}", tl.output_duration());
    click_label(&mut app, "Delete segment");
    assert_eq!(editor_proj(&app).trims.len(), 1);
    assert!(editor_proj(&app).speeds.is_empty(), "the speed went with its segment");
    // deleting the last segment is refused inline
    app.editor.as_mut().unwrap().t = 1.0;
    click_label(&mut app, "Delete segment");
    assert_eq!(editor_proj(&app).trims.len(), 1);
    assert!(screen_has(&mut app, "The only segment cannot be deleted."));
}

#[test]
fn zoom_tab_adds_rejects_overlaps_and_deletes() {
    let (mut app, _rx, _dir) = editor_app_in("zoomtab");
    render(&mut app, vec![]);
    app.editor.as_mut().unwrap().t = 6.0;
    click_label(&mut app, "Add at playhead");
    let p = editor_proj(&app);
    assert_eq!(p.zooms.len(), 2);
    assert_eq!((p.zooms[1].mode.as_str(), p.zooms[1].start), ("manual", 6.0));
    // the same spot again is refused with the reason, nothing changes
    click_label(&mut app, "Add at playhead");
    assert_eq!(editor_proj(&app).zooms.len(), 2);
    assert!(screen_has(&mut app, "There is already a zoom at the playhead."));
    // Auto zoom without events says why
    click_label(&mut app, "Auto zoom");
    assert!(screen_has(&mut app, "no cursor events"));
    // the new zoom is selected: its fields show below the list, Delete removes it
    let mut steps = 0;
    while !screen_has(&mut app, "Pick on preview") && steps < 8 {
        scroll_panel(&mut app, -60.0);
        steps += 1;
    }
    assert!(screen_has(&mut app, "Pick on preview"));
    while !screen_has(&mut app, "Delete zoom") && steps < 16 {
        scroll_panel(&mut app, -60.0);
        steps += 1;
    }
    click_label(&mut app, "Delete zoom");
    assert_eq!(editor_proj(&app).zooms.len(), 1);
    scroll_panel(&mut app, 2000.0);
    app.editor.as_mut().unwrap().apply(ferrocast_core::capture::timeline::EditKind::Zoom, 50.0, |p| {
        p.zooms.clear();
        Ok(())
    });
    assert!(screen_has(&mut app, "No zooms."));
}

#[test]
fn clicking_the_preview_while_picking_sets_the_zoom_focus() {
    let (mut app, _rx, _dir) = editor_app_in("pick");
    render(&mut app, vec![]);
    {
        let ed = app.editor.as_mut().unwrap();
        ed.st.selected_zoom = Some("z1".into());
        ed.st.picking = true;
    }
    render(&mut app, vec![]);
    let img: Rect = app.ctx.data(|d| d.get_temp(Id::new("editor-preview-rect"))).expect("preview drawn");
    click_at(&mut app, Pos2::new(img.min.x + img.width() * 0.25, img.center().y));
    let z = editor_proj(&app).zooms[0].clone();
    let f = z.focus.expect("focus set");
    // 16:9 canvas with the default 6 % padding: the video spans x 0.1067..0.8933 of the preview
    assert!((f[0] - 0.1822).abs() < 0.01 && (f[1] - 0.5).abs() < 0.01, "{f:?}");
    assert_eq!(z.mode, "manual", "an edited zoom becomes manual");
    assert!(!app.editor.as_ref().unwrap().st.picking, "picking ends with the click");
    // a click on the padding is refused and keeps picking
    {
        let ed = app.editor.as_mut().unwrap();
        ed.st.picking = true;
    }
    render(&mut app, vec![]);
    click_at(&mut app, Pos2::new(img.min.x + 1.0, img.min.y + 1.0));
    assert!(app.editor.as_ref().unwrap().st.picking);
    assert!(screen_has(&mut app, "Click inside the video"));
}

#[test]
fn style_and_cursor_controls_edit_and_the_preview_is_asked_again() {
    let (mut app, rx, _dir) = editor_app_in("style");
    set_tab(&mut app, Tab::Style);
    render(&mut app, vec![]);
    let first = editor_requests(&rx);
    assert_eq!(first.len(), 1);
    let dir = app.editor.as_ref().unwrap().dir.clone();
    let out = crate::editor_ctl::FrameOut { image: egui::ColorImage::example(), t: 0.0, helper_ms: 1.0, total_ms: 1.0 };
    app.handle_editor(crate::editor_ctl::EditorMsg::Frame { dir: dir.clone(), seq: first[0].seq, result: Ok(out) });
    // the decoded frame has its own aspect, so the preview width changes once: settle that request too
    render(&mut app, vec![]);
    for r in editor_requests(&rx) {
        let out = crate::editor_ctl::FrameOut { image: egui::ColorImage::example(), t: r.key.t, helper_ms: 1.0, total_ms: 1.0 };
        app.handle_editor(crate::editor_ctl::EditorMsg::Frame { dir: dir.clone(), seq: r.seq, result: Ok(out) });
    }
    render(&mut app, vec![]);
    assert!(editor_requests(&rx).is_empty() && !app.editor.as_ref().unwrap().queue.busy(), "settled");
    click_label(&mut app, "Dusk");
    let bg = editor_proj(&app).style.background;
    assert_eq!((bg.kind.as_str(), bg.stops[0].as_str(), bg.angle), ("gradient", "#2b1055", 160.0));
    let cmds: Vec<crate::editor_ctl::WorkerCmd> = rx.try_iter().collect();
    let proj_at =
        cmds.iter().position(|c| matches!(c, crate::editor_ctl::WorkerCmd::Project(p) if p.style.background.stops[0] == "#2b1055"));
    let frame_at = cmds.iter().position(|c| matches!(c, crate::editor_ctl::WorkerCmd::Frame(_)));
    assert!(proj_at.is_some(), "the worker got the edit");
    assert!(frame_at > proj_at, "the frame is rendered again after the edit, behind the project: {proj_at:?} {frame_at:?}");
    click_label(&mut app, "Blur");
    assert_eq!(editor_proj(&app).style.background.kind, "blur");
    click_label(&mut app, "1:1");
    assert_eq!(editor_proj(&app).style.aspect, "1:1");
    set_tab(&mut app, Tab::Cursor);
    render(&mut app, vec![]);
    click_label(&mut app, "Hidden");
    assert_eq!(editor_proj(&app).cursor.style, "hidden");
    click_label(&mut app, "Click ring");
    assert_eq!(editor_proj(&app).cursor.click_effect, "none");
    assert_eq!(app.editor.as_ref().unwrap().st.history.undo_len(), 5, "every discrete click is its own undo step");
}

#[test]
fn esc_flushes_the_pending_edit_without_waiting_for_the_idle_second() {
    let (mut app, _rx, dir) = editor_app_in("esc");
    set_tab(&mut app, Tab::Clips);
    render(&mut app, vec![]);
    app.editor.as_mut().unwrap().t = 2.0;
    click_label(&mut app, "Trim in");
    assert_eq!(ferrocast_core::capture::project::Project::load(&dir).unwrap().trims.len(), 0, "not saved yet: the second has not passed");
    assert!(screen_has(&mut app, "Saving…"));
    render(&mut app, vec![key_event(Key::Escape)]);
    let saved = ferrocast_core::capture::project::Project::load(&dir).unwrap();
    assert_eq!(saved.trims.len(), 1, "Esc wrote it at once");
    assert!(ferrocast_core::capture::timeline::validate(&saved).is_empty());
    assert!(!dir.join("project.json.tmp").exists());
}

#[test]
fn the_view_saves_one_idle_second_after_the_last_edit() {
    let (mut app, _rx, dir) = editor_app_in("idle");
    set_tab(&mut app, Tab::Cursor);
    render(&mut app, vec![]);
    click_label(&mut app, "Hidden");
    render(&mut app, vec![]);
    assert_eq!(ferrocast_core::capture::project::Project::load(&dir).unwrap().cursor.style, "arrow", "nothing yet");
    std::thread::sleep(std::time::Duration::from_millis(1150));
    render(&mut app, vec![]);
    assert_eq!(ferrocast_core::capture::project::Project::load(&dir).unwrap().cursor.style, "hidden", "written by the debounce");
    assert!(screen_has(&mut app, "Saved"));
}

#[test]
fn a_failed_autosave_shows_its_error_in_the_editor_and_leaves_it_dirty() {
    let (mut app, _rx, dir) = editor_app_in("failsave");
    set_tab(&mut app, Tab::Cursor);
    render(&mut app, vec![]);
    let gone = dir.with_file_name("Gone.ferrocast");
    std::fs::rename(&dir, &gone).unwrap();
    click_label(&mut app, "Hidden");
    app.editor.as_mut().unwrap().flush(1e9);
    assert!(screen_has(&mut app, "Could not save project.json"), "the footer carries the error text");
    assert!(app.editor.as_ref().unwrap().st.autosave.pending());
    std::fs::rename(&gone, &dir).unwrap();
}

// ---- MP4 export sheet (T28)

#[test]
fn cmd_e_and_the_button_open_the_export_sheet_and_esc_closes_only_the_sheet() {
    let (mut app, _rx, _dir) = editor_app_in("export-open");
    render(&mut app, vec![]);
    assert!(!app.editor.as_ref().unwrap().export.open);
    assert!(screen_has(&mut app, "Export…"), "the toolbar button");
    render(&mut app, vec![cmd_key(Key::E)]);
    render(&mut app, vec![]);
    let ed = app.editor.as_ref().unwrap();
    assert!(ed.export.open);
    let dest = ed.export.dest.clone().expect("a default destination");
    assert_eq!(dest.file_name().unwrap(), "Demo.mp4", "named after the project, next to it");
    assert!(screen_has(&mut app, "Export video") && screen_has(&mut app, "H.264") && screen_has(&mut app, "60 fps"));
    assert!(screen_has(&mut app, "GIF") && screen_has(&mut app, "MP4"), "the format choice (T29)");
    // Esc closes the sheet and the editor stays
    render(&mut app, vec![key_event(Key::Escape)]);
    assert!(!app.editor.as_ref().unwrap().export.open);
    assert!(matches!(app.view(), View::Editor { .. }), "the editor is still open");
    // the button opens it too
    click_label(&mut app, "Export…");
    render(&mut app, vec![]);
    assert!(app.editor.as_ref().unwrap().export.open);
}

#[test]
fn the_gif_choice_swaps_the_pickers_for_the_gif_limits_and_the_extension() {
    let (mut app, _rx, _dir) = editor_app_in("export-gif");
    render(&mut app, vec![cmd_key(Key::E)]);
    render(&mut app, vec![]);
    assert!(screen_has(&mut app, "60 fps") && screen_has(&mut app, "4K") && screen_has(&mut app, "H.264"));
    click_label(&mut app, "GIF");
    render(&mut app, vec![]);
    let ed = app.editor.as_ref().unwrap();
    assert_eq!(ed.export.format, ferrocast_core::capture::export::Format::Gif);
    assert_eq!(ed.export.dest.as_ref().unwrap().file_name().unwrap(), "Demo.gif");
    for gone in ["60 fps", "30 fps", "4K", "1080p", "H.264", "HEVC", "Quality"] {
        assert!(!screen_has(&mut app, gone), "{gone} is MP4-only");
    }
    for shown in ["480 px", "720 px", "960 px", "10 fps", "15 fps", "Export"] {
        assert!(screen_has(&mut app, shown), "{shown}");
    }
    // the defaults are 720 px and 15 fps and the summary is a GIF-sized plan
    let ed = app.editor.as_ref().unwrap();
    assert_eq!((ed.export.gif_res.label(), ed.export.gif_fps), ("720 px", 15));
    click_label(&mut app, "MP4");
    render(&mut app, vec![]);
    assert_eq!(app.editor.as_ref().unwrap().export.dest.as_ref().unwrap().file_name().unwrap(), "Demo.mp4");
    assert!(screen_has(&mut app, "60 fps") && !screen_has(&mut app, "480 px"));
}

#[test]
fn the_export_sheet_shows_progress_with_eta_and_cancel_stops_it() {
    use crate::editor_export::{ExportMsg, ExportRun};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    let (mut app, _rx, _dir) = editor_app_in("export-progress");
    render(&mut app, vec![cmd_key(Key::E)]);
    render(&mut app, vec![]);
    let cancel = Arc::new(AtomicBool::new(false));
    app.editor.as_mut().unwrap().export.run = Some(ExportRun { cancel: cancel.clone(), done: 0, total: 0, eta: None, cancelling: false });
    let dir = app.editor.as_ref().unwrap().dir.clone();
    app.handle_editor(crate::editor_ctl::EditorMsg::Export { dir, msg: ExportMsg::Progress { done: 42, total: 100, eta: Some(17.2) } });
    assert!(screen_has(&mut app, "42 % · 18 s left"), "progress and ETA");
    assert!(screen_has(&mut app, "Cancel"));
    click_label(&mut app, "Cancel");
    render(&mut app, vec![]);
    assert!(cancel.load(Ordering::Relaxed), "Cancel reaches the export thread's flag");
    assert!(screen_has(&mut app, "Cancelling…"));
}

#[test]
fn a_failed_export_keeps_the_sheet_open_with_the_helpers_message_and_success_toasts() {
    use crate::editor_export::ExportMsg;
    let (mut app, _rx, dir) = editor_app_in("export-result");
    render(&mut app, vec![cmd_key(Key::E)]);
    render(&mut app, vec![]);
    app.handle_editor(crate::editor_ctl::EditorMsg::Export {
        dir: dir.clone(),
        msg: ExportMsg::Failed("The disk is full: No space left".into()),
    });
    assert!(app.editor.as_ref().unwrap().export.open);
    assert!(screen_has(&mut app, "The disk is full: No space left"));
    // a message for another project is ignored
    app.handle_editor(crate::editor_ctl::EditorMsg::Export { dir: "/elsewhere".into(), msg: ExportMsg::Failed("other".into()) });
    assert!(!screen_has(&mut app, "other"));
    // success: the sheet closes and a toast names the file (Finder reveal is skipped for a path that does not exist)
    let out = dir.with_file_name("Done.mp4");
    app.handle_editor(crate::editor_ctl::EditorMsg::Export {
        dir,
        msg: ExportMsg::Done(ferrocast_core::capture::protocol::ExportDone {
            path: out.to_string_lossy().into_owned(),
            bytes: 1,
            duration: 1.0,
            frames: 30,
        }),
    });
    assert!(!app.editor.as_ref().unwrap().export.open);
    assert!(app.toast.as_ref().is_some_and(|(t, _)| t == "Exported Done.mp4"), "{:?}", app.toast);
}

#[test]
fn notes_appearance_maps_each_theme() {
    use ferrocast_macos::notes_theme::Appearance;
    assert_eq!(notes_appearance(Theme::System), Appearance::System);
    assert_eq!(notes_appearance(Theme::Dark), Appearance::Dark);
    assert_eq!(notes_appearance(Theme::Light), Appearance::Light);
}

#[test]
fn startup_theme_reaches_the_notes_windows() {
    let (mut app, _) = notes_tab_app(egui::ThemePreference::Dark);
    fx_taken();
    // App::new loads the saved settings and then calls set_theme
    app.settings.theme = Theme::Light;
    app.set_theme();
    assert_eq!(fx_taken(), vec!["appearance Light".to_string()]);
}

#[test]
fn a_theme_change_in_settings_reaches_the_notes_windows_once() {
    let (mut app, _) = notes_tab_app(egui::ThemePreference::Dark);
    fx_taken();
    for (theme, want) in [(Theme::Dark, "appearance Dark"), (Theme::Light, "appearance Light"), (Theme::System, "appearance System")] {
        let mut s = app.settings.clone();
        s.theme = theme;
        app.commit_settings(s);
        assert_eq!(fx_taken(), vec![want.to_string()], "{theme:?}");
        // the same theme again changes nothing
        let mut s = app.settings.clone();
        s.theme = theme;
        app.commit_settings(s);
        assert!(fx_taken().is_empty(), "unchanged {theme:?}");
    }
}

#[test]
fn notes_and_app_palettes_have_the_same_values() {
    use ferrocast_macos::notes_theme::{Rgba, DARK, LIGHT};
    for (dark, name, notes) in [(true, "dark", &DARK), (false, "light", &LIGHT)] {
        let p = ui::palette(dark);
        let tokens: [(&str, egui::Color32, Rgba); 11] = [
            ("bg", p.bg, notes.bg),
            ("border", p.border, notes.border),
            ("text", p.text, notes.text),
            ("sub", p.sub, notes.sub),
            ("sub_strong", p.sub_strong, notes.sub_strong),
            ("accent_text", p.accent_text, notes.accent_text),
            ("sel", p.sel, notes.sel),
            ("accent", p.accent, notes.accent),
            ("field", p.field, notes.field),
            ("sep", p.sep, notes.sep),
            ("danger", p.danger, notes.danger),
        ];
        for (token, c, n) in tokens {
            let a = c.to_srgba_unmultiplied();
            let b = [n.0, n.1, n.2, n.3];
            for i in 0..4 {
                assert!(a[i].abs_diff(b[i]) <= 1, "{name} token {token}: app {a:?} vs notes {b:?}");
            }
        }
    }
}

// ---- P5: the Transcript tab (egui harness; the helper is never started: the Mac has no Speech grant) ----

mod transcript_tab {
    use super::*;
    use crate::editor_panels::{TranscribeFailure, TranscribeRun};
    use ferrocast_core::capture::project::Project;
    use ferrocast_core::capture::transcript::{Transcript, Word};

    fn speech() -> Transcript {
        let w = |text: &str, start: f64, end: f64| Word { text: text.into(), start, end, confidence: 0.95 };
        Transcript {
            language: "en-US".into(),
            words: vec![
                w("alpha", 0.5, 0.8),
                w("um", 1.0, 1.3),
                w("bravo", 1.5, 1.8),
                w("charlie", 1.9, 2.1),
                w("delta", 2.2, 2.5),
                w("echo", 4.0, 4.4),
            ],
            ..Default::default()
        }
    }

    /// The camera fixture (mic + system) without its trim, so every word is kept at first.
    fn project(transcript: Option<Transcript>) -> Project {
        let mut p = p4_project();
        p.trims.clear();
        p.transcript = transcript;
        p
    }

    fn open(tag: &str, transcript: Option<Transcript>) -> (App, std::path::PathBuf) {
        let (mut app, dir, _rx) = editor_app_for(&format!("p5-{tag}"), project(transcript), &["mic.m4a"]);
        std::mem::forget(_rx); // the worker channel must stay open for `after_change`
        set_tab(&mut app, Tab::Transcript);
        (app, dir)
    }

    /// Every painted text with its own rectangle (a wrapped label's galley starts with the row's
    /// leading space) and its first section format (colour, background, strike-through).
    fn formats(app: &mut App) -> Vec<(String, Rect, egui::TextFormat)> {
        fn walk(s: &egui::Shape, out: &mut Vec<(String, Rect, egui::TextFormat)>) {
            match s {
                egui::Shape::Text(t) => {
                    if let Some(sec) = t.galley.job.sections.first() {
                        let r = Rect::from_min_size(t.pos, t.galley.size());
                        let r = Rect::from_min_max(Pos2::new(r.min.x + sec.leading_space, r.min.y), r.max);
                        out.push((t.galley.text().to_string(), r, sec.format.clone()));
                    }
                }
                egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, out)),
                _ => {}
            }
        }
        let input =
            egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(ui::WIDTH, ui::HEIGHT))), ..Default::default() };
        let ctx = app.ctx.clone();
        let mut output = ctx.run_ui(input, |ui| app.draw(ui));
        output.textures_delta.clear();
        let mut out = vec![];
        output.shapes.iter().for_each(|c| walk(&c.shape, &mut out));
        out
    }

    fn fmt(app: &mut App, word: &str) -> egui::TextFormat {
        formats(app).into_iter().find(|(t, _, _)| t == word).unwrap_or_else(|| panic!("no `{word}` on screen")).2
    }

    fn word_pos(app: &mut App, word: &str) -> Pos2 {
        formats(app).into_iter().find(|(t, _, _)| t == word).unwrap_or_else(|| panic!("no `{word}` on screen")).1.center()
    }

    fn click_word(app: &mut App, word: &str) {
        let p = word_pos(app, word);
        click_at(app, p);
    }

    fn texts(app: &mut App) -> Vec<String> {
        render_texts(app, vec![]).into_iter().map(|(t, _)| t).collect()
    }

    fn shift_click_label(app: &mut App, text: &str) {
        let pos = word_pos(app, text);
        let btn = |pressed| Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::SHIFT };
        render_mods(app, vec![Event::PointerMoved(pos)], Modifiers::SHIFT);
        render_mods(app, vec![btn(true)], Modifiers::SHIFT);
        render_mods(app, vec![btn(false)], Modifiers::SHIFT);
        render(app, vec![]);
    }

    #[test]
    fn a_project_without_audio_and_without_transcript_has_no_transcript_tab() {
        let (mut app, _rx, _dir) = editor_app_in("p5-plain");
        let t = tab_labels(&mut app);
        assert!(!t.iter().any(|x| x == "Transcript"), "{t:?}");
        // audio shows it; so does a transcript without audio
        let (mut app, _dir) = open("tab-audio", None);
        assert!(texts(&mut app).iter().any(|x| x == "Transcript"));
        let mut p = project(Some(speech()));
        p.sources.mic = None;
        p.sources.system = None;
        let (mut app, _d, _rx) = editor_app_for("p5-tab-only-transcript", p, &[]);
        assert!(texts(&mut app).iter().any(|x| x == "Transcript"));
    }

    #[test]
    fn idle_offers_a_language_and_the_transcribe_button() {
        let (mut app, _dir) = open("idle", None);
        let t = texts(&mut app);
        for want in ["Language", "System language", "Transcribe", "Transcription runs on this Mac. Nothing leaves it."] {
            assert!(t.iter().any(|x| x == want), "idle lacks `{want}`: {t:?}");
        }
        assert!(!t.iter().any(|x| x == "Cancel" || x == "Delete selection"));
        // in this test build the helper is not there: the click is answered with a message, nothing starts
        click_label(&mut app, "Transcribe");
        assert!(app.editor.as_ref().unwrap().st.tr.run.is_none());
        assert!(screen_has(&mut app, "Transcription is not available in this build."));
    }

    #[test]
    fn running_shows_progress_and_cancel_sets_the_flag() {
        let (mut app, _dir) = open("running", None);
        let mut run = TranscribeRun::new();
        (run.done, run.total) = (1.0, 4.0);
        let flag = run.cancel.clone();
        app.editor.as_mut().unwrap().st.tr.run = Some(run);
        let t = texts(&mut app);
        assert!(t.iter().any(|x| x == "Transcribing… 25 %") && t.iter().any(|x| x == "Cancel"), "{t:?}");
        assert!(!t.iter().any(|x| x == "Transcribe"), "no second start while running");
        click_label(&mut app, "Cancel");
        assert!(flag.load(std::sync::atomic::Ordering::Relaxed));
        assert!(screen_has(&mut app, "Cancelling…"));
    }

    #[test]
    fn errors_are_clear_and_the_permission_one_offers_the_settings_pane() {
        let (mut app, _dir) = open("error", None);
        app.editor.as_mut().unwrap().st.tr.error = Some(TranscribeFailure::Permission);
        let t = texts(&mut app);
        assert!(
            t.iter().any(|x| x
                == "Speech Recognition permission is needed. Open System Settings > Privacy & Security > Speech Recognition and allow Ferrocast."),
            "{t:?}"
        );
        assert!(t.iter().any(|x| x == "Open Speech Recognition settings") && t.iter().any(|x| x == "Transcribe"), "retry stays available");
        click_label(&mut app, "Open Speech Recognition settings"); // `open_pane` is inert in tests
        let msg = ferrocast_macos::capture::transcription_unavailable_message("Spanish (Spain)");
        app.editor.as_mut().unwrap().st.tr.error = Some(TranscribeFailure::Message(msg.clone()));
        let t = texts(&mut app);
        assert!(t.contains(&msg) && !t.iter().any(|x| x == "Open Speech Recognition settings"), "{t:?}");
    }

    #[test]
    fn with_a_transcript_the_words_flow_and_are_marked() {
        let (mut app, _dir) = open("words", Some(speech()));
        let t = texts(&mut app);
        for want in ["Delete selection", "Remove fillers", "Tighten pauses", "alpha", "um", "bravo", "charlie", "delta", "echo"] {
            assert!(t.iter().any(|x| x == want), "lacks `{want}`: {t:?}");
        }
        assert!(!t.iter().any(|x| x == "Transcribe"), "a transcript replaces the Transcribe button");
        let (plain, filler) = (fmt(&mut app, "alpha"), fmt(&mut app, "um"));
        assert_ne!(plain.color, filler.color, "the filler is marked");
        assert!(plain.strikethrough.width == 0.0 && plain.background == egui::Color32::TRANSPARENT);
        // the word under the playhead is highlighted
        app.editor.as_mut().unwrap().t = 1.6;
        assert_ne!(fmt(&mut app, "bravo").background, egui::Color32::TRANSPARENT);
        assert_eq!(fmt(&mut app, "charlie").background, egui::Color32::TRANSPARENT);
        // the transcript is read-only: no text field anywhere (words are labels)
        assert!(!app.ctx.egui_wants_keyboard_input());
    }

    #[test]
    fn clicking_a_word_seeks_and_a_cut_word_lands_on_the_nearest_kept_time() {
        let (mut app, _dir) = open("seek", Some(speech()));
        click_word(&mut app, "charlie");
        assert!((editor_t(&app) - 1.9).abs() < 1e-9, "{}", editor_t(&app));
        assert_eq!(app.editor.as_ref().unwrap().st.history.undo_len(), 0, "seeking is not an edit");
        // cut "um" (about 0.98-1.32): a click on it lands next to the cut, never inside it
        app.editor.as_mut().unwrap().transcript_cut(crate::editor_panels::CutAction::Fillers, 5.0);
        click_word(&mut app, "um");
        let t = editor_t(&app);
        assert!(app.editor.as_ref().unwrap().timeline.as_ref().unwrap().is_kept(t) && (t - 0.98).abs() < 0.05, "{t}");
    }

    #[test]
    fn cut_words_are_struck_through_and_dimmed() {
        let (mut app, _dir) = open("strike", Some(speech()));
        app.editor.as_mut().unwrap().transcript_cut(crate::editor_panels::CutAction::Fillers, 5.0);
        let (cut, kept) = (fmt(&mut app, "um"), fmt(&mut app, "alpha"));
        assert!(cut.strikethrough.width > 0.0 && kept.strikethrough.width == 0.0);
        assert_ne!(cut.color, kept.color, "dimmed");
        assert!(screen_has(&mut app, "Removed 1 cut"), "{:?}", texts(&mut app));
    }

    #[test]
    fn shift_click_selects_a_range_and_delete_selection_is_one_undo_step() {
        let (mut app, dir) = open("select", Some(speech()));
        let start = editor_proj(&app).to_json();
        click_word(&mut app, "bravo");
        assert!(screen_has(&mut app, "No selection"));
        assert!(
            tab_texts_scrolling(&mut app).iter().any(|t| t.starts_with("Click a word to move the playhead.")),
            "the hint sits under the words"
        );
        shift_click_label(&mut app, "delta");
        assert_eq!(app.editor.as_ref().unwrap().st.tr.sel, Some((2, 4)));
        assert!(screen_has(&mut app, "3 words selected"), "{:?}", texts(&mut app));
        assert_ne!(fmt(&mut app, "charlie").background, egui::Color32::TRANSPARENT, "the selection is highlighted");
        assert_eq!(fmt(&mut app, "alpha").background, egui::Color32::TRANSPARENT);
        click_label(&mut app, "Delete selection");
        let ed = app.editor.as_ref().unwrap();
        assert_eq!(ed.st.history.undo_len(), 1);
        assert_eq!(ed.st.tr.sel, None);
        let tl = ed.timeline.clone().unwrap();
        let w = speech().words;
        let cut: Vec<bool> = w.iter().map(|w| crate::editor_panels::word_is_cut(&tl, w)).collect();
        assert_eq!(cut, [false, false, true, true, true, false], "exactly the selected words");
        assert!(fmt(&mut app, "charlie").strikethrough.width > 0.0);
        app.editor.as_mut().unwrap().flush(1e6);
        assert_eq!(ferrocast_core::capture::project::Project::load(&dir).unwrap().to_json(), editor_proj(&app).to_json(), "autosaved");
        render(&mut app, vec![key_mod_event(Key::Z, Modifiers::COMMAND)]);
        assert_eq!(editor_proj(&app).to_json(), start, "Cmd+Z restores the project");
        assert_eq!(fmt(&mut app, "charlie").strikethrough.width, 0.0);
    }

    #[test]
    fn dragging_across_words_selects_them() {
        let (mut app, _dir) = open("drag", Some(speech()));
        let (a, b) = (word_pos(&mut app, "bravo"), word_pos(&mut app, "delta"));
        let btn = |pos, pressed| Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        render(&mut app, vec![Event::PointerMoved(a)]);
        render(&mut app, vec![btn(a, true)]);
        render(&mut app, vec![Event::PointerMoved(Pos2::new((a.x + b.x) / 2.0, a.y))]);
        render(&mut app, vec![Event::PointerMoved(b)]);
        render(&mut app, vec![btn(b, false)]);
        render(&mut app, vec![]);
        assert_eq!(app.editor.as_ref().unwrap().st.tr.sel, Some((2, 4)), "{:?}", app.editor.as_ref().unwrap().st.tr.sel);
        assert_eq!(app.editor.as_ref().unwrap().st.history.undo_len(), 0, "selecting edits nothing");
    }

    #[test]
    fn remove_fillers_and_tighten_pauses_each_report_and_undo_once() {
        let (mut app, dir) = open("actions", Some(speech()));
        let start = editor_proj(&app).to_json();
        click_label(&mut app, "Remove fillers");
        assert_eq!(app.editor.as_ref().unwrap().st.history.undo_len(), 1);
        assert!(screen_has(&mut app, "Removed 1 cut · 0.3 s"), "{:?}", texts(&mut app));
        click_label(&mut app, "Remove fillers");
        assert_eq!(app.editor.as_ref().unwrap().st.history.undo_len(), 1);
        assert!(screen_has(&mut app, "No filler words found."));
        click_label(&mut app, "Tighten pauses");
        assert_eq!(app.editor.as_ref().unwrap().st.history.undo_len(), 2);
        assert!(screen_has(&mut app, "Removed "), "{:?}", texts(&mut app));
        click_label(&mut app, "Tighten pauses");
        assert!(screen_has(&mut app, "No long pauses to tighten."));
        assert_eq!(app.editor.as_ref().unwrap().st.history.undo_len(), 2);
        // autosave writes the transcript together with the cuts
        app.editor.as_mut().unwrap().flush(1e6);
        let saved = ferrocast_core::capture::project::Project::load(&dir).unwrap();
        assert_eq!(saved.transcript, Some(speech()));
        assert!(!saved.trims.is_empty() || saved.to_json() != start, "the cuts are on disk");
        render(&mut app, vec![key_mod_event(Key::Z, Modifiers::COMMAND)]);
        render(&mut app, vec![key_mod_event(Key::Z, Modifiers::COMMAND)]);
        assert_eq!(editor_proj(&app).to_json(), start, "one Cmd+Z per action");
    }

    #[test]
    fn a_finished_transcription_shows_the_words_and_autosave_writes_them() {
        let (mut app, dir) = open("finish", None);
        app.handle_editor(crate::editor_ctl::EditorMsg::Transcribe {
            dir: dir.clone(),
            msg: crate::editor_panels::TranscribeMsg::Done(Box::new(speech())),
        });
        let t = texts(&mut app);
        assert!(t.iter().any(|x| x == "alpha") && t.iter().any(|x| x == "Remove fillers"), "{t:?}");
        app.editor.as_mut().unwrap().flush(1e6);
        assert_eq!(
            ferrocast_core::capture::project::Project::load(&dir).unwrap().transcript,
            Some(speech()),
            "autosave writes `transcript`"
        );
        // a message for another project is ignored
        app.handle_editor(crate::editor_ctl::EditorMsg::Transcribe {
            dir: "/elsewhere".into(),
            msg: crate::editor_panels::TranscribeMsg::Failed(TranscribeFailure::Permission),
        });
        assert!(app.editor.as_ref().unwrap().st.tr.error.is_none());
    }
}

// ---- Design pass: the editor panel's layout, at the real panel width ----

mod panel_design {
    use super::*;
    use crate::editor_panels::{chip_rows, TranscribeFailure, TranscribeRun, HIT_H};
    use ferrocast_core::capture::project::Project;
    use ferrocast_core::capture::transcript::{Transcript, Word};

    fn transcript() -> Transcript {
        let words = "So today I want to show you how the new export works um and why it is faster than before you know the first thing is the preview"
            .split_whitespace()
            .enumerate()
            .map(|(i, w)| Word { text: w.into(), start: 1.0 + i as f64 * 0.25, end: 1.2 + i as f64 * 0.25, confidence: 0.9 })
            .collect();
        Transcript { language: "en-US".into(), words, ..Default::default() }
    }

    fn open(tag: &str, tab: Tab, tr: Option<Transcript>) -> App {
        let mut p: Project = p4_project();
        p.trims.clear();
        p.transcript = tr;
        let (mut app, _dir, rx) = editor_app_for(&format!("design-{tag}"), p, &["mic.m4a", "camera.mov"]);
        std::mem::forget(rx);
        set_tab(&mut app, tab);
        app
    }

    fn panel_rect(app: &App) -> Rect {
        app.ctx.data(|d| d.get_temp::<Rect>(Id::new("editor-panel-rect"))).expect("the panel was drawn")
    }

    #[test]
    fn chip_rows_balance_and_fill_the_width() {
        // the seven tabs at 12.5 pt: Zoom Cursor Style Clips Camera Audio Transcript
        let w = [27.0, 37.0, 27.0, 25.0, 42.0, 30.0, 56.0];
        let rows = chip_rows(&w, 236.0, 4.0, 9.0);
        assert_eq!(rows.iter().map(Vec::len).collect::<Vec<_>>(), [4, 3], "7 tabs wrap as 4 + 3, not 6 + 1");
        for (row, texts) in rows.iter().zip([&w[..4], &w[4..]]) {
            let total: f32 = row.iter().sum::<f32>() + 4.0 * (row.len() as f32 - 1.0);
            assert!((total - 236.0).abs() < 0.01, "a row is exactly as wide as the panel: {total}");
            assert!(row.iter().zip(texts).all(|(c, t)| *c >= t + 18.0), "a chip is never narrower than its text: {row:?}");
        }
        // the four basic tabs share one row, five tabs split 3 + 2
        assert_eq!(chip_rows(&w[..4], 236.0, 4.0, 9.0).len(), 1);
        assert_eq!(chip_rows(&w[..5], 236.0, 4.0, 9.0).iter().map(Vec::len).collect::<Vec<_>>(), [3, 2]);
        // a panel too narrow for two chips puts each on its own row at full width; nothing is lost
        let narrow = chip_rows(&w, 60.0, 4.0, 9.0);
        assert_eq!(narrow.len(), 7);
        assert!(narrow.iter().all(|r| r == &vec![60.0]));
        assert!(chip_rows(&[], 236.0, 4.0, 9.0).is_empty());
    }

    #[test]
    fn clickable_rows_are_at_least_24_points_tall() {
        const { assert!(HIT_H >= 24.0) };
    }

    #[test]
    fn the_seven_tabs_sit_in_two_even_rows_inside_the_panel() {
        let mut app = open("tabs", Tab::Camera, Some(transcript()));
        let texts = render_texts(&mut app, vec![]);
        let panel = panel_rect(&app);
        let tabs: Vec<&(String, Rect)> = texts
            .iter()
            .filter(|(t, r)| {
                ["Zoom", "Cursor", "Style", "Clips", "Camera", "Audio", "Transcript"].contains(&t.as_str()) && r.max.y < panel.min.y + 80.0
            })
            .collect();
        assert_eq!(tabs.len(), 7, "{tabs:?}");
        let mut ys: Vec<i32> = tabs.iter().map(|(_, r)| r.center().y.round() as i32).collect();
        ys.dedup();
        assert_eq!(ys.len(), 2, "two rows: {ys:?}");
        assert!(ys[1] - ys[0] >= 24, "rows are at least a hit target apart: {ys:?}");
        for (t, r) in &tabs {
            assert!(r.min.x >= panel.min.x && r.max.x <= panel.max.x, "{t} inside the panel {panel:?}: {r:?}");
        }
    }

    /// Every state of the three tabs, dark and light: nothing leaves the panel sideways and no two
    /// visible texts overlap.
    #[test]
    fn every_state_fits_the_panel_without_overlap() {
        for theme in [egui::ThemePreference::Dark, egui::ThemePreference::Light] {
            let scenarios: Vec<(&str, App)> = vec![
                ("camera", open("c", Tab::Camera, None)),
                ("audio", open("a", Tab::Audio, None)),
                ("transcript idle", open("ti", Tab::Transcript, None)),
                ("transcript error", {
                    let mut a = open("te", Tab::Transcript, None);
                    a.editor.as_mut().unwrap().st.tr.error = Some(TranscribeFailure::Permission);
                    a
                }),
                ("transcript running", {
                    let mut a = open("tr", Tab::Transcript, None);
                    let mut run = TranscribeRun::new();
                    (run.done, run.total) = (1.0, 3.0);
                    a.editor.as_mut().unwrap().st.tr.run = Some(run);
                    a
                }),
                ("transcript words", open("tw", Tab::Transcript, Some(transcript()))),
            ];
            for (name, mut app) in scenarios {
                app.ctx.set_theme(theme);
                for _ in 0..2 {
                    render(&mut app, vec![]);
                }
                let texts = render_texts(&mut app, vec![]);
                let panel = panel_rect(&app);
                let visible: Vec<&(String, Rect)> = texts
                    .iter()
                    .filter(|(t, r)| !t.is_empty() && r.min.x >= panel.min.x - 40.0 && r.max.y <= panel.max.y && r.min.y >= panel.min.y)
                    .collect();
                assert!(visible.len() > 6, "{name}: something is drawn: {visible:?}");
                let words = name == "transcript words";
                for (t, r) in &visible {
                    assert!(
                        r.min.x >= panel.min.x - 0.5 && r.max.x <= panel.max.x + 0.5,
                        "{name} {theme:?}: `{t}` leaves the panel {panel:?}: {r:?}"
                    );
                }
                if words {
                    continue; // wrapped word labels carry their leading space in the rect
                }
                for (i, (ta, ra)) in visible.iter().enumerate() {
                    for (tb, rb) in &visible[i + 1..] {
                        let hit = ra.intersect(*rb);
                        assert!(hit.width() <= 0.5 || hit.height() <= 0.5, "{name} {theme:?}: `{ta}` {ra:?} overlaps `{tb}` {rb:?}");
                    }
                }
            }
        }
    }

    /// The selection line keeps its place and height, so the words never jump when a selection appears.
    #[test]
    fn the_word_flow_does_not_move_when_a_selection_appears() {
        let mut app = open("sel", Tab::Transcript, Some(transcript()));
        let status = label_rect(&mut app, "No selection");
        app.editor.as_mut().unwrap().st.tr.sel = Some((2, 4));
        let selected = label_rect(&mut app, "3 words selected · 0:01.50–0:02.20");
        assert_eq!(status.min.y, selected.min.y, "the count replaces `No selection` on the same line");
        assert_eq!(status.height(), selected.height());
    }

    #[test]
    fn the_new_colours_read_on_both_palettes() {
        let t = |c: Color32| [c.r() as f32, c.g() as f32, c.b() as f32];
        for dark in [true, false] {
            let pal = palette(dark);
            for backdrop in [0.0_f32, 255.0] {
                let panel = over(window_over(&pal, backdrop), pal.field);
                let card = over(panel, pal.field);
                let chip = over(panel, pal.field);
                let pill = over(panel, pal.field.gamma_multiply(1.6));
                let on_chip = over(panel, pal.accent.gamma_multiply(crate::editor_panels::ACCENT_TINT));
                let selected = over(card, pal.accent.gamma_multiply(0.38));
                let playhead = over(card, pal.accent.gamma_multiply(0.20));
                let filler = crate::editor_panels::filler_color(dark);
                let danger_card = over(panel, pal.danger.gamma_multiply(0.12));
                let rows = [
                    ("tab text", contrast(t(pal.sub_strong), chip), 4.5),
                    ("selected tab text", contrast(t(pal.accent_text), on_chip), 4.5),
                    ("secondary button text", contrast(t(pal.text), pill), 4.5),
                    ("mute pill text", contrast(t(pal.sub_strong), over(panel, pal.field.gamma_multiply(1.3))), 4.5),
                    ("muted pill text", contrast(t(pal.accent_text), on_chip), 4.5),
                    ("word on selection", contrast(t(pal.text), selected), 4.5),
                    ("word on playhead tint", contrast(t(pal.text), playhead), 4.5),
                    ("filler word", contrast(t(filler), card), 4.5),
                    ("cut word", contrast(t(pal.sub), card), 3.0),
                    ("error text", contrast(t(pal.text), danger_card), 4.5),
                    ("primary button", contrast(t(Color32::WHITE), t(pal.accent)), 3.0),
                ];
                for (what, got, want) in rows {
                    assert!(got >= want, "dark={dark} backdrop={backdrop}: {what} is {got:.2}, want >= {want}");
                }
            }
        }
    }
}

// ---- Design review: an offscreen software renderer for the egui harness ----
//
// `FC_SHOTS_DIR=/some/dir cargo test -p ferrocast shots -- --nocapture` writes one PNG per editor tab and state
// (dark and light, at 2x). Without the variable the tests do nothing. The rasteriser is only a
// triangle filler over `Context::tessellate`: enough to see spacing, colour and text without a GPU.

mod shots {
    use super::*;
    use egui::epaint::Primitive;
    use std::collections::HashMap;

    /// Frame buffer + the managed textures (the font atlas) collected from every frame's delta.
    pub struct Shot {
        textures: HashMap<egui::TextureId, (usize, usize, Vec<egui::Color32>)>,
    }

    impl Shot {
        pub fn new() -> Self {
            Shot { textures: HashMap::new() }
        }

        fn absorb(&mut self, delta: &egui::TexturesDelta) {
            for (id, d) in delta.set.iter().flat_map(|(id, ds)| ds.iter().map(move |d| (id, d))) {
                let egui::ImageData::Color(img) = &d.image;
                let [w, h] = img.size;
                match d.pos {
                    None => {
                        self.textures.insert(*id, (w, h, img.pixels.clone()));
                    }
                    Some([x, y]) => {
                        if let Some((tw, _, px)) = self.textures.get_mut(id) {
                            for row in 0..h {
                                for col in 0..w {
                                    px[(y + row) * *tw + x + col] = img.pixels[row * w + col];
                                }
                            }
                        }
                    }
                }
            }
        }

        /// Run one frame of the app and paint it; `None` for frames that only warm up the fonts.
        pub fn frame(&mut self, app: &mut App, size: Vec2, paint: bool, bg: egui::Color32) -> Option<image::RgbaImage> {
            let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), ..Default::default() };
            let ctx = app.ctx.clone();
            let mut out = ctx.run_ui(input, |ui| app.draw(ui));
            self.absorb(&out.textures_delta);
            out.textures_delta.clear();
            if !paint {
                return None;
            }
            let ppp = out.pixels_per_point;
            let prims = ctx.tessellate(out.shapes, ppp);
            let ss = 2.0;
            let ppp = ppp * ss;
            let (w, h) = ((size.x * ppp) as usize, (size.y * ppp) as usize);
            let mut fb = vec![[bg.r() as f32, bg.g() as f32, bg.b() as f32]; w * h];
            for p in prims {
                let Primitive::Mesh(m) = p.primitive else { continue };
                let Some((tw, th, tex)) = self.textures.get(&m.texture_id) else { continue };
                let clip = p.clip_rect;
                let (cx0, cy0) = (((clip.min.x * ppp).floor().max(0.0)) as usize, ((clip.min.y * ppp).floor().max(0.0)) as usize);
                let (cx1, cy1) = (((clip.max.x * ppp).ceil() as usize).min(w), ((clip.max.y * ppp).ceil() as usize).min(h));
                for t in m.indices.chunks_exact(3) {
                    let v = [&m.vertices[t[0] as usize], &m.vertices[t[1] as usize], &m.vertices[t[2] as usize]];
                    let pt = |i: usize| (v[i].pos.x * ppp, v[i].pos.y * ppp);
                    let (a, b, c) = (pt(0), pt(1), pt(2));
                    let area = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
                    if area.abs() < 1e-6 {
                        continue;
                    }
                    let x0 = (a.0.min(b.0).min(c.0).floor().max(cx0 as f32)) as usize;
                    let x1 = ((a.0.max(b.0).max(c.0).ceil()).min(cx1 as f32)) as usize;
                    let y0 = (a.1.min(b.1).min(c.1).floor().max(cy0 as f32)) as usize;
                    let y1 = ((a.1.max(b.1).max(c.1).ceil()).min(cy1 as f32)) as usize;
                    for y in y0..y1 {
                        for x in x0..x1 {
                            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                            let w0 = ((b.0 - px) * (c.1 - py) - (b.1 - py) * (c.0 - px)) / area;
                            let w1 = ((c.0 - px) * (a.1 - py) - (c.1 - py) * (a.0 - px)) / area;
                            let w2 = 1.0 - w0 - w1;
                            if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                                continue;
                            }
                            let u = w0 * v[0].uv.x + w1 * v[1].uv.x + w2 * v[2].uv.x;
                            let vv = w0 * v[0].uv.y + w1 * v[1].uv.y + w2 * v[2].uv.y;
                            let tx = ((u * *tw as f32) as usize).min(tw - 1);
                            let ty = ((vv * *th as f32) as usize).min(th - 1);
                            let tc = tex[ty * tw + tx];
                            let col = |f: fn(&egui::epaint::Vertex) -> u8| w0 * f(v[0]) as f32 + w1 * f(v[1]) as f32 + w2 * f(v[2]) as f32;
                            let vc = [col(|v| v.color.r()), col(|v| v.color.g()), col(|v| v.color.b()), col(|v| v.color.a())];
                            let s = [
                                vc[0] * tc.r() as f32 / 255.0,
                                vc[1] * tc.g() as f32 / 255.0,
                                vc[2] * tc.b() as f32 / 255.0,
                                vc[3] * tc.a() as f32 / 255.0,
                            ];
                            let d = &mut fb[y * w + x];
                            let k = 1.0 - s[3] / 255.0;
                            *d = [s[0] + d[0] * k, s[1] + d[1] * k, s[2] + d[2] * k];
                        }
                    }
                }
            }
            let (ow, oh) = (w / 2, h / 2);
            let mut img = image::RgbaImage::new(ow as u32, oh as u32);
            for y in 0..oh {
                for x in 0..ow {
                    let mut acc = [0.0f32; 3];
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let p = fb[(y * 2 + dy) * w + x * 2 + dx];
                        (0..3).for_each(|k| acc[k] += p[k] / 4.0);
                    }
                    img.put_pixel(
                        x as u32,
                        y as u32,
                        image::Rgba([acc[0].min(255.0) as u8, acc[1].min(255.0) as u8, acc[2].min(255.0) as u8, 255]),
                    );
                }
            }
            Some(img)
        }
    }

    pub fn dir() -> Option<std::path::PathBuf> {
        std::env::var_os("FC_SHOTS_DIR").map(std::path::PathBuf::from).inspect(|d| std::fs::create_dir_all(d).unwrap())
    }

    /// Paint the app as it is and write `<dir>/<name>-<dark|light>.png`, cropped to `crop` (points) when given.
    pub fn snap(app: &mut App, name: &str, dark: bool, crop: Option<Rect>) {
        let Some(dir) = dir() else { return };
        app.ctx.set_theme(if dark { egui::ThemePreference::Dark } else { egui::ThemePreference::Light });
        app.ctx.set_pixels_per_point(2.0);
        let mut shot = Shot::new();
        let height = std::env::var("FC_SHOTS_H").ok().and_then(|h| h.parse().ok()).unwrap_or(720.0);
        let size = Vec2::new(ui::WIDTH, height);
        let bg = if dark { egui::Color32::from_gray(40) } else { egui::Color32::from_gray(222) };
        for _ in 0..4 {
            shot.frame(app, size, false, bg);
        }
        let mut img = shot.frame(app, size, true, bg).unwrap();
        if let Some(c) = crop {
            img = image::imageops::crop_imm(
                &img,
                (c.min.x * 2.0) as u32,
                (c.min.y * 2.0) as u32,
                (c.width() * 2.0) as u32,
                (c.height() * 2.0) as u32,
            )
            .to_image();
        }
        img.save(dir.join(format!("{name}-{}.png", if dark { "dark" } else { "light" }))).unwrap();
    }

    /// The right-hand editor panel (the draw code places it at the right edge).
    pub fn panel_crop() -> Rect {
        Rect::from_min_max(
            Pos2::new(ui::WIDTH - crate::editor_panels::PANEL_W - 40.0, 40.0),
            Pos2::new(ui::WIDTH, 560.0f32.min(std::env::var("FC_SHOTS_H").ok().and_then(|h| h.parse().ok()).unwrap_or(720.0))),
        )
    }

    fn word(text: &str, start: f64, end: f64) -> ferrocast_core::capture::transcript::Word {
        ferrocast_core::capture::transcript::Word { text: text.into(), start, end, confidence: 0.95 }
    }

    fn long_transcript() -> ferrocast_core::capture::transcript::Transcript {
        let text = "So today I want to show you how the new export works um and why it is faster than before \
                    you know the first thing is the preview which uh renders in real time and then we just press the button";
        let mut t = 1.0;
        let words = text
            .split_whitespace()
            .map(|w| {
                let x = word(w, t, t + 0.25);
                t += 0.3;
                x
            })
            .collect();
        ferrocast_core::capture::transcript::Transcript { language: "en-US".into(), words, ..Default::default() }
    }

    fn open(tag: &str, tab: Tab, tr: Option<ferrocast_core::capture::transcript::Transcript>) -> App {
        let mut p = p4_project();
        p.trims.clear();
        p.transcript = tr;
        let (mut app, _dir, rx) = editor_app_for(&format!("shots-{tag}"), p, &["mic.m4a", "system.m4a", "camera.mov"]);
        std::mem::forget(rx);
        set_tab(&mut app, tab);
        app
    }

    #[test]
    fn editor_panel_shots() {
        if dir().is_none() {
            return;
        }
        use crate::editor_panels::{TranscribeFailure, TranscribeRun};
        for dark in [true, false] {
            let crop = Some(panel_crop());
            let mut app = open("cam", Tab::Camera, None);
            snap(&mut app, "camera", dark, crop);
            let mut app = open("aud", Tab::Audio, None);
            snap(&mut app, "audio", dark, crop);
            let mut app = open("idle", Tab::Transcript, None);
            snap(&mut app, "transcript-idle", dark, crop);
            let mut app = open("err", Tab::Transcript, None);
            app.editor.as_mut().unwrap().st.tr.error = Some(TranscribeFailure::Permission);
            snap(&mut app, "transcript-error", dark, crop);
            let mut app = open("run", Tab::Transcript, None);
            let mut run = TranscribeRun::new();
            (run.done, run.total) = (1.0, 4.0);
            app.editor.as_mut().unwrap().st.tr.run = Some(run);
            snap(&mut app, "transcript-running", dark, crop);
            let mut app = open("words", Tab::Transcript, Some(long_transcript()));
            app.editor.as_mut().unwrap().t = 3.1;
            app.editor.as_mut().unwrap().st.tr.sel = Some((6, 9));
            app.editor.as_mut().unwrap().transcript_cut(crate::editor_panels::CutAction::Fillers, 5.0);
            app.editor.as_mut().unwrap().st.tr.sel = Some((6, 9));
            snap(&mut app, "transcript-words", dark, crop);
            let mut app = open("fillers", Tab::Transcript, Some(long_transcript()));
            app.editor.as_mut().unwrap().t = 5.0;
            snap(&mut app, "transcript-fillers", dark, crop);
            let mut app = open("zoom", Tab::Style, None);
            snap(&mut app, "style", dark, crop);
            let mut app = open("export", Tab::Zoom, None);
            app.editor.as_mut().unwrap().export.open = true;
            snap(&mut app, "export", dark, None);
            for (name, camera, state) in [
                (
                    "settings-sources-failed",
                    false,
                    crate::record_ctl::DevicesState::Failed("Screen capture is not available in this build.".into()),
                ),
                (
                    "settings-sources-ready",
                    true,
                    crate::record_ctl::DevicesState::Ready(ferrocast_core::capture::protocol::Devices {
                        cameras: vec![ferrocast_core::capture::protocol::CameraDevice {
                            id: "0x1".into(),
                            name: "FaceTime HD Camera".into(),
                            position: "unspecified".into(),
                            default: true,
                        }],
                        microphones: vec![ferrocast_core::capture::protocol::MicrophoneDevice {
                            id: "m1".into(),
                            name: "Studio Mic".into(),
                            default: true,
                        }],
                    }),
                ),
            ] {
                let mut app = settings_app(CAPTURE_TAB, egui::ThemePreference::Dark);
                app.settings.capture.camera = camera;
                app.rt.devices = state;
                settle(&mut app);
                scroll_settings(&mut app, 330.0);
                snap(&mut app, name, dark, None);
            }
        }
    }
}
