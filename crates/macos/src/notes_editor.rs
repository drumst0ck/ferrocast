//! Rich-text Markdown editor for the Notes window (S6 to S9, S14).
//!
//! The note is stored as Markdown and edited as attributed text in an `NSTextView`. All Markdown
//! logic lives in `ferrocast_core::richtext`; this module only maps a [`Doc`] to and from an
//! `NSTextStorage` and wires the typing behavior.
//!
//! Storage model (the Markdown markers are never in the text):
//! * One paragraph (separated by `\n`) per block. A code block is a single paragraph whose lines
//!   are joined with U+2028 so Enter inside it never splits the block.
//! * Semantic attributes carry the model: `fcBlock` (kind, level, code language) on every
//!   character of a paragraph, `fcStyle` (bold/italic/strike/underline/code bits), `fcLink` and
//!   `fcMarker` for the generated list marker. The paragraph kind is read from its terminating
//!   `\n` (the last character for the final paragraph), so typing at the start of a line never
//!   changes it. Visual attributes (fonts, colors, indents) are derived from the semantic ones by
//!   `attrs`, which also applies the zoom.
//! * List items start with a marker (`•\t`, `1.\t`, `☐\t`) tagged `fcMarker`; the selection is
//!   clamped so the cursor cannot sit inside it, deletions that reach it are handled at block
//!   level, and `normalize` renumbers ordered lists after every change.
//! * Quote bars, code block backgrounds and horizontal rules are drawn by the text view.
//! * Empty headings, quotes and code blocks hold one U+200B so the kind survives; rules always do.
//!
//! The pure helpers (zoom steps, numbering, block toggles, span splitting, slash/emoji queries)
//! have unit tests; the AppKit behavior is verified by `examples/notes_demo.rs --check`.

use std::cell::{Cell, RefCell};
use std::ptr::{self, NonNull};
use std::rc::Rc;

use ferrocast_core::richtext::{self, Block, BlockKind, Doc, ListEnter, Span, Style, MAX_LEVEL};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject, Sel};
use objc2::{define_class, msg_send, sel, AllocAnyThread, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSBezierPath, NSColor, NSControl, NSControlTextEditingDelegate, NSEvent, NSEventModifierFlags, NSEventType, NSFont,
    NSFontAttributeName, NSFontManager, NSFontTraitMask, NSFontWeight, NSFontWeightBold, NSFontWeightLight, NSFontWeightMedium,
    NSFontWeightRegular, NSForegroundColorAttributeName, NSKernAttributeName, NSLayoutManager, NSMenu, NSMutableParagraphStyle,
    NSParagraphStyleAttributeName, NSPasteboard, NSPasteboardType, NSPasteboardTypeString, NSPopover, NSPopoverBehavior,
    NSStrikethroughStyleAttributeName, NSTextAlignment, NSTextContainer, NSTextField, NSTextFieldDelegate, NSTextInputClient,
    NSTextStorage, NSTextTab, NSTextView, NSUnderlineStyleAttributeName, NSView, NSViewController, NSWorkspace,
};
use objc2_foundation::{
    NSArray, NSAttributedString, NSAttributedStringKey, NSDictionary, NSMutableAttributedString, NSMutableDictionary, NSNumber, NSObject,
    NSObjectProtocol, NSPoint, NSRange, NSRect, NSRectEdge, NSSize, NSString, NSURL,
};

use crate::notes_format::Format;
use crate::notes_theme::{self, Token};

pub const ZOOM_MIN: f64 = 0.5;
pub const ZOOM_MAX: f64 = 3.0;

const LINE_SEP: char = '\u{2028}';
const ZWSP: char = '\u{200B}';
const ZWSP_U16: u16 = 0x200B;
const BASE_SIZE: f64 = 14.0;
/// Space above a code block (the line fragment of its first line includes it).
const CODE_SPACE_BEFORE: f64 = 14.0;
/// Alpha of the `Accent` selection background (S4); the demo checks the contrast it leaves.
pub const SELECTION_ALPHA: f64 = 0.28;
const LIST_INDENT: f64 = 22.0;
const LIST_BASE: f64 = 6.0;

const BOLD: u8 = 1;
const ITALIC: u8 = 2;
const STRIKE: u8 = 4;
const UNDERLINE: u8 = 8;
const CODE: u8 = 16;

// ------------------------------------------------------------------ public types

/// Screen-coordinate rectangle (origin bottom-left, like `NSScreen`) next to the typed trigger.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// `/` menu session events.
#[derive(Clone, Debug, PartialEq)]
pub enum SlashEvent {
    Open { anchor: Anchor },
    Query(String),
    Close,
}

/// `:` emoji session events.
#[derive(Clone, Debug, PartialEq)]
pub enum EmojiEvent {
    Open { anchor: Anchor },
    Query(String),
    Close,
}

/// Navigation keys routed to an open pop-up before the text view sees them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupKey {
    Up,
    Down,
    Enter,
    Tab,
    Esc,
}

// ------------------------------------------------------------------ pure logic

/// Next zoom level after `delta` steps of 10 %, clamped to 50 %..=300 %.
pub fn step_zoom(current: f64, delta: i32) -> f64 {
    let steps = (current * 10.0).round() as i32 + delta;
    (f64::from(steps) / 10.0).clamp(ZOOM_MIN, ZOOM_MAX)
}

fn is_list(k: &BlockKind) -> bool {
    matches!(k, BlockKind::Bullet | BlockKind::Ordered | BlockKind::Task { .. })
}

/// Effective level and ordered number of every block, with the same rules `to_markdown` uses:
/// a list item is at most one level deeper than the previous list item (0 after a non-list
/// block); ordered numbers count per level and restart after a paragraph or a bullet/task at the
/// same level. The number is 0 for non-ordered blocks.
pub fn layout_numbers(items: &[(BlockKind, u8)]) -> Vec<(u8, usize)> {
    let mut out = Vec::with_capacity(items.len());
    let mut prev: Option<u8> = None;
    let mut counters: Vec<usize> = Vec::new();
    for (kind, lvl) in items {
        let list = is_list(kind);
        let level = match (list, prev) {
            (true, Some(p)) => (*lvl).min(MAX_LEVEL).min(p + 1),
            _ => 0,
        };
        let mut number = 0;
        if *kind == BlockKind::Ordered {
            counters.truncate(level as usize + 1);
            while counters.len() <= level as usize {
                counters.push(0);
            }
            counters[level as usize] += 1;
            number = counters[level as usize];
        }
        if list {
            if *kind != BlockKind::Ordered {
                counters.truncate(level as usize);
            }
        } else {
            counters.clear();
        }
        prev = list.then_some(level);
        out.push((level, number));
    }
    out
}

fn block_text(b: &Block) -> String {
    b.spans.iter().map(|s| s.text.as_str()).collect()
}

fn para(spans: Vec<Span>) -> Block {
    Block { kind: BlockKind::Paragraph, level: 0, spans }
}

/// Code blocks become one paragraph per line, rules an empty paragraph; others stay as they are.
fn flatten(b: &Block) -> Vec<Block> {
    match &b.kind {
        BlockKind::Code { .. } => {
            let text = block_text(b);
            text.split('\n')
                .map(|l| para(if l.is_empty() { vec![] } else { vec![Span { text: l.to_string(), style: Style::default() }] }))
                .collect()
        }
        BlockKind::Rule => vec![para(vec![])],
        _ => vec![b.clone()],
    }
}

/// Result of applying (or removing) a block format to the selected blocks. Applying the format
/// the blocks already have turns them back into paragraphs.
pub fn toggle_block(blocks: &[Block], fmt: Format) -> Vec<Block> {
    let target = |b: &Block| -> Option<BlockKind> {
        Some(match fmt {
            Format::Heading(n) => BlockKind::Heading(n.clamp(1, 3)),
            Format::Quote => BlockKind::Quote,
            Format::BulletList => BlockKind::Bullet,
            Format::OrderedList => BlockKind::Ordered,
            Format::TaskList => BlockKind::Task { checked: matches!(b.kind, BlockKind::Task { checked: true }) },
            _ => return None,
        })
    };
    match fmt {
        Format::CodeBlock => {
            if !blocks.is_empty() && blocks.iter().all(|b| matches!(b.kind, BlockKind::Code { .. })) {
                return blocks.iter().flat_map(flatten).collect();
            }
            let lines: Vec<String> = blocks.iter().flat_map(flatten).map(|b| block_text(&b)).collect();
            let text = lines.join("\n");
            let spans = if text.is_empty() { vec![] } else { vec![Span { text, style: Style::default() }] };
            vec![Block { kind: BlockKind::Code { lang: String::new() }, level: 0, spans }]
        }
        Format::Rule => {
            let mut out: Vec<Block> = blocks.to_vec();
            let empty_last = out.last().is_some_and(|b| b.kind == BlockKind::Paragraph && b.spans.iter().all(|s| s.text.is_empty()));
            if empty_last {
                out.pop();
            }
            out.push(Block { kind: BlockKind::Rule, level: 0, spans: vec![] });
            out.push(para(vec![]));
            out
        }
        Format::Heading(_) | Format::Quote | Format::BulletList | Format::OrderedList | Format::TaskList => {
            let same = !blocks.is_empty()
                && blocks.iter().all(|b| match (target(b), &b.kind) {
                    (Some(BlockKind::Task { .. }), BlockKind::Task { .. }) => true,
                    (Some(t), k) => t == *k,
                    _ => false,
                });
            blocks
                .iter()
                .flat_map(flatten)
                .map(|mut b| {
                    let was_list = is_list(&b.kind);
                    match (same, target(&b)) {
                        (false, Some(t)) => {
                            b.level = if is_list(&t) && was_list { b.level } else { 0 };
                            b.kind = t;
                        }
                        _ => {
                            b.kind = BlockKind::Paragraph;
                            b.level = 0;
                        }
                    }
                    b
                })
                .collect()
        }
        _ => blocks.to_vec(),
    }
}

/// Split spans at a UTF-16 offset into the text before and after it.
pub fn split_spans(spans: &[Span], at: usize) -> (Vec<Span>, Vec<Span>) {
    let (mut left, mut right) = (Vec::new(), Vec::new());
    let mut pos = 0usize;
    for s in spans {
        let len: usize = s.text.encode_utf16().count();
        if pos + len <= at {
            left.push(s.clone());
        } else if pos >= at {
            right.push(s.clone());
        } else {
            let (mut a, mut b) = (String::new(), String::new());
            let mut off = pos;
            for c in s.text.chars() {
                if off < at {
                    a.push(c);
                } else {
                    b.push(c);
                }
                off += c.len_utf16();
            }
            if !a.is_empty() {
                left.push(Span { text: a, style: s.style.clone() });
            }
            if !b.is_empty() {
                right.push(Span { text: b, style: s.style.clone() });
            }
        }
        pos += len;
    }
    (left, right)
}

/// `/` menu query: only when the line is `/` plus non-space characters and the cursor is at its end.
pub fn slash_query(content: &str, before: &str) -> Option<String> {
    if content != before {
        return None;
    }
    let rest = content.strip_prefix('/')?;
    (!rest.chars().any(char::is_whitespace)).then(|| rest.to_string())
}

/// Emoji picker query: the text after the last `:` when that colon starts a word.
pub fn emoji_query(before: &str) -> Option<String> {
    let i = before.rfind(':')?;
    let q = &before[i + 1..];
    if !q.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '+' | '-')) {
        return None;
    }
    let prev = before[..i].chars().next_back();
    prev.is_none_or(char::is_whitespace).then(|| q.to_string())
}

fn style_bits(s: &Style) -> u8 {
    [(s.bold, BOLD), (s.italic, ITALIC), (s.strike, STRIKE), (s.underline, UNDERLINE), (s.code, CODE)]
        .iter()
        .filter(|(on, _)| *on)
        .fold(0, |acc, (_, bit)| acc | bit)
}

fn bits_style(bits: u8, link: Option<String>) -> Style {
    Style {
        bold: bits & BOLD != 0,
        italic: bits & ITALIC != 0,
        strike: bits & STRIKE != 0,
        underline: bits & UNDERLINE != 0,
        code: bits & CODE != 0,
        link,
    }
}

fn encode_tag(kind: &BlockKind, level: u8) -> String {
    let (code, extra) = match kind {
        BlockKind::Paragraph => ("p".to_string(), String::new()),
        BlockKind::Heading(n) => (format!("h{n}"), String::new()),
        BlockKind::Quote => ("q".into(), String::new()),
        BlockKind::Bullet => ("b".into(), String::new()),
        BlockKind::Ordered => ("o".into(), String::new()),
        BlockKind::Task { checked } => (if *checked { "t1" } else { "t0" }.into(), String::new()),
        BlockKind::Code { lang } => ("c".into(), lang.clone()),
        BlockKind::Rule => ("r".into(), String::new()),
    };
    format!("{code}|{level}|{extra}")
}

fn decode_tag(s: &str) -> (BlockKind, u8) {
    let mut it = s.splitn(3, '|');
    let code = it.next().unwrap_or("p");
    let level = it.next().and_then(|l| l.parse().ok()).unwrap_or(0u8).min(MAX_LEVEL);
    let extra = it.next().unwrap_or("");
    let kind = match code {
        "h1" => BlockKind::Heading(1),
        "h2" => BlockKind::Heading(2),
        "h3" => BlockKind::Heading(3),
        "q" => BlockKind::Quote,
        "b" => BlockKind::Bullet,
        "o" => BlockKind::Ordered,
        "t0" => BlockKind::Task { checked: false },
        "t1" => BlockKind::Task { checked: true },
        "c" => BlockKind::Code { lang: extra.to_string() },
        "r" => BlockKind::Rule,
        _ => BlockKind::Paragraph,
    };
    let level = if is_list(&kind) { level } else { 0 };
    (kind, level)
}

