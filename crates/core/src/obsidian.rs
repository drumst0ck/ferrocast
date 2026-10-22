//! Obsidian: vault discovery, obsidian:// URIs, vault index and preview blocks (no UI).

use crate::fuzzy::{fuzzy_match, Sensitivity};
use std::fs;
use std::path::{Path, PathBuf};

const MAX_ENTRIES: usize = 50_000;
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_SCAN_BYTES: u64 = 20 * 1024 * 1024;
const MAX_URI_CHARS: usize = 8_000;
const MAX_PREVIEW_BYTES: usize = 200 * 1024;
const HEADING_WINDOW: usize = 4 * 1024;

// ---------------------------------------------------------------- vaults

/// A vault known to the Obsidian app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Vault {
    pub id: String,
    pub path: PathBuf,
    pub name: String,
}

/// Where Obsidian keeps its vault list.
pub fn config_path(home: &Path) -> PathBuf {
    home.join("Library/Application Support/obsidian/obsidian.json")
}

/// Parse `obsidian.json`: open vault first, then newest `ts` first. Invalid JSON → empty.
pub fn parse_vault_config(json: &str) -> Vec<Vault> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let Some(map) = value.get("vaults").and_then(|v| v.as_object()) else {
        return Vec::new();
    };
    let mut found: Vec<(bool, i64, Vault)> = Vec::new();
    for (id, entry) in map {
        let Some(path) = entry.get("path").and_then(|p| p.as_str()).filter(|p| !p.is_empty()) else {
            continue;
        };
        let path = PathBuf::from(path);
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.to_string_lossy().into_owned());
        let open = entry.get("open").and_then(|o| o.as_bool()).unwrap_or(false);
        let ts = entry.get("ts").and_then(|t| t.as_i64()).unwrap_or(0);
        found.push((open, ts, Vault { id: id.clone(), path, name }));
    }
    found.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(a.2.id.cmp(&b.2.id)));
    found.into_iter().map(|f| f.2).collect()
}

// ---------------------------------------------------------------- URIs

/// Percent-encode every byte except ASCII letters, digits and `-._~`.
pub fn encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

pub fn uri_open(vault_name: &str, file_rel: &str) -> String {
    format!("obsidian://open?vault={}&file={}", encode_component(vault_name), encode_component(file_rel))
}

pub fn uri_new(vault_name: &str, name: &str, content: &str, silent: bool) -> String {
    let mut u = format!(
        "obsidian://new?vault={}&name={}&content={}",
        encode_component(vault_name),
        encode_component(name),
        encode_component(content)
    );
    if silent {
        u.push_str("&silent=true");
    }
    u
}

pub fn uri_daily(vault_name: &str, content: &str, append: bool, silent: bool) -> String {
    let mut u = format!("obsidian://daily?vault={}&content={}", encode_component(vault_name), encode_component(content));
    if append {
        u.push_str("&append=true");
    }
    if silent {
        u.push_str("&silent=true");
    }
    u
}

pub fn uri_search(vault_name: &str, query: &str) -> String {
    format!("obsidian://search?vault={}&query={}", encode_component(vault_name), encode_component(query))
}

/// False above 8,000 characters (callers refuse instead of truncating).
pub fn uri_len_ok(uri: &str) -> bool {
    uri.chars().count() <= MAX_URI_CHARS
}

// ---------------------------------------------------------------- frontmatter

fn unquote(s: &str) -> String {
    let s = s.trim();
    for q in ['"', '\''] {
        if s.len() >= 2 && s.starts_with(q) && s.ends_with(q) {
            return s[1..s.len() - 1].to_string();
        }
    }
    s.to_string()
}

/// Split a `---` fenced block at the very start. Returns (frontmatter lines, body).
/// Without a closing fence there is no frontmatter.
fn split_frontmatter(text: &str) -> (Option<Vec<&str>>, &str) {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.split_inclusive('\n');
    let Some(first) = lines.next() else {
        return (None, text);
    };
    if first.trim_end() != "---" {
        return (None, text);
    }
    let mut offset = first.len();
    let mut fm = Vec::new();
    for line in lines {
        offset += line.len();
        let t = line.trim_end();
        if t == "---" || t == "..." {
            return (Some(fm), &text[offset..]);
        }
        fm.push(t);
    }
    (None, text)
}

