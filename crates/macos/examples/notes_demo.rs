//! Check for the Notes window shell and the rich-text Markdown editor:
//! `cargo run -p ferrocast-macos --example notes_demo -- --check [--out <dir>]`
//! Opens a real window for a couple of seconds, drives it programmatically, asserts the
//! behavior and writes light/dark PNG snapshots. Exit code 0 only when every assertion holds.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use ferrocast_core::richtext::{self, BlockKind};
use ferrocast_macos::notes::{self, Anchor, EmojiEvent, Event, PopupKey, SlashEvent};
use ferrocast_macos::notes_format::Format;
use ferrocast_macos::notes_palette::{self, PaletteItem};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{msg_send, MainThreadMarker, Message};
use objc2_app_kit::{
    NSAppearance, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSApplication, NSApplicationActivationPolicy, NSButton, NSEvent,
    NSEventModifierFlags, NSEventType, NSPasteboard, NSPasteboardTypeString, NSTextView, NSView,
};
use objc2_app_kit::{NSBackingStoreType, NSFontAttributeName, NSTextField, NSTextStorage, NSWindow, NSWindowStyleMask};
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

fn find_label(view: &NSView, text: &str) -> Option<Retained<NSTextField>> {
    for sub in view.subviews().iter() {
        if let Some(f) = sub.downcast_ref::<NSTextField>() {
            if f.stringValue().to_string() == text {
                return Some(f.retain());
            }
        }
        if let Some(found) = find_label(&sub, text) {
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

    let want = |md: &str| richtext::to_markdown(&richtext::parse(md));
    // Regression: content set before the window is first shown must reach the toolbar title.
    notes::set_text("# Early title\nbody");
    check("set_text before the window exists keeps the text", notes::text() == want("# Early title\nbody"), &notes::text());
    check("creating the window by set_text does not show it", !notes::is_visible(), "visible");

    notes::show();
    if !wait_key(1500) && !args.iter().any(|a| a == "--bundled") {
        println!("note: this process cannot take focus (not started by LaunchServices); re-running inside an app bundle");
        notes::hide();
        relaunch_in_bundle(&args);
    }
    check("is_visible after show", notes::is_visible(), "window not visible");
    check("is_key after show (accessory app activated)", notes::is_key(), "window did not become key");
    check("editor is first responder", first_responder_text_view(&app).is_some(), "first responder is not the NSTextView");
    check("show() fires Shown once", *events.borrow() == vec![Event::Shown], &format!("{:?}", events.borrow()));
    events.borrow_mut().clear();
    {
        let label = app.keyWindow().and_then(|w| w.contentView()).and_then(|c| find_label(&c, "Early title"));
        check(
            "toolbar title shows the text set before the window existed",
            label.is_some() && notes::title() == "Early title",
            &format!("title {:?}, label found {}", notes::title(), label.is_some()),
        );
    }

    // set_text: title, round trip, no callback
    notes::set_text("# Groceries\nmilk");
    check("title after set_text", notes::title() == "Groceries", &notes::title());
    check("text round trip", notes::text() == want("# Groceries\nmilk"), &notes::text());
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
        changes.borrow().first().is_some_and(|t| *t == want("# Groceries\nmilk\nbread"))
            && notes::text() == want("# Groceries\nmilk\nbread"),
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
    check("title follows edits", notes::title() == "> Weekly Groceries", &notes::title());
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

    editor_checks(&app, &out);
    window_checks(&app, &out, &events);

    notes::hide();

    let failures = FAILURES.with(|f| f.borrow().clone());
    if failures.is_empty() {
        println!("notes_demo: all checks passed");
    } else {
        println!("notes_demo: {} check(s) FAILED: {}", failures.len(), failures.join("; "));
        std::process::exit(1);
    }
}

// ------------------------------------------------------------------ editor checks

/// `replace_selection_markdown`, `append_markdown` and `insert_markdown_after_selection_line`:
/// formatting kept, one undo step, one change callback, blank results refused.
fn markdown_result_checks(app: &NSApplication) {
    let md = notes::markdown;
    let fired: Rc<RefCell<Vec<String>>> = Rc::default();
    let f = fired.clone();
    notes::set_on_change(Box::new(move |t| f.borrow_mut().push(t)));
    let undo = |app: &NSApplication| key(app, "z", 6, CMD);

    // replace a selection inside a paragraph with styled spans
    reset(app, "Say hello to the **world** today");
    let before = md();
    tv_of(app).setSelectedRange(NSRange::new(4, 5));
    fired.borrow_mut().clear();
    let ok = notes::replace_selection_markdown("*hi* there **you**");
    check(
        "replace_selection_markdown replaces the selection with styled spans",
        ok && md() == md_of("Say *hi* there **you** to the **world** today"),
        &md(),
    );
    check("replace_selection_markdown fires the change callback once", fired.borrow().len() == 1, &format!("{}", fired.borrow().len()));
    undo(app);
    check("one undo restores the Markdown exactly after a span replacement", md() == before, &md());

    // keeps the host paragraph kind (heading) for an inline result
    reset(app, "## Big title here");
    tv_of(app).setSelectedRange(NSRange::new(4, 5));
    notes::replace_selection_markdown("small");
    check("an inline result keeps the heading it lands in", md() == md_of("## Big small here"), &md());

    // no selection: the whole note, with blocks
    reset(app, "one\n\ntwo");
    let before = md();
    tv_of(app).setSelectedRange(NSRange::new(1, 0));
    fired.borrow_mut().clear();
    let ok = notes::replace_selection_markdown("# Title\n\n- a\n- **b**\n\n```\ncode\n```");
    check(
        "replace_selection_markdown without a selection replaces the whole note with blocks",
        ok && md() == md_of("# Title\n\n- a\n- **b**\n\n```\ncode\n```"),
        &md(),
    );
    check("whole-note replacement fires the change callback once", fired.borrow().len() == 1, &format!("{}", fired.borrow().len()));
    undo(app);
    check("one undo restores the whole note exactly", md() == before, &md());

    // multi-block result over a selection that starts and ends mid-paragraph
    reset(app, "alpha beta\ngamma!");
    let before = md();
    tv_of(app).setSelectedRange(NSRange::new(6, 10));
    notes::replace_selection_markdown("X\n\n- y\n- z");
    check("a multi-block result keeps the unselected rest around it", md() == md_of("alpha X\n\n- y\n- z\n\n!"), &md());
    undo(app);
    check("one undo restores a multi-paragraph replacement", md() == before, &md());

    // the helpers dictation and the AI apply step rely on
    reset(app, "hello world");
    notes::select_range(6, 11);
    check(
        "select_range and selection_range agree and text_in_range reads storage text",
        notes::selection_range() == (6, 11)
            && notes::text_in_range(0, 5).as_deref() == Some("hello")
            && notes::text_in_range(6, 99).is_none(),
        &format!("{:?}", notes::selection_range()),
    );

    // blank results are refused and change nothing
    reset(app, "keep me");
    let before = md();
    fired.borrow_mut().clear();
    let refused = !notes::replace_selection_markdown("")
        && !notes::replace_selection_markdown(" \n ")
        && !notes::append_markdown("\n")
        && !notes::insert_markdown_after_selection_line("  ");
    check("blank replacements are refused", refused && md() == before && fired.borrow().is_empty(), &md());

    // append
    reset(app, "first **line**");
    let before = md();
    fired.borrow_mut().clear();
    let ok = notes::append_markdown("## Summary\n\n- one\n- *two*");
    check("append_markdown adds blocks after the last block", ok && md() == md_of("first **line**\n\n## Summary\n\n- one\n- *two*"), &md());
    check("append_markdown fires the change callback once", fired.borrow().len() == 1, &format!("{}", fired.borrow().len()));
    undo(app);
    check("one undo restores the note after append_markdown", md() == before, &md());
    reset(app, "");
    notes::append_markdown("only **this**");
    check("append_markdown on an empty note just adds the blocks", md() == md_of("only **this**"), &md());

    // insert after the selection line
    reset(app, "line one\nline two\nline three");
    let before = md();
    tv_of(app).setSelectedRange(NSRange::new(2, 3));
    fired.borrow_mut().clear();
    let ok = notes::insert_markdown_after_selection_line("It means **one**.");
    check(
        "insert_markdown_after_selection_line inserts below the line of the selection",
        ok && md() == md_of("line one\n\nIt means **one**.\nline two\nline three"),
        &md(),
    );
    check("insert after the line fires the change callback once", fired.borrow().len() == 1, &format!("{}", fired.borrow().len()));
    undo(app);
    check("one undo restores the note after the line insert", md() == before, &md());
    notes::set_on_change(Box::new(|_| {}));
}

fn md_of(src: &str) -> String {
    richtext::to_markdown(&richtext::parse(src))
}

fn tv_of(app: &NSApplication) -> Retained<NSTextView> {
    first_responder_text_view(app).expect("editor is first responder")
}

fn text_len(tv: &NSTextView) -> usize {
    tv.string().length()
}

/// Load a note and put the cursor at the end of it.
fn reset(app: &NSApplication, md: &str) {
    if !notes::is_key() {
        notes::show();
        wait_key(1500);
    }
    notes::focus_editor();
    notes::set_markdown(md);
    let tv = tv_of(app);
    tv.setSelectedRange(NSRange::new(text_len(&tv), 0));
}

fn select_all(app: &NSApplication) {
    let tv = tv_of(app);
    tv.setSelectedRange(NSRange::new(0, text_len(&tv)));
}

/// Type text one character at a time through `insertText:`, like the keyboard does.
fn typ(app: &NSApplication, text: &str) {
    let tv = tv_of(app);
    for c in text.chars() {
        let s = NSString::from_str(&c.to_string());
        unsafe {
            let _: () = msg_send![&*tv, insertText: &*s as &AnyObject];
        }
    }
    pump(10);
}

const CMD: NSEventModifierFlags = NSEventModifierFlags::Command;
const SHIFT: NSEventModifierFlags = NSEventModifierFlags::Shift;
const OPT: NSEventModifierFlags = NSEventModifierFlags::Option;
const NONE: NSEventModifierFlags = NSEventModifierFlags::empty();

fn kinds(md: &str) -> Vec<BlockKind> {
    richtext::parse(md).blocks.into_iter().map(|b| b.kind).collect()
}

fn mouse_click(app: &NSApplication, x: f64, y: f64) {
    let window_number = app.keyWindow().map(|w| w.windowNumber()).unwrap_or(0);
    for (i, ty) in [NSEventType::LeftMouseDown, NSEventType::LeftMouseUp].into_iter().enumerate() {
        let ev = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
            ty,
            NSPoint::new(x, y),
            NONE,
            NSProcessInfo::processInfo().systemUptime(),
            window_number,
            None,
            i as isize,
            1,
            1.0,
        )
        .expect("mouse event");
        app.sendEvent(&ev);
    }
    pump(50);
}

fn font_size_at(tv: &NSTextView, i: usize) -> f64 {
    let st: Retained<NSTextStorage> = unsafe { tv.textStorage() }.expect("storage");
    let mut eff = NSRange::new(0, 0);
    let v = unsafe { st.attribute_atIndex_effectiveRange(NSFontAttributeName, i, &mut eff as *mut NSRange) };
    v.and_then(|v| v.downcast_ref::<objc2_app_kit::NSFont>().map(|f| f.pointSize())).unwrap_or(0.0)
}

const ROUND_TRIP: &[&str] = &[
    "",
    "plain",
    "# H1",
    "## H2\n\ntext",
    "### H3",
    "**bold** and *italic* and ~~strike~~",
    "<u>under</u> text",
    "`code` inline",
    "[link](https://example.com) text",
    "> quote\n> second",
    "- a\n- b\n- c",
    "1. a\n2. b\n3. c",
    "- [ ] todo\n- [x] done",
    "- a\n  - b\n    - c\n  - d\n- e",
    "1. a\n  1. b\n  2. c\n2. d",
    "- [ ] a\n  - [x] b\n    - [ ] c",
    "```rust\nfn main() {}\n```",
    "```\na\n\nb\n```",
    "a\n\n---\n\nb",
    "a\n\n<br>\n\nb",
    "# Title\n\nPara with **bold**, *it*, `c`, [l](https://a.b)\n\n> q\n\n- x\n- y\n\n1. one\n2. two\n\n- [x] t\n\n```\ncode\n```\n\n---\n\nend",
    "***both***",
    "[**bold link**](https://x.y)",
    "# Title with **bold** and `code`",
    "😀 emoji **😀**",
    "- item with [link](https://a.b) and ~~x~~",
    "> quote with *italic*",
    "- a\n\n1. b",
    "-\n- x",
    "1. a\n\ntext\n\n1. b",
    "- l0\n  - l1\n    - l2\n      - l3\n        - l4",
    "\\# not a heading",
    "héllo wörld — ñ",
    "~~~\ntilde\n~~~",
    "a\n\n<br>",
    ">",
    "```\n```",
    "1. 1\n2. 2\n3. 3\n4. 4\n5. 5\n6. 6\n7. 7\n8. 8\n9. 9\n10. 10\n11. 11",
    "**a** *b* ~~c~~ <u>d</u> `e` [f](https://g.h)",
    "- [x] ~~done~~ and **bold**",
    "line one\nline two",
];

#[allow(clippy::too_many_lines)]
fn editor_checks(app: &NSApplication, out: &std::path::Path) {
    let md = notes::markdown;
    notes::set_escape_loses_focus(false);
    reset(app, "");

    // 1. Markdown round trip
    let mut bad = Vec::new();
    for src in ROUND_TRIP {
        notes::set_markdown(src);
        let got = md();
        if got != md_of(src) {
            bad.push(format!("{src:?} -> {got:?}"));
        }
    }
    check(
        &format!("round trip of {} notes: markdown(set_markdown(x)) == to_markdown(parse(x))", ROUND_TRIP.len()),
        bad.is_empty(),
        &bad.join(" | "),
    );
    notes::set_markdown("- a\n1. b\n> q\n## h\n```\nc\n```\n---");
    let shown = tv_of(app).string().to_string();
    check(
        "Markdown markers are not in the text",
        !shown.contains("- ") && !shown.contains("> ") && !shown.contains("##") && !shown.contains("```") && !shown.contains("---"),
        &format!("{shown:?}"),
    );
    check(
        "set_text / text aliases",
        {
            notes::set_text("# A\nb");
            notes::text() == notes::markdown() && notes::title() == "A"
        },
        &notes::text(),
    );

    // 2. block shortcuts typed character by character
    let shortcuts: &[(&str, &str, &str)] = &[
        ("# ", "T", "# T"),
        ("## ", "T", "## T"),
        ("### ", "T", "### T"),
        ("> ", "T", "> T"),
        ("- ", "T", "- T"),
        ("* ", "T", "- T"),
        ("1. ", "T", "1. T"),
        ("[] ", "T", "- [ ] T"),
        ("[ ] ", "T", "- [ ] T"),
        ("[x] ", "T", "- [x] T"),
        ("```rust ", "T", "```rust\nT\n```"),
        ("``` ", "T", "```\nT\n```"),
        ("~~~", "T", "```\nT\n```"),
        ("--- ", "T", "---\n\nT"),
        ("___ ", "T", "---\n\nT"),
    ];
    let mut bad = Vec::new();
    for (typed, after, expect) in shortcuts {
        reset(app, "");
        typ(app, typed);
        typ(app, after);
        let got = md();
        if got != md_of(expect) {
            bad.push(format!("{typed:?}: {got:?} != {:?}", md_of(expect)));
        }
    }
    check("block shortcuts (# ## ### > - * 1. [] [ ] [x] ``` ~~~ --- ___) convert and remove the prefix", bad.is_empty(), &bad.join(" | "));
    reset(app, "");
    typ(app, "#### x");
    check("`####` is not a shortcut", kinds(&md()) == vec![BlockKind::Paragraph] && tv_of(app).string().to_string() == "#### x", &md());
    reset(app, "word");
    tv_of(app).setSelectedRange(NSRange::new(2, 0));
    typ(app, "- ");
    check("shortcut only at the start of a line", kinds(&md()) == vec![BlockKind::Paragraph], &md());

    // 3. inline autoformat
    let inline: &[(&str, &str)] = &[
        ("**b**", "**b**"),
        ("__b__", "**b**"),
        ("*i*", "*i*"),
        ("_i_", "*i*"),
        ("~~s~~", "~~s~~"),
        ("`c`", "`c`"),
        ("a *i* b", "a *i* b"),
        ("x **bold** y `c` z", "x **bold** y `c` z"),
    ];
    let mut bad = Vec::new();
    for (typed, expect) in inline {
        reset(app, "");
        typ(app, typed);
        let got = md();
        let shown = tv_of(app).string().to_string();
        if got != md_of(expect) || shown.contains('*') || shown.contains('~') || shown.contains('`') {
            bad.push(format!("{typed:?}: {got:?} shown {shown:?}"));
        }
    }
    check("inline autoformat (** __ * _ ~~ `) styles the text and removes the markers", bad.is_empty(), &bad.join(" | "));
    reset(app, "");
    typ(app, "snake_case_word");
    check("no `_` autoformat inside words", tv_of(app).string().to_string() == "snake_case_word", &tv_of(app).string().to_string());

    // 4. Enter / Tab / Backspace
    let ret = |app: &NSApplication| key(app, "\r", 36, NONE);
    let tab = |app: &NSApplication| key(app, "\t", 48, NONE);
    let backtab = |app: &NSApplication| key(app, "\u{19}", 48, SHIFT);
    let bs = |app: &NSApplication| key(app, "\u{7f}", 51, NONE);
    reset(app, "- a");
    ret(app);
    typ(app, "b");
    check("Enter in a bullet continues the list", md() == md_of("- a\n- b"), &md());
    ret(app);
    ret(app);
    check("Enter on an empty item exits the list", kinds(&md()) == vec![BlockKind::Bullet, BlockKind::Bullet, BlockKind::Paragraph], &md());
    reset(app, "- a");
    ret(app);
    typ(app, "b");
    tab(app);
    check("Tab indents the item", md() == md_of("- a\n  - b"), &md());
    tab(app);
    check("Tab cannot go deeper than previous level + 1", md() == md_of("- a\n  - b"), &md());
    backtab(app);
    check("Shift-Tab outdents the item", md() == md_of("- a\n- b"), &md());
    reset(app, "- a\n- b");
    tab(app);
    ret(app);
    ret(app);
    check("Enter on an empty indented item outdents first", kinds(&md()).len() == 3 && md() == md_of("- a\n  - b\n-"), &md());
    reset(app, "1. a\n2. b");
    tv_of(app).setSelectedRange(NSRange::new(4, 0));
    ret(app);
    typ(app, "x");
    check("ordered list renumbers live", md() == md_of("1. a\n2. x\n3. b"), &md());
    reset(app, "- [x] done");
    ret(app);
    typ(app, "next");
    check("Enter in a task starts an unchecked task", md() == md_of("- [x] done\n- [ ] next"), &md());
    reset(app, "> q");
    ret(app);
    typ(app, "r");
    check("Enter in a quote continues it", md() == md_of("> q\n> r"), &md());
    ret(app);
    ret(app);
    check("Enter on an empty quote line exits it", kinds(&md()).last() == Some(&BlockKind::Paragraph), &md());
    reset(app, "- a");
    tv_of(app).setSelectedRange(NSRange::new(0, 0));
    check(
        "cursor cannot sit inside the list marker",
        tv_of(app).selectedRange().location == 2,
        &format!("{:?}", tv_of(app).selectedRange()),
    );
    bs(app);
    check("Backspace at the start of a list item removes the list format", md() == "a", &md());
    reset(app, "> q");
    tv_of(app).setSelectedRange(NSRange::new(0, 0));
    bs(app);
    check("Backspace at the start of a quote makes it a paragraph", md() == "q", &md());
    reset(app, "x\n- a");
    tv_of(app).setSelectedRange(NSRange::new(4, 0));
    key(app, "\u{f702}", 123, NONE);
    check(
        "Left at the start of an item goes to the end of the previous line",
        tv_of(app).selectedRange().location == 1,
        &format!("{:?}", tv_of(app).selectedRange()),
    );
    reset(app, "a\n- b");
    tv_of(app).setSelectedRange(NSRange::new(1, 0));
    key(app, "\u{f728}", 117, NONE);
    check("Forward delete joins the next list item without its marker", md() == "ab", &md());
    reset(app, "# H");
    ret(app);
    typ(app, "x");
    check("Enter after a heading starts a paragraph", md() == md_of("# H\n\nx"), &md());
    reset(app, "```\nab\n```");
    ret(app);
    typ(app, "c");
    check(
        "Enter in a code block adds a line inside it",
        kinds(&md()) == vec![BlockKind::Code { lang: String::new() }] && md() == md_of("```\nab\nc\n```"),
        &md(),
    );
    ret(app);
    ret(app);
    typ(app, "z");
    check("Enter on an empty last code line leaves the block", md() == md_of("```\nab\nc\n```\n\nz"), &md());
    reset(app, "a\n\n---\n\nb");
    let rule_start = notes::paragraph_start(1).unwrap_or(0);
    tv_of(app).setSelectedRange(NSRange::new(rule_start + 1, 0));
    bs(app);
    check("Backspace on a rule removes it", !md().contains("---"), &md());

    // edits in the middle of a document keep the surrounding blocks intact
    reset(app, "a\n\nb\n\nc");
    tv_of(app).setSelectedRange(NSRange::new(3, 0));
    key(app, "1", 18, CMD.union(OPT));
    check("block format in the middle of a note", md() == md_of("a\n\n# b\n\nc"), &md());
    key(app, "1", 18, CMD.union(OPT));
    check("...and toggles back there", md() == md_of("a\n\nb\n\nc"), &md());
    reset(app, "x\n- a\ny");
    tv_of(app).setSelectedRange(NSRange::new(4, 0));
    bs(app);
    check("Backspace on a middle list item keeps the neighbours", md() == md_of("x\n\na\n\ny"), &md());
    reset(app, "# H\nz");
    tv_of(app).setSelectedRange(NSRange::new(1, 0));
    ret(app);
    typ(app, "m");
    check("Enter after a middle heading starts a paragraph", md() == md_of("# H\n\nm\n\nz"), &md());
    reset(app, "- a\n- b\n- c");
    tv_of(app).setSelectedRange(NSRange::new(6, 0));
    tab(app);
    check("Tab on a middle item", md() == md_of("- a\n  - b\n- c"), &md());
    reset(app, "- [ ] a\n- [ ] b\n- [ ] c");
    tv_of(app).setSelectedRange(NSRange::new(6, 0));
    key(app, "\r", 36, CMD);
    check("⌘Enter on a middle task", md() == md_of("- [ ] a\n- [x] b\n- [ ] c"), &md());

    // 5. key shortcuts toggle on and off
    let keys: &[(&str, &str, u16, NSEventModifierFlags, &str)] = &[
        ("⌘B", "b", 11, CMD, "**hello**"),
        ("⌘I", "i", 34, CMD, "*hello*"),
        ("⇧⌘S", "s", 1, CMD.union(SHIFT), "~~hello~~"),
        ("⌘U", "u", 32, CMD, "<u>hello</u>"),
        ("⌘E", "e", 14, CMD, "`hello`"),
        ("⌥⌘1", "1", 18, CMD.union(OPT), "# hello"),
        ("⌥⌘2", "2", 19, CMD.union(OPT), "## hello"),
        ("⌥⌘3", "3", 20, CMD.union(OPT), "### hello"),
        ("⌥⌘C", "c", 8, CMD.union(OPT), "```\nhello\n```"),
        ("⇧⌘B", "b", 11, CMD.union(SHIFT), "> hello"),
        ("⇧⌘7", "&", 26, CMD.union(SHIFT), "1. hello"),
        ("⇧⌘8", "*", 28, CMD.union(SHIFT), "- hello"),
        ("⇧⌘9", "(", 25, CMD.union(SHIFT), "- [ ] hello"),
    ];
    for (name, chars, code, mods, expect) in keys {
        reset(app, "hello");
        select_all(app);
        key(app, chars, *code, *mods);
        let on = md();
        let active = !notes::active_formats().is_empty();
        key(app, chars, *code, *mods);
        let off = md();
        check(
            &format!("{name} applies and toggles off"),
            on == md_of(expect) && off == "hello" && active,
            &format!("on={on:?} off={off:?} active={active}"),
        );
    }
    reset(app, "hello");
    select_all(app);
    key(app, "b", 11, CMD);
    check("active_formats reports Bold", notes::active_formats() == vec![Format::Bold], &format!("{:?}", notes::active_formats()));
    key(app, "z", 6, CMD);
    check("⌘Z undoes an applied format", md() == "hello", &md());
    key(app, "z", 6, CMD | SHIFT);
    check("⇧⌘Z redoes it", md() == md_of("**hello**"), &md());
    reset(app, "hello");
    select_all(app);
    key(app, "1", 18, CMD.union(OPT));
    key(app, "z", 6, CMD);
    check("⌘Z undoes a block format", md() == "hello" && kinds(&md()) == vec![BlockKind::Paragraph], &md());
    reset(app, "");
    key(app, "b", 11, CMD);
    typ(app, "bold");
    key(app, "b", 11, CMD);
    typ(app, " plain");
    check("⌘B with no selection toggles the typing style", md() == md_of("**bold** plain"), &md());
    reset(app, "a\n\nb");
    select_all(app);
    key(app, "8", 28, CMD.union(SHIFT));
    check("list format applies to every selected paragraph", md() == md_of("- a\n- b"), &md());

    // links
    reset(app, "hello");
    select_all(app);
    key(app, "l", 37, CMD);
    check(
        "⌘L opens the URL prompt (empty)",
        notes::link_prompt_open() && notes::link_prompt_value().as_deref() == Some(""),
        &format!("{:?}", notes::link_prompt_value()),
    );
    notes::link_prompt_submit("https://example.com");
    wait_key(1500);
    check("link prompt confirm adds the link", md() == md_of("[hello](https://example.com)") && !notes::link_prompt_open(), &md());
    notes::focus_editor();
    tv_of(app).setSelectedRange(NSRange::new(2, 0));
    key(app, "l", 37, CMD);
    check(
        "⌘L on a link is prefilled",
        notes::link_prompt_value().as_deref() == Some("https://example.com"),
        &format!("{:?}", notes::link_prompt_value()),
    );
    notes::link_prompt_cancel();
    check(
        "Esc/cancel closes the prompt without changes",
        !notes::link_prompt_open() && md() == md_of("[hello](https://example.com)"),
        &md(),
    );
    wait_key(1500);
    notes::focus_editor();
    tv_of(app).setSelectedRange(NSRange::new(2, 0));
    key(app, "l", 37, CMD);
    notes::link_prompt_submit("");
    check("empty URL removes the link", md() == "hello", &md());
    wait_key(1500);
    notes::focus_editor();
    reset(app, "see [site](https://a.b)");
    tv_of(app).setSelectedRange(NSRange::new(text_len(&tv_of(app)), 0));
    typ(app, " more");
    check("text typed after a link is not part of it", md() == md_of("see [site](https://a.b) more"), &md());

    // tasks
    reset(app, "- [ ] t");
    key(app, "\r", 36, CMD);
    check("⌘Enter checks the task", md() == md_of("- [x] t"), &md());
    key(app, "\r", 36, CMD);
    check("⌘Enter unchecks it", md() == md_of("- [ ] t"), &md());
    if let Some((x, y)) = notes::window_point_for_char(0) {
        mouse_click(app, x, y);
    }
    check("clicking the checkbox toggles the task", md() == md_of("- [x] t"), &md());
    key(app, "z", 6, CMD);
    check("task toggle is undoable", md() == md_of("- [ ] t"), &md());

    // 6. zoom
    let zooms: Rc<RefCell<Vec<f64>>> = Rc::default();
    {
        let z = zooms.clone();
        notes::set_on_zoom_change(Box::new(move |v| z.borrow_mut().push(v)));
    }
    reset(app, "plain");
    notes::set_zoom(1.0);
    zooms.borrow_mut().clear();
    key(app, "=", 24, CMD);
    key(app, "+", 24, CMD | SHIFT);
    let up = notes::zoom();
    key(app, "-", 27, CMD);
    let down = notes::zoom();
    check("⌘= / ⌘+ / ⌘- zoom in steps of 10 %", (up - 1.2).abs() < 1e-6 && (down - 1.1).abs() < 1e-6, &format!("{up} {down}"));
    key(app, "0", 29, CMD);
    check("⌘0 resets the zoom", (notes::zoom() - 1.0).abs() < 1e-9, &notes::zoom().to_string());
    check("zoom callback receives every step", *zooms.borrow() == vec![1.1, 1.2, 1.1, 1.0], &format!("{:?}", zooms.borrow()));
    for _ in 0..8 {
        key(app, "-", 27, CMD);
    }
    check("zoom stops at 50 %", (notes::zoom() - 0.5).abs() < 1e-9, &notes::zoom().to_string());
    for _ in 0..30 {
        key(app, "=", 24, CMD);
    }
    check("zoom stops at 300 %", (notes::zoom() - 3.0).abs() < 1e-9, &notes::zoom().to_string());
    notes::set_zoom(1.5);
    check("set_zoom scales font sizes", (font_size_at(&tv_of(app), 0) - 21.0).abs() < 0.01, &font_size_at(&tv_of(app), 0).to_string());
    notes::set_zoom(1.0);
    check("zoom 100 % gives 14 pt", (font_size_at(&tv_of(app), 0) - 14.0).abs() < 0.01, &font_size_at(&tv_of(app), 0).to_string());
    reset(app, "# H");
    check("heading 1 is 24 pt", (font_size_at(&tv_of(app), 0) - 24.0).abs() < 0.01, &font_size_at(&tv_of(app), 0).to_string());
    check(
        "zoom does not change the Markdown",
        {
            notes::set_zoom(2.0);
            let m = md();
            notes::set_zoom(1.0);
            m == "# H"
        },
        &md(),
    );

    // 7. slash menu, emoji, pop-up keys
    let slash: Rc<RefCell<Vec<SlashEvent>>> = Rc::default();
    let emoji: Rc<RefCell<Vec<EmojiEvent>>> = Rc::default();
    let popup: Rc<RefCell<Vec<PopupKey>>> = Rc::default();
    {
        let s = slash.clone();
        notes::set_on_slash(Box::new(move |e| s.borrow_mut().push(e)));
        let e = emoji.clone();
        notes::set_on_emoji(Box::new(move |ev| e.borrow_mut().push(ev)));
        let p = popup.clone();
        notes::set_popup_key_handler(Box::new(move |k| {
            p.borrow_mut().push(k);
            k != PopupKey::Esc
        }));
    }
    reset(app, "");
    typ(app, "/");
    let opened = matches!(slash.borrow().first(), Some(SlashEvent::Open { anchor: Anchor { h, .. } }) if *h > 0.0);
    typ(app, "he");
    check(
        "typing `/` on an empty line fires Open then Query for each letter",
        opened && slash.borrow()[1..] == [SlashEvent::Query(String::new()), SlashEvent::Query("h".into()), SlashEvent::Query("he".into())],
        &format!("{:?}", slash.borrow()),
    );
    popup.borrow_mut().clear();
    key(app, "\u{f701}", 125, NONE);
    key(app, "\u{f700}", 126, NONE);
    key(app, "\r", 36, NONE);
    key(app, "\t", 48, NONE);
    check(
        "Up/Down/Enter/Tab are routed to the pop-up handler and consumed",
        *popup.borrow() == vec![PopupKey::Down, PopupKey::Up, PopupKey::Enter, PopupKey::Tab] && md() == md_of("/he"),
        &format!("{:?} {:?}", popup.borrow(), md()),
    );
    notes::slash_commit(Format::Heading(1));
    typ(app, "T");
    check("slash_commit removes `/query` and applies the format", md() == md_of("# T"), &md());
    slash.borrow_mut().clear();
    reset(app, "");
    typ(app, "/x");
    slash.borrow_mut().clear();
    key(app, "\u{1b}", 53, NONE);
    check(
        "Esc goes to the pop-up first, then closes the session without hiding the window",
        slash.borrow().last() == Some(&SlashEvent::Close) && notes::is_visible() && popup.borrow().last() == Some(&PopupKey::Esc),
        &format!("{:?}", slash.borrow()),
    );
    reset(app, "");
    typ(app, "/");
    slash.borrow_mut().clear();
    key(app, "\u{7f}", 51, NONE);
    check("deleting past `/` closes the session", slash.borrow().last() == Some(&SlashEvent::Close), &format!("{:?}", slash.borrow()));
    slash.borrow_mut().clear();
    reset(app, "a");
    typ(app, "/");
    check("`/` after text does not open the menu", slash.borrow().is_empty(), &format!("{:?}", slash.borrow()));
    reset(app, "");
    typ(app, "/he");
    tv_of(app).setSelectedRange(NSRange::new(0, 0));
    check("moving the cursor away closes the session", slash.borrow().last() == Some(&SlashEvent::Close), &format!("{:?}", slash.borrow()));
    popup.borrow_mut().clear();
    reset(app, "a");
    key(app, "\r", 36, NONE);
    check(
        "keys are not routed when no pop-up is open",
        popup.borrow().is_empty() && kinds(&md()).len() == 2,
        &format!("{:?}", popup.borrow()),
    );

    reset(app, "");
    typ(app, "hi :");
    let opened = matches!(emoji.borrow().first(), Some(EmojiEvent::Open { .. }));
    typ(app, "smi");
    check(
        "typing `:` fires emoji Open then Query",
        opened && emoji.borrow().last() == Some(&EmojiEvent::Query("smi".into())),
        &format!("{:?}", emoji.borrow()),
    );
    notes::emoji_commit("😀");
    check("emoji_commit replaces `:query` with the emoji", md() == "hi 😀", &md());
    emoji.borrow_mut().clear();
    reset(app, "");
    typ(app, "at 10:30");
    check("`:` inside a word or time opens nothing", emoji.borrow().is_empty(), &format!("{:?}", emoji.borrow()));
    reset(app, "");
    typ(app, ":sm");
    typ(app, " ");
    check("a space closes the emoji session", emoji.borrow().last() == Some(&EmojiEvent::Close), &format!("{:?}", emoji.borrow()));
    notes::set_on_slash(Box::new(|_| {}));
    notes::set_on_emoji(Box::new(|_| {}));

    // 8. selection, replace, insert, paste
    reset(app, "# Title\n\n- one\n- **two**");
    select_all(app);
    let (text, a, b) = notes::selection();
    check(
        "selection() returns marker-free plain text and the range",
        text == "Title\none\ntwo" && a == 0 && b == text_len(&tv_of(app)),
        &format!("{text:?} {a} {b}"),
    );
    check(
        "markdown_of_selection() keeps Markdown",
        notes::markdown_of_selection() == md_of("# Title\n\n- one\n- **two**"),
        &notes::markdown_of_selection(),
    );
    tv_of(app).setSelectedRange(NSRange::new(3, 0));
    check("selection() is empty without a selection", notes::selection() == (String::new(), 3, 3), &format!("{:?}", notes::selection()));
    reset(app, "hello **world**");
    notes::replace_range(0, 5, "HELLO");
    check("replace_range replaces plain text", md() == md_of("HELLO **world**"), &md());
    key(app, "z", 6, CMD);
    check("replace_range is undoable", md() == md_of("hello **world**"), &md());
    notes::replace_range(6, 7, "W");
    check("replace_range takes the style of the start", md() == md_of("hello **World**"), &md());
    reset(app, "ab");
    tv_of(app).setSelectedRange(NSRange::new(1, 0));
    notes::insert_at_cursor("XY");
    check("insert_at_cursor inserts at the cursor", md() == "aXYb", &md());
    key(app, "z", 6, CMD);
    check("insert_at_cursor is undoable", md() == "ab", &md());
    reset(app, "- a");
    notes::insert_at_cursor("\nline2");
    check("multi-line insert becomes paragraphs", md().contains("line2") && kinds(&md()).len() == 2, &md());

    markdown_result_checks(app);

    let pb = NSPasteboard::generalPasteboard();
    let saved = pb.stringForType(unsafe { NSPasteboardTypeString });
    pb.clearContents();
    pb.setString_forType(&NSString::from_str("**x** # y\n- z"), unsafe { NSPasteboardTypeString });
    reset(app, "");
    key(app, "v", 9, CMD);
    let shown = tv_of(app).string().to_string();
    check(
        "pasted Markdown stays as text",
        shown == "**x** # y\n- z" && richtext::parse(&md()).blocks.iter().all(|b| b.kind == BlockKind::Paragraph),
        &format!("{shown:?} {:?}", md()),
    );
    reset(app, "- a\n- b");
    select_all(app);
    key(app, "c", 8, CMD);
    let copied = pb.stringForType(unsafe { NSPasteboardTypeString }).map(|s| s.to_string());
    check("copy gives marker-free plain text", copied.as_deref() == Some("a\nb"), &format!("{copied:?}"));
    match saved {
        Some(s) => {
            pb.clearContents();
            pb.setString_forType(&s, unsafe { NSPasteboardTypeString });
        }
        None => {
            pb.clearContents();
        }
    }

    // existing shortcuts still work with the rich editor
    reset(app, "x");
    let before = md();
    key(app, "a", 0, CMD);
    key(app, "x", 7, CMD);
    key(app, "z", 6, CMD);
    check("⌘A ⌘X ⌘Z still work", md() == before, &md());

    // 9. snapshots
    let note = "# Ferrocast Notes\n\n## Second heading\n\n### Third heading\n\n\
        A paragraph with **bold**, *italic*, ~~strikethrough~~, <u>underline</u>, `inline code` and a [link](https://example.com), \
        long enough to wrap onto a second line of the window.\n\n\
        > A blockquote line\n> and its second line\n\n\
        - Bullet one\n  - Nested bullet\n    - Third level\n- Bullet two\n\n\
        1. First\n2. Second\n  1. Nested one\n  2. Nested two\n3. Third\n\n\
        - [ ] Open task\n- [x] Finished task\n\n\
        ```rust\nfn main() {\n\n    println!(\"hi\");\n}\n```\n\n---\n\nThe end.";
    reset(app, note);
    tv_of(app).setSelectedRange(NSRange::new(0, 0));
    notes::set_zoom(1.0);
    // a tall window so the whole note is visible in the snapshots
    let win = app.keyWindow();
    if let Some(w) = &win {
        w.setContentSize(objc2_foundation::NSSize::new(600.0, 1050.0));
    }
    for (name, appearance, z) in [
        ("light", unsafe { NSAppearanceNameAqua }, 1.0),
        ("dark", unsafe { NSAppearanceNameDarkAqua }, 1.0),
        ("light-zoom150", unsafe { NSAppearanceNameAqua }, 1.5),
        ("dark-zoom150", unsafe { NSAppearanceNameDarkAqua }, 1.5),
        ("light-zoom50", unsafe { NSAppearanceNameAqua }, 0.5),
        ("dark-zoom50", unsafe { NSAppearanceNameDarkAqua }, 0.5),
    ] {
        app.setAppearance(NSAppearance::appearanceNamed(appearance).as_deref());
        notes::set_zoom(z);
        if let Some(w) = &win {
            w.setContentSize(objc2_foundation::NSSize::new(if z > 1.2 { 840.0 } else { 600.0 }, if z > 1.2 { 1450.0 } else { 1050.0 }));
        }
        pump(400);
        let path = out.join(format!("notes-{name}.png"));
        let ok = notes::snapshot_png(&path);
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        check(&format!("snapshot {name} written"), ok && size > 1000, &format!("ok={ok} size={size}"));
        println!("      {}", path.display());
    }
    notes::set_zoom(1.0);
    if let Some(w) = &win {
        w.setContentSize(objc2_foundation::NSSize::new(560.0, 420.0));
    }
    app.setAppearance(None);
    check("snapshot note round-trips", md() == md_of(note), &md());
    theme_checks(app, out);
    style_checks(app);
    reset(app, note);
}

/// T2 (S4, S6, S7): heading hierarchy at every zoom level, and the selection stays readable.
fn style_checks(app: &NSApplication) {
    use ferrocast_macos::notes_theme::{self as theme, Rgba};
    use objc2_foundation::NSObjectProtocol as _;
    reset(app, "# Alpha\n\n## Beta\n\n### Gamma\n\nDelta body");
    let text = tv_of(app).string().to_string();
    let at = |w: &str| text.find(w).expect("word in text");
    for z in [0.5, 1.0, 1.5, 3.0] {
        notes::set_zoom(z);
        let tv = tv_of(app);
        let (h1, h2, h3, body) =
            (font_size_at(&tv, at("Alpha")), font_size_at(&tv, at("Beta")), font_size_at(&tv, at("Gamma")), font_size_at(&tv, at("Delta")));
        check(
            &format!("zoom {z}: headings are larger than the body and strictly ordered 1 > 2 > 3"),
            body > 0.0 && h3 > body && h2 > h3 && h1 > h2,
            &format!("h1 {h1} h2 {h2} h3 {h3} body {body}"),
        );
    }
    notes::set_zoom(1.0);
    let alpha = ferrocast_macos::notes_editor::SELECTION_ALPHA;
    for (name, t, dark) in [("dark", theme::DARK, true), ("light", theme::LIGHT, false)] {
        let bg = theme::window_bg(dark);
        let sel_bg =
            theme::composite_over(Rgba(t.accent.0, t.accent.1, t.accent.2, (alpha * 255.0).round() as u8), Rgba(bg.0, bg.1, bg.2, 255));
        let ratio = theme::contrast_ratio(theme::composite_over(t.text, Rgba(sel_bg.0, sel_bg.1, sel_bg.2, 255)), sel_bg);
        check(
            &format!("selection {name}: Text over the composited Accent at {:.0}% reaches 4.5:1", alpha * 100.0),
            ratio >= 4.5,
            &format!("selection bg {sel_bg:?}, ratio {ratio:.2}"),
        );
    }
    let tv = tv_of(app);
    let caret = tv.insertionPointColor();
    let sel_attrs = tv.selectedTextAttributes();
    check(
        "caret and selection use the accent, the selection keeps the text colour",
        !caret.isEqual(Some(&objc2_app_kit::NSColor::controlAccentColor()))
            && sel_attrs.count() == 1
            && unsafe { sel_attrs.objectForKey(objc2_app_kit::NSBackgroundColorAttributeName) }.is_some(),
        &format!("attrs {}", sel_attrs.count()),
    );
}

/// Read pixel `(x, y)` (top-left origin) of a snapshot image as 0 to 255 per channel.
fn pixel(rep: &objc2_app_kit::NSBitmapImageRep, x: isize, y: isize) -> (u8, u8, u8) {
    // Raw stored components, no colour-space conversion: the snapshot is tagged with this Mac's
    // display profile but holds the values AppKit drew, so converting would make the check
    // depend on the monitor.
    let c = rep.colorAtX_y(x, y).expect("pixel colour");
    let f = |v: f64| (v * 255.0).round() as u8;
    (f(c.redComponent()), f(c.greenComponent()), f(c.blueComponent()))
}

fn close(a: (u8, u8, u8), b: (u8, u8, u8), tol: i32) -> bool {
    [(a.0, b.0), (a.1, b.1), (a.2, b.2)].iter().all(|&(x, y)| (x as i32 - y as i32).abs() <= tol)
}

/// T1 (S2, S5, S8): the window chrome uses the app palette in both themes, whatever the macOS
/// appearance of this Mac is, and the setting round-trips.
fn theme_checks(app: &NSApplication, out: &std::path::Path) {
    use ferrocast_macos::notes_theme::{self as theme, Appearance, Rgba};
    reset(app, "# Theme\n\nShort note.");
    let mut heading_ink: Vec<(u8, u8, u8)> = Vec::new();
    for (name, setting, dark) in [("dark", Appearance::Dark, true), ("light", Appearance::Light, false)] {
        theme::set_appearance(setting);
        check(
            &format!("theme {name}: appearance() returns what was set"),
            theme::appearance() == setting,
            &format!("{:?}", theme::appearance()),
        );
        pump(300);
        let path = out.join(format!("notes-theme-{name}.png"));
        let ok = notes::snapshot_png(&path);
        check(&format!("theme {name}: snapshot written"), ok, "failed");
        let rep = std::fs::read(&path)
            .ok()
            .and_then(|d| objc2_app_kit::NSBitmapImageRep::imageRepWithData(&objc2_foundation::NSData::with_bytes(&d)));
        let Some(rep) = rep else {
            check(&format!("theme {name}: snapshot readable"), false, "no image");
            continue;
        };
        let (w, h) = (rep.pixelsWide(), rep.pixelsHigh());
        let t = if dark { theme::DARK } else { theme::LIGHT };
        let bg = theme::window_bg(dark);
        let got_bg = pixel(&rep, w / 2, h * 4 / 5);
        check(
            &format!("theme {name}: window background is the opaque bg token"),
            close(got_bg, bg, 2),
            &format!("got {got_bg:?} want {bg:?}"),
        );
        let scale = w as f64 / app.keyWindow().map_or(560.0, |win| win.contentView().map_or(560.0, |v| v.bounds().size.width));
        let row = (43.5 * scale).round() as isize;
        let got_sep = pixel(&rep, w / 4, row);
        let want_sep = theme::composite_over(t.sep, Rgba(bg.0, bg.1, bg.2, 255));
        check(
            &format!("theme {name}: hairline row differs from the background"),
            !close(got_sep, got_bg, 1),
            &format!("sep {got_sep:?} bg {got_bg:?}"),
        );
        check(
            &format!("theme {name}: hairline is the composited sep token"),
            close(got_sep, want_sep, 6),
            &format!("got {got_sep:?} want {want_sep:?}"),
        );
        // The heading glyph pixels (the pixel farthest from the background in the heading band):
        // the text was laid out once and only the appearance changed, so a dynamic colour is
        // what recolours it.
        let mut ink = (got_bg, 0i32);
        for y in (46.0 * scale) as isize..(100.0 * scale) as isize {
            for x in 0..w / 2 {
                let p = pixel(&rep, x, y);
                let d = (p.0 as i32 - got_bg.0 as i32).abs() + (p.1 as i32 - got_bg.1 as i32).abs() + (p.2 as i32 - got_bg.2 as i32).abs();
                if d > ink.1 {
                    ink = (p, d);
                }
            }
        }
        let want = (t.text.0, t.text.1, t.text.2);
        check(
            &format!("theme {name}: heading text pixels are nearer the Text token than the background"),
            ink.1 > 150 && close(ink.0, want, 70),
            &format!("ink {:?} want {want:?} bg {got_bg:?}", ink.0),
        );
        heading_ink.push(ink.0);
        println!("      {}", path.display());
    }
    if let [dark_ink, light_ink] = heading_ink[..] {
        check(
            "theme: heading pixels differ between Dark and Light without set_markdown",
            !close(dark_ink, light_ink, 60) && dark_ink.0 > light_ink.0,
            &format!("dark {dark_ink:?} light {light_ink:?}"),
        );
    }
    theme::set_appearance(Appearance::System);
    check("theme: restored to System", theme::appearance() == Appearance::System, "");
    pump(100);
}

// ------------------------------------------------------------------ window integration checks

fn mouse_event(ty: NSEventType, window_number: isize, x: f64, y: f64, n: isize) -> Retained<NSEvent> {
    NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
        ty,
        NSPoint::new(x, y),
        NONE,
        NSProcessInfo::processInfo().systemUptime(),
        window_number,
        None,
        n,
        1,
        1.0,
    )
    .expect("mouse event")
}

