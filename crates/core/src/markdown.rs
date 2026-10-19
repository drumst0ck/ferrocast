//! Small, pure (std-only) Markdown / HTML / plain-text converters used by the
//! file-conversion matrix (e.g. `textutil -convert html` output → Markdown).
//!
//! The Markdown dialect is deliberately minimal: ATX headings, paragraphs,
//! `**bold**`, `*italic*` / `_italic_`, `` `code` ``, fenced code blocks,
//! links, flat bulleted / numbered lists, `>` blockquotes and `---` rules.
//! Every function is total: malformed input never panics.

use std::collections::HashSet;

// ───────────────────────────── entities ─────────────────────────────

fn entity(name: &str) -> Option<char> {
    if let Some(num) = name.strip_prefix('#') {
        let code = match num.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => num.parse::<u32>().ok()?,
        };
        return char::from_u32(code);
    }
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => '\u{a0}',
        "copy" => '©',
        "reg" => '®',
        "trade" => '™',
        "hellip" => '…',
        "mdash" => '—',
        "ndash" => '–',
        "lsquo" => '‘',
        "rsquo" => '’',
        "ldquo" => '“',
        "rdquo" => '”',
        "bull" => '•',
        "euro" => '€',
        _ => return None,
    })
}

/// Decodes the named/numeric HTML entities we care about; unknown ones stay
/// literal. `&nbsp;` becomes U+00A0.
fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(p) = rest.find('&') {
        out.push_str(&rest[..p]);
        let after = &rest[p + 1..];
        let decoded = after.find(';').filter(|&e| e > 0 && e <= 10).and_then(|e| entity(&after[..e]).map(|c| (c, e)));
        match decoded {
            Some((c, e)) => {
                out.push(c);
                rest = &after[e + 1..];
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

// ───────────────────────────── HTML → Markdown ─────────────────────────────

#[derive(Clone, Debug)]
enum Mark {
    Bold,
    Italic,
    Code,
    Link(String),
}

struct Open {
    mark: Mark,
    /// Byte index in `cur` of the opening marker.
    start: usize,
    /// False for nested duplicates / suppressed marks (nothing was written).
    emitted: bool,
}

struct Frame {
    name: String,
    marks: usize,
}

#[derive(Default)]
struct Converter {
    out: String,
    cur: String,
    prefix: String,
    last_li: bool,
    opens: Vec<Open>,
    frames: Vec<Frame>,
    lists: Vec<(bool, u32)>,
    quote_depth: usize,
    in_heading: usize,
    in_pre: bool,
    in_head: bool,
    in_cell: bool,
    rows: Vec<Vec<String>>,
    row: Vec<String>,
    bold_depth: usize,
    italic_depth: usize,
    bold_classes: HashSet<String>,
    italic_classes: HashSet<String>,
}

fn parse_attrs(s: &str) -> Vec<(String, String)> {
    let c: Vec<char> = s.chars().collect();
    let n = c.len();
    let mut i = 0;
    let mut out = Vec::new();
    while i < n {
        while i < n && (c[i].is_whitespace() || c[i] == '/') {
            i += 1;
        }
        let ks = i;
        while i < n && !c[i].is_whitespace() && c[i] != '=' && c[i] != '/' {
            i += 1;
        }
        if ks == i {
            i += 1;
            continue;
        }
        let key: String = c[ks..i].iter().collect::<String>().to_ascii_lowercase();
        while i < n && c[i].is_whitespace() {
            i += 1;
        }
        let mut val = String::new();
        if i < n && c[i] == '=' {
            i += 1;
            while i < n && c[i].is_whitespace() {
                i += 1;
            }
            if i < n && (c[i] == '"' || c[i] == '\'') {
                let q = c[i];
                i += 1;
                let vs = i;
                while i < n && c[i] != q {
                    i += 1;
                }
                val = c[vs..i].iter().collect();
                i += 1;
            } else {
                let vs = i;
                while i < n && !c[i].is_whitespace() {
                    i += 1;
                }
                val = c[vs..i].iter().collect();
            }
        }
        out.push((key, decode_entities(&val)));
    }
    out
}

fn attr<'a>(attrs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

/// Returns `(bold, italic)` for a CSS declaration block body.
fn style_flags(decls: &str) -> (bool, bool) {
    let (mut bold, mut italic) = (false, false);
    for d in decls.split(';') {
        let Some((prop, val)) = d.split_once(':') else {
            continue;
        };
        let prop = prop.trim().to_ascii_lowercase();
        let val = val.trim().to_ascii_lowercase();
        match prop.as_str() {
            "font-weight" => bold = val == "bold" || val == "bolder" || val.parse::<u32>().is_ok_and(|w| w >= 600),
            "font-style" => italic = val == "italic" || val == "oblique",
            _ => {}
        }
    }
    (bold, italic)
}

impl Converter {
    fn parse_style(&mut self, css: &str) {
        for rule in css.split('}') {
            let Some((sels, body)) = rule.split_once('{') else {
                continue;
            };
            let (bold, italic) = style_flags(body);
            if !bold && !italic {
                continue;
            }
            for sel in sels.split(',') {
                let Some((_, class)) = sel.trim().rsplit_once('.') else {
                    continue;
                };
                let class: String = class.chars().take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_').collect();
                if class.is_empty() {
                    continue;
                }
                if bold {
                    self.bold_classes.insert(class.clone());
                }
                if italic {
                    self.italic_classes.insert(class);
                }
            }
        }
    }

    fn text(&mut self, raw: &str) {
        if self.in_head {
            return;
        }
        let decoded = decode_entities(raw);
        if self.in_pre {
            self.cur.push_str(&decoded.replace('\u{a0}', " "));
            return;
        }
        for c in decoded.chars() {
            if c == '\u{a0}' {
                self.cur.push(' ');
            } else if c.is_whitespace() {
                if !(self.cur.is_empty() || self.cur.ends_with([' ', '\n'])) {
                    self.cur.push(' ');
                }
            } else {
                self.cur.push(c);
            }
        }
    }

    fn open_mark(&mut self, mark: Mark) {
        let start = self.cur.len();
        let emitted = match &mark {
            Mark::Bold => {
                let e = self.bold_depth == 0 && self.in_heading == 0;
                self.bold_depth += 1;
                if e {
                    self.cur.push_str("**");
                }
                e
            }
            Mark::Italic => {
                let e = self.italic_depth == 0;
                self.italic_depth += 1;
                if e {
                    self.cur.push('*');
                }
                e
            }
            Mark::Code => {
                self.cur.push('`');
                true
            }
            Mark::Link(_) => {
                self.cur.push('[');
                true
            }
        };
        self.opens.push(Open { mark, start, emitted });
    }

    fn close_last_open(&mut self) {
        let Some(o) = self.opens.pop() else {
            return;
        };
        match o.mark {
            Mark::Bold => self.bold_depth = self.bold_depth.saturating_sub(1),
            Mark::Italic => self.italic_depth = self.italic_depth.saturating_sub(1),
            _ => {}
        }
        if !o.emitted || o.start > self.cur.len() || !self.cur.is_char_boundary(o.start) {
            return;
        }
        let (open, close) = match &o.mark {
            Mark::Bold => ("**", "**".to_string()),
            Mark::Italic => ("*", "*".to_string()),
            Mark::Code => ("`", "`".to_string()),
            Mark::Link(h) => ("[", format!("]({})", h.replace(' ', "%20").replace('(', "%28").replace(')', "%29"))),
        };
        let seg = self.cur.split_off(o.start);
        let inner = seg.strip_prefix(open).unwrap_or(&seg);
        let core = inner.trim();
        if core.is_empty() {
            if !self.cur.ends_with([' ', '\n']) {
                self.cur.push_str(inner);
            }
            return;
        }
        let lead = &inner[..inner.len() - inner.trim_start().len()];
        let trail = &inner[inner.trim_end().len()..];
        if !self.cur.ends_with([' ', '\n']) || self.cur.is_empty() {
            self.cur.push_str(lead);
        }
        self.cur.push_str(open);
        self.cur.push_str(core);
        self.cur.push_str(&close);
        self.cur.push_str(trail);
    }

    fn close_all_opens(&mut self) {
        while !self.opens.is_empty() {
            self.close_last_open();
        }
        for f in &mut self.frames {
            f.marks = 0;
        }
    }

    fn emit_raw(&mut self, s: &str) {
        if !self.out.is_empty() {
            self.out.push_str("\n\n");
        }
        self.out.push_str(s);
        self.last_li = false;
    }

    /// Writes the pending inline text as one Markdown block.
    fn flush(&mut self) {
        self.close_all_opens();
        if self.in_pre || self.in_cell {
            return;
        }
        let body = std::mem::take(&mut self.cur);
        let body = body.trim();
        if body.is_empty() {
            return;
        }
        let prefix = std::mem::take(&mut self.prefix);
        let is_li = !prefix.is_empty() && !prefix.starts_with('#');
        let hang = if is_li { " ".repeat(prefix.len()) } else { String::new() };
        let quote = "> ".repeat(self.quote_depth);
        let mut text = String::new();
        for (n, line) in body.lines().enumerate() {
            if n > 0 {
                text.push('\n');
            }
            text.push_str(&quote);
            text.push_str(if n == 0 { &prefix } else { &hang });
            text.push_str(line.trim_end());
        }
        let sep = if self.out.is_empty() {
            ""
        } else if is_li && self.last_li {
            "\n"
        } else {
            "\n\n"
        };
        self.out.push_str(sep);
        self.out.push_str(&text);
        self.last_li = is_li;
    }

    /// A block boundary: flush, except inside table cells (becomes a space).
    fn block_break(&mut self) {
        if self.in_pre {
            return;
        }
        if self.in_cell {
            if !self.cur.is_empty() && !self.cur.ends_with([' ', '\n']) {
                self.cur.push(' ');
            }
            return;
        }
        self.flush();
    }

    fn open_tag(&mut self, name: &str, attrs: &[(String, String)], self_closing: bool) {
        match name {
            "br" => {
                self.cur.push(if self.in_cell { ' ' } else { '\n' });
                return;
            }
            "hr" => {
                self.block_break();
                self.emit_raw("---");
                return;
            }
            "img" => {
                let src = attr(attrs, "src").unwrap_or("");
                let alt = attr(attrs, "alt").unwrap_or("");
                if !src.is_empty() {
                    self.cur.push_str(&format!("![{alt}]({src})"));
                }
                return;
            }
            "meta" | "link" | "input" | "col" | "wbr" | "base" => return,
            _ => {}
        }
        let mut marks = 0;
        match name {
            "head" => self.in_head = true,
            "p" | "div" | "section" | "article" | "header" | "footer" | "nav" | "main" | "table" | "ul" | "ol" => {
                self.block_break();
                if name == "ul" || name == "ol" {
                    self.lists.push((name == "ol", 0));
                }
            }
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                self.block_break();
                let level = usize::from(name.as_bytes()[1] - b'0');
                self.prefix = format!("{} ", "#".repeat(level));
                self.in_heading = level;
            }
            "li" => {
                self.block_break();
                let depth = self.lists.len();
                let indent = "  ".repeat(depth.saturating_sub(1));
                self.prefix = match self.lists.last_mut() {
                    Some((true, n)) => {
                        *n += 1;
                        format!("{indent}{n}. ")
                    }
                    _ => format!("{indent}- "),
                };
            }
            "blockquote" => {
                self.block_break();
                self.quote_depth += 1;
            }
            "pre" => {
                self.block_break();
                self.in_pre = true;
            }
            "tr" if !self.row.is_empty() => {
                let row = std::mem::take(&mut self.row);
                self.rows.push(row);
            }
            "td" | "th" => {
                self.cur.clear();
                self.in_cell = true;
            }
            "b" | "strong" => {
                self.open_mark(Mark::Bold);
                marks += 1;
            }
            "i" | "em" | "cite" => {
                self.open_mark(Mark::Italic);
                marks += 1;
            }
            "code" | "tt" | "kbd" if !self.in_pre => {
                self.open_mark(Mark::Code);
                marks += 1;
            }
            "a" => {
                if let Some(h) = attr(attrs, "href").filter(|h| !h.is_empty()) {
                    self.open_mark(Mark::Link(h.to_string()));
                    marks += 1;
                }
            }
            "span" => {
                let (mut bold, mut italic) = (false, false);
                for class in attr(attrs, "class").unwrap_or("").split_whitespace() {
                    bold |= self.bold_classes.contains(class);
                    italic |= self.italic_classes.contains(class);
                }
                if let Some(style) = attr(attrs, "style") {
                    let (b, i) = style_flags(style);
                    bold |= b;
                    italic |= i;
                }
                if italic {
                    self.open_mark(Mark::Italic);
                    marks += 1;
                }
                if bold {
                    self.open_mark(Mark::Bold);
                    marks += 1;
                }
            }
            _ => {}
        }
        self.frames.push(Frame { name: name.to_string(), marks });
        if self_closing {
            self.close_tag(name);
        }
    }

    fn close_tag(&mut self, name: &str) {
        let Some(pos) = self.frames.iter().rposition(|f| f.name == name) else {
            if name == "p" || name == "div" {
                self.block_break();
            }
            return;
        };
        while self.frames.len() > pos {
            let Some(frame) = self.frames.pop() else {
                break;
            };
            for _ in 0..frame.marks {
                self.close_last_open();
            }
            self.on_close(&frame.name);
        }
    }

    fn on_close(&mut self, name: &str) {
        match name {
            "head" => self.in_head = false,
            "p" | "div" | "section" | "article" | "header" | "footer" | "nav" | "main" => self.block_break(),
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                self.flush();
                self.prefix.clear();
                self.in_heading = 0;
            }
            "ul" | "ol" => {
                self.block_break();
                self.lists.pop();
                if self.lists.is_empty() {
                    self.last_li = false;
                }
            }
            "li" => {
                self.flush();
                self.prefix.clear();
            }
            "blockquote" => {
                self.flush();
                self.quote_depth = self.quote_depth.saturating_sub(1);
            }
            "pre" => {
                self.in_pre = false;
                let code = std::mem::take(&mut self.cur);
                let code = code.trim_matches('\n');
                if !code.is_empty() {
                    self.emit_raw(&format!("```\n{code}\n```"));
                }
            }
            "td" | "th" => {
                let cell = std::mem::take(&mut self.cur);
                let cell = cell.trim().replace('\n', " ").replace('|', "\\|");
                self.row.push(cell);
                self.in_cell = false;
            }
            "tr" if !self.row.is_empty() => {
                let row = std::mem::take(&mut self.row);
                self.rows.push(row);
            }
            "table" => {
                if !self.row.is_empty() {
                    let row = std::mem::take(&mut self.row);
                    self.rows.push(row);
                }
                self.in_cell = false;
                let rows = std::mem::take(&mut self.rows);
                if let Some(table) = render_table(&rows) {
                    self.emit_raw(&table);
                }
            }
            _ => {}
        }
    }

    fn finish(mut self) -> String {
        while let Some(f) = self.frames.pop() {
            for _ in 0..f.marks {
                self.close_last_open();
            }
            self.on_close(&f.name);
        }
        self.in_pre = false;
        self.in_cell = false;
        self.flush();
        let out = self.out.trim_end();
        if out.is_empty() {
            String::new()
        } else {
            format!("{out}\n")
        }
    }
}

fn render_table(rows: &[Vec<String>]) -> Option<String> {
    let cols = rows.iter().map(Vec::len).max().filter(|&c| c > 0)?;
    let line = |r: &Vec<String>| {
        let cells: Vec<&str> = (0..cols).map(|i| r.get(i).map_or("", String::as_str)).collect();
        format!("| {} |", cells.join(" | "))
    };
    let mut out = vec![line(&rows[0])];
    out.push(format!("|{}", " --- |".repeat(cols)));
    out.extend(rows[1..].iter().map(line));
    Some(out.join("\n"))
}

/// Index of the `>` closing the tag that starts at `rest[0] == '<'`,
/// ignoring `>` inside quoted attribute values.
fn find_tag_end(rest: &str) -> Option<usize> {
    let mut quote: Option<u8> = None;
    for (i, &b) in rest.as_bytes().iter().enumerate().skip(1) {
        match quote {
            Some(q) => {
                if b == q {
                    quote = None;
                }
            }
            None => match b {
                b'"' | b'\'' => quote = Some(b),
                b'>' => return Some(i),
                _ => {}
            },
        }
    }
    rest.find('>')
}

/// Converts HTML (notably the document produced by `textutil -convert html`)
/// into readable Markdown. The result ends with a newline unless empty.
pub fn html_to_markdown(html: &str) -> String {
    let mut conv = Converter::default();
    let len = html.len();
    let mut i = 0;
    while i < len {
        if html.as_bytes()[i] != b'<' {
            let j = html[i..].find('<').map_or(len, |p| i + p);
            conv.text(&html[i..j]);
            i = j;
            continue;
        }
        let rest = &html[i..];
        if rest.starts_with("<!--") {
            i = rest.find("-->").map_or(len, |p| i + p + 3);
            continue;
        }
        let tag_like = rest[1..].chars().next().is_some_and(|c| c.is_ascii_alphabetic() || matches!(c, '/' | '!' | '?'));
        let end = if tag_like { find_tag_end(rest) } else { None };
        let Some(end) = end else {
            conv.text("<");
            i += 1;
            continue;
        };
        let inner = &rest[1..end];
        i += end + 1;
        if inner.starts_with('!') || inner.starts_with('?') {
            continue;
        }
        let (closing, inner) = match inner.strip_prefix('/') {
            Some(r) => (true, r),
            None => (false, inner),
        };
        let name_end = inner.find(|c: char| !(c.is_ascii_alphanumeric() || c == '-')).unwrap_or(inner.len());
        let name = inner[..name_end].to_ascii_lowercase();
        if name.is_empty() {
            continue;
        }
        if closing {
            conv.close_tag(&name);
            continue;
        }
        if matches!(name.as_str(), "style" | "script" | "title") {
            let lower = html[i..].to_ascii_lowercase();
            let close = format!("</{name}");
            let (content_end, after) = match lower.find(&close) {
                Some(p) => (i + p, html[i + p..].find('>').map_or(len, |q| i + p + q + 1)),
                None => (len, len),
            };
            if name == "style" {
                conv.parse_style(&html[i..content_end]);
            }
            i = after;
            continue;
        }
        let attrs_src = &inner[name_end..];
        let attrs = parse_attrs(attrs_src);
        conv.open_tag(&name, &attrs, attrs_src.trim_end().ends_with('/'));
    }
    conv.finish()
}

// ───────────────────────────── Markdown parsing helpers ─────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Html,
    Plain,
}