fn split_inline_list(inner: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for c in inner.chars() {
        match quote {
            Some(q) if c == q => {
                quote = None;
                cur.push(c);
            }
            Some(_) => cur.push(c),
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                cur.push(c);
            }
            None if c == ',' => {
                items.push(std::mem::take(&mut cur));
            }
            None => cur.push(c),
        }
    }
    items.push(cur);
    items.iter().map(|i| unquote(i)).filter(|i| !i.is_empty()).collect()
}

fn parse_fm_fields(lines: &[&str]) -> (Option<String>, Vec<String>) {
    let mut title = None;
    let mut aliases = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        i += 1;
        let Some((key, value)) = line.split_once(':') else { continue };
        if line.starts_with(' ') || line.starts_with('\t') {
            continue;
        }
        let value = value.trim();
        match key.trim() {
            "title" => {
                let t = unquote(value);
                if !t.is_empty() {
                    title = Some(t);
                }
            }
            "aliases" => {
                if value.is_empty() {
                    while i < lines.len() {
                        let l = lines[i].trim();
                        let Some(item) = l.strip_prefix('-') else { break };
                        let item = unquote(item);
                        if !item.is_empty() {
                            aliases.push(item);
                        }
                        i += 1;
                    }
                } else if let Some(inner) = value.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
                    aliases = split_inline_list(inner);
                } else {
                    let a = unquote(value);
                    if !a.is_empty() {
                        aliases.push(a);
                    }
                }
            }
            _ => {}
        }
    }
    (title, aliases)
}

fn first_heading(body: &str) -> Option<String> {
    let mut end = body.len().min(HEADING_WINDOW);
    while !body.is_char_boundary(end) {
        end -= 1;
    }
    body[..end].lines().find_map(|l| {
        let l = l.trim_end();
        l.strip_prefix("# ").map(|h| h.trim().to_string()).filter(|h| !h.is_empty())
    })
}

// ---------------------------------------------------------------- index

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteEntry {
    pub rel_path: String,
    pub title: String,
    pub aliases: Vec<String>,
    /// Unix seconds.
    pub modified: u64,
    pub size: u64,
}

fn is_md(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("md"))
}

fn build_entry(root: &Path, path: &Path, size: u64, modified: u64) -> Option<NoteEntry> {
    let rel = path.strip_prefix(root).ok()?;
    let rel_path = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let (mut title, mut aliases) = (None, Vec::new());
    if let Ok(bytes) = fs::read(path) {
        let text = String::from_utf8_lossy(&bytes);
        let (fm, body) = split_frontmatter(&text);
        if let Some(fm) = fm {
            let (t, a) = parse_fm_fields(&fm);
            title = t;
            aliases = a;
        }
        if title.is_none() {
            title = first_heading(body);
        }
    }
    Some(NoteEntry { rel_path, title: title.unwrap_or(stem), aliases, modified, size })
}

/// Walk the vault for Markdown notes. Never panics; unreadable entries are skipped
/// (or indexed by file stem when only the content cannot be read).
pub fn index_vault(root: &Path) -> Vec<NoteEntry> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        let mut children: Vec<_> = rd.flatten().collect();
        children.sort_by_key(|c| c.file_name());
        let mut subdirs = Vec::new();
        for child in children {
            let name = child.file_name();
            let Ok(ft) = child.file_type() else { continue };
            let path = child.path();
            if ft.is_dir() {
                if !name.to_string_lossy().starts_with('.') {
                    subdirs.push(path);
                }
                continue;
            }
            if !is_md(&path) {
                continue;
            }
            // A symlink counts only when it points at a regular file.
            let Ok(meta) = fs::metadata(&path) else { continue };
            if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
                continue;
            }
            let modified =
                meta.modified().ok().and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
            if let Some(e) = build_entry(root, &path, meta.len(), modified) {
                out.push(e);
                if out.len() >= MAX_ENTRIES {
                    out.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
                    return out;
                }
            }
        }
        // Reverse so the stack pops in name order.
        stack.extend(subdirs.into_iter().rev());
    }
    out.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    out
}

