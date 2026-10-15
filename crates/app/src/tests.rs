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
        runs: HashMap::new(),
        next_run: 1,
        inline: HashMap::new(),
        focus_first: false,
        dark: true,
        started: true,
        act_item: None,
        scratch: String::new(),
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
