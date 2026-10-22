//! Notes: store logic for the floating Notes window. Pure functions, no I/O, no clock.

use crate::markdown::{markdown_to_html, markdown_to_plain};
use crate::models::Note;

/// Days a note stays in Recently Deleted before `purge` removes it.
/// This retention period is Ferrocast's own choice, not taken from Raycast.
pub const TRASH_DAYS: i64 = 30;

const DAY: i64 = 86_400;

fn find_mut<'a>(notes: &'a mut [Note], id: &str) -> Option<&'a mut Note> {
    notes.iter_mut().find(|n| n.id == id)
}

/// Appends an empty note and returns its id.
pub fn create(notes: &mut Vec<Note>, id: &str, now: i64) -> String {
    notes.push(Note { id: id.into(), text: String::new(), created: now, modified: now, pinned_at: None, deleted_at: None });
    id.into()
}

/// Updates the text; `modified` changes only when the text really changed.
pub fn set_text(notes: &mut [Note], id: &str, text: &str, now: i64) {
    if let Some(n) = find_mut(notes, id) {
        if n.text != text {
            n.text = text.into();
            n.modified = now;
        }
    }
}

/// First non-blank line without Markdown markers, at most 80 chars; "Untitled" when empty.
pub fn display_title(note: &Note) -> String {
    let line = note.text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let mut s = line.trim().to_string();
    loop {
        let t = s.trim_start();
        let rest = strip_block_marker(t);
        match rest {
            Some(r) => s = r.to_string(),
            None => break,
        }
    }
    let s = strip_links(&s);
    let mut s = s;
    for m in ["**", "__", "~~", "`", "<u>", "</u>", "*", "_"] {
        s = s.replace(m, "");
    }
    let t: String = s.trim().chars().take(80).collect();
    let t = t.trim().to_string();
    if t.is_empty() {
        "Untitled".into()
    } else {
        t
    }
}

fn strip_block_marker(t: &str) -> Option<&str> {
    if t.starts_with('#') {
        return Some(t.trim_start_matches('#'));
    }
    for p in ["- [ ]", "- [x]", "- [X]", "* [ ]", "* [x]", "* [X]", "[ ]", "[x]", "[X]"] {
        if let Some(r) = t.strip_prefix(p) {
            return Some(r);
        }
    }
    if let Some(r) = t.strip_prefix('>') {
        return Some(r);
    }
    for p in ["- ", "* ", "+ "] {
        if let Some(r) = t.strip_prefix(p) {
            return Some(r);
        }
    }
    let digits = t.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 {
        if let Some(r) = t[digits..].strip_prefix(". ") {
            return Some(r);
        }
    }
    None
}

/// `[text](url)` → `text`.
fn strip_links(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('[') {
        let after = &rest[i + 1..];
        if let Some(j) = after.find("](") {
            if let Some(k) = after[j + 2..].find(')') {
                out.push_str(&rest[..i]);
                out.push_str(&after[..j]);
                rest = &after[j + 2 + k + 1..];
                continue;
            }
        }
        out.push_str(&rest[..=i]);
        rest = after;
    }
    out.push_str(rest);
    out
}

/// Pins or unpins. Pinning an already pinned note keeps its slot.
pub fn set_pinned(notes: &mut [Note], id: &str, pinned: bool, now: i64) {
    if let Some(n) = find_mut(notes, id) {
        if pinned {
            if n.pinned_at.is_none() && n.deleted_at.is_none() {
                n.pinned_at = Some(now);
            }
        } else {
            n.pinned_at = None;
        }
    }
}

/// Id of the nth (1..=9) pinned, non-deleted note in pin order.
pub fn pinned_slot(notes: &[Note], n: usize) -> Option<String> {
    if !(1..=9).contains(&n) {
        return None;
    }
    let mut p: Vec<&Note> = notes.iter().filter(|x| x.deleted_at.is_none() && x.pinned_at.is_some()).collect();
    p.sort_by_key(|x| x.pinned_at);
    p.get(n - 1).map(|x| x.id.clone())
}