// ---------------------------------------------------------------- search

fn title_score(e: &NoteEntry, query: &str) -> Option<i64> {
    std::iter::once(e.title.as_str())
        .chain(e.aliases.iter().map(|a| a.as_str()))
        .filter_map(|c| fuzzy_match(query, c, Sensitivity::Medium))
        .map(|m| m.score)
        .max()
}

/// Ranked indices: title/alias matches, then path matches; empty query → newest first.
pub fn search(entries: &[NoteEntry], query: &str, limit: usize) -> Vec<usize> {
    if query.trim().is_empty() {
        let mut idx: Vec<usize> = (0..entries.len()).collect();
        idx.sort_by(|&a, &b| entries[b].modified.cmp(&entries[a].modified).then(a.cmp(&b)));
        idx.truncate(limit);
        return idx;
    }
    let mut titled: Vec<(i64, usize)> = Vec::new();
    let mut pathed: Vec<(i64, usize)> = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        if let Some(s) = title_score(e, query) {
            titled.push((s, i));
        } else if let Some(m) = fuzzy_match(query, &e.rel_path, Sensitivity::Medium) {
            pathed.push((m.score, i));
        }
    }
    let by_score = |a: &(i64, usize), b: &(i64, usize)| b.0.cmp(&a.0).then(a.1.cmp(&b.1));
    titled.sort_by(by_score);
    pathed.sort_by(by_score);
    titled.into_iter().chain(pathed).map(|(_, i)| i).take(limit).collect()
}

/// Case-insensitive substring scan of note contents (3+ chars), bounded to `limit`
/// hits and 20 MB read. Entries that already match by title/alias are skipped.
pub fn content_hits(root: &Path, entries: &[NoteEntry], query: &str, limit: usize) -> Vec<usize> {
    let q = query.trim().to_lowercase();
    if q.chars().count() < 3 || limit == 0 {
        return Vec::new();
    }
    let mut hits = Vec::new();
    let mut scanned = 0u64;
    for (i, e) in entries.iter().enumerate() {
        if hits.len() >= limit || scanned >= MAX_SCAN_BYTES {
            break;
        }
        if title_score(e, query.trim()).is_some() {
            continue;
        }
        let Ok(bytes) = fs::read(root.join(&e.rel_path)) else { continue };
        scanned += bytes.len() as u64;
        if String::from_utf8_lossy(&bytes).to_lowercase().contains(&q) {
            hits.push(i);
        }
    }
    hits
}

// ---------------------------------------------------------------- preview

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Heading(u8, String),
    Paragraph(String),
    Bullet(u8, String),
    Ordered(u8, usize, String),
    Task(u8, bool, String),
    Quote(String),
    Code(Option<String>, String),
    Rule,
}

fn remove_between(s: &str, open: &str, close: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(a) = rest.find(open) {
        let after = &rest[a + open.len()..];
        match after.find(close) {
            Some(b) => {
                out.push_str(&rest[..a]);
                rest = &after[b + close.len()..];
            }
            None => break,
        }
    }
    out.push_str(rest);
    out
}

fn wikilink_text(inner: &str) -> String {
    if let Some((_, alias)) = inner.split_once('|') {
        return alias.trim().to_string();
    }
    match inner.split_once('#') {
        Some((note, heading)) if note.trim().is_empty() => heading.trim().to_string(),
        Some((note, heading)) => format!("{} > {}", note.trim(), heading.trim()),
        None => inner.trim().to_string(),
    }
}

