//! Dynamic placeholders for Snippets and Quicklinks (`{clipboard}`, `{date}`,
//! `{argument name="q"}`, modifiers `{clipboard | uppercase}` …).

use chrono::{DateTime, Datelike, Duration, Local, Months, Timelike};
use std::collections::HashMap;

/// Where the expanded text is used (affects default escaping).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Snippet,
    /// Values of placeholders are percent-encoded so the link stays valid.
    Quicklink,
}

/// Inputs available while expanding.
#[derive(Default)]
pub struct Context<'a> {
    /// Clipboard history, most recent first.
    pub clipboard: Vec<String>,
    pub selection: Option<String>,
    /// Argument values by name (unnamed arguments use "argument", "argument2"…).
    pub arguments: HashMap<String, String>,
    /// Snippet lookup by name, for `{snippet name="…"}`.
    pub snippets: Option<&'a dyn Fn(&str) -> Option<String>>,
    /// Fixed "now" for tests.
    pub now: Option<DateTime<Local>>,
}

/// Expansion output; `cursor` is a char offset into `text` if `{cursor}` was used.
#[derive(Debug, Clone, PartialEq)]
pub struct Expanded {
    pub text: String,
    pub cursor: Option<usize>,
}

/// An argument declared inside a template.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ArgumentDef {
    pub name: String,
    pub default: Option<String>,
    pub options: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct Placeholder {
    name: String,
    attrs: Vec<(String, String)>,
    modifiers: Vec<String>,
}

impl Placeholder {
    fn attr(&self, k: &str) -> Option<&str> {
        self.attrs.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str())
    }
}

const KNOWN: &[&str] = &["clipboard", "snippet", "cursor", "date", "time", "datetime", "day", "uuid", "selection", "argument"];

/// Parse the body between `{` and `}`; returns None if it isn't a placeholder.
fn parse_body(body: &str) -> Option<Placeholder> {
    let mut parts = split_pipes(body);
    let head = parts.remove(0);
    let head = head.trim();
    let (name, rest) = match head.find(char::is_whitespace) {
        Some(i) => (&head[..i], head[i..].trim()),
        None => (head, ""),
    };
    if !KNOWN.contains(&name) {
        return None;
    }
    let attrs = parse_attrs(rest)?;
    Some(Placeholder { name: name.to_string(), attrs, modifiers: parts.into_iter().map(|m| m.trim().to_string()).collect() })
}

/// Split on `|` outside of quotes.
fn split_pipes(s: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut in_q = false;
    for c in s.chars() {
        match c {
            '"' => {
                in_q = !in_q;
                out.last_mut().unwrap().push(c);
            }
            '|' if !in_q => out.push(String::new()),
            _ => out.last_mut().unwrap().push(c),
        }
    }
    out
}

fn parse_attrs(s: &str) -> Option<Vec<(String, String)>> {
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut out = vec![];
    while i < chars.len() {
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }
        let start = i;
        while i < chars.len() && chars[i] != '=' && !chars[i].is_whitespace() {
            i += 1;
        }
        let key: String = chars[start..i].iter().collect();
        if i >= chars.len() || chars[i] != '=' {
            return None;
        }
        i += 1;
        let val: String = if chars.get(i) == Some(&'"') {
            i += 1;
            let s = i;
            while i < chars.len() && chars[i] != '"' {
                i += 1;
            }
            let v = chars[s..i].iter().collect();
            i += 1;
            v
        } else {
            let s = i;
            while i < chars.len() && !chars[i].is_whitespace() {
                i += 1;
            }
            chars[s..i].iter().collect()
        };
        out.push((key, val));
    }
    Some(out)
}

/// Tokenized template: literal text and placeholders.
enum Piece {
    Text(String),
    Ph(Placeholder),
}

fn tokenize(t: &str) -> Vec<Piece> {
    let mut out = vec![];
    let mut buf = String::new();
    let mut rest = t;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        // find matching close brace (quotes may contain braces)
        let mut in_q = false;
        let mut close = None;
        for (i, c) in after.char_indices() {
            match c {
                '"' => in_q = !in_q,
                '}' if !in_q => {
                    close = Some(i);
                    break;
                }
                '{' if !in_q => break,
                _ => {}
            }
        }
        match close.and_then(|c| parse_body(&after[..c]).map(|p| (c, p))) {
            Some((c, p)) => {
                buf.push_str(&rest[..open]);
                if !buf.is_empty() {
                    out.push(Piece::Text(std::mem::take(&mut buf)));
                }
                out.push(Piece::Ph(p));
                rest = &after[c + 1..];
            }
            None => {
                buf.push_str(&rest[..open + 1]);
                rest = after;
            }
        }
    }
    buf.push_str(rest);
    if !buf.is_empty() {
        out.push(Piece::Text(buf));
    }
    out
}