/// Visible notes: pinned first (by `pinned_at`), then by `modified` descending.
/// A non-empty query matches title or content case-insensitively; title matches come first.
pub fn browse<'a>(notes: &'a [Note], query: &str) -> Vec<&'a Note> {
    let q = query.trim().to_lowercase();
    let mut v: Vec<(&Note, u8)> = notes
        .iter()
        .filter(|n| n.deleted_at.is_none())
        .filter_map(|n| {
            if q.is_empty() || display_title(n).to_lowercase().contains(&q) {
                Some((n, 0))
            } else if n.text.to_lowercase().contains(&q) {
                Some((n, 1))
            } else {
                None
            }
        })
        .collect();
    v.sort_by(|(a, ra), (b, rb)| match (a.pinned_at, b.pinned_at) {
        (Some(x), Some(y)) => x.cmp(&y).then(ra.cmp(rb)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => ra.cmp(rb).then(b.modified.cmp(&a.modified)),
    });
    v.into_iter().map(|(n, _)| n).collect()
}

fn trash(n: &mut Note, now: i64) {
    if n.deleted_at.is_none() {
        n.deleted_at = Some(now);
    }
    n.pinned_at = None;
}

/// Moves a note to Recently Deleted.
pub fn delete(notes: &mut [Note], id: &str, now: i64) {
    if let Some(n) = find_mut(notes, id) {
        trash(n, now);
    }
}

pub fn delete_all(notes: &mut [Note], now: i64) {
    notes.iter_mut().for_each(|n| trash(n, now));
}

pub fn restore(notes: &mut [Note], id: &str, _now: i64) {
    if let Some(n) = find_mut(notes, id) {
        n.deleted_at = None;
    }
}

pub fn delete_forever(notes: &mut Vec<Note>, id: &str) {
    notes.retain(|n| n.id != id);
}

/// Removes notes deleted more than `TRASH_DAYS` ago.
pub fn purge(notes: &mut Vec<Note>, now: i64) {
    notes.retain(|n| n.deleted_at.is_none_or(|d| now - d <= TRASH_DAYS * DAY));
}

/// Deleted notes, newest deletion first.
pub fn recently_deleted(notes: &[Note]) -> Vec<&Note> {
    let mut v: Vec<&Note> = notes.iter().filter(|n| n.deleted_at.is_some()).collect();
    v.sort_by_key(|n| std::cmp::Reverse(n.deleted_at));
    v
}

/// Back/forward history of opened notes, like a browser.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct History {
    items: Vec<String>,
    pos: usize,
}

impl History {
    pub fn current(&self) -> Option<&str> {
        self.items.get(self.pos).map(String::as_str)
    }

    /// Opens a note: drops the forward part; a repeat of the current id is ignored.
    pub fn visit(&mut self, id: &str) {
        if self.current() == Some(id) {
            return;
        }
        if !self.items.is_empty() {
            self.items.truncate(self.pos + 1);
        }
        self.items.push(id.into());
        self.pos = self.items.len() - 1;
    }

    pub fn back(&mut self) -> Option<&str> {
        if self.pos == 0 {
            return None;
        }
        self.pos -= 1;
        self.current()
    }

    pub fn forward(&mut self) -> Option<&str> {
        if self.pos + 1 >= self.items.len() {
            return None;
        }
        self.pos += 1;
        self.current()
    }

    /// Removes a note (e.g. deleted) from the history; returns the new current id.
    pub fn forget(&mut self, id: &str) -> Option<&str> {
        let before = self.items[..(self.pos + 1).min(self.items.len())].iter().filter(|x| *x != id).count();
        self.items.retain(|x| x != id);
        self.pos = before.saturating_sub(1).min(self.items.len().saturating_sub(1));
        self.current()
    }
}

pub fn export_plain(note: &Note) -> String {
    markdown_to_plain(&note.text)
}

pub fn export_markdown(note: &Note) -> String {
    note.text.clone()
}

/// Complete HTML document whose `<title>` is the note title.
pub fn export_html(note: &Note) -> String {
    let html = markdown_to_html(&note.text);
    let title = display_title(note).replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    match (html.find("<title>"), html.find("</title>")) {
        (Some(a), Some(b)) if a < b => format!("{}<title>{}{}", &html[..a], title, &html[b..]),
        _ => html,
    }
}