fn clean_inline(s: &str) -> String {
    let s = remove_between(s, "%%", "%%");
    let mut out = String::new();
    let mut rest = s.as_str();
    while let Some(a) = rest.find("[[") {
        let Some(b) = rest[a + 2..].find("]]") else { break };
        let inner = &rest[a + 2..a + 2 + b];
        if rest[..a].ends_with('!') {
            out.push_str(&rest[..a - 1]);
            out.push('[');
            out.push_str(inner);
            out.push(']');
        } else {
            out.push_str(&rest[..a]);
            out.push_str(&wikilink_text(inner));
        }
        rest = &rest[a + 2 + b + 2..];
    }
    out.push_str(rest);
    out.replace("==", "")
}

fn indent_level(line: &str) -> (u8, &str) {
    let mut spaces = 0usize;
    let mut tabs = 0usize;
    let mut end = 0;
    for (i, c) in line.char_indices() {
        match c {
            ' ' => spaces += 1,
            '\t' => tabs += 1,
            _ => {
                end = i;
                break;
            }
        }
        end = i + 1;
    }
    ((tabs + spaces / 2).min(255) as u8, &line[end..])
}

fn is_rule(t: &str) -> bool {
    let t: String = t.chars().filter(|c| !c.is_whitespace()).collect();
    t.len() >= 3 && (t.chars().all(|c| c == '-') || t.chars().all(|c| c == '*') || t.chars().all(|c| c == '_'))
}

fn fence_of(t: &str) -> Option<(char, usize, &str)> {
    let c = t.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let n = t.chars().take_while(|x| *x == c).count();
    (n >= 3).then(|| (c, n, t[n..].trim()))
}

fn list_item(t: &str) -> Option<&str> {
    ["- ", "* ", "+ "].iter().find_map(|p| t.strip_prefix(p)).or(if matches!(t, "-" | "*" | "+") { Some("") } else { None })
}

fn ordered_item(t: &str) -> Option<(usize, &str)> {
    let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits == 0 || digits > 9 {
        return None;
    }
    let rest = t[digits..].strip_prefix(". ")?;
    Some((t[..digits].parse().ok()?, rest))
}

