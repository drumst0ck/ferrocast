//! Check for the Notes window shell:
//! `cargo run -p ferrocast-macos --example notes_demo -- --check [--out <dir>]`
//! Opens a real window for a couple of seconds, drives it programmatically, asserts the
//! behavior and writes light/dark PNG snapshots. Exit code 0 only when every assertion holds.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use ferrocast_macos::notes::{self, Event};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{msg_send, MainThreadMarker, Message};
use objc2_app_kit::{
    NSAppearance, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSApplication, NSApplicationActivationPolicy, NSButton, NSEvent,
    NSEventModifierFlags, NSEventType, NSPasteboard, NSPasteboardTypeString, NSTextView, NSView,
};
use objc2_foundation::{NSDate, NSPoint, NSProcessInfo, NSRange, NSRunLoop, NSString};

thread_local! {
    static FAILURES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn check(name: &str, ok: bool, detail: &str) {
    if ok {
        println!("PASS  {name}");
    } else {
        println!("FAIL  {name}  ({detail})");
        FAILURES.with(|f| f.borrow_mut().push(name.to_string()));
    }
}

fn pump(ms: u64) {
    NSRunLoop::currentRunLoop().runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(ms as f64 / 1000.0));
}

/// Post a real key-down event through `NSApplication::sendEvent`, like the system would.
fn key(app: &NSApplication, chars: &str, code: u16, mods: NSEventModifierFlags) {
    let window_number = app.keyWindow().map(|w| w.windowNumber()).unwrap_or(0);
    let s = NSString::from_str(chars);
    let ev = NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
        NSEventType::KeyDown,
        NSPoint::new(0.0, 0.0),
        mods,
        NSProcessInfo::processInfo().systemUptime(),
        window_number,
        None,
        &s,
        &s,
        false,
        code,
    )
    .expect("key event");
    app.sendEvent(&ev);
    pump(50);
}

/// Wait up to `ms` for the window to become key.
fn wait_key(ms: u64) -> bool {
    for _ in 0..ms / 50 {
        if notes::is_key() {
            return true;
        }
        pump(50);
    }
    notes::is_key()
}