fn marker_text(kind: &BlockKind, level: u8, number: usize) -> String {
    match kind {
        BlockKind::Bullet => format!("{}\t", ["\u{2022}", "\u{25E6}", "\u{25AA}"][(level as usize).min(2)]),
        BlockKind::Ordered => format!("{number}.\t"),
        BlockKind::Task { checked } => format!("{}\t", if *checked { "\u{2611}" } else { "\u{2610}" }),
        _ => String::new(),
    }
}

fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

fn clean_text(units: &[u16], code: bool) -> String {
    let s: String = String::from_utf16_lossy(units);
    s.chars().filter(|&c| c != ZWSP).map(|c| if code && c == LINE_SEP { '\n' } else { c }).collect()
}

// ------------------------------------------------------------------ state

type SlashCb = Rc<dyn Fn(SlashEvent)>;
type EmojiCb = Rc<dyn Fn(EmojiEvent)>;
type PopupCb = Rc<dyn Fn(PopupKey) -> bool>;
type ZoomCb = Rc<dyn Fn(f64)>;

/// One open trigger session: where the trigger character sits and the last query sent.
#[derive(Clone, Debug, PartialEq)]
struct Session {
    loc: usize,
    query: String,
}

struct LinkPrompt {
    popover: Retained<NSPopover>,
    field: Retained<NSTextField>,
    range: NSRange,
    _target: Retained<LinkTarget>,
}

thread_local! {
    static ZOOM: Cell<f64> = const { Cell::new(1.0) };
    static SUPPRESS: Cell<bool> = const { Cell::new(false) };
    /// True while this module edits the storage itself, so the delegate hooks stay out of the way.
    static PROGRAMMATIC: Cell<bool> = const { Cell::new(false) };
    /// Range touched by the edit in flight (new coordinates); `None` means "everything".
    static DIRTY: Cell<Option<(usize, usize)>> = const { Cell::new(None) };
    /// A plain typing edit in flight: where it landed and how long it is (for link inheritance).
    static TYPED: Cell<Option<(usize, usize)>> = const { Cell::new(None) };
    static SLASH: RefCell<Option<Session>> = const { RefCell::new(None) };
    static EMOJI: RefCell<Option<Session>> = const { RefCell::new(None) };
    static SLASH_DISMISSED: Cell<Option<usize>> = const { Cell::new(None) };
    static EMOJI_DISMISSED: Cell<Option<usize>> = const { Cell::new(None) };
    static ON_SLASH: RefCell<Option<SlashCb>> = const { RefCell::new(None) };
    static ON_EMOJI: RefCell<Option<EmojiCb>> = const { RefCell::new(None) };
    static ON_POPUP_KEY: RefCell<Option<PopupCb>> = const { RefCell::new(None) };
    static ON_ZOOM: RefCell<Option<ZoomCb>> = const { RefCell::new(None) };
    static LINK: RefCell<Option<LinkPrompt>> = const { RefCell::new(None) };
    static KEYS: Keys = Keys::new();
}

struct Keys {
    block: Retained<NSString>,
    style: Retained<NSString>,
    link: Retained<NSString>,
    marker: Retained<NSString>,
}

impl Keys {
    fn new() -> Self {
        Keys {
            block: NSString::from_str("fcBlock"),
            style: NSString::from_str("fcStyle"),
            link: NSString::from_str("fcLink"),
            marker: NSString::from_str("fcMarker"),
        }
    }
}

fn storage_of(tv: &NSTextView) -> Option<Retained<NSTextStorage>> {
    unsafe { tv.textStorage() }
}

fn layout_of(tv: &NSTextView) -> Option<Retained<NSLayoutManager>> {
    unsafe { tv.layoutManager() }
}

fn container_of(tv: &NSTextView) -> Option<Retained<NSTextContainer>> {
    unsafe { tv.textContainer() }
}

fn range(loc: usize, len: usize) -> NSRange {
    NSRange::new(loc, len)
}

fn handles() -> Option<(Retained<NSTextView>, Retained<NSTextStorage>)> {
    let tv = crate::notes::text_view()?;
    let st = storage_of(&tv)?;
    Some((tv, st))
}

// ------------------------------------------------------------------ attributes

type AttrDict = NSMutableDictionary<NSAttributedStringKey, AnyObject>;

fn put(d: &AttrDict, key: &NSString, value: &AnyObject) {
    unsafe { d.setObject_forKey(value, ProtocolObject::from_ref(key)) };
}

fn font_for(size: f64, weight: NSFontWeight, italic: bool, mono: bool) -> Retained<NSFont> {
    let base = if mono { NSFont::monospacedSystemFontOfSize_weight(size, weight) } else { NSFont::systemFontOfSize_weight(size, weight) };
    if italic {
        if let Some(mtm) = MainThreadMarker::new() {
            return NSFontManager::sharedFontManager(mtm).convertFont_toHaveTrait(&base, NSFontTraitMask::ItalicFontMask);
        }
    }
    base
}

fn tab_stop(location: f64) -> Retained<NSTextTab> {
    let opts: Retained<NSDictionary<NSString, AnyObject>> = NSDictionary::new();
    unsafe { NSTextTab::initWithTextAlignment_location_options(NSTextTab::alloc(), NSTextAlignment::Left, location, &opts) }
}

/// Every attribute of one run, semantic and visual, for the current zoom.
fn attrs(kind: &BlockKind, level: u8, bits: u8, link: Option<&str>, marker: bool) -> Retained<AttrDict> {
    let z = zoom();
    let d: Retained<AttrDict> = NSMutableDictionary::new();
    let (code_block, rule, heading) =
        (matches!(kind, BlockKind::Code { .. }), *kind == BlockKind::Rule, matches!(kind, BlockKind::Heading(_)));
    let (bits, link) = if code_block || rule { (0, None) } else { (bits, link) };
    KEYS.with(|k| {
        put(&d, &k.block, &NSString::from_str(&encode_tag(kind, level)));
        put(&d, &k.style, &NSNumber::new_isize(bits as isize));
        if let Some(l) = link {
            put(&d, &k.link, &NSString::from_str(l));
        }
        if marker {
            put(&d, &k.marker, &NSNumber::new_isize(1));
        }
    });

    let base = match kind {
        BlockKind::Heading(1) => 24.0,
        BlockKind::Heading(2) => 20.0,
        BlockKind::Heading(_) => 17.0,
        BlockKind::Code { .. } => 13.0,
        BlockKind::Rule => 4.0,
        _ => BASE_SIZE,
    };
    let checked = matches!(kind, BlockKind::Task { checked: true });
    let size = if marker && matches!(kind, BlockKind::Task { .. }) { base * 1.15 } else { base } * z;
    let inline_code = bits & CODE != 0;
    // S6: headings echo the web's display style (light h1, regular h2, medium h3, -0.022 em).
    let weight = unsafe {
        if bits & BOLD != 0 {
            NSFontWeightBold
        } else {
            match kind {
                BlockKind::Heading(1) => NSFontWeightLight,
                BlockKind::Heading(2) => NSFontWeightRegular,
                BlockKind::Heading(_) => NSFontWeightMedium,
                _ => NSFontWeightRegular,
            }
        }
    };
    let font = font_for(if inline_code { size * 0.93 } else { size }, weight, bits & ITALIC != 0, code_block || inline_code);
    // Task boxes are drawn by `draw_decorations`; their glyph stays in the text (click target,
    // markdown round trip) but is invisible.
    let color = if marker {
        match kind {
            BlockKind::Task { .. } => NSColor::clearColor(),
            _ => notes_theme::color(Token::Sub),
        }
    } else if checked {
        notes_theme::color(Token::Sub)
    } else if *kind == BlockKind::Quote {
        notes_theme::color(Token::SubStrong)
    } else if link.is_some() {
        notes_theme::color(Token::AccentText)
    } else {
        notes_theme::color(Token::Text)
    };
    unsafe {
        put(&d, NSFontAttributeName, &font);
        if heading && !marker {
            put(&d, NSKernAttributeName, &NSNumber::new_f64(-0.022 * size));
        }
        put(&d, NSForegroundColorAttributeName, &color);
        if !marker && (bits & STRIKE != 0 || checked) {
            put(&d, NSStrikethroughStyleAttributeName, &NSNumber::new_isize(1));
        }
        if !marker && (bits & UNDERLINE != 0 || link.is_some()) {
            put(&d, NSUnderlineStyleAttributeName, &NSNumber::new_isize(1));
        }
    }

    let ps = NSMutableParagraphStyle::new();
    // About 1.4 line height on body text, tighter on headings and code (S6).
    match kind {
        BlockKind::Heading(_) => {
            ps.setLineSpacing(0.0);
            ps.setLineHeightMultiple(1.12);
        }
        BlockKind::Code { .. } => {
            ps.setLineSpacing(2.0 * z);
            ps.setLineHeightMultiple(1.15);
        }
        BlockKind::Rule => ps.setLineSpacing(2.0 * z),
        _ => {
            ps.setLineSpacing(0.0);
            ps.setLineHeightMultiple(1.4);
        }
    }
    ps.setParagraphSpacing(match kind {
        BlockKind::Quote => 4.0 * z,
        BlockKind::Rule => 6.0 * z,
        k if is_list(k) => 2.0 * z,
        _ => 6.0 * z,
    });
    ps.setParagraphSpacingBefore(match kind {
        BlockKind::Heading(_) => 6.0 * z,
        BlockKind::Code { .. } => CODE_SPACE_BEFORE * z,
        BlockKind::Rule => 6.0 * z,
        _ => 0.0,
    });
    match kind {
        k if is_list(k) => {
            let first = (LIST_BASE + LIST_INDENT * f64::from(level)) * z;
            let head = first + LIST_INDENT * z;
            ps.setFirstLineHeadIndent(first);
            ps.setHeadIndent(head);
            ps.setTabStops(Some(&NSArray::from_retained_slice(&[tab_stop(head)])));
        }
        BlockKind::Quote => {
            ps.setFirstLineHeadIndent(14.0 * z);
            ps.setHeadIndent(14.0 * z);
        }
        BlockKind::Code { .. } => {
            ps.setFirstLineHeadIndent(10.0 * z);
            ps.setHeadIndent(10.0 * z);
            ps.setTailIndent(-10.0 * z);
        }
        _ => {}
    }
    unsafe { put(&d, NSParagraphStyleAttributeName, &ps) };
    d
}

fn attributed(text: &str, a: &AttrDict) -> Retained<NSAttributedString> {
    let dict: &NSDictionary<NSAttributedStringKey, AnyObject> = a;
    unsafe { NSAttributedString::initWithString_attributes(NSAttributedString::alloc(), &NSString::from_str(text), Some(dict)) }
}

// ------------------------------------------------------------------ reading the storage

#[derive(Clone, Debug)]
struct Run {
    a: usize,
    b: usize,
    bits: u8,
    link: Option<String>,
}

/// One paragraph of the storage. `start..cs` is the marker, `cs..end` the content.
#[derive(Clone, Debug)]
struct Para {
    start: usize,
    cs: usize,
    end: usize,
    nl: bool,
    kind: BlockKind,
    level: u8,
    runs: Vec<Run>,
}

fn attr_value(st: &NSAttributedString, key: &NSString, i: usize, eff: &mut NSRange) -> Option<Retained<AnyObject>> {
    if i >= st.length() {
        return None;
    }
    unsafe { st.attribute_atIndex_effectiveRange(key, i, eff as *mut NSRange) }
}

fn attr_string(st: &NSAttributedString, key: &NSString, i: usize, eff: &mut NSRange) -> Option<String> {
    attr_value(st, key, i, eff)?.downcast_ref::<NSString>().map(|s| s.to_string())
}

fn attr_int(st: &NSAttributedString, key: &NSString, i: usize, eff: &mut NSRange) -> Option<isize> {
    attr_value(st, key, i, eff)?.downcast_ref::<NSNumber>().map(|n| n.as_isize())
}

fn units_of(st: &NSAttributedString) -> Vec<u16> {
    let s = st.string();
    let n = s.length();
    let mut u = vec![0u16; n];
    if n > 0 {
        unsafe { s.getCharacters_range(NonNull::new_unchecked(u.as_mut_ptr()), range(0, n)) };
    }
    u
}

fn read_paras(st: &NSAttributedString, with_runs: bool) -> (Vec<u16>, Vec<Para>) {
    let u = units_of(st);
    let mut out = Vec::new();
    let mut eff = NSRange::new(0, 0);
    let mut start = 0usize;
    loop {
        let end = u[start..].iter().position(|&c| c == 10).map_or(u.len(), |p| start + p);
        let nl = end < u.len();
        let tag_at = if nl {
            Some(end)
        } else if end > start {
            Some(end - 1)
        } else {
            None
        };
        let (mut kind, mut level) = tag_at
            .and_then(|i| KEYS.with(|k| attr_string(st, &k.block, i, &mut eff)))
            .map_or((BlockKind::Paragraph, 0), |t| decode_tag(&t));
        let mut cs = start;
        if is_list(&kind) {
            let has_marker = end > start && KEYS.with(|k| attr_int(st, &k.marker, start, &mut eff)) == Some(1);
            match (has_marker, u[start..end].iter().position(|&c| c == 9)) {
                (true, Some(t)) => cs = start + t + 1,
                _ => {
                    kind = BlockKind::Paragraph;
                    level = 0;
                }
            }
        }
        let mut runs = Vec::new();
        if with_runs {
            let mut i = cs;
            while i < end {
                let (bits, link, e1, e2) = KEYS.with(|k| {
                    let mut r1 = NSRange::new(0, 0);
                    let mut r2 = NSRange::new(0, 0);
                    let bits = attr_int(st, &k.style, i, &mut r1).unwrap_or(0) as u8;
                    let link = attr_string(st, &k.link, i, &mut r2);
                    (bits, link, r1.location + r1.length, r2.location + r2.length)
                });
                let e = e1.min(e2).min(end).max(i + 1);
                runs.push(Run { a: i, b: e, bits, link });
                i = e;
            }
        }
        out.push(Para { start, cs, end, nl, kind, level, runs });
        if !nl {
            break;
        }
        start = end + 1;
    }
    (u, out)
}

