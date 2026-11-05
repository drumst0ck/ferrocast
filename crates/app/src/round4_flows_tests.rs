//! Round 4, flows group (child module of `tests`): updater, scripts, child processes, clipboard images.
use super::*;
use std::os::unix::fs::PermissionsExt;

fn tmp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ferrocast-r4-{tag}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn bus() -> (services::Bus, std::sync::mpsc::Receiver<Msg>) {
    let (tx, rx) = std::sync::mpsc::channel();
    (services::Bus { tx, ctx: Context::default() }, rx)
}

/// A script file with `mode`, parsed as a command.
fn script_at(dir: &std::path::Path, name: &str, body: &str, mode: u32) -> ferrocast_core::script::ScriptCommand {
    let p = dir.join(name);
    let src = format!("#!/bin/sh\n# @raycast.schemaVersion 1\n# @raycast.title T\n# @raycast.mode fullOutput\n{body}\n");
    std::fs::write(&p, &src).unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(mode)).unwrap();
    ferrocast_core::script::parse(&p, &src).unwrap()
}

fn wait_script(rx: &std::sync::mpsc::Receiver<Msg>, secs: u64) -> (String, i32) {
    match rx.recv_timeout(Duration::from_secs(secs)) {
        Ok(Msg::Script { text, code, .. }) => (text, code),
        Ok(_) => panic!("unexpected message"),
        Err(e) => panic!("the script run never reported back: {e}"),
    }
}

// ---- S60: running a script keeps its permissions -------------------------------------------------

