//! The action panel (⌘K) and Browse Notes of the Notes window, as pure logic.
//!
//! The controller renders `top_level` / `children` / `search_all` in a palette and executes the
//! picked item by its `NoteAction::id`. No I/O, no clock, no UI.
//!
//! Visibility rule (decided here): an action that cannot apply is omitted, never shown
//! disabled. A disabled row in a short palette is noise and teaches the user nothing; the
//! only thing a user can act on is what is listed. `Entry::enabled` stays in the API for the
//! controller and for `filter`, which sorts disabled rows last, but the lists built here set it
//! to `true`.

use crate::models::Note;
use crate::note_ai;
use crate::notes;

/// Output format of Copy As and Export (mirrors the three export functions of `notes`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format3 {
    PlainText,
    Markdown,
    Html,
}

impl Format3 {
    pub const ALL: [Format3; 3] = [Format3::PlainText, Format3::Markdown, Format3::Html];

    fn slug(self) -> &'static str {
        match self {
            Format3::PlainText => "plain",
            Format3::Markdown => "markdown",
            Format3::Html => "html",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Format3::PlainText => "Plain Text",
            Format3::Markdown => "Markdown",
            Format3::Html => "HTML",
        }
    }

    fn from_slug(s: &str) -> Option<Format3> {
        Format3::ALL.into_iter().find(|f| f.slug() == s)
    }
}

/// Editor formats as plain strings (mirror of `Format` in the macos crate, which core cannot
/// depend on): (name, menu title, shortcut). Menu order; the first six are inline formats.
pub const FORMATS: [(&str, &str, Option<&str>); 15] = [
    ("bold", "Bold", Some("⌘B")),
    ("italic", "Italic", Some("⌘I")),
    ("strikethrough", "Strikethrough", Some("⇧⌘S")),
    ("underline", "Underline", Some("⌘U")),
    ("code", "Code", Some("⌘E")),
    ("link", "Link", Some("⌘L")),
    ("heading1", "Heading 1", Some("⌥⌘1")),
    ("heading2", "Heading 2", Some("⌥⌘2")),
    ("heading3", "Heading 3", Some("⌥⌘3")),
    ("codeblock", "Code Block", Some("⌥⌘C")),
    ("quote", "Blockquote", Some("⇧⌘B")),
    ("ordered", "Ordered List", Some("⇧⌘7")),
    ("bullet", "Bullet List", Some("⇧⌘8")),
    ("task", "Task List", Some("⇧⌘9")),
    ("rule", "Horizontal Rule", None),
];