/// Safe file name from the title: no path separators, no leading dot, at most 60 chars.
pub fn export_file_name(note: &Note, ext: &str) -> String {
    let t: String =
        display_title(note).chars().map(|c| if matches!(c, '/' | '\\' | ':' | '\0') || c.is_control() { '-' } else { c }).collect();
    let t = t.trim_start_matches(['.', '-', ' ']).trim();
    let t: String = t.chars().take(60).collect();
    let t = t.trim_end().to_string();
    let t = if t.is_empty() { "Untitled".to_string() } else { t };
    format!("{t}.{}", ext.trim_start_matches('.'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Note;

    fn note(id: &str, text: &str, modified: i64) -> Note {
        Note { id: id.into(), text: text.into(), created: 0, modified, pinned_at: None, deleted_at: None }
    }
    fn ids<'a>(v: &[&'a Note]) -> Vec<&'a str> {
        v.iter().map(|n| n.id.as_str()).collect()
    }

    #[test]
    fn create_and_set_text() {
        let mut v = vec![];
        assert_eq!(create(&mut v, "a", 5), "a");
        assert_eq!(v.len(), 1);
        assert_eq!((v[0].text.as_str(), v[0].created, v[0].modified), ("", 5, 5));
        set_text(&mut v, "a", "hi", 9);
        assert_eq!((v[0].text.as_str(), v[0].modified, v[0].created), ("hi", 9, 5));
        set_text(&mut v, "a", "hi", 20);
        assert_eq!(v[0].modified, 9, "unchanged text keeps modified");
        let before = v.clone();
        set_text(&mut v, "zzz", "x", 30);
        assert_eq!(v, before);
    }

    #[test]
    fn titles() {
        let t = |s: &str| display_title(&note("1", s, 0));
        assert_eq!(t(""), "Untitled");
        assert_eq!(t("  \n\n"), "Untitled");
        assert_eq!(t("\n## Groceries\nmilk"), "Groceries");
        assert_eq!(t("> quoted"), "quoted");
        assert_eq!(t("- item"), "item");
        assert_eq!(t("* item"), "item");
        assert_eq!(t("1. first"), "first");
        assert_eq!(t("- [ ] todo"), "todo");
        assert_eq!(t("- [x] done"), "done");
        assert_eq!(t("[x] done"), "done");
        assert_eq!(t("**bold** and __b2__"), "bold and b2");
        assert_eq!(t("*it* _it2_ ~~gone~~ `code` <u>under</u>"), "it it2 gone code under");
        assert_eq!(t("see [the site](https://x.y/z) now"), "see the site now");
        assert_eq!(t("# "), "Untitled");
        assert_eq!(t(&"a".repeat(200)).chars().count(), 80);
    }

    #[test]
    fn pinning_and_slots() {
        let mut v = vec![note("a", "A", 1), note("b", "B", 2), note("c", "C", 3)];
        set_pinned(&mut v, "b", true, 10);
        set_pinned(&mut v, "a", true, 20);
        assert_eq!(pinned_slot(&v, 1), Some("b".to_string()));
        assert_eq!(pinned_slot(&v, 2), Some("a".to_string()));
        assert_eq!(pinned_slot(&v, 3), None);
        assert_eq!(pinned_slot(&v, 0), None);
        assert_eq!(pinned_slot(&v, 10), None);
        set_pinned(&mut v, "b", true, 99);
        assert_eq!(v[1].pinned_at, Some(10), "repin keeps slot");
        set_pinned(&mut v, "b", false, 30);
        assert_eq!(v[1].pinned_at, None);
        assert_eq!(pinned_slot(&v, 1), Some("a".to_string()));
        let before = v.clone();
        set_pinned(&mut v, "nope", true, 1);
        assert_eq!(v, before);
        delete(&mut v, "a", 40);
        assert_eq!(pinned_slot(&v, 1), None);
    }

    #[test]
    fn browse_order_and_search() {
        let mut v = vec![note("old", "Old", 1), note("new", "New", 5), note("p", "Pinned", 0), note("q", "Pinned2", 0)];
        set_pinned(&mut v, "q", true, 1);
        set_pinned(&mut v, "p", true, 2);
        assert_eq!(ids(&browse(&v, "")), ["q", "p", "new", "old"]);
        delete(&mut v, "new", 9);
        assert_eq!(ids(&browse(&v, "")), ["q", "p", "old"]);

        let v = vec![
            note("body", "Zed\nmentions apple here", 9),
            note("title", "My APPLE pie", 1),
            note("none", "other", 5),
            note("pt", "apple pinned", 0),
            note("pb", "x\napple", 0),
        ];
        let mut v = v;
        set_pinned(&mut v, "pb", true, 1);
        set_pinned(&mut v, "pt", true, 2);
        assert_eq!(ids(&browse(&v, "apple")), ["pb", "pt", "title", "body"]);
        assert_eq!(ids(&browse(&v, "APPLE TART")), Vec::<&str>::new());
        assert_eq!(ids(&browse(&v, "pie")), ["title"]);
    }

    #[test]
    fn delete_restore_purge() {
        let mut v = vec![note("a", "A", 1), note("b", "B", 2)];
        set_pinned(&mut v, "a", true, 3);
        delete(&mut v, "a", 100);
        assert_eq!((v[0].deleted_at, v[0].pinned_at), (Some(100), None));
        assert_eq!(v.len(), 2);
        let before = v.clone();
        delete(&mut v, "zzz", 1);
        assert_eq!(v, before);
        restore(&mut v, "a", 200);
        assert_eq!(v[0].deleted_at, None);
        assert_eq!(v[0].pinned_at, None, "restore does not re-pin");
        delete_all(&mut v, 300);
        assert!(v.iter().all(|n| n.deleted_at == Some(300)));
        assert_eq!(browse(&v, "").len(), 0);
        delete(&mut v, "a", 500);
        assert_eq!(v[0].deleted_at, Some(300), "already deleted keeps first time");
        v[1].deleted_at = Some(400);
        assert_eq!(ids(&recently_deleted(&v)), ["b", "a"]);
        delete_forever(&mut v, "b");
        assert_eq!(v.len(), 1);
        delete_forever(&mut v, "b");
        assert_eq!(v.len(), 1);

        let day = 86_400;
        let mut v = vec![note("keep", "k", 0), note("edge", "e", 0), note("gone", "g", 0)];
        v[1].deleted_at = Some(1000);
        v[2].deleted_at = Some(999);
        let now = 1000 + TRASH_DAYS * day;
        purge(&mut v, now);
        assert_eq!(v.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(), ["keep", "edge"]);
        purge(&mut v, now + 1);
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn history_like_a_browser() {
        let mut h = History::default();
        assert_eq!(h.current(), None);
        assert_eq!(h.back(), None);
        h.visit("a");
        h.visit("b");
        h.visit("b");
        h.visit("c");
        assert_eq!(h.current(), Some("c"));
        assert_eq!(h.back(), Some("b"));
        assert_eq!(h.back(), Some("a"));
        assert_eq!(h.back(), None);
        assert_eq!(h.current(), Some("a"));
        assert_eq!(h.forward(), Some("b"));
        h.visit("d");
        assert_eq!(h.forward(), None);
        assert_eq!(h.back(), Some("b"));
        assert_eq!(h.forward(), Some("d"));
        assert_eq!(h.forward(), None);
        h.forget("b");
        assert_eq!(h.back(), Some("a"));
        assert_eq!(h.forward(), Some("d"));
        h.forget("d");
        assert_eq!(h.current(), Some("a"));
        h.forget("a");
        assert_eq!(h.current(), None);
    }

    #[test]
    fn exports() {
        let n = note("1", "# Hi <there>\n\nsome **bold** text", 0);
        assert_eq!(export_markdown(&n), n.text);
        let plain = export_plain(&n);
        assert!(plain.contains("Hi <there>") && plain.contains("bold") && !plain.contains("**"), "{plain}");
        let html = export_html(&n);
        assert!(html.contains("<!DOCTYPE html>") && html.contains("</html>"));
        assert!(html.contains("<title>Hi &lt;there&gt;</title>"), "{html}");
        assert!(html.contains("<strong>bold</strong>"), "{html}");
    }

    #[test]
    fn file_names() {
        let f = |s: &str, e: &str| export_file_name(&note("1", s, 0), e);
        assert_eq!(f("Groceries", "md"), "Groceries.md");
        assert_eq!(f("a/b\\c:d", "txt"), "a-b-c-d.txt");
        assert_eq!(f("..hidden", "html"), "hidden.html");
        assert_eq!(f("", "md"), "Untitled.md");
        assert_eq!(f("///", "md"), "Untitled.md");
        assert_eq!(f(&"x".repeat(100), "md"), format!("{}.md", "x".repeat(60)));
    }

    #[test]
    fn old_notes_json_round_trips() {
        let old = r##"[{"id":"1","text":"# A","created":1,"modified":2}]"##;
        let v: Vec<Note> = serde_json::from_str(old).unwrap();
        assert_eq!((v[0].pinned_at, v[0].deleted_at), (None, None));
        assert_eq!(serde_json::to_string(&v).unwrap(), old);
        let mut v = v;
        set_pinned(&mut v, "1", true, 7);
        let s = serde_json::to_string(&v).unwrap();
        assert!(s.contains("\"pinned_at\":7") && !s.contains("deleted_at"), "{s}");
        assert_eq!(serde_json::from_str::<Vec<Note>>(&s).unwrap(), v);
    }
}
