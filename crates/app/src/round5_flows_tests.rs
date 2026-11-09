//! Round 5, flows + macOS group (child module of `tests`).
use super::*;

/// One frame on `ctx` with Ctrl held and `events`; egui marks a press of a key that is already
/// down as an auto-repeat, so reusing the context across frames is what a held key looks like.
fn frame(ctx: &Context, events: Vec<Event>, acts: &[Action]) -> Option<Action> {
    let mut events = events;
    events.insert(0, Event::ModifiersChanged(CTRL));
    let input = egui::RawInput { events, ..Default::default() };
    let mut hit = None;
    let mut out = ctx.run_ui(input, |ui| {
        hit = ui.ctx().input_mut(|i| draw::pick_shortcut_action(i, acts));
    });
    out.textures_delta.clear();
    hit
}

fn press(key: Key) -> Event {
    Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: CTRL }
}

#[test]
fn a_held_delete_shortcut_runs_once_and_its_repeats_are_swallowed() {
    let del = vec![Action::new("Delete Entry", Act::ClipDelete("a".into())).key(CTRL, Key::X).danger()];
    let ctx = Context::default();
    assert!(frame(&ctx, vec![press(Key::X)], &del).is_some(), "a fresh press runs");
    assert!(frame(&ctx, vec![press(Key::X)], &del).is_none(), "the auto-repeat does not");
    assert!(frame(&ctx, vec![press(Key::X)], &del).is_none(), "nor the next one");
    let release = Event::Key { key: Key::X, physical_key: None, pressed: false, repeat: false, modifiers: CTRL };
    frame(&ctx, vec![release], &del);
    assert!(frame(&ctx, vec![press(Key::X)], &del).is_some(), "a new press after release runs again");

    let safe = vec![Action::new("Copy", Act::ClipDelete("a".into())).key(CTRL, Key::X)];
    let ctx = Context::default();
    assert!(frame(&ctx, vec![press(Key::X)], &safe).is_some());
    assert!(frame(&ctx, vec![press(Key::X)], &safe).is_some(), "harmless shortcuts keep repeating");
}

#[test]
fn the_home_folder_becomes_a_tilde_only_at_the_start_of_a_path() {
    let home = std::path::Path::new("/Users/ann");
    let t = |p: &str| tilde_home(std::path::Path::new(p), Some(home));
    assert_eq!(t("/Users/ann"), "~");
    assert_eq!(t("/Users/ann/Documents"), "~/Documents");
    assert_eq!(t("/Volumes/x/Users/ann/Documents"), "/Volumes/x/Users/ann/Documents", "not in the middle");
    assert_eq!(t("/Users/ann2/Documents"), "/Users/ann2/Documents", "not a sibling that shares the prefix");
    assert_eq!(tilde_home(std::path::Path::new("/tmp/a"), None), "/tmp/a");
    assert_eq!(tilde_home(std::path::Path::new("/tmp/a"), Some(std::path::Path::new(""))), "/tmp/a", "an empty home matches nothing");
}

fn png_bytes() -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    image::RgbaImage::from_pixel(4, 4, image::Rgba([1, 2, 3, 255])).write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

#[test]
fn app_icon_textures_and_prefetched_bytes_are_capped() {
    let mut app = fixture();
    let ctx = app.ctx.clone();
    let png = png_bytes();
    let total = ui::ICON_TEXTURES + 5;
    for i in 0..total {
        app.icons.accept_prefetched(format!("app:/a/{i}.app"), png.clone());
        assert!(app.icons.prefetched.len() <= ui::PREFETCHED_MAX, "fetched bytes pile up");
        app.icons.new_frame();
        app.icons.texture(&ctx, &Icon::App(format!("/a/{i}.app")));
    }
    let apps = app.icons.map.keys().filter(|k| k.starts_with("app:")).count();
    assert_eq!(apps, ui::ICON_TEXTURES, "the texture cache never grows past its cap");
    assert!(!app.icons.map.contains_key("app:/a/0.app"), "the oldest was dropped");
    assert!(app.icons.map.contains_key(&format!("app:/a/{}.app", total - 1)));
    // bytes for icons nobody asks for are bounded too
    for i in 0..(ui::PREFETCHED_MAX * 2) {
        app.icons.accept_prefetched(format!("app:/b/{i}.app"), png.clone());
    }
    assert!(app.icons.prefetched.len() <= ui::PREFETCHED_MAX);
}

fn src(file: &str) -> String {
    let s = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(file)).unwrap();
    s[..s.find("\n#[cfg(test)]").unwrap_or(s.len())].to_string()
}