/// One action of the panel. Submenu parents have `Menu`-like names; their leaves are
/// `Format(name)`, `CopyAs`, `Export` and `Ai(command id)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteAction {
    NewNote,
    BrowseNotes,
    /// Submenu: every editor format.
    FormatMenu,
    /// Shows or hides the Format Bar at the bottom of the window.
    ToggleFormatBar,
    PinNote,
    UnpinNote,
    NavigateBack,
    NavigateForward,
    /// Submenu: Plain Text, Markdown, HTML.
    CopyNoteAs,
    /// Submenu: Plain Text, Markdown, HTML.
    ExportNote,
    CreateQuicklink,
    /// Submenu built from `note_ai::commands()`.
    AiCommands,
    /// Nested submenu of AI Commands holding the "Change Tone" commands.
    ChangeTone,
    /// Starts or stops dictation (the title flips with `Context::dictating`).
    Dictate,
    ZoomIn,
    ZoomOut,
    ActualSize,
    DeleteNote,
    DeleteAllNotes,
    /// A view the controller opens (see `deleted_rows`).
    RecentlyDeleted,
    CopyAs(Format3),
    Export(Format3),
    /// A `note_ai` command id.
    Ai(&'static str),
    /// A name from `FORMATS`.
    Format(&'static str),
}

impl NoteAction {
    /// Every action, submenu leaves included.
    pub fn all() -> Vec<NoteAction> {
        use NoteAction::*;
        let mut v = vec![
            NewNote,
            BrowseNotes,
            FormatMenu,
            ToggleFormatBar,
            PinNote,
            UnpinNote,
            NavigateBack,
            NavigateForward,
            CopyNoteAs,
            ExportNote,
            CreateQuicklink,
            AiCommands,
            ChangeTone,
            Dictate,
            ZoomIn,
            ZoomOut,
            ActualSize,
            DeleteNote,
            DeleteAllNotes,
            RecentlyDeleted,
        ];
        v.extend(Format3::ALL.into_iter().map(CopyAs));
        v.extend(Format3::ALL.into_iter().map(Export));
        v.extend(note_ai::commands().into_iter().map(|c| Ai(c.id)));
        v.extend(FORMATS.iter().map(|f| Format(f.0)));
        v
    }

    /// Stable id string, e.g. `new`, `pin`, `export:markdown`, `ai:fix-grammar`, `format:bold`.
    pub fn id(&self) -> String {
        use NoteAction::*;
        match self {
            NewNote => "new".into(),
            BrowseNotes => "browse".into(),
            FormatMenu => "format".into(),
            ToggleFormatBar => "format-bar".into(),
            PinNote => "pin".into(),
            UnpinNote => "unpin".into(),
            NavigateBack => "back".into(),
            NavigateForward => "forward".into(),
            CopyNoteAs => "copy-as".into(),
            ExportNote => "export".into(),
            CreateQuicklink => "quicklink".into(),
            AiCommands => "ai".into(),
            ChangeTone => "ai-tone".into(),
            Dictate => "dictate".into(),
            ZoomIn => "zoom-in".into(),
            ZoomOut => "zoom-out".into(),
            ActualSize => "zoom-reset".into(),
            DeleteNote => "delete".into(),
            DeleteAllNotes => "delete-all".into(),
            RecentlyDeleted => "deleted".into(),
            CopyAs(f) => format!("copy-as:{}", f.slug()),
            Export(f) => format!("export:{}", f.slug()),
            Ai(id) => format!("ai:{id}"),
            Format(name) => format!("format:{name}"),
        }
    }

    /// Inverse of `id`; `None` for an unknown id.
    pub fn from_id(id: &str) -> Option<NoteAction> {
        use NoteAction::*;
        Some(match id {
            "new" => NewNote,
            "browse" => BrowseNotes,
            "format" => FormatMenu,
            "format-bar" => ToggleFormatBar,
            "pin" => PinNote,
            "unpin" => UnpinNote,
            "back" => NavigateBack,
            "forward" => NavigateForward,
            "copy-as" => CopyNoteAs,
            "export" => ExportNote,
            "quicklink" => CreateQuicklink,
            "ai" => AiCommands,
            "ai-tone" => ChangeTone,
            "dictate" => Dictate,
            "zoom-in" => ZoomIn,
            "zoom-out" => ZoomOut,
            "zoom-reset" => ActualSize,
            "delete" => DeleteNote,
            "delete-all" => DeleteAllNotes,
            "deleted" => RecentlyDeleted,
            _ => {
                if let Some(s) = id.strip_prefix("copy-as:") {
                    CopyAs(Format3::from_slug(s)?)
                } else if let Some(s) = id.strip_prefix("export:") {
                    Export(Format3::from_slug(s)?)
                } else if let Some(s) = id.strip_prefix("ai:") {
                    Ai(note_ai::commands().into_iter().find(|c| c.id == s)?.id)
                } else if let Some(s) = id.strip_prefix("format:") {
                    Format(FORMATS.iter().find(|f| f.0 == s)?.0)
                } else {
                    return None;
                }
            }
        })
    }
}

/// What the window knows when the panel opens.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Context {
    pub has_note: bool,
    pub has_selection: bool,
    pub is_pinned: bool,
    pub can_back: bool,
    pub can_forward: bool,
    pub note_count: usize,
    pub deleted_count: usize,
    pub dictating: bool,
    pub ai_available: bool,
    pub quicklinks_available: bool,
    /// The Format Bar is showing (flips the title of `ToggleFormatBar`).
    pub format_bar: bool,
}

/// One row of the panel.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub action: NoteAction,
    pub title: String,
    pub subtitle: Option<String>,
    /// macOS glyphs, e.g. "⇧⌘P".
    pub shortcut: Option<&'static str>,
    pub section: &'static str,
    pub enabled: bool,
    pub danger: bool,
    pub has_children: bool,
}

fn entry(action: NoteAction, title: &str, shortcut: Option<&'static str>, section: &'static str) -> Entry {
    Entry { action, title: title.into(), subtitle: None, shortcut, section, enabled: true, danger: false, has_children: false }
}

fn menu(mut e: Entry) -> Entry {
    e.has_children = true;
    e
}

fn danger(mut e: Entry) -> Entry {
    e.danger = true;
    e
}