#[test]
fn running_a_script_only_adds_the_owner_execute_bit() {
    let dir = tmp_dir("s60");
    for (mode, expect) in [(0o600, 0o700), (0o700, 0o700), (0o644, 0o744), (0o750, 0o750), (0o755, 0o755)] {
        let s = script_at(&dir, &format!("s{mode:o}.sh"), "echo hi", mode);
        let (b, rx) = bus();
        services::run_script(&s, vec![], 1, b);
        let (text, code) = wait_script(&rx, 10);
        assert_eq!((text.trim(), code), ("hi", 0), "mode {mode:o}");
        let now = std::fs::metadata(&s.path).unwrap().permissions().mode() & 0o777;
        assert_eq!(now, expect, "a script at {mode:o} must end at {expect:o}, not {now:o}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- S61: WebReplaced no longer abandons the drain -------------------------------------------------

#[test]
fn a_web_replace_that_worked_does_not_stop_the_messages_queued_behind_it() {
    use ferrocast_core::translate::WebResult;
    let mut app = fixture();
    app.bus.send(Msg::WebReplaced { lang: "Spanish".into(), text: "Hola".into(), result: WebResult::Replaced });
    app.bus.send(Msg::Toast("second message".into()));
    app.drain();
    assert_eq!(toast_text(&app), "second message", "the queued message is handled in the same drain");
    assert!(app.rx.try_recv().is_err(), "nothing is left in the channel");
}

// ---- S57: repeated update checks do not stack Update screens --------------------------------------

#[test]
fn two_update_checks_leave_one_update_screen() {
    use ferrocast_core::update::Channel;
    let mut app = fixture();
    app.settings.update_install = UpdateInstall::AskFirst;
    app.handle_update_checked(true, Ok(Some(fake_release("99.0.0"))), Channel::Direct, true);
    let depth = app.stack.len();
    app.handle_update_checked(true, Ok(Some(fake_release("99.0.1"))), Channel::Direct, true);
    assert_eq!(app.stack.len(), depth, "the second check replaces the screen");
    assert_eq!(app.stack.iter().filter(|v| matches!(v, View::Update { .. })).count(), 1);
    match app.view() {
        View::Update { release } => assert_eq!(release.tag, "v99.0.1", "the newest release is shown"),
        _ => panic!("expected the update view"),
    }
    app.pop();
    assert!(matches!(app.view(), View::Root), "one Esc leaves the update screen");
}

// ---- S56: update, sync and Apple Events messages are translated -----------------------------------

#[test]
fn update_sync_and_apple_events_messages_have_rows_in_every_language() {
    use ferrocast_core::i18n::{take_misses, tr, with_language, Lang};
    use ferrocast_core::update::{feed_result, plan_for, Channel, Plan};
    let mut keys: Vec<String> = vec![
        ferrocast_core::translate::JS_PERMISSION_HINT.to_string(),
        ferrocast_core::sync_io::Unavailable::ICloudMissing.message(),
        "Could not install the update, the old version was restored ({e})".into(),
        "Could not move the current app aside: {e}".into(),
        "Script timed out after {secs} s and was stopped".into(),
        "[output truncated at {kb} KB]".into(),
    ];
    keys.push(feed_result(false, 0, "").unwrap_err());
    keys.push(feed_result(true, 429, "").unwrap_err());
    for ch in [(Channel::Dev, true), (Channel::Direct, false)] {
        if let Plan::Refuse(m) = plan_for(ch.0, ch.1) {
            keys.push(m);
        }
    }
    for lang in [Lang::Es, Lang::De, Lang::Fr, Lang::PtBr, Lang::Ja, Lang::ZhHans] {
        with_language(lang, || {
            let _ = take_misses();
            for k in &keys {
                let _ = tr(k);
            }
            assert_eq!(take_misses(), Vec::<String>::new(), "{lang:?} lacks rows");
        });
    }
}

#[test]
fn the_github_http_error_is_translated_with_its_code() {
    use ferrocast_core::i18n::{with_language, Lang};
    let msg = with_language(Lang::De, || ferrocast_core::update::feed_result(true, 500, "").unwrap_err());
    assert!(msg.contains("500"), "{msg}");
    assert!(!msg.contains("GitHub answered"), "still English: {msg}");
}

#[test]
fn an_installer_error_is_translated_before_it_is_wrapped() {
    use ferrocast_core::i18n::{tr_in, with_language, Lang};
    let raw = "The update archive has no Ferrocast.app";
    let mut app = fixture();
    with_language(Lang::De, || {
        app.bus.send(Msg::UpdateFailed(raw.into()));
        app.drain();
        let german = tr_in(raw, Lang::De).into_owned();
        assert_ne!(german, raw);
        assert!(toast_text(&app).contains(&german), "{}", toast_text(&app));
    });
}

// ---- S55 / S62: deadlines kill the whole process group, output is capped ---------------------------

/// True while `pid` is alive (signal 0 reaches it).
fn alive(pid: &str) -> bool {
    std::process::Command::new("/bin/kill").args(["-0", pid]).stderr(std::process::Stdio::null()).status().is_ok_and(|s| s.success())
}

fn wait_dead(pid: &str) -> bool {
    for _ in 0..40 {
        if !alive(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

#[test]
fn a_timed_out_command_takes_its_descendants_with_it() {
    let dir = tmp_dir("s55");
    let pidfile = dir.join("pid");
    let mut c = std::process::Command::new("/bin/sh");
    c.arg("-c").arg(format!("sleep 60 & echo $! > '{}'; wait", pidfile.display()));
    let started = Instant::now();
    let err = services::run_deadline(c, 1).unwrap_err();
    assert!(err.contains("timed out"), "{err}");
    assert!(started.elapsed() < Duration::from_secs(8), "returns promptly: {:?}", started.elapsed());
    let pid = std::fs::read_to_string(&pidfile).unwrap().trim().to_string();
    assert!(wait_dead(&pid), "the grandchild {pid} is still running");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_command_whose_descendant_keeps_the_pipes_open_does_not_block_the_reader() {
    let mut c = std::process::Command::new("/bin/sh");
    c.arg("-c").arg("sleep 8 >/dev/null 2>&1 & echo done");
    let started = Instant::now();
    let out = services::run_deadline(c, 20).unwrap();
    assert_eq!(String::from_utf8_lossy(&out).trim(), "done");
    assert!(started.elapsed() < Duration::from_secs(6), "waited for the lingering descendant: {:?}", started.elapsed());
}

#[test]
fn a_script_past_its_deadline_is_stopped_and_reports_it() {
    let dir = tmp_dir("s62-timeout");
    let pidfile = dir.join("pid");
    let s = script_at(&dir, "hang.sh", &format!("sleep 60 &\necho $! > '{}'\necho before\nwait", pidfile.display()), 0o700);
    let (b, rx) = bus();
    let started = Instant::now();
    services::run_script_within(&s, vec![], 7, b, 1, 1000);
    let (text, code) = wait_script(&rx, 15);
    assert!(started.elapsed() < Duration::from_secs(10));
    assert_eq!(code, -1);
    assert!(text.contains("before"), "the output so far is kept: {text:?}");
    assert!(text.contains("timed out after 1 s"), "{text:?}");
    let pid = std::fs::read_to_string(&pidfile).unwrap().trim().to_string();
    assert!(wait_dead(&pid), "the script's child {pid} is still running");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn script_output_is_capped() {
    let dir = tmp_dir("s62-cap");
    let s = script_at(&dir, "chatty.sh", "yes xxxxxxxxxxxxxxxx | head -c 3000000", 0o700);
    let (b, rx) = bus();
    services::run_script_within(&s, vec![], 8, b, 30, 4096);
    let (text, code) = wait_script(&rx, 30);
    assert_eq!(code, 0);
    assert!(text.len() < 4096 + 200, "{} bytes", text.len());
    assert!(text.contains("output truncated"), "the cut is announced");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- S58: clipboard images are written atomically and repaired ------------------------------------

#[test]
fn a_truncated_clipboard_image_is_rewritten_on_the_next_copy() {
    let dir = tmp_dir("s58");
    let png: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
    let name = services::store_clip_image(&dir, &png);
    let path = dir.join(&name);
    assert_eq!(std::fs::read(&path).unwrap(), png);
    std::fs::write(&path, &png[..100]).unwrap(); // a crash / full disk left half a file
    assert_eq!(services::store_clip_image(&dir, &png), name);
    assert_eq!(std::fs::read(&path).unwrap(), png, "the half-written file is repaired");
    let names: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
    assert_eq!(names, vec![name], "no temp file is left behind");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- S59: the update is extracted from the verified bytes ------------------------------------------

#[test]
fn the_update_zip_is_extracted_from_the_bytes_that_were_verified() {
    let dir = tmp_dir("s59");
    let src = dir.join("src/Ferrocast.app/Contents");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("v"), "verified").unwrap();
    let zip = dir.join("a.zip");
    let ok = std::process::Command::new("/usr/bin/ditto")
        .args(["-c", "-k", "--keepParent"])
        .arg(dir.join("src/Ferrocast.app"))
        .arg(&zip)
        .status()
        .unwrap();
    assert!(ok.success());
    let bytes = std::fs::read(&zip).unwrap();
    std::fs::write(&zip, b"swapped by someone else").unwrap(); // the copy on disk no longer matters
    let out = dir.join("extract");
    services::extract_zip_bytes(bytes, &out).unwrap();
    assert_eq!(std::fs::read_to_string(out.join("Ferrocast.app/Contents/v")).unwrap(), "verified");
    assert!(services::extract_zip_bytes(b"not a zip".to_vec(), &dir.join("bad")).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- S54: the new app is staged beside the bundle before the old one moves -------------------------

#[test]
fn the_bundle_is_never_absent_while_the_new_app_is_staged() {
    let root = tmp_dir("s54");
    let (bundle, new_app) = (root.join("apps/Ferrocast.app"), root.join("extract/Ferrocast.app"));
    std::fs::create_dir_all(bundle.join("Contents")).unwrap();
    std::fs::write(bundle.join("Contents/v"), "old").unwrap();
    std::fs::create_dir_all(new_app.join("Contents")).unwrap();
    std::fs::write(new_app.join("Contents/v"), "new").unwrap();
    let seen = std::cell::Cell::new(false);
    services::swap_bundle_via(&new_app, &bundle, |from, staged| {
        seen.set(bundle.join("Contents/v").exists());
        assert_eq!(staged.parent(), bundle.parent(), "staged on the bundle's own volume");
        std::fs::rename(from, staged).map_err(|e| e.to_string())
    })
    .unwrap();
    assert!(seen.get(), "the old bundle was still in place while staging");
    assert_eq!(std::fs::read_to_string(bundle.join("Contents/v")).unwrap(), "new");
    assert_eq!(std::fs::read_to_string(root.join("apps/Ferrocast.app.previous/Contents/v")).unwrap(), "old");
    assert_eq!(std::fs::read_dir(root.join("apps")).unwrap().count(), 2, "no staging leftover");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_staging_failure_leaves_the_bundle_untouched() {
    let root = tmp_dir("s54-fail");
    let (bundle, new_app) = (root.join("apps/Ferrocast.app"), root.join("extract/Ferrocast.app"));
    std::fs::create_dir_all(bundle.join("Contents")).unwrap();
    std::fs::write(bundle.join("Contents/v"), "old").unwrap();
    let err = services::swap_bundle_via(&new_app, &bundle, |_, staged| {
        std::fs::create_dir_all(staged).unwrap(); // a partial copy, then the "process" fails
        Err("disk full".into())
    })
    .unwrap_err();
    assert!(err.contains("disk full"), "{err}");
    assert_eq!(std::fs::read_to_string(bundle.join("Contents/v")).unwrap(), "old");
    assert_eq!(std::fs::read_dir(root.join("apps")).unwrap().count(), 1, "partial copy and previous are gone");
    let _ = std::fs::remove_dir_all(&root);
}