fn arg_key(p: &Placeholder, unnamed_idx: &mut usize) -> String {
    match p.attr("name") {
        Some(n) => n.to_string(),
        None => {
            *unnamed_idx += 1;
            if *unnamed_idx == 1 {
                "argument".into()
            } else {
                format!("argument{}", unnamed_idx)
            }
        }
    }
}

/// List the distinct arguments (max 3) a template asks for, in order.
pub fn arguments(template: &str) -> Vec<ArgumentDef> {
    let mut out: Vec<ArgumentDef> = vec![];
    let mut idx = 0;
    for piece in tokenize(template) {
        if let Piece::Ph(p) = piece {
            if p.name == "argument" {
                let name = arg_key(&p, &mut idx);
                if out.iter().any(|a| a.name == name) {
                    continue;
                }
                out.push(ArgumentDef {
                    name,
                    default: p.attr("default").map(String::from),
                    options: p
                        .attr("options")
                        .map(|o| o.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
                        .unwrap_or_default(),
                });
                if out.len() == 3 {
                    break;
                }
            }
        }
    }
    out
}

/// Parse `+2y +5M -3d +1h +30m`.
fn apply_offset(mut t: DateTime<Local>, off: &str) -> DateTime<Local> {
    for part in off.split_whitespace() {
        let (sign, rest) = match part.chars().next() {
            Some('+') => (1i64, &part[1..]),
            Some('-') => (-1i64, &part[1..]),
            _ => (1, part),
        };
        let Some(unit) = rest.chars().last() else { continue };
        let Ok(n) = rest[..rest.len() - unit.len_utf8()].parse::<i64>() else { continue };
        let n = n * sign;
        t = match unit {
            'm' => t + Duration::minutes(n),
            'h' => t + Duration::hours(n),
            'd' => t + Duration::days(n),
            'w' => t + Duration::weeks(n),
            'M' | 'y' => {
                let months = if unit == 'y' { n * 12 } else { n };
                if months >= 0 {
                    t.checked_add_months(Months::new(months as u32)).unwrap_or(t)
                } else {
                    t.checked_sub_months(Months::new((-months) as u32)).unwrap_or(t)
                }
            }
            _ => t,
        };
    }
    t
}

const MONTHS: [&str; 12] =
    ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const DAYS: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

/// Format with a Unicode (ICU) date pattern like `EEEE, MMM d, yyyy`.
pub fn format_unicode(t: &DateTime<Local>, pattern: &str) -> String {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' {
            // quoted literal; '' is a literal quote
            if chars.get(i + 1) == Some(&'\'') {
                out.push('\'');
                i += 2;
                continue;
            }
            i += 1;
            while i < chars.len() && chars[i] != '\'' {
                out.push(chars[i]);
                i += 1;
            }
            i += 1;
            continue;
        }
        if !c.is_ascii_alphabetic() {
            out.push(c);
            i += 1;
            continue;
        }
        let mut n = 1;
        while i + n < chars.len() && chars[i + n] == c {
            n += 1;
        }
        i += n;
        let month = t.month0() as usize;
        let wd = t.weekday().num_days_from_monday() as usize;
        let h12 = { let h = t.hour() % 12; if h == 0 { 12 } else { h } };
        let s = match (c, n) {
            ('y', 2) => format!("{:02}", t.year() % 100),
            ('y', 1) => t.year().to_string(),
            ('y', k) => format!("{:0width$}", t.year(), width = k),
            ('Q', 1) => ((month / 3) + 1).to_string(),
            ('Q', 2) => format!("{:02}", (month / 3) + 1),
            ('Q', 3) => format!("Q{}", (month / 3) + 1),
            ('Q', _) => format!("{} quarter", ["1st", "2nd", "3rd", "4th"][month / 3]),
            ('M', 1) | ('L', 1) => (month + 1).to_string(),
            ('M', 2) | ('L', 2) => format!("{:02}", month + 1),
            ('M', 3) | ('L', 3) => MONTHS[month][..3].to_string(),
            ('M', 4) | ('L', 4) => MONTHS[month].to_string(),
            ('M', _) | ('L', _) => MONTHS[month][..1].to_string(),
            ('d', 1) => t.day().to_string(),
            ('d', _) => format!("{:02}", t.day()),
            ('D', _) => t.ordinal().to_string(),
            ('F', _) => (((t.day() - 1) / 7) + 1).to_string(),
            ('E', 1..=3) => DAYS[wd][..3].to_string(),
            ('E', 4) => DAYS[wd].to_string(),
            ('E', 5) => DAYS[wd][..1].to_string(),
            ('E', _) => DAYS[wd][..2].to_string(),
            ('h', 1) => h12.to_string(),
            ('h', _) => format!("{:02}", h12),
            ('H', 1) => t.hour().to_string(),
            ('H', _) => format!("{:02}", t.hour()),
            ('a', _) => if t.hour() < 12 { "AM" } else { "PM" }.to_string(),
            ('m', 1) => t.minute().to_string(),
            ('m', _) => format!("{:02}", t.minute()),
            ('s', 1) => t.second().to_string(),
            ('s', _) => format!("{:02}", t.second()),
            ('S', k) => {
                let ms = format!("{:03}", t.timestamp_subsec_millis());
                ms.chars().chain(std::iter::repeat('0')).take(k).collect()
            }
            ('z', 1..=3) => t.format("%Z").to_string(),
            ('z', _) => t.format("%Z").to_string(),
            ('Z', 1..=3) => t.format("%z").to_string(),
            ('Z', 4) => format!("GMT{}", t.format("%:z")),
            ('Z', _) => t.format("%:z").to_string(),
            _ => std::iter::repeat(c).take(n).collect(),
        };
        out.push_str(&s);
    }
    out
}