/// The ⌘K root. Note-bound actions need `has_note`; see the module comment for omission.
pub fn top_level(ctx: &Context) -> Vec<Entry> {
    use NoteAction::*;
    let mut v = vec![entry(NewNote, "New Note", Some("⌘N"), "Note")];
    if ctx.note_count > 0 {
        v.push(entry(BrowseNotes, "Browse Notes", Some("⌘P"), "Note"));
    }
    if ctx.has_note {
        if ctx.is_pinned {
            v.push(entry(UnpinNote, "Unpin Note", Some("⇧⌘P"), "Note"));
        } else {
            v.push(entry(PinNote, "Pin Note", Some("⇧⌘P"), "Note"));
        }
    }
    if ctx.can_back {
        v.push(entry(NavigateBack, "Navigate Back", Some("⌘["), "Note"));
    }
    if ctx.can_forward {
        v.push(entry(NavigateForward, "Navigate Forward", Some("⌘]"), "Note"));
    }
    if ctx.has_note {
        v.push(menu(entry(FormatMenu, "Format", None, "Edit")));
        let t = if ctx.format_bar { "Hide Format Bar" } else { "Show Format Bar" };
        v.push(entry(ToggleFormatBar, t, None, "Edit"));
        let t = if ctx.dictating { "Stop Dictation" } else { "Start Dictation" };
        v.push(entry(Dictate, t, None, "Edit"));
        v.push(menu(entry(CopyNoteAs, "Copy As", None, "Share")));
        v.push(menu(entry(ExportNote, "Export", None, "Share")));
        if ctx.quicklinks_available {
            v.push(entry(CreateQuicklink, "Create Quicklink", None, "Share"));
        }
        if ctx.ai_available {
            v.push(menu(entry(AiCommands, "AI Commands", None, "AI")));
        }
    }
    v.push(entry(ZoomIn, "Zoom In", Some("⌘+"), "View"));
    v.push(entry(ZoomOut, "Zoom Out", Some("⌘-"), "View"));
    v.push(entry(ActualSize, "Actual Size", Some("⌘0"), "View"));
    if ctx.has_note {
        v.push(danger(entry(DeleteNote, "Delete Note", None, "Danger")));
    }
    if ctx.note_count > 0 {
        v.push(danger(entry(DeleteAllNotes, "Delete All Notes", None, "Danger")));
    }
    if ctx.deleted_count > 0 {
        v.push(entry(RecentlyDeleted, "Recently Deleted", None, "Danger"));
    }
    v
}

fn ai_entry(action: NoteAction, title: &str, c: &note_ai::NoteCommand, ctx: &Context) -> Entry {
    let mut e = entry(action, title, None, "AI");
    if c.scope == note_ai::Scope::Selection && !ctx.has_selection {
        e.subtitle = Some("(whole note)".into());
    }
    e
}

/// The items inside a submenu; empty for actions without one and when the submenu cannot apply.
pub fn children(action: NoteAction, ctx: &Context) -> Vec<Entry> {
    use NoteAction::*;
    match action {
        FormatMenu if ctx.has_note => {
            FORMATS.iter().enumerate().map(|(i, f)| entry(Format(f.0), f.1, f.2, if i < 6 { "Text" } else { "Paragraph" })).collect()
        }
        CopyNoteAs if ctx.has_note => Format3::ALL.into_iter().map(|f| entry(CopyAs(f), f.title(), None, "Copy As")).collect(),
        ExportNote if ctx.has_note => Format3::ALL.into_iter().map(|f| entry(Export(f), f.title(), None, "Export")).collect(),
        AiCommands if ctx.has_note && ctx.ai_available => {
            let mut v = Vec::new();
            let mut tone_done = false;
            for c in note_ai::commands() {
                if c.tone.is_some() {
                    if !tone_done {
                        tone_done = true;
                        v.push(menu(ai_entry(ChangeTone, "Change Tone", &c, ctx)));
                    }
                } else {
                    v.push(ai_entry(Ai(c.id), c.title, &c, ctx));
                }
            }
            v
        }
        ChangeTone if ctx.has_note && ctx.ai_available => note_ai::commands()
            .into_iter()
            .filter(|c| c.tone.is_some())
            .map(|c| ai_entry(Ai(c.id), c.title.strip_prefix("Change Tone to ").unwrap_or(c.title), &c, ctx))
            .collect(),
        _ => Vec::new(),
    }
}

/// Match quality of `title` for `q` (lowercase, non-empty); lower is better.
fn rank(title: &str, q: &str) -> Option<u8> {
    let t = title.to_lowercase();
    if t.starts_with(q) {
        return Some(0);
    }
    if t.split(|c: char| !c.is_alphanumeric()).any(|w| !w.is_empty() && w.starts_with(q)) {
        return Some(1);
    }
    if t.contains(q) {
        return Some(2);
    }
    let mut it = t.chars();
    q.chars().all(|c| it.any(|x| x == c)).then_some(3)
}

fn sorted(mut scored: Vec<(Entry, u8)>) -> Vec<Entry> {
    scored.sort_by_key(|(e, r)| (!e.enabled, *r));
    scored
        .into_iter()
        .map(|(mut e, _)| {
            e.section = "Results";
            e
        })
        .collect()
}

/// Case-insensitive prefix / word-prefix / substring / subsequence match on the title, best
/// first (stable), disabled rows last. An empty query returns the entries unchanged, with their
/// sections; otherwise every result is in the "Results" section.
pub fn filter(entries: &[Entry], query: &str) -> Vec<Entry> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return entries.to_vec();
    }
    sorted(entries.iter().filter_map(|e| rank(&e.title, &q).map(|r| (e.clone(), r))).collect())
}

const SEP: &str = " › ";