fn push_esc(out: &mut String, c: char, mode: Mode) {
    match (mode, c) {
        (Mode::Html, '&') => out.push_str("&amp;"),
        (Mode::Html, '<') => out.push_str("&lt;"),
        (Mode::Html, '>') => out.push_str("&gt;"),
        _ => out.push(c),
    }
}

fn esc_html(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    s.chars().for_each(|c| push_esc(&mut o, c, Mode::Html));
    o
}

fn esc_attr(s: &str) -> String {
    esc_html(s).replace('"', "&quot;")
}

/// Parses `[text](url)` whose `[` is at `s[start]`. Returns
/// `(text_range, url, next_index)`.
fn parse_link(s: &[char], start: usize) -> Option<((usize, usize), String, usize)> {
    let n = s.len();
    let mut depth = 0usize;
    let mut j = start;
    let close = loop {
        if j >= n {
            return None;
        }
        match s[j] {
            '\\' => j += 1,
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    break j;
                }
            }
            _ => {}
        }
        j += 1;
    };
    if s.get(close + 1) != Some(&'(') {
        return None;
    }
    let mut depth = 1usize;
    let mut k = close + 2;
    while k < n {
        match s[k] {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
        k += 1;
    }
    if k >= n {
        return None;
    }
    let raw: String = s[close + 2..k].iter().collect();
    let url = raw.split_whitespace().next().unwrap_or("").to_string();
    Some(((start + 1, close), url, k + 1))
}