fn apply_modifier(v: String, m: &str) -> String {
    match m {
        "uppercase" => v.to_uppercase(),
        "lowercase" => v.to_lowercase(),
        "trim" => v.trim().to_string(),
        "percent-encode" => percent_encode(&v),
        "json-stringify" => serde_json::to_string(&v).unwrap_or(v),
        "raw" => v,
        _ => v,
    }
}

/// RFC 3986 unreserved characters are kept, everything else encoded.
pub fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Expand a template.
pub fn expand(template: &str, target: Target, ctx: &Context) -> Expanded {
    let now = ctx.now.unwrap_or_else(Local::now);
    let mut text = String::new();
    let mut cursor = None;
    let mut unnamed = 0;
    for piece in tokenize(template) {
        let p = match piece {
            Piece::Text(t) => {
                text.push_str(&t);
                continue;
            }
            Piece::Ph(p) => p,
        };
        let time = p.attr("offset").map_or(now, |o| apply_offset(now, o));
        let value: String = match p.name.as_str() {
            "cursor" => {
                if cursor.is_none() {
                    cursor = Some(text.chars().count());
                }
                continue;
            }
            "clipboard" => {
                let off = p.attr("offset").and_then(|o| o.parse::<usize>().ok()).unwrap_or(0);
                ctx.clipboard.get(off).cloned().unwrap_or_default()
            }
            "selection" => ctx.selection.clone().unwrap_or_default(),
            "uuid" => uuid::Uuid::new_v4().to_string().to_uppercase(),
            "snippet" => p.attr("name").and_then(|n| ctx.snippets.and_then(|f| f(n))).unwrap_or_default(),
            "argument" => {
                let key = arg_key(&p, &mut unnamed);
                ctx.arguments.get(&key).filter(|v| !v.is_empty()).cloned().or_else(|| p.attr("default").map(String::from)).unwrap_or_default()
            }
            "date" => match p.attr("format") {
                Some(f) => format_unicode(&time, f),
                None => format_unicode(&time, "d MMM yyyy"),
            },
            "time" => match p.attr("format") {
                Some(f) => format_unicode(&time, f),
                None => format_unicode(&time, "h:mm a"),
            },
            "datetime" => match p.attr("format") {
                Some(f) => format_unicode(&time, f),
                None => format!("{} at {}", format_unicode(&time, "d MMM yyyy"), format_unicode(&time, "h:mm a")),
            },
            "day" => format_unicode(&time, "EEEE"),
            _ => unreachable!(),
        };
        let mut v = value;
        let mut raw = false;
        for m in &p.modifiers {
            if m == "raw" {
                raw = true;
            }
            v = apply_modifier(v, m);
        }
        if target == Target::Quicklink && !raw && !p.modifiers.iter().any(|m| m == "percent-encode") {
            v = percent_encode(&v);
        }
        text.push_str(&v);
    }
    Expanded { text, cursor }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn ctx() -> Context<'static> {
        Context {
            clipboard: vec!["Foo Bar".into(), "second".into()],
            now: Some(Local.with_ymd_and_hms(2022, 6, 15, 13, 44, 39).unwrap()),
            ..Default::default()
        }
    }

    #[test]
    fn clipboard_and_modifiers() {
        let c = ctx();
        assert_eq!(expand("x {clipboard} y", Target::Snippet, &c).text, "x Foo Bar y");
        assert_eq!(expand("{clipboard | uppercase}", Target::Snippet, &c).text, "FOO BAR");
        assert_eq!(expand("{clipboard offset=1}", Target::Snippet, &c).text, "second");
        assert_eq!(expand("{clipboard | percent-encode}", Target::Snippet, &c).text, "Foo%20Bar");
        assert_eq!(expand("{clipboard|json-stringify}", Target::Snippet, &c).text, "\"Foo Bar\"");
    }

    #[test]
    fn dates() {
        let c = ctx();
        assert_eq!(expand("{date}", Target::Snippet, &c).text, "15 Jun 2022");
        assert_eq!(expand("{day}", Target::Snippet, &c).text, "Wednesday");
        assert_eq!(expand("{date format=\"EEEE, MMM d, yyyy\"}", Target::Snippet, &c).text, "Wednesday, Jun 15, 2022");
        assert_eq!(expand("{date format=\"MM-dd-yyyy HH:mm\"}", Target::Snippet, &c).text, "06-15-2022 13:44");
        assert_eq!(expand("{date format=\"dd.MM.yy\"}", Target::Snippet, &c).text, "15.06.22");
        assert_eq!(expand("{date format=\"MMM d, h:mm a\"}", Target::Snippet, &c).text, "Jun 15, 1:44 PM");
        assert_eq!(expand("{date format=\"yyyy-MM-dd\" offset=\"+3M -5d\"}", Target::Snippet, &c).text, "2022-09-10");
        assert_eq!(expand("{time offset=\"+3h +30m\" format=\"HH:mm\"}", Target::Snippet, &c).text, "17:14");
        assert_eq!(expand("{date format=\"h:mm 'on the eve of' MMMM d\"}", Target::Snippet, &c).text, "1:44 on the eve of June 15");
    }

    #[test]
    fn cursor() {
        let e = expand("console.log({cursor})", Target::Snippet, &ctx());
        assert_eq!(e.text, "console.log()");
        assert_eq!(e.cursor, Some(12));
    }

    #[test]
    fn arguments_quicklink() {
        let t = "https://google.com/search?q={argument name=\"query\"}";
        assert_eq!(arguments(t), vec![ArgumentDef { name: "query".into(), ..Default::default() }]);
        let mut c = ctx();
        c.arguments.insert("query".into(), "rust lang".into());
        assert_eq!(expand(t, Target::Quicklink, &c).text, "https://google.com/search?q=rust%20lang");
    }

    #[test]
    fn argument_defaults_options() {
        let t = "{argument name=\"lang\" default=\"en\" options=\"en, es, fr\"} {argument}";
        let a = arguments(t);
        assert_eq!(a[0].options, vec!["en", "es", "fr"]);
        assert_eq!(a[1].name, "argument");
        assert_eq!(expand(t, Target::Snippet, &ctx()).text, "en ");
    }

    #[test]
    fn unknown_braces_are_literal() {
        let c = ctx();
        assert_eq!(expand("fn x() { return {a}; }", Target::Snippet, &c).text, "fn x() { return {a}; }");
    }

    #[test]
    fn snippet_ref() {
        let f = |n: &str| (n == "sig").then(|| "-- Me".to_string());
        let c = Context { snippets: Some(&f), ..ctx() };
        assert_eq!(expand("Hi\n{snippet name=\"sig\"}", Target::Snippet, &c).text, "Hi\n-- Me");
    }
}
