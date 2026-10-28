//! Check for images in Notes (attachments drawn inline, round trip, editing around them, paste):
//! `cargo run -p ferrocast-macos --example notes_images_demo [-- --out <dir>]`
//! Opens the real Notes window, drives it programmatically and asserts the behavior. Exit code 0
//! only when every assertion holds. Writes a snapshot of the note with an image.
//!
//! Careful: it overwrites the general clipboard (the paste checks clear it); only the plain text it
//! held before is put back at the end, any other content (images, files, rich text) is lost.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use ferrocast_core::attachments;
use ferrocast_macos::notes::{self, ImageInput};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{msg_send, MainThreadMarker, Message};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSAttachmentAttributeName, NSPasteboard, NSPasteboardTypePNG, NSPasteboardTypeString,
    NSTextAttachment, NSTextView, NSView,
};
use objc2_foundation::{NSData, NSDate, NSRange, NSRunLoop, NSString};

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

fn find_text_view(v: &NSView) -> Option<Retained<NSTextView>> {
    if let Some(t) = v.downcast_ref::<NSTextView>() {
        return Some(t.retain());
    }
    v.subviews().iter().find_map(|s| find_text_view(&s))
}

fn editor(app: &NSApplication) -> Retained<NSTextView> {
    app.windows().iter().filter_map(|w| w.contentView()).find_map(|c| find_text_view(&c)).expect("the editor view")
}