/// Click an `NSButton`: its tracking loop waits for the mouse-up, so queue it first.
fn click_button(app: &NSApplication, x: f64, y: f64) {
    let win = app.keyWindow().map(|w| w.windowNumber()).unwrap_or(0);
    let up = mouse_event(NSEventType::LeftMouseUp, win, x, y, 1);
    app.postEvent_atStart(&up, true);
    app.sendEvent(&mouse_event(NSEventType::LeftMouseDown, win, x, y, 0));
    pump(80);
}

/// Window-coordinate rectangle `(x, y, w, h)` of the button with this tooltip.
fn button_rect(app: &NSApplication, tip: &str) -> Option<(f64, f64, f64, f64)> {
    let content = app.keyWindow()?.contentView()?;
    let b = find_button(&content, tip)?;
    let r = b.convertRect_toView(b.bounds(), None);
    Some((r.origin.x, r.origin.y, r.size.width, r.size.height))
}

fn click_tip(app: &NSApplication, tip: &str) -> bool {
    match button_rect(app, tip) {
        Some((x, y, w, h)) => {
            click_button(app, x + w / 2.0, y + h / 2.0);
            true
        }
        None => false,
    }
}

fn bar_button_on(app: &NSApplication, tip: &str) -> Option<bool> {
    let content = app.keyWindow()?.contentView()?;
    find_button(&content, tip).map(|b| b.state() == 1)
}

