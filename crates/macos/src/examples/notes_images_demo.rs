//! Check for images in Notes (attachments drawn inline, round trip, editing around them, paste):
//! `cargo run -p ferrocast-macos --example notes_images_demo [-- --out <dir>]`
//! Opens the real Notes window, drives it programmatically and asserts the behavior. Exit code 0
//! only when every assertion holds. Writes a snapshot of the note with an image.

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