/// Spans of `p` clipped to `a..b` (UTF-16 offsets into the storage), adjacent equal styles merged.
fn spans_of(u: &[u16], p: &Para, a: usize, b: usize) -> Vec<Span> {
    if matches!(p.kind, BlockKind::Rule) {
        return vec![];
    }
    let code = matches!(p.kind, BlockKind::Code { .. });
    let mut out: Vec<Span> = Vec::new();
    for r in &p.runs {
        let (x, y) = (r.a.max(a), r.b.min(b));
        if x >= y {
            continue;
        }
        let text = clean_text(&u[x..y], code);
        if text.is_empty() {
            continue;
        }
        let style = bits_style(r.bits, r.link.clone());
        match out.last_mut() {
            Some(last) if last.style == style => last.text.push_str(&text),
            _ => out.push(Span { text, style }),
        }
    }
    out
}

fn block_of(u: &[u16], p: &Para) -> Block {
    Block { kind: p.kind.clone(), level: p.level, spans: spans_of(u, p, p.cs, p.end) }
}

/// Index of the paragraph that holds storage position `x` (a `\n` belongs to the paragraph it ends).
fn para_at(paras: &[Para], x: usize) -> usize {
    paras.iter().position(|p| x <= p.end).unwrap_or(paras.len() - 1)
}

/// Paragraphs touched by the selection `a..b`.
fn sel_paras(paras: &[Para], a: usize, b: usize) -> (usize, usize) {
    let lo = para_at(paras, a);
    let mut hi = para_at(paras, b);
    if b > a && hi > lo && b == paras[hi].start {
        hi -= 1;
    }
    (lo, hi.max(lo))
}

fn in_selection(p: &Para, a: usize, b: usize) -> bool {
    p.start < b && (p.end > a || p.start >= a)
}

fn doc_of(st: &NSAttributedString) -> Doc {
    let (u, paras) = read_paras(st, true);
    if u.is_empty() {
        return Doc::default();
    }
    Doc { blocks: paras.iter().map(|p| block_of(&u, p)).collect() }
}

// ------------------------------------------------------------------ rendering

fn render(blocks: &[Block], trailing: bool) -> (Retained<NSMutableAttributedString>, Vec<usize>) {
    let out = NSMutableAttributedString::new();
    let items: Vec<(BlockKind, u8)> = blocks.iter().map(|b| (b.kind.clone(), b.level)).collect();
    let lay = layout_numbers(&items);
    let mut starts = Vec::with_capacity(blocks.len());
    for (i, b) in blocks.iter().enumerate() {
        let number = lay[i].1;
        // The level is kept as given: `normalize` clamps it against the whole document.
        let level = if is_list(&b.kind) { b.level.min(MAX_LEVEL) } else { 0 };
        starts.push(out.length());
        if is_list(&b.kind) {
            out.appendAttributedString(&attributed(&marker_text(&b.kind, level, number), &attrs(&b.kind, level, 0, None, true)));
        }
        let mut wrote = false;
        for s in &b.spans {
            if s.text.is_empty() {
                continue;
            }
            let text = if matches!(b.kind, BlockKind::Code { .. }) {
                s.text.replace('\n', &LINE_SEP.to_string())
            } else {
                s.text.replace('\n', " ")
            };
            let a = attrs(&b.kind, level, style_bits(&s.style), s.style.link.as_deref(), false);
            out.appendAttributedString(&attributed(&text, &a));
            wrote = true;
        }
        if !wrote && matches!(b.kind, BlockKind::Heading(_) | BlockKind::Quote | BlockKind::Code { .. } | BlockKind::Rule) {
            out.appendAttributedString(&attributed(&ZWSP.to_string(), &attrs(&b.kind, level, 0, None, false)));
        }
        if i + 1 < blocks.len() || trailing {
            out.appendAttributedString(&attributed("\n", &attrs(&b.kind, level, 0, None, false)));
        }
    }
    (out, starts)
}

fn apply_attrs(st: &NSTextStorage, a: &AttrDict, r: NSRange) {
    if r.length > 0 {
        let dict: &NSDictionary<NSAttributedStringKey, AnyObject> = a;
        unsafe { st.setAttributes_range(Some(dict), r) };
    }
}

/// Rewrite every attribute of a paragraph from its semantic state (zoom, kind, level).
fn restyle(st: &NSTextStorage, p: &Para, kind: &BlockKind, level: u8) {
    if p.cs > p.start {
        apply_attrs(st, &attrs(kind, level, 0, None, true), range(p.start, p.cs - p.start));
    }
    for r in &p.runs {
        apply_attrs(st, &attrs(kind, level, r.bits, r.link.as_deref(), false), range(r.a, r.b - r.a));
    }
    if p.nl {
        apply_attrs(st, &attrs(kind, level, 0, None, false), range(p.end, 1));
    }
}

fn restyle_all() {
    let Some((tv, st)) = handles() else { return };
    let (_, paras) = read_paras(&st, true);
    let items: Vec<(BlockKind, u8)> = paras.iter().map(|p| (p.kind.clone(), p.level)).collect();
    let lay = layout_numbers(&items);
    st.beginEditing();
    for (p, (level, _)) in paras.iter().zip(lay) {
        restyle(&st, p, &p.kind, level);
    }
    st.endEditing();
    tv.setNeedsDisplay(true);
}

// ------------------------------------------------------------------ editing primitives

/// Replace a storage range with attributed text as one undoable edit.
fn replace(tv: &NSTextView, r: NSRange, with: &NSAttributedString) {
    let Some(st) = storage_of(tv) else { return };
    let plain = with.string();
    let was = PROGRAMMATIC.replace(true);
    DIRTY.set(Some((r.location, with.length())));
    if tv.shouldChangeTextInRange_replacementString(r, Some(&plain)) {
        st.replaceCharactersInRange_withAttributedString(r, with);
        tv.didChangeText();
    }
    PROGRAMMATIC.set(was);
}

/// Change attributes (not text) of a range as one undoable edit.
fn mutate(tv: &NSTextView, r: NSRange, f: impl FnOnce(&NSTextStorage)) {
    let Some(st) = storage_of(tv) else { return };
    let was = PROGRAMMATIC.replace(true);
    DIRTY.set(Some((r.location, r.length)));
    if tv.shouldChangeTextInRange_replacementString(r, None) {
        st.beginEditing();
        f(&st);
        st.endEditing();
        tv.didChangeText();
    }
    PROGRAMMATIC.set(was);
}

/// Replace whole paragraphs `lo..=hi` by blocks and put the caret in block `caret.0`, `caret.1`
/// UTF-16 units into its content.
fn replace_blocks(lo: usize, hi: usize, blocks: &[Block], caret: Option<(usize, usize)>) {
    let Some((tv, st)) = handles() else { return };
    let (_, paras) = read_paras(&st, false);
    let hi = hi.min(paras.len() - 1);
    let lo = lo.min(hi);
    // The paragraph kind lives on its terminating newline, so the range includes it.
    let nl = paras[hi].nl;
    let r = range(paras[lo].start, paras[hi].end + usize::from(nl) - paras[lo].start);
    let (text, _) = render(blocks, nl && !blocks.is_empty());
    replace(&tv, r, &text);
    if let Some((bi, off)) = caret {
        let (_, after) = read_paras(&st, false);
        let p = &after[(lo + bi).min(after.len() - 1)];
        set_caret(&tv, (p.cs + off).min(p.end), &p.kind, p.level);
    }
}

fn set_caret(tv: &NSTextView, loc: usize, kind: &BlockKind, level: u8) {
    tv.setSelectedRange(range(loc, 0));
    let a = attrs(kind, level, 0, None, false);
    let dict: &NSDictionary<NSAttributedStringKey, AnyObject> = &a;
    unsafe { tv.setTypingAttributes(dict) };
}

/// Plain text with the style of the character at `at` (or before it at the end of a paragraph).
fn styled_text(u: &[u16], paras: &[Para], at: usize, text: &str) -> Retained<NSAttributedString> {
    let p = &paras[para_at(paras, at)];
    let probe = at.min(p.end);
    let run = p.runs.iter().find(|r| r.a <= probe && probe < r.b).or_else(|| p.runs.iter().rev().find(|r| r.b <= probe));
    let _ = u;
    let (bits, link) = run.map_or((0, None), |r| (r.bits, r.link.clone()));
    let code = matches!(p.kind, BlockKind::Code { .. });
    let text = if code { text.replace('\n', &LINE_SEP.to_string()) } else { text.to_string() };
    attributed(&text, &attrs(&p.kind, p.level, bits, link.as_deref(), false))
}

/// General text replacement at block level: handles ranges that cross paragraphs, reach list
/// markers, and replacements that contain newlines. `range` is in storage coordinates.
fn edit_text(r: NSRange, repl: &str) {
    let Some((tv, st)) = handles() else { return };
    let (u, paras) = read_paras(&st, true);
    let r = range(r.location.min(u.len()), r.length.min(u.len() - r.location.min(u.len())));
    let (a, b) = (r.location, r.location + r.length);
    let (lo, hi) = (para_at(&paras, a), para_at(&paras, b));
    let repl = repl.replace("\r\n", "\n").replace('\r', "\n");
    let simple = lo == hi && a >= paras[lo].cs && (!repl.contains('\n') || matches!(paras[lo].kind, BlockKind::Code { .. }));
    if simple {
        replace(&tv, r, &styled_text(&u, &paras, a, &repl));
        let after = a + utf16_len(&repl);
        tv.setSelectedRange(range(after, 0));
        return;
    }
    let (lp, hp) = (&paras[lo], &paras[hi]);
    let a_c = a.clamp(lp.cs, lp.end);
    let b_c = b.clamp(hp.cs, hp.end);
    let left = spans_of(&u, lp, lp.cs, a_c);
    let right = spans_of(&u, hp, b_c, hp.end);
    let first_kind = if left.is_empty() && lo != hi { (hp.kind.clone(), hp.level) } else { (lp.kind.clone(), lp.level) };
    let last_kind = if lo != hi { (hp.kind.clone(), hp.level) } else { (BlockKind::Paragraph, 0) };
    let ins_style = left.last().map(|s| s.style.clone()).unwrap_or_default();
    let make = |kind: (BlockKind, u8), mut spans: Vec<Span>, tail: Vec<Span>| {
        spans.extend(tail);
        let kind = if kind.0 == BlockKind::Rule && !spans.is_empty() { (BlockKind::Paragraph, 0) } else { kind };
        Block { kind: kind.0, level: kind.1, spans }
    };
    let text_span = |t: &str| if t.is_empty() { vec![] } else { vec![Span { text: t.to_string(), style: ins_style.clone() }] };
    let code_first = matches!(first_kind.0, BlockKind::Code { .. });
    let (blocks, caret): (Vec<Block>, (usize, usize)) = if !repl.contains('\n') || code_first {
        let mut spans = left.clone();
        spans.extend(text_span(&repl));
        let caret = spans.iter().map(|s| utf16_len(&s.text)).sum();
        (vec![make(first_kind, spans, right)], (0, caret))
    } else {
        let lines: Vec<&str> = repl.split('\n').collect();
        let n = lines.len();
        let mut blocks = Vec::new();
        let mut first = left.clone();
        first.extend(text_span(lines[0]));
        blocks.push(make(first_kind, first, vec![]));
        for l in &lines[1..n - 1] {
            blocks.push(make((BlockKind::Paragraph, 0), text_span(l), vec![]));
        }
        let last_text = text_span(lines[n - 1]);
        let caret = (n - 1, last_text.iter().map(|s| utf16_len(&s.text)).sum());
        blocks.push(make(last_kind, last_text, right));
        (blocks, caret)
    };
    replace_blocks(lo, hi, &blocks, Some(caret));
}

// ------------------------------------------------------------------ public API: content

/// Replace the content with a Markdown note without firing the change callback; resets undo.
pub fn set_markdown(md: &str) {
    let Some((tv, st)) = handles() else { return };
    let doc = richtext::parse(md);
    let (text, _) = render(&doc.blocks, false);
    SUPPRESS.set(true);
    PROGRAMMATIC.set(true);
    st.setAttributedString(&text);
    if let Some(um) = tv.undoManager() {
        um.removeAllActions();
    }
    let first = doc.blocks.first();
    let cs = first.map_or(0, |b| if is_list(&b.kind) { utf16_len(&marker_text(&b.kind, b.level, 1)) } else { 0 });
    let (kind, level) = first.map_or((BlockKind::Paragraph, 0), |b| (b.kind.clone(), b.level));
    set_caret(&tv, cs.min(st.length()), &kind, level);
    PROGRAMMATIC.set(false);
    SUPPRESS.set(false);
    SLASH.with(|s| *s.borrow_mut() = None);
    EMOJI.with(|s| *s.borrow_mut() = None);
    crate::notes::refresh_title(&title());
    crate::notes::dismiss_popups();
    crate::notes::refresh_format_bar();
    tv.setNeedsDisplay(true);
}

/// The note as Markdown, serialized from the text storage by `richtext::to_markdown`.
pub fn markdown() -> String {
    handles().map(|(_, st)| richtext::to_markdown(&doc_of(&st))).unwrap_or_default()
}