fn find_close(s: &[char], i: usize, len: usize, c: char) -> Option<usize> {
    let n = s.len();
    let after = i + len;
    if after >= n || s[after].is_whitespace() {
        return None;
    }
    if c == '_' && i > 0 && s[i - 1].is_alphanumeric() {
        return None;
    }
    let mut j = after;
    while j < n {
        if s[j] == '\\' {
            j += 2;
            continue;
        }
        if s[j] == c {
            let run = s[j..].iter().take_while(|&&x| x == c).count();
            let ok =
                run == len && j > after && !s[j - 1].is_whitespace() && (c != '_' || s.get(j + len).is_none_or(|x| !x.is_alphanumeric()));
            if ok || (run >= len && len == 3 && j > after) {
                return Some(j);
            }
            j += run;
            continue;
        }
        j += 1;
    }
    None
}

fn inline(s: &[char], mode: Mode) -> String {
    let n = s.len();
    let mut out = String::new();
    let mut i = 0;
    while i < n {
        let c = s[i];
        match c {
            '\\' if i + 1 < n && s[i + 1].is_ascii_punctuation() => {
                push_esc(&mut out, s[i + 1], mode);
                i += 2;
            }
            '`' => {
                let close = (i + 1..n).find(|&j| s[j] == '`').filter(|&j| j > i + 1);
                match close {
                    Some(j) => {
                        let code: String = s[i + 1..j].iter().collect();
                        match mode {
                            Mode::Html => out.push_str(&format!("<code>{}</code>", esc_html(&code))),
                            Mode::Plain => out.push_str(&code),
                        }
                        i = j + 1;
                    }
                    None => {
                        out.push('`');
                        i += 1;
                    }
                }
            }
            '!' | '[' => {
                let bang = c == '!';
                let open = if bang { i + 1 } else { i };
                let link = if s.get(open) == Some(&'[') { parse_link(s, open) } else { None };
                match link {
                    Some(((a, b), url, next)) => {
                        let text = inline(&s[a..b], mode);
                        match (mode, bang) {
                            (Mode::Html, false) => out.push_str(&format!("<a href=\"{}\">{}</a>", esc_attr(&url), text)),
                            (Mode::Html, true) => {
                                let alt = inline(&s[a..b], Mode::Plain);
                                out.push_str(&format!("<img src=\"{}\" alt=\"{}\">", esc_attr(&url), esc_attr(&alt)));
                            }
                            (Mode::Plain, _) => {
                                if text.is_empty() {
                                    out.push_str(&url);
                                } else if url.is_empty() {
                                    out.push_str(&text);
                                } else {
                                    out.push_str(&format!("{text} ({url})"));
                                }
                            }
                        }
                        i = next;
                    }
                    None => {
                        out.push(c);
                        i += 1;
                    }
                }
            }
            '*' | '_' => {
                let run = s[i..].iter().take_while(|&&x| x == c).count().min(3);
                let found = (1..=run).rev().find_map(|len| find_close(s, i, len, c).map(|j| (len, j)));
                match found {
                    Some((len, j)) => {
                        let inner = inline(&s[i + len..j], mode);
                        match (mode, len) {
                            (Mode::Plain, _) => out.push_str(&inner),
                            (Mode::Html, 3) => out.push_str(&format!("<strong><em>{inner}</em></strong>")),
                            (Mode::Html, 2) => out.push_str(&format!("<strong>{inner}</strong>")),
                            (Mode::Html, _) => out.push_str(&format!("<em>{inner}</em>")),
                        }
                        i = j + len;
                    }
                    None => {
                        out.push(c);
                        i += 1;
                    }
                }
            }
            _ => {
                push_esc(&mut out, c, mode);
                i += 1;
            }
        }
    }
    out
}

