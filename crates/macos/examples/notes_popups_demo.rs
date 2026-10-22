//! Check for the Notes pop-ups (suggestion popup and Format Bar):
//! `cargo run -p ferrocast-macos --example notes_popups_demo --locked -- --check [--out <dir>]`
//! Opens a real window, drives the pop-ups programmatically, asserts the behavior and writes
//! light/dark PNG snapshots. Exit code 0 only when every assertion holds.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use ferrocast_macos::notes_format::Format;
use ferrocast_macos::notes_popups::{
    emoji_items, slash_items, FormatBar, Item, Rect, SuggestionPopup, MAX_VISIBLE_ROWS, POPUP_PAD, ROW_HEIGHT,
};
use objc2::rc::Retained;
use objc2::{MainThreadMarker, Message};
use objc2_app_kit::{
    NSAppearance, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSApplication, NSApplicationActivationPolicy, NSAutoresizingMaskOptions,
    NSBackingStoreType, NSBitmapImageFileType, NSBitmapImageRep, NSButton, NSColor, NSControlStateValueOn, NSEvent, NSEventModifierFlags,
    NSEventType, NSFont, NSImage, NSScrollView, NSTextView, NSView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSDate, NSDictionary, NSPoint, NSProcessInfo, NSRect, NSRunLoop, NSSize, NSString};

const WIN_W: f64 = 560.0;
const WIN_H: f64 = 420.0;

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

fn rect(x: f64, y: f64, w: f64, h: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(w, h))
}