/// Title for the toolbar: `Doc::title()`, "Untitled" when empty.
pub fn title() -> String {
    let t = handles().map(|(_, st)| doc_of(&st).title()).unwrap_or_default();
    if t.is_empty() {
        "Untitled".to_string()
    } else {
        t
    }
}

fn selection_bounds() -> Option<(Retained<NSTextView>, Retained<NSTextStorage>, usize, usize)> {
    let (tv, st) = handles()?;
    let r = tv.selectedRange();
    Some((tv, st, r.location, r.location + r.length))
}

/// Selected text without Markdown or list markers (lines joined with `\n`) and its range in
/// storage UTF-16 offsets. Empty text and an empty range at the cursor when nothing is selected.
pub fn selection() -> (String, usize, usize) {
    let Some((_, st, a, b)) = selection_bounds() else { return (String::new(), 0, 0) };
    if a == b {
        return (String::new(), a, a);
    }
    let (u, paras) = read_paras(&st, true);
    let lines: Vec<String> = paras
        .iter()
        .filter(|p| in_selection(p, a, b))
        .map(|p| {
            let (x, y) = (a.max(p.cs), b.min(p.end));
            if x < y && !matches!(p.kind, BlockKind::Rule) {
                clean_text(&u[x..y], matches!(p.kind, BlockKind::Code { .. }))
            } else {
                String::new()
            }
        })
        .collect();
    (lines.join("\n"), a, b)
}

/// The selection as Markdown (block kinds kept, inline styles and links serialized).
pub fn markdown_of_selection() -> String {
    let Some((_, st, a, b)) = selection_bounds() else { return String::new() };
    if a == b {
        return String::new();
    }
    let (u, paras) = read_paras(&st, true);
    let blocks = paras
        .iter()
        .filter(|p| in_selection(p, a, b))
        .map(|p| Block { kind: p.kind.clone(), level: p.level, spans: spans_of(&u, p, a.max(p.cs), b.min(p.end)) })
        .collect();
    richtext::to_markdown(&Doc { blocks })
}

/// Replace storage range `start..end` with plain text (undoable); the text takes the style at `start`.
pub fn replace_range(start: usize, end: usize, text: &str) {
    if handles().is_some() {
        edit_text(range(start.min(end), end.saturating_sub(start)), text);
    }
}

/// Insert plain text at the cursor, replacing the selection (undoable).
pub fn insert_at_cursor(text: &str) {
    if let Some((_, _, a, b)) = selection_bounds() {
        edit_text(range(a, b - a), text);
    }
}

/// The selection as storage UTF-16 offsets `(start, end)`; equal when there is none (the cursor).
pub fn selection_range() -> (usize, usize) {
    selection_bounds().map_or((0, 0), |(_, _, a, b)| (a, b))
}

/// Select storage range `start..end` (clamped to the text; `start == end` puts the cursor there).
pub fn select_range(start: usize, end: usize) {
    if let Some((tv, st)) = handles() {
        let len = st.length();
        let s = start.min(end).min(len);
        tv.setSelectedRange(range(s, end.max(start).min(len) - s));
    }
}

/// The raw storage text of `start..end` (UTF-16 offsets); `None` when the range is outside the text.
pub fn text_in_range(start: usize, end: usize) -> Option<String> {
    let (_, st) = handles()?;
    let (u, _) = read_paras(&st, false);
    (start <= end && end <= u.len()).then(|| String::from_utf16_lossy(&u[start..end]))
}

/// Blocks of a Markdown result; `None` for a blank one.
fn result_blocks(md: &str) -> Option<Vec<Block>> {
    let md = md.trim_matches(['\n', '\r']);
    if md.trim().is_empty() {
        return None;
    }
    let blocks = richtext::parse(md).blocks;
    (!blocks.is_empty()).then_some(blocks)
}

fn spans_len(spans: &[Span]) -> usize {
    spans.iter().map(|s| utf16_len(&s.text)).sum()
}

/// Replace the selection (the whole note when nothing is selected) with a Markdown result, as ONE
/// undoable edit that fires the change callback once. A single-paragraph result becomes styled
/// spans inside the paragraph it lands in (that paragraph keeps its kind); a longer one becomes
/// whole blocks, with the unselected rest of the first and last paragraph kept around it.
/// Returns false (nothing changes) for a blank result or without an editor.
pub fn replace_selection_markdown(md: &str) -> bool {
    let Some(mut blocks) = result_blocks(md) else { return false };
    let Some((tv, st)) = handles() else { return false };
    let (u, paras) = read_paras(&st, true);
    let sel = tv.selectedRange();
    let (lo, hi, a, b) = if sel.length == 0 {
        (0, paras.len() - 1, 0, u.len())
    } else {
        let (a, b) = (sel.location, sel.location + sel.length);
        let (lo, hi) = sel_paras(&paras, a, b);
        (lo, hi, a, b)
    };
    let (lp, hp) = (&paras[lo], &paras[hi]);
    let left = spans_of(&u, lp, lp.cs, a.clamp(lp.cs, lp.end));
    let right = spans_of(&u, hp, b.clamp(hp.cs, hp.end), hp.end);
    let host = |p: &Para| if p.kind == BlockKind::Rule { (BlockKind::Paragraph, 0) } else { (p.kind.clone(), p.level) };
    let one_paragraph = blocks.len() == 1 && blocks[0].kind == BlockKind::Paragraph;
    if one_paragraph {
        let (kind, level) = host(lp);
        let mut spans = left;
        spans.append(&mut blocks[0].spans);
        spans.extend(right.iter().cloned());
        blocks = vec![Block { kind, level, spans }];
    } else {
        if !left.is_empty() {
            if blocks[0].kind == BlockKind::Paragraph {
                let (kind, level) = host(lp);
                let mut spans = left;
                spans.append(&mut blocks[0].spans);
                blocks[0] = Block { kind, level, spans };
            } else {
                let (kind, level) = host(lp);
                blocks.insert(0, Block { kind, level, spans: left });
            }
        }
        if !right.is_empty() {
            let last = blocks.len() - 1;
            if blocks[last].kind == BlockKind::Paragraph {
                blocks[last].spans.extend(right.iter().cloned());
            } else {
                let (kind, level) = host(hp);
                blocks.push(Block { kind, level, spans: right.clone() });
            }
        }
    }
    let last = blocks.len() - 1;
    let caret = spans_len(&blocks[last].spans) - spans_len(&right);
    replace_blocks(lo, hi, &blocks, Some((last, caret)));
    true
}

/// Insert `new` as blocks right after paragraph `at` (Markdown separates blocks by a blank line,
/// so no empty paragraph is added). An empty paragraph at `at` is replaced by them. One undoable edit.
fn insert_blocks_after(at: usize, mut new: Vec<Block>) {
    let Some((_, st)) = handles() else { return };
    let (u, paras) = read_paras(&st, true);
    let p = &paras[at];
    let mut blocks = Vec::new();
    if p.cs != p.end {
        blocks.push(block_of(&u, p));
    }
    blocks.append(&mut new);
    let last = blocks.len() - 1;
    let caret = spans_len(&blocks[last].spans);
    replace_blocks(at, at, &blocks, Some((last, caret)));
}

/// Add Markdown blocks at the end of the note, after a blank line in its Markdown (an empty note
/// or an empty last line just gets them). One undoable edit that fires the change callback once; false for a blank result.
pub fn append_markdown(md: &str) -> bool {
    let Some(blocks) = result_blocks(md) else { return false };
    let Some((_, st)) = handles() else { return false };
    let (_, paras) = read_paras(&st, false);
    insert_blocks_after(paras.len() - 1, blocks);
    true
}

/// Insert Markdown blocks after the line that ends the selection (at the end of the note
/// without a selection). One undoable edit; false for a blank result.
pub fn insert_markdown_after_selection_line(md: &str) -> bool {
    let Some(blocks) = result_blocks(md) else { return false };
    let Some((tv, st)) = handles() else { return false };
    let (_, paras) = read_paras(&st, false);
    let sel = tv.selectedRange();
    let at = if sel.length == 0 { paras.len() - 1 } else { sel_paras(&paras, sel.location, sel.location + sel.length).1 };
    insert_blocks_after(at, blocks);
    true
}

// ------------------------------------------------------------------ normalization

fn from_units(u: &[u16]) -> String {
    String::from_utf16_lossy(u)
}

/// Bring the storage back to the invariants after any change: renumber ordered lists, clamp list
/// depth, drop stray U+200B, strip inherited marker flags, and restyle the touched paragraphs.
fn normalize(dirty: Option<(usize, usize)>) {
    let Some((tv, st)) = handles() else { return };
    // Phase 1: text edits (marker numbers, stray ZWSP), applied from the end.
    let (u, paras) = read_paras(&st, true);
    let items: Vec<(BlockKind, u8)> = paras.iter().map(|p| (p.kind.clone(), p.level)).collect();
    let lay = layout_numbers(&items);
    let mut edits: Vec<(NSRange, Option<Retained<NSAttributedString>>)> = Vec::new();
    for (p, (level, number)) in paras.iter().zip(&lay) {
        if is_list(&p.kind) {
            let want = marker_text(&p.kind, *level, *number);
            if from_units(&u[p.start..p.cs]) != want {
                edits.push((range(p.start, p.cs - p.start), Some(attributed(&want, &attrs(&p.kind, *level, 0, None, true)))));
            }
        }
        if p.kind != BlockKind::Rule && p.end - p.cs > 1 {
            for i in (p.cs..p.end).rev() {
                if u[i] == ZWSP_U16 {
                    edits.push((range(i, 1), None));
                }
            }
        }
    }
    if !edits.is_empty() {
        let sel = tv.selectedRange();
        let mut loc = sel.location;
        edits.sort_by_key(|(r, _)| std::cmp::Reverse(r.location));
        st.beginEditing();
        for (r, with) in &edits {
            let new_len = with.as_ref().map_or(0, |w| w.length());
            match with {
                Some(w) => st.replaceCharactersInRange_withAttributedString(*r, w),
                None => st.deleteCharactersInRange(*r),
            }
            let end = r.location + r.length;
            if end <= sel.location {
                loc = (loc + new_len).saturating_sub(r.length);
            } else if r.location < sel.location {
                loc = r.location + new_len;
            }
        }
        st.endEditing();
        tv.setSelectedRange(range(loc, sel.length));
    }
    // Phase 2: attributes.
    let (_, paras) = read_paras(&st, true);
    let items: Vec<(BlockKind, u8)> = paras.iter().map(|p| (p.kind.clone(), p.level)).collect();
    let lay = layout_numbers(&items);
    st.beginEditing();
    for (p, (level, _)) in paras.iter().zip(&lay) {
        let touched = dirty.is_none_or(|(l, n)| p.start <= l + n && p.end + 1 >= l);
        if touched || *level != p.level {
            restyle(&st, p, &p.kind, *level);
        }
    }
    st.endEditing();
}

/// Typed text next to a link inherits it; keep that only when typing inside the link.
fn fix_link_inheritance(typed: (usize, usize)) {
    let Some((_, st)) = handles() else { return };
    let (loc, len) = typed;
    if len == 0 || loc + len > st.length() {
        return;
    }
    let mut eff = NSRange::new(0, 0);
    let (mine, next) = KEYS.with(|k| (attr_string(&st, &k.link, loc, &mut eff), attr_string(&st, &k.link, loc + len, &mut eff)));
    if mine.is_some() && mine != next {
        KEYS.with(|k| st.removeAttribute_range(&k.link, range(loc, len)));
    }
}

/// Called by the delegate after every user-visible change of the text.
pub(crate) fn did_change() {
    if SUPPRESS.get() {
        return;
    }
    if let Some(t) = TYPED.take() {
        fix_link_inheritance(t);
    }
    normalize(DIRTY.take());
    update_triggers();
    crate::notes::refresh_title(&title());
    crate::notes::refresh_format_bar();
    crate::notes::notify_change(markdown());
    if let Some((tv, _)) = handles() {
        tv.setNeedsDisplay(true);
    }
}

// ------------------------------------------------------------------ selection

/// Keep the cursor out of list markers; Left at the start of an item goes to the previous line.
pub(crate) fn will_change_selection(old: NSRange, new: NSRange) -> NSRange {
    if PROGRAMMATIC.get() && SUPPRESS.get() {
        return new;
    }
    let Some((_, st)) = handles() else { return new };
    let (_, paras) = read_paras(&st, false);
    let Some(p) = paras.get(para_at(&paras, new.location)) else { return new };
    let marked = |p: &Para| p.cs > p.start;
    if new.length == 0 {
        if marked(p) && new.location < p.cs {
            let came_from_content_start = old.length == 0 && old.location == p.cs && new.location + 1 == p.cs && p.start > 0;
            return range(if came_from_content_start { p.start - 1 } else { p.cs }, 0);
        }
        return new;
    }
    let mut loc = new.location;
    let mut end = new.location + new.length;
    if marked(p) && loc > p.start && loc < p.cs {
        loc = p.start;
    }
    let q = &paras[para_at(&paras, end)];
    if marked(q) && end > q.start && end < q.cs {
        end = q.start.max(loc);
    }
    range(loc, end - loc)
}

pub(crate) fn did_change_selection() {
    if SUPPRESS.get() {
        return;
    }
    update_triggers();
    crate::notes::refresh_format_bar();
    // An empty last paragraph has no character to inherit its kind from.
    if let Some((tv, st)) = handles() {
        let r = tv.selectedRange();
        if r.length == 0 && r.location == st.length() {
            let (_, paras) = read_paras(&st, false);
            if let Some(p) = paras.last().filter(|p| p.start == r.location && p.start > 0) {
                let kind = if is_list(&p.kind) { BlockKind::Paragraph } else { p.kind.clone() };
                let a = attrs(&kind, 0, 0, None, false);
                let dict: &NSDictionary<NSAttributedStringKey, AnyObject> = &a;
                unsafe { tv.setTypingAttributes(dict) };
            }
        }
    }
}