fn set_window_size(app: &NSApplication, w: f64, h: f64) {
    if let Some(win) = app.keyWindow() {
        win.setContentSize(objc2_foundation::NSSize::new(w, h));
    }
    pump(150);
}

fn editor_scroll_bottom(app: &NSApplication) -> f64 {
    tv_of(app).enclosingScrollView().map(|s| s.frame().origin.y).unwrap_or(-1.0)
}

/// Click row `row` of a popup with a real mouse event addressed to the popup window.
fn click_popup_row(app: &NSApplication, popup: &ferrocast_macos::notes_popups::SuggestionPopup, row: usize) {
    let f = popup.frame();
    let y = f.h - ferrocast_macos::notes_popups::POPUP_PAD - (row as f64 + 0.5) * ferrocast_macos::notes_popups::ROW_HEIGHT;
    let win = popup.window_number();
    app.sendEvent(&mouse_event(NSEventType::LeftMouseDown, win, 100.0, y, 0));
    app.sendEvent(&mouse_event(NSEventType::LeftMouseUp, win, 100.0, y, 1));
    pump(80);
}

fn slash_visible() -> bool {
    notes::slash_popup().is_some_and(|p| p.is_visible())
}

fn emoji_visible() -> bool {
    notes::emoji_popup().is_some_and(|p| p.is_visible())
}