/// macOS 14+ only lets a process take focus when LaunchServices started it. A bare
/// `cargo run` from a background context is refused, so re-run this very check inside a
/// throwaway `.app` bundle started with `open`, and report its verdict.
fn relaunch_in_bundle(args: &[String]) -> ! {
    let tmp = std::env::temp_dir().join(format!("notes-popups-demo-bundle-{}", std::process::id()));
    let app = tmp.join("NotesPopupsDemo.app");
    let macos = app.join("Contents/MacOS");
    std::fs::create_dir_all(&macos).expect("create bundle");
    std::fs::copy(std::env::current_exe().expect("exe"), macos.join("notes_popups_demo")).expect("copy exe");
    std::fs::write(
        app.join("Contents/Info.plist"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict><key>CFBundleExecutable</key><string>notes_popups_demo</string><key>CFBundleIdentifier</key><string>dev.ferrocast.notespopupsdemo</string><key>CFBundleName</key><string>NotesPopupsDemo</string><key>CFBundlePackageType</key><string>APPL</string></dict></plist>"#,
    )
    .expect("write plist");
    let log = tmp.join("out.log");
    let mut cmd = std::process::Command::new("open");
    cmd.args(["-W", "-n", "--stdout"]).arg(&log).arg("--stderr").arg(&log).arg(&app).arg("--args").args(&args[1..]).arg("--bundled");
    let status = cmd.status();
    let out = std::fs::read_to_string(&log).unwrap_or_default();
    print!("{out}");
    let ok = status.map(|s| s.success()).unwrap_or(false) && out.contains("notes_popups_demo: all checks passed");
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

fn tooltip(f: Format) -> String {
    match f.shortcut() {
        Some(s) => format!("{} {s}", f.name()),
        None => f.name().to_string(),
    }
}

fn text_view_is_first_responder(window: &NSWindow) -> bool {
    window.firstResponder().is_some_and(|fr| fr.downcast_ref::<NSTextView>().is_some())
}

/// Post a real left-click through `NSApplication::sendEvent` at `point` (window coordinates).
fn click(app: &NSApplication, window_number: isize, point: NSPoint) {
    for ty in [NSEventType::LeftMouseDown, NSEventType::LeftMouseUp] {
        let ev = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
            ty,
            point,
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
    pump(50);
}

/// Window content (plus the popup when given) as one PNG.
fn snapshot(window: &NSWindow, popup: Option<&SuggestionPopup>, path: &Path) -> bool {
    let mtm = MainThreadMarker::new().expect("main thread");
    let Some(view) = window.contentView() else { return false };
    let bounds = view.bounds();
    let Some(rep) = view.bitmapImageRepForCachingDisplayInRect(bounds) else { return false };
    view.cacheDisplayInRect_toBitmapImageRep(bounds, &rep);
    let popup_png = path.with_extension("popup.png");
    let popup_image = popup
        .filter(|p| p.is_visible() && p.snapshot_png(&popup_png))
        .and_then(|_| NSImage::initWithContentsOfFile(mtm.alloc::<NSImage>(), &NSString::from_str(&popup_png.to_string_lossy())));
    let canvas = NSImage::initWithSize(mtm.alloc::<NSImage>(), bounds.size);
    #[allow(deprecated)]
    canvas.lockFocus();
    #[allow(deprecated)]
    unsafe {
        NSAppearance::setCurrentAppearance(Some(&NSApplication::sharedApplication(mtm).effectiveAppearance()))
    };
    NSColor::windowBackgroundColor().setFill();
    objc2_app_kit::NSBezierPath::fillRect(bounds);
    rep.drawInRect(bounds);
    if let (Some(p), Some(img)) = (popup, &popup_image) {
        let content = window.contentRectForFrameRect(window.frame());
        let f = p.frame();
        img.drawInRect(rect(f.x - content.origin.x, f.y - content.origin.y, f.w, f.h));
    }
    #[allow(deprecated)]
    canvas.unlockFocus();
    let _ = std::fs::remove_file(&popup_png);
    let Some(tiff) = canvas.TIFFRepresentation() else { return false };
    let Some(out) = NSBitmapImageRep::imageRepWithData(&tiff) else { return false };
    let Some(data) = (unsafe { out.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new()) }) else {
        return false;
    };
    data.writeToFile_atomically(&NSString::from_str(&path.to_string_lossy()), true)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if !args.iter().any(|a| a == "--check") {
        eprintln!("usage: notes_popups_demo --check [--out <dir>]");
        std::process::exit(2);
    }
    let out: PathBuf = args
        .iter()
        .position(|a| a == "--out")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("ferrocast-notes-popups-demo"));
    std::fs::create_dir_all(&out).expect("create out dir");

    let mtm = MainThreadMarker::new().expect("main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    app.finishLaunching();

    // --- host window with a text view; the Format Bar goes at its bottom
    let window: Retained<NSWindow> = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            mtm.alloc::<NSWindow>(),
            rect(0.0, 0.0, WIN_W, WIN_H),
            NSWindowStyleMask::Titled | NSWindowStyleMask::Closable | NSWindowStyleMask::Resizable,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    unsafe { window.setReleasedWhenClosed(false) };
    let content = window.contentView().expect("content view");
    let all = NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable;
    let scroll = NSScrollView::initWithFrame(mtm.alloc::<NSScrollView>(), rect(0.0, FormatBar::HEIGHT, WIN_W, WIN_H - FormatBar::HEIGHT));
    scroll.setHasVerticalScroller(true);
    scroll.setDrawsBackground(false);
    scroll.setAutoresizingMask(all);
    let text_view = NSTextView::initWithFrame(mtm.alloc::<NSTextView>(), rect(0.0, 0.0, WIN_W, WIN_H));
    text_view.setMinSize(NSSize::new(0.0, 0.0));
    text_view.setMaxSize(NSSize::new(f64::MAX, f64::MAX));
    text_view.setVerticallyResizable(true);
    text_view.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable);
    text_view.setFont(Some(&NSFont::systemFontOfSize(14.0)));
    text_view.setTextColor(Some(&NSColor::textColor()));
    text_view.setDrawsBackground(false);
    text_view.setString(&NSString::from_str("Weekly notes\n\nType / for formatting, or : for an emoji\n/"));
    scroll.setDocumentView(Some(&text_view));
    content.addSubview(&scroll);
    window.center();

    window.makeKeyAndOrderFront(None);
    app.activate();
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);
    let _ = window.makeFirstResponder(Some(&text_view));
    let mut keyed = false;
    for _ in 0..30 {
        if window.isKeyWindow() {
            keyed = true;
            break;
        }
        pump(50);
    }
    if !keyed && !args.iter().any(|a| a == "--bundled") {
        println!("note: this process cannot take focus (not started by LaunchServices); re-running inside an app bundle");
        relaunch_in_bundle(&args);
    }
    check("window is key", window.isKeyWindow(), "host window did not become key");
    check("text view is first responder", text_view_is_first_responder(&window), "not the NSTextView");

    // --- suggestion popup: the `/` menu
    let popup = SuggestionPopup::new();
    let picks: Rc<RefCell<Vec<usize>>> = Rc::default();
    {
        let p = picks.clone();
        popup.set_on_pick(Box::new(move |i| p.borrow_mut().push(i)));
    }
    check("popup hidden before show", !popup.is_visible() && popup.selected().is_none(), "visible/selected before show");
    let wf = window.frame();
    let anchor = Rect { x: wf.origin.x + 40.0, y: wf.origin.y + wf.size.height - 120.0, w: 2.0, h: 16.0 };
    let menu = slash_items("");
    check("slash menu has 9 entries, no Link", menu.len() == 9 && menu.iter().all(|(f, _)| *f != Format::Link), &format!("{}", menu.len()));
    let items = |v: &[(Format, Item)]| v.iter().map(|(_, i)| i.clone()).collect::<Vec<_>>();
    popup.show(anchor, items(&menu));
    pump(150);
    check("popup visible after show", popup.is_visible(), "not visible");
    check("selection starts on the first row", popup.selected() == Some(0), &format!("{:?}", popup.selected()));
    let f = popup.frame();
    check(
        "8 of 9 rows visible, row height 28",
        (f.h - (MAX_VISIBLE_ROWS as f64 * ROW_HEIGHT + 2.0 * POPUP_PAD)).abs() < 0.5,
        &format!("h={}", f.h),
    );
    check("popup sits below the caret", f.y + f.h <= anchor.y + 0.5, &format!("popup top {} caret {}", f.y + f.h, anchor.y));
    check("popup stays on screen", f.x >= 0.0 && f.y >= 0.0, &format!("{f:?}"));

    // focus must stay with the editor
    check("popup does not take key focus", !popup.is_key() && window.isKeyWindow(), "popup became key or window lost key");
    check("text view still first responder while popup is visible", text_view_is_first_responder(&window), "focus moved");
    check(
        "keyWindow is the host window",
        app.keyWindow().is_some_and(|w| w.windowNumber() == window.windowNumber() && w.windowNumber() != popup.window_number()),
        "key window changed",
    );

    // selection movement and wrap
    popup.move_selection(1);
    check("move +1", popup.selected() == Some(1), &format!("{:?}", popup.selected()));
    popup.move_selection(-2);
    check("wraps backwards to the last row", popup.selected() == Some(8), &format!("{:?}", popup.selected()));
    popup.move_selection(1);
    check("wraps forwards to the first row", popup.selected() == Some(0), &format!("{:?}", popup.selected()));
    popup.move_selection(8);
    pump(50);
    check("scrolls to a row below the fold", popup.selected() == Some(8), &format!("{:?}", popup.selected()));
    popup.move_selection(1);

    // narrowing
    let narrowed = slash_items("head");
    popup.update(items(&narrowed));
    pump(100);
    let f2 = popup.frame();
    check(
        "update narrows to 3 headings",
        narrowed.len() == 3 && (f2.h - (3.0 * ROW_HEIGHT + 2.0 * POPUP_PAD)).abs() < 0.5,
        &format!("h={}", f2.h),
    );
    check("update resets selection", popup.selected() == Some(0), &format!("{:?}", popup.selected()));
    check("popup still visible after update", popup.is_visible(), "hidden");

    // click on row 1 (row 0 is at the top of the content, below the padding)
    let f3 = popup.frame();
    let row1_y = f3.h - POPUP_PAD - 1.5 * ROW_HEIGHT;
    click(&app, popup.window_number(), NSPoint::new(40.0, row1_y));
    check("click on row 1 fires on_pick(1)", *picks.borrow() == vec![1], &format!("{:?}", picks.borrow()));
    check("click selects the row", popup.selected() == Some(1), &format!("{:?}", popup.selected()));
    check(
        "still key window and first responder after click",
        window.isKeyWindow() && text_view_is_first_responder(&window),
        "focus moved by click",
    );

    // empty list hides
    popup.update(Vec::new());
    check("empty update hides the popup", !popup.is_visible() && popup.selected().is_none(), "still visible");
    popup.show(anchor, Vec::new());
    check("empty show keeps it hidden", !popup.is_visible(), "visible");

    // --- emoji list with a fake provider
    let provider = |q: &str| -> Vec<(String, String)> {
        if q.starts_with("sm") {
            let faces = [
                "😀", "😃", "😄", "😁", "😆", "😅", "🤣", "😂", "🙂", "🙃", "😉", "😊", "😇", "🥰", "😍", "🤩", "😘", "😗", "😚", "😙",
                "😋", "😛",
            ];
            faces.iter().enumerate().map(|(i, e)| (e.to_string(), format!("smile {i}"))).collect()
        } else {
            Vec::new()
        }
    };
    check("emoji: one character gives nothing", emoji_items("s", &provider).is_empty(), "items returned");
    let emojis = emoji_items("sm", &provider);
    check("emoji: capped at 20 rows", emojis.len() == 20 && emojis[0].glyph == "😀", &format!("{}", emojis.len()));
    popup.show(anchor, emojis.clone());
    pump(150);
    check("emoji popup visible, 8 rows high", popup.is_visible() && (popup.frame().h - 232.0).abs() < 0.5, &format!("{:?}", popup.frame()));
    popup.move_selection(-1);
    check("emoji selection wraps to row 19", popup.selected() == Some(19), &format!("{:?}", popup.selected()));
    check("emoji popup keeps focus in the editor", window.isKeyWindow() && text_view_is_first_responder(&window), "focus moved");
    popup.move_selection(1);
    let emoji_png = (out.join("emoji-light.png"), out.join("emoji-dark.png"));
    popup.hide();
    check("hide() hides the emoji popup", !popup.is_visible(), "still visible");

    // --- near the bottom of the screen the popup flips above the caret
    let low = Rect { x: wf.origin.x + 40.0, y: 30.0, w: 2.0, h: 16.0 };
    popup.show(low, items(&menu));
    pump(100);
    let fl = popup.frame();
    check("popup flips above a caret at the bottom of the screen", fl.y >= low.y + low.h && fl.y >= 0.0, &format!("{fl:?}"));
    popup.hide();

    // --- Format Bar
    let bar = FormatBar::new();
    let view = bar.view();
    view.setFrame(rect(0.0, 0.0, WIN_W, FormatBar::HEIGHT));
    content.addSubview(&view);
    check(
        "bar height is 36",
        (FormatBar::HEIGHT - 36.0).abs() < f64::EPSILON && (view.frame().size.height - 36.0).abs() < 0.5,
        "bad height",
    );
    let got: Rc<RefCell<Vec<Format>>> = Rc::default();
    {
        let g = got.clone();
        bar.set_on_format(Box::new(move |f| g.borrow_mut().push(f)));
    }
    let mut missing = Vec::new();
    for f in Format::ALL {
        match find_button(&view, &tooltip(f)) {
            Some(b) => {
                let has_face = b.image().is_some() || !b.title().to_string().is_empty();
                if !has_face {
                    missing.push(format!("{} has no icon or title", f.name()));
                }
                unsafe { b.performClick(None) };
            }
            None => missing.push(format!("{} button", f.name())),
        }
    }
    check("every Format has a button with tooltip and icon/title", missing.is_empty(), &missing.join(", "));
    check("clicking every button delivers its Format in order", *got.borrow() == Format::ALL.to_vec(), &format!("{:?}", got.borrow()));
    check("bar clicks keep the editor focused", window.isKeyWindow() && text_view_is_first_responder(&window), "focus moved");
    let states = |view: &NSView| -> Vec<Format> {
        Format::ALL
            .iter()
            .copied()
            .filter(|f| find_button(view, &tooltip(*f)).is_some_and(|b| b.state() == NSControlStateValueOn))
            .collect()
    };
    check("no active buttons initially", states(&view).is_empty(), &format!("{:?}", states(&view)));
    bar.set_active(&[Format::Bold, Format::Heading(2)]);
    check(
        "set_active highlights exactly Bold and Heading 2",
        states(&view) == vec![Format::Bold, Format::Heading(2)],
        &format!("{:?}", states(&view)),
    );
    bar.set_active(&[Format::Italic]);
    check("set_active replaces the previous set", states(&view) == vec![Format::Italic], &format!("{:?}", states(&view)));
    bar.set_active(&[]);
    check("set_active(&[]) clears", states(&view).is_empty(), "still active");

    // --- snapshots
    bar.set_active(&[Format::Bold, Format::Heading(2), Format::BulletList]);
    for (name, appearance) in [("light", unsafe { NSAppearanceNameAqua }), ("dark", unsafe { NSAppearanceNameDarkAqua })] {
        app.setAppearance(NSAppearance::appearanceNamed(appearance).as_deref());
        pump(300);
        // slash popup open (second row selected) over the window with the bar
        popup.show(anchor, items(&menu));
        popup.move_selection(1);
        pump(300);
        let p = out.join(format!("popup-{name}.png"));
        let ok = snapshot(&window, Some(&popup), &p);
        let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
        check(&format!("snapshot popup-{name} written"), ok && size > 1000, &format!("ok={ok} size={size}"));
        println!("      {}", p.display());
        // emoji popup
        popup.show(anchor, emojis.clone());
        popup.move_selection(2);
        pump(300);
        let p = if name == "light" { emoji_png.0.clone() } else { emoji_png.1.clone() };
        let ok = snapshot(&window, Some(&popup), &p);
        let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
        check(&format!("snapshot emoji-{name} written"), ok && size > 1000, &format!("ok={ok} size={size}"));
        println!("      {}", p.display());
        popup.hide();
    }
    window.orderOut(None);

    let failures = FAILURES.with(|f| f.borrow().clone());
    if failures.is_empty() {
        println!("notes_popups_demo: all checks passed");
    } else {
        println!("notes_popups_demo: {} check(s) FAILED: {}", failures.len(), failures.join("; "));
        std::process::exit(1);
    }
}