// ------------------------------------------------------------------ typing hooks

/// `textView:shouldChangeTextInRange:replacementString:`. Returns false when the edit was
/// handled here (block shortcut, inline autoformat, block-level edit).
pub(crate) fn should_change(r: NSRange, repl: Option<&NSString>) -> bool {
    if PROGRAMMATIC.get() || SUPPRESS.get() {
        return true;
    }
    let Some(repl) = repl.map(|s| s.to_string()) else { return true };
    let Some((tv, st)) = handles() else { return true };
    if tv.hasMarkedText() {
        return true;
    }
    // Undo and redo replay stored attributed strings; they must not be reinterpreted.
    if tv.undoManager().is_some_and(|um| um.isUndoing() || um.isRedoing()) {
        return true;
    }
    let (u, paras) = read_paras(&st, true);
    let (a, b) = (r.location, r.location + r.length);
    if a > u.len() || b > u.len() {
        return true;
    }
    let (lo, hi) = (para_at(&paras, a), para_at(&paras, b));
    let p = &paras[lo];
    let crosses = lo != hi || (r.length > 0 && a < p.cs);
    if repl == "\n" && r.length == 0 {
        enter_key();
        return false;
    }
    if crosses
        || (repl.contains(['\n', '\r']) && !matches!(p.kind, BlockKind::Code { .. }))
        || (repl.contains(['\n', '\r']) && r.length > 0)
    {
        edit_text(r, &repl);
        return false;
    }
    if repl.contains(['\n', '\r']) {
        edit_text(r, &repl);
        return false;
    }
    if r.length == 0 && repl.chars().count() == 1 && a >= p.cs && a <= p.end {
        let typed = repl.chars().next().unwrap_or(' ');
        let before = clean_text(&u[p.cs..a], false);
        if p.kind == BlockKind::Paragraph {
            let candidate = format!("{before}{typed}");
            if let Some((kind, _)) = richtext::block_shortcut(&candidate) {
                let rest = spans_of(&u, p, a, p.end);
                if kind != BlockKind::Rule || rest.is_empty() {
                    apply_block_shortcut(lo, kind, rest, lo + 1 == paras.len());
                    return false;
                }
            }
        }
        if matches!(typed, '*' | '_' | '~' | '`') && !matches!(p.kind, BlockKind::Code { .. } | BlockKind::Rule) {
            let candidate = format!("{before}{typed}");
            if let Some(edit) = richtext::inline_autoformat(&candidate) {
                apply_inline_autoformat(&u, p, a, &before, &edit);
                return false;
            }
        }
    }
    TYPED.set(Some((a, utf16_len(&repl))));
    DIRTY.set(Some((a, utf16_len(&repl))));
    true
}

fn apply_block_shortcut(index: usize, kind: BlockKind, rest: Vec<Span>, is_last: bool) {
    let mut blocks = vec![Block { kind: kind.clone(), level: 0, spans: rest }];
    let mut caret = Some((0, 0));
    if kind == BlockKind::Rule {
        if is_last {
            blocks.push(para(vec![]));
            caret = Some((1, 0));
        } else {
            caret = None;
        }
    }
    replace_blocks(index, index, &blocks, caret);
    if caret.is_none() {
        // Cursor into the paragraph after the rule.
        if let Some((tv, st)) = handles() {
            let (_, paras) = read_paras(&st, false);
            if let Some(p) = paras.get(index + 1) {
                set_caret(&tv, p.cs, &p.kind, p.level);
            }
        }
    }
}

fn apply_inline_autoformat(u: &[u16], p: &Para, cursor: usize, before: &str, edit: &richtext::InlineEdit) {
    let Some((tv, _)) = handles() else { return };
    let to_u16 = |chars: usize| -> usize { before.chars().take(chars).map(char::len_utf16).sum() };
    let marker_chars = (edit.end - edit.start - edit.inner.chars().count()) / 2;
    let open = p.cs + to_u16(edit.start);
    let inner_a = p.cs + to_u16(edit.start + marker_chars);
    let inner_b = p.cs + to_u16((edit.end - marker_chars).min(before.chars().count()));
    let bits = style_bits(&edit.style);
    let piece = NSMutableAttributedString::new();
    for r in p.runs.iter().filter(|r| r.a < inner_b && r.b > inner_a) {
        let (x, y) = (r.a.max(inner_a), r.b.min(inner_b));
        piece.appendAttributedString(&attributed(
            &clean_text(&u[x..y], false),
            &attrs(&p.kind, p.level, r.bits | bits, r.link.as_deref(), false),
        ));
    }
    if piece.length() == 0 {
        piece.appendAttributedString(&attributed(&edit.inner, &attrs(&p.kind, p.level, bits, None, false)));
    }
    let len = piece.length();
    replace(&tv, range(open, cursor - open), &piece);
    tv.setSelectedRange(range(open + len, 0));
    let a = attrs(&p.kind, p.level, 0, None, false);
    let dict: &NSDictionary<NSAttributedStringKey, AnyObject> = &a;
    unsafe { tv.setTypingAttributes(dict) };
}

/// Enter with an empty selection (selections are replaced by `edit_text` first).
fn enter_key() {
    let Some((tv, st)) = handles() else { return };
    let (u, paras) = read_paras(&st, true);
    let loc = tv.selectedRange().location.min(u.len());
    let k = para_at(&paras, loc);
    let p = &paras[k];
    let block = block_of(&u, p);
    let content_len: usize = block.spans.iter().map(|s| utf16_len(&s.text)).sum();
    let off = loc.saturating_sub(p.cs).min(content_len);
    let (left, right) = split_spans(&block.spans, off);
    match &block.kind {
        BlockKind::Code { .. } => {
            let text = block_text(&block);
            if text.is_empty() {
                replace_blocks(k, k, &[para(vec![])], Some((0, 0)));
            } else if off == content_len && text.ends_with('\n') {
                let mut code = block.clone();
                let trimmed = text.trim_end_matches('\n').to_string();
                code.spans = if trimmed.is_empty() { vec![] } else { vec![Span { text: trimmed, style: Style::default() }] };
                replace_blocks(k, k, &[code, para(vec![])], Some((1, 0)));
            } else {
                edit_text(range(loc, 0), "\n");
            }
        }
        BlockKind::Rule => replace_blocks(k, k, &[block.clone(), para(vec![])], Some((1, 0))),
        BlockKind::Heading(_) => {
            let blocks = [Block { kind: block.kind.clone(), level: 0, spans: left }, para(right)];
            replace_blocks(k, k, &blocks, Some((1, 0)));
        }
        _ => match richtext::list_continue(&block) {
            ListEnter::Continue { kind, level } => {
                let level = if is_list(&kind) { level } else { 0 };
                let blocks = [Block { kind: block.kind.clone(), level: block.level, spans: left }, Block { kind, level, spans: right }];
                replace_blocks(k, k, &blocks, Some((1, 0)));
            }
            ListEnter::Outdent { level } => {
                let b = Block { kind: block.kind.clone(), level, spans: vec![] };
                replace_blocks(k, k, &[b], Some((0, 0)));
            }
            ListEnter::Exit => replace_blocks(k, k, &[para(vec![])], Some((0, 0))),
            ListEnter::NotAList => {
                let blocks = [Block { kind: BlockKind::Paragraph, level: 0, spans: left }, para(right)];
                replace_blocks(k, k, &blocks, Some((1, 0)));
            }
        },
    }
}

fn max_level_for(paras: &[Para], k: usize) -> u8 {
    if k > 0 && is_list(&paras[k - 1].kind) {
        (paras[k - 1].level + 1).min(MAX_LEVEL)
    } else {
        0
    }
}

fn indent_item(delta: i8) -> bool {
    let Some((tv, st)) = handles() else { return false };
    let (u, paras) = read_paras(&st, true);
    let sel = tv.selectedRange();
    let (lo, hi) = sel_paras(&paras, sel.location, sel.location + sel.length);
    if !paras[lo..=hi].iter().all(|p| is_list(&p.kind)) {
        return false;
    }
    let loc = sel.location;
    let off = loc.saturating_sub(paras[lo].cs);
    let mut blocks = Vec::new();
    for k in lo..=hi {
        let mut b = block_of(&u, &paras[k]);
        let cap = if k == lo { max_level_for(&paras, k) } else { MAX_LEVEL };
        b.level = richtext::indent(b.level, delta).min(cap);
        blocks.push(b);
    }
    replace_blocks(lo, hi, &blocks, Some((0, off)));
    true
}

/// `textView:doCommandBySelector:`.
pub(crate) fn do_command(cmd: Sel) -> bool {
    if PROGRAMMATIC.get() {
        return false;
    }
    let Some((tv, st)) = handles() else { return false };
    let sel = tv.selectedRange();
    if cmd == sel!(insertNewline:) || cmd == sel!(insertNewlineIgnoringFieldEditor:) || cmd == sel!(insertLineBreak:) {
        if sel.length > 0 {
            edit_text(sel, "");
        }
        enter_key();
        return true;
    }
    if cmd == sel!(insertTab:) {
        return indent_item(1);
    }
    if cmd == sel!(insertBacktab:) {
        return indent_item(-1) || {
            let (_, paras) = read_paras(&st, false);
            paras.get(para_at(&paras, sel.location)).is_some_and(|p| is_list(&p.kind))
        };
    }
    if cmd == sel!(deleteBackward:) && sel.length == 0 {
        return backspace(&tv, &st, sel.location);
    }
    false
}

fn backspace(_tv: &NSTextView, st: &NSTextStorage, loc: usize) -> bool {
    let (u, paras) = read_paras(st, true);
    let k = para_at(&paras, loc);
    let p = &paras[k];
    let block = block_of(&u, p);
    match &p.kind {
        BlockKind::Rule => {
            replace_blocks(k, k, &[para(vec![])], Some((0, 0)));
            true
        }
        BlockKind::Paragraph => {
            if loc != p.start || k == 0 {
                return false;
            }
            let prev = &paras[k - 1];
            let mut merged = block_of(&u, prev);
            if prev.kind == BlockKind::Rule {
                replace_blocks(k - 1, k, &[block], Some((0, 0)));
                return true;
            }
            let off: usize = merged.spans.iter().map(|s| utf16_len(&s.text)).sum();
            merged.spans.extend(block.spans);
            replace_blocks(k - 1, k, &[merged], Some((0, off)));
            true
        }
        kind => {
            if loc != p.cs {
                return false;
            }
            let _ = kind;
            let blocks = if matches!(block.kind, BlockKind::Code { .. }) {
                toggle_block(&[block], Format::CodeBlock)
            } else {
                vec![para(block.spans)]
            };
            replace_blocks(k, k, &blocks, Some((0, 0)));
            true
        }
    }
}

// ------------------------------------------------------------------ formats

const INLINE_FORMATS: [(Format, u8); 5] =
    [(Format::Bold, BOLD), (Format::Italic, ITALIC), (Format::Strikethrough, STRIKE), (Format::Underline, UNDERLINE), (Format::Code, CODE)];

fn inline_runs(paras: &[Para], a: usize, b: usize) -> Vec<(usize, usize, usize, Run)> {
    let (lo, hi) = sel_paras(paras, a, b);
    let mut out = Vec::new();
    for (k, p) in paras.iter().enumerate().take(hi + 1).skip(lo) {
        if matches!(p.kind, BlockKind::Code { .. } | BlockKind::Rule) {
            continue;
        }
        for r in &p.runs {
            let (x, y) = (r.a.max(a), r.b.min(b));
            if x < y {
                out.push((k, x, y, r.clone()));
            }
        }
    }
    out
}

fn typing_bits(tv: &NSTextView) -> u8 {
    let d = tv.typingAttributes();
    KEYS.with(|k| d.objectForKey(&k.style)).and_then(|v| v.downcast_ref::<NSNumber>().map(|n| n.as_isize() as u8)).unwrap_or(0)
}

fn toggle_inline(bit: u8) {
    let Some((tv, st, a, b)) = selection_bounds() else { return };
    let (_, paras) = read_paras(&st, true);
    if a == b {
        let p = &paras[para_at(&paras, a)];
        if matches!(p.kind, BlockKind::Code { .. } | BlockKind::Rule) {
            return;
        }
        let bits = typing_bits(&tv) ^ bit;
        let at = attrs(&p.kind, p.level, bits, None, false);
        let dict: &NSDictionary<NSAttributedStringKey, AnyObject> = &at;
        unsafe { tv.setTypingAttributes(dict) };
        return;
    }
    let runs = inline_runs(&paras, a, b);
    if runs.is_empty() {
        return;
    }
    let all = runs.iter().all(|(_, _, _, r)| r.bits & bit != 0);
    mutate(&tv, range(a, b - a), |st| {
        for (k, x, y, r) in &runs {
            let p = &paras[*k];
            let bits = if all { r.bits & !bit } else { r.bits | bit };
            apply_attrs(st, &attrs(&p.kind, p.level, bits, r.link.as_deref(), false), range(*x, y - x));
        }
    });
}

/// Apply `url` (or remove the link when `None`) on `r`.
fn set_link_on(r: NSRange, url: Option<&str>) {
    let Some((tv, st)) = handles() else { return };
    let (_, paras) = read_paras(&st, true);
    let runs = inline_runs(&paras, r.location, r.location + r.length);
    if runs.is_empty() {
        return;
    }
    mutate(&tv, r, |st| {
        for (k, x, y, run) in &runs {
            let p = &paras[*k];
            apply_attrs(st, &attrs(&p.kind, p.level, run.bits, url, false), range(*x, y - x));
        }
    });
}