fn inline_str(s: &str, mode: Mode) -> String {
    inline(&s.chars().collect::<Vec<_>>(), mode)
}

fn normalize_newlines(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

fn parse_heading(t: &str) -> Option<(usize, &str)> {
    let hashes = t.chars().take_while(|&c| c == '#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = &t[hashes..];
    if !rest.is_empty() && !rest.starts_with([' ', '\t']) {
        return None;
    }
    let rest = rest.trim();
    let stripped = rest.trim_end_matches('#');
    let text = if stripped.len() != rest.len() && (stripped.is_empty() || stripped.ends_with(' ')) { stripped.trim_end() } else { rest };
    Some((hashes, text))
}

fn is_hr(t: &str) -> bool {
    let mut chars = t.chars().filter(|c| !c.is_whitespace());
    let Some(first) = chars.next() else {
        return false;
    };
    matches!(first, '-' | '*' | '_') && t.chars().all(|c| c == first || c.is_whitespace()) && {
        t.chars().filter(|&c| c == first).count() >= 3
    }
}

fn is_fence(t: &str) -> bool {
    t.starts_with("```") || t.starts_with("~~~")
}

struct ListItem<'a> {
    indent: &'a str,
    ordered: bool,
    /// Original number token, e.g. `3.`
    number: &'a str,
    value: u32,
    content: &'a str,
}

fn list_marker(line: &str) -> Option<ListItem<'_>> {
    let t = line.trim_start();
    let indent = &line[..line.len() - t.len()];
    let first = t.chars().next()?;
    if matches!(first, '-' | '*' | '+') {
        let rest = &t[1..];
        if rest.is_empty() || rest.starts_with([' ', '\t']) {
            return Some(ListItem { indent, ordered: false, number: &t[..1], value: 0, content: rest.trim() });
        }
        return None;
    }
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 || digits > 9 {
        return None;
    }
    let rest = &t[digits..];
    let delim = rest.chars().next().filter(|c| matches!(c, '.' | ')'))?;
    let after = &rest[delim.len_utf8()..];
    if !after.is_empty() && !after.starts_with([' ', '\t']) {
        return None;
    }
    Some(ListItem { indent, ordered: true, number: &t[..digits + 1], value: t[..digits].parse().unwrap_or(1), content: after.trim() })
}