fn fake_emoji(q: &str) -> Vec<(String, String)> {
    if q.starts_with("smi") {
        vec![("😀".into(), "grinning face".into()), ("😄".into(), "smile".into())]
    } else {
        Vec::new()
    }
}

fn reload(app: &NSApplication, md: &str) {
    reset(app, md);
    notes::install_popup_hooks();
}

#[allow(clippy::too_many_lines)]
fn window_checks(app: &NSApplication, out: &std::path::Path, events: &Rc<RefCell<Vec<Event>>>) {
    let md = notes::markdown;
    let changes: Rc<RefCell<Vec<String>>> = Rc::default();
    {
        let c = changes.clone();
        notes::set_on_change(Box::new(move |t| c.borrow_mut().push(t)));
    }
    notes::set_escape_loses_focus(false);
    notes::set_zoom(1.0);
    notes::install_popup_hooks();
    notes::show();
    wait_key(1500);
    set_window_size(app, 560.0, 420.0);

    // ---- window shortcuts fire their events once and never edit the note
    reset(app, "keep me");
    select_all(app);
    events.borrow_mut().clear();
    changes.borrow_mut().clear();
    let digit_codes: [u16; 9] = [18, 19, 20, 21, 23, 22, 26, 28, 25];
    key(app, "k", 40, CMD);
    check("⌘K fires Actions once", *events.borrow() == vec![Event::Actions], &format!("{:?}", events.borrow()));
    events.borrow_mut().clear();
    key(app, "[", 33, CMD);
    check("⌘[ fires Back once", *events.borrow() == vec![Event::Back], &format!("{:?}", events.borrow()));
    events.borrow_mut().clear();
    key(app, "]", 30, CMD);
    check("⌘] fires Forward once", *events.borrow() == vec![Event::Forward], &format!("{:?}", events.borrow()));
    events.borrow_mut().clear();
    key(app, "p", 35, CMD | SHIFT);
    check("⇧⌘P fires TogglePin once (not Browse)", *events.borrow() == vec![Event::TogglePin], &format!("{:?}", events.borrow()));
    events.borrow_mut().clear();
    for (i, code) in digit_codes.iter().enumerate() {
        key(app, &(i + 1).to_string(), *code, CMD);
    }
    check(
        "⌘1 to ⌘9 fire Pinned(1..=9) once each",
        *events.borrow() == (1..=9).map(Event::Pinned).collect::<Vec<_>>(),
        &format!("{:?}", events.borrow()),
    );
    events.borrow_mut().clear();
    key(app, "0", 29, CMD);
    check(
        "⌘0 stays zoom reset and fires nothing",
        events.borrow().is_empty() && (notes::zoom() - 1.0).abs() < 1e-9,
        &format!("{:?}", events.borrow()),
    );
    check(
        "none of the shortcuts edited or replaced the note",
        md() == "keep me" && changes.borrow().is_empty() && tv_of(app).selectedRange().length == 7 && notes::is_key(),
        &format!("{:?} {:?} key={}", md(), changes.borrow(), notes::is_key()),
    );

    // ---- palette open: shortcuts are swallowed, ⌘[ goes up one level
    let item = |id: &str| PaletteItem { id: id.into(), title: id.into(), ..PaletteItem::default() };
    notes_palette::show("Search", vec![item("a"), item("b")]);
    notes_palette::push_level("Sub", vec![item("c")]);
    events.borrow_mut().clear();
    key(app, "[", 33, CMD);
    let up = notes_palette::depth() == 1;
    for (chars, code, mods) in [("k", 40u16, CMD), ("]", 30, CMD), ("p", 35, CMD | SHIFT), ("1", 18, CMD), ("[", 33, CMD)] {
        key(app, chars, code, mods);
    }
    check(
        "palette open: ⌘[ goes up one level, no shortcut fires, the note is untouched",
        up && events.borrow().is_empty() && md() == "keep me" && notes_palette::is_visible(),
        &format!("up={up} {:?} {:?}", events.borrow(), md()),
    );
    notes_palette::hide();
    pump(100);

    // ---- ⌘K closes an open popup first
    reload(app, "");
    typ(app, "/h");
    let was_open = slash_visible();
    events.borrow_mut().clear();
    key(app, "k", 40, CMD);
    check(
        "⌘K with the `/` popup open closes it and fires Actions once",
        was_open && !slash_visible() && *events.borrow() == vec![Event::Actions] && md() == md_of("/h"),
        &format!("open={was_open} {:?} {:?}", events.borrow(), md()),
    );
    reload(app, "hello");
    select_all(app);
    key(app, "l", 37, CMD);
    let prompt = notes::link_prompt_open();
    events.borrow_mut().clear();
    key(app, "k", 40, CMD);
    pump(200);
    check(
        "⌘K with the link prompt open closes it and fires Actions once",
        prompt && !notes::link_prompt_open() && *events.borrow() == vec![Event::Actions] && md() == "hello",
        &format!("prompt={prompt} {:?} {:?}", events.borrow(), md()),
    );
    wait_key(1500);
    notes::focus_editor();

    // ---- Shown / Hidden exactly once per transition
    events.borrow_mut().clear();
    notes::hide();
    notes::hide();
    notes::show();
    notes::show();
    wait_key(1500);
    key(app, "w", 13, CMD);
    notes::show();
    wait_key(1500);
    key(app, "\u{1b}", 53, NONE);
    notes::show();
    wait_key(1500);
    notes::toggle();
    notes::toggle();
    wait_key(1500);
    use Event::{Hidden as H, Shown as S};
    check(
        "Shown / Hidden fire once per transition (hide, show, ⌘W, Esc, toggle)",
        *events.borrow() == vec![H, S, H, S, H, S, H, S],
        &format!("{:?}", events.borrow()),
    );
    events.borrow_mut().clear();
    notes::set_escape_loses_focus(true);
    key(app, "\u{1b}", 53, NONE);
    pump(200);
    check(
        "Esc in loses-focus mode fires neither Shown nor Hidden",
        events.borrow().is_empty() && notes::is_visible(),
        &format!("{:?}", events.borrow()),
    );
    notes::set_escape_loses_focus(false);
    // the app gave up the focus: a fresh hide + show takes it back
    notes::hide();
    notes::show();
    wait_key(2000);
    events.borrow_mut().clear();

    // ---- the host calls back into the window from inside the callback
    let calls = Rc::new(Cell::new(0u32));
    {
        let c = calls.clone();
        let log = events.clone();
        notes::set_on_event(Box::new(move |ev| {
            log.borrow_mut().push(ev);
            if ev == Event::Actions {
                c.set(c.get() + 1);
                notes::hide();
                notes::show();
                notes::set_markdown("# Reentrant");
                notes_palette::show("Search", vec![PaletteItem { id: "x".into(), title: "x".into(), ..PaletteItem::default() }]);
                notes_palette::hide();
                notes::set_on_event(Box::new(|_| {}));
            }
        }));
    }
    reset(app, "before");
    key(app, "k", 40, CMD);
    pump(200);
    check(
        "on_event may call hide, show, set_markdown, the palette and set_on_event (no RefCell panic)",
        calls.get() == 1 && notes::title() == "Reentrant" && notes::is_visible(),
        &format!("calls={} title={:?} visible={}", calls.get(), notes::title(), notes::is_visible()),
    );
    {
        let log = events.clone();
        notes::set_on_event(Box::new(move |ev| {
            log.borrow_mut().push(ev);
            if ev == Event::Hidden {
                notes::show();
            }
        }));
    }
    events.borrow_mut().clear();
    notes::hide();
    pump(100);
    check(
        "a Hidden callback may show the window again",
        *events.borrow() == vec![Event::Hidden, Event::Shown] && notes::is_visible(),
        &format!("{:?}", events.borrow()),
    );
    {
        let log = events.clone();
        notes::set_on_event(Box::new(move |ev| log.borrow_mut().push(ev)));
    }
    wait_key(1500);

    // ---- Format Bar
    reset(app, "hello world");
    check("Format Bar is hidden by default", !notes::format_bar_visible(), "visible");
    let bottom_hidden = editor_scroll_bottom(app);
    notes::set_format_bar_visible(true);
    pump(150);
    let bold = bar_button_on(app, "Bold ⌘B");
    check(
        "set_format_bar_visible(true) shows the bar and shrinks the editor",
        notes::format_bar_visible() && bold.is_some() && bottom_hidden == 0.0 && (editor_scroll_bottom(app) - 36.0).abs() < 0.5,
        &format!("bottom {bottom_hidden} -> {}", editor_scroll_bottom(app)),
    );
    tv_of(app).setSelectedRange(NSRange::new(0, 5));
    pump(50);
    check(
        "Bold is not pressed on plain text",
        bar_button_on(app, "Bold ⌘B") == Some(false),
        &format!("{:?}", bar_button_on(app, "Bold ⌘B")),
    );
    let clicked = click_tip(app, "Bold ⌘B");
    check(
        "clicking Bold makes the selection bold (Markdown) and keeps the editor focused",
        clicked && md() == md_of("**hello** world") && first_responder_text_view(app).is_some() && notes::is_key(),
        &format!("clicked={clicked} {:?}", md()),
    );
    check(
        "active_formats follows and the Bold button is pressed",
        notes::active_formats() == vec![Format::Bold] && bar_button_on(app, "Bold ⌘B") == Some(true),
        &format!("{:?} {:?}", notes::active_formats(), bar_button_on(app, "Bold ⌘B")),
    );
    tv_of(app).setSelectedRange(NSRange::new(7, 3));
    pump(50);
    check(
        "moving the selection to plain text releases the Bold button",
        notes::active_formats().is_empty() && bar_button_on(app, "Bold ⌘B") == Some(false),
        &format!("{:?}", notes::active_formats()),
    );
    tv_of(app).setSelectedRange(NSRange::new(0, 5));
    key(app, "i", 34, CMD);
    check(
        "a key shortcut also updates the bar (Italic pressed)",
        bar_button_on(app, "Italic ⌘I") == Some(true) && bar_button_on(app, "Bold ⌘B") == Some(true),
        &format!("{:?}", notes::active_formats()),
    );
    key(app, "i", 34, CMD);
    click_tip(app, "Bold ⌘B");
    check("clicking Bold again removes it", md() == "hello world" && bar_button_on(app, "Bold ⌘B") == Some(false), &md());
    reset(app, "title");
    click_tip(app, "Heading 2 ⌥⌘2");
    check(
        "clicking Heading 2 formats the paragraph",
        md() == md_of("## title") && bar_button_on(app, "Heading 2 ⌥⌘2") == Some(true),
        &md(),
    );

    // narrow window: the bar scrolls horizontally, every button stays reachable
    set_window_size(app, 320.0, 420.0);
    let (content_w, visible_w) = notes::format_bar_metrics().unwrap_or((0.0, 0.0));
    println!("      bar content {content_w:.0} pt, visible {visible_w:.0} pt");
    check("bar content is about 500 pt", (440.0..=560.0).contains(&content_w), &content_w.to_string());
    check(
        "narrow window: bar content is wider than the window (scrolls)",
        content_w > visible_w && visible_w <= 321.0,
        &format!("{content_w} vs {visible_w}"),
    );
    let win_w = app.keyWindow().and_then(|w| w.contentView()).map(|c| c.bounds().size.width).unwrap_or(0.0);
    let rule = button_rect(app, "Horizontal Rule").unwrap_or((0.0, 0.0, 0.0, 0.0));
    check("before scrolling, the last button is outside the window", rule.0 + rule.2 > win_w + 0.5, &format!("{rule:?} width {win_w}"));
    snapshot(out, "bar-narrow-start");
    let reached = notes::format_bar_scroll_to(10_000.0);
    pump(100);
    let rule = button_rect(app, "Horizontal Rule").unwrap_or((0.0, 0.0, 0.0, 0.0));
    let bold_r = button_rect(app, "Bold ⌘B").unwrap_or((0.0, 0.0, 0.0, 0.0));
    check(
        "scrolled to the end, the last button is inside the window and the first one is out",
        reached > 0.0 && rule.0 >= -0.5 && rule.0 + rule.2 <= win_w + 0.5 && bold_r.0 + bold_r.2 < 0.5,
        &format!("offset {reached} rule {rule:?} bold {bold_r:?}"),
    );
    snapshot(out, "bar-narrow-end");
    reset(app, "text");
    let ok = click_tip(app, "Horizontal Rule");
    check("a scrolled-to button can be clicked (Horizontal Rule inserts a rule)", ok && md().contains("---"), &md());
    notes::format_bar_scroll_to(0.0);
    set_window_size(app, 560.0, 420.0);
    let (_, visible_w) = notes::format_bar_metrics().unwrap_or((0.0, 0.0));
    check("wide window: the whole bar fits again", visible_w >= content_w - 0.5, &format!("{visible_w} vs {content_w}"));

    // ---- slash menu end to end
    reload(app, "");
    typ(app, "/");
    let popup = notes::slash_popup();
    check(
        "typing `/` opens the popup (visible, never key, window stays key)",
        slash_visible() && popup.as_ref().is_some_and(|p| !p.is_key()) && notes::is_key() && first_responder_text_view(app).is_some(),
        "popup missing or key",
    );
    typ(app, "h2");
    check("the query filters the list (Heading 2 first)", popup.as_ref().and_then(|p| p.selected()) == Some(0) && slash_visible(), "");
    key(app, "\r", 36, NONE);
    typ(app, "T");
    check(
        "/h2 + Enter turns the paragraph into Heading 2 and the `/h2` text is gone",
        md() == md_of("## T") && tv_of(app).string().to_string() == "T" && !slash_visible(),
        &format!("{:?} {:?}", md(), tv_of(app).string().to_string()),
    );
    reload(app, "");
    typ(app, "/he");
    key(app, "\u{1b}", 53, NONE);
    check(
        "Esc closes the popup only: text stays, window stays visible",
        !slash_visible() && tv_of(app).string().to_string() == "/he" && notes::is_visible() && md() == md_of("/he"),
        &format!("{:?}", tv_of(app).string().to_string()),
    );
    reload(app, "");
    typ(app, "/");
    key(app, "\u{f701}", 125, NONE);
    check("Down moves the selection", popup_selected() == Some(1), &format!("{:?}", popup_selected()));
    key(app, "\u{f700}", 126, NONE);
    key(app, "\u{f700}", 126, NONE);
    check("Up wraps around", popup_selected() == Some(8), &format!("{:?}", popup_selected()));
    key(app, "\u{f701}", 125, NONE);
    key(app, "\u{f701}", 125, NONE);
    key(app, "\u{f701}", 125, NONE);
    key(app, "\r", 36, NONE);
    typ(app, "T");
    check("Up, Up, then Down x3 from the last row lands on row 2 and Enter picks Heading 3", md() == md_of("### T"), &md());
    reload(app, "");
    typ(app, "/code");
    key(app, "\t", 48, NONE);
    typ(app, "T");
    check("Tab picks too (Code Block)", md() == md_of("```\nT\n```"), &md());
    reload(app, "");
    typ(app, "/quote");
    if let Some(p) = notes::slash_popup() {
        click_popup_row(app, &p, 0);
    }
    typ(app, "T");
    check("clicking a row picks it (Blockquote)", md() == md_of("> T") && !slash_visible(), &md());
    reload(app, "");
    typ(app, "/zzz");
    check("an empty result hides the popup", !slash_visible(), "still visible");
    key(app, "\u{7f}", 51, NONE);
    key(app, "\u{7f}", 51, NONE);
    key(app, "\u{7f}", 51, NONE);
    check("...and a shorter query brings it back", slash_visible(), "not visible");
    check(
        "the popup never took key status during the flow",
        notes::slash_popup().is_some_and(|p| !p.is_key()) && notes::is_key(),
        "popup key",
    );

    // ---- emoji flow
    reload(app, "");
    typ(app, "hi :smi");
    check("without an emoji provider the popup never opens", notes::emoji_popup().is_none() && !emoji_visible(), "emoji popup opened");
    notes::set_emoji_provider(Box::new(fake_emoji));
    reload(app, "");
    typ(app, "hi :smi");
    check(
        "with a provider `:smi` opens the emoji popup",
        emoji_visible() && !notes::emoji_popup().is_some_and(|p| p.is_key()),
        "not visible",
    );
    key(app, "\r", 36, NONE);
    check(
        "Enter replaces `:smi` with the emoji",
        md() == "hi 😀" && tv_of(app).string().to_string() == "hi 😀" && !emoji_visible(),
        &format!("{:?}", md()),
    );
    reload(app, "");
    typ(app, ":smi");
    key(app, "\u{f701}", 125, NONE);
    key(app, "\r", 36, NONE);
    check("Down + Enter picks the second emoji", md() == "😄", &md());
    reload(app, "");
    typ(app, ":s");
    check("one character after `:` lists nothing", !emoji_visible(), "visible");
    typ(app, "mi");
    check("...two do", emoji_visible(), "not visible");
    key(app, "\u{1b}", 53, NONE);
    check("Esc closes the emoji popup and keeps the text", !emoji_visible() && md() == md_of(":smi") && notes::is_visible(), &md());

    // ---- popups hide with the window, on key loss, palette and set_markdown
    reload(app, "");
    typ(app, "/h");
    notes::hide();
    check("hide() hides the `/` popup", !slash_visible(), "visible");
    notes::show();
    wait_key(1500);
    reload(app, "");
    typ(app, "x :smi");
    key(app, "w", 13, CMD);
    check("⌘W hides the emoji popup", !emoji_visible() && !notes::is_visible(), "visible");
    notes::show();
    wait_key(1500);
    reload(app, "");
    typ(app, "/h");
    notes::toggle();
    check("toggle() hides the popup", !slash_visible(), "visible");
    notes::toggle();
    wait_key(1500);
    reload(app, "");
    typ(app, "/h");
    notes_palette::show("Search", vec![item("a")]);
    check("opening the palette hides the popup", !slash_visible(), "visible");
    notes_palette::hide();
    wait_key(1500);
    reload(app, "");
    typ(app, "/h");
    notes::set_markdown("# Replaced");
    check("set_markdown hides the popup", !slash_visible() && notes::title() == "Replaced", "visible");
    reload(app, "");
    typ(app, "/h");
    let other = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            MainThreadMarker::new().expect("main").alloc::<NSWindow>(),
            objc2_foundation::NSRect::new(NSPoint::new(100.0, 100.0), objc2_foundation::NSSize::new(200.0, 100.0)),
            NSWindowStyleMask::Titled,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    unsafe { other.setReleasedWhenClosed(false) };
    other.makeKeyAndOrderFront(None);
    pump(300);
    if !notes::is_key() {
        check("losing key status hides the popup", !slash_visible(), "visible");
    } else {
        println!("note: could not make another window key; key-loss hiding not verified here");
    }
    other.orderOut(None);
    notes::show();
    wait_key(1500);

    // ---- snapshots: Format Bar visible, slash popup open
    app.setAppearance(None);
    let note = "# Weekly plan\n\nShip the **Notes** window and write the docs for it.\n\n";
    for (name, appearance) in [("light", unsafe { NSAppearanceNameAqua }), ("dark", unsafe { NSAppearanceNameDarkAqua })] {
        app.setAppearance(NSAppearance::appearanceNamed(appearance).as_deref());
        reload(app, note);
        notes::set_format_bar_visible(true);
        typ(app, "\n/");
        pump(400);
        snapshot(out, &format!("bar-{name}"));
        if let Some(p) = notes::slash_popup() {
            let path = out.join(format!("notes-slash-popup-{name}.png"));
            let ok = p.snapshot_png(&path);
            check(&format!("slash popup snapshot {name} written"), ok && p.is_visible(), "failed");
            println!("      {}", path.display());
            let f = p.frame();
            println!("      popup frame {:.0},{:.0} {:.0}x{:.0}", f.x, f.y, f.w, f.h);
        }
        notes::dismiss_popups();
    }
    app.setAppearance(None);
    notes::set_format_bar_visible(false);
    check("Format Bar hides again", !notes::format_bar_visible() && editor_scroll_bottom(app) == 0.0, "still visible");
    notes::set_on_change(Box::new(|_| {}));
}

fn popup_selected() -> Option<usize> {
    notes::slash_popup().and_then(|p| p.selected())
}

fn snapshot(out: &std::path::Path, name: &str) {
    let path = out.join(format!("notes-{name}.png"));
    let ok = notes::snapshot_png(&path);
    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    check(&format!("snapshot {name} written"), ok && size > 1000, &format!("ok={ok} size={size}"));
    println!("      {}", path.display());
}