fn flatten(ctx: &Context, path: &str, entries: Vec<Entry>, out: &mut Vec<Entry>) {
    for mut e in entries {
        if !path.is_empty() {
            e.title = format!("{path}{SEP}{}", e.title);
        }
        let (action, title, has) = (e.action, e.title.clone(), e.has_children);
        out.push(e);
        if has {
            flatten(ctx, &title, children(action, ctx), out);
        }
    }
}

/// Searches the root and every submenu level at once; nested items carry their path in the
/// title (`Format › Bold`). The best of the full title and the last segment ranks the row.
/// An empty query returns the plain root.
pub fn search_all(ctx: &Context, query: &str) -> Vec<Entry> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return top_level(ctx);
    }
    let mut all = Vec::new();
    flatten(ctx, "", top_level(ctx), &mut all);
    sorted(
        all.into_iter()
            .filter_map(|e| {
                let leaf = e.title.rsplit(SEP).next().unwrap_or(&e.title).to_string();
                let r = match (rank(&e.title, &q), rank(&leaf, &q)) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (a, b) => a.or(b),
                }?;
                Some((e, r))
            })
            .collect(),
    )
}

/// One row of Browse Notes or Recently Deleted.
#[derive(Debug, Clone, PartialEq)]
pub struct BrowseRow {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub pinned: bool,
    /// 1..=9 for the first nine pinned notes (the ⌘1..⌘9 slots).
    pub slot: Option<u8>,
    pub current: bool,
}

const DAY: i64 = 86_400;

/// Days since 1970-01-01 → (year, month, day), proleptic Gregorian, UTC.
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

fn plural(n: i64, unit: &str) -> String {
    if n == 1 {
        format!("1 {unit}")
    } else {
        format!("{n} {unit}s")
    }
}

/// "just now", "5 min ago", "2 hours ago", "yesterday", "3 days ago"; from 7 days on, YYYY-MM-DD.
fn relative_time(modified: i64, now: i64) -> String {
    let d = (now - modified).max(0);
    if d < 60 {
        "just now".into()
    } else if d < 3600 {
        format!("{} min ago", d / 60)
    } else if d < DAY {
        format!("{} ago", plural(d / 3600, "hour"))
    } else if d < 2 * DAY {
        "yesterday".into()
    } else if d < 7 * DAY {
        format!("{} days ago", d / DAY)
    } else {
        let (y, m, day) = civil(modified.div_euclid(DAY));
        format!("{y:04}-{m:02}-{day:02}")
    }
}

/// First non-blank line after the title line, markers stripped, at most 60 chars.
fn preview(note: &Note) -> String {
    let Some(line) = note.text.lines().filter(|l| !l.trim().is_empty()).nth(1) else {
        return String::new();
    };
    let probe = Note { id: String::new(), text: line.into(), created: 0, modified: 0, pinned_at: None, deleted_at: None };
    let t = notes::display_title(&probe);
    if t == "Untitled" && !line.contains("Untitled") {
        return String::new();
    }
    if t.chars().count() > 60 {
        let cut: String = t.chars().take(59).collect();
        format!("{}…", cut.trim_end())
    } else {
        t
    }
}

/// Browse Notes rows for `query`: pinned first (with their ⌘1..⌘9 slot), then recent notes.
pub fn browse_rows(notes_list: &[Note], query: &str, current_id: Option<&str>, now: i64) -> Vec<BrowseRow> {
    let slots: Vec<Option<String>> = (1..=9).map(|n| notes::pinned_slot(notes_list, n)).collect();
    notes::browse(notes_list, query)
        .into_iter()
        .map(|n| {
            let slot = slots.iter().position(|s| s.as_deref() == Some(n.id.as_str())).map(|i| i as u8 + 1);
            let p = preview(n);
            let rel = relative_time(n.modified, now);
            BrowseRow {
                id: n.id.clone(),
                title: notes::display_title(n),
                subtitle: if p.is_empty() { rel } else { format!("{rel} · {p}") },
                pinned: n.pinned_at.is_some(),
                slot,
                current: current_id == Some(n.id.as_str()),
            }
        })
        .collect()
}