// ───────────────────────────── Markdown → HTML ─────────────────────────────

fn starts_block(line: &str) -> bool {
    let t = line.trim();
    is_fence(t)
        || parse_heading(t).is_some()
        || is_hr(t)
        || t.starts_with('>')
        || list_marker(line).is_some_and(|m| !m.ordered || m.value == 1)
}

fn render_blocks(lines: &[String], out: &mut String, title: &mut Option<String>) {
    let mut i = 0;
    while i < lines.len() {
        let line = &lines[i];
        let t = line.trim();
        if t.is_empty() {
            i += 1;
        } else if is_fence(t) {
            let info = t.trim_start_matches(['`', '~']).trim();
            let mut code = Vec::new();
            i += 1;
            while i < lines.len() && !is_fence(lines[i].trim()) {
                code.push(lines[i].as_str());
                i += 1;
            }
            i += 1;
            let class = match info.split_whitespace().next() {
                Some(lang) => format!(" class=\"language-{}\"", esc_attr(lang)),
                None => String::new(),
            };
            out.push_str(&format!("<pre><code{class}>{}</code></pre>\n", esc_html(&code.join("\n"))));
        } else if let Some((level, text)) = parse_heading(t) {
            if title.is_none() {
                *title = Some(inline_str(text, Mode::Plain));
            }
            out.push_str(&format!("<h{level}>{}</h{level}>\n", inline_str(text, Mode::Html)));
            i += 1;
        } else if is_hr(t) {
            out.push_str("<hr>\n");
            i += 1;
        } else if t.starts_with('>') {
            let mut inner = Vec::new();
            while i < lines.len() && lines[i].trim_start().starts_with('>') {
                let l = lines[i].trim_start()[1..].to_string();
                inner.push(l.strip_prefix(' ').map_or(l.clone(), str::to_string));
                i += 1;
            }
            out.push_str("<blockquote>\n");
            render_blocks(&inner, out, title);
            out.push_str("</blockquote>\n");
        } else if let Some(first) = list_marker(line) {
            let ordered = first.ordered;
            let start = first.value;
            let mut items: Vec<Vec<&str>> = Vec::new();
            while i < lines.len() {
                let l = &lines[i];
                if let Some(m) = list_marker(l).filter(|m| m.ordered == ordered && !is_hr(l.trim())) {
                    items.push(vec![m.content]);
                    i += 1;
                } else if l.trim().is_empty() {
                    let next = lines[i + 1..].iter().find(|x| !x.trim().is_empty());
                    match next {
                        Some(x) if list_marker(x).is_some_and(|m| m.ordered == ordered) => i += 1,
                        _ => break,
                    }
                } else if l.starts_with([' ', '\t']) && !items.is_empty() && !starts_block(l) {
                    if let Some(last) = items.last_mut() {
                        last.push(l.trim());
                    }
                    i += 1;
                } else {
                    break;
                }
            }
            let tag = if ordered { "ol" } else { "ul" };
            if ordered && start != 1 {
                out.push_str(&format!("<ol start=\"{start}\">\n"));
            } else {
                out.push_str(&format!("<{tag}>\n"));
            }
            for item in items {
                let body: Vec<String> = item.iter().map(|l| inline_str(l, Mode::Html)).collect();
                out.push_str(&format!("<li>{}</li>\n", body.join("<br>\n")));
            }
            out.push_str(&format!("</{tag}>\n"));
        } else {
            let mut para = vec![t];
            i += 1;
            while i < lines.len() && !lines[i].trim().is_empty() && !starts_block(&lines[i]) {
                para.push(lines[i].trim());
                i += 1;
            }
            let body: Vec<String> = para.iter().map(|l| inline_str(l.strip_suffix('\\').unwrap_or(l).trim_end(), Mode::Html)).collect();
            out.push_str(&format!("<p>{}</p>\n", body.join("<br>\n")));
        }
    }
}