#[test]
fn opening_language_settings_does_not_wait_for_open_and_no_open_child_is_left_unreaped() {
    let services = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/services.rs")).unwrap();
    let at = services.find("pub fn open_language_settings").unwrap();
    let body = &services[at..services[at..].find("\n}\n").map_or(services.len(), |e| at + e)];
    assert!(!body.contains(".status()"), "open_language_settings runs on the UI thread: {body}");
    for f in ["app.rs", "draw.rs", "notion_io.rs", "capture_ctl.rs", "notes_ctl.rs", "services.rs"] {
        for (n, line) in src(f).lines().enumerate() {
            assert!(
                !(line.contains("\"/usr/bin/open\"") && line.contains(".spawn()") && !line.contains("spawn_detached")),
                "{f}:{} spawns open without ever waiting for it: {line}",
                n + 1
            );
        }
    }
}

#[test]
fn a_reaped_child_is_collected_without_blocking_the_caller() {
    let child = std::process::Command::new("/usr/bin/true").spawn().unwrap();
    let pid = child.id();
    let t = Instant::now();
    reap_in_background(child);
    assert!(t.elapsed() < Duration::from_millis(500));
    let t = Instant::now();
    // once waited on, the pid is no longer a zombie of ours: `ps` shows no Z state
    loop {
        let out = std::process::Command::new("/bin/ps").args(["-o", "stat=", "-p", &pid.to_string()]).output().unwrap();
        if !String::from_utf8_lossy(&out.stdout).trim().starts_with('Z') {
            break;
        }
        assert!(t.elapsed() < Duration::from_secs(5), "the child stayed a zombie");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn pruning_clipboard_images_waits_for_a_copy_that_is_being_recorded() {
    let app = fixture();
    let dir = app.store.images_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("new.png");
    // `record` holds the list lock while it writes the image and only then adds the entry
    let guard = app.shared.clipboard.lock().unwrap();
    std::fs::write(&file, b"png").unwrap();
    let shared = app.shared.clone();
    let pruner = std::thread::spawn(move || shared.prune_clip_images());
    std::thread::sleep(Duration::from_millis(150));
    assert!(file.exists(), "the prune must not delete the image of a copy that is not in the list yet");
    drop(guard);
    pruner.join().unwrap();
    assert!(!file.exists(), "once the list is settled, an unreferenced image is pruned");
}

#[test]
fn deleting_a_clipboard_entry_writes_the_history_off_the_calling_thread() {
    let mut app = fixture();
    let entry = |id: &str| ClipEntry {
        id: id.into(),
        kind: ClipKind::Text,
        content: format!("text {id}"),
        title: None,
        copied_at: 1,
        pinned: false,
        source_app: None,
        times_copied: 1,
        width: 0,
        height: 0,
    };
    *app.shared.clipboard.lock().unwrap() = vec![entry("a"), entry("b")];
    app.shared.save_clipboard();
    app.perform(Act::ClipDelete("a".into()), true);
    assert!(app.shared.clip_dirty.load(std::sync::atomic::Ordering::Relaxed), "sync is told at once");
    let t = Instant::now();
    loop {
        let saved: Vec<ClipEntry> = app.store.load("clipboard.json");
        if saved.len() == 1 && saved[0].id == "b" {
            break;
        }
        assert!(t.elapsed() < Duration::from_secs(5), "the history file never caught up: {saved:?}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn the_cannot_reach_message_names_the_server_that_was_contacted() {
    use ferrocast_core::models::AiProvider as P;
    let mut s = ferrocast_core::models::Settings { ai_base_url: "http://localhost:11434/v1".into(), ..Default::default() };
    let msg = |s: &ferrocast_core::models::Settings| services::unreachable_message(s, "curl: (7) Failed to connect to x").unwrap();
    s.ai_provider = P::OpenAiCompatible;
    assert!(msg(&s).contains("localhost:11434") && msg(&s).contains("Ollama"));
    s.ai_provider = P::Anthropic;
    assert!(msg(&s).contains("api.anthropic.com") && !msg(&s).contains("localhost") && !msg(&s).contains("Ollama"), "{}", msg(&s));
    s.ai_provider = P::ChatGpt;
    assert!(msg(&s).contains("chatgpt.com") && !msg(&s).contains("localhost"), "{}", msg(&s));
    assert!(services::unreachable_message(&s, "curl: (28) timed out").is_none());
    ferrocast_core::i18n::with_language(ferrocast_core::i18n::Lang::Es, || {
        s.ai_provider = P::Anthropic;
        assert!(msg(&s).starts_with("No se puede llegar a api.anthropic.com"), "{}", msg(&s));
    });
}

#[test]
fn the_chatgpt_device_code_is_put_on_the_clipboard_as_concealed() {
    let services = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/services.rs")).unwrap();
    let at = services.find("pub fn chatgpt_sign_in_device").unwrap();
    let body = &services[at..at + 2500];
    assert!(body.contains("write_text_concealed(&d.user_code)"), "the code would be recorded in the clipboard history");
    assert!(!body.contains("clipboard::write_text(&d.user_code)"));
}

#[test]
fn a_launch_that_cannot_start_node_keeps_the_running_command_and_its_screens() {
    let mut app = fixture();
    app.settings.run_raycast_extensions = true;
    let dir = std::env::temp_dir().join(format!("ferrocast-r5-ext-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("x.js"), "").unwrap();
    let mut m = ferrocast_core::extensions::manifest::parse_manifest(r#"{"name":"e","commands":[{"name":"x","mode":"view"}]}"#).unwrap();
    m.dir = dir.clone();
    app.ext.test_set_manifests(vec![m]);
    app.ext.test_install_session(1, true, Duration::ZERO);
    app.ext.test_set_base(1);
    app.stack = vec![View::Root, View::ExtList, View::ExtDetail];
    app.ext.test_set_node(Some(ferrocast_core::extensions::node::NodeInfo {
        path: "/nonexistent/ferrocast-node".into(),
        version: (22, 1, 0),
    }));
    let err = app.ext_launch_with("e", "x", Default::default()).expect_err("node cannot start");
    assert!(err.contains("Node"), "{err}");
    assert!(app.ext.test_has_session(), "the running command must survive a failed launch");
    assert_eq!(app.stack.len(), 3, "its screens stay");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn reload_scripts_returns_at_once_and_the_list_arrives_with_the_toast() {
    let mut app = fixture();
    let dir = app.store.scripts_dir();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("one.sh"), "#!/bin/sh\n# @raycast.schemaVersion 1\n# @raycast.title One\n# @raycast.mode silent\ntrue\n")
        .unwrap();
    app.scripts.clear();
    app.run_builtin(Builtin::ReloadScripts);
    let t = Instant::now();
    while app.scripts.is_empty() {
        app.drain();
        assert!(t.elapsed() < Duration::from_secs(10), "the scripts never arrived");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(app.scripts[0].title, "One");
    assert!(app.toast.as_ref().is_some_and(|t| t.0.contains("1 script command")), "{:?}", app.toast);
}

#[test]
fn concurrent_chatgpt_requests_refresh_the_session_once() {
    use ferrocast_core::chatgpt::Credentials;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    let stored = Arc::new(Mutex::new(Credentials { access: "old".into(), refresh: "r1".into(), expires: 0, account_id: "a".into() }));
    let refreshes = Arc::new(AtomicUsize::new(0));
    let handles: Vec<_> = (0..6)
        .map(|_| {
            let (stored, refreshes) = (stored.clone(), refreshes.clone());
            std::thread::spawn(move || {
                let (s1, s2) = (stored.clone(), stored.clone());
                services::fresh_credentials_with(
                    move || Some(s1.lock().unwrap().clone()),
                    |c| {
                        refreshes.fetch_add(1, Ordering::SeqCst);
                        assert_eq!(c.refresh, "r1", "a second refresh would reuse a rotated token");
                        std::thread::sleep(Duration::from_millis(50));
                        Ok(Credentials { access: "new".into(), refresh: "r2".into(), expires: 10_000, account_id: "a".into() })
                    },
                    move |c| *s2.lock().unwrap() = c.clone(),
                    || 100,
                )
            })
        })
        .collect();
    for h in handles {
        assert_eq!(h.join().unwrap().unwrap().access, "new");
    }
    assert_eq!(refreshes.load(Ordering::SeqCst), 1, "one refresh served every waiting request");
}

fn r5_clip(id: &str, kind: ClipKind, content: &str) -> ClipEntry {
    ClipEntry {
        id: id.into(),
        kind,
        content: content.into(),
        title: Some("t".into()),
        copied_at: 7,
        pinned: true,
        source_app: Some("Safari".into()),
        times_copied: 3,
        width: 4,
        height: 5,
    }
}

#[test]
fn deleting_a_clipboard_entry_offers_undo_that_restores_it_in_place_and_keeps_the_image() {
    let mut app = fixture();
    let dir = app.store.images_dir();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("pic.png"), b"x").unwrap();
    let pic = r5_clip("b", ClipKind::Image, "pic.png");
    *app.shared.clipboard.lock().unwrap() = vec![r5_clip("a", ClipKind::Text, "one"), pic.clone(), r5_clip("c", ClipKind::Text, "three")];
    app.perform(Act::ClipDelete("b".into()), false);
    assert_eq!(app.toast.as_ref().map(|t| t.0.as_str()), Some(draw::DELETED_TOAST));
    assert_eq!(app.shared.clipboard.lock().unwrap().len(), 2);
    assert!(dir.join("pic.png").exists(), "the image survives while Undo is on offer");
    assert!(app.undo_entry_delete());
    let list = app.shared.clipboard.lock().unwrap().clone();
    assert_eq!(list.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["a", "b", "c"]);
    assert_eq!(list[1], pic, "same content and metadata");
    drop(list);
    assert!(!app.undo_entry_delete(), "one Undo per delete");
    // once the window closes the image goes
    app.perform(Act::ClipDelete("b".into()), false);
    app.toast = app.toast.take().map(|(t, _)| (t, Instant::now() - Duration::from_secs(9)));
    app.expire_toasts();
    assert!(app.entry_undo.is_none());
    assert!(!dir.join("pic.png").exists(), "pruned once Undo expired");
    assert!(!app.undo_entry_delete());
    let _ = std::fs::remove_dir_all(&app.store.root);
}

#[test]
fn deleting_a_note_or_a_calculator_entry_offers_undo() {
    let (mut app, _) = notes_app(vec![notes_note("a", "Alpha", 1), notes_note("b", "Beta", 2)]);
    app.notes[1].pinned_at = Some(5);
    app.perform(Act::NoteDelete("b".into()), false);
    assert!(app.notes[1].deleted_at.is_some());
    assert_eq!(app.toast.as_ref().map(|t| t.0.as_str()), Some(draw::DELETED_TOAST));
    assert!(app.undo_entry_delete());
    assert!(app.notes[1].deleted_at.is_none() && app.notes[1].pinned_at == Some(5), "back, pin included");
    let saved: Vec<Note> = app.store.load("notes.json");
    assert!(saved[1].deleted_at.is_none());

    let e =
        |q: &str| ferrocast_core::models::CalcEntry { question: q.into(), display: q.into(), raw: q.into(), value: 1.0, unit: None, at: 1 };
    app.calc_history = vec![e("1+1"), e("2+2"), e("3+3")];
    app.perform(Act::DeleteCalcEntry(1), false);
    assert_eq!(app.calc_history.len(), 2);
    assert_eq!(app.toast.as_ref().map(|t| t.0.as_str()), Some(draw::DELETED_TOAST));
    assert!(app.undo_entry_delete());
    assert_eq!(app.calc_history.iter().map(|c| c.question.as_str()).collect::<Vec<_>>(), ["1+1", "2+2", "3+3"]);
    let _ = std::fs::remove_dir_all(&app.store.root);
}

#[test]
fn undo_after_a_delete_works_from_the_footer_link_and_cmd_z() {
    let mut app = fixture();
    app.calc_history = (0..3)
        .map(|i| ferrocast_core::models::CalcEntry {
            question: format!("{i}+0"),
            display: String::new(),
            raw: String::new(),
            value: 0.0,
            unit: None,
            at: 1,
        })
        .collect();
    app.perform(Act::DeleteCalcEntry(0), false);
    sd4_render(&mut app);
    let u = sd4_rect(&app, "settings-undo-rect").expect("the footer offers Undo");
    click_at(&mut app, u.center());
    assert_eq!(app.calc_history.len(), 3, "the link restored it");
    app.perform(Act::DeleteCalcEntry(0), false);
    sd4_render(&mut app);
    let z = Event::Key { key: Key::Z, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND };
    render_mods(&mut app, vec![z], Modifiers::COMMAND);
    assert_eq!(app.calc_history.len(), 3, "⌘Z restored it");
    assert!(app.entry_undo.is_none());
}

#[test]
fn another_message_waits_for_the_delete_undo_instead_of_removing_it() {
    let mut app = fixture();
    app.calc_history = vec![ferrocast_core::models::CalcEntry {
        question: "1+1".into(),
        display: String::new(),
        raw: String::new(),
        value: 0.0,
        unit: None,
        at: 1,
    }];
    app.perform(Act::DeleteCalcEntry(0), false);
    app.toast("Something else");
    assert_eq!(app.toast.as_ref().map(|t| t.0.as_str()), Some(draw::DELETED_TOAST));
    assert!(app.undo_entry_delete());
}

#[test]
fn leaving_ask_ai_cancels_the_request_in_flight() {
    let mut app = fixture();
    let cancel = Arc::new(services::AiCancel::default());
    app.ai_cancel = Some(cancel.clone());
    app.stack = vec![
        View::Root,
        View::Ai { prompt: String::new(), asked: String::new(), answer: String::new(), history: vec![], busy: true, run: 1 },
    ];
    app.cool_claude();
    assert!(!cancel.is_stopped(), "Ask AI is still open");
    app.stack.pop();
    app.cool_claude();
    assert!(cancel.is_stopped(), "closing Ask AI stops the request");
    assert!(app.ai_cancel.is_none());
}
