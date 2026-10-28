//! Rich-text document model for Notes.
//!
//! A [`Doc`] is a flat list of [`Block`]s, each holding styled [`Span`]s. A native
//! text editor converts to and from it, and Markdown is the storage format
//! ([`parse`] and [`to_markdown`]). Underline has no Markdown syntax, so it is
//! stored as `<u>text</u>`. The module also holds the pure "typing helpers" the
//! editor needs (block shortcuts, inline autoformat, list continuation, indent).
//! Everything here is pure and never panics on any input.

/// Deepest list indent level the editor and the parser allow.
pub const MAX_LEVEL: u8 = 4;

/// What a block is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockKind {
    /// Plain text.
    Paragraph,
    /// Heading with level 1 to 3.
    Heading(u8),
    /// Block quote line (`> text`).
    Quote,
    /// Bullet list item (`- text`).
    Bullet,
    /// Ordered list item (`1. text`); the number is implicit.
    Ordered,
    /// Task list item (`- [ ] text` / `- [x] text`).
    Task { checked: bool },
    /// Fenced code block; the single span holds the verbatim contents.
    Code { lang: String },
    /// Horizontal rule (`---`).
    Rule,
}

/// Inline formatting of a span.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub underline: bool,
    pub code: bool,
    /// Link target, when the span is a link.
    pub link: Option<String>,
    /// Image source (a relative path such as `attachments/<hash>.png`), when the span is an image;
    /// the span text is then its alt text and the span is atomic.
    pub image: Option<String>,
}

/// A run of text with one style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub style: Style,
}

/// One line-level element of the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub kind: BlockKind,
    /// List indent, 0-based (only meaningful for list kinds).
    pub level: u8,
    pub spans: Vec<Span>,
}

/// A whole note.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Doc {
    pub blocks: Vec<Block>,
}

/// Result of the inline autoformat check, in `char` offsets of the checked string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineEdit {
    /// Start of the whole marked run (opening marker included).
    pub start: usize,
    /// End of the whole marked run (closing marker included).
    pub end: usize,
    /// The text between the markers.
    pub inner: String,
    /// The style to apply to `inner`.
    pub style: Style,
}

/// What the editor does on Enter in a list, task or quote line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListEnter {
    /// Start a new block of this kind and level (tasks start unchecked).
    Continue { kind: BlockKind, level: u8 },
    /// Empty indented item: move it one level out, keeping its kind.
    Outdent { level: u8 },
    /// Empty top-level item: turn the line into a Paragraph.
    Exit,
    /// Not a list-like block; Enter behaves normally.
    NotAList,
}

/// Line that stands for an empty paragraph in Markdown.
const BR: &str = "<br>";
const MAX_DEPTH: usize = 24;
const WINDOW: usize = 1024;
const ESCAPABLE: &str = "\\`*_~[]()<>#-+.!|";

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parse Markdown into a [`Doc`].
///
/// Never panics. CRLF is treated as LF, blank lines separate nothing (every
/// non-blank line becomes one block), unmatched inline markers stay literal.
pub fn parse(md: &str) -> Doc {
    let text = md.replace("\r\n", "\n");
    let mut lines: Vec<&str> = text.split('\n').collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    let mut blocks = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        i += 1;
        if line.trim().is_empty() {
            continue;
        }
        if let Some((fence, len, lang)) = fence_open(line) {
            stack.clear();
            let mut body = Vec::new();
            while i < lines.len() {
                let l = lines[i];
                i += 1;
                if is_fence_close(l, fence, len) {
                    break;
                }
                body.push(l);
            }
            let content = body.join("\n");
            let spans = if content.is_empty() { vec![] } else { vec![plain_span(content)] };
            blocks.push(Block { kind: BlockKind::Code { lang }, level: 0, spans });
            continue;
        }
        if line.trim() == BR {
            stack.clear();
            blocks.push(Block { kind: BlockKind::Paragraph, level: 0, spans: vec![] });
            continue;
        }
        match parse_line(line) {
            Some((kind, width, rest)) => {
                let level = if is_list(&kind) {
                    assign_level(&mut stack, width)
                } else {
                    stack.clear();
                    0
                };
                let spans = if kind == BlockKind::Rule { vec![] } else { parse_inline(&rest) };
                blocks.push(Block { kind, level, spans });
            }
            None => {
                stack.clear();
                blocks.push(Block { kind: BlockKind::Paragraph, level: 0, spans: parse_inline(line.trim()) });
            }
        }
    }
    Doc { blocks }
}

fn plain_span(text: String) -> Span {
    Span { text, style: Style::default() }
}

fn is_list(kind: &BlockKind) -> bool {
    matches!(kind, BlockKind::Bullet | BlockKind::Ordered | BlockKind::Task { .. })
}

fn assign_level(stack: &mut Vec<usize>, width: usize) -> u8 {
    if stack.is_empty() {
        stack.push(width);
        return 0;
    }
    while stack.len() > 1 && stack.last().is_some_and(|&top| top > width) {
        stack.pop();
    }
    if stack.last().is_some_and(|&top| top < width) {
        stack.push(width);
    }
    (stack.len() - 1).min(MAX_LEVEL as usize) as u8
}

fn run_len(ch: &[char], i: usize, c: char) -> usize {
    ch[i..].iter().take_while(|&&x| x == c).count()
}

/// Opening code fence: (fence char, run length, language).
fn fence_open(line: &str) -> Option<(char, usize, String)> {
    let t = line.trim();
    let c = t.chars().next()?;
    if c != '`' && c != '~' {
        return None;
    }
    let run = t.chars().take_while(|&x| x == c).count();
    if run < 3 {
        return None;
    }
    let info = t[run..].trim();
    if c == '`' && info.contains('`') {
        return None;
    }
    Some((c, run, info.to_string()))
}

fn is_fence_close(line: &str, fence: char, len: usize) -> bool {
    let t = line.trim();
    t.len() >= len && t.chars().all(|x| x == fence)
}

/// `[ ]`, `[]`, `[x]` or `[X]` at the start of `s`, followed by a space or the end.
fn task_prefix(s: &str) -> Option<(bool, &str)> {
    let (checked, rest) = if let Some(r) = s.strip_prefix("[ ]") {
        (false, r)
    } else if let Some(r) = s.strip_prefix("[]") {
        (false, r)
    } else if let Some(r) = s.strip_prefix("[x]").or_else(|| s.strip_prefix("[X]")) {
        (true, r)
    } else {
        return None;
    };
    if rest.is_empty() || rest.starts_with(char::is_whitespace) {
        Some((checked, rest.trim()))
    } else {
        None
    }
}