/// Recently Deleted rows, newest deletion first: "deleted 2 days ago · 28 days left".
pub fn deleted_rows(notes_list: &[Note], now: i64) -> Vec<BrowseRow> {
    notes::recently_deleted(notes_list)
        .into_iter()
        .map(|n| {
            let at = n.deleted_at.unwrap_or(now);
            let d = (now - at).max(0);
            let ago = if d < DAY { relative_time(at, now) } else { format!("{} ago", plural(d / DAY, "day")) };
            let left = (notes::TRASH_DAYS - d / DAY).max(0);
            BrowseRow {
                id: n.id.clone(),
                title: notes::display_title(n),
                subtitle: format!("deleted {ago} · {} left", plural(left, "day")),
                pinned: false,
                slot: None,
                current: false,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every title, subtitle and section the action panel can show is in the translation table.
    #[test]
    fn every_text_of_the_action_panel_is_translated() {
        use crate::i18n::{lookup, Lang};
        let mut seen = std::collections::BTreeSet::new();
        for mask in 0u32..(1 << 7) {
            let ctx = Context {
                has_note: mask & 1 != 0,
                has_selection: mask & 2 != 0,
                is_pinned: mask & 4 != 0,
                can_back: true,
                can_forward: true,
                note_count: 3,
                deleted_count: 1,
                dictating: mask & 8 != 0,
                ai_available: true,
                quicklinks_available: true,
                format_bar: mask & 16 != 0,
            };
            let mut all = top_level(&ctx);
            for e in top_level(&ctx) {
                for c in children(e.action, &ctx) {
                    all.extend(children(c.action, &ctx));
                    all.push(c);
                }
            }
            all.extend(search_all(&ctx, ""));
            for e in all {
                seen.insert(e.title.clone());
                seen.insert(e.section.to_string());
                seen.extend(e.subtitle.clone());
            }
        }
        let missing: Vec<&String> =
            seen.iter().filter(|t| !["Markdown", "HTML"].contains(&t.as_str()) && lookup(t, Lang::De).is_none()).collect();
        assert!(missing.is_empty(), "action panel texts without a translation: {missing:#?}");
    }
    use std::collections::HashSet;

    fn titles(v: &[Entry]) -> Vec<&str> {
        v.iter().map(|e| e.title.as_str()).collect()
    }

    fn full() -> Context {
        Context {
            has_note: true,
            has_selection: true,
            is_pinned: false,
            can_back: true,
            can_forward: true,
            note_count: 3,
            deleted_count: 2,
            dictating: false,
            ai_available: true,
            quicklinks_available: true,
            format_bar: false,
        }
    }

    #[test]
    fn ids_round_trip_and_are_unique() {
        let all = NoteAction::all();
        assert_eq!(all.len(), 20 + 3 + 3 + note_ai::commands().len() + 15);
        let mut seen = HashSet::new();
        for a in &all {
            assert!(seen.insert(a.id()), "duplicate id {}", a.id());
            assert_eq!(NoteAction::from_id(&a.id()), Some(*a), "round trip {}", a.id());
        }
    }

    #[test]
    fn ids_are_stable_strings() {
        assert_eq!(NoteAction::NewNote.id(), "new");
        assert_eq!(NoteAction::BrowseNotes.id(), "browse");
        assert_eq!(NoteAction::PinNote.id(), "pin");
        assert_eq!(NoteAction::NavigateBack.id(), "back");
        assert_eq!(NoteAction::Export(Format3::Markdown).id(), "export:markdown");
        assert_eq!(NoteAction::CopyAs(Format3::PlainText).id(), "copy-as:plain");
        assert_eq!(NoteAction::Ai("fix-grammar").id(), "ai:fix-grammar");
        assert_eq!(NoteAction::Format("bold").id(), "format:bold");
        assert_eq!(NoteAction::ToggleFormatBar.id(), "format-bar");
    }

    #[test]
    fn format_bar_toggle_title_follows_the_context() {
        let hidden = top_level(&full());
        let e = hidden.iter().find(|e| e.action == NoteAction::ToggleFormatBar).unwrap();
        assert_eq!((e.title.as_str(), e.section, e.shortcut, e.has_children), ("Show Format Bar", "Edit", None, false));
        let shown = top_level(&Context { format_bar: true, ..full() });
        let e = shown.iter().find(|e| e.action == NoteAction::ToggleFormatBar).unwrap();
        assert_eq!(e.title, "Hide Format Bar");
        assert!(!top_level(&Context::default()).iter().any(|e| e.action == NoteAction::ToggleFormatBar), "needs a note");
    }

    #[test]
    fn unknown_ids_give_none() {
        for id in ["", "nope", "ai:nope", "ai:", "format:nope", "export:pdf", "copy-as:", "New", "format"] {
            let r = NoteAction::from_id(id);
            if id == "format" {
                assert_eq!(r, Some(NoteAction::FormatMenu));
            } else {
                assert_eq!(r, None, "{id}");
            }
        }
    }

    #[test]
    fn format_names_cover_the_spec_list() {
        let names: Vec<&str> = FORMATS.iter().map(|f| f.0).collect();
        for n in [
            "bold",
            "italic",
            "strikethrough",
            "underline",
            "code",
            "link",
            "heading1",
            "heading2",
            "heading3",
            "codeblock",
            "quote",
            "bullet",
            "ordered",
            "task",
            "rule",
        ] {
            assert!(names.contains(&n), "{n}");
        }
    }

    #[test]
    fn root_of_an_empty_context() {
        let v = top_level(&Context::default());
        assert_eq!(titles(&v), ["New Note", "Zoom In", "Zoom Out", "Actual Size"]);
        assert_eq!(v[0].shortcut, Some("⌘N"));
    }

    #[test]
    fn root_of_a_full_context() {
        let v = top_level(&full());
        assert_eq!(
            titles(&v),
            [
                "New Note",
                "Browse Notes",
                "Pin Note",
                "Navigate Back",
                "Navigate Forward",
                "Format",
                "Show Format Bar",
                "Start Dictation",
                "Copy As",
                "Export",
                "Create Quicklink",
                "AI Commands",
                "Zoom In",
                "Zoom Out",
                "Actual Size",
                "Delete Note",
                "Delete All Notes",
                "Recently Deleted"
            ]
        );
        let sections: Vec<&str> = v.iter().map(|e| e.section).collect();
        assert_eq!(
            sections,
            [
                "Note", "Note", "Note", "Note", "Note", "Edit", "Edit", "Edit", "Share", "Share", "Share", "AI", "View", "View", "View",
                "Danger", "Danger", "Danger"
            ]
        );
        let shortcuts: Vec<Option<&str>> = v.iter().map(|e| e.shortcut).collect();
        assert_eq!(&shortcuts[..5], [Some("⌘N"), Some("⌘P"), Some("⇧⌘P"), Some("⌘["), Some("⌘]")]);
        assert_eq!(&shortcuts[12..15], [Some("⌘+"), Some("⌘-"), Some("⌘0")]);
        let menus: Vec<&str> = v.iter().filter(|e| e.has_children).map(|e| e.title.as_str()).collect();
        assert_eq!(menus, ["Format", "Copy As", "Export", "AI Commands"]);
        let dangers: Vec<&str> = v.iter().filter(|e| e.danger).map(|e| e.title.as_str()).collect();
        assert_eq!(dangers, ["Delete Note", "Delete All Notes"]);
        assert!(v.iter().all(|e| e.enabled));
    }

    #[test]
    fn root_of_a_pinned_note_with_history() {
        let ctx = Context {
            is_pinned: true,
            can_forward: false,
            dictating: true,
            ai_available: false,
            quicklinks_available: false,
            deleted_count: 0,
            note_count: 1,
            ..full()
        };
        let v = top_level(&ctx);
        assert_eq!(
            titles(&v),
            [
                "New Note",
                "Browse Notes",
                "Unpin Note",
                "Navigate Back",
                "Format",
                "Show Format Bar",
                "Stop Dictation",
                "Copy As",
                "Export",
                "Zoom In",
                "Zoom Out",
                "Actual Size",
                "Delete Note",
                "Delete All Notes"
            ]
        );
        assert_eq!(v[2].action, NoteAction::UnpinNote);
        assert_eq!(v[2].shortcut, Some("⇧⌘P"));
    }

    #[test]
    fn pin_flips_on_state_and_needs_a_note() {
        let on = top_level(&full());
        assert_eq!(on[2].action, NoteAction::PinNote);
        let none = top_level(&Context { has_note: false, ..full() });
        assert!(none.iter().all(|e| !matches!(e.action, NoteAction::PinNote | NoteAction::DeleteNote | NoteAction::FormatMenu)));
    }

    #[test]
    fn format_children_list_every_format_with_shortcuts() {
        let v = children(NoteAction::FormatMenu, &full());
        let got: Vec<(&str, Option<&str>)> = v.iter().map(|e| (e.title.as_str(), e.shortcut)).collect();
        assert_eq!(
            got,
            [
                ("Bold", Some("⌘B")),
                ("Italic", Some("⌘I")),
                ("Strikethrough", Some("⇧⌘S")),
                ("Underline", Some("⌘U")),
                ("Code", Some("⌘E")),
                ("Link", Some("⌘L")),
                ("Heading 1", Some("⌥⌘1")),
                ("Heading 2", Some("⌥⌘2")),
                ("Heading 3", Some("⌥⌘3")),
                ("Code Block", Some("⌥⌘C")),
                ("Blockquote", Some("⇧⌘B")),
                ("Ordered List", Some("⇧⌘7")),
                ("Bullet List", Some("⇧⌘8")),
                ("Task List", Some("⇧⌘9")),
                ("Horizontal Rule", None),
            ]
        );
        assert_eq!(v[0].action, NoteAction::Format("bold"));
    }

    #[test]
    fn copy_and_export_children() {
        for (menu, leaf) in
            [(NoteAction::CopyNoteAs, NoteAction::CopyAs as fn(Format3) -> NoteAction), (NoteAction::ExportNote, NoteAction::Export)]
        {
            let v = children(menu, &full());
            assert_eq!(titles(&v), ["Plain Text", "Markdown", "HTML"]);
            assert_eq!(v[1].action, leaf(Format3::Markdown));
        }
        assert_eq!(children(NoteAction::Export(Format3::Html), &full()), vec![]);
        assert_eq!(children(NoteAction::ExportNote, &Context::default()), vec![]);
    }

    #[test]
    fn ai_children_group_tone_and_mark_whole_note() {
        let v = children(NoteAction::AiCommands, &Context { has_selection: false, ..full() });
        assert_eq!(
            titles(&v),
            [
                "Fix Spelling and Grammar",
                "Improve Writing",
                "Make Shorter",
                "Make Longer",
                "Change Tone",
                "Summarize",
                "Explain This",
                "Continue Writing",
                "Translate to English"
            ]
        );
        assert_eq!(v[0].subtitle.as_deref(), Some("(whole note)"));
        assert_eq!(v[0].action, NoteAction::Ai("fix-grammar"));
        assert_eq!(v[4].action, NoteAction::ChangeTone);
        assert!(v[4].has_children);
        // Note-scope commands never need the hint.
        assert_eq!(v[5].subtitle, None);
        let sel = children(NoteAction::AiCommands, &full());
        assert!(sel.iter().all(|e| e.subtitle.is_none()));
        let tones = children(NoteAction::ChangeTone, &Context { has_selection: false, ..full() });
        assert_eq!(titles(&tones), ["Professional", "Casual", "Friendly", "Confident", "Straightforward"]);
        assert_eq!(tones[0].action, NoteAction::Ai("tone-professional"));
        assert_eq!(tones[0].subtitle.as_deref(), Some("(whole note)"));
        assert_eq!(children(NoteAction::AiCommands, &Context { ai_available: false, ..full() }), vec![]);
    }

    #[test]
    fn filter_examples() {
        let fmt = children(NoteAction::FormatMenu, &full());
        assert_eq!(titles(&filter(&fmt, "BOLD")), ["Bold"]);
        let r = filter(&fmt, "bo");
        assert_eq!(r[0].title, "Bold");
        assert!(r.iter().all(|e| e.section == "Results"));
        // Empty query keeps order and sections.
        let all = filter(&fmt, "  ");
        assert_eq!(all, fmt);
        assert!(filter(&fmt, "zzz").is_empty());
    }

    #[test]
    fn filter_ranks_prefix_before_word_before_substring_before_subsequence_and_disabled_last() {
        let mk = |t: &str, enabled| Entry { enabled, ..entry(NoteAction::NewNote, t, None, "x") };
        let v = vec![
            mk("Xa Zebra", true),
            mk("a b c", true),
            mk("Banana", true),
            mk("Zebra", false),
            mk("Zebra Crossing", true),
            mk("zxebra", true),
        ];
        assert_eq!(titles(&filter(&v, "zeb")), ["Zebra Crossing", "Xa Zebra", "zxebra", "Zebra"]);
        let v2 = vec![mk("Dazb", true), mk("Table", true), mk("Abz", true), mk("Zed", true)];
        assert_eq!(titles(&filter(&v2, "z")), ["Zed", "Dazb", "Abz"]);
        let v3 = vec![mk("Ab cd", true), mk("Xabc", true), mk("a_x_b", true), mk("Abc", true)];
        assert_eq!(titles(&filter(&v3, "ab")), ["Ab cd", "Abc", "Xabc", "a_x_b"]);
    }

    #[test]
    fn search_all_finds_nested_items_from_the_root() {
        let ctx = full();
        let r = search_all(&ctx, "bold");
        assert_eq!(titles(&r), ["Format › Bold"]);
        assert_eq!(r[0].action, NoteAction::Format("bold"));
        assert_eq!(r[0].shortcut, Some("⌘B"));
        let m = search_all(&ctx, "markdown");
        assert_eq!(titles(&m), ["Copy As › Markdown", "Export › Markdown"]);
        assert_eq!(m[1].action, NoteAction::Export(Format3::Markdown));
        let t = search_all(&ctx, "professional");
        assert_eq!(t[0].title, "AI Commands › Change Tone › Professional");
        assert_eq!(t[0].action, NoteAction::Ai("tone-professional"));
        // Root entries are searchable too, and an empty query is the plain root.
        assert_eq!(titles(&search_all(&ctx, "new")[..1]), ["New Note"]);
        assert_eq!(search_all(&ctx, ""), top_level(&ctx));
        assert!(search_all(&Context { ai_available: false, ..ctx }, "professional").is_empty());
    }

    fn note(id: &str, text: &str, modified: i64, pinned: Option<i64>, deleted: Option<i64>) -> Note {
        Note { id: id.into(), text: text.into(), created: 0, modified, pinned_at: pinned, deleted_at: deleted }
    }

    #[test]
    fn relative_time_boundaries() {
        let now = 1_700_000_000;
        let r = |d: i64| relative_time(now - d, now);
        assert_eq!(r(0), "just now");
        assert_eq!(r(59), "just now");
        assert_eq!(r(60), "1 min ago");
        assert_eq!(r(300), "5 min ago");
        assert_eq!(r(3599), "59 min ago");
        assert_eq!(r(3600), "1 hour ago");
        assert_eq!(r(7200), "2 hours ago");
        assert_eq!(r(86_399), "23 hours ago");
        assert_eq!(r(86_400), "yesterday");
        assert_eq!(r(2 * 86_400 - 1), "yesterday");
        assert_eq!(r(2 * 86_400), "2 days ago");
        assert_eq!(r(6 * 86_400 + 5), "6 days ago");
        assert_eq!(relative_time(0, 7 * 86_400), "1970-01-01");
        assert_eq!(relative_time(1_700_000_000, 1_700_000_000 + 30 * 86_400), "2023-11-14");
        assert_eq!(relative_time(now + 100, now), "just now");
    }

    #[test]
    fn browse_rows_pins_slots_and_current() {
        let now = 10_000;
        let ns = vec![
            note("a", "Alpha", 9_000, None, None),
            note("b", "Beta", 5_000, Some(2), None),
            note("c", "Gamma", 1_000, Some(1), None),
            note("d", "Gone", 9_900, None, Some(5)),
        ];
        let rows = browse_rows(&ns, "", Some("a"), now);
        let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["c", "b", "a"]);
        assert_eq!(rows.iter().map(|r| r.slot).collect::<Vec<_>>(), [Some(1), Some(2), None]);
        assert_eq!(rows.iter().map(|r| r.pinned).collect::<Vec<_>>(), [true, true, false]);
        assert_eq!(rows.iter().map(|r| r.current).collect::<Vec<_>>(), [false, false, true]);
        assert_eq!(rows[2].title, "Alpha");
        assert_eq!(rows[2].subtitle, "16 min ago");
        // A search keeps the real slot of a pinned note.
        let q = browse_rows(&ns, "beta", None, now);
        assert_eq!((q.len(), q[0].slot), (1, Some(2)));
    }

    #[test]
    fn more_than_nine_pinned_gives_the_tenth_no_slot() {
        let ns: Vec<Note> = (1..=10).map(|i| note(&format!("n{i}"), &format!("Note {i}"), 0, Some(i), None)).collect();
        let rows = browse_rows(&ns, "", None, 0);
        assert_eq!(rows.len(), 10);
        for (i, r) in rows.iter().enumerate().take(9) {
            assert_eq!(r.slot, Some(i as u8 + 1));
        }
        assert_eq!(rows[9].slot, None);
        assert!(rows[9].pinned);
    }

    #[test]
    fn browse_rows_search_by_title_and_content_with_preview() {
        let now = 100;
        let ns = vec![
            note("t", "# Shopping\n\n- **milk** and eggs", 50, None, None),
            note("c", "Ideas\nbuy shopping bag", 90, None, None),
            note("x", "Other", 95, None, None),
        ];
        let rows = browse_rows(&ns, "shopping", None, now);
        let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["t", "c"]);
        assert_eq!(rows[0].title, "Shopping");
        assert_eq!(rows[0].subtitle, "just now · milk and eggs");
        assert_eq!(rows[1].subtitle, "just now · buy shopping bag");
        // Title only: no preview and no dangling separator.
        assert_eq!(browse_rows(&ns, "other", None, now)[0].subtitle, "just now");
    }

    #[test]
    fn preview_is_one_line_at_most_sixty_chars() {
        let long = "word ".repeat(30);
        let n = note("l", &format!("Title\n\n{long}\nthird"), 0, None, None);
        let p = preview(&n);
        assert!(p.chars().count() <= 60, "{p}");
        assert!(p.ends_with('…'));
        assert!(!p.contains('\n'));
        assert_eq!(preview(&note("m", "Title\n   \n> quoted line", 0, None, None)), "quoted line");
    }

    #[test]
    fn deleted_rows_newest_first_with_days_left() {
        let day = 86_400;
        let now = 100 * day;
        let ns = vec![
            note("old", "Old", 0, None, Some(now - 5 * day)),
            note("new", "New\nbody", 0, None, Some(now - 2 * day)),
            note("live", "Live", 0, None, None),
            note("fresh", "Fresh", 0, None, Some(now - 600)),
            note("last", "Last", 0, None, Some(now - 29 * day)),
        ];
        let rows = deleted_rows(&ns, now);
        let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["fresh", "new", "old", "last"]);
        assert_eq!(rows[0].subtitle, "deleted 10 min ago · 30 days left");
        assert_eq!(rows[1].subtitle, "deleted 2 days ago · 28 days left");
        assert_eq!(rows[2].subtitle, "deleted 5 days ago · 25 days left");
        assert_eq!(rows[3].subtitle, "deleted 29 days ago · 1 day left");
        assert!(rows.iter().all(|r| !r.pinned && r.slot.is_none() && !r.current));
    }
}