fn apply_block_format(f: Format) {
    let Some((tv, st, a, b)) = selection_bounds() else { return };
    let (u, paras) = read_paras(&st, true);
    let (lo, hi) = sel_paras(&paras, a, b);
    let blocks: Vec<Block> = paras[lo..=hi].iter().map(|p| block_of(&u, p)).collect();
    let off = a.saturating_sub(paras[lo].cs);
    let out = toggle_block(&blocks, f);
    let rule = f == Format::Rule;
    let caret = if rule { (out.len() - 1, 0) } else { (0, off) };
    let sel_all = a != b && !rule;
    replace_blocks(lo, hi, &out, Some(caret));
    if sel_all {
        let (_, after) = read_paras(&st, false);
        let end = (lo + out.len() - 1).min(after.len() - 1);
        tv.setSelectedRange(range(after[lo].cs, after[end].end - after[lo].cs));
    }
}

/// Run one formatting action at the cursor / selection (toggles on and off). The key shortcuts,
/// the `/` menu, the Format Bar and the action panel all call this.
pub fn apply_format(f: Format) {
    match f {
        Format::Bold | Format::Italic | Format::Strikethrough | Format::Underline | Format::Code => {
            if let Some((_, bit)) = INLINE_FORMATS.iter().find(|(x, _)| *x == f) {
                toggle_inline(*bit);
            }
        }
        Format::Link => open_link_prompt(),
        _ => apply_block_format(f),
    }
    crate::notes::refresh_format_bar();
}

/// Formats active at the cursor or on the whole selection, in Format Bar order.
pub fn active_formats() -> Vec<Format> {
    let Some((tv, st, a, b)) = selection_bounds() else { return vec![] };
    let (_, paras) = read_paras(&st, true);
    let (lo, hi) = sel_paras(&paras, a, b);
    let mut on = Vec::new();
    if a == b {
        let bits = typing_bits(&tv);
        for (f, bit) in INLINE_FORMATS {
            if bits & bit != 0 {
                on.push(f);
            }
        }
        if link_at_cursor().is_some() {
            on.push(Format::Link);
        }
    } else {
        let runs = inline_runs(&paras, a, b);
        if !runs.is_empty() {
            for (f, bit) in INLINE_FORMATS {
                if runs.iter().all(|(_, _, _, r)| r.bits & bit != 0) {
                    on.push(f);
                }
            }
            if runs.iter().all(|(_, _, _, r)| r.link.is_some()) {
                on.push(Format::Link);
            }
        }
    }
    let kinds: Vec<&BlockKind> = paras[lo..=hi].iter().map(|p| &p.kind).collect();
    let all = |f: &dyn Fn(&BlockKind) -> bool| kinds.iter().all(|k| f(k));
    for n in 1..=3u8 {
        if all(&|k| *k == BlockKind::Heading(n)) {
            on.push(Format::Heading(n));
        }
    }
    if all(&|k| *k == BlockKind::Quote) {
        on.push(Format::Quote);
    }
    if all(&|k| *k == BlockKind::Bullet) {
        on.push(Format::BulletList);
    }
    if all(&|k| *k == BlockKind::Ordered) {
        on.push(Format::OrderedList);
    }
    if all(&|k| matches!(k, BlockKind::Task { .. })) {
        on.push(Format::TaskList);
    }
    if all(&|k| matches!(k, BlockKind::Code { .. })) {
        on.push(Format::CodeBlock);
    }
    Format::ALL.iter().copied().filter(|f| on.contains(f)).collect()
}

fn toggle_task_para(k: usize) {
    let Some((tv, st)) = handles() else { return };
    let (u, paras) = read_paras(&st, true);
    let Some(p) = paras.get(k) else { return };
    let BlockKind::Task { checked } = p.kind else { return };
    let mut b = block_of(&u, p);
    b.kind = BlockKind::Task { checked: !checked };
    let sel = tv.selectedRange();
    replace_blocks(k, k, &[b], None);
    tv.setSelectedRange(sel);
}

/// ⌘Enter: toggle the task on the cursor line. False when the line is not a task.
pub fn toggle_task_at_cursor() -> bool {
    let Some((_, st, a, _)) = selection_bounds() else { return false };
    let (_, paras) = read_paras(&st, false);
    let k = para_at(&paras, a);
    if !matches!(paras[k].kind, BlockKind::Task { .. }) {
        return false;
    }
    toggle_task_para(k);
    true
}

// ------------------------------------------------------------------ `/` menu and emoji

pub fn set_on_slash(cb: Box<dyn Fn(SlashEvent)>) {
    ON_SLASH.with(|c| *c.borrow_mut() = Some(Rc::from(cb)));
}

pub fn set_on_emoji(cb: Box<dyn Fn(EmojiEvent)>) {
    ON_EMOJI.with(|c| *c.borrow_mut() = Some(Rc::from(cb)));
}

/// Pop-up navigation keys (Up, Down, Enter, Tab, Esc) go here first while a pop-up is open;
/// return true to consume the key.
pub fn set_popup_key_handler(cb: Box<dyn Fn(PopupKey) -> bool>) {
    ON_POPUP_KEY.with(|c| *c.borrow_mut() = Some(Rc::from(cb)));
}

fn caret_anchor(tv: &NSTextView, loc: usize) -> Anchor {
    let r: NSRect = unsafe { tv.firstRectForCharacterRange_actualRange(range(loc, 0), ptr::null_mut()) };
    Anchor { x: r.origin.x, y: r.origin.y, w: r.size.width, h: r.size.height }
}

fn fire_slash(e: SlashEvent) {
    if let Some(cb) = ON_SLASH.with(|c| c.borrow().clone()) {
        cb(e);
    }
}

fn fire_emoji(e: EmojiEvent) {
    if let Some(cb) = ON_EMOJI.with(|c| c.borrow().clone()) {
        cb(e);
    }
}

fn update_triggers() {
    let Some((tv, st)) = handles() else { return };
    let sel = tv.selectedRange();
    let mut slash: Option<Session> = None;
    let mut emoji: Option<Session> = None;
    if sel.length == 0 {
        let (u, paras) = read_paras(&st, false);
        let p = &paras[para_at(&paras, sel.location)];
        if sel.location >= p.cs && !matches!(p.kind, BlockKind::Code { .. } | BlockKind::Rule) {
            let before = clean_text(&u[p.cs..sel.location], false);
            let content = clean_text(&u[p.cs..p.end], false);
            slash = slash_query(&content, &before).map(|query| Session { loc: p.cs, query });
            if slash.is_none() {
                emoji = emoji_query(&before).map(|query| Session { loc: sel.location - utf16_len(&query) - 1, query });
            }
        }
    }
    if slash.as_ref().map(|s| s.loc) != SLASH_DISMISSED.get() {
        SLASH_DISMISSED.set(None);
    }
    if emoji.as_ref().map(|s| s.loc) != EMOJI_DISMISSED.get() {
        EMOJI_DISMISSED.set(None);
    }
    if slash.as_ref().map(|s| s.loc) == SLASH_DISMISSED.get() && slash.is_some() {
        slash = None;
    }
    if emoji.as_ref().map(|s| s.loc) == EMOJI_DISMISSED.get() && emoji.is_some() {
        emoji = None;
    }
    let old = SLASH.with(|s| s.borrow().clone());
    if old != slash {
        SLASH.with(|s| *s.borrow_mut() = slash.clone());
        match (&old, &slash) {
            (_, Some(n)) if old.as_ref().is_none_or(|o| o.loc != n.loc) => {
                if old.is_some() {
                    fire_slash(SlashEvent::Close);
                }
                fire_slash(SlashEvent::Open { anchor: caret_anchor(&tv, n.loc) });
                fire_slash(SlashEvent::Query(n.query.clone()));
            }
            (_, Some(n)) => fire_slash(SlashEvent::Query(n.query.clone())),
            (Some(_), None) => fire_slash(SlashEvent::Close),
            _ => {}
        }
    }
    let old = EMOJI.with(|s| s.borrow().clone());
    if old != emoji {
        EMOJI.with(|s| *s.borrow_mut() = emoji.clone());
        match (&old, &emoji) {
            (_, Some(n)) if old.as_ref().is_none_or(|o| o.loc != n.loc) => {
                if old.is_some() {
                    fire_emoji(EmojiEvent::Close);
                }
                fire_emoji(EmojiEvent::Open { anchor: caret_anchor(&tv, n.loc) });
                fire_emoji(EmojiEvent::Query(n.query.clone()));
            }
            (_, Some(n)) => fire_emoji(EmojiEvent::Query(n.query.clone())),
            (Some(_), None) => fire_emoji(EmojiEvent::Close),
            _ => {}
        }
    }
}

/// End the open `/` and `:` sessions without events (the popups were hidden by the window).
pub(crate) fn end_trigger_sessions() {
    SLASH.with(|s| *s.borrow_mut() = None);
    EMOJI.with(|s| *s.borrow_mut() = None);
}

/// Remove the typed `/query` and apply `f` to the (now empty) line. Ends the session silently.
pub fn slash_commit(f: Format) {
    let Some(s) = SLASH.with(|s| s.borrow_mut().take()) else { return };
    edit_text(range(s.loc, 1 + utf16_len(&s.query)), "");
    apply_format(f);
}

/// Replace the typed `:query` with `emoji`. Ends the session silently.
pub fn emoji_commit(emoji: &str) {
    let Some(s) = EMOJI.with(|s| s.borrow_mut().take()) else { return };
    edit_text(range(s.loc, 1 + utf16_len(&s.query)), emoji);
}

/// Route Up/Down/Enter/Tab/Esc to an open pop-up. True when the key was consumed.
pub(crate) fn handle_popup_key(event: &NSEvent) -> bool {
    let slash = SLASH.with(|s| s.borrow().clone());
    let emoji = EMOJI.with(|s| s.borrow().clone());
    if slash.is_none() && emoji.is_none() {
        return false;
    }
    let mods = event.modifierFlags() & (NSEventModifierFlags::Command | NSEventModifierFlags::Option | NSEventModifierFlags::Control);
    if !mods.is_empty() {
        return false;
    }
    let key = match event.keyCode() {
        126 => PopupKey::Up,
        125 => PopupKey::Down,
        36 | 76 => PopupKey::Enter,
        48 => PopupKey::Tab,
        53 => PopupKey::Esc,
        _ => return false,
    };
    if let Some(cb) = ON_POPUP_KEY.with(|c| c.borrow().clone()) {
        if cb(key) {
            return true;
        }
    }
    if key == PopupKey::Esc {
        if let Some(s) = SLASH.with(|s| s.borrow_mut().take()) {
            SLASH_DISMISSED.set(Some(s.loc));
            fire_slash(SlashEvent::Close);
        }
        if let Some(s) = EMOJI.with(|s| s.borrow_mut().take()) {
            EMOJI_DISMISSED.set(Some(s.loc));
            fire_emoji(EmojiEvent::Close);
        }
        return true;
    }
    false
}

// ------------------------------------------------------------------ zoom

pub fn zoom() -> f64 {
    ZOOM.get()
}

/// Set the zoom (clamped to 0.5..=3.0), restyle the text and fire the zoom callback on change.
pub fn set_zoom(z: f64) {
    let z = ((z.clamp(ZOOM_MIN, ZOOM_MAX)) * 100.0).round() / 100.0;
    if (z - ZOOM.get()).abs() < 1e-9 {
        return;
    }
    ZOOM.set(z);
    restyle_all();
    if let Some(cb) = ON_ZOOM.with(|c| c.borrow().clone()) {
        cb(z);
    }
}

pub fn set_on_zoom_change(cb: Box<dyn Fn(f64)>) {
    ON_ZOOM.with(|c| *c.borrow_mut() = Some(Rc::from(cb)));
}

// ------------------------------------------------------------------ links

fn link_bounds(paras: &[Para], loc: usize) -> Option<(usize, usize, String)> {
    let p = &paras[para_at(paras, loc)];
    let hit = p
        .runs
        .iter()
        .find(|r| r.link.is_some() && r.a <= loc && loc < r.b)
        .or_else(|| p.runs.iter().find(|r| r.link.is_some() && r.b == loc))?;
    let url = hit.link.clone()?;
    let mut a = hit.a;
    let mut b = hit.b;
    for r in &p.runs {
        if r.link.as_deref() == Some(&url) && r.b == a {
            a = r.a;
        }
    }
    for r in &p.runs {
        if r.link.as_deref() == Some(&url) && r.a == b {
            b = r.b;
        }
    }
    Some((a, b, url))
}

/// URL of the link under the cursor (or the one the cursor touches), if any.
pub fn link_at_cursor() -> Option<String> {
    let (_, st, a, _) = selection_bounds()?;
    let (_, paras) = read_paras(&st, true);
    link_bounds(&paras, a).map(|(_, _, u)| u)
}

fn open_url(url: &str) {
    let with_scheme = if url.contains("://") || url.starts_with("mailto:") { url.to_string() } else { format!("https://{url}") };
    if let Some(u) = NSURL::URLWithString(&NSString::from_str(&with_scheme)) {
        NSWorkspace::sharedWorkspace().openURL(&u);
    }
}

define_class!(
    /// Target of the link prompt field (Enter confirms, Esc cancels).
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastNotesLinkTarget"]
    struct LinkTarget;

    impl LinkTarget {
        #[unsafe(method(linkConfirm:))]
        fn link_confirm(&self, _sender: Option<&AnyObject>) {
            if let Some(url) = LINK.with(|l| l.borrow().as_ref().map(|p| p.field.stringValue().to_string())) {
                link_prompt_submit(&url);
            }
        }
    }

    unsafe impl NSObjectProtocol for LinkTarget {}

    unsafe impl NSControlTextEditingDelegate for LinkTarget {
        #[unsafe(method(control:textView:doCommandBySelector:))]
        unsafe fn control_text_view_do_command(&self, _control: &NSControl, _tv: &NSTextView, command: Sel) -> bool {
            let cancel = command == sel!(cancelOperation:);
            if cancel {
                link_prompt_cancel();
            }
            cancel
        }
    }

    unsafe impl NSTextFieldDelegate for LinkTarget {}
);