/// Renders Markdown as a complete minimal HTML5 document. The `<title>` is the
/// plain text of the first heading (empty when there is none).
pub fn markdown_to_html(md: &str) -> String {
    let lines: Vec<String> = normalize_newlines(md).split('\n').map(String::from).collect();
    let mut body = String::new();
    let mut title = None;
    render_blocks(&lines, &mut body, &mut title);
    format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>{}</title></head><body>\n{}</body></html>\n",
        esc_html(&title.unwrap_or_default()),
        body
    )
}

// ───────────────────────────── Markdown → plain ─────────────────────────────

fn plain_line(line: &str) -> String {
    let t = line.trim();
    if let Some(rest) = t.strip_prefix('>') {
        return plain_line(rest.trim_start());
    }
    if is_hr(t) {
        return String::new();
    }
    if let Some((_, text)) = parse_heading(t) {
        return inline_str(text, Mode::Plain);
    }
    if let Some(m) = list_marker(line) {
        let marker = if m.ordered { m.number } else { "-" };
        let content = inline_str(m.content, Mode::Plain);
        return format!("{}{marker} {content}", m.indent).trim_end().to_string();
    }
    inline_str(line.trim_end().trim_end_matches('\\'), Mode::Plain)
}

/// Strips Markdown syntax: no `#`, emphasis markers or backticks; links become
/// `text (url)`; list markers are kept as `- ` / `1. `.
pub fn markdown_to_plain(md: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut in_fence = false;
    for line in normalize_newlines(md).split('\n') {
        if is_fence(line.trim()) {
            in_fence = !in_fence;
        } else if in_fence {
            out.push(line.to_string());
        } else {
            out.push(plain_line(line));
        }
    }
    let joined = out.join("\n");
    let trimmed = joined.trim_end();
    if trimmed.is_empty() {
        String::new()
    } else {
        format!("{trimmed}\n")
    }
}

// ───────────────────────────── plain → Markdown ─────────────────────────────

fn escape_plain_line(line: &str) -> String {
    let rest = line.trim_start();
    let indent = &line[..line.len() - rest.len()];
    let ch: Vec<char> = rest.chars().collect();
    let n = ch.len();
    let has_link = rest.contains("](");
    let mut start_escaped = false;
    let mut digit_escape: Option<usize> = None;
    if let Some(&c0) = ch.first() {
        let next_blank = ch.get(1).is_none_or(|c| c.is_whitespace());
        if c0 == '#' || c0 == '>' || (matches!(c0, '-' | '+' | '*' | '_') && (next_blank || is_hr(rest))) {
            start_escaped = true;
        } else if c0.is_ascii_digit() {
            let digits = ch.iter().take_while(|c| c.is_ascii_digit()).count();
            if matches!(ch.get(digits), Some('.' | ')')) && ch.get(digits + 1).is_none_or(|c| c.is_whitespace()) {
                digit_escape = Some(digits);
            }
        }
    }
    let mut o = String::from(indent);
    if start_escaped {
        o.push('\\');
    }
    for (i, &c) in ch.iter().enumerate() {
        if digit_escape == Some(i) {
            o.push('\\');
        }
        let prev_ws = i == 0 || ch[i - 1].is_whitespace();
        let next_ws = i + 1 >= n || ch[i + 1].is_whitespace();
        let escape = if i == 0 && start_escaped {
            false
        } else {
            match c {
                '\\' => ch.get(i + 1).is_some_and(char::is_ascii_punctuation),
                '`' => true,
                '*' => !(prev_ws && next_ws),
                '_' => {
                    let intraword = i > 0 && ch[i - 1].is_alphanumeric() && i + 1 < n && ch[i + 1].is_alphanumeric();
                    !(intraword || prev_ws && next_ws)
                }
                '[' => has_link,
                '<' => ch.get(i + 1).is_some_and(|x| x.is_ascii_alphabetic() || matches!(x, '/' | '!')),
                _ => false,
            }
        };
        if escape {
            o.push('\\');
        }
        o.push(c);
    }
    o
}

