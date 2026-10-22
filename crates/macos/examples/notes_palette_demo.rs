//! Check for the Notes palette (searchable list overlay):
//! `cargo run -p ferrocast-macos --example notes_palette_demo --locked -- --check [--out <dir>]`
//! Opens the real notes window, drives the palette with real key and mouse events, asserts the
//! behavior and writes light/dark PNG snapshots. Exit code 0 only when every assertion holds.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use ferrocast_macos::notes::{self, Event};
use ferrocast_macos::notes_palette::{self as palette, PaletteItem};
use ferrocast_macos::notes_theme::{self as theme, Appearance, Rgba};
use objc2::rc::Retained;
use objc2::{MainThreadMarker, Message};
use objc2_app_kit::{
    NSAppearance, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSApplication, NSApplicationActivationPolicy, NSBitmapImageRep, NSColor,
    NSEvent, NSEventModifierFlags, NSEventType, NSTextView,
};
use objc2_foundation::{NSData, NSDate, NSPoint, NSProcessInfo, NSRange, NSRunLoop, NSString};

const NOTE: &str = "# Weekly notes\n\nBuy oat milk and bread.\nCall the dentist on Tuesday.\n\n- ship the palette\n- review the PR";

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

fn plain(app: &NSApplication, chars: &str, code: u16) {
    key(app, chars, code, NSEventModifierFlags::empty());
}

fn cmd(app: &NSApplication, chars: &str, code: u16) {
    key(app, chars, code, NSEventModifierFlags::Command);
}

/// Real left-click at `point` (window coordinates) of the key window.
fn click(app: &NSApplication, point: (f64, f64)) {
    let window_number = app.keyWindow().map(|w| w.windowNumber()).unwrap_or(0);
    for ty in [NSEventType::LeftMouseDown, NSEventType::LeftMouseUp] {
        let ev = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
            ty,
            NSPoint::new(point.0, point.1),
            NSEventModifierFlags::empty(),
            NSProcessInfo::processInfo().systemUptime(),
            window_number,
            None,
            0,
            1,
            1.0,
        )
        .expect("mouse event");
        app.sendEvent(&ev);
    }
    pump(80);
}

fn wait_key(ms: u64) -> bool {
    for _ in 0..ms / 50 {
        if notes::is_key() {
            return true;
        }
        pump(50);
    }
    notes::is_key()
}