/// Parse Markdown into preview blocks. Input is capped at 200 KB.
pub fn parse_preview(markdown: &str) -> Vec<Block> {
    let mut end = markdown.len().min(MAX_PREVIEW_BYTES);
    while !markdown.is_char_boundary(end) {
        end -= 1;
    }
    let (_, body) = split_frontmatter(&markdown[..end]);
    let mut blocks = Vec::new();
    let mut para: Vec<String> = Vec::new();
    let mut quote: Vec<String> = Vec::new();

    fn flush(blocks: &mut Vec<Block>, para: &mut Vec<String>, quote: &mut Vec<String>) {
        if !para.is_empty() {
            blocks.push(Block::Paragraph(para.join(" ")));
            para.clear();
        }
        if !quote.is_empty() {
            blocks.push(Block::Quote(quote.join(" ")));
            quote.clear();
        }
    }

    let mut lines = body.lines().map(|l| l.trim_end_matches('\r')).peekable();
    while let Some(line) = lines.next() {
        let (level, t) = indent_level(line);
        if t.is_empty() {
            flush(&mut blocks, &mut para, &mut quote);
            continue;
        }
        if let Some((fc, n, lang)) = fence_of(t) {
            flush(&mut blocks, &mut para, &mut quote);
            let mut code = Vec::new();
            for l in lines.by_ref() {
                let lt = l.trim();
                if lt.chars().all(|c| c == fc) && lt.len() >= n {
                    break;
                }
                code.push(l);
            }
            let lang = (!lang.is_empty()).then(|| lang.to_string());
            blocks.push(Block::Code(lang, code.join("\n")));
            continue;
        }
        if let Some(q) = t.strip_prefix('>') {
            if !para.is_empty() {
                flush(&mut blocks, &mut para, &mut quote);
            }
            quote.push(clean_inline(q.trim()));
            continue;
        }
        let hashes = t.chars().take_while(|c| *c == '#').count();
        if (1..=6).contains(&hashes) && (t.len() == hashes || t[hashes..].starts_with(' ')) {
            flush(&mut blocks, &mut para, &mut quote);
            blocks.push(Block::Heading(hashes as u8, clean_inline(t[hashes..].trim())));
            continue;
        }
        if is_rule(t) {
            flush(&mut blocks, &mut para, &mut quote);
            blocks.push(Block::Rule);
            continue;
        }
        if let Some(item) = list_item(t) {
            flush(&mut blocks, &mut para, &mut quote);
            let task = [("[ ] ", false), ("[x] ", true), ("[X] ", true)].iter().find_map(|(p, d)| item.strip_prefix(p).map(|r| (*d, r)));
            blocks.push(match task {
                Some((done, rest)) => Block::Task(level, done, clean_inline(rest.trim())),
                None => Block::Bullet(level, clean_inline(item.trim())),
            });
            continue;
        }
        if let Some((n, rest)) = ordered_item(t) {
            flush(&mut blocks, &mut para, &mut quote);
            blocks.push(Block::Ordered(level, n, clean_inline(rest.trim())));
            continue;
        }
        if !quote.is_empty() {
            flush(&mut blocks, &mut para, &mut quote);
        }
        para.push(clean_inline(t.trim()));
    }
    flush(&mut blocks, &mut para, &mut quote);
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static N: AtomicUsize = AtomicUsize::new(0);

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("ferrocast-obsidian-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn write(root: &Path, rel: &str, body: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, body).unwrap();
    }

    fn entry(path: &str, title: &str, aliases: &[&str], modified: u64) -> NoteEntry {
        NoteEntry {
            rel_path: path.into(),
            title: title.into(),
            aliases: aliases.iter().map(|s| s.to_string()).collect(),
            modified,
            size: 1,
        }
    }

    // ---- vaults

    #[test]
    fn config_path_is_under_application_support() {
        assert_eq!(config_path(Path::new("/Users/me")), PathBuf::from("/Users/me/Library/Application Support/obsidian/obsidian.json"));
    }

    #[test]
    fn vaults_sorted_open_first_then_ts_desc_and_pathless_ignored() {
        let json = r#"{"vaults":{
            "a":{"path":"/v/Old","ts":100},
            "b":{"path":"/v/New","ts":300},
            "c":{"path":"/v/Open","ts":50,"open":true},
            "d":{"ts":999},
            "e":{"path":"","ts":999}}}"#;
        let v = parse_vault_config(json);
        let got: Vec<(&str, &str, &str)> = v.iter().map(|v| (v.id.as_str(), v.name.as_str(), v.path.to_str().unwrap())).collect();
        assert_eq!(got, vec![("c", "Open", "/v/Open"), ("b", "New", "/v/New"), ("a", "Old", "/v/Old")]);
    }

    #[test]
    fn vault_config_garbage_is_empty() {
        assert!(parse_vault_config("not json").is_empty());
        assert!(parse_vault_config("{}").is_empty());
        assert!(parse_vault_config(r#"{"vaults":[]}"#).is_empty());
    }

    // ---- URIs

    #[test]
    fn uri_open_encodes_space_slash_and_unicode() {
        assert_eq!(uri_open("My Vault", "Dir/Ñandú ☕.md"), "obsidian://open?vault=My%20Vault&file=Dir%2F%C3%91and%C3%BA%20%E2%98%95.md");
    }

    #[test]
    fn unreserved_kept_everything_else_encoded() {
        assert_eq!(encode_component("aZ09-._~"), "aZ09-._~");
        assert_eq!(encode_component("a+b&c=d?e#f%g\n"), "a%2Bb%26c%3Dd%3Fe%23f%25g%0A");
    }

    #[test]
    fn uri_new_silent_flag() {
        assert_eq!(uri_new("V", "A b", "x/y", true), "obsidian://new?vault=V&name=A%20b&content=x%2Fy&silent=true");
        assert_eq!(uri_new("V", "A", "", false), "obsidian://new?vault=V&name=A&content=");
    }

    #[test]
    fn uri_daily_flags() {
        assert_eq!(uri_daily("V", "- hi", true, true), "obsidian://daily?vault=V&content=-%20hi&append=true&silent=true");
        assert_eq!(uri_daily("V", "hi", true, false), "obsidian://daily?vault=V&content=hi&append=true");
        assert_eq!(uri_daily("V", "hi", false, true), "obsidian://daily?vault=V&content=hi&silent=true");
        assert_eq!(uri_daily("V", "hi", false, false), "obsidian://daily?vault=V&content=hi");
    }

    #[test]
    fn uri_search_and_length() {
        assert_eq!(uri_search("My V", "a b"), "obsidian://search?vault=My%20V&query=a%20b");
        assert!(uri_len_ok(&"a".repeat(8000)));
        assert!(!uri_len_ok(&"a".repeat(8001)));
    }

    // ---- index

    #[test]
    fn index_walks_nested_unicode_and_skips_dot_dirs() {
        let r = tmp();
        write(&r, "Top.md", "hello");
        write(&r, "Proyectos/Ñandú/Idea ☕.MD", "# Gran Idea\ntext");
        write(&r, ".obsidian/workspace.md", "x");
        write(&r, ".trash/gone.md", "x");
        write(&r, ".git/readme.md", "x");
        write(&r, "notes.txt", "x");
        let e = index_vault(&r);
        let got: Vec<(&str, &str)> = e.iter().map(|e| (e.rel_path.as_str(), e.title.as_str())).collect();
        assert_eq!(got, vec![("Proyectos/Ñandú/Idea ☕.MD", "Gran Idea"), ("Top.md", "Top")]);
        assert_eq!(e[1].size, 5);
        assert!(e[1].modified > 1_600_000_000);
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn index_survives_symlink_loop_and_unreadable_file() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let r = tmp();
        write(&r, "a/one.md", "# One");
        symlink(&r, r.join("a/loop")).unwrap();
        write(&r, "locked.md", "# Secret");
        fs::set_permissions(r.join("locked.md"), fs::Permissions::from_mode(0o000)).unwrap();
        let e = index_vault(&r);
        let got: Vec<(&str, &str)> = e.iter().map(|e| (e.rel_path.as_str(), e.title.as_str())).collect();
        assert!(got.contains(&("a/one.md", "One")));
        assert_eq!(got.iter().filter(|g| g.0.ends_with("one.md")).count(), 1);
        assert!(got.iter().all(|g| !g.0.contains("loop")));
        // Unreadable content falls back to the file stem (or is skipped when root can read it).
        assert!(got.contains(&("locked.md", "locked")) || got.contains(&("locked.md", "Secret")));
        fs::set_permissions(r.join("locked.md"), fs::Permissions::from_mode(0o644)).unwrap();
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn index_ignores_files_over_2mb() {
        let r = tmp();
        write(&r, "big.md", &"a".repeat(3 * 1024 * 1024));
        write(&r, "edge.md", &"a".repeat(2 * 1024 * 1024));
        write(&r, "small.md", "x");
        let got: Vec<String> = index_vault(&r).into_iter().map(|e| e.rel_path).collect();
        assert_eq!(got, vec!["edge.md", "small.md"]);
        let _ = fs::remove_dir_all(&r);
    }

    fn title_of(body: &str) -> (String, Vec<String>) {
        let r = tmp();
        write(&r, "File Name.md", body);
        let e = index_vault(&r).remove(0);
        let _ = fs::remove_dir_all(&r);
        (e.title, e.aliases)
    }

    #[test]
    fn title_precedence_and_frontmatter_variants() {
        assert_eq!(title_of("---\ntitle: FM Title\n---\n# Heading\n").0, "FM Title");
        assert_eq!(title_of("---\ntitle: \"Quoted: Title\"\n---\n# Heading\n").0, "Quoted: Title");
        assert_eq!(title_of("---\ntitle: 'Single'\n---\n").0, "Single");
        assert_eq!(title_of("---\ntags: x\n---\n\nintro\n# Heading\n").0, "Heading");
        assert_eq!(title_of("no heading here\n## Sub\n").0, "File Name");
        assert_eq!(title_of("").0, "File Name");
        // Heading beyond the first 4 KB is not used.
        assert_eq!(title_of(&format!("{}\n# Late\n", "x".repeat(5000))).0, "File Name");
        // Heading must be level 1.
        assert_eq!(title_of("## Two\n").0, "File Name");
    }

    #[test]
    fn frontmatter_crlf_and_missing_or_unclosed_fence() {
        assert_eq!(title_of("---\r\ntitle: Win\r\naliases: [a, b]\r\n---\r\nbody\r\n"), ("Win".into(), vec!["a".into(), "b".into()]));
        // Unclosed fence: no frontmatter, so the heading wins? There is none: stem.
        assert_eq!(title_of("---\ntitle: Nope\nbody").0, "File Name");
        // Fence not at the very start.
        assert_eq!(title_of("\n---\ntitle: Nope\n---\n").0, "File Name");
        // Dots closing fence and BOM.
        assert_eq!(title_of("\u{feff}---\ntitle: Dots\n...\nx").0, "Dots");
    }

    #[test]
    fn aliases_inline_list_and_single() {
        assert_eq!(title_of("---\naliases: [\"A, one\", 'B', c]\n---\n").1, vec!["A, one", "B", "c"]);
        assert_eq!(
            title_of("---\naliases:\n  - First\n  - \"Second\"\n- Third\ntitle: T\n---\n"),
            ("T".into(), vec!["First".into(), "Second".into(), "Third".into()])
        );
        assert_eq!(title_of("---\naliases: Solo\n---\n").1, vec!["Solo"]);
        assert_eq!(title_of("---\naliases: []\n---\n").1, Vec::<String>::new());
        assert_eq!(title_of("---\naliases:\n---\n").1, Vec::<String>::new());
    }

    // ---- search

    #[test]
    fn search_empty_query_is_newest_first_with_limit() {
        let e = vec![entry("a.md", "A", &[], 10), entry("b.md", "B", &[], 30), entry("c.md", "C", &[], 20)];
        assert_eq!(search(&e, "", 10), vec![1, 2, 0]);
        assert_eq!(search(&e, "  ", 2), vec![1, 2]);
    }

    #[test]
    fn search_ranks_titles_and_aliases_before_paths() {
        let e = vec![
            entry("projects/meeting/x.md", "Unrelated", &[], 1), // path only
            entry("m.md", "Weekly meeting", &[], 1),             // title substring
            entry("n.md", "Notes", &["Meeting minutes"], 1),     // alias
            entry("zzz.md", "Nothing", &[], 1),                  // no match
            entry("o.md", "Meeting", &[], 1),                    // exact: best
        ];
        assert_eq!(search(&e, "meeting", 10), vec![4, 2, 1, 0]);
        assert_eq!(search(&e, "meeting", 2), vec![4, 2]);
        assert_eq!(search(&e, "qqq", 10), Vec::<usize>::new());
    }

    #[test]
    fn content_hits_bounded_and_skip_title_matches() {
        let r = tmp();
        write(&r, "a.md", "The Quick brown fox");
        write(&r, "b.md", "quick again");
        write(&r, "c.md", "nothing");
        write(&r, "quick title.md", "quick inside too");
        write(&r, "d.md", "QUICK third");
        let e = index_vault(&r);
        let names: Vec<&str> = e.iter().map(|e| e.rel_path.as_str()).collect();
        assert_eq!(names, vec!["a.md", "b.md", "c.md", "d.md", "quick title.md"]);
        assert_eq!(content_hits(&r, &e, "quick", 10), vec![0, 1, 3]);
        assert_eq!(content_hits(&r, &e, "QuIcK", 2), vec![0, 1]);
        assert_eq!(content_hits(&r, &e, "qu", 10), Vec::<usize>::new());
        assert_eq!(content_hits(&r, &e, "quick", 0), Vec::<usize>::new());
        let _ = fs::remove_dir_all(&r);
    }

    #[test]
    fn content_hits_stops_after_20mb_and_tolerates_missing_files() {
        let r = tmp();
        let filler = "a".repeat(2 * 1024 * 1024 - 100);
        let mut e = vec![entry("gone.md", "gone", &[], 0)];
        for i in 0..11 {
            let rel = format!("f{i:02}.md");
            write(&r, &rel, &filler);
            e.push(entry(&rel, &format!("f{i:02}"), &[], 0));
        }
        write(&r, "late.md", "needle here");
        e.push(entry("late.md", "late", &[], 0));
        // 11 files of ~2 MB = ~22 MB scanned before `late.md`: budget exhausted.
        assert_eq!(content_hits(&r, &e, "needle", 5), Vec::<usize>::new());
        let _ = fs::remove_dir_all(&r);
    }

    // ---- preview

    use Block::*;

    fn s(x: &str) -> String {
        x.to_string()
    }

    #[test]
    fn preview_hides_frontmatter_and_parses_headings_paragraphs() {
        let md = "---\ntitle: X\n---\n# One\n\nline a\nline b\n\n###### Six\n####### seven\n#nospace\n";
        assert_eq!(
            parse_preview(md),
            vec![Heading(1, s("One")), Paragraph(s("line a line b")), Heading(6, s("Six")), Paragraph(s("####### seven #nospace"))]
        );
    }

    #[test]
    fn preview_lists_tasks_and_nesting() {
        let md = "- a\n  * b\n\t+ c\n1. one\n   2. two\n- [ ] todo\n  - [x] done\n- [X] Done\n";
        assert_eq!(
            parse_preview(md),
            vec![
                Bullet(0, s("a")),
                Bullet(1, s("b")),
                Bullet(1, s("c")),
                Ordered(0, 1, s("one")),
                Ordered(1, 2, s("two")),
                Task(0, false, s("todo")),
                Task(1, true, s("done")),
                Task(0, true, s("Done")),
            ]
        );
    }

    #[test]
    fn preview_quotes_code_and_rules() {
        let md = "> q1\n> q2\n\n```rust\nfn main() {}\n\n  x\n```\n~~~\nplain\n~~~\n---\n***\ntext\n```\nunterminated";
        assert_eq!(
            parse_preview(md),
            vec![
                Quote(s("q1 q2")),
                Code(Some(s("rust")), s("fn main() {}\n\n  x")),
                Code(None, s("plain")),
                Rule,
                Rule,
                Paragraph(s("text")),
                Code(None, s("unterminated")),
            ]
        );
    }

    #[test]
    fn preview_code_content_is_not_interpreted() {
        assert_eq!(parse_preview("```\n# not heading\n[[x]]\n```"), vec![Code(None, s("# not heading\n[[x]]"))]);
    }

    #[test]
    fn preview_inline_wikilinks_embeds_highlights_comments() {
        assert_eq!(
            parse_preview("See [[Note|alias]] and [[Note]] and [[Note#Head]] and [[#Only]] ![[pic.png]] ==marked== %%hidden%% end"),
            vec![Paragraph(s("See alias and Note and Note > Head and Only [pic.png] marked  end"))]
        );
        assert_eq!(parse_preview("- [[A|b]] ![[c]]"), vec![Bullet(0, s("b [c]"))]);
        assert_eq!(parse_preview("unclosed [[x and %%y"), vec![Paragraph(s("unclosed [[x and %%y"))]);
    }

    #[test]
    fn preview_crlf_and_passthrough_and_cap() {
        assert_eq!(
            parse_preview("# T\r\n\r\ntext **bold** `code` [l](http://x)\r\n"),
            vec![Heading(1, s("T")), Paragraph(s("text **bold** `code` [l](http://x)"))]
        );
        assert_eq!(parse_preview(""), Vec::<Block>::new());
        let big = format!("{}\n# After", "a\n".repeat(150 * 1024));
        let blocks = parse_preview(&big);
        assert_eq!(blocks.len(), 1);
        let Paragraph(p) = &blocks[0] else { panic!() };
        assert!(p.len() <= 200 * 1024);
    }
}