/// Classify a non-fence line: (kind, indent width, inline source). `None` means paragraph.
fn parse_line(line: &str) -> Option<(BlockKind, usize, String)> {
    let mut width = 0;
    let mut start = line.len();
    for (idx, c) in line.char_indices() {
        match c {
            ' ' => width += 1,
            '\t' => width += 4,
            _ => {
                start = idx;
                break;
            }
        }
    }
    let t = line[start..].trim_end();
    let first = t.chars().next()?;
    if t.chars().count() >= 3 && (first == '-' || first == '_') && t.chars().all(|c| c == first) {
        return Some((BlockKind::Rule, width, String::new()));
    }
    if first == '#' {
        let hashes = t.chars().take_while(|&c| c == '#').count();
        let rest = &t[hashes..];
        if hashes <= 3 && rest.starts_with([' ', '\t']) && !rest.trim().is_empty() {
            return Some((BlockKind::Heading(hashes as u8), width, rest.trim().to_string()));
        }
        return None;
    }
    if first == '>' {
        return Some((BlockKind::Quote, width, t[1..].trim().to_string()));
    }
    if (first == '-' || first == '*') && (t.len() == 1 || t[1..].starts_with([' ', '\t'])) {
        let rest = t[1..].trim();
        return Some(match task_prefix(rest) {
            Some((checked, r)) => (BlockKind::Task { checked }, width, r.to_string()),
            None => (BlockKind::Bullet, width, rest.to_string()),
        });
    }
    if first.is_ascii_digit() {
        let digits = t.chars().take_while(char::is_ascii_digit).count();
        let rest = &t[digits..];
        if digits <= 9 && rest.starts_with('.') && (rest.len() == 1 || rest[1..].starts_with([' ', '\t'])) {
            return Some((BlockKind::Ordered, width, rest[1..].trim().to_string()));
        }
        return None;
    }
    if first == '[' {
        if let Some((checked, r)) = task_prefix(t) {
            return Some((BlockKind::Task { checked }, width, r.to_string()));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Inline parsing
// ---------------------------------------------------------------------------

fn parse_inline(text: &str) -> Vec<Span> {
    let ch: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    inline(&ch, &Style::default(), &mut out, 0);
    merge(out)
}

fn merge(spans: Vec<Span>) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::new();
    for s in spans {
        if s.text.is_empty() {
            continue;
        }
        match out.last_mut() {
            Some(last) if last.style == s.style => last.text.push_str(&s.text),
            _ => out.push(s),
        }
    }
    out
}

/// Skip one unit during scanning: an escape pair, a whole code span, or one char.
fn skip(ch: &[char], j: usize) -> usize {
    match ch[j] {
        '\\' if j + 1 < ch.len() => j + 2,
        '`' => {
            let r = run_len(ch, j, '`');
            match find_code_close(ch, j + r, r) {
                Some(close) => close + r,
                None => j + r,
            }
        }
        _ => j + 1,
    }
}

fn scan(ch: &[char], from: usize, mut pred: impl FnMut(usize) -> Option<usize>) -> Option<usize> {
    let end = ch.len().min(from.saturating_add(WINDOW));
    let mut j = from;
    while j < end {
        if let Some(r) = pred(j) {
            return Some(r);
        }
        j = skip(ch, j);
    }
    None
}

/// Start index of the closing backtick run of length `run`, searching from `from`.
fn find_code_close(ch: &[char], from: usize, run: usize) -> Option<usize> {
    let end = ch.len().min(from.saturating_add(WINDOW));
    let mut j = from;
    while j < end {
        if ch[j] == '`' {
            let r = run_len(ch, j, '`');
            if r == run {
                return Some(j);
            }
            j += r;
        } else {
            j += 1;
        }
    }
    None
}

fn count_singles(ch: &[char], from: usize, to: usize, c: char) -> usize {
    let mut k = from;
    let mut count = 0;
    while k < to {
        if ch[k] == '\\' {
            k += 2;
        } else if ch[k] == c {
            let r = run_len(ch, k, c);
            if r == 1 {
                count += 1;
            }
            k += r;
        } else {
            k += 1;
        }
    }
    count
}

fn flush(buf: &mut String, base: &Style, out: &mut Vec<Span>) {
    if !buf.is_empty() {
        out.push(Span { text: std::mem::take(buf), style: base.clone() });
    }
}

fn inline(ch: &[char], base: &Style, out: &mut Vec<Span>, depth: usize) {
    let n = ch.len();
    let mut buf = String::new();
    let mut i = 0;
    while i < n {
        let c = ch[i];
        if c == '\\' && i + 1 < n && ESCAPABLE.contains(ch[i + 1]) {
            buf.push(ch[i + 1]);
            i += 2;
        } else if c == '`' {
            let run = run_len(ch, i, '`');
            if let Some(close) = find_code_close(ch, i + run, run) {
                let mut content: String = ch[i + run..close].iter().collect();
                if content.len() >= 2 && content.starts_with(' ') && content.ends_with(' ') && !content.chars().all(|x| x == ' ') {
                    content = content[1..content.len() - 1].to_string();
                }
                flush(&mut buf, base, out);
                let mut style = base.clone();
                style.code = true;
                out.push(Span { text: content, style });
                i = close + run;
            } else {
                buf.extend(std::iter::repeat_n('`', run));
                i += run;
            }
        } else if c == '<' && depth < MAX_DEPTH && ch[i..].starts_with(&['<', 'u', '>']) {
            let close = scan(ch, i + 3, |j| ch[j..].starts_with(&['<', '/', 'u', '>']).then_some(j));
            match close {
                Some(close) if close > i + 3 => {
                    flush(&mut buf, base, out);
                    let mut style = base.clone();
                    style.underline = true;
                    inline(&ch[i + 3..close], &style, out, depth + 1);
                    i = close + 4;
                }
                _ => {
                    buf.push('<');
                    i += 1;
                }
            }
        } else if c == '!' && depth < MAX_DEPTH && base.link.is_none() && ch.get(i + 1) == Some(&'[') {
            match try_link(ch, i + 1, true) {
                Some((close, url, next)) => {
                    flush(&mut buf, base, out);
                    let mut style = base.clone();
                    style.image = Some(url);
                    // spans are never empty, so an image without alt text gets a generic one
                    let alt = unescape_alt(&ch[i + 2..close]);
                    out.push(Span { text: if alt.is_empty() { "image".into() } else { alt }, style });
                    i = next;
                }
                None => {
                    buf.push('!');
                    i += 1;
                }
            }
        } else if c == '[' && depth < MAX_DEPTH && base.link.is_none() {
            match try_link(ch, i, false) {
                Some((close, url, next)) => {
                    flush(&mut buf, base, out);
                    let mut style = base.clone();
                    style.link = Some(url);
                    inline(&ch[i + 1..close], &style, out, depth + 1);
                    i = next;
                }
                None => {
                    buf.push('[');
                    i += 1;
                }
            }
        } else if matches!(c, '*' | '_' | '~') && depth < MAX_DEPTH {
            let run = run_len(ch, i, c);
            let cands: &[usize] = match (c, run) {
                ('~', 1) => &[],
                ('~', _) => &[2],
                (_, 1) => &[1],
                _ => &[2, 1],
            };
            let mut matched = None;
            for &m in cands {
                if ch.get(i + m).is_none_or(|x| x.is_whitespace()) {
                    continue;
                }
                if c == '_' && i > 0 && ch[i - 1].is_alphanumeric() {
                    continue;
                }
                if let Some(pos) = find_emph_close(ch, i, m, c) {
                    matched = Some((m, pos));
                    break;
                }
            }
            match matched {
                Some((m, pos)) => {
                    flush(&mut buf, base, out);
                    let mut style = base.clone();
                    match (c, m) {
                        ('~', _) => style.strike = true,
                        (_, 2) => style.bold = true,
                        _ => style.italic = true,
                    }
                    inline(&ch[i + m..pos], &style, out, depth + 1);
                    i = pos + m;
                }
                None => {
                    buf.extend(std::iter::repeat_n(c, run));
                    i += run;
                }
            }
        } else {
            buf.push(c);
            i += 1;
        }
    }
    flush(&mut buf, base, out);
}

/// Index of the closing marker for an opener of `m` chars of `c` at `i`.
fn find_emph_close(ch: &[char], i: usize, m: usize, c: char) -> Option<usize> {
    let start = i + m;
    let mut consumed = start;
    scan(ch, start, |j| {
        if j < consumed || ch[j] != c {
            return None;
        }
        let r = run_len(ch, j, c);
        consumed = j + r;
        if j <= start || ch[j - 1].is_whitespace() {
            return None;
        }
        let pos = if m == 2 {
            if r < 2 {
                return None;
            }
            if r >= 3 && count_singles(ch, start, j, c) % 2 == 1 {
                j + 1
            } else {
                j
            }
        } else {
            if r == 2 {
                return None;
            }
            j
        };
        if pos <= start || (c == '_' && ch.get(pos + m).is_some_and(|x| x.is_alphanumeric())) {
            return None;
        }
        Some(pos)
    })
}

/// Alt text of an image: the raw text with backslash escapes resolved.
fn unescape_alt(ch: &[char]) -> String {
    let mut out = String::new();
    let mut k = 0;
    while k < ch.len() {
        if ch[k] == '\\' && k + 1 < ch.len() && ESCAPABLE.contains(ch[k + 1]) {
            k += 1;
        }
        out.push(ch[k]);
        k += 1;
    }
    out
}

/// Parse `[text](url)` at `i`: (index of `]`, url, index after `)`). `empty_ok` allows `[]` (images).
fn try_link(ch: &[char], i: usize, empty_ok: bool) -> Option<(usize, String, usize)> {
    let mut depth = 1usize;
    let close = scan(ch, i + 1, |j| match ch[j] {
        '[' => {
            depth += 1;
            None
        }
        ']' => {
            depth -= 1;
            (depth == 0).then_some(j)
        }
        _ => None,
    })?;
    if (close == i + 1 && !empty_ok) || ch.get(close + 1) != Some(&'(') {
        return None;
    }
    let mut k = close + 2;
    let end = ch.len().min(k + WINDOW);
    let mut parens = 0usize;
    let mut url = String::new();
    while k < end {
        match ch[k] {
            '\\' if k + 1 < ch.len() && matches!(ch[k + 1], '\\' | '(' | ')') => {
                url.push(ch[k + 1]);
                k += 2;
            }
            '(' => {
                parens += 1;
                url.push('(');
                k += 1;
            }
            ')' if parens == 0 => {
                let url = url.trim().to_string();
                return if url.is_empty() { None } else { Some((close, url, k + 1)) };
            }
            ')' => {
                parens -= 1;
                url.push(')');
                k += 1;
            }
            other => {
                url.push(other);
                k += 1;
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Serialization
// ---------------------------------------------------------------------------

/// Serialize a [`Doc`] to canonical Markdown.
///
/// One blank line separates blocks, except between consecutive quote lines and
/// between list items (a blank line is only used when a bullet/task item and an
/// ordered item meet at the same level). Ordered items are renumbered per run
/// and level, lists use two spaces per level, and the trailing newline is
/// omitted. An empty paragraph is written as a `<br>` line and parsed back as an
/// empty paragraph; empty headings are dropped, while empty quote and list items
/// are kept as bare markers (`>`, `-`, `1.`, `- [ ]`).
pub fn to_markdown(doc: &Doc) -> String {
    let mut out = String::new();
    let mut prev: Option<(BlockKind, u8)> = None;
    let mut counters: Vec<usize> = Vec::new();
    for b in &doc.blocks {
        let level = match (&prev, is_list(&b.kind)) {
            (_, false) => 0,
            (Some((k, l)), true) if is_list(k) => b.level.min(MAX_LEVEL).min(l + 1),
            _ => 0,
        };
        let pad = "  ".repeat(level as usize);
        let chunk = match &b.kind {
            BlockKind::Paragraph => {
                let t = inline_md(&b.spans);
                if t.is_empty() {
                    BR.to_string()
                } else {
                    protect_paragraph(t)
                }
            }
            BlockKind::Heading(n) => {
                let t = inline_md(&b.spans);
                if t.is_empty() {
                    continue;
                }
                format!("{} {t}", "#".repeat((*n).clamp(1, 3) as usize))
            }
            BlockKind::Quote => with_prefix(">", &inline_md(&b.spans)),
            BlockKind::Bullet => with_prefix(&format!("{pad}-"), &inline_md(&b.spans)),
            BlockKind::Task { checked } => {
                let mark = if *checked { "[x]" } else { "[ ]" };
                with_prefix(&format!("{pad}- {mark}"), &inline_md(&b.spans))
            }
            BlockKind::Ordered => {
                counters.truncate(level as usize + 1);
                while counters.len() <= level as usize {
                    counters.push(0);
                }
                counters[level as usize] += 1;
                with_prefix(&format!("{pad}{}.", counters[level as usize]), &inline_md(&b.spans))
            }
            BlockKind::Rule => "---".to_string(),
            BlockKind::Code { lang } => code_block(lang, &b.spans),
        };
        if is_list(&b.kind) {
            if b.kind != BlockKind::Ordered {
                counters.truncate(level as usize);
            }
        } else {
            counters.clear();
        }
        if let Some((pk, pl)) = &prev {
            out.push_str(if needs_blank(pk, *pl, &b.kind, level) { "\n\n" } else { "\n" });
        }
        out.push_str(&chunk);
        prev = Some((b.kind.clone(), level));
    }
    out
}

fn with_prefix(prefix: &str, text: &str) -> String {
    if text.is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix} {text}")
    }
}

fn needs_blank(pk: &BlockKind, pl: u8, ck: &BlockKind, cl: u8) -> bool {
    if is_list(pk) && is_list(ck) {
        let family = |k: &BlockKind| *k == BlockKind::Ordered;
        return pl == cl && family(pk) != family(ck);
    }
    !(*pk == BlockKind::Quote && *ck == BlockKind::Quote)
}

fn code_block(lang: &str, spans: &[Span]) -> String {
    let content: String = spans.iter().map(|s| s.text.as_str()).collect();
    let lang = lang.trim();
    let fence = if lang.contains('`') { '~' } else { '`' };
    let mut longest = 0;
    let mut run = 0;
    for c in content.chars() {
        if c == fence {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    let marks = fence.to_string().repeat(longest.max(2) + 1);
    if content.is_empty() {
        return format!("{marks}{lang}\n{marks}");
    }
    format!("{marks}{lang}\n{content}\n{marks}")
}

/// Backslash-escape a leading character that would turn a paragraph into a block.
fn protect_paragraph(line: String) -> String {
    if line == BR {
        return format!("\\{BR}");
    }
    if fence_open(&line).is_none() && parse_line(&line).is_none() {
        return line;
    }
    let at = if line.starts_with(|c: char| c.is_ascii_digit()) { line.find('.').unwrap_or(0) } else { 0 };
    let mut fixed = line;
    fixed.insert(at, '\\');
    fixed
}

#[derive(Clone, PartialEq)]
enum Layer {
    Link(String),
    Bold,
    Italic,
    Strike,
    Underline,
}

fn layers(style: &Style) -> Vec<Layer> {
    let mut v = Vec::new();
    if let Some(url) = &style.link {
        v.push(Layer::Link(url.clone()));
    }
    if style.bold {
        v.push(Layer::Bold);
    }
    if style.italic {
        v.push(Layer::Italic);
    }
    if style.strike {
        v.push(Layer::Strike);
    }
    if style.underline {
        v.push(Layer::Underline);
    }
    v
}

/// Inline Markdown for `spans`. Falls back to plain escaped text when the
/// styled form would not survive a parse/serialize cycle unchanged.
fn inline_md(spans: &[Span]) -> String {
    let s1 = raw_inline(spans).trim().to_string();
    let s2 = raw_inline(&parse_inline(&s1)).trim().to_string();
    if s1 == s2 {
        return s1;
    }
    let plain: Vec<Span> = spans.iter().map(|s| plain_span(s.text.clone())).collect();
    let text: String = plain.iter().map(|s| s.text.as_str()).collect();
    let mut out = String::new();
    escape_into(text.trim(), false, &mut out);
    out
}

fn raw_inline(spans: &[Span]) -> String {
    let mut out = String::new();
    let mut stack: Vec<Layer> = Vec::new();
    for s in spans {
        if s.text.is_empty() {
            continue;
        }
        if !s.style.code && s.text.chars().all(char::is_whitespace) {
            out.push_str(&s.text);
            continue;
        }
        if let Some(src) = &s.style.image {
            close_layers(&mut out, &mut stack, 0);
            push_image(&s.text, src, &mut out);
            continue;
        }
        let target = layers(&s.style);
        let k = stack.iter().zip(&target).take_while(|(a, b)| a == b).count();
        close_layers(&mut out, &mut stack, k);
        let mut text = s.text.as_str();
        if target.len() > k && !s.style.code {
            let t = text.trim_start();
            out.push_str(&text[..text.len() - t.len()]);
            text = t;
        }
        for l in &target[k..] {
            if matches!(l, Layer::Link(_)) && out.ends_with('!') && !out.ends_with("\\!") {
                // a literal `!` right before a link must not read as an image
                out.pop();
                out.push_str("\\!");
            }
            out.push_str(match l {
                Layer::Link(_) => "[",
                Layer::Bold => "**",
                Layer::Italic => "*",
                Layer::Strike => "~~",
                Layer::Underline => "<u>",
            });
            stack.push(l.clone());
        }
        if s.style.code {
            push_code(text, &mut out);
        } else {
            let in_link = stack.iter().any(|l| matches!(l, Layer::Link(_)));
            escape_into(text, in_link, &mut out);
        }
    }
    close_layers(&mut out, &mut stack, 0);
    out
}

/// `![alt](src)` with the characters that would end the alt or the source escaped.
fn push_image(alt: &str, src: &str, out: &mut String) {
    out.push_str("![");
    for c in alt.chars() {
        if matches!(c, '\\' | '[' | ']') {
            out.push('\\');
        }
        out.push(if c == '\n' { ' ' } else { c });
    }
    out.push_str("](");
    for c in src.chars() {
        if matches!(c, '\\' | '(' | ')') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push(')');
}

fn close_layers(out: &mut String, stack: &mut Vec<Layer>, keep: usize) {
    if stack.len() <= keep {
        return;
    }
    let trimmed = out.trim_end().len();
    let ws = out.split_off(trimmed);
    for l in stack.drain(keep..).rev() {
        match l {
            Layer::Link(url) => {
                out.push_str("](");
                for c in url.chars() {
                    if matches!(c, '\\' | '(' | ')') {
                        out.push('\\');
                    }
                    out.push(c);
                }
                out.push(')');
            }
            Layer::Bold => out.push_str("**"),
            Layer::Italic => out.push('*'),
            Layer::Strike => out.push_str("~~"),
            Layer::Underline => out.push_str("</u>"),
        }
    }
    out.push_str(&ws);
}

fn push_code(text: &str, out: &mut String) {
    let mut runs = std::collections::BTreeSet::new();
    let mut run = 0usize;
    for c in text.chars().chain(std::iter::once(' ')) {
        if c == '`' {
            run += 1;
        } else {
            if run > 0 {
                runs.insert(run);
            }
            run = 0;
        }
    }
    let len = (1..).find(|k| !runs.contains(k)).unwrap_or(1);
    let ticks = "`".repeat(len);
    let all_space = text.chars().all(|c| c == ' ');
    let pad = text.starts_with('`') || text.ends_with('`') || (text.starts_with(' ') && text.ends_with(' ') && !all_space);
    out.push_str(&ticks);
    if pad {
        out.push(' ');
    }
    out.push_str(text);
    if pad {
        out.push(' ');
    }
    out.push_str(&ticks);
}

/// Append `text`, escaping only characters that would otherwise be read as markup.
fn escape_into(text: &str, in_link: bool, out: &mut String) {
    let ch: Vec<char> = text.chars().collect();
    for (idx, &c) in ch.iter().enumerate() {
        let prev = idx.checked_sub(1).map(|p| ch[p]);
        let next = ch.get(idx + 1).copied();
        let escape = match c {
            '\\' => next.is_none_or(|n| ESCAPABLE.contains(n)),
            '*' | '`' | '[' => true,
            ']' => in_link,
            '_' => !(prev.is_some_and(char::is_alphanumeric) && next.is_some_and(char::is_alphanumeric)),
            '~' => !(prev.is_some_and(|p| p != '~') && next.is_some_and(|n| n != '~')),
            '<' => next.is_none() || ch[idx..].starts_with(&['<', 'u', '>']) || ch[idx..].starts_with(&['<', '/', 'u', '>']),
            _ => false,
        };
        if escape {
            out.push('\\');
        }
        out.push(c);
    }
}

impl Doc {
    /// The text without any markup or list prefixes, one line per block.
    /// This is what search indexing sees.
    pub fn plain_text(&self) -> String {
        let lines: Vec<String> = self.blocks.iter().map(block_text).collect();
        lines.join("\n")
    }

    /// The note title: the first non-blank line as plain text, at most 80 chars.
    pub fn title(&self) -> String {
        self.blocks
            .iter()
            .flat_map(|b| block_text(b).lines().map(str::to_string).collect::<Vec<_>>())
            .map(|l| l.trim().to_string())
            .find(|l| !l.is_empty())
            .map(|l| l.chars().take(80).collect())
            .unwrap_or_default()
    }
}

fn block_text(b: &Block) -> String {
    b.spans.iter().map(|s| s.text.as_str()).collect()
}

// ---------------------------------------------------------------------------
// Typing helpers
// ---------------------------------------------------------------------------

/// Markdown typing shortcut at the start of a line.
///
/// `line_prefix` is the text typed so far on the line, including the space that
/// triggers it. Returns the new block kind and how many chars to remove, or
/// `None` when the prefix is not a shortcut (`####`, `#` without a space, `1.5 `).
pub fn block_shortcut(line_prefix: &str) -> Option<(BlockKind, usize)> {
    let n = line_prefix.chars().count();
    let kind = match line_prefix {
        "# " => BlockKind::Heading(1),
        "## " => BlockKind::Heading(2),
        "### " => BlockKind::Heading(3),
        "> " => BlockKind::Quote,
        "* " | "- " => BlockKind::Bullet,
        "[] " | "[ ] " => BlockKind::Task { checked: false },
        "[x] " | "[X] " => BlockKind::Task { checked: true },
        "--- " | "___ " => BlockKind::Rule,
        "~~~" => BlockKind::Code { lang: String::new() },
        _ => return shortcut_dynamic(line_prefix).map(|k| (k, n)),
    };
    Some((kind, n))
}

fn shortcut_dynamic(p: &str) -> Option<BlockKind> {
    let body = p.strip_suffix(' ')?;
    if let Some(num) = body.strip_suffix('.') {
        let ok = !num.is_empty() && num.len() <= 9 && num.chars().all(|c| c.is_ascii_digit());
        return ok.then_some(BlockKind::Ordered);
    }
    for fence in ["```", "~~~"] {
        if let Some(lang) = body.strip_prefix(fence) {
            if !lang.chars().any(|c| c.is_whitespace() || c == '`' || c == '~') {
                return Some(BlockKind::Code { lang: lang.to_string() });
            }
        }
    }
    None
}

/// Autoformat marker: char, run length, style setter.
type Marker = (char, usize, fn(&mut Style));

/// Check whether the user just typed a closing inline marker.
///
/// `line_before_cursor` ends with the closing marker. Detects `**x**`, `__x__`,
/// `*x*`, `_x_`, `~~x~~` and `` `x` ``. It does not fire for empty inner text,
/// inner text that starts or ends with whitespace, `_` markers inside a word,
/// or a bare `**`.
pub fn inline_autoformat(line_before_cursor: &str) -> Option<InlineEdit> {
    let ch: Vec<char> = line_before_cursor.chars().collect();
    let n = ch.len();
    let table: [Marker; 6] = [
        ('*', 2, |s| s.bold = true),
        ('_', 2, |s| s.bold = true),
        ('~', 2, |s| s.strike = true),
        ('*', 1, |s| s.italic = true),
        ('_', 1, |s| s.italic = true),
        ('`', 1, |s| s.code = true),
    ];
    for (mc, m, apply) in table {
        if n < 2 * m + 1 || ch[n - m..].iter().any(|&c| c != mc) {
            continue;
        }
        let close = n - m;
        let Some(p) = ch[..close].iter().rposition(|&c| c == mc) else { continue };
        let open = if m == 2 {
            if p == 0 || ch[p - 1] != mc {
                continue;
            }
            p - 1
        } else {
            p
        };
        let before = open.checked_sub(1).map(|b| ch[b]);
        let inner = &ch[open + m..close];
        let (Some(f), Some(l)) = (inner.first(), inner.last()) else { continue };
        let bad = f.is_whitespace()
            || l.is_whitespace()
            || before == Some(mc)
            || (mc == '_' && before.is_some_and(char::is_alphanumeric))
            || (m == 1 && close > 0 && ch[close - 1] == mc);
        if bad {
            continue;
        }
        let mut style = Style::default();
        apply(&mut style);
        return Some(InlineEdit { start: open, end: n, inner: inner.iter().collect(), style });
    }
    None
}

/// What Enter does on `block`: continue the list/task/quote, or leave it when the
/// item is empty (outdent first when indented).
pub fn list_continue(block: &Block) -> ListEnter {
    let listy = is_list(&block.kind) || block.kind == BlockKind::Quote;
    if !listy {
        return ListEnter::NotAList;
    }
    if block.spans.iter().all(|s| s.text.is_empty()) {
        return if is_list(&block.kind) && block.level > 0 { ListEnter::Outdent { level: block.level - 1 } } else { ListEnter::Exit };
    }
    let kind = match &block.kind {
        BlockKind::Task { .. } => BlockKind::Task { checked: false },
        other => other.clone(),
    };
    ListEnter::Continue { kind, level: block.level.min(MAX_LEVEL) }
}

/// New list level after Tab (`delta` = 1) or Shift-Tab (`delta` = -1), clamped to 0..=4.
pub fn indent(level: u8, delta: i8) -> u8 {
    (i16::from(level) + i16::from(delta)).clamp(0, i16::from(MAX_LEVEL)) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn md(x: &str) -> String {
        to_markdown(&parse(x))
    }

    fn plain(text: &str) -> Span {
        Span { text: text.into(), style: Style::default() }
    }

    fn styled(text: &str, f: impl Fn(&mut Style)) -> Span {
        let mut style = Style::default();
        f(&mut style);
        Span { text: text.into(), style }
    }

    fn block(kind: BlockKind, level: u8, text: &str) -> Block {
        Block { kind, level, spans: if text.is_empty() { vec![] } else { vec![plain(text)] } }
    }

    fn first(x: &str) -> Block {
        parse(x).blocks.into_iter().next().expect("one block")
    }

    // ---------- S7: paragraph formats ----------

    #[test]
    fn parse_heading_1_2_3_and_not_4() {
        assert_eq!(first("# a").kind, BlockKind::Heading(1));
        assert_eq!(first("## a").kind, BlockKind::Heading(2));
        assert_eq!(first("### a").kind, BlockKind::Heading(3));
        let h4 = first("#### a");
        assert_eq!(h4.kind, BlockKind::Paragraph);
        assert_eq!(h4.spans, vec![plain("#### a")]);
        assert_eq!(first("#a").kind, BlockKind::Paragraph);
        assert_eq!(first("# a").spans, vec![plain("a")]);
    }

    #[test]
    fn parse_quote_and_bullets_and_ordered() {
        assert_eq!(first("> q").kind, BlockKind::Quote);
        assert_eq!(first("- a").kind, BlockKind::Bullet);
        assert_eq!(first("* a").kind, BlockKind::Bullet);
        assert_eq!(first("1. a").kind, BlockKind::Ordered);
        assert_eq!(first("42. a").kind, BlockKind::Ordered);
        assert_eq!(first("1.5 a").kind, BlockKind::Paragraph);
        assert_eq!(first("- a").spans, vec![plain("a")]);
    }

    #[test]
    fn parse_tasks_all_forms() {
        assert_eq!(first("- [ ] a").kind, BlockKind::Task { checked: false });
        assert_eq!(first("- [x] a").kind, BlockKind::Task { checked: true });
        assert_eq!(first("[] a").kind, BlockKind::Task { checked: false });
        assert_eq!(first("[x] a").kind, BlockKind::Task { checked: true });
        assert_eq!(first("[ ] a").kind, BlockKind::Task { checked: false });
        assert_eq!(first("[x] a").spans, vec![plain("a")]);
    }

    #[test]
    fn parse_code_fences() {
        let b = first("```rust\nlet x = 1;\n  **raw**\n```");
        assert_eq!(b.kind, BlockKind::Code { lang: "rust".into() });
        assert_eq!(b.spans, vec![plain("let x = 1;\n  **raw**")]);
        let t = first("~~~\nabc\n~~~");
        assert_eq!(t.kind, BlockKind::Code { lang: String::new() });
        let open = parse("```\nabc\n\nmore");
        assert_eq!(open.blocks.len(), 1);
        assert_eq!(open.blocks[0].spans, vec![plain("abc\n\nmore")]);
        // a closing fence must be at least as long as the opening one
        let longer = first("````\n```\nx\n````");
        assert_eq!(longer.spans, vec![plain("```\nx")]);
    }

    #[test]
    fn parse_rules() {
        assert_eq!(first("---").kind, BlockKind::Rule);
        assert_eq!(first("___").kind, BlockKind::Rule);
        assert_eq!(first("--").kind, BlockKind::Paragraph);
    }

    #[test]
    fn parse_list_levels_two_four_spaces_and_tab() {
        let d = parse("- a\n  - b\n    - c\n- d");
        let levels: Vec<u8> = d.blocks.iter().map(|b| b.level).collect();
        assert_eq!(levels, vec![0, 1, 2, 0]);
        let d = parse("- a\n    - b\n        - c");
        let levels: Vec<u8> = d.blocks.iter().map(|b| b.level).collect();
        assert_eq!(levels, vec![0, 1, 2]);
        let d = parse("- a\n\t- b\n\t\t- c\n1. x\n\t1. y");
        let levels: Vec<u8> = d.blocks.iter().map(|b| b.level).collect();
        assert_eq!(levels, vec![0, 1, 2, 0, 1]);
        let deep = parse("- a\n  - b\n    - c\n      - d\n        - e\n          - f");
        assert_eq!(deep.blocks.iter().map(|b| b.level).max(), Some(MAX_LEVEL));
    }

    // ---------- S8: text formats ----------

    #[test]
    fn parse_inline_styles() {
        let bold = Style { bold: true, ..Style::default() };
        let it = Style { italic: true, ..Style::default() };
        assert_eq!(first("**x**").spans, vec![Span { text: "x".into(), style: bold.clone() }]);
        assert_eq!(first("__x__").spans, vec![Span { text: "x".into(), style: bold }]);
        assert_eq!(first("*x*").spans, vec![Span { text: "x".into(), style: it.clone() }]);
        assert_eq!(first("_x_").spans, vec![Span { text: "x".into(), style: it }]);
        assert_eq!(first("~~x~~").spans, vec![styled("x", |s| s.strike = true)]);
        assert_eq!(first("`x`").spans, vec![styled("x", |s| s.code = true)]);
        assert_eq!(first("<u>x</u>").spans, vec![styled("x", |s| s.underline = true)]);
        assert_eq!(first("[t](http://a.b/c)").spans, vec![styled("t", |s| s.link = Some("http://a.b/c".into()))]);
    }

    #[test]
    fn parse_nested_and_mixed_inline() {
        let d = first("*a **b** c*");
        assert_eq!(
            d.spans,
            vec![
                styled("a ", |s| s.italic = true),
                styled("b", |s| {
                    s.italic = true;
                    s.bold = true
                }),
                styled(" c", |s| s.italic = true),
            ]
        );
        assert_eq!(
            first("***x***").spans,
            vec![styled("x", |s| {
                s.bold = true;
                s.italic = true
            })]
        );
        assert_eq!(
            first("[**b**](u)").spans,
            vec![styled("b", |s| {
                s.bold = true;
                s.link = Some("u".into())
            })]
        );
        assert_eq!(first("`**x**`").spans, vec![styled("**x**", |s| s.code = true)]);
    }

    #[test]
    fn parse_escapes_and_unmatched_markers_stay_literal() {
        assert_eq!(first("\\*a\\*").spans, vec![plain("*a*")]);
        assert_eq!(first("\\# a").spans, vec![plain("# a")]);
        for lit in ["**a", "*a", "a*", "_a", "~~a", "`a", "[a", "[a](", "<u>a", "a ** b", "snake_case_name"] {
            assert_eq!(first(lit).spans, vec![plain(lit)], "{lit}");
        }
    }

    #[test]
    fn parse_never_panics_and_handles_crlf() {
        for s in ["", "\n", "\r\n", "*", "**", "[", "](", "`", "```", "~~~", "<u>", "</u>", "\\", "- ", "#", ">", "1.", "é*日本*😀"] {
            let _ = parse(s);
        }
        let long = "*a ".repeat(50_000);
        let _ = md(&long);
        let long2 = "[".repeat(100_000) + &"`".repeat(1000);
        let _ = md(&long2);
        assert_eq!(parse("a\r\n\r\nb\r\n"), parse("a\n\nb\n"));
        assert_eq!(parse("```\r\nx\r\n```"), parse("```\nx\n```"));
        assert_eq!(parse("").blocks.len(), 0);
    }

    // ---------- to_markdown ----------

    #[test]
    fn to_markdown_blank_line_policy() {
        assert_eq!(md("a\nb"), "a\n\nb");
        assert_eq!(md("- a\n- b"), "- a\n- b");
        assert_eq!(md("# h\ntext\n- a"), "# h\n\ntext\n\n- a");
        assert_eq!(md("> a\n> b"), "> a\n> b");
        assert_eq!(md("- a\n1. b"), "- a\n\n1. b");
        assert_eq!(md("a\n\n\n\nb\n\n"), "a\n\nb");
    }

    #[test]
    fn to_markdown_renumbers_ordered_runs_per_level() {
        assert_eq!(md("5. a\n9. b\n2. c"), "1. a\n2. b\n3. c");
        assert_eq!(md("1. a\n  1. x\n  1. y\n1. b"), "1. a\n  1. x\n  2. y\n2. b");
        assert_eq!(md("1. a\n- b\n1. c"), "1. a\n\n- b\n\n1. c");
    }

    #[test]
    fn to_markdown_two_spaces_per_level_even_for_tabs_and_four_spaces() {
        assert_eq!(md("- a\n\t- b\n\t\t- c"), "- a\n  - b\n    - c");
        assert_eq!(md("- a\n    - b"), "- a\n  - b");
    }

    #[test]
    fn to_markdown_code_fences_grow_and_inline_code_backticks() {
        let d = Doc { blocks: vec![block(BlockKind::Code { lang: "rs".into() }, 0, "a ``` b\nc")] };
        assert_eq!(to_markdown(&d), "````rs\na ``` b\nc\n````");
        assert_eq!(parse(&to_markdown(&d)), d);
        let code = Doc { blocks: vec![Block { kind: BlockKind::Paragraph, level: 0, spans: vec![styled("a`b", |s| s.code = true)] }] };
        assert_eq!(to_markdown(&code), "``a`b``");
        assert_eq!(parse(&to_markdown(&code)), code);
        let edge = Doc { blocks: vec![Block { kind: BlockKind::Paragraph, level: 0, spans: vec![styled("`x`", |s| s.code = true)] }] };
        assert_eq!(parse(&to_markdown(&edge)), edge);
    }

    #[test]
    fn to_markdown_escapes_only_what_would_be_markup() {
        let p = |t: &str| Doc { blocks: vec![block(BlockKind::Paragraph, 0, t)] };
        assert_eq!(to_markdown(&p("a * b")), "a \\* b");
        assert_eq!(to_markdown(&p("snake_case_name")), "snake_case_name");
        assert_eq!(to_markdown(&p("# x")), "\\# x");
        assert_eq!(to_markdown(&p("1. x")), "1\\. x");
        assert_eq!(to_markdown(&p("- x")), "\\- x");
        assert_eq!(to_markdown(&p("> x")), "\\> x");
        assert_eq!(to_markdown(&p("plain text 1.5 and a-b")), "plain text 1.5 and a-b");
        for t in ["# x", "1. x", "- x", "> x", "---", "```", "~~~", "[x] a", "[] a", "a_b_c", "_x_", "<u>a</u>", "\\", "a\\*", "[a](b)"] {
            assert_eq!(parse(&to_markdown(&p(t))), p(t), "{t}");
        }
    }

    #[test]
    fn to_markdown_link_url_with_parens_roundtrips() {
        let d = Doc {
            blocks: vec![Block {
                kind: BlockKind::Paragraph,
                level: 0,
                spans: vec![styled("w", |s| s.link = Some("https://x.y/a_(b)".into()))],
            }],
        };
        assert_eq!(parse(&to_markdown(&d)), d);
        assert_eq!(first("[w](https://x.y/a_(b))").spans, d.blocks[0].spans);
    }

    // ---------- idempotence and fidelity ----------

    const SAMPLES: &[&str] = &[
        "# h",
        "## h",
        "### h",
        "#### h",
        "#h",
        "> q",
        "> > q",
        ">",
        "* a",
        "- a",
        "-",
        "*",
        "1. a",
        "12. a",
        "1.",
        "1.5 a",
        "[] t",
        "[x] t",
        "[X] t",
        "[ ] t",
        "- [ ] t",
        "- [x] t",
        "- [x]",
        "```\nx\n```",
        "```rust\nx\n```",
        "~~~\nx\n~~~",
        "```\nunterminated\n\nmore",
        "````\n```\n````",
        "```a`b\nx\n```",
        "~~~ a`b\nx\n~~~",
        "---",
        "___",
        "--",
        "- - -",
        "__b__",
        "**b**",
        "*i*",
        "_i_",
        "~~s~~",
        "<u>u</u>",
        "`c`",
        "[t](u)",
        "[t](u",
        "[t]()",
        "[](u)",
        "[[a](b)](c)",
        "**a *b***",
        "*a **b***",
        "***x***",
        "**a***b*",
        "*a***b**",
        "__a_b__",
        "snake_case_name",
        "a_b_",
        "<u>a **b** c</u>",
        "**<u>x</u>**",
        "`a``b`",
        "`` ` ``",
        "\\*",
        "\\\\*",
        "\\",
        "a\\",
        "**a **",
        "** a**",
        "* a*",
        "- a\n  - b\n    - c\n- d",
        "\t- a\n\t\t- b",
        "    - a\n  - b",
        "1. a\n  - b\n2. c",
        "a\r\nb",
        "\r\n",
        "",
        "héllo **wörld** 日本語 *x* 😀",
        "text\n\n\n\nmore",
        "- a\n\n1. b\n\n> c\n\n# d",
        "- [ ] a **b** [c](d)",
        "> **q** `c`",
        "**bold *it* more**",
        "*a ~~b~~ c*",
        "[a **b** c](http://u)",
        "~~~",
        "~~",
        "~~a~~b~~",
        "<u></u>",
        "**__**",
        "[x]",
        "[]",
        "a  \nb",
        "   spaced   ",
        "**a**  *b*",
        "*a*b*c*",
        "_a_b_",
        "\\# \\> \\- \\1.",
        "1\\. a",
        "](",
        "* * *",
        "- *a*",
        "**\n**",
        "<br>",
        " <br> ",
        "a\n\n\n<br>\n\nb",
        "\\<br>",
        "<br>x",
        "- <br>",
        "> <br>",
        "<b",
        "<br",
        "br>",
        "<<br>>",
        "*<br>*",
        "<br>\n<br>\n<br>",
        "a\n<br>\n- b\n<br>\n1. c",
    ];

    #[test]
    fn idempotent_on_samples() {
        assert!(SAMPLES.len() >= 40);
        for x in SAMPLES {
            let once = md(x);
            assert_eq!(md(&once), once, "input {x:?} -> {once:?}");
        }
    }

    #[test]
    fn canonical_markdown_is_unchanged() {
        let canonical = [
            "# Title",
            "## Sub",
            "### Third",
            "> quote",
            "> a\n> b",
            "- a\n- b",
            "1. a\n2. b\n3. c",
            "- [ ] todo\n- [x] done",
            "```rust\nfn main() {}\n```",
            "```\nplain\n```",
            "---",
            "**bold**",
            "*italic*",
            "~~strike~~",
            "<u>under</u>",
            "`code`",
            "[text](https://example.com)",
            "para one\n\npara two",
            "- a\n  - b\n    - c\n- d",
            "# T\n\ntext\n\n- a\n- b\n\n1. x",
            "***both***",
            "**bold** and *it* plain",
            "1. a\n  - b\n2. c",
            "\\*not italic\\*",
            "snake_case_name",
            "- [ ]",
            "````\ncode with ```\n````",
            "`` `tick` ``",
            "[**bold link**](http://x.y)",
            "\\# not heading",
            "1\\. not list",
            "> **q** text",
            "- **a**\n- *b*",
            "text\n\n---",
            "a ~~b~~ c",
            "- a\n\n1. b",
            "- [ ] a\n- b",
            "**bold *it***",
            "<u>a</u> **b**",
            "",
            "```\n```",
        ];
        for x in canonical {
            assert_eq!(md(x), x, "canonical {x:?}");
        }
    }

    #[test]
    fn fuzz_idempotence_and_no_panic() {
        let alphabet: Vec<char> = "**__~~``[]()<>u/#- 1.\\\n\tabrx:é".chars().collect();
        let mut state: u64 = 0x2545_F491_4F6C_DD1D;
        let mut next = move || {
            state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            (state >> 33) as usize
        };
        for _ in 0..6000 {
            let len = next() % 28;
            let s: String = (0..len).map(|_| alphabet[next() % alphabet.len()]).collect();
            let once = md(&s);
            assert_eq!(md(&once), once, "input {s:?} -> {once:?}");
        }
    }

    // ---------- empty paragraphs (<br>) ----------

    fn empty_para() -> Block {
        Block { kind: BlockKind::Paragraph, level: 0, spans: vec![] }
    }

    #[test]
    fn empty_paragraphs_survive_save_and_reload() {
        let doc = Doc { blocks: vec![block(BlockKind::Paragraph, 0, "a"), empty_para(), block(BlockKind::Paragraph, 0, "b")] };
        assert_eq!(to_markdown(&doc), "a\n\n<br>\n\nb");
        assert_eq!(parse(&to_markdown(&doc)), doc);
        let three = Doc { blocks: vec![empty_para(), empty_para(), empty_para()] };
        assert_eq!(to_markdown(&three), "<br>\n\n<br>\n\n<br>");
        assert_eq!(parse(&to_markdown(&three)), three);
        let only_empty_text = Doc { blocks: vec![Block { spans: vec![plain("")], ..empty_para() }] };
        assert_eq!(to_markdown(&only_empty_text), "<br>");
        assert_eq!(parse("  <br>  \r\n"), Doc { blocks: vec![empty_para()] });
    }

    #[test]
    fn empty_paragraph_canonical_text() {
        for x in ["a\n\n<br>\n\nb", "<br>\n\n<br>\n\n<br>", "<br>", "# h\n\n<br>\n\n- a", "\\<br>", "a <br> b"] {
            assert_eq!(md(x), x, "{x:?}");
        }
        assert_eq!(md("a\n\n\n\nb"), "a\n\nb");
    }

    #[test]
    fn literal_br_text_is_escaped_and_roundtrips() {
        let doc = Doc { blocks: vec![block(BlockKind::Paragraph, 0, "<br>")] };
        assert_eq!(to_markdown(&doc), "\\<br>");
        assert_eq!(parse("\\<br>"), doc);
        assert_eq!(parse(&to_markdown(&doc)), doc);
    }

    #[test]
    fn empty_non_paragraph_blocks_behavior() {
        // Empty headings are dropped; empty quote and list items are kept as bare markers.
        let doc =
            Doc { blocks: vec![block(BlockKind::Heading(1), 0, ""), block(BlockKind::Quote, 0, ""), block(BlockKind::Bullet, 0, "")] };
        assert_eq!(to_markdown(&doc), ">\n\n-");
    }

    #[test]
    fn plain_text_and_title_with_empty_paragraphs() {
        let doc =
            Doc { blocks: vec![empty_para(), block(BlockKind::Paragraph, 0, "a"), empty_para(), block(BlockKind::Paragraph, 0, "b")] };
        assert_eq!(doc.plain_text(), "\na\n\nb");
        assert_eq!(doc.title(), "a");
    }

    /// Test-only mirror of the guard in `inline_md`: does the styled form fail the fixpoint check?
    fn falls_back(spans: &[Span]) -> bool {
        let s1 = raw_inline(spans).trim().to_string();
        s1 != raw_inline(&parse_inline(&s1)).trim()
    }

    #[test]
    fn fallback_fires_on_known_inputs() {
        // Italic runs split by a plain space merge into one italic span, and the `<` escape
        // that depended on the old span boundary then changes: the styled form is not stable,
        // so the line is written as plain text and the italics are lost.
        for x in ["*<* *_*", "*<* *a*"] {
            let d = parse(x);
            assert!(falls_back(&d.blocks[0].spans), "{x:?}");
            let out = md(x);
            assert!(!out.contains('*') || out.contains("\\*"), "{out:?}");
            assert_eq!(md(&out), out);
        }
        assert!(!falls_back(&parse("*a* *b*").blocks[0].spans));
    }

    #[test]
    fn fallback_corpus_report() {
        let alphabet: Vec<char> = "**__~~``[]()<>u/#- 1.\\\n\tabrx:é".chars().collect();
        let mut state: u64 = 0x2545_F491_4F6C_DD1D;
        let mut next = move || {
            state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            (state >> 33) as usize
        };
        let (mut lines, mut fired, mut lossy) = (0usize, 0usize, 0usize);
        let mut shortest: Vec<String> = Vec::new();
        for _ in 0..6000 {
            let len = next() % 28;
            let s: String = (0..len).map(|_| alphabet[next() % alphabet.len()]).collect();
            for b in parse(&s).blocks {
                if matches!(b.kind, BlockKind::Code { .. } | BlockKind::Rule) {
                    continue;
                }
                lines += 1;
                if falls_back(&b.spans) {
                    fired += 1;
                    if b.spans.iter().any(|sp| sp.style != Style::default()) {
                        lossy += 1;
                        shortest.push(s.clone());
                    }
                }
            }
        }
        shortest.sort_by_key(|s| s.chars().count());
        shortest.dedup();
        eprintln!(
            "FALLBACK: {fired} of {lines} styled-or-plain lines fell back ({lossy} had styles to lose); shortest: {:?}",
            &shortest[..shortest.len().min(8)]
        );
    }

    // ---------- helpers: plain text, title ----------

    #[test]
    fn plain_text_strips_markup() {
        let d = parse("# Hello **world**\n\n- a\n- [x] b\n\n```\ncode\n```");
        assert_eq!(d.plain_text(), "Hello world\na\nb\ncode");
    }

    #[test]
    fn title_is_first_non_blank_block_up_to_80_chars() {
        assert_eq!(parse("\n\n# Hello **world**\nmore").title(), "Hello world");
        assert_eq!(parse("").title(), "");
        assert_eq!(parse("---\ntext").title(), "text");
        let long = "x".repeat(200);
        assert_eq!(parse(&long).title().chars().count(), 80);
    }

    // ---------- S7: block shortcuts ----------

    #[test]
    fn shortcut_headings() {
        assert_eq!(block_shortcut("# "), Some((BlockKind::Heading(1), 2)));
        assert_eq!(block_shortcut("## "), Some((BlockKind::Heading(2), 3)));
        assert_eq!(block_shortcut("### "), Some((BlockKind::Heading(3), 4)));
        assert_eq!(block_shortcut("#### "), None);
        assert_eq!(block_shortcut("#"), None);
        assert_eq!(block_shortcut("##"), None);
    }

    #[test]
    fn shortcut_quote_bullets_ordered() {
        assert_eq!(block_shortcut("> "), Some((BlockKind::Quote, 2)));
        assert_eq!(block_shortcut("* "), Some((BlockKind::Bullet, 2)));
        assert_eq!(block_shortcut("- "), Some((BlockKind::Bullet, 2)));
        assert_eq!(block_shortcut("1. "), Some((BlockKind::Ordered, 3)));
        assert_eq!(block_shortcut("12. "), Some((BlockKind::Ordered, 4)));
        assert_eq!(block_shortcut("1.5 "), None);
        assert_eq!(block_shortcut("1 "), None);
        assert_eq!(block_shortcut("a. "), None);
        assert_eq!(block_shortcut("-"), None);
        assert_eq!(block_shortcut("a "), None);
    }

    #[test]
    fn shortcut_tasks() {
        assert_eq!(block_shortcut("[] "), Some((BlockKind::Task { checked: false }, 3)));
        assert_eq!(block_shortcut("[x] "), Some((BlockKind::Task { checked: true }, 4)));
        assert_eq!(block_shortcut("[ ] "), Some((BlockKind::Task { checked: false }, 4)));
        assert_eq!(block_shortcut("[y] "), None);
    }

    #[test]
    fn shortcut_code_blocks() {
        assert_eq!(block_shortcut("``` "), Some((BlockKind::Code { lang: String::new() }, 4)));
        assert_eq!(block_shortcut("~~~"), Some((BlockKind::Code { lang: String::new() }, 3)));
        assert_eq!(block_shortcut("~~~ "), Some((BlockKind::Code { lang: String::new() }, 4)));
        assert_eq!(block_shortcut("```rust "), Some((BlockKind::Code { lang: "rust".into() }, 8)));
        assert_eq!(block_shortcut("```"), None);
        assert_eq!(block_shortcut("`` "), None);
    }

    #[test]
    fn shortcut_rules() {
        assert_eq!(block_shortcut("--- "), Some((BlockKind::Rule, 4)));
        assert_eq!(block_shortcut("___ "), Some((BlockKind::Rule, 4)));
        assert_eq!(block_shortcut("-- "), None);
        assert_eq!(block_shortcut("---"), None);
        assert_eq!(block_shortcut(""), None);
    }

    // ---------- S8: inline autoformat ----------

    fn edit(line: &str) -> Option<(usize, usize, String, Style)> {
        inline_autoformat(line).map(|e| (e.start, e.end, e.inner, e.style))
    }

    #[test]
    fn autoformat_bold_italic_strike_code() {
        let bold = Style { bold: true, ..Style::default() };
        let it = Style { italic: true, ..Style::default() };
        assert_eq!(edit("say **hi**"), Some((4, 10, "hi".into(), bold.clone())));
        assert_eq!(edit("say __hi__"), Some((4, 10, "hi".into(), bold)));
        assert_eq!(edit("say *hi*"), Some((4, 8, "hi".into(), it.clone())));
        assert_eq!(edit("say _hi_"), Some((4, 8, "hi".into(), it)));
        assert_eq!(edit("~~gone~~"), Some((0, 8, "gone".into(), styled("", |s| s.strike = true).style)));
        assert_eq!(edit("run `ls -l`"), Some((4, 11, "ls -l".into(), styled("", |s| s.code = true).style)));
        assert_eq!(edit("é **日本**").map(|e| (e.0, e.1)), Some((2, 8)));
    }

    #[test]
    fn autoformat_rejections() {
        for line in [
            "",
            "**",
            "****",
            "* *",
            "** a**",
            "**a **",
            "*a *",
            "* a*",
            "snake_case_",
            "a_b_",
            "**x*",
            "~~",
            "~~~~",
            "``",
            "plain",
            "**x** more",
        ] {
            assert_eq!(edit(line), None, "{line:?}");
        }
        assert!(edit("(_x_").is_some());
    }

    // ---------- Enter, indent ----------

    #[test]
    fn enter_continues_non_empty_items() {
        assert_eq!(list_continue(&block(BlockKind::Bullet, 2, "a")), ListEnter::Continue { kind: BlockKind::Bullet, level: 2 });
        assert_eq!(list_continue(&block(BlockKind::Ordered, 1, "a")), ListEnter::Continue { kind: BlockKind::Ordered, level: 1 });
        assert_eq!(
            list_continue(&block(BlockKind::Task { checked: true }, 0, "a")),
            ListEnter::Continue { kind: BlockKind::Task { checked: false }, level: 0 }
        );
        assert_eq!(list_continue(&block(BlockKind::Quote, 0, "a")), ListEnter::Continue { kind: BlockKind::Quote, level: 0 });
    }

    #[test]
    fn enter_on_empty_item_exits_or_outdents() {
        assert_eq!(list_continue(&block(BlockKind::Bullet, 0, "")), ListEnter::Exit);
        assert_eq!(list_continue(&block(BlockKind::Task { checked: false }, 0, "")), ListEnter::Exit);
        assert_eq!(list_continue(&block(BlockKind::Quote, 0, "")), ListEnter::Exit);
        assert_eq!(list_continue(&block(BlockKind::Ordered, 2, "")), ListEnter::Outdent { level: 1 });
        assert_eq!(list_continue(&block(BlockKind::Paragraph, 0, "a")), ListEnter::NotAList);
        assert_eq!(list_continue(&block(BlockKind::Heading(1), 0, "a")), ListEnter::NotAList);
        assert_eq!(list_continue(&block(BlockKind::Code { lang: String::new() }, 0, "a")), ListEnter::NotAList);
    }

    #[test]
    fn indent_is_clamped() {
        assert_eq!(indent(0, 1), 1);
        assert_eq!(indent(4, 1), 4);
        assert_eq!(indent(0, -1), 0);
        assert_eq!(indent(3, -1), 2);
        assert_eq!(indent(2, 100), 4);
        assert_eq!(indent(200, 0), 4);
        assert_eq!(indent(1, -128), 0);
    }

    // ---------- images ----------

    fn image(alt: &str, src: &str) -> Span {
        styled(alt, |s| s.image = Some(src.into()))
    }

    #[test]
    fn an_image_is_one_atomic_span_with_its_alt_and_source() {
        assert_eq!(first("![shot](attachments/a.png)").spans, vec![image("shot", "attachments/a.png")]);
        assert_eq!(first("![](attachments/a.png)").spans, vec![image("image", "attachments/a.png")]);
        assert_eq!(first("see ![x](a.png) here").spans, vec![plain("see "), image("x", "a.png"), plain(" here")]);
        // a link is not an image, and an image inside a link text stays text
        assert_eq!(first("[t](http://a.b)").spans, vec![styled("t", |s| s.link = Some("http://a.b".into()))]);
        // an unfinished image stays literal
        assert_eq!(first("![x](").spans, vec![plain("![x](")]);
    }

    #[test]
    fn images_round_trip_and_keep_their_text_neighbours() {
        for src in ["![shot](attachments/a.png)", "text ![x](attachments/a.png) more", "![a\\]b](my pic \\(1\\).png)"] {
            assert_eq!(md(src), src, "{src}");
        }
        let d = Doc { blocks: vec![Block { kind: BlockKind::Paragraph, level: 0, spans: vec![image("a [b]", "p/(x).png")] }] };
        assert_eq!(parse(&to_markdown(&d)), d);
    }

    #[test]
    fn a_literal_bang_before_a_link_does_not_turn_into_an_image() {
        let d = Doc {
            blocks: vec![Block {
                kind: BlockKind::Paragraph,
                level: 0,
                spans: vec![plain("wow!"), styled("t", |s| s.link = Some("u".into()))],
            }],
        };
        assert_eq!(parse(&to_markdown(&d)), d);
    }

    #[test]
    fn the_plain_text_and_title_of_an_image_use_its_alt() {
        assert_eq!(parse("![shot](attachments/a.png)").title(), "shot");
    }
}