/// Converts plain text into Markdown that renders as the same text: line-start
/// block syntax (`#`, `-`, `*`, `+`, `>`, `1.`) and inline emphasis / code
/// characters are backslash-escaped, blank-line paragraphs are kept and line
/// endings are normalised to `\n`.
pub fn plain_to_markdown(txt: &str) -> String {
    normalize_newlines(txt).split('\n').map(escape_plain_line).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = "# Release notes\n\n\
First paragraph with **bold** and *italic* and a [link](https://example.com/a?b=1&c=2).\n\n\
Second paragraph.\n\n\
- apples\n- pears\n- plums\n\n\
1. first\n2. second\n3. third\n";

    const TEXTUTIL: &str = r#"<!DOCTYPE html PUBLIC "-//W3C//DTD HTML 4.01//EN" "http://www.w3.org/TR/html4/strict.dtd">
<html>
<head>
<meta http-equiv="Content-Type" content="text/html; charset=UTF-8">
<meta http-equiv="Content-Style-Type" content="text/css">
<title></title>
<meta name="Generator" content="Cocoa HTML Writer">
<style type="text/css">
p.p1 {margin: 0.0px 0.0px 0.0px 0.0px; font: 12.0px Helvetica}
p.p2 {margin: 0.0px 0.0px 0.0px 0.0px; font: 12.0px Helvetica; min-height: 14.0px}
span.s1 {font-weight: bold}
span.s2 {font-style: italic}
span.s3 {font-weight: 700; font-style: italic}
span.Apple-converted-space {font: 12.0px Helvetica}
</style>
</head>
<body>
<p class="p1"><span class="s1">Meeting notes</span></p>
<p class="p2"><span class="s1"><br></span></p>
<p class="p1">Plain text, then <span class="s1">bold</span><span class="Apple-converted-space">&nbsp;</span>and <span class="s2">italic</span> &amp; <span class="s3">both</span>.</p>
<p class="p1">Tom&#39;s &lt;draft&gt; &#x41;&#66;&quot;</p>
</body>
</html>
"#;

    #[test]
    fn round_trip_fixture() {
        assert_eq!(html_to_markdown(&markdown_to_html(FIXTURE)), FIXTURE);
    }

    #[test]
    fn markdown_to_html_document_shape() {
        let html = markdown_to_html(FIXTURE);
        assert!(html.starts_with("<!DOCTYPE html><html><head><meta charset=\"utf-8\">"));
        assert!(html.contains("<title>Release notes</title>"));
        assert!(html.contains("<h1>Release notes</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<em>italic</em>"));
        assert!(html.contains("<a href=\"https://example.com/a?b=1&amp;c=2\">link</a>"));
        assert!(html.contains("<ul>\n<li>apples</li>"));
        assert!(html.contains("<ol>\n<li>first</li>"));
        assert!(html.trim_end().ends_with("</body></html>"));
        assert!(markdown_to_html("just text").contains("<title></title>"));
    }

    #[test]
    fn markdown_to_html_inline_and_blocks() {
        let html = markdown_to_html(
            "_it_ `a<b` snake_case 2 * 3\n\n```rust\nlet x = 1 < 2 && y;\n```\n\n> quoted **q**\n\n---\n\nline one\nline two\n",
        );
        assert!(html.contains("<em>it</em> <code>a&lt;b</code> snake_case 2 * 3"));
        assert!(html.contains("<pre><code class=\"language-rust\">let x = 1 &lt; 2 &amp;&amp; y;</code></pre>"));
        assert!(html.contains("<blockquote>\n<p>quoted <strong>q</strong></p>\n</blockquote>"));
        assert!(html.contains("<hr>"));
        assert!(html.contains("<p>line one<br>\nline two</p>"));
    }

    #[test]
    fn markdown_to_html_lists_and_escapes() {
        let html = markdown_to_html("* a\n+ b\n\n3. x\n4. y\n\n<script>&\n\n\\*not em\\*");
        assert!(html.contains("<ul>\n<li>a</li>\n<li>b</li>\n</ul>"));
        assert!(html.contains("<ol start=\"3\">"));
        assert!(html.contains("&lt;script&gt;&amp;"));
        assert!(html.contains("<p>*not em*</p>"));
    }

    #[test]
    fn html_basic_blocks_and_inline() {
        let md = html_to_markdown(
            "<h2>Sub</h2><p>a <b>b</b> <em>c</em> <a href=\"http://x.y\">l</a><br>next</p>\
             <ul><li>one</li><li>two</li></ul><ol><li>x</li><li>y</li></ol>",
        );
        assert_eq!(md, "## Sub\n\na **b** *c* [l](http://x.y)\nnext\n\n- one\n- two\n\n1. x\n2. y\n");
    }

    #[test]
    fn html_entities_decoded() {
        let md = html_to_markdown("<p>&amp; &lt; &gt; &quot; &#39; a&nbsp;b &#65; &#x42; &bogus; &</p>");
        assert_eq!(md, "& < > \" ' a b A B &bogus; &\n");
    }

    #[test]
    fn html_textutil_sample() {
        let md = html_to_markdown(TEXTUTIL);
        assert_eq!(md, "**Meeting notes**\n\nPlain text, then **bold** and *italic* & ***both***.\n\nTom's <draft> AB\"\n");
    }

    #[test]
    fn html_ignores_head_style_script() {
        let md = html_to_markdown(
            "<html><head><title>T</title><style>p {x:y}</style></head><body><script>var a = '<p>x</p>';</script><p>hi</p></body></html>",
        );
        assert_eq!(md, "hi\n");
    }

    #[test]
    fn html_inline_style_attribute_and_numeric_weight() {
        let md = html_to_markdown(
            "<style>.w {font-weight: 600} .n {font-weight: 400}</style>\
             <p><span class=\"w\">w</span> <span class=\"n\">n</span> <span style=\"font-style: italic\">i</span></p>",
        );
        assert_eq!(md, "**w** n *i*\n");
    }

    #[test]
    fn html_bold_whitespace_moves_outside() {
        let md = html_to_markdown("<p>a<b> bold </b>b <b> </b>c</p>");
        assert_eq!(md, "a **bold** b c\n");
    }

    #[test]
    fn html_nested_emphasis_not_doubled() {
        let md = html_to_markdown("<p><b>a <strong>b</strong></b> <h1>x</h1></p>");
        assert!(md.starts_with("**a b**"));
        let md = html_to_markdown("<h1><b>Title</b></h1>");
        assert_eq!(md, "# Title\n");
    }

    #[test]
    fn html_table_pipe() {
        let md = html_to_markdown(
            "<p>t</p><table><tr><th>A</th><th>B</th></tr><tr><td>1</td><td>x|y</td></tr><tr><td>2</td></tr></table><p>after</p>",
        );
        assert_eq!(md, "t\n\n| A | B |\n| --- | --- |\n| 1 | x\\|y |\n| 2 |  |\n\nafter\n");
    }

    #[test]
    fn html_pre_blockquote_hr_code() {
        let md = html_to_markdown("<blockquote><p>q</p></blockquote><hr><pre><code>a  b\n c</code></pre><p>use <code>x</code></p>");
        assert_eq!(md, "> q\n\n---\n\n```\na  b\n c\n```\n\nuse `x`\n");
    }

    #[test]
    fn html_malformed_never_panics() {
        for bad in [
            "<p>unclosed <b>bold",
            "a < b and c > d",
            "<",
            "<<>>",
            "<a href=\"x>text</a>",
            "</p></b></ul>stray",
            "<ul><li>a<li>b</ul>",
            "<!-- never closed",
            "<style>never closed",
            "<p>é<b>ü</i></b>",
            "<table><td>x",
            "&#99999999999; &#xZZ;",
            "",
        ] {
            let _ = html_to_markdown(bad);
        }
        assert_eq!(html_to_markdown("a < b and c > d"), "a < b and c > d\n");
        assert_eq!(html_to_markdown("<p>unclosed <b>bold"), "unclosed **bold**\n");
        assert_eq!(html_to_markdown("<ul><li>a<li>b</ul>"), "- a\n- b\n");
        assert_eq!(html_to_markdown(""), "");
    }

    #[test]
    fn markdown_to_plain_strips_syntax() {
        let plain = markdown_to_plain(
            "# Title\n\nSome **bold** and *it* and `code` with [a link](http://x.y).\n\n* one\n+ two\n3. three\n\n> quote\n\n---\n\n```\nkeep **this**\n```\n",
        );
        assert_eq!(
            plain,
            "Title\n\nSome bold and it and code with a link (http://x.y).\n\n- one\n- two\n3. three\n\nquote\n\n\n\nkeep **this**\n"
        );
        assert!(!markdown_to_plain(FIXTURE).contains(['#', '`']));
        assert!(markdown_to_plain(FIXTURE).contains("- apples\n"));
        assert!(markdown_to_plain(FIXTURE).contains("1. first\n"));
        assert_eq!(markdown_to_plain(""), "");
    }

    #[test]
    fn plain_to_markdown_escapes_line_starts() {
        assert_eq!(
            plain_to_markdown("# h\n- a\n* b\n+ c\n> q\n1. one\n12) two\nplain\n---"),
            "\\# h\n\\- a\n\\* b\n\\+ c\n\\> q\n1\\. one\n12\\) two\nplain\n\\---"
        );
    }

    #[test]
    fn plain_to_markdown_keeps_paragraphs_and_normalises_newlines() {
        assert_eq!(plain_to_markdown("a\r\nb\r\n\r\nc\rd"), "a\nb\n\nc\nd");
        assert_eq!(plain_to_markdown("-5 and 2019. done, snake_case, 2 * 3"), "-5 and 2019. done, snake_case, 2 * 3");
        assert_eq!(plain_to_markdown("*x* `y`"), "\\*x\\* \\`y\\`");
    }

    #[test]
    fn plain_markdown_plain_round_trip() {
        let txt = "# not a heading\n- not a list\n1. nope\n> nor quote\n*stars* and `ticks`";
        let back = markdown_to_plain(&plain_to_markdown(txt));
        assert_eq!(back, format!("{txt}\n"));
        let html = markdown_to_html(&plain_to_markdown(txt));
        assert!(!html.contains("<h1>") && !html.contains("<ul>") && !html.contains("<em>"));
    }
}