/// macOS 14+ only lets a process take focus when LaunchServices started it (or the user
/// handed activation over). A bare `cargo run` from a background context (CI, an agent
/// runner) is refused, so re-run this very check inside a throwaway `.app` bundle started
/// with `open`, and report its verdict.
fn relaunch_in_bundle(args: &[String]) -> ! {
    let tmp = std::env::temp_dir().join(format!("notes-demo-bundle-{}", std::process::id()));
    let app = tmp.join("NotesDemo.app");
    let macos = app.join("Contents/MacOS");
    std::fs::create_dir_all(&macos).expect("create bundle");
    std::fs::copy(std::env::current_exe().expect("exe"), macos.join("notes_demo")).expect("copy exe");
    std::fs::write(
        app.join("Contents/Info.plist"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict><key>CFBundleExecutable</key><string>notes_demo</string><key>CFBundleIdentifier</key><string>dev.ferrocast.notesdemo</string><key>CFBundleName</key><string>NotesDemo</string><key>CFBundlePackageType</key><string>APPL</string></dict></plist>"#,
    )
    .expect("write plist");
    let log = tmp.join("out.log");
    let mut cmd = std::process::Command::new("open");
    cmd.args(["-W", "-n", "--stdout"]).arg(&log).arg("--stderr").arg(&log).arg(&app).arg("--args").args(&args[1..]).arg("--bundled");
    let status = cmd.status();
    let out = std::fs::read_to_string(&log).unwrap_or_default();
    print!("{out}");
    let ok = status.map(|s| s.success()).unwrap_or(false) && out.contains("notes_demo: all checks passed");
    let _ = std::fs::remove_file(&log);
    std::process::exit(if ok { 0 } else { 1 });
}

fn find_button(view: &NSView, tip: &str) -> Option<Retained<NSButton>> {
    for sub in view.subviews().iter() {
        if let Some(b) = sub.downcast_ref::<NSButton>() {
            if b.toolTip().is_some_and(|t| t.to_string() == tip) {
                return Some(b.retain());
            }
        }
        if let Some(found) = find_button(&sub, tip) {
            return Some(found);
        }
    }
    None
}

fn first_responder_text_view(app: &NSApplication) -> Option<Retained<NSTextView>> {
    let fr = app.keyWindow()?.firstResponder()?;
    fr.downcast_ref::<NSTextView>().map(|t| t.retain())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if !args.iter().any(|a| a == "--check") {
        eprintln!("usage: notes_demo --check [--out <dir>]");
        std::process::exit(2);
    }
    let out: PathBuf = args
        .iter()
        .position(|a| a == "--out")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("ferrocast-notes-demo"));
    std::fs::create_dir_all(&out).expect("create out dir");

    let mtm = MainThreadMarker::new().expect("main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    app.finishLaunching();

    let changes: Rc<RefCell<Vec<String>>> = Rc::default();
    let events: Rc<RefCell<Vec<Event>>> = Rc::default();
    {
        let c = changes.clone();
        notes::set_on_change(Box::new(move |t| c.borrow_mut().push(t)));
        let e = events.clone();
        notes::set_on_event(Box::new(move |ev| e.borrow_mut().push(ev)));
    }

    // API is safe before the window exists
    check("not visible before show", !notes::is_visible() && !notes::is_key(), "visible before show");

    notes::show();
    if !wait_key(1500) && !args.iter().any(|a| a == "--bundled") {
        println!("note: this process cannot take focus (not started by LaunchServices); re-running inside an app bundle");
        notes::hide();
        relaunch_in_bundle(&args);
    }
    check("is_visible after show", notes::is_visible(), "window not visible");
    check("is_key after show (accessory app activated)", notes::is_key(), "window did not become key");
    check("editor is first responder", first_responder_text_view(&app).is_some(), "first responder is not the NSTextView");

    // set_text: title, round trip, no callback
    notes::set_text("# Groceries\nmilk");
    check("title after set_text", notes::title() == "Groceries", &notes::title());
    check("text round trip", notes::text() == "# Groceries\nmilk", &notes::text());
    check("set_text does not fire on_change", changes.borrow().is_empty(), "callback fired");

    // user-style edit through insertText:
    if let Some(tv) = first_responder_text_view(&app) {
        let len = notes::text().encode_utf16().count();
        tv.setSelectedRange(NSRange::new(len, 0));
        let s = NSString::from_str("\nbread");
        unsafe {
            let _: () = msg_send![&*tv, insertText: &*s as &AnyObject];
        }
    }
    pump(50);
    check("on_change fired once for insertText", changes.borrow().len() == 1, &format!("{} calls", changes.borrow().len()));
    check(
        "on_change carries the full text",
        changes.borrow().first().is_some_and(|t| t == "# Groceries\nmilk\nbread") && notes::text() == "# Groceries\nmilk\nbread",
        &format!("{:?}", changes.borrow()),
    );
    changes.borrow_mut().clear();

    // typing a new first line updates the title live
    if let Some(tv) = first_responder_text_view(&app) {
        tv.setSelectedRange(NSRange::new(0, 0));
        let s = NSString::from_str("> Weekly ");
        unsafe {
            let _: () = msg_send![&*tv, insertText: &*s as &AnyObject];
        }
    }
    check("title follows edits", notes::title() == "Weekly # Groceries", &notes::title());
    notes::set_text("# Groceries\nmilk\nbread\n\nPlain text with **markdown**, \"quotes\" -- and dashes.");
    changes.borrow_mut().clear();

    // quote/dash substitution is off
    if let Some(tv) = first_responder_text_view(&app) {
        check(
            "automatic quote/dash substitution off",
            !tv.isAutomaticQuoteSubstitutionEnabled() && !tv.isAutomaticDashSubstitutionEnabled(),
            "substitution enabled",
        );
    }

    // ⌘N / ⌘P / toolbar buttons
    let cmd = NSEventModifierFlags::Command;
    key(&app, "n", 45, cmd);
    key(&app, "p", 35, cmd);
    check("⌘N and ⌘P dispatch New, Browse", *events.borrow() == vec![Event::New, Event::Browse], &format!("{:?}", events.borrow()));
    events.borrow_mut().clear();
    if let Some(content) = app.keyWindow().and_then(|w| w.contentView()) {
        let b = find_button(&content, "Browse Notes ⌘P");
        let n = find_button(&content, "New Note ⌘N");
        check("toolbar buttons exist with tooltips", b.is_some() && n.is_some(), "button missing");
        if let (Some(b), Some(n)) = (b, n) {
            unsafe {
                n.performClick(None);
                b.performClick(None);
            }
        }
    }
    check("toolbar buttons dispatch New, Browse", *events.borrow() == vec![Event::New, Event::Browse], &format!("{:?}", events.borrow()));

    // copy / paste / undo / redo keep working in the text view
    let pb = NSPasteboard::generalPasteboard();
    let saved = pb.stringForType(unsafe { NSPasteboardTypeString });
    notes::set_text("hello");
    key(&app, "a", 0, cmd);
    key(&app, "c", 8, cmd);
    let copied = pb.stringForType(unsafe { NSPasteboardTypeString }).map(|s| s.to_string());
    check("⌘A + ⌘C copy the text", copied.as_deref() == Some("hello"), &format!("{copied:?}"));
    if let Some(tv) = first_responder_text_view(&app) {
        tv.setSelectedRange(NSRange::new(5, 0));
    }
    key(&app, "v", 9, cmd);
    check("⌘V pastes", notes::text() == "hellohello", &notes::text());
    key(&app, "z", 6, cmd);
    check("⌘Z undoes the paste", notes::text() == "hello", &notes::text());
    key(&app, "z", 6, cmd | NSEventModifierFlags::Shift);
    check("⇧⌘Z redoes it", notes::text() == "hellohello", &notes::text());
    key(&app, "a", 0, cmd);
    key(&app, "x", 7, cmd);
    check("⌘X cuts", notes::text().is_empty(), &notes::text());
    match saved {
        Some(s) => {
            pb.clearContents();
            pb.setString_forType(&s, unsafe { NSPasteboardTypeString });
        }
        None => {
            pb.clearContents();
        }
    }

    // ⌘W hides
    key(&app, "w", 13, cmd);
    check("⌘W hides the window", !notes::is_visible(), "still visible");

    // Esc, default mode: hides
    notes::show();
    pump(200);
    key(&app, "\u{1b}", 53, NSEventModifierFlags::empty());
    check("Esc hides (escape_loses_focus = false)", !notes::is_visible(), "still visible");

    // Esc, loses-focus mode: stays visible, not key
    notes::set_escape_loses_focus(true);
    notes::show();
    wait_key(2000);
    check("key again before Esc", notes::is_key(), "not key");
    key(&app, "\u{1b}", 53, NSEventModifierFlags::empty());
    pump(300);
    check("Esc keeps the window visible (escape_loses_focus = true)", notes::is_visible(), "hidden");
    check("Esc resigns key (escape_loses_focus = true)", !notes::is_key(), "still key");
    notes::set_escape_loses_focus(false);

    // hide / toggle
    notes::hide();
    check("hide()", !notes::is_visible(), "still visible");
    notes::toggle();
    check("toggle() shows", notes::is_visible(), "not visible");
    notes::toggle();
    check("toggle() hides", !notes::is_visible(), "still visible");
    notes::show();
    pump(200);

    // snapshots
    notes::set_text(
        "# Groceries\n\nmilk\nbread\neggs\n\nA longer paragraph to check wrapping and the text colour: the quick brown fox jumps over the lazy dog. \
         Plain text only for now.\n\n> quote line\n- list line",
    );
    for (name, appearance) in [("light", unsafe { NSAppearanceNameAqua }), ("dark", unsafe { NSAppearanceNameDarkAqua })] {
        app.setAppearance(NSAppearance::appearanceNamed(appearance).as_deref());
        pump(300);
        let path = out.join(format!("notes-{name}.png"));
        let ok = notes::snapshot_png(&path);
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        check(&format!("snapshot {name} written"), ok && size > 1000, &format!("ok={ok} size={size}"));
        println!("      {}", path.display());
    }
    notes::hide();

    let failures = FAILURES.with(|f| f.borrow().clone());
    if failures.is_empty() {
        println!("notes_demo: all checks passed");
    } else {
        println!("notes_demo: {} check(s) FAILED: {}", failures.len(), failures.join("; "));
        std::process::exit(1);
    }
}