/// macOS 14+ only lets a process take focus when LaunchServices started it. A bare
/// `cargo run` from a background context is refused, so re-run this very check inside a
/// throwaway `.app` bundle started with `open`, and report its verdict.
fn relaunch_in_bundle(args: &[String]) -> ! {
    let tmp = std::env::temp_dir().join(format!("notes-palette-demo-bundle-{}", std::process::id()));
    let app = tmp.join("NotesPaletteDemo.app");
    let macos = app.join("Contents/MacOS");
    std::fs::create_dir_all(&macos).expect("create bundle");
    std::fs::copy(std::env::current_exe().expect("exe"), macos.join("notes_palette_demo")).expect("copy exe");
    std::fs::write(
        app.join("Contents/Info.plist"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict><key>CFBundleExecutable</key><string>notes_palette_demo</string><key>CFBundleIdentifier</key><string>dev.ferrocast.notespalettedemo</string><key>CFBundleName</key><string>NotesPaletteDemo</string><key>CFBundlePackageType</key><string>APPL</string></dict></plist>"#,
    )
    .expect("write plist");
    let log = tmp.join("out.log");
    let mut cmd = std::process::Command::new("open");
    cmd.args(["-W", "-n", "--stdout"]).arg(&log).arg("--stderr").arg(&log).arg(&app).arg("--args").args(&args[1..]).arg("--bundled");
    let status = cmd.status();
    let out = std::fs::read_to_string(&log).unwrap_or_default();
    print!("{out}");
    let ok = status.map(|s| s.success()).unwrap_or(false) && out.contains("notes_palette_demo: all checks passed");
    let _ = std::fs::remove_file(&log);
    std::process::exit(if ok { 0 } else { 1 });
}

/// Read pixel `(x, y)` (top-left origin) raw, in the snapshot's own colour space.
fn pixel(rep: &NSBitmapImageRep, x: isize, y: isize) -> (u8, u8, u8) {
    let c = rep.colorAtX_y(x, y).expect("pixel colour");
    let f = |v: f64| (v * 255.0).round() as u8;
    (f(c.redComponent()), f(c.greenComponent()), f(c.blueComponent()))
}

/// The sRGB colour `c` as stored in `rep`. A snapshot holds what AppKit drew in this Mac's
/// display profile: grays are unchanged by it but a saturated coral moves by about 10 per
/// channel, so the expected value is converted into the rep's space before comparing.
fn in_rep(rep: &NSBitmapImageRep, c: (u8, u8, u8)) -> (u8, u8, u8) {
    let f = |v: u8| v as f64 / 255.0;
    let want = NSColor::colorWithSRGBRed_green_blue_alpha(f(c.0), f(c.1), f(c.2), 1.0);
    let g = |v: f64| (v * 255.0).round() as u8;
    match want.colorUsingColorSpace(&rep.colorSpace()) {
        Some(w) => (g(w.redComponent()), g(w.greenComponent()), g(w.blueComponent())),
        None => c,
    }
}

fn close(a: (u8, u8, u8), b: (u8, u8, u8), tol: i32) -> bool {
    [(a.0, b.0), (a.1, b.1), (a.2, b.2)].iter().all(|&(x, y)| (x as i32 - y as i32).abs() <= tol)
}

/// T3 (S2, S4, S5, S8): with the palette open, the card, the Sep line, the dimmed backdrop and the
/// selected row use the chosen appearance whatever the macOS appearance is (the app is forced to
/// the opposite one).
fn theme_checks(app: &NSApplication, out: &std::path::Path) {
    for (name, setting, dark, opposite) in [
        ("dark", Appearance::Dark, true, unsafe { NSAppearanceNameAqua }),
        ("light", Appearance::Light, false, unsafe { NSAppearanceNameDarkAqua }),
    ] {
        theme::set_appearance(setting);
        app.setAppearance(NSAppearance::appearanceNamed(opposite).as_deref());
        palette::set_items(sample());
        plain(app, "\u{f701}", 125);
        plain(app, "\u{f701}", 125);
        pump(300);
        let t = if dark { theme::DARK } else { theme::LIGHT };
        let bg = theme::window_bg(dark);
        let path = out.join(format!("palette-theme-{name}.png"));
        check(&format!("theme {name}: palette snapshot written"), palette::snapshot_png(&path), "failed");
        let rep = std::fs::read(&path).ok().and_then(|d| NSBitmapImageRep::imageRepWithData(&NSData::with_bytes(&d)));
        let Some(rep) = rep else {
            check(&format!("theme {name}: palette snapshot readable"), false, "no image");
            continue;
        };
        let content =
            app.keyWindow().and_then(|w| w.contentView()).map(|v| v.bounds().size).unwrap_or(objc2_foundation::NSSize::new(560.0, 420.0));
        let scale = rep.pixelsWide() as f64 / content.width;
        let at = |x: f64, y: f64| pixel(&rep, (x * scale) as isize, (y * scale) as isize);
        let (cw, _) = palette::card_size().unwrap_or((420.0, 0.0));
        let card_x = (content.width - cw) / 2.0;
        // search row, right of the field text: card background (top-origin: 52 pt margin + 22)
        let got_bg = at(card_x + cw - 6.0, 52.0 + 22.0);
        check(
            &format!("theme {name}: palette card is the opaque Bg token"),
            close(got_bg, in_rep(&rep, bg), 3),
            &format!("got {got_bg:?} want {:?}", in_rep(&rep, bg)),
        );
        // the search field has no fill or border of its own: the same pixel row left of the caret area
        let got_field = at(card_x + 200.0, 52.0 + 8.0);
        check(
            &format!("theme {name}: search field has no fill (card Bg shows)"),
            close(got_field, in_rep(&rep, bg), 3),
            &format!("got {got_field:?}"),
        );
        // dimmed backdrop outside the card: window bg under black at 35% (dark) / 18% (light)
        let dim = if dark { 89 } else { 46 };
        let want_dim = theme::composite_over(Rgba(0, 0, 0, dim), Rgba(bg.0, bg.1, bg.2, 255));
        let got_dim = at(8.0, content.height - 120.0);
        check(
            &format!("theme {name}: backdrop is Bg dimmed by black ({}%)", if dark { 35 } else { 18 }),
            close(got_dim, in_rep(&rep, want_dim), 6),
            &format!("got {got_dim:?} want {:?}", in_rep(&rep, want_dim)),
        );
        // selected row: left of the glyph, inside the coral fill
        match (palette::selected_item_id().and_then(|id| palette::row_window_point(&id)), palette::selected_item_id()) {
            (Some((px, py)), Some(id)) => {
                let got_sel = at(px - 58.0, content.height - py);
                check(
                    &format!("theme {name}: selected row ({id}) is the Accent token"),
                    close(got_sel, in_rep(&rep, (t.accent.0, t.accent.1, t.accent.2)), 6),
                    &format!("got {got_sel:?} want {:?}", in_rep(&rep, (t.accent.0, t.accent.1, t.accent.2))),
                );
            }
            _ => check(&format!("theme {name}: selected row has a position"), false, "none"),
        }
        // the Sep line is the last point row of the 44 pt search area (top-origin: 52 pt margin + 43.5)
        let want_sep = theme::composite_over(t.sep, Rgba(bg.0, bg.1, bg.2, 255));
        let got_sep = at(card_x + cw - 6.0, 52.0 + 43.5);
        check(
            &format!("theme {name}: Sep line under the search field"),
            !close(got_sep, got_bg, 1) && close(got_sep, in_rep(&rep, want_sep), 6),
            &format!("got {got_sep:?} want {:?}", in_rep(&rep, want_sep)),
        );
        println!("      {}", path.display());
    }
    theme::set_appearance(Appearance::System);
    app.setAppearance(None);
    check("theme: restored to System", theme::appearance() == Appearance::System, "");
    pump(100);
}

fn editor(app: &NSApplication) -> Option<Retained<NSTextView>> {
    let fr = app.keyWindow()?.firstResponder()?;
    // The search field's field editor is an NSTextView too: only the note editor counts.
    fr.downcast_ref::<NSTextView>().filter(|t| !t.isFieldEditor()).map(|t| t.retain())
}

fn item(id: &str, title: &str, section: &str) -> PaletteItem {
    PaletteItem { id: id.into(), title: title.into(), section: Some(section.into()), ..PaletteItem::default() }
}

/// 12 rows in 2 sections: "Pinned" (3) and "Notes" (9).
fn sample() -> Vec<PaletteItem> {
    let mut v = vec![
        PaletteItem {
            subtitle: Some("Updated today".into()),
            accessory: Some("⌘1".into()),
            glyph: Some("sf:pin.fill".into()),
            ..item("p1", "Weekly notes", "Pinned")
        },
        PaletteItem { accessory: Some("⌘2".into()), glyph: Some("sf:pin.fill".into()), ..item("p2", "Groceries", "Pinned") },
        PaletteItem { glyph: Some("💡".into()), ..item("p3", "Ideas", "Pinned") },
        item("n1", "Meeting recap", "Notes"),
        item("n2", "Travel plan", "Notes"),
        item("n3", "Reading list", "Notes"),
        item("n4", "Recipes", "Notes"),
        item("n5", "Budget 2026", "Notes"),
        PaletteItem { has_children: true, glyph: Some("sf:folder".into()), ..item("n6", "Project Atlas", "Notes") },
        item("n7", "Old drafts", "Notes"),
        item("n8", "Archive", "Notes"),
        PaletteItem { danger: true, glyph: Some("sf:trash".into()), accessory: Some("⌃X".into()), ..item("n9", "Delete note", "Notes") },
    ];
    v[3].subtitle = Some("Yesterday".into());
    v
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if !args.iter().any(|a| a == "--check") {
        eprintln!("usage: notes_palette_demo --check [--out <dir>]");
        std::process::exit(2);
    }
    let out: PathBuf = args
        .iter()
        .position(|a| a == "--out")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("ferrocast-notes-palette-demo"));
    std::fs::create_dir_all(&out).expect("create out dir");

    let mtm = MainThreadMarker::new().expect("main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    app.finishLaunching();

    let events: Rc<RefCell<Vec<Event>>> = Rc::default();
    let picks: Rc<RefCell<Vec<String>>> = Rc::default();
    let closes: Rc<RefCell<usize>> = Rc::default();
    let queries: Rc<RefCell<Vec<String>>> = Rc::default();
    {
        let e = events.clone();
        notes::set_on_event(Box::new(move |ev| e.borrow_mut().push(ev)));
        let p = picks.clone();
        palette::set_on_pick(Box::new(move |id| p.borrow_mut().push(id)));
        let c = closes.clone();
        palette::set_on_close(Box::new(move || *c.borrow_mut() += 1));
        // The demo plays the controller: refilter on every edit.
        let q = queries.clone();
        palette::set_on_query(Box::new(move |text| {
            q.borrow_mut().push(text.to_string());
            let needle = text.to_lowercase();
            let rows: Vec<PaletteItem> = sample().into_iter().filter(|i| i.title.to_lowercase().contains(&needle)).collect();
            palette::set_items(rows);
        }));
    }

    // The API is safe before the window exists.
    check("palette hidden before the window exists", !palette::is_visible() && palette::depth() == 0, "visible");
    check(
        "hide() before show is a no-op",
        {
            palette::hide();
            *closes.borrow() == 0
        },
        "on_close fired",
    );
    check("pop_level() at the root is false", !palette::pop_level(), "popped");

    notes::show();
    if let Some(w) = app.keyWindow().or_else(|| app.windows().iter().next().map(|w| w.retain())) {
        w.setContentSize(objc2_foundation::NSSize::new(560.0, 420.0));
    }
    if !wait_key(1500) && !args.iter().any(|a| a == "--bundled") {
        println!("note: this process cannot take focus (not started by LaunchServices); re-running inside an app bundle");
        notes::hide();
        relaunch_in_bundle(&args);
    }
    check("notes window is key", notes::is_key(), "not key");
    notes::set_text(NOTE);
    notes::focus_editor();
    let note_text = notes::text();
    check("the note has text", note_text.contains("Call the dentist") && notes::title() == "Weekly notes", &note_text);
    let tv = editor(&app);
    check("editor is first responder before the palette", tv.is_some(), "no editor");
    // Select the whole note: if a stray ⌘B reached the editor it would wrap the text in `**`.
    if let Some(tv) = &tv {
        tv.setSelectedRange(NSRange::new(0, note_text.encode_utf16().count()));
    }

    pump(300);
    check("snapshot of the note alone written", notes::snapshot_png(&out.join("note-only.png")), "failed");

    // --- open
    let items = sample();
    check("sample has 12 items", items.len() == 12, "len");
    palette::show("Search notes…", items);
    pump(200);
    check("is_visible after show", palette::is_visible() && palette::depth() == 1, &format!("depth {}", palette::depth()));
    check("search field is first responder", palette::search_focused(), "field not focused");
    check("editor is not first responder while open", editor(&app).is_none(), "editor still focused");
    check("window is still key", notes::is_key(), "lost key");
    check(
        "selection starts on the first row, not a header",
        palette::selected_item_id().as_deref() == Some("p1"),
        &format!("{:?}", palette::selected_item_id()),
    );
    let (cw, ch) = palette::card_size().unwrap_or((0.0, 0.0));
    check("card is 420 wide and at most 340 high", (cw - 420.0).abs() < 0.5 && ch <= 340.5 && ch > 100.0, &format!("{cw}x{ch}"));

    // --- typing filters through on_query (the demo calls set_items)
    plain(&app, "r", 15);
    plain(&app, "e", 14);
    check("query() reflects the typed text", palette::query() == "re", &palette::query());
    check("on_query fired for each edit", queries.borrow().as_slice() == ["r", "re"], &format!("{:?}", queries.borrow()));
    check(
        "selection moved to the first remaining row",
        palette::selected_item_id().as_deref() == Some("n1"),
        &format!("{:?}", palette::selected_item_id()),
    );
    plain(&app, "\u{7f}", 51);
    plain(&app, "\u{7f}", 51);
    check("backspace clears the query", palette::query().is_empty(), &palette::query());
    check(
        "full list is back and the same id is kept when it exists",
        palette::selected_item_id().as_deref() == Some("n1"),
        &format!("{:?}", palette::selected_item_id()),
    );

    // --- selection: wrap and header skipping
    palette::set_items(sample());
    check(
        "set_items keeps the selected id",
        palette::selected_item_id().as_deref() == Some("n1"),
        &format!("{:?}", palette::selected_item_id()),
    );
    palette::set_items(sample().into_iter().filter(|i| i.id != "n1").collect());
    check(
        "set_items selects the first row when the id is gone",
        palette::selected_item_id().as_deref() == Some("p1"),
        &format!("{:?}", palette::selected_item_id()),
    );
    palette::set_items(sample());
    plain(&app, "\u{f701}", 125);
    check(
        "Down moves to the next row",
        palette::selected_item_id().as_deref() == Some("p2"),
        &format!("{:?}", palette::selected_item_id()),
    );
    plain(&app, "\u{f701}", 125);
    plain(&app, "\u{f701}", 125);
    check(
        "Down skips the section header",
        palette::selected_item_id().as_deref() == Some("n1"),
        &format!("{:?}", palette::selected_item_id()),
    );
    plain(&app, "\u{f700}", 126);
    check(
        "Up skips the section header",
        palette::selected_item_id().as_deref() == Some("p3"),
        &format!("{:?}", palette::selected_item_id()),
    );
    plain(&app, "\u{f700}", 126);
    plain(&app, "\u{f700}", 126);
    plain(&app, "\u{f700}", 126);
    check("Up wraps to the last row", palette::selected_item_id().as_deref() == Some("n9"), &format!("{:?}", palette::selected_item_id()));
    pump(100);
    check(
        "last row scrolled into view (its point is inside the window)",
        {
            let p = palette::row_window_point("n9");
            let h = app.keyWindow().map(|w| w.contentLayoutRect().size.height).unwrap_or(0.0);
            p.is_some_and(|(_, y)| y > 0.0 && y < h)
        },
        &format!("{:?}", palette::row_window_point("n9")),
    );
    plain(&app, "\u{f701}", 125);
    check(
        "Down wraps to the first row",
        palette::selected_item_id().as_deref() == Some("p1"),
        &format!("{:?}", palette::selected_item_id()),
    );

    // --- Enter picks and does not close
    plain(&app, "\u{f701}", 125);
    plain(&app, "\r", 36);
    check("Enter fires on_pick with the id", picks.borrow().as_slice() == ["p2"], &format!("{:?}", picks.borrow()));
    check("Enter does not close the palette", palette::is_visible() && *closes.borrow() == 0, "closed");

    // --- click on a row picks it
    picks.borrow_mut().clear();
    match palette::row_window_point("n3") {
        Some(pt) => click(&app, pt),
        None => check("row n3 has a position", false, "none"),
    }
    check("click on a row fires on_pick", picks.borrow().as_slice() == ["n3"], &format!("{:?}", picks.borrow()));
    check(
        "click selects the row and keeps the palette open",
        palette::selected_item_id().as_deref() == Some("n3") && palette::is_visible(),
        "state",
    );
    check("search still focused after a click", palette::search_focused(), "focus moved");

    // --- levels
    palette::push_level("Project Atlas…", vec![item("a1", "Rename", "Atlas"), item("a2", "Open", "Atlas")]);
    pump(100);
    check(
        "push_level: depth 2, first row selected",
        palette::depth() == 2 && palette::selected_item_id().as_deref() == Some("a1"),
        &format!("{}", palette::depth()),
    );
    check("push_level clears the search text", palette::query().is_empty(), &palette::query());
    plain(&app, "\u{1b}", 53);
    check(
        "Esc pops one level and does not close",
        palette::depth() == 1 && palette::is_visible() && *closes.borrow() == 0,
        &format!("depth {}", palette::depth()),
    );
    check(
        "pop restores the previous rows and selection",
        palette::selected_item_id().as_deref() == Some("n3"),
        &format!("{:?}", palette::selected_item_id()),
    );
    palette::push_level("Sub…", vec![item("b1", "One", "Sub")]);
    cmd(&app, "[", 33);
    check("⌘[ pops a level", palette::depth() == 1 && palette::is_visible(), &format!("depth {}", palette::depth()));
    palette::push_level("Sub…", vec![item("b1", "One", "Sub")]);
    check("pop_level() returns true then false", palette::pop_level() && !palette::pop_level(), "pop results");
    cmd(&app, "[", 33);
    check("⌘[ at the root does nothing", palette::depth() == 1 && palette::is_visible(), "closed or popped");

    // --- window shortcuts must not reach the note or the window
    events.borrow_mut().clear();
    cmd(&app, "n", 45);
    cmd(&app, "p", 35);
    cmd(&app, "b", 11);
    cmd(&app, "w", 13);
    check("⌘N ⌘P ⌘B ⌘W do not fire window events", events.borrow().is_empty(), &format!("{:?}", events.borrow()));
    check("⌘W does not hide the window", notes::is_visible(), "window hidden");
    check("the note text is unchanged by ⌘B / ⌘N", notes::text() == note_text, &notes::text());
    check("palette still open after those keys", palette::is_visible(), "closed");

    // --- edit shortcuts still work in the search field
    plain(&app, "a", 0);
    plain(&app, "b", 11);
    cmd(&app, "a", 0);
    check("⌘A selects inside the search field only", notes::text() == note_text && palette::query() == "ab", &palette::query());
    plain(&app, "\u{7f}", 51);
    check("typing after ⌘A replaces the field text", palette::query().is_empty(), &palette::query());

    // --- empty state
    palette::set_empty_text("No matching notes");
    palette::set_items(Vec::new());
    pump(100);
    check("empty text shows for an empty list", palette::empty_text_visible(), "not shown");
    check("no selection in the empty state", palette::selected_item_id().is_none(), "selected");
    picks.borrow_mut().clear();
    plain(&app, "\r", 36);
    plain(&app, "\u{f701}", 125);
    check("Enter and Down do nothing in the empty state", picks.borrow().is_empty() && palette::selected_item_id().is_none(), "acted");
    palette::set_items(sample());
    check(
        "empty state goes away with rows",
        !palette::empty_text_visible() && palette::selected_item_id().as_deref() == Some("p1"),
        "still empty",
    );

    // --- snapshots (palette open over a note with text)
    for (name, appearance) in [("light", unsafe { NSAppearanceNameAqua }), ("dark", unsafe { NSAppearanceNameDarkAqua })] {
        app.setAppearance(NSAppearance::appearanceNamed(appearance).as_deref());
        pump(300);
        palette::set_items(sample());
        plain(&app, "\u{f701}", 125);
        plain(&app, "\u{f701}", 125);
        pump(200);
        let p = out.join(format!("palette-{name}.png"));
        let ok = palette::snapshot_png(&p);
        let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
        check(&format!("snapshot palette-{name} written"), ok && size > 1000, &format!("ok={ok} size={size}"));
        println!("      {}", p.display());
        // submenu level with a section-less danger row, and the empty state
        palette::set_items(sample().into_iter().filter(|i| i.id.starts_with('n')).skip(5).collect());
        pump(200);
        let p = out.join(format!("palette-{name}-short.png"));
        let ok = palette::snapshot_png(&p);
        check(&format!("snapshot palette-{name}-short written"), ok, "failed");
        println!("      {}", p.display());
    }
    app.setAppearance(None);
    pump(100);
    theme_checks(&app, &out);

    // --- Esc at the root closes the palette, not the window
    palette::set_items(sample());
    plain(&app, "\u{1b}", 53);
    pump(100);
    check("Esc at the root closes the palette", !palette::is_visible() && palette::depth() == 0, "still visible");
    check("on_close fired once", *closes.borrow() == 1, &format!("{}", closes.borrow()));
    check("Esc did not hide the notes window", notes::is_visible(), "window hidden");
    check("editor has the focus back", editor(&app).is_some() && !palette::search_focused(), "no editor focus");
    check("note text unchanged after the palette closed", notes::text() == note_text, &notes::text());

    // --- reopen and close by backdrop click
    palette::show("Search notes…", sample());
    pump(200);
    check(
        "reopens at depth 1 with the first row selected",
        palette::depth() == 1 && palette::selected_item_id().as_deref() == Some("p1"),
        "state",
    );
    check("reopening clears the old search text", palette::query().is_empty(), &palette::query());
    click(&app, (40.0, 40.0));
    check("click on the backdrop closes", !palette::is_visible() && *closes.borrow() == 2, &format!("closes {}", closes.borrow()));
    check("backdrop click did not hide the window", notes::is_visible(), "hidden");
    check("editor focus is back after the backdrop click", editor(&app).is_some(), "no editor");

    // --- hide() closes and fires on_close; a narrow window shrinks the card
    palette::show("Search notes…", sample());
    pump(100);
    palette::hide();
    check("hide() closes and fires on_close", !palette::is_visible() && *closes.borrow() == 3, &format!("closes {}", closes.borrow()));
    palette::show("Search notes…", sample());
    if let Some(w) = app.keyWindow() {
        w.setContentSize(objc2_foundation::NSSize::new(360.0, 420.0));
    }
    pump(200);
    let (cw, _) = palette::card_size().unwrap_or((0.0, 0.0));
    check("card is never wider than the window minus 32", cw <= 360.0 - 32.0 + 0.5 && cw > 200.0, &format!("{cw}"));
    palette::hide();
    // The window remembers its frame: put the default size back.
    if let Some(w) = app.keyWindow() {
        w.setContentSize(objc2_foundation::NSSize::new(560.0, 420.0));
    }

    notes::hide();
    let failures = FAILURES.with(|f| f.borrow().clone());
    if failures.is_empty() {
        println!("notes_palette_demo: all checks passed");
    } else {
        println!("notes_palette_demo: {} check(s) FAILED: {}", failures.len(), failures.join("; "));
        std::process::exit(1);
    }
}