/// A solid-colour PNG without compression (stored deflate blocks), `w` x `h` pixels.
fn png(w: u32, h: u32) -> Vec<u8> {
    fn crc(data: &[u8]) -> u32 {
        let mut c = 0xffff_ffffu32;
        for b in data {
            c ^= u32::from(*b);
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
            }
        }
        !c
    }
    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend((data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend(data);
        out.extend(&body);
        out.extend(crc(&body).to_be_bytes());
    }
    let mut raw = Vec::new();
    for _ in 0..h {
        raw.extend(std::iter::once(0u8).chain((0..w).flat_map(|x| [(x * 255 / w) as u8, 120, 200])));
    }
    let mut z = vec![0x78, 0x01];
    let mut chunks = raw.chunks(65535).peekable();
    while let Some(c) = chunks.next() {
        z.push(u8::from(chunks.peek().is_none()));
        z.extend((c.len() as u16).to_le_bytes());
        z.extend((!(c.len() as u16)).to_le_bytes());
        z.extend(c);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for x in &raw {
        a = (a + u32::from(*x)) % 65521;
        b = (b + a) % 65521;
    }
    z.extend(((b << 16) | a).to_be_bytes());
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    let mut ihdr = Vec::new();
    ihdr.extend(w.to_be_bytes());
    ihdr.extend(h.to_be_bytes());
    ihdr.extend([8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

/// Attachments of the storage: (character offset, drawn width, drawn height).
fn attachments_of(tv: &NSTextView) -> Vec<(usize, f64, f64)> {
    let st = unsafe { tv.textStorage() }.expect("storage");
    let mut out = Vec::new();
    for i in 0..st.length() {
        let v: Option<Retained<AnyObject>> =
            unsafe { st.attribute_atIndex_effectiveRange(NSAttachmentAttributeName, i, std::ptr::null_mut()) };
        if let Some(a) = v.as_ref().and_then(|a| a.downcast_ref::<NSTextAttachment>()) {
            let b = a.bounds();
            out.push((i, b.size.width, b.size.height));
        }
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out: PathBuf = args
        .iter()
        .position(|a| a == "--out")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("ferrocast-notes-images-demo"));
    std::fs::create_dir_all(&out).expect("out dir");
    let root = out.join("data");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("data dir");

    let mtm = MainThreadMarker::new().expect("main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    app.finishLaunching();

    notes::set_attachments_root(root.clone());
    let got: Rc<RefCell<Vec<ImageInput>>> = Rc::default();
    let changes: Rc<RefCell<Vec<String>>> = Rc::default();
    {
        let g = got.clone();
        notes::set_on_images(Box::new(move |i| g.borrow_mut().push(i)));
        let c = changes.clone();
        notes::set_on_change(Box::new(move |t| c.borrow_mut().push(t)));
    }
    notes::show();
    pump(400);
    let tv = editor(&app);

    // a wide image is drawn at the max width keeping its aspect ratio; a small one at its size
    let wide = attachments::store(&root, &png(960, 480), Some("png")).expect("store wide");
    let small = attachments::store(&root, &png(100, 50), Some("png")).expect("store small");
    let md = format!("hello\n\n![wide](attachments/{})\n\nmiddle ![small](attachments/{}) end", wide.name, small.name);
    notes::set_markdown(&md);
    pump(100);
    let at = attachments_of(&tv);
    check("two images become two attachments", at.len() == 2, &format!("{at:?}"));
    check(
        "wide image is capped at 480 pt, aspect ratio kept",
        at.first().is_some_and(|a| (a.1 - 480.0).abs() < 0.01 && (a.2 - 240.0).abs() < 0.01),
        &format!("{at:?}"),
    );
    check(
        "small image keeps its size",
        at.get(1).is_some_and(|a| (a.1 - 100.0).abs() < 0.01 && (a.2 - 50.0).abs() < 0.01),
        &format!("{at:?}"),
    );
    check("the note round-trips to the same Markdown", notes::markdown() == md, &notes::markdown());
    check("alt text and relative path survive (title is the first text)", notes::title() == "hello", &notes::title());

    // a link outside the attachments folder or to a missing file never reads another file
    let ghost = "0123456789abcdef0123456789abcdef.png";
    let weird = format!("![a](attachments/{ghost})\n\n![b](../../etc/hosts)");
    notes::set_markdown(&weird);
    check("missing and unsafe links keep their Markdown", notes::markdown() == weird, &notes::markdown());
    check("missing and unsafe links show a placeholder, not a file", attachments_of(&tv).len() == 2, &format!("{:?}", attachments_of(&tv)));

    // typing next to an image does not make the text an image
    notes::set_markdown(&format!("![wide](attachments/{})", wide.name));
    let len = tv.string().length();
    tv.setSelectedRange(NSRange::new(len, 0));
    let s = NSString::from_str(" typed");
    unsafe {
        let _: () = msg_send![&*tv, insertText: &*s as &AnyObject];
    }
    pump(50);
    let expect = format!("![wide](attachments/{}) typed", wide.name);
    check("text typed after an image stays text", notes::markdown() == expect, &notes::markdown());
    check("still one attachment after typing", attachments_of(&tv).len() == 1, &format!("{:?}", attachments_of(&tv)));

    // deleting the image character removes the link
    tv.setSelectedRange(NSRange::new(0, 1));
    unsafe {
        let _: () = msg_send![&*tv, delete: std::ptr::null::<AnyObject>()];
    }
    pump(50);
    check("deleting the image removes its link from the note", notes::markdown() == "typed", &notes::markdown());

    // inserting a link line after the cursor line
    notes::set_markdown("one\n\ntwo");
    tv.setSelectedRange(NSRange::new(1, 0));
    check(
        "an image link is inserted after the current line",
        notes::insert_markdown_after_cursor_line(&attachments::markdown_link("new", &small.name)),
        "refused",
    );
    pump(50);
    check(
        "inserted image sits on its own line",
        notes::markdown() == format!("one\n\n![new](attachments/{})\n\ntwo", small.name),
        &notes::markdown(),
    );

    // paste: image data without text goes to the host, text with it stays text, files are images
    let pb = NSPasteboard::generalPasteboard();
    let saved = pb.stringForType(unsafe { NSPasteboardTypeString });
    let saved_text = saved.clone();
    pb.clearContents();
    let bytes = png(40, 20);
    pb.setData_forType(Some(&NSData::with_bytes(&bytes)), unsafe { NSPasteboardTypePNG });
    notes::set_markdown("abc");
    unsafe {
        let _: () = msg_send![&*tv, paste: std::ptr::null::<AnyObject>()];
    }
    check(
        "pasting a screenshot hands PNG data to the host",
        *got.borrow() == vec![ImageInput::Png(bytes.clone())],
        &format!("{} calls", got.borrow().len()),
    );
    check("the note text is untouched by that paste", notes::markdown() == "abc", &notes::markdown());
    got.borrow_mut().clear();
    pb.clearContents();
    pb.setString_forType(&NSString::from_str("plain"), unsafe { objc2_app_kit::NSPasteboardTypeString });
    pb.setData_forType(Some(&NSData::with_bytes(&bytes)), unsafe { NSPasteboardTypePNG });
    tv.setSelectedRange(NSRange::new(3, 0));
    unsafe {
        let _: () = msg_send![&*tv, paste: std::ptr::null::<AnyObject>()];
    }
    check("text next to a picture is pasted as text", got.borrow().is_empty() && notes::markdown() == "abcplain", &notes::markdown());
    match saved {
        Some(s) => {
            pb.clearContents();
            pb.setString_forType(&s, unsafe { NSPasteboardTypeString });
        }
        None => {
            pb.clearContents();
        }
    }

    // copy, cut and paste keep images: the selection travels as Markdown
    let link = format!("![wide](attachments/{})", wide.name);
    let both = format!("before\n\n{link}\n\nafter");
    notes::set_markdown(&both);
    let all = tv.string().length();
    tv.setSelectedRange(NSRange::new(0, all));
    pb.clearContents();
    unsafe {
        let _: () = msg_send![&*tv, copy: std::ptr::null::<AnyObject>()];
    }
    let copied = pb.stringForType(unsafe { NSPasteboardTypeString }).map(|s| s.to_string()).unwrap_or_default();
    check("copying a selection with an image puts its Markdown on the pasteboard", copied == both, &copied);
    notes::set_markdown("zz");
    tv.setSelectedRange(NSRange::new(2, 0));
    unsafe {
        let _: () = msg_send![&*tv, paste: std::ptr::null::<AnyObject>()];
    }
    pump(50);
    check("pasting it back brings the image with it", notes::markdown() == format!("zzbefore\n\n{link}\n\nafter"), &notes::markdown());
    check("the pasted image is drawn", attachments_of(&tv).len() == 1, &format!("{:?}", attachments_of(&tv)));
    // cut moves a paragraph with its image
    notes::set_markdown(&both);
    tv.setSelectedRange(NSRange::new(0, tv.string().length()));
    unsafe {
        let _: () = msg_send![&*tv, cut: std::ptr::null::<AnyObject>()];
    }
    pump(50);
    check("cutting removes the image from the note", !notes::markdown().contains("attachments/"), &notes::markdown());
    unsafe {
        let _: () = msg_send![&*tv, paste: std::ptr::null::<AnyObject>()];
    }
    pump(50);
    check("pasting after a cut restores the whole selection", notes::markdown() == both, &notes::markdown());
    // text without images stays plain text on the pasteboard
    notes::set_markdown("**bold** word");
    tv.setSelectedRange(NSRange::new(0, 4));
    pb.clearContents();
    unsafe {
        let _: () = msg_send![&*tv, copy: std::ptr::null::<AnyObject>()];
    }
    check(
        "a selection without images is still copied as plain text",
        pb.stringForType(unsafe { NSPasteboardTypeString }).map(|s| s.to_string()).as_deref() == Some("bold"),
        "not plain",
    );
    match saved_text {
        Some(s) => {
            pb.clearContents();
            pb.setString_forType(&s, unsafe { NSPasteboardTypeString });
        }
        None => {
            pb.clearContents();
        }
    }

    // typing around a pasted picture, like a person: text, Return, paste, Return, text
    {
        let ins = |t: &str| {
            let s = NSString::from_str(t);
            unsafe {
                let _: () = msg_send![&*tv, insertText: &*s as &AnyObject];
            }
            pump(30);
        };
        let newline = || {
            unsafe {
                let _: () = msg_send![&*tv, insertNewline: std::ptr::null::<AnyObject>()];
            }
            pump(30);
        };
        notes::set_markdown("");
        tv.setSelectedRange(NSRange::new(0, 0));
        ins("Foto de prueba");
        newline();
        let pb = NSPasteboard::generalPasteboard();
        pb.clearContents();
        let bytes = png(64, 32);
        pb.setData_forType(Some(&NSData::with_bytes(&bytes)), unsafe { NSPasteboardTypePNG });
        got.borrow_mut().clear();
        unsafe {
            let _: () = msg_send![&*tv, paste: std::ptr::null::<AnyObject>()];
        }
        // what the app does with a pasted picture: store it and link it after the cursor line
        let pasted = match got.borrow().first() {
            Some(ImageInput::Png(b)) => attachments::store(&root, b, Some("png")).expect("store pasted"),
            other => panic!("expected pasted PNG data, got {other:?}"),
        };
        notes::insert_markdown_after_cursor_line(&attachments::markdown_link(&pasted.alt, &pasted.name));
        pump(30);
        newline();
        ins("Texto debajo");
        let want = format!("Foto de prueba\n\n![image](attachments/{})\n\nTexto debajo", pasted.name);
        check(
            "text, Return, paste, Return, text gives the picture on its own line between them",
            notes::markdown() == want,
            &notes::markdown(),
        );
        let at = attachments_of(&tv);
        check("exactly one picture is drawn, at the picture's position", at.len() == 1, &format!("{at:?}"));
        check(
            "no typed character carries the image",
            !notes::markdown().contains("![mage]") && !notes::markdown().contains("![i]"),
            &notes::markdown(),
        );
        // typing more after the picture line is plain text too
        newline();
        ins("otra linea");
        check(
            "a further line is plain text",
            notes::markdown() == format!("{want}\n\notra linea") && attachments_of(&tv).len() == 1,
            &notes::markdown(),
        );
        // Return after an image in the middle of a line splits around it without touching its alt text
        notes::set_markdown(&format!("a ![long alt text](attachments/{}) b", pasted.name));
        tv.setSelectedRange(NSRange::new(3, 0));
        newline();
        ins("X");
        check(
            "Return after an inline image keeps the whole alt text and the rest of the line",
            notes::markdown() == format!("a ![long alt text](attachments/{})\n\nX b", pasted.name),
            &notes::markdown(),
        );
        got.borrow_mut().clear();
    }

    // moving and copying a picture line inside the note (the drop path, same view as the source)
    {
        let name = wide.name.clone();
        let img = format!("![image](attachments/{name})");
        let base = format!("Foto de prueba\n\n{img}\n\nTexto debajo");
        let end = "Foto de prueba\n".encode_utf16().count() + 2 + "Texto debajo".encode_utf16().count();
        let select_image_line = || {
            notes::set_markdown(&base);
            // caret at the start of the image line, Shift+Down: the image and its line break
            tv.setSelectedRange(NSRange::new(15, 2));
            notes::markdown_of_selection()
        };
        // a copied picture line pasted after text goes on its own paragraph, not inline
        let md = select_image_line();
        tv.setSelectedRange(NSRange::new(end, 0));
        check("pasting a copied picture line at the end of a text line", notes::paste_markdown(&md), "refused");
        pump(30);
        check(
            "the pasted picture goes on its own paragraph after that line",
            notes::markdown() == format!("{base}\n\n{img}"),
            &notes::markdown(),
        );
        // dragging the picture line to the end of the text line moves it: once, on its own paragraph
        let md = select_image_line();
        check("the drop moves the picture line", notes::drop_markdown(&md, end, Some((15, 17))), "refused");
        pump(30);
        check(
            "moved: the original is gone and the picture follows the text on its own paragraph",
            notes::markdown() == format!("Foto de prueba\n\nTexto debajo\n\n{img}") && attachments_of(&tv).len() == 1,
            &notes::markdown(),
        );
        if let Some(um) = tv.undoManager() {
            um.undo();
            pump(30);
        }
        check("one undo puts a moved picture back", notes::markdown() == base && attachments_of(&tv).len() == 1, &notes::markdown());
        // ⌥-drag copies: both stay
        let md = select_image_line();
        notes::drop_markdown(&md, end, None);
        pump(30);
        check(
            "copied: the picture is in both places, each on its own paragraph",
            notes::markdown() == format!("{base}\n\n{img}") && attachments_of(&tv).len() == 2,
            &notes::markdown(),
        );
        // dropping onto the dragged range changes nothing; dropping before the source works too
        let md = select_image_line();
        notes::drop_markdown(&md, 16, Some((15, 17)));
        pump(30);
        check("dropping a selection onto itself changes nothing", notes::markdown() == base, &notes::markdown());
        notes::set_markdown(&format!("Foto de prueba\n\n{img}\n\nTexto debajo\n\n{img}"));
        let last = "Foto de prueba\n".encode_utf16().count() + 2 + "Texto debajo".encode_utf16().count() + 1;
        tv.setSelectedRange(NSRange::new(last, 1));
        let md = notes::markdown_of_selection();
        notes::drop_markdown(&md, 5, Some((last, last + 1)));
        pump(30);
        check(
            "a picture at the end moved up to the first line lands after it, once",
            notes::markdown() == format!("Foto de prueba\n\n{img}\n\n{img}\n\nTexto debajo") && attachments_of(&tv).len() == 2,
            &notes::markdown(),
        );
    }

    // undo after a move in a realistic session: typing, paste, Return, typing, then the drag
    {
        let ins = |t: &str| {
            let s = NSString::from_str(t);
            unsafe {
                let _: () = msg_send![&*tv, insertText: &*s as &AnyObject];
            }
            pump(30);
        };
        let newline = || {
            unsafe {
                let _: () = msg_send![&*tv, insertNewline: std::ptr::null::<AnyObject>()];
            }
            pump(30);
        };
        let undo = |n: &str| {
            if let Some(um) = tv.undoManager() {
                um.undo();
            }
            pump(30);
            println!("      undo {n}: {:?}", notes::markdown());
        };
        notes::set_markdown("");
        tv.setSelectedRange(NSRange::new(0, 0));
        ins("Foto de prueba");
        newline();
        let pb = NSPasteboard::generalPasteboard();
        pb.clearContents();
        pb.setData_forType(Some(&NSData::with_bytes(&png(64, 32))), unsafe { NSPasteboardTypePNG });
        got.borrow_mut().clear();
        unsafe {
            let _: () = msg_send![&*tv, paste: std::ptr::null::<AnyObject>()];
        }
        let pasted = match got.borrow().first() {
            Some(ImageInput::Png(b)) => attachments::store(&root, b, Some("png")).expect("store pasted"),
            other => panic!("expected pasted PNG data, got {other:?}"),
        };
        notes::insert_markdown_after_cursor_line(&attachments::markdown_link(&pasted.alt, &pasted.name));
        pump(30);
        newline();
        ins("Texto debajo");
        let img = format!("![image](attachments/{})", pasted.name);
        let before = format!("Foto de prueba\n\n{img}\n\nTexto debajo");
        check("the session body before the drag", notes::markdown() == before, &notes::markdown());
        // select the picture line (caret at its start, Shift+Down) and drop it below the text
        // The drag starts on a selection that was already there and the drop does not touch the
        // selection first: the typing that came before may still be coalescing in the text view.
        let r = NSRange::new(15, 2);
        let md = format!("{img}\n\n<br>");
        let md2 = img.clone();
        let end = tv.string().length();
        // a real drop runs inside the drag session's event-tracking run loop mode
        let from = Some((r.location, r.location + r.length));
        let blk = block2::RcBlock::new(move || {
            notes::drop_markdown(&md, end, from);
        });
        unsafe {
            NSRunLoop::currentRunLoop()
                .performInModes_block(&objc2_foundation::NSArray::from_slice(&[objc2_app_kit::NSEventTrackingRunLoopMode]), &blk);
        }
        unsafe {
            NSRunLoop::currentRunLoop()
                .runMode_beforeDate(objc2_app_kit::NSEventTrackingRunLoopMode, &NSDate::dateWithTimeIntervalSinceNow(0.2))
        };
        pump(30);
        let moved = "Foto de prueba\n\nTexto debajo\n\n".to_string() + &img;
        check("the move", notes::markdown() == moved, &notes::markdown());
        undo("1");
        check("one undo restores exactly the pre-drag body", notes::markdown() == before, &notes::markdown());
        if let Some(um) = tv.undoManager() {
            um.redo();
        }
        pump(30);
        check("redo applies the move again", notes::markdown() == moved, &notes::markdown());
        undo("1 again");
        check("and undo puts it back again", notes::markdown() == before, &notes::markdown());
        // an ⌥-drag copy in the same session: both pictures, one undo removes the copy only
        notes::drop_markdown(&md2, end, None);
        pump(30);
        check(
            "copied below the text",
            notes::markdown() == format!("{before}\n\n{img}") && attachments_of(&tv).len() == 2,
            &notes::markdown(),
        );
        undo("copy");
        check(
            "one undo removes the copy and nothing else",
            notes::markdown() == before && attachments_of(&tv).len() == 1,
            &notes::markdown(),
        );
        undo("2");
        check(
            "the next undo undoes typing, never leaving an empty paragraph with lost text",
            !notes::markdown().contains("<br>") || notes::markdown() == "",
            &notes::markdown(),
        );
    }

    // a file removed from disk is not drawn from the cache
    let tmp_img = attachments::store(&root, &png(20, 10), Some("png")).expect("store");
    let line = format!("![t](attachments/{})", tmp_img.name);
    notes::set_markdown(&line);
    check(
        "an image on disk is drawn at its size",
        attachments_of(&tv).first().is_some_and(|a| (a.1 - 20.0).abs() < 0.01),
        &format!("{:?}", attachments_of(&tv)),
    );
    std::fs::remove_file(root.join(&tmp_img.rel)).unwrap();
    notes::set_markdown(&line);
    check(
        "after the file is deleted the placeholder is drawn, not the cached picture",
        attachments_of(&tv).first().is_some_and(|a| (a.1 - 48.0).abs() < 0.01),
        &format!("{:?}", attachments_of(&tv)),
    );

    // a snapshot for the eye
    notes::set_markdown(&format!(
        "# Trip notes\n\nPhoto from the **hotel**:\n\n![wide](attachments/{})\n\nAnd a thumbnail ![small](attachments/{}) inline.",
        wide.name, small.name
    ));
    pump(400);
    let shot: &Path = &out.join("notes-images.png");
    check("snapshot written", notes::snapshot_png(shot), "snapshot failed");
    println!("      {}", shot.display());

    let failures = FAILURES.with(|f| f.borrow().clone());
    if failures.is_empty() {
        println!("notes_images_demo: all checks passed");
    } else {
        println!("notes_images_demo: {} failed: {failures:?}", failures.len());
        std::process::exit(1);
    }
}