fn open_link_prompt() {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let Some((tv, st, a, b)) = selection_bounds() else { return };
    link_prompt_cancel();
    let (_, paras) = read_paras(&st, true);
    let (target, prefill) = if a == b {
        match link_bounds(&paras, a) {
            Some((x, y, url)) => (range(x, y - x), url),
            None => (range(a, 0), String::new()),
        }
    } else {
        let runs = inline_runs(&paras, a, b);
        let first =
            runs.first().and_then(|(_, _, _, r)| r.link.clone()).filter(|l| runs.iter().all(|(_, _, _, r)| r.link.as_deref() == Some(l)));
        (range(a, b - a), first.unwrap_or_default())
    };
    let handler: Retained<LinkTarget> = unsafe { msg_send![mtm.alloc::<LinkTarget>(), init] };
    let field = NSTextField::textFieldWithString(&NSString::from_str(&prefill), mtm);
    field.setPlaceholderString(Some(&NSString::from_str("https://")));
    field.setFrame(NSRect::new(NSPoint::new(10.0, 10.0), NSSize::new(260.0, 24.0)));
    unsafe {
        field.setTarget(Some(&handler));
        field.setAction(Some(sel!(linkConfirm:)));
    }
    let delegate: &ProtocolObject<dyn NSTextFieldDelegate> = ProtocolObject::from_ref(&*handler);
    unsafe { field.setDelegate(Some(delegate)) };
    let content = NSView::initWithFrame(mtm.alloc::<NSView>(), NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(280.0, 44.0)));
    content.addSubview(&field);
    let vc = NSViewController::new(mtm);
    vc.setView(&content);
    let popover = NSPopover::new(mtm);
    popover.setContentViewController(Some(&vc));
    popover.setBehavior(NSPopoverBehavior::Transient);
    let caret = caret_anchor(&tv, a);
    let mut rect = NSRect::new(NSPoint::new(caret.x, caret.y), NSSize::new(caret.w.max(1.0), caret.h.max(16.0)));
    if let Some(w) = tv.window() {
        rect = w.convertRectFromScreen(rect);
        rect = tv.convertRect_fromView(rect, None);
    }
    popover.showRelativeToRect_ofView_preferredEdge(rect, &tv, NSRectEdge::MaxY);
    if let Some(w) = field.window() {
        let _ = w.makeFirstResponder(Some(&field));
    }
    LINK.with(|l| *l.borrow_mut() = Some(LinkPrompt { popover, field, range: target, _target: handler }));
}

pub fn link_prompt_open() -> bool {
    LINK.with(|l| l.borrow().is_some())
}

/// Text currently in the link prompt field.
pub fn link_prompt_value() -> Option<String> {
    LINK.with(|l| l.borrow().as_ref().map(|p| p.field.stringValue().to_string()))
}

/// Confirm the link prompt with `url` (what Enter does): empty removes the link.
pub fn link_prompt_submit(url: &str) {
    let Some(p) = LINK.with(|l| l.borrow_mut().take()) else { return };
    p.popover.close();
    crate::notes::focus_editor();
    let url = url.trim();
    if p.range.length == 0 {
        if !url.is_empty() {
            let at = p.range.location;
            edit_text(p.range, url);
            set_link_on(range(at, utf16_len(url)), Some(url));
            if let Some((tv, _)) = handles() {
                tv.setSelectedRange(range(at + utf16_len(url), 0));
            }
        }
    } else {
        set_link_on(p.range, (!url.is_empty()).then_some(url));
    }
}

/// Close the link prompt without changing anything (what Esc does).
pub fn link_prompt_cancel() {
    if let Some(p) = LINK.with(|l| l.borrow_mut().take()) {
        p.popover.close();
        crate::notes::focus_editor();
    }
}

// ------------------------------------------------------------------ key shortcuts

/// Editor shortcuts of S7, S8 and S14. Call only when the editor is the first responder.
pub(crate) fn handle_key(event: &NSEvent) -> bool {
    if event.r#type() != NSEventType::KeyDown {
        return false;
    }
    let m = event.modifierFlags()
        & (NSEventModifierFlags::Command | NSEventModifierFlags::Shift | NSEventModifierFlags::Option | NSEventModifierFlags::Control);
    let cmd = NSEventModifierFlags::Command;
    let (shift, opt) = (NSEventModifierFlags::Shift, NSEventModifierFlags::Option);
    if !m.contains(cmd) || m.contains(NSEventModifierFlags::Control) {
        return false;
    }
    let key = event.charactersIgnoringModifiers().map(|s| s.to_string().to_lowercase()).unwrap_or_default();
    let code = event.keyCode();
    let only = |extra: NSEventModifierFlags| m == cmd | extra;
    let fmt = if only(NSEventModifierFlags::empty()) {
        match key.as_str() {
            "b" => Some(Format::Bold),
            "i" => Some(Format::Italic),
            "u" => Some(Format::Underline),
            "e" => Some(Format::Code),
            "l" => Some(Format::Link),
            _ => None,
        }
    } else if only(shift) {
        match (key.as_str(), code) {
            ("s", _) => Some(Format::Strikethrough),
            ("b", _) => Some(Format::Quote),
            (_, 26) => Some(Format::OrderedList),
            (_, 28) => Some(Format::BulletList),
            (_, 25) => Some(Format::TaskList),
            _ => None,
        }
    } else if only(opt) {
        match (key.as_str(), code) {
            (_, 18) => Some(Format::Heading(1)),
            (_, 19) => Some(Format::Heading(2)),
            (_, 20) => Some(Format::Heading(3)),
            ("c", _) => Some(Format::CodeBlock),
            _ => None,
        }
    } else {
        None
    };
    if let Some(f) = fmt {
        apply_format(f);
        return true;
    }
    if only(NSEventModifierFlags::empty()) && (code == 36 || code == 76) {
        toggle_task_at_cursor();
        return true;
    }
    let plain = only(NSEventModifierFlags::empty()) || only(shift);
    if plain && matches!(key.as_str(), "=" | "+") {
        set_zoom(step_zoom(zoom(), 1));
        return true;
    }
    if plain && key == "-" {
        set_zoom(step_zoom(zoom(), -1));
        return true;
    }
    if only(NSEventModifierFlags::empty()) && key == "0" {
        set_zoom(1.0);
        return true;
    }
    false
}

// ------------------------------------------------------------------ text view

/// Storage position under a point of the text view (view coordinates), with the glyph rect.
fn hit_char(tv: &NSTextView, point: NSPoint) -> Option<(usize, NSRect)> {
    let lm = layout_of(tv)?;
    let tc = container_of(tv)?;
    let o = tv.textContainerOrigin();
    let pt = NSPoint::new(point.x - o.x, point.y - o.y);
    let idx = unsafe { lm.characterIndexForPoint_inTextContainer_fractionOfDistanceBetweenInsertionPoints(pt, &tc, ptr::null_mut()) };
    let st = storage_of(tv)?;
    if idx >= st.length() {
        return None;
    }
    let g = lm.glyphIndexForCharacterAtIndex(idx);
    let line = unsafe { lm.lineFragmentRectForGlyphAtIndex_effectiveRange(g, ptr::null_mut()) };
    if pt.y < line.origin.y || pt.y > line.origin.y + line.size.height {
        return None;
    }
    let glyph = lm.boundingRectForGlyphRange_inTextContainer(range(g, 1), &tc);
    Some((idx, glyph))
}

fn handle_click(tv: &NSTextView, event: &NSEvent) -> bool {
    if event.r#type() != NSEventType::LeftMouseDown {
        return false;
    }
    let point = tv.convertPoint_fromView(event.locationInWindow(), None);
    let Some((idx, glyph)) = hit_char(tv, point) else { return false };
    let Some(st) = storage_of(tv) else { return false };
    let (_, paras) = read_paras(&st, true);
    let k = para_at(&paras, idx);
    let p = &paras[k];
    if matches!(p.kind, BlockKind::Task { .. }) && idx < p.cs {
        toggle_task_para(k);
        return true;
    }
    if event.modifierFlags().contains(NSEventModifierFlags::Command) {
        let o = tv.textContainerOrigin();
        let inside = point.x >= glyph.origin.x + o.x && point.x <= glyph.origin.x + o.x + glyph.size.width;
        if let (true, Some((_, _, url))) = (inside, link_bounds(&paras, idx)) {
            open_url(&url);
            return true;
        }
    }
    false
}

fn paste_plain(tv: &NSTextView) {
    let pb = NSPasteboard::generalPasteboard();
    if let Some(s) = pb.stringForType(unsafe { NSPasteboardTypeString }) {
        unsafe {
            let _: () = msg_send![tv, insertText: &*s as &AnyObject];
        }
    }
}

/// One visual line of a character range: x extent and the baseline, in text-container
/// coordinates.
struct LinePiece {
    x: f64,
    w: f64,
    baseline: f64,
}

/// The line pieces of `chars` (a range inside one paragraph), one per laid-out line.
fn line_pieces(lm: &NSLayoutManager, tc: &NSTextContainer, chars: NSRange) -> Vec<LinePiece> {
    let glyphs = unsafe { lm.glyphRangeForCharacterRange_actualCharacterRange(chars, ptr::null_mut()) };
    let end = glyphs.location + glyphs.length;
    let mut out = Vec::new();
    let mut g = glyphs.location;
    while g < end {
        let mut eff = NSRange::new(0, 0);
        let frag = unsafe { lm.lineFragmentRectForGlyphAtIndex_effectiveRange(g, &mut eff as *mut NSRange) };
        let upto = end.min(eff.location + eff.length).max(g + 1);
        let r = lm.boundingRectForGlyphRange_inTextContainer(range(g, upto - g), tc);
        let loc = lm.locationForGlyphAtIndex(g);
        out.push(LinePiece { x: r.origin.x, w: r.size.width, baseline: frag.origin.y + loc.y });
        g = upto;
    }
    out
}

fn fill_rounded(r: NSRect, radius: f64) {
    NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(r, radius, radius).fill();
}

/// A task checkbox (S4): `Sub` outline with radius 4 when open; a filled `Accent` box with the
/// accent ink (#1a0709, D4) check mark when done. Same footprint as the glyph it covers.
fn draw_checkbox(x: f64, baseline: f64, checked: bool, z: f64) {
    let side = 14.0 * z;
    let r = NSRect::new(NSPoint::new(x + 1.0 * z, baseline - 5.2 * z - side / 2.0), NSSize::new(side, side));
    if checked {
        notes_theme::color(Token::Accent).setFill();
        fill_rounded(r, 4.0 * z);
        let ink = NSColor::colorWithSRGBRed_green_blue_alpha(26.0 / 255.0, 7.0 / 255.0, 9.0 / 255.0, 1.0);
        ink.setStroke();
        let check = NSBezierPath::bezierPath();
        let at = |fx: f64, fy: f64| NSPoint::new(r.origin.x + fx * side, r.origin.y + fy * side);
        check.moveToPoint(at(0.26, 0.52));
        check.lineToPoint(at(0.43, 0.69));
        check.lineToPoint(at(0.75, 0.31));
        check.setLineWidth(1.9 * z);
        check.setLineCapStyle(objc2_app_kit::NSLineCapStyle::Round);
        check.setLineJoinStyle(objc2_app_kit::NSLineJoinStyle::Round);
        check.stroke();
    } else {
        let w = 1.4 * z;
        notes_theme::color(Token::Sub).setStroke();
        let inset = NSRect::new(NSPoint::new(r.origin.x + w / 2.0, r.origin.y + w / 2.0), NSSize::new(side - w, side - w));
        let path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(inset, 4.0 * z, 4.0 * z);
        path.setLineWidth(w);
        path.stroke();
    }
}

fn draw_decorations(tv: &NSTextView) {
    let (Some(st), Some(lm), Some(tc)) = (storage_of(tv), layout_of(tv), container_of(tv)) else { return };
    let (_, paras) = read_paras(&st, true);
    let o = tv.textContainerOrigin();
    let width = tc.containerSize().width;
    let z = zoom();
    for p in &paras {
        if p.end <= p.start && !p.nl {
            continue;
        }
        if let BlockKind::Task { checked } = p.kind {
            if p.cs > p.start {
                if let Some(l) = line_pieces(&lm, &tc, range(p.start, 1)).first() {
                    draw_checkbox(l.x + o.x, l.baseline + o.y, checked, z);
                }
            }
        }
        // inline code: a `Field` pill with radius 5 behind the mono text
        if !matches!(p.kind, BlockKind::Code { .. } | BlockKind::Rule) {
            let f = match p.kind {
                BlockKind::Heading(1) => 24.0 / BASE_SIZE,
                BlockKind::Heading(2) => 20.0 / BASE_SIZE,
                BlockKind::Heading(_) => 17.0 / BASE_SIZE,
                _ => 1.0,
            };
            for run in p.runs.iter().filter(|r| r.bits & CODE != 0 && r.b > r.a) {
                for l in line_pieces(&lm, &tc, range(run.a, run.b - run.a)) {
                    notes_theme::color(Token::Field).setFill();
                    let r = NSRect::new(
                        NSPoint::new(l.x + o.x - 1.5 * z, l.baseline + o.y - 11.5 * z * f),
                        NSSize::new(l.w + 3.0 * z, 15.5 * z * f),
                    );
                    fill_rounded(r, 5.0 * z);
                }
            }
        }
    }
    for (k, p) in paras.iter().enumerate() {
        let wants = matches!(p.kind, BlockKind::Code { .. } | BlockKind::Rule | BlockKind::Quote);
        let total = p.end + usize::from(p.nl) - p.start;
        if !wants || total == 0 {
            continue;
        }
        let glyphs = unsafe { lm.glyphRangeForCharacterRange_actualCharacterRange(range(p.start, total), ptr::null_mut()) };
        if glyphs.length == 0 {
            continue;
        }
        let first = unsafe { lm.lineFragmentRectForGlyphAtIndex_effectiveRange(glyphs.location, ptr::null_mut()) };
        let last = unsafe { lm.lineFragmentRectForGlyphAtIndex_effectiveRange(glyphs.location + glyphs.length - 1, ptr::null_mut()) };
        let (mut top, mut bottom) = (first.origin.y + o.y, last.origin.y + last.size.height + o.y);
        if matches!(p.kind, BlockKind::Code { .. }) && k > 0 {
            // the first line fragment includes `paragraphSpacingBefore`: start the fill below it
            top += CODE_SPACE_BEFORE * z;
        }
        if p.kind == BlockKind::Quote {
            // one bar per run of quote lines
            if k > 0 && paras[k - 1].kind == BlockKind::Quote {
                continue;
            }
            let mut j = k;
            while paras.get(j + 1).is_some_and(|n| n.kind == BlockKind::Quote) {
                j += 1;
            }
            let (e, nl) = (&paras[j], usize::from(paras[j].nl));
            let g = unsafe { lm.glyphRangeForCharacterRange_actualCharacterRange(range(e.start, e.end + nl - e.start), ptr::null_mut()) };
            if g.length > 0 {
                let l = unsafe { lm.lineFragmentRectForGlyphAtIndex_effectiveRange(g.location + g.length - 1, ptr::null_mut()) };
                bottom = l.origin.y + l.size.height + o.y;
            }
        }
        match &p.kind {
            BlockKind::Code { .. } => {
                notes_theme::color(Token::Field).setFill();
                let r = NSRect::new(NSPoint::new(o.x + 1.0, top - 2.0), NSSize::new((width - 2.0).max(0.0), bottom - top + 4.0));
                fill_rounded(r, 10.0 * z);
            }
            BlockKind::Rule => {
                notes_theme::color(Token::Sep).setFill();
                let mid = (top + bottom) / 2.0;
                NSBezierPath::fillRect(NSRect::new(NSPoint::new(o.x + 5.0, mid - 0.5), NSSize::new((width - 10.0).max(0.0), 1.0)));
            }
            _ => {
                notes_theme::color(Token::Accent).setFill();
                fill_rounded(NSRect::new(NSPoint::new(o.x + 5.0 + 2.0 * z, top), NSSize::new(2.5 * z, bottom - top)), 1.25 * z);
            }
        }
    }
}

define_class!(
    /// The editor view: task checkbox / link clicks, a decorations pass (quote bars, code
    /// backgrounds, rules) and plain-text-only pasteboard traffic.
    #[unsafe(super(NSTextView))]
    #[thread_kind = MainThreadOnly]
    #[name = "FerrocastNotesTextView"]
    pub struct NotesTextView;

    impl NotesTextView {
        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            if !handle_click(self, event) {
                unsafe {
                    let _: () = msg_send![super(self), mouseDown: event];
                }
            }
        }

        // Right click: the standard text menu, then the window commands.
        #[unsafe(method(menuForEvent:))]
        fn menu_for_event(&self, event: &NSEvent) -> *mut NSMenu {
            let mtm = MainThreadMarker::from(self);
            let base: Option<Retained<NSMenu>> = unsafe { msg_send![super(self), menuForEvent: event] };
            let menu = base.unwrap_or_else(|| NSMenu::new(mtm));
            crate::notes::append_context_items(&menu, mtm);
            Retained::autorelease_ptr(menu)
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, dirty: NSRect) {
            draw_decorations(self);
            unsafe {
                let _: () = msg_send![super(self), drawRect: dirty];
            }
        }

        #[unsafe(method(paste:))]
        fn paste(&self, _sender: Option<&AnyObject>) {
            paste_plain(self);
        }

        #[unsafe(method(pasteAsRichText:))]
        fn paste_as_rich_text(&self, _sender: Option<&AnyObject>) {
            paste_plain(self);
        }

        #[unsafe(method(writablePasteboardTypes))]
        fn writable_pasteboard_types(&self) -> *mut NSObject {
            Retained::autorelease_ptr(Retained::into_super(NSArray::from_slice(&[unsafe { NSPasteboardTypeString }])))
        }

        #[unsafe(method(writeSelectionToPasteboard:type:))]
        fn write_selection(&self, pb: &NSPasteboard, _type: &NSPasteboardType) -> bool {
            let (text, _, _) = selection();
            pb.setString_forType(&NSString::from_str(&text), unsafe { NSPasteboardTypeString })
        }
    }
);

/// Create the editor view (called by the window shell).
pub(crate) fn new_text_view(mtm: MainThreadMarker, frame: NSRect) -> Retained<NSTextView> {
    let tv: Retained<NotesTextView> = unsafe { msg_send![mtm.alloc::<NotesTextView>(), initWithFrame: frame] };
    let tv: Retained<NSTextView> = Retained::into_super(tv);
    // Use the classic layout manager from the start (the editor reads glyph geometry).
    let _ = layout_of(&tv);
    tv.setSmartInsertDeleteEnabled(false);
    tv.setImportsGraphics(false);
    tv.setUsesFontPanel(false);
    // S4: the coral accent for the caret and the selection (28% alpha, text colour unchanged).
    tv.setInsertionPointColor(Some(&notes_theme::color(Token::Accent)));
    let sel: Retained<NSDictionary<NSAttributedStringKey, AnyObject>> = {
        let d: Retained<AttrDict> = NSMutableDictionary::new();
        put(
            &d,
            unsafe { objc2_app_kit::NSBackgroundColorAttributeName },
            &notes_theme::color(Token::Accent).colorWithAlphaComponent(SELECTION_ALPHA),
        );
        Retained::into_super(d)
    };
    unsafe { tv.setSelectedTextAttributes(&sel) };
    tv
}

/// True when the editor is the key window's first responder.
pub(crate) fn is_editor(responder: Option<&objc2_app_kit::NSResponder>, tv: &NSTextView) -> bool {
    responder.is_some_and(|r| ptr::eq(r as *const _ as *const u8, tv as *const _ as *const u8))
}

// ------------------------------------------------------------------ geometry for checks

/// Window-coordinate center of the glyph at storage position `loc`, for synthesized clicks.
pub fn window_point_for_char(loc: usize) -> Option<(f64, f64)> {
    let (tv, _) = handles()?;
    let lm = layout_of(&tv)?;
    let tc = container_of(&tv)?;
    let g = lm.glyphIndexForCharacterAtIndex(loc);
    let r = lm.boundingRectForGlyphRange_inTextContainer(range(g, 1), &tc);
    let o = tv.textContainerOrigin();
    let p = tv.convertPoint_toView(NSPoint::new(r.origin.x + o.x + r.size.width / 2.0, r.origin.y + o.y + r.size.height / 2.0), None);
    Some((p.x, p.y))
}

/// Storage start of the n-th paragraph (for checks).
pub fn paragraph_start(n: usize) -> Option<usize> {
    let (_, st) = handles()?;
    let (_, paras) = read_paras(&st, false);
    paras.get(n).map(|p| p.start)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ferrocast_core::richtext::{parse, Style};

    fn blocks(md: &str) -> Vec<Block> {
        parse(md).blocks
    }
    fn kinds(b: &[Block]) -> Vec<BlockKind> {
        b.iter().map(|b| b.kind.clone()).collect()
    }
    fn span(t: &str, bold: bool) -> Span {
        Span { text: t.into(), style: Style { bold, ..Style::default() } }
    }

    #[test]
    fn zoom_steps_by_ten_percent_and_clamps() {
        assert_eq!(step_zoom(1.0, 1), 1.1);
        assert_eq!(step_zoom(1.0, -1), 0.9);
        assert_eq!(step_zoom(0.5, -1), 0.5);
        assert_eq!(step_zoom(3.0, 1), 3.0);
        assert_eq!(step_zoom(2.95, 1), 3.0);
        assert_eq!(step_zoom(0.55, -1), 0.5);
    }

    #[test]
    fn numbering_follows_to_markdown_rules() {
        let k = |ks: &[(BlockKind, u8)]| layout_numbers(ks).into_iter().collect::<Vec<_>>();
        let o = BlockKind::Ordered;
        assert_eq!(
            k(&[(o.clone(), 0), (o.clone(), 0), (o.clone(), 1), (o.clone(), 1), (o.clone(), 0)]),
            vec![(0, 1), (0, 2), (1, 1), (1, 2), (0, 3)]
        );
        // first item cannot be indented, depth is at most previous + 1
        assert_eq!(k(&[(o.clone(), 3), (o.clone(), 3)]), vec![(0, 1), (1, 1)]);
        // a paragraph restarts the numbering
        assert_eq!(k(&[(o.clone(), 0), (BlockKind::Paragraph, 0), (o.clone(), 0)]), vec![(0, 1), (0, 0), (0, 1)]);
        // a bullet at the same level restarts it too
        assert_eq!(k(&[(o.clone(), 0), (BlockKind::Bullet, 0), (o, 0)]), vec![(0, 1), (0, 0), (0, 1)]);
    }

    #[test]
    fn heading_toggles_on_and_off() {
        let b = blocks("hello");
        let h = toggle_block(&b, Format::Heading(2));
        assert_eq!(kinds(&h), vec![BlockKind::Heading(2)]);
        assert_eq!(kinds(&toggle_block(&h, Format::Heading(2))), vec![BlockKind::Paragraph]);
        assert_eq!(kinds(&toggle_block(&h, Format::Heading(1))), vec![BlockKind::Heading(1)]);
    }

    #[test]
    fn list_toggle_keeps_level_and_switches_kind() {
        let b = blocks("- a\n  - b");
        let o = toggle_block(&b, Format::OrderedList);
        assert_eq!(kinds(&o), vec![BlockKind::Ordered, BlockKind::Ordered]);
        assert_eq!(o[1].level, 1);
        let back = toggle_block(&o, Format::OrderedList);
        assert_eq!(kinds(&back), vec![BlockKind::Paragraph, BlockKind::Paragraph]);
        let t = toggle_block(&b, Format::TaskList);
        assert_eq!(kinds(&t), vec![BlockKind::Task { checked: false }; 2]);
    }

    #[test]
    fn mixed_selection_applies_instead_of_removing() {
        let b = blocks("# a\n\nb");
        let q = toggle_block(&b, Format::Quote);
        assert_eq!(kinds(&q), vec![BlockKind::Quote, BlockKind::Quote]);
    }

    #[test]
    fn code_block_merges_lines_and_flattens_back() {
        let b = blocks("one\n\ntwo");
        let c = toggle_block(&b, Format::CodeBlock);
        assert_eq!(c.len(), 1);
        assert!(matches!(c[0].kind, BlockKind::Code { .. }));
        assert_eq!(c[0].spans.iter().map(|s| s.text.as_str()).collect::<String>(), "one\ntwo");
        let back = toggle_block(&c, Format::CodeBlock);
        assert_eq!(kinds(&back), vec![BlockKind::Paragraph, BlockKind::Paragraph]);
        // a heading inside a code block flattens first
        assert_eq!(kinds(&toggle_block(&c, Format::Heading(1))), vec![BlockKind::Heading(1), BlockKind::Heading(1)]);
    }

    #[test]
    fn rule_adds_a_trailing_paragraph() {
        let b = blocks("text");
        let r = toggle_block(&b, Format::Rule);
        assert_eq!(kinds(&r), vec![BlockKind::Paragraph, BlockKind::Rule, BlockKind::Paragraph]);
        let empty = vec![Block { kind: BlockKind::Paragraph, level: 0, spans: vec![] }];
        assert_eq!(kinds(&toggle_block(&empty, Format::Rule)), vec![BlockKind::Rule, BlockKind::Paragraph]);
    }

    #[test]
    fn split_spans_keeps_styles_and_handles_bounds() {
        let s = vec![span("ab", false), span("cd", true)];
        let (l, r) = split_spans(&s, 3);
        assert_eq!(l, vec![span("ab", false), span("c", true)]);
        assert_eq!(r, vec![span("d", true)]);
        let (l, r) = split_spans(&s, 0);
        assert!(l.is_empty());
        assert_eq!(r, s);
        let (l, r) = split_spans(&s, 99);
        assert_eq!(l, s);
        assert!(r.is_empty());
        let e = vec![span("😀x", false)];
        let (l, r) = split_spans(&e, 2);
        assert_eq!((l[0].text.as_str(), r[0].text.as_str()), ("😀", "x"));
    }

    #[test]
    fn slash_only_on_an_otherwise_empty_line() {
        assert_eq!(slash_query("/", "/"), Some(String::new()));
        assert_eq!(slash_query("/hea", "/hea"), Some("hea".into()));
        assert_eq!(slash_query("/hea there", "/hea there"), None);
        assert_eq!(slash_query("a/b", "a/b"), None);
        assert_eq!(slash_query("/he", "/h"), None, "cursor must be at the end");
        assert_eq!(slash_query("hello", "hello"), None);
    }

    #[test]
    fn emoji_needs_a_word_boundary() {
        assert_eq!(emoji_query(":"), Some(String::new()));
        assert_eq!(emoji_query("hi :smi"), Some("smi".into()));
        assert_eq!(emoji_query("10:30"), None);
        assert_eq!(emoji_query("hi :smi le"), None);
        assert_eq!(emoji_query("hello"), None);
        assert_eq!(emoji_query("a :+1"), Some("+1".into()));
    }
}
