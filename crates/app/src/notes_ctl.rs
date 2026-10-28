//! Controller of the floating Notes window (T6): which note is open, the history, the palette
//! mode, debounced saving and what every action does.
//!
//! Native callbacks (editor change, window events, palette query / pick / close, zoom) only send
//! a [`NMsg`] on the bus; the work happens in `App::drain` through [`NotesCtl::handle`]. All
//! AppKit and file-system effects go through [`NotesFx`], so tests use a fake and never open a
//! window. The controller never persists anything itself: every call returns an [`Out`] telling
//! the app what to save and which toast to show.
//!
//! The URL scheme (`NMsg::Url`), the AI commands (`NMsg::AiDone`) and dictation
//! (`NMsg::Dictation*`) arrive the same way. The settings screen and the menu-bar toggle are one
//! more match arm each: see `execute` and `handle`.

use crate::services::{Bus, Msg, Shared};
use ferrocast_core::models::{Note, Quicklink, Settings};
use ferrocast_core::note_actions::{self, BrowseRow, Context as ActCtx, Entry, NoteAction};
use ferrocast_core::note_ai;
use ferrocast_core::notes::{self, History};
use ferrocast_macos::notes::Event;
use ferrocast_macos::notes_dictation::Status as DictStatus;
use ferrocast_macos::notes_format::Format;
use ferrocast_macos::notes_palette::PaletteItem;
use ferrocast_macos::url_scheme::Route;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Saving to notes.json while typing happens at most once per this many milliseconds.
pub const DEBOUNCE_MS: u64 = 500;
/// Toast when a note is opened by a `ferrocast://` URL that points at nothing.
pub const NOTE_GONE: &str = "That note no longer exists";
pub const NOTE_IN_TRASH: &str = "That note is in Recently Deleted";
pub const AI_BUSY: &str = "An AI command is already running";
pub const AI_UNUSABLE: &str = "The AI reply could not be used";
pub const AI_STALE: &str = "The note changed while the AI was working; nothing was applied";
pub const AI_NOT_SET_UP: &str = "AI is not set up. Open Settings → AI, choose a provider and add its key (or sign in)";
pub const DICTATION_DENIED: &str = "Microphone or speech recognition access is off for Ferrocast";
/// System Settings pane where the microphone permission lives.
pub const PRIVACY_PANE: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone";

/// What a native callback tells the app.
#[derive(Debug, Clone, PartialEq)]
pub enum NMsg {
    /// The editor text changed (full Markdown).
    Changed(String),
    /// A window event (toolbar or shortcut).
    Event(Event),
    /// The palette search text changed.
    Query(String),
    /// A palette row was picked (Enter or click); the palette does not close itself.
    Pick(String),
    /// The palette hid itself.
    PaletteClosed,
    /// The editor zoom changed.
    Zoom(f64),
    /// A save timer fired.
    Tick,
    /// A `ferrocast://` URL was opened.
    Url(Route),
    /// An AI command finished (the reply is the model's raw text).
    AiDone {
        request_id: u64,
        command_id: String,
        /// The note's Markdown when the request was made.
        original_md: String,
        /// Editor offsets (UTF-16) of the selection the command worked on, if it used one.
        selection: Option<(usize, usize)>,
        reply: Result<String, String>,
    },
    /// The permission prompt of dictation was answered.
    DictationAccess(DictStatus),
    /// The full transcript so far.
    DictationPartial(String),
    /// The final transcript; recognition is over.
    DictationFinal(String),
    /// Recognition failed after it started.
    DictationError(String),
}

/// Everything the worker thread needs to run one AI command.
#[derive(Debug, Clone)]
pub struct AiRequest {
    pub request_id: u64,
    pub command_id: String,
    pub original_md: String,
    pub selection: Option<(usize, usize)>,
    pub system: &'static str,
    pub prompt: String,
    /// Settings carrying the provider and model to use (the translation engine's resolution).
    pub settings: Settings,
}

/// What the palette is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    None,
    Browse,
    Actions,
    /// A submenu of the action panel (Format, Copy As, Export, AI Commands, Change Tone).
    Submenu(NoteAction),
    RecentlyDeleted,
}

/// What the app must do after a call.
#[derive(Debug, Default, PartialEq)]
pub struct Out {
    pub toast: Option<String>,
    pub save_notes: bool,
    pub save_settings: bool,
    pub save_quicklinks: bool,
}

/// The stores the controller edits, plus the clock.
pub struct Data<'a> {
    pub notes: &'a mut Vec<Note>,
    pub settings: &'a mut Settings,
    pub quicklinks: &'a mut Vec<Quicklink>,
    /// Unix seconds (model timestamps).
    pub now: i64,
    /// Monotonic-enough milliseconds (debounce only).
    pub now_ms: u64,
}

/// Every effect the controller has on the outside world. The real one talks to AppKit.
pub trait NotesFx {
    fn show(&mut self);
    fn hide(&mut self);
    fn is_visible(&self) -> bool;
    fn focus_editor(&mut self);
    fn set_markdown(&mut self, md: &str);
    /// Unix milliseconds of the last change the editor emitted (0 = none), stamped where the editor
    /// calls back, before the `Changed` message reaches the controller.
    fn last_keystroke_ms(&self) -> u64 {
        0
    }
    fn apply_format(&mut self, f: Format);
    fn has_selection(&self) -> bool;
    fn zoom(&self) -> f64;
    fn set_zoom(&mut self, z: f64);
    fn set_escape_loses_focus(&mut self, on: bool);
    fn set_format_bar(&mut self, on: bool);
    fn palette_show(&mut self, placeholder: &str, items: Vec<PaletteItem>);
    fn palette_set_items(&mut self, items: Vec<PaletteItem>);
    fn palette_push(&mut self, placeholder: &str, items: Vec<PaletteItem>);
    fn palette_hide(&mut self);
    fn palette_visible(&self) -> bool;
    /// Open levels: 0 hidden, 1 root, more inside submenus.
    fn palette_depth(&self) -> usize;
    fn palette_empty_text(&mut self, text: &str);
    fn set_clipboard(&mut self, text: &str);
    fn downloads_dir(&self) -> Option<PathBuf>;
    fn file_exists(&self, p: &Path) -> bool;
    fn write_file(&mut self, p: &Path, contents: &str) -> Result<(), String>;
    fn reveal(&mut self, p: &Path);
    /// Send `NMsg::Tick` after `ms` milliseconds.
    fn schedule_tick(&mut self, ms: u64);
    /// The selection as editor UTF-16 offsets; equal when there is none (the cursor).
    fn selection_range(&self) -> (usize, usize);
    fn select_range(&mut self, start: usize, end: usize);
    /// The selection as Markdown.
    fn selection_markdown(&self) -> String;
    /// Replace the selection (the whole note without one) by a Markdown result: one undo step.
    fn replace_selection_markdown(&mut self, md: &str) -> bool;
    fn append_markdown(&mut self, md: &str) -> bool;
    fn insert_markdown_after_selection_line(&mut self, md: &str) -> bool;
    /// Plain-text replacement of the editor range `start..end` (UTF-16 offsets).
    fn replace_range(&mut self, start: usize, end: usize, text: &str);
    fn text_in_range(&self, start: usize, end: usize) -> Option<String>;
    /// Is an AI provider usable with these settings?
    fn ai_available(&self, s: &Settings) -> bool;
    /// Run the request on a worker thread; the answer comes back as `NMsg::AiDone`.
    fn ai_start(&mut self, req: AiRequest);
    /// Never prompts.
    fn dictation_status(&self) -> DictStatus;
    /// Shows the system prompts; the answer comes back as `NMsg::DictationAccess`.
    fn dictation_request_access(&mut self);
    /// Starts recognition; transcripts come back as `NMsg::Dictation*`.
    fn dictation_start(&mut self) -> Result<(), String>;
    fn dictation_stop(&mut self);
    fn open_url(&mut self, url: &str);
}

// ---------------------------------------------------------------- pure helpers

/// May the notes be written now? (`last` and `now` in milliseconds.)
pub fn debounce_due(last: Option<u64>, now: u64) -> bool {
    last.is_none_or(|l| now < l || now - l >= DEBOUNCE_MS)
}

/// Milliseconds left until `debounce_due` turns true.
pub fn debounce_wait(last: Option<u64>, now: u64) -> u64 {
    match last {
        Some(l) if now >= l => DEBOUNCE_MS.saturating_sub(now - l),
        _ => 0,
    }
}

/// `name`, or `stem 2.ext`, `stem 3.ext`… for the first one `exists` does not report.
pub fn unique_file_name(name: &str, exists: &dyn Fn(&str) -> bool) -> String {
    if !exists(name) {
        return name.to_string();
    }
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    (2..10_000).map(|n| format!("{stem} {n}{ext}")).find(|c| !exists(c)).unwrap_or_else(|| name.to_string())
}

/// The note the window opens on: `last` when it exists and is not deleted, else the most recent.
pub fn initial_id(list: &[Note], last: Option<&str>) -> Option<String> {
    if let Some(l) = last.filter(|l| list.iter().any(|n| n.id == *l && n.deleted_at.is_none())) {
        return Some(l.to_string());
    }
    list.iter().filter(|n| n.deleted_at.is_none()).max_by_key(|n| n.modified).map(|n| n.id.clone())
}

/// The editor format a `note_actions::FORMATS` name stands for.
pub fn format_for(name: &str) -> Option<Format> {
    Some(match name {
        "bold" => Format::Bold,
        "italic" => Format::Italic,
        "strikethrough" => Format::Strikethrough,
        "underline" => Format::Underline,
        "code" => Format::Code,
        "link" => Format::Link,
        "heading1" => Format::Heading(1),
        "heading2" => Format::Heading(2),
        "heading3" => Format::Heading(3),
        "codeblock" => Format::CodeBlock,
        "quote" => Format::Quote,
        "ordered" => Format::OrderedList,
        "bullet" => Format::BulletList,
        "task" => Format::TaskList,
        "rule" => Format::Rule,
        _ => return None,
    })
}

/// Browse rows as palette rows. With `current_first` the open note leads, so it starts selected.
pub fn browse_items(rows: &[BrowseRow], current_first: bool) -> Vec<PaletteItem> {
    let mut v: Vec<&BrowseRow> = rows.iter().collect();
    if current_first {
        if let Some(i) = v.iter().position(|r| r.current) {
            let r = v.remove(i);
            v.insert(0, r);
        }
    }
    v.into_iter()
        .map(|r| PaletteItem {
            id: r.id.clone(),
            title: r.title.clone(),
            subtitle: Some(r.subtitle.clone()),
            glyph: r.pinned.then(|| "sf:pin.fill".to_string()),
            accessory: r.slot.map(|n| format!("⌘{n}")),
            section: Some(ferrocast_core::i18n::tr(if r.pinned { "Pinned" } else { "Notes" }).into_owned()),
            danger: false,
            has_children: false,
        })
        .collect()
}

/// Action panel rows.
pub fn action_items(entries: &[Entry]) -> Vec<PaletteItem> {
    entries
        .iter()
        .map(|e| PaletteItem {
            id: e.action.id(),
            title: ferrocast_core::i18n::tr(&e.title).into_owned(),
            subtitle: e.subtitle.as_deref().map(|s| ferrocast_core::i18n::tr(s).into_owned()),
            glyph: None,
            accessory: e.shortcut.map(str::to_string),
            section: Some(ferrocast_core::i18n::tr(e.section).into_owned()),
            danger: e.danger,
            has_children: e.has_children,
        })
        .collect()
}

/// Recently Deleted rows.
pub fn deleted_items(rows: &[BrowseRow]) -> Vec<PaletteItem> {
    rows.iter()
        .map(|r| PaletteItem { id: r.id.clone(), title: r.title.clone(), subtitle: Some(r.subtitle.clone()), ..Default::default() })
        .collect()
}

/// Is the provider usable now? A local OpenAI-compatible server needs no key.
pub fn ai_usable(provider: ferrocast_core::models::AiProvider, has_key: bool, signed_in: bool, claude_found: bool) -> bool {
    use ferrocast_core::models::AiProvider as P;
    match provider {
        P::OpenAiCompatible => true,
        P::Anthropic => has_key,
        P::ChatGpt => signed_in,
        P::ClaudeCode => claude_found,
    }
}

/// Asks once; when the answer has no usable text asks one more time (an error on the second try
/// keeps the first answer). The first error is returned as it is.
pub fn ask_with_retry(ask: &mut dyn FnMut(&str) -> Result<String, String>, prompt: &str) -> Result<String, String> {
    let first = ask(prompt)?;
    if note_ai::result_text(&first).is_some() {
        return Ok(first);
    }
    match ask(prompt) {
        Ok(second) if note_ai::result_text(&second).is_some() => Ok(second),
        _ => Ok(first),
    }
}

/// Emoji for the `:` picker: `(emoji, name)` pairs, names that start with the query first, at most
/// `MAX_EMOJI`. Cheap: the catalogue is in memory and nothing is read from disk.
pub fn emoji_matches(query: &str) -> Vec<(String, String)> {
    let q = query.trim().to_lowercase();
    let mut hits: Vec<&emojis::Emoji> =
        emojis::iter().filter(|e| q.is_empty() || e.name().contains(&q) || e.shortcodes().any(|s| s.contains(&q))).collect();
    // rank before cutting, so a name that starts with the query is never lost to weaker matches
    hits.sort_by_key(|e| !e.name().starts_with(&q));
    hits.into_iter().take(MAX_EMOJI).map(|e| (e.as_str().to_string(), e.name().to_string())).collect()
}

/// Most emoji the `:` picker is given.
pub const MAX_EMOJI: usize = 30;

// ---------------------------------------------------------------- controller

/// Unix milliseconds of the last editor change, written by the native callback itself.
static LAST_KEYSTROKE_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Without a keystroke for this long the open note counts as idle for sync merges.
pub const TYPING_IDLE_MS: u64 = 2_000;

pub struct NotesCtl {
    fx: Box<dyn NotesFx>,
    /// The note shown in the editor (kept while the window is hidden).
    pub current: Option<String>,
    pub history: History,
    pub mode: Mode,
    ready: bool,
    dirty: bool,
    last_save_ms: Option<u64>,
    /// Time of the last keystroke that changed the open note.
    last_typed_ms: Option<u64>,
    tick_pending: bool,
    /// The running AI request, if any.
    ai: Option<AiJob>,
    next_request: u64,
    /// The running dictation, if any.
    dictating: Option<Dictation>,
    /// A permission prompt is open and dictation starts when it is granted.
    awaiting_access: bool,
}

struct AiJob {
    request_id: u64,
    note: String,
}

trait TakeJob {
    fn take_if_id(&mut self, id: u64) -> Option<AiJob>;
}

impl TakeJob for Option<AiJob> {
    /// The job when it is request `id`; a stale answer leaves the running one alone.
    fn take_if_id(&mut self, id: u64) -> Option<AiJob> {
        if self.as_ref().is_some_and(|j| j.request_id == id) {
            self.take()
        } else {
            None
        }
    }
}

/// Where the dictated text sits in the editor (UTF-16 offsets) and what was inserted so far.
struct Dictation {
    start: usize,
    len: usize,
    text: String,
    /// A space put before the transcript when it follows a word.
    prefix: String,
}

impl NotesCtl {
    pub fn new(fx: Box<dyn NotesFx>) -> NotesCtl {
        NotesCtl {
            fx,
            current: None,
            history: History::default(),
            mode: Mode::None,
            ready: false,
            dirty: false,
            last_save_ms: None,
            last_typed_ms: None,
            tick_pending: false,
            ai: None,
            next_request: 0,
            dictating: None,
            awaiting_access: false,
        }
    }

    // ------------------------------------------------------------ entry points

    /// Once at startup: Recently Deleted notes past their retention are removed for good.
    pub fn startup(&mut self, d: &mut Data) -> Out {
        let before = d.notes.len();
        notes::purge(d.notes, d.now);
        Out { save_notes: d.notes.len() != before, ..Out::default() }
    }

    /// The `Notes` command: show the window or hide it.
    pub fn toggle(&mut self, d: &mut Data) -> Out {
        let mut out = Out::default();
        if self.fx.is_visible() {
            self.fx.hide();
            self.after_hidden(d, &mut out);
        } else {
            self.show_window(d, &mut out);
        }
        out
    }

    /// The `Create Note` command: a new note in the window, editor focused.
    pub fn create_note(&mut self, d: &mut Data) -> Out {
        let mut out = Out::default();
        self.ensure_ready(d);
        self.close_palette();
        self.new_note(d, &mut out);
        self.fx.show();
        self.fx.focus_editor();
        out
    }

    /// The `Create Dictated Note` command: a new note with dictation running.
    pub fn create_dictated_note(&mut self, d: &mut Data) -> Out {
        let mut out = self.create_note(d);
        self.toggle_dictation(&mut out);
        out
    }

    /// Opens a note (from the launcher or a Quicklink) and shows the window on it.
    pub fn open_and_show(&mut self, d: &mut Data, id: &str) -> Out {
        let mut out = Out::default();
        self.ensure_ready(d);
        self.close_palette();
        self.open(d, id, true, &mut out);
        if self.current.is_none() {
            self.show_window(d, &mut out);
        } else {
            self.fx.show();
            self.fx.focus_editor();
        }
        out
    }

    /// Pins or unpins a note by id (Search Notes in the launcher).
    pub fn toggle_pin_id(&mut self, d: &mut Data, id: &str) -> Out {
        let mut out = Out::default();
        self.toggle_pin(d, id, &mut out);
        out
    }

    /// Moves a note to Recently Deleted (Search Notes in the launcher).
    pub fn delete_id(&mut self, d: &mut Data, id: &str) -> Out {
        let mut out = Out::default();
        self.delete_note(d, id, &mut out);
        out
    }

    /// The window is open and the user is typing: text is unsaved or was typed less than
    /// `TYPING_IDLE_MS` ago. A sync merge waits, so it cannot jump the caret or be overwritten by a
    /// keystroke the editor still has queued.
    pub fn typing_recently(&self, now_ms: u64) -> bool {
        let recent = |t: u64| t > 0 && now_ms.saturating_sub(t) < TYPING_IDLE_MS;
        self.fx.is_visible() && (self.dirty || self.last_typed_ms.is_some_and(recent) || recent(self.fx.last_keystroke_ms()))
    }

    /// After a sync merge replaced the notes. A deleted or trashed open note leaves the editor
    /// (another note opens when the window is showing); otherwise the editor shows the merged text
    /// when it differs from `previous` (what the editor had).
    pub fn after_merge(&mut self, d: &mut Data, previous: &str) -> Out {
        let mut out = Out::default();
        let Some(id) = self.current.clone() else { return out };
        if !Self::live(d, &id) {
            self.stop_dictation();
            self.history.forget(&id);
            self.current = None;
            if self.fx.is_visible() {
                self.open_initial(d, &mut out);
            }
        } else if let Some(n) = d.notes.iter().find(|n| n.id == id) {
            if n.text != previous {
                self.fx.set_markdown(&n.text.clone());
            }
        }
        out
    }

    /// Writes pending text now (app quit).
    pub fn flush(&mut self, d: &mut Data) -> Out {
        let mut out = Out::default();
        self.stop_dictation();
        self.flush_pending(d, &mut out);
        out
    }

    /// Pushes the persisted settings to the window; the settings screen calls this after a change.
    pub fn apply_settings(&mut self, d: &Data) {
        self.fx.set_zoom(d.settings.notes_zoom);
        self.fx.set_escape_loses_focus(d.settings.notes_escape_loses_focus);
        self.fx.set_format_bar(d.settings.notes_format_bar);
        self.ready = true;
    }

    /// One native callback, delivered through the bus.
    pub fn handle(&mut self, m: NMsg, d: &mut Data) -> Out {
        let mut out = Out::default();
        match m {
            NMsg::Changed(text) => self.on_change(d, &text, &mut out),
            NMsg::Tick => {
                self.tick_pending = false;
                if self.dirty {
                    if debounce_due(self.last_save_ms, d.now_ms) {
                        self.save(d, &mut out);
                    } else {
                        self.schedule(debounce_wait(self.last_save_ms, d.now_ms));
                    }
                }
            }
            NMsg::Zoom(z) => {
                if (d.settings.notes_zoom - z).abs() > 1e-9 {
                    d.settings.notes_zoom = z;
                    out.save_settings = true;
                }
            }
            NMsg::Event(e) => self.on_event(e, d, &mut out),
            NMsg::Query(q) => self.on_query(d, &q),
            NMsg::Pick(id) => self.on_pick(d, &id, &mut out),
            NMsg::PaletteClosed => {
                if !self.fx.palette_visible() {
                    self.mode = Mode::None;
                }
            }
            NMsg::Url(route) => out = self.on_url(route, d),
            NMsg::AiDone { request_id, command_id, original_md, selection, reply } => {
                self.finish_ai(request_id, &command_id, &original_md, selection, reply, d, &mut out)
            }
            NMsg::DictationAccess(status) => {
                if std::mem::take(&mut self.awaiting_access) {
                    self.begin_with(status, &mut out);
                }
            }
            NMsg::DictationPartial(t) => self.on_transcript(&t, false, &mut out),
            NMsg::DictationFinal(t) => self.on_transcript(&t, true, &mut out),
            NMsg::DictationError(e) => {
                if self.dictating.is_some() {
                    self.stop_dictation();
                    out.toast = Some(ferrocast_core::i18n::trf("Dictation stopped: {e}", &[("e", &e.to_string())]));
                }
            }
        }
        out
    }

    // ------------------------------------------------------------ persistence

    fn save(&mut self, d: &Data, out: &mut Out) {
        out.save_notes = true;
        self.dirty = false;
        self.last_save_ms = Some(d.now_ms);
    }

    fn flush_pending(&mut self, d: &Data, out: &mut Out) {
        if self.dirty {
            self.save(d, out);
        }
    }

    fn schedule(&mut self, ms: u64) {
        if !self.tick_pending {
            self.tick_pending = true;
            self.fx.schedule_tick(ms);
        }
    }

    fn on_change(&mut self, d: &mut Data, text: &str, out: &mut Out) {
        let Some(id) = self.current.clone() else { return };
        let Some(n) = d.notes.iter().find(|n| n.id == id) else { return };
        if n.text == text {
            return;
        }
        notes::set_text(d.notes, &id, text, d.now);
        self.dirty = true;
        self.last_typed_ms = Some(d.now_ms);
        if debounce_due(self.last_save_ms, d.now_ms) {
            self.save(d, out);
        } else {
            self.schedule(debounce_wait(self.last_save_ms, d.now_ms));
        }
    }

    // ------------------------------------------------------------ window and notes

    /// Applies the persisted settings the first time the window is used.
    fn ensure_ready(&mut self, d: &Data) {
        if !self.ready {
            self.apply_settings(d);
        }
    }

    fn live(d: &Data, id: &str) -> bool {
        d.notes.iter().any(|n| n.id == id && n.deleted_at.is_none())
    }

    fn live_count(d: &Data) -> usize {
        d.notes.iter().filter(|n| n.deleted_at.is_none()).count()
    }

    /// Shows the window on the note it still has loaded, else the last, else the most recent,
    /// else a new empty note.
    fn show_window(&mut self, d: &mut Data, out: &mut Out) {
        self.ensure_ready(d);
        if !self.current.as_deref().is_some_and(|c| Self::live(d, c)) {
            self.current = None;
            self.open_initial(d, out);
        }
        self.fx.show();
        self.fx.focus_editor();
    }

    fn open_initial(&mut self, d: &mut Data, out: &mut Out) {
        match initial_id(d.notes, d.settings.notes_last.as_deref()) {
            Some(id) => self.open(d, &id, true, out),
            None => self.new_note(d, out),
        }
    }

    /// Leaving the current note: an empty one is dropped for good unless it is the only note.
    /// Returns true when it was dropped (nothing is open afterwards).
    fn discard_if_empty(&mut self, d: &mut Data, out: &mut Out) -> bool {
        let Some(cur) = self.current.clone() else { return false };
        let empty = d.notes.iter().any(|n| n.id == cur && n.deleted_at.is_none() && n.text.trim().is_empty());
        if empty && Self::live_count(d) > 1 {
            notes::delete_forever(d.notes, &cur);
            self.history.forget(&cur);
            self.current = None;
            self.save(d, out);
            return true;
        }
        false
    }

    /// Opens a note in the editor: the current one is saved (or dropped when empty) first.
    fn open(&mut self, d: &mut Data, id: &str, record: bool, out: &mut Out) {
        if !Self::live(d, id) || self.current.as_deref() == Some(id) {
            return;
        }
        self.stop_dictation();
        if !self.discard_if_empty(d, out) {
            self.flush_pending(d, out);
        }
        if record {
            self.history.visit(id);
        }
        let text = d.notes.iter().find(|n| n.id == id).map(|n| n.text.clone()).unwrap_or_default();
        self.fx.set_markdown(&text);
        self.current = Some(id.to_string());
        if d.settings.notes_last.as_deref() != Some(id) {
            d.settings.notes_last = Some(id.to_string());
            out.save_settings = true;
        }
        self.fx.focus_editor();
    }

    fn new_note(&mut self, d: &mut Data, out: &mut Out) {
        let id = uuid::Uuid::new_v4().to_string();
        notes::create(d.notes, &id, d.now);
        self.save(d, out);
        self.open(d, &id, true, out);
    }

    /// The window went away (⌘W, Esc, toggle): nothing stays pending.
    fn after_hidden(&mut self, d: &mut Data, out: &mut Out) {
        self.stop_dictation();
        self.close_palette();
        if !self.discard_if_empty(d, out) {
            self.flush_pending(d, out);
        }
    }

    fn close_palette(&mut self) {
        if self.fx.palette_visible() {
            self.fx.palette_hide();
        }
        self.mode = Mode::None;
    }

    fn toggle_pin(&mut self, d: &mut Data, id: &str, out: &mut Out) {
        let Some(n) = d.notes.iter().find(|n| n.id == id && n.deleted_at.is_none()) else { return };
        let pinned = n.pinned_at.is_some();
        notes::set_pinned(d.notes, id, !pinned, d.now);
        self.save(d, out);
    }

    fn delete_note(&mut self, d: &mut Data, id: &str, out: &mut Out) {
        notes::delete(d.notes, id, d.now);
        let next = self.history.forget(id).map(str::to_string);
        self.save(d, out);
        if self.current.as_deref() != Some(id) {
            return;
        }
        self.stop_dictation();
        self.current = None;
        if !self.fx.is_visible() {
            return;
        }
        let target = next.filter(|n| Self::live(d, n)).or_else(|| initial_id(d.notes, None));
        match target {
            Some(t) => self.open(d, &t, false, out),
            None => self.new_note(d, out),
        }
    }

    fn delete_all(&mut self, d: &mut Data, out: &mut Out) {
        self.stop_dictation();
        notes::delete_all(d.notes, d.now);
        self.history = History::default();
        self.current = None;
        self.save(d, out);
        self.new_note(d, out);
    }

    fn navigate(&mut self, d: &mut Data, back: bool, out: &mut Out) {
        loop {
            let id = if back { self.history.back() } else { self.history.forward() }.map(str::to_string);
            match id {
                None => break,
                Some(id) if Self::live(d, &id) => {
                    self.open(d, &id, false, out);
                    break;
                }
                Some(_) => {}
            }
        }
    }

    // ------------------------------------------------------------ window events

    fn on_event(&mut self, e: Event, d: &mut Data, out: &mut Out) {
        match e {
            Event::New => {
                self.close_palette();
                self.new_note(d, out);
            }
            Event::Browse => self.open_browse(d),
            Event::Actions => {
                if self.fx.palette_visible() && self.mode == Mode::Actions {
                    self.close_palette();
                } else {
                    self.open_actions(d);
                }
            }
            Event::Back => self.navigate(d, true, out),
            Event::Forward => self.navigate(d, false, out),
            Event::TogglePin => {
                if let Some(id) = self.current.clone() {
                    self.toggle_pin(d, &id, out);
                }
            }
            Event::Pinned(n) => {
                if let Some(id) = notes::pinned_slot(d.notes, n as usize) {
                    self.close_palette();
                    self.open(d, &id, true, out);
                }
            }
            Event::Shown => {
                self.ensure_ready(d);
                if self.current.is_none() {
                    self.open_initial(d, out);
                }
            }
            Event::Hidden => self.after_hidden(d, out),
        }
    }

    // ------------------------------------------------------------ palette

    fn open_browse(&mut self, d: &Data) {
        let rows = note_actions::browse_rows(d.notes, "", self.current.as_deref(), d.now);
        self.close_palette();
        self.fx.palette_show(&ferrocast_core::i18n::tr("Browse notes…"), browse_items(&rows, true));
        self.fx.palette_empty_text(&ferrocast_core::i18n::tr("No notes found"));
        self.mode = Mode::Browse;
    }

    fn act_ctx(&self, d: &Data) -> ActCtx {
        let cur = self.current.as_deref().and_then(|c| d.notes.iter().find(|n| n.id == c && n.deleted_at.is_none()));
        ActCtx {
            has_note: cur.is_some(),
            has_selection: self.fx.has_selection(),
            is_pinned: cur.is_some_and(|n| n.pinned_at.is_some()),
            can_back: self.history.clone().back().is_some(),
            can_forward: self.history.clone().forward().is_some(),
            note_count: Self::live_count(d),
            deleted_count: d.notes.iter().filter(|n| n.deleted_at.is_some()).count(),
            dictating: self.dictating.is_some(),
            ai_available: self.fx.ai_available(d.settings),
            quicklinks_available: true,
            format_bar: d.settings.notes_format_bar,
        }
    }

    fn open_actions(&mut self, d: &Data) {
        let items = action_items(&note_actions::top_level(&self.act_ctx(d)));
        self.close_palette();
        self.fx.palette_show(&ferrocast_core::i18n::tr("Search actions…"), items);
        self.fx.palette_empty_text(&ferrocast_core::i18n::tr("No actions found"));
        self.mode = Mode::Actions;
    }

    /// The palette can pop a level by itself (Esc, ⌘[): trust its depth over the recorded mode.
    fn sync_mode(&mut self) {
        if matches!(self.mode, Mode::Submenu(_) | Mode::RecentlyDeleted) && self.fx.palette_depth() <= 1 {
            self.mode = Mode::Actions;
        }
    }

    fn deleted_rows(d: &Data, query: &str) -> Vec<BrowseRow> {
        let q = query.trim().to_lowercase();
        note_actions::deleted_rows(d.notes, d.now).into_iter().filter(|r| q.is_empty() || r.title.to_lowercase().contains(&q)).collect()
    }

    fn on_query(&mut self, d: &Data, q: &str) {
        self.sync_mode();
        let items = match self.mode {
            Mode::None => return,
            Mode::Browse => browse_items(&note_actions::browse_rows(d.notes, q, self.current.as_deref(), d.now), false),
            Mode::Actions => action_items(&note_actions::search_all(&self.act_ctx(d), q)),
            Mode::Submenu(a) => action_items(&note_actions::filter(&note_actions::children(a, &self.act_ctx(d)), q)),
            Mode::RecentlyDeleted => deleted_items(&Self::deleted_rows(d, q)),
        };
        self.fx.palette_set_items(items);
    }

    fn on_pick(&mut self, d: &mut Data, id: &str, out: &mut Out) {
        self.sync_mode();
        match self.mode {
            Mode::None => {}
            Mode::Browse => {
                self.close_palette();
                self.open(d, id, true, out);
            }
            Mode::RecentlyDeleted => {
                notes::restore(d.notes, id, d.now);
                self.save(d, out);
                self.close_palette();
                self.open(d, id, true, out);
            }
            Mode::Actions | Mode::Submenu(_) => {
                let Some(action) = NoteAction::from_id(id) else { return };
                let ctx = self.act_ctx(d);
                if action == NoteAction::RecentlyDeleted {
                    let items = deleted_items(&Self::deleted_rows(d, ""));
                    self.fx.palette_push(&ferrocast_core::i18n::tr("Recently deleted…"), items);
                    self.fx.palette_empty_text(&ferrocast_core::i18n::tr("Nothing was deleted"));
                    self.mode = Mode::RecentlyDeleted;
                    return;
                }
                let kids = note_actions::children(action, &ctx);
                if !kids.is_empty() {
                    self.fx.palette_push(&ferrocast_core::i18n::tr("Search…"), action_items(&kids));
                    self.mode = Mode::Submenu(action);
                } else if action == NoteAction::BrowseNotes {
                    self.open_browse(d);
                } else {
                    self.close_palette();
                    self.execute(action, d, out);
                }
            }
        }
    }

    // ------------------------------------------------------------ actions

    /// Runs one leaf action on the current note. A later step adds its arm here (AI, dictation).
    fn execute(&mut self, action: NoteAction, d: &mut Data, out: &mut Out) {
        use NoteAction::*;
        let cur = self.current.clone();
        match action {
            NewNote => self.new_note(d, out),
            BrowseNotes => self.open_browse(d),
            Format(name) => {
                if let Some(f) = format_for(name) {
                    self.fx.apply_format(f);
                }
            }
            PinNote | UnpinNote => {
                if let Some(id) = cur {
                    self.toggle_pin(d, &id, out);
                }
            }
            NavigateBack => self.navigate(d, true, out),
            NavigateForward => self.navigate(d, false, out),
            ZoomIn => self.fx.set_zoom(ferrocast_macos::notes_editor::step_zoom(self.fx.zoom(), 1)),
            ZoomOut => self.fx.set_zoom(ferrocast_macos::notes_editor::step_zoom(self.fx.zoom(), -1)),
            ActualSize => self.fx.set_zoom(1.0),
            CopyAs(f) => {
                if let Some(n) = cur.and_then(|c| d.notes.iter().find(|n| n.id == c)) {
                    let text = export_text(n, f);
                    self.fx.set_clipboard(&text);
                    out.toast = Some(ferrocast_core::i18n::trf("Copied as {format}", &[("format", &ferrocast_core::i18n::tr(f.title()))]));
                }
            }
            Export(f) => {
                if let Some(n) = cur.and_then(|c| d.notes.iter().find(|n| n.id == c)).cloned() {
                    out.toast = Some(self.export(&n, f));
                }
            }
            DeleteNote => {
                if let Some(id) = cur {
                    self.delete_note(d, &id, out);
                    out.toast = Some("Moved to Recently Deleted".into());
                }
            }
            DeleteAllNotes => {
                self.delete_all(d, out);
                out.toast = Some("Moved all notes to Recently Deleted".into());
            }
            ToggleFormatBar => {
                d.settings.notes_format_bar = !d.settings.notes_format_bar;
                self.fx.set_format_bar(d.settings.notes_format_bar);
                out.save_settings = true;
            }
            CreateQuicklink => {
                if let Some(n) = cur.and_then(|c| d.notes.iter().find(|n| n.id == c)) {
                    let link = ferrocast_macos::url_scheme::note_url(&n.id);
                    if d.quicklinks.iter().any(|q| q.link == link) {
                        out.toast = Some("A quicklink for this note already exists".into());
                    } else {
                        d.quicklinks.push(Quicklink::new(&notes::display_title(n), &link));
                        out.save_quicklinks = true;
                        out.toast = Some("Quicklink created".into());
                    }
                }
            }
            Dictate => self.toggle_dictation(out),
            Ai(id) => self.run_ai(id, d, out),
            // menus are opened by `on_pick`; reaching them here means there is nothing to do
            FormatMenu | CopyNoteAs | ExportNote | AiCommands | ChangeTone | RecentlyDeleted => {}
        }
    }

    // ------------------------------------------------------------ URL scheme

    /// A `ferrocast://` URL. The launcher is hidden by the app before this runs.
    fn on_url(&mut self, route: Route, d: &mut Data) -> Out {
        match route {
            Route::Notes => self.toggle(d),
            Route::NewNote => self.create_note(d),
            Route::Note(id) => match d.notes.iter().find(|n| n.id == id) {
                None => Out { toast: Some(NOTE_GONE.into()), ..Out::default() },
                Some(n) if n.deleted_at.is_some() => Out { toast: Some(NOTE_IN_TRASH.into()), ..Out::default() },
                Some(_) => self.open_and_show(d, &id),
            },
        }
    }

    // ------------------------------------------------------------ AI commands

    /// Starts the AI command `id` on the selection or the whole note.
    fn run_ai(&mut self, id: &str, d: &mut Data, out: &mut Out) {
        let Some(cmd) = note_ai::commands().into_iter().find(|c| c.id == id) else { return };
        if !self.fx.ai_available(d.settings) {
            out.toast = Some(AI_NOT_SET_UP.into());
            return;
        }
        if self.ai.is_some() {
            out.toast = Some(AI_BUSY.into());
            return;
        }
        let Some(note) = self.current.clone() else { return };
        let Some(md) = d.notes.iter().find(|n| n.id == note && n.deleted_at.is_none()).map(|n| n.text.clone()) else { return };
        let (a, b) = self.fx.selection_range();
        let selection = (cmd.scope == note_ai::Scope::Selection && a != b).then_some((a.min(b), a.max(b)));
        let text = if selection.is_some() { self.fx.selection_markdown() } else { md.clone() };
        if text.trim().is_empty() {
            out.toast = Some("Nothing to work on: the note is empty".into());
            return;
        }
        if note_ai::too_long(&text) {
            out.toast = Some(ferrocast_core::i18n::trf(
                "The text is too long for an AI command (over {n} characters)",
                &[("n", &note_ai::MAX_CHARS.to_string())],
            ));
            return;
        }
        self.next_request += 1;
        let request_id = self.next_request;
        self.ai = Some(AiJob { request_id, note });
        self.fx.ai_start(AiRequest {
            request_id,
            command_id: cmd.id.to_string(),
            original_md: md,
            selection,
            system: note_ai::SYSTEM,
            prompt: note_ai::prompt(&cmd, &text),
            settings: ferrocast_core::translate::engine_settings(d.settings),
        });
        out.toast = Some(ferrocast_core::i18n::trf("Running {title}…", &[("title", &ferrocast_core::i18n::tr(cmd.title))]));
    }

    /// The answer of the worker: applies it only when the note is exactly what it was.
    #[allow(clippy::too_many_arguments)]
    fn finish_ai(
        &mut self,
        request_id: u64,
        command_id: &str,
        original_md: &str,
        selection: Option<(usize, usize)>,
        reply: Result<String, String>,
        d: &Data,
        out: &mut Out,
    ) {
        let Some(job) = self.ai.take_if_id(request_id) else { return };
        let reply = match reply {
            Ok(r) => r,
            Err(e) => {
                out.toast = Some(ferrocast_core::i18n::trf("The AI command failed: {e}", &[("e", &e.to_string())]));
                return;
            }
        };
        let Some(text) = note_ai::result_text(&reply) else {
            out.toast = Some(AI_UNUSABLE.into());
            return;
        };
        let same_note = self.current.as_deref() == Some(job.note.as_str());
        let unchanged = d
            .notes
            .iter()
            .find(|n| n.id == job.note && n.deleted_at.is_none())
            .is_some_and(|n| note_ai::safe_to_apply(original_md, &n.text));
        if !same_note || !unchanged {
            out.toast = Some(AI_STALE.into());
            return;
        }
        let Some(cmd) = note_ai::commands().into_iter().find(|c| c.id == command_id) else { return };
        let applied = match (cmd.id, selection) {
            ("summarize" | "continue-writing", _) | ("explain", None) => self.fx.append_markdown(&text),
            ("explain", Some((a, b))) => {
                self.fx.select_range(a, b);
                self.fx.insert_markdown_after_selection_line(&text)
            }
            (_, Some((a, b))) => {
                self.fx.select_range(a, b);
                self.fx.replace_selection_markdown(&text)
            }
            (_, None) => {
                // the whole note is the target: an empty selection means exactly that
                let (a, _) = self.fx.selection_range();
                self.fx.select_range(a, a);
                self.fx.replace_selection_markdown(&text)
            }
        };
        out.toast = Some(if applied {
            ferrocast_core::i18n::trf("Done: {title}", &[("title", &ferrocast_core::i18n::tr(cmd.title))])
        } else {
            "The AI result could not be applied".into()
        });
    }

    // ------------------------------------------------------------ dictation

    /// Stops the recognizer and forgets the span. Safe to call when idle.
    fn stop_dictation(&mut self) {
        self.awaiting_access = false;
        if self.dictating.take().is_some() {
            self.fx.dictation_stop();
        }
    }

    /// The Dictate action: off when it runs, else checks the permission and starts.
    fn toggle_dictation(&mut self, out: &mut Out) {
        if self.dictating.is_some() {
            self.stop_dictation();
            return;
        }
        if self.current.is_none() || self.awaiting_access {
            return;
        }
        match self.fx.dictation_status() {
            DictStatus::NotDetermined => {
                self.awaiting_access = true;
                self.fx.dictation_request_access();
            }
            status => self.begin_with(status, out),
        }
    }

    fn begin_with(&mut self, status: DictStatus, out: &mut Out) {
        match status {
            DictStatus::Authorized => {
                if self.current.is_some() && self.fx.is_visible() {
                    self.start_dictation(out);
                }
            }
            DictStatus::Denied | DictStatus::Restricted => {
                out.toast = Some(DICTATION_DENIED.into());
                self.fx.open_url(PRIVACY_PANE);
            }
            DictStatus::NotDetermined => {}
        }
    }

    fn start_dictation(&mut self, out: &mut Out) {
        if let Err(e) = self.fx.dictation_start() {
            out.toast = Some(ferrocast_core::i18n::trf("Dictation could not start: {e}", &[("e", &e.to_string())]));
            return;
        }
        let (_, cursor) = self.fx.selection_range();
        let after_word = cursor > 0 && self.fx.text_in_range(cursor - 1, cursor).is_some_and(|c| !c.chars().all(char::is_whitespace));
        self.dictating = Some(Dictation { start: cursor, len: 0, text: String::new(), prefix: if after_word { " " } else { "" }.into() });
    }

    /// A transcript: replaces what was inserted before, never appends to it.
    fn on_transcript(&mut self, transcript: &str, last: bool, out: &mut Out) {
        let Some(dc) = self.dictating.as_ref() else { return };
        // the span must still hold exactly what was inserted, else the user edited it
        if self.fx.text_in_range(dc.start, dc.start + dc.len).as_deref() != Some(dc.text.as_str()) {
            self.stop_dictation();
            out.toast = Some("Dictation stopped because the note was edited".into());
            return;
        }
        let t = transcript.trim();
        let new = if t.is_empty() { String::new() } else { format!("{}{t}", dc.prefix) };
        if new != dc.text {
            self.fx.replace_range(dc.start, dc.start + dc.len, &new);
            if let Some(dc) = self.dictating.as_mut() {
                dc.len = new.encode_utf16().count();
                dc.text = new;
            }
        }
        if last {
            self.stop_dictation();
        }
    }

    /// Writes the note into the Downloads folder (this replaces a save dialog: the file never
    /// overwrites another one, it is revealed in Finder and the toast names it). Returns the toast.
    fn export(&mut self, n: &Note, f: note_actions::Format3) -> String {
        let Some(dir) = self.fx.downloads_dir() else { return "Export failed: no Downloads folder".into() };
        let wanted = notes::export_file_name(n, export_ext(f));
        let name = unique_file_name(&wanted, &|c| self.fx.file_exists(&dir.join(c)));
        let path = dir.join(&name);
        match self.fx.write_file(&path, &export_text(n, f)) {
            Ok(()) => {
                self.fx.reveal(&path);
                ferrocast_core::i18n::trf("Exported {name} to Downloads", &[("name", &name)])
            }
            Err(e) => ferrocast_core::i18n::trf("Export failed: {e}", &[("e", &e.to_string())]),
        }
    }
}

fn export_ext(f: note_actions::Format3) -> &'static str {
    match f {
        note_actions::Format3::PlainText => "txt",
        note_actions::Format3::Markdown => "md",
        note_actions::Format3::Html => "html",
    }
}

fn export_text(n: &Note, f: note_actions::Format3) -> String {
    match f {
        note_actions::Format3::PlainText => notes::export_plain(n),
        note_actions::Format3::Markdown => notes::export_markdown(n),
        note_actions::Format3::Html => notes::export_html(n),
    }
}

// ---------------------------------------------------------------- real effects

/// AppKit, the pasteboard and Finder.
pub struct MacFx {
    pub bus: Bus,
    pub shared: Arc<Shared>,
    /// Last answer of `ai_available`: the Keychain is not read on every palette keystroke.
    ai_cache: std::cell::RefCell<Option<(std::time::Instant, ferrocast_core::models::AiProvider, bool)>>,
}

impl MacFx {
    pub fn new(bus: Bus, shared: Arc<Shared>) -> MacFx {
        MacFx { bus, shared, ai_cache: Default::default() }
    }
}

impl NotesFx for MacFx {
    fn show(&mut self) {
        ferrocast_macos::notes::show();
    }
    fn hide(&mut self) {
        ferrocast_macos::notes::hide();
    }
    fn is_visible(&self) -> bool {
        ferrocast_macos::notes::is_visible()
    }
    fn focus_editor(&mut self) {
        ferrocast_macos::notes::focus_editor();
    }
    fn last_keystroke_ms(&self) -> u64 {
        LAST_KEYSTROKE_MS.load(std::sync::atomic::Ordering::Relaxed)
    }
    fn set_markdown(&mut self, md: &str) {
        ferrocast_macos::notes::set_markdown(md);
    }
    fn apply_format(&mut self, f: Format) {
        ferrocast_macos::notes::apply_format(f);
    }
    fn has_selection(&self) -> bool {
        let (_, a, b) = ferrocast_macos::notes::selection();
        a != b
    }
    fn zoom(&self) -> f64 {
        ferrocast_macos::notes::zoom()
    }
    fn set_zoom(&mut self, z: f64) {
        ferrocast_macos::notes::set_zoom(z);
    }
    fn set_escape_loses_focus(&mut self, on: bool) {
        ferrocast_macos::notes::set_escape_loses_focus(on);
    }
    fn set_format_bar(&mut self, on: bool) {
        ferrocast_macos::notes::set_format_bar_visible(on);
    }
    fn palette_show(&mut self, placeholder: &str, items: Vec<PaletteItem>) {
        ferrocast_macos::notes_palette::show(placeholder, items);
    }
    fn palette_set_items(&mut self, items: Vec<PaletteItem>) {
        ferrocast_macos::notes_palette::set_items(items);
    }
    fn palette_push(&mut self, placeholder: &str, items: Vec<PaletteItem>) {
        ferrocast_macos::notes_palette::push_level(placeholder, items);
    }
    fn palette_hide(&mut self) {
        ferrocast_macos::notes_palette::hide();
    }
    fn palette_visible(&self) -> bool {
        ferrocast_macos::notes_palette::is_visible()
    }
    fn palette_depth(&self) -> usize {
        ferrocast_macos::notes_palette::depth()
    }
    fn palette_empty_text(&mut self, text: &str) {
        ferrocast_macos::notes_palette::set_empty_text(text);
    }
    fn set_clipboard(&mut self, text: &str) {
        ferrocast_macos::clipboard::write_text(text);
        self.shared.mark_clipboard_write();
    }
    fn downloads_dir(&self) -> Option<PathBuf> {
        dirs::download_dir()
    }
    fn file_exists(&self, p: &Path) -> bool {
        p.exists()
    }
    fn write_file(&mut self, p: &Path, contents: &str) -> Result<(), String> {
        std::fs::write(p, contents).map_err(|e| e.to_string())
    }
    fn reveal(&mut self, p: &Path) {
        let _ = ferrocast_macos::apps::reveal(p);
    }
    fn schedule_tick(&mut self, ms: u64) {
        let bus = self.bus.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(ms));
            bus.send(Msg::Notes(NMsg::Tick));
        });
    }
    fn selection_range(&self) -> (usize, usize) {
        ferrocast_macos::notes::selection_range()
    }
    fn select_range(&mut self, start: usize, end: usize) {
        ferrocast_macos::notes::select_range(start, end);
    }
    fn selection_markdown(&self) -> String {
        ferrocast_macos::notes::markdown_of_selection()
    }
    fn replace_selection_markdown(&mut self, md: &str) -> bool {
        ferrocast_macos::notes::replace_selection_markdown(md)
    }
    fn append_markdown(&mut self, md: &str) -> bool {
        ferrocast_macos::notes::append_markdown(md)
    }
    fn insert_markdown_after_selection_line(&mut self, md: &str) -> bool {
        ferrocast_macos::notes::insert_markdown_after_selection_line(md)
    }
    fn replace_range(&mut self, start: usize, end: usize, text: &str) {
        ferrocast_macos::notes::replace_range(start, end, text);
    }
    fn text_in_range(&self, start: usize, end: usize) -> Option<String> {
        ferrocast_macos::notes::text_in_range(start, end)
    }
    fn ai_available(&self, s: &Settings) -> bool {
        use ferrocast_core::models::AiProvider as P;
        let p = ferrocast_core::translate::engine_settings(s).ai_provider;
        if let Some((at, cached, ok)) = *self.ai_cache.borrow() {
            if cached == p && at.elapsed() < std::time::Duration::from_secs(15) {
                return ok;
            }
        }
        let ok = ai_usable(
            p,
            matches!(p, P::OpenAiCompatible | P::Anthropic) && crate::services::ai_key_for(p).is_some(),
            p == P::ChatGpt && crate::services::chatgpt_credentials().is_some(),
            p == P::ClaudeCode && crate::services::claude_code_path().is_some(),
        );
        *self.ai_cache.borrow_mut() = Some((std::time::Instant::now(), p, ok));
        ok
    }
    fn ai_start(&mut self, req: AiRequest) {
        let bus = self.bus.clone();
        std::thread::spawn(move || {
            let mut ask = |prompt: &str| {
                let mut out = String::new();
                crate::services::ai_stream_system(&req.settings, req.system, &[], prompt, |t| out.push_str(t)).map(|()| out)
            };
            let reply = ask_with_retry(&mut ask, &req.prompt);
            bus.send(Msg::Notes(NMsg::AiDone {
                request_id: req.request_id,
                command_id: req.command_id,
                original_md: req.original_md,
                selection: req.selection,
                reply,
            }));
        });
    }
    fn dictation_status(&self) -> DictStatus {
        ferrocast_macos::notes_dictation::status()
    }
    fn dictation_request_access(&mut self) {
        let b = self.bus.clone();
        ferrocast_macos::notes_dictation::request_access(Box::new(move |st| b.send(Msg::Notes(NMsg::DictationAccess(st)))));
    }
    fn dictation_start(&mut self) -> Result<(), String> {
        let (p, f, e) = (self.bus.clone(), self.bus.clone(), self.bus.clone());
        ferrocast_macos::notes_dictation::start(
            Box::new(move |t| p.send(Msg::Notes(NMsg::DictationPartial(t)))),
            Box::new(move |t| f.send(Msg::Notes(NMsg::DictationFinal(t)))),
            Box::new(move |m| e.send(Msg::Notes(NMsg::DictationError(m)))),
        )
    }
    fn dictation_stop(&mut self) {
        ferrocast_macos::notes_dictation::stop();
    }
    fn open_url(&mut self, url: &str) {
        let _ = std::process::Command::new("/usr/bin/open").arg(url).spawn();
    }
}

/// Route every native callback to the bus (the shelf pattern): the callbacks only send.
pub fn install_callbacks(bus: &Bus) {
    use ferrocast_macos::{notes, notes_palette};
    let b = bus.clone();
    notes::set_on_change(Box::new(move |t| {
        // stamped here, not when the queued message is handled: a first keystroke after a pause
        // must already hold a sync merge back
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
        LAST_KEYSTROKE_MS.store(now, std::sync::atomic::Ordering::Relaxed);
        b.send(Msg::Notes(NMsg::Changed(t)))
    }));
    let b = bus.clone();
    notes::set_on_event(Box::new(move |e| b.send(Msg::Notes(NMsg::Event(e)))));
    let b = bus.clone();
    notes::set_on_zoom_change(Box::new(move |z| b.send(Msg::Notes(NMsg::Zoom(z)))));
    let b = bus.clone();
    notes_palette::set_on_query(Box::new(move |q| b.send(Msg::Notes(NMsg::Query(q.to_string())))));
    let b = bus.clone();
    notes_palette::set_on_pick(Box::new(move |id| b.send(Msg::Notes(NMsg::Pick(id)))));
    let b = bus.clone();
    notes_palette::set_on_close(Box::new(move || b.send(Msg::Notes(NMsg::PaletteClosed))));
    notes::set_emoji_provider(Box::new(emoji_matches));
    // before the first frame, so a cold launch through a URL still delivers; the closure only sends
    let b = bus.clone();
    ferrocast_macos::url_scheme::install(Box::new(move |r| b.send(Msg::Notes(NMsg::Url(r)))));
}

// ---------------------------------------------------------------- test double

#[cfg(test)]
pub(crate) mod fake {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;

    #[derive(Default)]
    pub struct St {
        pub visible: bool,
        pub focus: usize,
        pub markdown: String,
        pub log: Vec<String>,
        pub zoom: f64,
        pub selection: bool,
        pub levels: Vec<(String, Vec<PaletteItem>)>,
        pub palette_on: bool,
        pub clipboard: Option<String>,
        pub downloads: Option<PathBuf>,
        pub files: HashMap<PathBuf, String>,
        pub ticks: Vec<u64>,
        /// Editor selection (offsets are chars: tests use ASCII).
        pub sel: (usize, usize),
        pub ai_ok: bool,
        pub ai_requests: Vec<AiRequest>,
        pub apply_ok: bool,
        pub dict_status: Option<DictStatus>,
        pub dict_start_err: Option<String>,
        pub dict_active: bool,
        pub access_requests: usize,
        pub urls: Vec<String>,
        /// What `last_keystroke_ms` reports.
        pub keystroke_ms: u64,
    }

    /// A window that is only a record of what was asked of it.
    pub struct FakeFx(pub Rc<RefCell<St>>);

    impl FakeFx {
        pub fn new() -> (FakeFx, Rc<RefCell<St>>) {
            let st = Rc::new(RefCell::new(St {
                zoom: 1.0,
                downloads: Some(PathBuf::from("/dl")),
                ai_ok: true,
                apply_ok: true,
                dict_status: Some(DictStatus::Authorized),
                ..Default::default()
            }));
            (FakeFx(st.clone()), st)
        }
        fn log(&self, s: impl Into<String>) {
            self.0.borrow_mut().log.push(s.into());
        }
    }

    impl NotesFx for FakeFx {
        fn show(&mut self) {
            self.0.borrow_mut().visible = true;
            self.log("show");
        }
        fn hide(&mut self) {
            self.0.borrow_mut().visible = false;
            self.log("hide");
        }
        fn is_visible(&self) -> bool {
            self.0.borrow().visible
        }
        fn focus_editor(&mut self) {
            self.0.borrow_mut().focus += 1;
        }
        fn last_keystroke_ms(&self) -> u64 {
            self.0.borrow().keystroke_ms
        }
        fn set_markdown(&mut self, md: &str) {
            self.0.borrow_mut().markdown = md.into();
            self.log(format!("markdown:{md}"));
        }
        fn apply_format(&mut self, f: Format) {
            self.log(format!("format:{f:?}"));
        }
        fn has_selection(&self) -> bool {
            let s = self.0.borrow();
            s.selection || s.sel.0 != s.sel.1
        }
        fn zoom(&self) -> f64 {
            self.0.borrow().zoom
        }
        fn set_zoom(&mut self, z: f64) {
            self.0.borrow_mut().zoom = z;
            self.log(format!("zoom:{z}"));
        }
        fn set_escape_loses_focus(&mut self, on: bool) {
            self.log(format!("esc:{on}"));
        }
        fn set_format_bar(&mut self, on: bool) {
            self.log(format!("bar:{on}"));
        }
        fn palette_show(&mut self, placeholder: &str, items: Vec<PaletteItem>) {
            let mut s = self.0.borrow_mut();
            s.levels = vec![(placeholder.into(), items)];
            s.palette_on = true;
        }
        fn palette_set_items(&mut self, items: Vec<PaletteItem>) {
            if let Some(l) = self.0.borrow_mut().levels.last_mut() {
                l.1 = items;
            }
        }
        fn palette_push(&mut self, placeholder: &str, items: Vec<PaletteItem>) {
            self.0.borrow_mut().levels.push((placeholder.into(), items));
        }
        fn palette_hide(&mut self) {
            let mut s = self.0.borrow_mut();
            s.palette_on = false;
            s.levels.clear();
        }
        fn palette_visible(&self) -> bool {
            self.0.borrow().palette_on
        }
        fn palette_depth(&self) -> usize {
            let s = self.0.borrow();
            if s.palette_on {
                s.levels.len()
            } else {
                0
            }
        }
        fn palette_empty_text(&mut self, _text: &str) {}
        fn set_clipboard(&mut self, text: &str) {
            self.0.borrow_mut().clipboard = Some(text.into());
        }
        fn downloads_dir(&self) -> Option<PathBuf> {
            self.0.borrow().downloads.clone()
        }
        fn file_exists(&self, p: &Path) -> bool {
            self.0.borrow().files.contains_key(p)
        }
        fn write_file(&mut self, p: &Path, contents: &str) -> Result<(), String> {
            self.0.borrow_mut().files.insert(p.to_path_buf(), contents.into());
            Ok(())
        }
        fn reveal(&mut self, p: &Path) {
            self.log(format!("reveal:{}", p.display()));
        }
        fn schedule_tick(&mut self, ms: u64) {
            self.0.borrow_mut().ticks.push(ms);
        }
        fn selection_range(&self) -> (usize, usize) {
            self.0.borrow().sel
        }
        fn select_range(&mut self, start: usize, end: usize) {
            self.0.borrow_mut().sel = (start, end);
            self.log(format!("select:{start}-{end}"));
        }
        fn selection_markdown(&self) -> String {
            let s = self.0.borrow();
            s.markdown.chars().skip(s.sel.0).take(s.sel.1 - s.sel.0).collect()
        }
        fn replace_selection_markdown(&mut self, md: &str) -> bool {
            let mut s = self.0.borrow_mut();
            if !s.apply_ok || md.trim().is_empty() {
                return false;
            }
            let (a, b) = if s.sel.0 == s.sel.1 { (0, s.markdown.chars().count()) } else { s.sel };
            let chars: Vec<char> = s.markdown.chars().collect();
            s.markdown = chars[..a].iter().collect::<String>() + md + &chars[b..].iter().collect::<String>();
            s.log.push(format!("replace-selection:{md}"));
            true
        }
        fn append_markdown(&mut self, md: &str) -> bool {
            let mut s = self.0.borrow_mut();
            if !s.apply_ok || md.trim().is_empty() {
                return false;
            }
            s.markdown = if s.markdown.is_empty() { md.to_string() } else { format!("{}\n\n{md}", s.markdown.trim_end()) };
            s.log.push(format!("append:{md}"));
            true
        }
        fn insert_markdown_after_selection_line(&mut self, md: &str) -> bool {
            let mut s = self.0.borrow_mut();
            if !s.apply_ok || md.trim().is_empty() {
                return false;
            }
            let chars: Vec<char> = s.markdown.chars().collect();
            let end = (s.sel.1..chars.len()).find(|i| chars[*i] == '\n').unwrap_or(chars.len());
            s.markdown = chars[..end].iter().collect::<String>() + "\n\n" + md + &chars[end..].iter().collect::<String>();
            s.log.push(format!("insert-after-line:{md}"));
            true
        }
        fn replace_range(&mut self, start: usize, end: usize, text: &str) {
            let mut s = self.0.borrow_mut();
            let chars: Vec<char> = s.markdown.chars().collect();
            s.markdown = chars[..start].iter().collect::<String>() + text + &chars[end..].iter().collect::<String>();
            s.sel = (start + text.chars().count(), start + text.chars().count());
            s.log.push(format!("replace-range:{start}-{end}:{text}"));
        }
        fn text_in_range(&self, start: usize, end: usize) -> Option<String> {
            let s = self.0.borrow();
            let chars: Vec<char> = s.markdown.chars().collect();
            (start <= end && end <= chars.len()).then(|| chars[start..end].iter().collect())
        }
        fn ai_available(&self, _s: &Settings) -> bool {
            self.0.borrow().ai_ok
        }
        fn ai_start(&mut self, req: AiRequest) {
            self.log(format!("ai-start:{}", req.command_id));
            self.0.borrow_mut().ai_requests.push(req);
        }
        fn dictation_status(&self) -> DictStatus {
            self.0.borrow().dict_status.expect("status")
        }
        fn dictation_request_access(&mut self) {
            self.0.borrow_mut().access_requests += 1;
        }
        fn dictation_start(&mut self) -> Result<(), String> {
            let mut s = self.0.borrow_mut();
            if let Some(e) = s.dict_start_err.clone() {
                return Err(e);
            }
            s.dict_active = true;
            s.log.push("dictation:start".into());
            Ok(())
        }
        fn dictation_stop(&mut self) {
            let mut s = self.0.borrow_mut();
            s.dict_active = false;
            s.log.push("dictation:stop".into());
        }
        fn open_url(&mut self, url: &str) {
            self.0.borrow_mut().urls.push(url.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::*;
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn note(id: &str, text: &str, modified: i64) -> Note {
        Note { id: id.into(), text: text.into(), created: 0, modified, pinned_at: None, deleted_at: None }
    }

    struct W {
        notes: Vec<Note>,
        settings: Settings,
        ql: Vec<Quicklink>,
        now: i64,
        now_ms: u64,
        ctl: NotesCtl,
        st: Rc<RefCell<St>>,
    }

    impl W {
        fn new(notes: Vec<Note>) -> W {
            let (fx, st) = FakeFx::new();
            W { notes, settings: Settings::default(), ql: vec![], now: 1000, now_ms: 10_000, ctl: NotesCtl::new(Box::new(fx)), st }
        }
        fn run(&mut self, f: impl FnOnce(&mut NotesCtl, &mut Data) -> Out) -> Out {
            let mut d =
                Data { notes: &mut self.notes, settings: &mut self.settings, quicklinks: &mut self.ql, now: self.now, now_ms: self.now_ms };
            f(&mut self.ctl, &mut d)
        }
        fn msg(&mut self, m: NMsg) -> Out {
            self.run(|c, d| c.handle(m, d))
        }
        fn ev(&mut self, e: Event) -> Out {
            self.msg(NMsg::Event(e))
        }
        fn pick(&mut self, id: &str) -> Out {
            self.msg(NMsg::Pick(id.into()))
        }
        fn toggle(&mut self) -> Out {
            self.run(|c, d| c.toggle(d))
        }
        fn open(&mut self, id: &str) {
            self.run(|c, d| c.open_and_show(d, id));
        }
        fn log(&self) -> Vec<String> {
            self.st.borrow().log.clone()
        }
        fn level_ids(&self) -> Vec<String> {
            self.st.borrow().levels.last().map(|l| l.1.iter().map(|i| i.id.clone()).collect()).unwrap_or_default()
        }
        fn live(&self) -> usize {
            self.notes.iter().filter(|n| n.deleted_at.is_none()).count()
        }
        fn cur(&self) -> Option<&str> {
            self.ctl.current.as_deref()
        }
        fn by_id(&self, id: &str) -> &Note {
            self.notes.iter().find(|n| n.id == id).unwrap()
        }
    }

    // ---- pure helpers

    #[test]
    fn debounce_rule_is_a_function_of_timestamps() {
        assert!(debounce_due(None, 0));
        assert!(!debounce_due(Some(1000), 1000));
        assert!(!debounce_due(Some(1000), 1499));
        assert!(debounce_due(Some(1000), 1500));
        assert!(debounce_due(Some(1000), 9000));
        assert!(debounce_due(Some(5000), 100), "a clock that went back never blocks saving");
        assert_eq!(debounce_wait(None, 5), 0);
        assert_eq!(debounce_wait(Some(1000), 1200), 300);
        assert_eq!(debounce_wait(Some(1000), 1500), 0);
        assert_eq!(debounce_wait(Some(1000), 2000), 0);
    }

    #[test]
    fn export_names_never_overwrite() {
        let taken = ["Groceries.md", "Groceries 2.md"];
        let ex = |n: &str| taken.contains(&n);
        assert_eq!(unique_file_name("Groceries.md", &ex), "Groceries 3.md");
        assert_eq!(unique_file_name("Other.md", &ex), "Other.md");
        assert_eq!(unique_file_name("Groceries.txt", &ex), "Groceries.txt");
        let ex = |n: &str| n == "noext";
        assert_eq!(unique_file_name("noext", &ex), "noext 2");
        let ex = |n: &str| n == ".md";
        assert_eq!(unique_file_name(".md", &ex), ".md 2");
    }

    #[test]
    fn initial_note_order() {
        let mut v = vec![note("a", "A", 1), note("b", "B", 9), note("c", "C", 5)];
        assert_eq!(initial_id(&v, Some("a")).as_deref(), Some("a"), "the last one wins");
        assert_eq!(initial_id(&v, Some("zzz")).as_deref(), Some("b"), "unknown: most recent");
        assert_eq!(initial_id(&v, None).as_deref(), Some("b"));
        v[0].deleted_at = Some(1);
        assert_eq!(initial_id(&v, Some("a")).as_deref(), Some("b"), "a deleted last note is skipped");
        v[1].deleted_at = Some(1);
        v[2].deleted_at = Some(1);
        assert_eq!(initial_id(&v, Some("a")), None);
        assert_eq!(initial_id(&[], None), None);
    }

    #[test]
    fn every_format_name_maps_to_an_editor_format() {
        for (name, _, _) in note_actions::FORMATS {
            assert!(format_for(name).is_some(), "{name}");
        }
        assert_eq!(format_for("heading2"), Some(Format::Heading(2)));
        assert_eq!(format_for("task"), Some(Format::TaskList));
        assert_eq!(format_for("nope"), None);
    }

    #[test]
    fn browse_rows_become_palette_rows() {
        let mut v = vec![note("a", "Alpha\nbody", 5), note("b", "Beta", 9), note("c", "Gamma", 1)];
        notes::set_pinned(&mut v, "c", true, 10);
        let rows = note_actions::browse_rows(&v, "", Some("b"), 100);
        let plain = browse_items(&rows, false);
        let ids: Vec<&str> = plain.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["c", "b", "a"]);
        assert_eq!(plain[0].glyph.as_deref(), Some("sf:pin.fill"));
        assert_eq!(plain[0].accessory.as_deref(), Some("⌘1"));
        assert_eq!(plain[0].section.as_deref(), Some("Pinned"));
        assert_eq!((plain[1].glyph.clone(), plain[1].accessory.clone()), (None, None));
        assert_eq!(plain[1].section.as_deref(), Some("Notes"));
        assert_eq!(plain[0].title, "Gamma");
        assert!(plain[2].subtitle.as_deref().unwrap().contains("body"));
        let first = browse_items(&rows, true);
        let ids: Vec<&str> = first.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["b", "c", "a"], "the open note leads so it starts selected");
    }

    #[test]
    fn action_entries_become_palette_rows() {
        let ctx = ActCtx { has_note: true, note_count: 2, ai_available: true, quicklinks_available: true, ..Default::default() };
        let rows = action_items(&note_actions::top_level(&ctx));
        let new = rows.iter().find(|r| r.id == "new").unwrap();
        assert_eq!((new.title.as_str(), new.accessory.as_deref(), new.section.as_deref()), ("New Note", Some("⌘N"), Some("Note")));
        let del = rows.iter().find(|r| r.id == "delete").unwrap();
        assert!(del.danger && !del.has_children);
        assert!(rows.iter().find(|r| r.id == "format").unwrap().has_children);
        assert!(rows.iter().all(|r| NoteAction::from_id(&r.id).is_some()), "every id is an action id");
    }

    // ---- opening

    #[test]
    fn toggle_on_an_empty_store_creates_a_note_and_focuses_the_editor() {
        let mut w = W::new(vec![]);
        let out = w.toggle();
        assert_eq!(w.notes.len(), 1);
        let id = w.notes[0].id.clone();
        assert_eq!(w.cur(), Some(id.as_str()));
        assert!(w.st.borrow().visible);
        assert!(out.save_notes && out.save_settings);
        assert_eq!(w.settings.notes_last, Some(id));
        assert!(w.st.borrow().focus >= 1 || w.log().contains(&"show".to_string()));
    }

    #[test]
    fn toggle_reopens_the_last_note_else_the_most_recent() {
        let mut w = W::new(vec![note("old", "Old", 1), note("new", "New", 9)]);
        w.settings.notes_last = Some("old".into());
        w.toggle();
        assert_eq!(w.cur(), Some("old"));
        assert_eq!(w.st.borrow().markdown, "Old");
        let mut w = W::new(vec![note("old", "Old", 1), note("new", "New", 9)]);
        w.settings.notes_last = Some("gone".into());
        w.toggle();
        assert_eq!(w.cur(), Some("new"));
        let mut w = W::new(vec![note("old", "Old", 1), note("new", "New", 9)]);
        w.notes[0].deleted_at = Some(5);
        w.settings.notes_last = Some("old".into());
        w.toggle();
        assert_eq!(w.cur(), Some("new"), "a deleted last note is not reopened");
    }

    #[test]
    fn toggle_hides_a_visible_window_and_keeps_the_note_loaded() {
        let mut w = W::new(vec![note("a", "A", 1)]);
        w.toggle();
        w.toggle();
        assert!(!w.st.borrow().visible);
        let loads = w.log().iter().filter(|l| l.starts_with("markdown:")).count();
        w.toggle();
        assert!(w.st.borrow().visible);
        assert_eq!(w.log().iter().filter(|l| l.starts_with("markdown:")).count(), loads, "no reload (undo and cursor stay)");
    }

    #[test]
    fn settings_are_applied_once_when_the_window_is_first_used() {
        let mut w = W::new(vec![note("a", "A", 1)]);
        w.settings.notes_zoom = 1.4;
        w.settings.notes_escape_loses_focus = true;
        assert!(w.settings.notes_format_bar, "the Format Bar is on by default");
        w.toggle();
        let log = w.log();
        assert!(
            log.contains(&"zoom:1.4".to_string()) && log.contains(&"esc:true".to_string()) && log.contains(&"bar:true".to_string()),
            "{log:?}"
        );
        w.toggle();
        w.toggle();
        assert_eq!(w.log().iter().filter(|l| l.starts_with("esc:")).count(), 1);
    }

    #[test]
    fn create_note_opens_the_window_on_a_new_empty_note() {
        let mut w = W::new(vec![note("a", "A", 1)]);
        let out = w.run(|c, d| c.create_note(d));
        assert_eq!(w.live(), 2);
        let id = w.cur().unwrap().to_string();
        assert_ne!(id, "a");
        assert_eq!(w.by_id(&id).text, "");
        assert!(w.st.borrow().visible && out.save_notes);
        assert_eq!(w.settings.notes_last, Some(id));
    }

    #[test]
    fn opening_a_note_saves_the_current_one_and_records_history() {
        let mut w = W::new(vec![note("a", "A", 1), note("b", "B", 2)]);
        w.open("a");
        w.msg(NMsg::Changed("A edit".into()));
        w.now_ms += 10;
        w.msg(NMsg::Changed("A edited".into()));
        let out = w.run(|c, d| c.open_and_show(d, "b"));
        assert!(out.save_notes, "the pending text is written when another note opens");
        assert_eq!(w.by_id("a").text, "A edited");
        assert_eq!(w.cur(), Some("b"));
        assert_eq!(w.st.borrow().markdown, "B");
        assert_eq!(w.ctl.history.current(), Some("b"));
        assert_eq!(w.ctl.history.clone().back(), Some("a"));
        assert_eq!(w.settings.notes_last.as_deref(), Some("b"));
    }

    #[test]
    fn opening_a_deleted_or_unknown_note_does_nothing() {
        let mut w = W::new(vec![note("a", "A", 1), note("b", "B", 2)]);
        w.notes[1].deleted_at = Some(3);
        w.open("a");
        w.open("b");
        w.open("zzz");
        assert_eq!(w.cur(), Some("a"));
    }

    // ---- empty notes

    #[test]
    fn leaving_an_empty_note_discards_it_unless_it_is_the_only_one() {
        let mut w = W::new(vec![note("a", "A", 1), note("e", "", 2)]);
        w.open("e");
        w.open("a");
        assert!(w.notes.iter().all(|n| n.id != "e"), "discarded for good, not moved to Recently Deleted");
        let mut w = W::new(vec![note("e", "", 2)]);
        w.open("e");
        w.ev(Event::Hidden);
        assert_eq!(w.notes.len(), 1, "the only note stays");
        assert_eq!(w.cur(), Some("e"));
    }

    #[test]
    fn hiding_the_window_discards_an_empty_extra_note() {
        let mut w = W::new(vec![note("a", "A", 1), note("e", "", 2)]);
        w.open("e");
        let out = w.ev(Event::Hidden);
        assert_eq!(w.notes.len(), 1);
        assert!(out.save_notes);
        assert_eq!(w.cur(), None);
        w.st.borrow_mut().visible = false;
        w.toggle();
        assert_eq!(w.cur(), Some("a"));
    }

    // ---- editing and the debounce

    #[test]
    fn typing_saves_at_most_every_500_ms_and_a_timer_catches_up() {
        let mut w = W::new(vec![note("a", "A", 1)]);
        w.open("a");
        let first = w.msg(NMsg::Changed("A1".into()));
        assert!(first.save_notes, "nothing was saved yet");
        assert_eq!(w.by_id("a").modified, 1000);
        w.now_ms += 100;
        w.now += 1;
        let second = w.msg(NMsg::Changed("A12".into()));
        assert!(!second.save_notes, "100 ms after a save");
        assert_eq!(w.st.borrow().ticks, vec![400], "a timer for the rest of the window");
        w.now_ms += 100;
        let third = w.msg(NMsg::Changed("A123".into()));
        assert!(!third.save_notes);
        assert_eq!(w.st.borrow().ticks.len(), 1, "one timer at a time");
        assert_eq!(w.by_id("a").text, "A123");
        w.now_ms += 300;
        let tick = w.msg(NMsg::Tick);
        assert!(tick.save_notes, "the timer writes what is pending");
        let again = w.msg(NMsg::Tick);
        assert!(!again.save_notes, "nothing left to write");
    }

    #[test]
    fn an_early_timer_reschedules_and_unchanged_text_is_free() {
        let mut w = W::new(vec![note("a", "A", 1)]);
        w.open("a");
        w.msg(NMsg::Changed("A1".into()));
        w.now_ms += 50;
        w.msg(NMsg::Changed("A12".into()));
        w.now_ms += 50;
        let t = w.msg(NMsg::Tick);
        assert!(!t.save_notes);
        assert_eq!(w.st.borrow().ticks, vec![450, 400], "still 400 ms short of a save 100 ms ago");
        let m = w.by_id("a").modified;
        w.now += 50;
        w.msg(NMsg::Changed("A12".into()));
        assert_eq!(w.by_id("a").modified, m, "same text keeps `modified`");
    }

    #[test]
    fn hiding_the_window_flushes_pending_text() {
        let mut w = W::new(vec![note("a", "A", 1)]);
        w.open("a");
        w.msg(NMsg::Changed("A1".into()));
        w.now_ms += 10;
        assert!(!w.msg(NMsg::Changed("A12".into())).save_notes);
        let out = w.ev(Event::Hidden);
        assert!(out.save_notes);
        assert!(!w.ev(Event::Hidden).save_notes, "nothing pending the second time");
        w.msg(NMsg::Changed("A123".into()));
        assert!(w.run(|c, d| c.flush(d)).save_notes);
        assert_eq!(w.by_id("a").text, "A123");
    }

    #[test]
    fn flush_on_quit_writes_pending_text() {
        let mut w = W::new(vec![note("a", "A", 1)]);
        w.open("a");
        w.msg(NMsg::Changed("A1".into()));
        w.now_ms += 10;
        w.msg(NMsg::Changed("A12".into()));
        assert!(w.run(|c, d| c.flush(d)).save_notes);
        assert!(!w.run(|c, d| c.flush(d)).save_notes);
    }

    #[test]
    fn a_change_with_no_open_note_is_ignored() {
        let mut w = W::new(vec![note("a", "A", 1)]);
        let out = w.msg(NMsg::Changed("zzz".into()));
        assert_eq!(out, Out::default());
        assert_eq!(w.by_id("a").text, "A");
    }

    // ---- browse

    #[test]
    fn browse_lists_notes_refilters_and_opens_the_pick() {
        let mut w = W::new(vec![note("a", "Alpha", 1), note("b", "Beta", 2), note("c", "Gamma apple", 3)]);
        notes::set_pinned(&mut w.notes, "a", true, 5);
        w.open("b");
        w.ev(Event::Browse);
        assert_eq!(w.ctl.mode, Mode::Browse);
        assert_eq!(w.level_ids(), ["b", "a", "c"], "open note first, then pinned, then recent");
        w.msg(NMsg::Query("apple".into()));
        assert_eq!(w.level_ids(), ["c"]);
        w.msg(NMsg::Query("".into()));
        assert_eq!(w.level_ids(), ["a", "c", "b"], "pinned, then the most recently edited");
        w.pick("c");
        assert_eq!(w.cur(), Some("c"));
        assert!(!w.st.borrow().palette_on);
        assert_eq!(w.ctl.mode, Mode::None);
    }

    #[test]
    fn a_stale_close_message_does_not_clear_a_palette_that_is_open_again() {
        let mut w = W::new(vec![note("a", "A", 1)]);
        w.open("a");
        w.ev(Event::Browse);
        w.msg(NMsg::PaletteClosed);
        assert_eq!(w.ctl.mode, Mode::Browse);
        w.st.borrow_mut().palette_on = false;
        w.msg(NMsg::PaletteClosed);
        assert_eq!(w.ctl.mode, Mode::None);
    }

    // ---- actions palette

    fn open_actions(w: &mut W) {
        w.ev(Event::Actions);
        assert_eq!(w.ctl.mode, Mode::Actions);
    }

    #[test]
    fn the_action_panel_lists_the_root_and_a_leaf_executes() {
        let mut w = W::new(vec![note("a", "A", 1)]);
        w.open("a");
        open_actions(&mut w);
        let ids = w.level_ids();
        for want in ["new", "browse", "pin", "format", "format-bar", "copy-as", "export", "quicklink", "ai", "delete", "delete-all"] {
            assert!(ids.contains(&want.to_string()), "{want} in {ids:?}");
        }
        w.pick("zoom-in");
        assert!(w.log().contains(&"zoom:1.1".to_string()), "{:?}", w.log());
        assert!(!w.st.borrow().palette_on, "a leaf closes the panel");
        assert_eq!(w.ctl.mode, Mode::None);
    }

    #[test]
    fn a_row_with_children_opens_a_level_and_its_leaf_runs() {
        let mut w = W::new(vec![note("a", "A", 1)]);
        w.open("a");
        open_actions(&mut w);
        w.pick("format");
        assert_eq!(w.ctl.mode, Mode::Submenu(NoteAction::FormatMenu));
        assert_eq!(w.st.borrow().levels.len(), 2);
        assert!(w.level_ids().contains(&"format:bold".to_string()));
        w.msg(NMsg::Query("head".into()));
        assert_eq!(w.level_ids(), ["format:heading1", "format:heading2", "format:heading3"]);
        w.pick("format:heading2");
        assert!(w.log().contains(&"format:Heading(2)".to_string()), "{:?}", w.log());
        assert!(!w.st.borrow().palette_on);
    }

    #[test]
    fn typing_in_the_panel_searches_every_level_and_popping_a_level_resets_the_mode() {
        let mut w = W::new(vec![note("a", "A", 1)]);
        w.open("a");
        open_actions(&mut w);
        w.msg(NMsg::Query("bold".into()));
        assert!(w.level_ids().contains(&"format:bold".to_string()));
        w.pick("format:bold");
        assert!(w.log().contains(&"format:Bold".to_string()));
        open_actions(&mut w);
        w.pick("copy-as");
        assert_eq!(w.ctl.mode, Mode::Submenu(NoteAction::CopyNoteAs));
        w.st.borrow_mut().levels.pop();
        w.msg(NMsg::Query("new".into()));
        assert_eq!(w.ctl.mode, Mode::Actions, "the palette popped back to the root");
        assert!(w.level_ids().contains(&"new".to_string()));
    }

    #[test]
    fn browse_notes_action_switches_the_panel_to_browse() {
        let mut w = W::new(vec![note("a", "A", 1), note("b", "B", 2)]);
        w.open("a");
        open_actions(&mut w);
        w.pick("browse");
        assert_eq!(w.ctl.mode, Mode::Browse);
        assert!(w.st.borrow().palette_on);
        assert_eq!(w.level_ids(), ["a", "b"]);
    }

    #[test]
    fn the_actions_event_toggles_the_panel() {
        let mut w = W::new(vec![note("a", "A", 1)]);
        w.open("a");
        open_actions(&mut w);
        w.ev(Event::Actions);
        assert!(!w.st.borrow().palette_on);
        assert_eq!(w.ctl.mode, Mode::None);
    }

    #[test]
    fn hidden_closes_the_palette_and_clears_the_mode() {
        let mut w = W::new(vec![note("a", "A", 1)]);
        w.open("a");
        w.ev(Event::Browse);
        w.ev(Event::Hidden);
        assert!(!w.st.borrow().palette_on);
        assert_eq!(w.ctl.mode, Mode::None);
    }

    // ---- pinning and history

    #[test]
    fn toggle_pin_pins_unpins_and_keeps_the_slot() {
        let mut w = W::new(vec![note("a", "A", 1), note("b", "B", 2)]);
        w.open("a");
        let out = w.ev(Event::TogglePin);
        assert!(out.save_notes);
        assert_eq!(w.by_id("a").pinned_at, Some(1000));
        w.now += 50;
        w.open("b");
        w.ev(Event::TogglePin);
        assert_eq!(notes::pinned_slot(&w.notes, 1).as_deref(), Some("a"));
        assert_eq!(notes::pinned_slot(&w.notes, 2).as_deref(), Some("b"));
        w.ev(Event::TogglePin);
        assert_eq!(w.by_id("b").pinned_at, None);
        assert_eq!(w.by_id("a").pinned_at, Some(1000), "the other slot is untouched");
    }

    #[test]
    fn the_pin_action_follows_the_state() {
        let mut w = W::new(vec![note("a", "A", 1)]);
        w.open("a");
        open_actions(&mut w);
        assert!(w.level_ids().contains(&"pin".to_string()));
        w.pick("pin");
        assert!(w.by_id("a").pinned_at.is_some());
        open_actions(&mut w);
        assert!(w.level_ids().contains(&"unpin".to_string()));
        w.pick("unpin");
        assert!(w.by_id("a").pinned_at.is_none());
    }

    #[test]
    fn pinned_slots_open_their_note() {
        let mut w = W::new(vec![note("a", "A", 1), note("b", "B", 2)]);
        notes::set_pinned(&mut w.notes, "b", true, 3);
        w.open("a");
        w.ev(Event::Pinned(1));
        assert_eq!(w.cur(), Some("b"));
        w.ev(Event::Pinned(2));
        assert_eq!(w.cur(), Some("b"), "an empty slot does nothing");
        w.ev(Event::Pinned(9));
        assert_eq!(w.cur(), Some("b"));
    }

    #[test]
    fn back_and_forward_move_through_history_without_adding_to_it() {
        let mut w = W::new(vec![note("a", "A", 1), note("b", "B", 2), note("c", "C", 3)]);
        w.open("a");
        w.open("b");
        w.open("c");
        w.ev(Event::Back);
        assert_eq!(w.cur(), Some("b"));
        w.ev(Event::Back);
        assert_eq!(w.cur(), Some("a"));
        w.ev(Event::Back);
        assert_eq!(w.cur(), Some("a"), "at the start");
        w.ev(Event::Forward);
        assert_eq!(w.cur(), Some("b"));
        w.ev(Event::Forward);
        w.ev(Event::Forward);
        assert_eq!(w.cur(), Some("c"));
        w.ev(Event::Back);
        w.pick_action("back");
        assert_eq!(w.cur(), Some("a"));
    }

    impl W {
        fn pick_action(&mut self, id: &str) {
            self.ev(Event::Actions);
            self.pick(id);
        }
    }

    #[test]
    fn back_skips_notes_that_are_gone() {
        let mut w = W::new(vec![note("a", "A", 1), note("b", "B", 2), note("c", "C", 3)]);
        w.open("a");
        w.open("b");
        w.open("c");
        w.notes[1].deleted_at = Some(9);
        w.ev(Event::Back);
        assert_eq!(w.cur(), Some("a"));
    }

    // ---- deleting

    #[test]
    fn deleting_the_current_note_opens_the_previous_one_in_history() {
        let mut w = W::new(vec![note("a", "A", 1), note("b", "B", 2), note("c", "C", 3)]);
        w.open("a");
        w.open("c");
        w.pick_action("delete");
        assert!(w.by_id("c").deleted_at.is_some());
        assert_eq!(w.cur(), Some("a"));
        assert_eq!(w.ctl.history.current(), Some("a"));
        assert!(w.ctl.history.clone().back().is_none_or(|x| x != "c"));
    }

    #[test]
    fn deleting_with_no_history_opens_the_most_recent_then_a_new_note() {
        let mut w = W::new(vec![note("a", "A", 1), note("b", "B", 7)]);
        w.open("a");
        let out = w.run(|c, d| c.delete_id(d, "a"));
        assert!(out.save_notes);
        assert_eq!(w.cur(), Some("b"));
        w.run(|c, d| c.delete_id(d, "b"));
        assert_eq!(w.live(), 1);
        let id = w.cur().unwrap().to_string();
        assert!(w.by_id(&id).text.is_empty(), "a new empty note");
        assert_eq!(w.notes.len(), 3, "the two deleted ones are kept");
    }

    #[test]
    fn delete_all_moves_everything_to_recently_deleted_and_opens_a_new_note() {
        let mut w = W::new(vec![note("a", "A", 1), note("b", "B", 2)]);
        w.open("a");
        w.pick_action("delete-all");
        assert!(w.by_id("a").deleted_at.is_some() && w.by_id("b").deleted_at.is_some());
        assert_eq!(w.live(), 1);
        let id = w.cur().unwrap().to_string();
        assert_eq!(w.by_id(&id).text, "");
        assert_eq!(w.ctl.history.clone().back(), None);
    }

    #[test]
    fn deleting_from_the_launcher_while_the_window_is_hidden_only_forgets_the_note() {
        let mut w = W::new(vec![note("a", "A", 1), note("b", "B", 2)]);
        w.open("a");
        w.ev(Event::Hidden);
        w.st.borrow_mut().visible = false;
        w.run(|c, d| c.delete_id(d, "a"));
        assert_eq!(w.cur(), None);
        assert!(!w.st.borrow().visible);
        w.toggle();
        assert_eq!(w.cur(), Some("b"));
    }

    #[test]
    fn recently_deleted_level_restores_and_opens_the_pick() {
        let mut w = W::new(vec![note("a", "A", 1), note("b", "B", 2)]);
        w.open("a");
        w.run(|c, d| c.delete_id(d, "b"));
        w.pick_action("deleted");
        assert_eq!(w.ctl.mode, Mode::RecentlyDeleted);
        assert_eq!(w.level_ids(), ["b"]);
        w.msg(NMsg::Query("zzz".into()));
        assert!(w.level_ids().is_empty());
        w.msg(NMsg::Query("".into()));
        w.pick("b");
        assert_eq!(w.by_id("b").deleted_at, None);
        assert_eq!(w.cur(), Some("b"));
        assert!(!w.st.borrow().palette_on);
    }

    // ---- sharing

    #[test]
    fn copy_as_puts_the_export_on_the_clipboard() {
        let mut w = W::new(vec![note("a", "# Title\n**bold** text", 1)]);
        w.open("a");
        open_actions(&mut w);
        w.pick("copy-as");
        let out = w.pick("copy-as:markdown");
        assert_eq!(w.st.borrow().clipboard.as_deref(), Some("# Title\n**bold** text"));
        assert!(out.toast.unwrap().contains("Markdown"));
        w.pick_action("copy-as");
        w.pick("copy-as:plain");
        let plain = w.st.borrow().clipboard.clone().unwrap();
        assert!(!plain.contains("**") && plain.contains("bold text"), "{plain:?}");
        w.pick_action("copy-as");
        w.pick("copy-as:html");
        assert!(w.st.borrow().clipboard.clone().unwrap().contains("<strong>bold</strong>"));
    }

    #[test]
    fn export_writes_to_downloads_without_overwriting_and_reveals_the_file() {
        let mut w = W::new(vec![note("a", "# Groceries\nmilk", 1)]);
        w.open("a");
        w.pick_action("export");
        let out = w.pick("export:markdown");
        assert_eq!(w.st.borrow().files.get(Path::new("/dl/Groceries.md")).map(String::as_str), Some("# Groceries\nmilk"));
        assert!(w.log().contains(&"reveal:/dl/Groceries.md".to_string()));
        assert!(out.toast.unwrap().contains("Groceries.md"));
        w.pick_action("export");
        let out = w.pick("export:markdown");
        assert!(w.st.borrow().files.contains_key(Path::new("/dl/Groceries 2.md")));
        assert!(out.toast.unwrap().contains("Groceries 2.md"));
        w.pick_action("export");
        w.pick("export:plain");
        assert!(w.st.borrow().files.contains_key(Path::new("/dl/Groceries.txt")));
        w.pick_action("export");
        w.pick("export:html");
        assert!(w.st.borrow().files.get(Path::new("/dl/Groceries.html")).unwrap().contains("<title>Groceries</title>"));
    }

    #[test]
    fn export_without_a_downloads_folder_says_so() {
        let mut w = W::new(vec![note("a", "A", 1)]);
        w.open("a");
        w.st.borrow_mut().downloads = None;
        w.pick_action("export");
        let out = w.pick("export:markdown");
        assert!(out.toast.unwrap().contains("Downloads"));
        assert!(w.st.borrow().files.is_empty());
    }

    #[test]
    fn create_quicklink_adds_one_link_per_note() {
        let mut w = W::new(vec![note("abc-1", "# My note", 1)]);
        w.open("abc-1");
        w.pick_action("quicklink");
        assert_eq!(w.ql.len(), 1);
        assert_eq!((w.ql[0].name.as_str(), w.ql[0].link.as_str()), ("My note", "ferrocast://notes/abc-1"));
        let before = w.ql.clone();
        let out = {
            w.ev(Event::Actions);
            w.pick("quicklink")
        };
        assert_eq!(w.ql, before, "no duplicate");
        assert!(out.toast.unwrap().contains("already exists"));
        assert!(!out.save_quicklinks);
    }

    #[test]
    fn create_quicklink_reports_the_save() {
        let mut w = W::new(vec![note("abc-1", "# My note", 1)]);
        w.open("abc-1");
        w.ev(Event::Actions);
        let out = w.pick("quicklink");
        assert!(out.save_quicklinks && out.toast.is_some());
    }

    // ---- view and settings

    #[test]
    fn zoom_actions_step_and_reset_and_the_zoom_callback_persists() {
        let mut w = W::new(vec![note("a", "A", 1)]);
        w.open("a");
        w.pick_action("zoom-in");
        w.pick_action("zoom-in");
        assert_eq!(w.st.borrow().zoom, 1.2);
        w.pick_action("zoom-out");
        assert_eq!(w.st.borrow().zoom, 1.1);
        w.pick_action("zoom-reset");
        assert_eq!(w.st.borrow().zoom, 1.0);
        let out = w.msg(NMsg::Zoom(1.3));
        assert!(out.save_settings);
        assert_eq!(w.settings.notes_zoom, 1.3);
        assert!(!w.msg(NMsg::Zoom(1.3)).save_settings, "unchanged");
    }

    #[test]
    fn a_saved_hidden_format_bar_is_applied_at_first_use() {
        let mut w = W::new(vec![note("a", "A", 1)]);
        w.settings.notes_format_bar = false;
        w.toggle();
        assert!(w.log().contains(&"bar:false".to_string()), "{:?}", w.log());
    }

    #[test]
    fn format_bar_action_flips_the_setting_and_the_window() {
        let mut w = W::new(vec![note("a", "A", 1)]);
        w.open("a");
        assert!(w.settings.notes_format_bar, "visible by default");
        w.ev(Event::Actions);
        assert!(w.st.borrow().levels[0].1.iter().any(|i| i.title == "Hide Format Bar"));
        let out = w.pick("format-bar");
        assert!(out.save_settings && !w.settings.notes_format_bar);
        assert!(w.log().contains(&"bar:false".to_string()));
        w.ev(Event::Actions);
        assert!(w.st.borrow().levels[0].1.iter().any(|i| i.title == "Show Format Bar"));
        let out = w.pick("format-bar");
        assert!(out.save_settings && w.settings.notes_format_bar);
        assert!(w.log().contains(&"bar:true".to_string()));
    }

    // ---- emoji, AI helpers

    #[test]
    fn emoji_provider_finds_smiling_faces_cheaply() {
        let hits = emoji_matches("smi");
        assert!(!hits.is_empty() && hits.len() <= MAX_EMOJI, "{}", hits.len());
        assert!(hits.iter().any(|(_, n)| n.contains("smiling")), "{hits:?}");
        assert!(hits.iter().all(|(g, n)| !g.is_empty() && !n.is_empty()));
        let first = emoji_matches("grin");
        assert!(first[0].1.starts_with("grin"), "names that start with the query lead: {first:?}");
        assert!(emoji_matches("zzzzqqqq").is_empty());
        assert!(emoji_matches("SMI").iter().any(|(_, n)| n.contains("smiling")), "case-insensitive");
        assert!(emoji_matches("").len() <= MAX_EMOJI);
    }

    #[test]
    fn ai_usability_follows_the_provider() {
        use ferrocast_core::models::AiProvider as P;
        assert!(ai_usable(P::OpenAiCompatible, false, false, false), "a local server needs no key");
        assert!(ai_usable(P::Anthropic, true, false, false));
        assert!(!ai_usable(P::Anthropic, false, true, true));
        assert!(ai_usable(P::ChatGpt, false, true, false));
        assert!(!ai_usable(P::ChatGpt, true, false, true));
        assert!(ai_usable(P::ClaudeCode, false, false, true));
        assert!(!ai_usable(P::ClaudeCode, true, true, false));
    }

    #[test]
    fn asking_retries_once_when_the_answer_has_no_usable_text() {
        let mut n = 0;
        let r = ask_with_retry(
            &mut |_| {
                n += 1;
                Ok("{\"text\": \"fine\"}".into())
            },
            "p",
        );
        assert_eq!((r, n), (Ok("{\"text\": \"fine\"}".to_string()), 1));
        let mut n = 0;
        let r = ask_with_retry(
            &mut |_| {
                n += 1;
                Ok(if n == 1 { "{\"text\": 5}" } else { "{\"text\": \"second\"}" }.into())
            },
            "p",
        );
        assert_eq!((r, n), (Ok("{\"text\": \"second\"}".to_string()), 2));
        let mut n = 0;
        let r = ask_with_retry(
            &mut |_| {
                n += 1;
                Err("no key".into())
            },
            "p",
        );
        assert_eq!((r, n), (Err("no key".to_string()), 1), "an error is not retried");
        let mut n = 0;
        let r = ask_with_retry(
            &mut |_| {
                n += 1;
                if n == 1 {
                    Ok("  ".into())
                } else {
                    Err("offline".into())
                }
            },
            "p",
        );
        assert_eq!((r, n), (Ok("  ".to_string()), 2), "a failing retry keeps the first answer");
        let mut n = 0;
        let r = ask_with_retry(
            &mut |_| {
                n += 1;
                Ok("{\"text\": 5}".into())
            },
            "p",
        );
        assert_eq!((r, n), (Ok("{\"text\": 5}".to_string()), 2), "twice unusable: the caller reports it");
    }

    // ---- URL scheme

    fn url(w: &mut W, r: Route) -> Out {
        w.msg(NMsg::Url(r))
    }

    #[test]
    fn a_note_url_opens_that_note_in_the_window() {
        let mut w = W::new(vec![note("a", "A", 1), note("b", "B", 2)]);
        w.open("a");
        w.ev(Event::Hidden);
        w.st.borrow_mut().visible = false;
        let out = url(&mut w, Route::Note("b".into()));
        assert_eq!(w.cur(), Some("b"));
        assert!(w.st.borrow().visible);
        assert_eq!(w.st.borrow().markdown, "B");
        assert_eq!(out.toast, None);
    }

    #[test]
    fn a_url_for_an_unknown_note_toasts_and_creates_nothing() {
        let mut w = W::new(vec![note("a", "A", 1)]);
        let out = url(&mut w, Route::Note("nope".into()));
        assert_eq!(out.toast.as_deref(), Some(NOTE_GONE));
        assert_eq!(w.notes.len(), 1);
        assert!(!w.st.borrow().visible && w.cur().is_none());
        assert!(!out.save_notes);
    }

    #[test]
    fn a_url_for_a_deleted_note_says_it_is_in_recently_deleted() {
        let mut w = W::new(vec![note("a", "A", 1), note("b", "B", 2)]);
        w.notes[1].deleted_at = Some(5);
        let out = url(&mut w, Route::Note("b".into()));
        assert_eq!(out.toast.as_deref(), Some(NOTE_IN_TRASH));
        assert_eq!(w.notes[1].deleted_at, Some(5), "not restored");
        assert!(!w.st.borrow().visible && w.cur().is_none());
    }

    #[test]
    fn the_new_note_url_creates_and_opens_a_note_and_the_bare_url_toggles() {
        let mut w = W::new(vec![note("a", "A", 1)]);
        let out = url(&mut w, Route::NewNote);
        assert!(out.save_notes);
        assert_eq!(w.live(), 2);
        assert_ne!(w.cur(), Some("a"));
        assert!(w.st.borrow().visible);
        url(&mut w, Route::Notes);
        assert!(!w.st.borrow().visible, "the bare URL toggles the window off");
        url(&mut w, Route::Notes);
        assert!(w.st.borrow().visible);
    }

    // ---- AI commands

    fn ai_ids(w: &W) -> Vec<String> {
        w.level_ids()
    }

    fn run_ai(w: &mut W, id: &str) -> Out {
        w.ev(Event::Actions);
        w.pick("ai");
        w.pick(&format!("ai:{id}"))
    }

    fn done(w: &mut W, command: &str, reply: Result<&str, &str>) -> Out {
        let req = w.st.borrow().ai_requests.last().cloned().expect("a request was started");
        w.msg(NMsg::AiDone {
            request_id: req.request_id,
            command_id: command.into(),
            original_md: req.original_md,
            selection: req.selection,
            reply: reply.map(String::from).map_err(String::from),
        })
    }

    #[test]
    fn the_ai_entry_is_hidden_when_no_provider_is_usable() {
        let mut w = W::new(vec![note("a", "A", 1)]);
        w.open("a");
        w.ev(Event::Actions);
        assert!(ai_ids(&w).contains(&"ai".to_string()));
        w.ev(Event::Actions);
        w.st.borrow_mut().ai_ok = false;
        w.ev(Event::Actions);
        assert!(!ai_ids(&w).contains(&"ai".to_string()));
        let out = w.pick("ai:fix-grammar");
        assert_eq!(out.toast.as_deref(), Some(AI_NOT_SET_UP));
        assert!(w.st.borrow().ai_requests.is_empty());
    }

    #[test]
    fn an_ai_command_sends_the_selection_as_markdown_and_shows_running() {
        let mut w = W::new(vec![note("a", "teh **cat** sat", 1)]);
        w.open("a");
        w.st.borrow_mut().sel = (4, 11);
        let out = run_ai(&mut w, "fix-grammar");
        assert_eq!(out.toast.as_deref(), Some("Running Fix Spelling and Grammar…"));
        let st = w.st.borrow();
        let req = &st.ai_requests[0];
        assert_eq!(req.selection, Some((4, 11)));
        assert_eq!(req.original_md, "teh **cat** sat");
        assert_eq!(req.system, note_ai::SYSTEM);
        let cmd = note_ai::commands().into_iter().find(|c| c.id == "fix-grammar").unwrap();
        assert_eq!(req.prompt, note_ai::prompt(&cmd, "**cat**"));
    }

    #[test]
    fn without_a_selection_or_for_a_note_scope_command_the_whole_note_is_sent() {
        let mut w = W::new(vec![note("a", "# T\nbody", 1)]);
        w.open("a");
        run_ai(&mut w, "fix-grammar");
        {
            let st = w.st.borrow();
            let cmd = note_ai::commands().into_iter().find(|c| c.id == "fix-grammar").unwrap();
            assert_eq!(st.ai_requests[0].prompt, note_ai::prompt(&cmd, "# T\nbody"));
            assert_eq!(st.ai_requests[0].selection, None);
        }
        done(&mut w, "fix-grammar", Ok("{\"text\": \"x\"}"));
        w.st.borrow_mut().sel = (0, 3);
        run_ai(&mut w, "summarize");
        let st = w.st.borrow();
        let cmd = note_ai::commands().into_iter().find(|c| c.id == "summarize").unwrap();
        assert_eq!(st.ai_requests[1].prompt, note_ai::prompt(&cmd, "# T\nbody"), "a Note command ignores the selection");
        assert_eq!(st.ai_requests[1].selection, None);
    }

    #[test]
    fn empty_and_too_long_notes_are_refused_with_a_toast() {
        let mut w = W::new(vec![note("e", "  \n", 1)]);
        w.open("e");
        let out = run_ai(&mut w, "fix-grammar");
        assert!(out.toast.is_some() && w.st.borrow().ai_requests.is_empty(), "{out:?}");
        let mut w = W::new(vec![note("l", &"x".repeat(note_ai::MAX_CHARS + 1), 1)]);
        w.open("l");
        let out = run_ai(&mut w, "fix-grammar");
        assert!(out.toast.unwrap().contains("too long"));
        assert!(w.st.borrow().ai_requests.is_empty());
        // the selection is what counts when there is one
        w.st.borrow_mut().sel = (0, 10);
        let out = run_ai(&mut w, "fix-grammar");
        assert!(out.toast.unwrap().starts_with("Running"));
        assert_eq!(w.st.borrow().ai_requests.len(), 1);
    }

    #[test]
    fn only_one_ai_request_runs_at_a_time() {
        let mut w = W::new(vec![note("a", "hello", 1)]);
        w.open("a");
        run_ai(&mut w, "shorter");
        let out = run_ai(&mut w, "longer");
        assert_eq!(out.toast.as_deref(), Some(AI_BUSY));
        assert_eq!(w.st.borrow().ai_requests.len(), 1);
        done(&mut w, "shorter", Ok("{\"text\": \"hi\"}"));
        let out = run_ai(&mut w, "longer");
        assert!(out.toast.unwrap().starts_with("Running"), "free again after the answer");
        assert_eq!(w.st.borrow().ai_requests.len(), 2);
    }

    #[test]
    fn a_replace_command_replaces_the_selection_through_one_markdown_edit() {
        let mut w = W::new(vec![note("a", "teh cat sat", 1)]);
        w.open("a");
        w.st.borrow_mut().sel = (0, 7);
        run_ai(&mut w, "fix-grammar");
        w.st.borrow_mut().sel = (9, 9); // the cursor moved meanwhile: the result still lands on its target
        let out = done(&mut w, "fix-grammar", Ok("```json\n{\"text\": \"the **cat**\"}\n```"));
        assert_eq!(w.st.borrow().markdown, "the **cat** sat");
        assert!(w.log().contains(&"select:0-7".to_string()));
        assert!(out.toast.unwrap().contains("Fix Spelling and Grammar"));
        assert!(w.ctl.ai.is_none());
    }

    #[test]
    fn a_replace_command_without_a_selection_replaces_the_whole_note() {
        let mut w = W::new(vec![note("a", "teh cat", 1)]);
        w.open("a");
        w.st.borrow_mut().sel = (3, 3);
        run_ai(&mut w, "improve-writing");
        done(&mut w, "improve-writing", Ok("{\"text\": \"The cat\"}"));
        assert_eq!(w.st.borrow().markdown, "The cat");
    }

    #[test]
    fn inserting_commands_put_the_result_below() {
        let mut w = W::new(vec![note("a", "line one\nline two", 1)]);
        w.open("a");
        run_ai(&mut w, "summarize");
        done(&mut w, "summarize", Ok("{\"text\": \"- short\"}"));
        assert_eq!(w.st.borrow().markdown, "line one\nline two\n\n- short");
        assert!(w.log().iter().any(|l| l == "append:- short"));
        run_ai(&mut w, "continue-writing");
        done(&mut w, "continue-writing", Ok("{\"text\": \"more\"}"));
        assert!(w.st.borrow().markdown.ends_with("\n\nmore"));
        // Explain This with a selection goes after the selected line, without one at the end
        let mut w = W::new(vec![note("a", "line one\nline two", 1)]);
        w.open("a");
        w.st.borrow_mut().sel = (2, 5);
        run_ai(&mut w, "explain");
        done(&mut w, "explain", Ok("{\"text\": \"It means one.\"}"));
        assert_eq!(w.st.borrow().markdown, "line one\n\nIt means one.\nline two");
        let mut w = W::new(vec![note("a", "line one", 1)]);
        w.open("a");
        run_ai(&mut w, "explain");
        done(&mut w, "explain", Ok("{\"text\": \"E\"}"));
        assert_eq!(w.st.borrow().markdown, "line one\n\nE");
    }

    #[test]
    fn the_inserting_command_ids_match_the_core_rules() {
        // the controller picks the editor call by id; `note_ai::apply` is the reference for it
        for id in ["summarize", "continue-writing", "explain"] {
            let c = note_ai::commands().into_iter().find(|c| c.id == id).unwrap();
            let out = note_ai::apply("a\nb", Some((0, 1)), &c, "R");
            assert!(out.starts_with("a\n\nR") || out.ends_with("\n\nR"), "{id}: {out:?}");
        }
        let c = note_ai::commands().into_iter().find(|c| c.id == "shorter").unwrap();
        assert_eq!(note_ai::apply("abc", Some((0, 1)), &c, "R"), "Rbc");
    }

    #[test]
    fn a_reply_after_the_note_changed_applies_nothing() {
        let mut w = W::new(vec![note("a", "hello", 1)]);
        w.open("a");
        run_ai(&mut w, "shorter");
        w.msg(NMsg::Changed("hello there".into()));
        let before = w.st.borrow().markdown.clone();
        let out = done(&mut w, "shorter", Ok("{\"text\": \"hi\"}"));
        assert_eq!(out.toast.as_deref(), Some(AI_STALE));
        assert_eq!(w.st.borrow().markdown, before);
        assert!(!w.log().iter().any(|l| l.starts_with("replace-selection") || l.starts_with("append")));
        // another note is open now: also nothing applied
        let mut w = W::new(vec![note("a", "hello", 1), note("b", "other", 2)]);
        w.open("a");
        run_ai(&mut w, "shorter");
        w.ev(Event::Hidden);
        w.open("b");
        let out = done(&mut w, "shorter", Ok("{\"text\": \"hi\"}"));
        assert_eq!(out.toast.as_deref(), Some(AI_STALE));
        assert_eq!(w.st.borrow().markdown, "other");
        assert_eq!(w.by_id("b").text, "other");
    }

    #[test]
    fn a_reply_for_a_deleted_current_note_applies_nothing() {
        let mut w = W::new(vec![note("a", "hello", 1), note("b", "other", 2)]);
        w.open("a");
        run_ai(&mut w, "shorter");
        w.run(|c, d| c.delete_id(d, "a"));
        let out = done(&mut w, "shorter", Ok("{\"text\": \"hi\"}"));
        assert_eq!(out.toast.as_deref(), Some(AI_STALE));
    }

    #[test]
    fn unusable_and_failed_replies_are_reported_and_free_the_slot() {
        let mut w = W::new(vec![note("a", "hello", 1)]);
        w.open("a");
        run_ai(&mut w, "shorter");
        let out = done(&mut w, "shorter", Ok("{\"text\": 5}"));
        assert_eq!(out.toast.as_deref(), Some(AI_UNUSABLE));
        assert_eq!(w.st.borrow().markdown, "hello");
        run_ai(&mut w, "shorter");
        let out = done(&mut w, "shorter", Err("No Anthropic API key yet."));
        assert!(out.toast.unwrap().contains("No Anthropic API key yet."));
        run_ai(&mut w, "shorter");
        w.st.borrow_mut().apply_ok = false;
        let out = done(&mut w, "shorter", Ok("{\"text\": \"x\"}"));
        assert!(out.toast.unwrap().contains("could not be applied"));
        assert_eq!(w.st.borrow().markdown, "hello");
    }

    #[test]
    fn an_answer_for_another_request_is_ignored() {
        let mut w = W::new(vec![note("a", "hello", 1)]);
        w.open("a");
        run_ai(&mut w, "shorter");
        let out = w.msg(NMsg::AiDone {
            request_id: 999,
            command_id: "shorter".into(),
            original_md: "hello".into(),
            selection: None,
            reply: Ok("{\"text\": \"x\"}".into()),
        });
        assert_eq!(out, Out::default());
        assert!(w.ctl.ai.is_some(), "the real request is still running");
        assert_eq!(w.st.borrow().markdown, "hello");
    }

    // ---- dictation

    fn dictate(w: &mut W) -> Out {
        w.ev(Event::Actions);
        w.pick("dictate")
    }

    fn partial(w: &mut W, t: &str) -> Out {
        w.msg(NMsg::DictationPartial(t.into()))
    }

    fn started(note_text: &str, cursor: usize) -> W {
        let mut w = W::new(vec![note("a", note_text, 1)]);
        w.open("a");
        w.st.borrow_mut().sel = (cursor, cursor);
        dictate(&mut w);
        assert!(w.st.borrow().dict_active);
        w
    }

    #[test]
    fn dictation_starts_when_authorized_and_the_entry_flips() {
        let mut w = W::new(vec![note("a", "A", 1)]);
        w.open("a");
        w.ev(Event::Actions);
        assert!(w.st.borrow().levels[0].1.iter().any(|i| i.id == "dictate" && i.title == "Start Dictation"));
        w.pick("dictate");
        assert!(w.st.borrow().dict_active && w.log().contains(&"dictation:start".to_string()));
        w.ev(Event::Actions);
        assert!(w.st.borrow().levels[0].1.iter().any(|i| i.id == "dictate" && i.title == "Stop Dictation"));
    }

    #[test]
    fn a_second_dictate_toggles_it_off() {
        let mut w = started("A", 1);
        dictate(&mut w);
        assert!(!w.st.borrow().dict_active);
        assert!(w.ctl.dictating.is_none());
        let out = partial(&mut w, "late");
        assert_eq!(out, Out::default());
        assert_eq!(w.st.borrow().markdown, "A", "a late transcript is dropped");
    }

    #[test]
    fn denied_or_restricted_access_toasts_opens_system_settings_and_never_starts() {
        for status in [DictStatus::Denied, DictStatus::Restricted] {
            let mut w = W::new(vec![note("a", "A", 1)]);
            w.open("a");
            w.st.borrow_mut().dict_status = Some(status);
            let out = dictate(&mut w);
            assert_eq!(out.toast.as_deref(), Some(DICTATION_DENIED));
            assert_eq!(w.st.borrow().urls, vec![PRIVACY_PANE.to_string()]);
            assert!(!w.st.borrow().dict_active && w.st.borrow().access_requests == 0);
        }
    }

    #[test]
    fn an_undetermined_status_asks_first_and_starts_only_when_granted() {
        let mut w = W::new(vec![note("a", "A", 1)]);
        w.open("a");
        w.st.borrow_mut().dict_status = Some(DictStatus::NotDetermined);
        let out = dictate(&mut w);
        assert_eq!(out.toast, None);
        assert_eq!(w.st.borrow().access_requests, 1);
        assert!(!w.st.borrow().dict_active);
        w.msg(NMsg::DictationAccess(DictStatus::Authorized));
        assert!(w.st.borrow().dict_active);
        // refused at the prompt
        let mut w = W::new(vec![note("a", "A", 1)]);
        w.open("a");
        w.st.borrow_mut().dict_status = Some(DictStatus::NotDetermined);
        dictate(&mut w);
        let out = w.msg(NMsg::DictationAccess(DictStatus::Denied));
        assert_eq!(out.toast.as_deref(), Some(DICTATION_DENIED));
        assert!(!w.st.borrow().dict_active);
        assert_eq!(w.st.borrow().urls.len(), 1);
        // an answer nobody asked for starts nothing
        let mut w = W::new(vec![note("a", "A", 1)]);
        w.open("a");
        w.msg(NMsg::DictationAccess(DictStatus::Authorized));
        assert!(!w.st.borrow().dict_active);
    }

    #[test]
    fn a_start_failure_is_reported() {
        let mut w = W::new(vec![note("a", "A", 1)]);
        w.open("a");
        w.st.borrow_mut().dict_start_err = Some("no input device".into());
        let out = dictate(&mut w);
        assert!(out.toast.unwrap().contains("no input device"));
        assert!(w.ctl.dictating.is_none());
    }

    #[test]
    fn partials_replace_the_same_span_and_never_duplicate() {
        let mut w = started("Hello ", 6);
        partial(&mut w, "hi");
        assert_eq!(w.st.borrow().markdown, "Hello hi");
        partial(&mut w, "hi there");
        assert_eq!(w.st.borrow().markdown, "Hello hi there");
        partial(&mut w, "hi there");
        assert_eq!(w.st.borrow().markdown, "Hello hi there");
        partial(&mut w, "high there you");
        assert_eq!(w.st.borrow().markdown, "Hello high there you");
        assert_eq!(w.log().iter().filter(|l| l.starts_with("replace-range:")).count(), 3, "an unchanged partial is not re-applied");
    }

    #[test]
    fn dictating_after_a_word_adds_a_space_and_the_note_keeps_its_tail() {
        let mut w = started("Hello world", 5);
        partial(&mut w, "big");
        assert_eq!(w.st.borrow().markdown, "Hello big world");
        partial(&mut w, "big bright");
        assert_eq!(w.st.borrow().markdown, "Hello big bright world");
    }

    #[test]
    fn a_blank_partial_inserts_nothing() {
        let mut w = started("Hi", 2);
        partial(&mut w, "   ");
        assert_eq!(w.st.borrow().markdown, "Hi");
        partial(&mut w, "x");
        partial(&mut w, "");
        assert_eq!(w.st.borrow().markdown, "Hi");
    }

    #[test]
    fn the_final_transcript_replaces_once_more_and_stops() {
        let mut w = started("Hello ", 6);
        partial(&mut w, "hi there");
        w.msg(NMsg::DictationFinal("Hi there.".into()));
        assert_eq!(w.st.borrow().markdown, "Hello Hi there.");
        assert!(!w.st.borrow().dict_active && w.ctl.dictating.is_none());
        let out = partial(&mut w, "again");
        assert_eq!(out, Out::default());
        assert_eq!(w.st.borrow().markdown, "Hello Hi there.");
    }

    #[test]
    fn a_recognizer_error_toasts_and_stops_keeping_what_was_inserted() {
        let mut w = started("A ", 2);
        partial(&mut w, "so far");
        let out = w.msg(NMsg::DictationError("The recognizer stopped".into()));
        assert!(out.toast.unwrap().contains("The recognizer stopped"));
        assert!(!w.st.borrow().dict_active);
        assert_eq!(w.st.borrow().markdown, "A so far");
    }

    #[test]
    fn hiding_the_window_stops_dictation() {
        let mut w = started("A ", 2);
        w.ev(Event::Hidden);
        assert!(!w.st.borrow().dict_active);
        let mut w = started("A ", 2);
        w.toggle();
        assert!(!w.st.borrow().dict_active);
    }

    #[test]
    fn opening_another_note_deleting_the_note_and_quitting_stop_dictation() {
        let mut w = W::new(vec![note("a", "A ", 1), note("b", "B", 2)]);
        w.open("a");
        w.st.borrow_mut().sel = (2, 2);
        dictate(&mut w);
        w.open("b");
        assert!(!w.st.borrow().dict_active, "another note");
        w.open("a");
        w.st.borrow_mut().sel = (2, 2);
        dictate(&mut w);
        w.run(|c, d| c.delete_id(d, "a"));
        assert!(!w.st.borrow().dict_active, "deleted note");
        w.open("b");
        w.st.borrow_mut().sel = (1, 1);
        dictate(&mut w);
        w.run(|c, d| c.flush(d));
        assert!(!w.st.borrow().dict_active, "quit");
        assert!(w.ctl.dictating.is_none());
    }

    #[test]
    fn typing_after_the_transcript_is_fine_but_editing_inside_it_stops_dictation() {
        let mut w = started("Hello ", 6);
        partial(&mut w, "hi");
        // the user types after the transcript: the span is intact
        w.st.borrow_mut().markdown.push_str(" more");
        let out = partial(&mut w, "hi there");
        assert_eq!(out.toast, None);
        assert_eq!(w.st.borrow().markdown, "Hello hi there more");
        // the user edits inside it
        w.st.borrow_mut().markdown = "Hello hX there more".into();
        let out = partial(&mut w, "hi there you");
        assert!(out.toast.unwrap().contains("edited"));
        assert!(!w.st.borrow().dict_active);
        assert_eq!(w.st.borrow().markdown, "Hello hX there more", "nothing was overwritten");
    }

    #[test]
    fn create_dictated_note_opens_a_new_note_and_starts_dictation() {
        let mut w = W::new(vec![note("a", "A", 1)]);
        let out = w.run(|c, d| c.create_dictated_note(d));
        assert!(out.save_notes);
        assert_eq!(w.live(), 2);
        assert!(w.st.borrow().visible && w.st.borrow().dict_active);
        partial(&mut w, "buy milk");
        assert_eq!(w.st.borrow().markdown, "buy milk");
        // denied: the note is still created and the user is told
        let mut w = W::new(vec![note("a", "A", 1)]);
        w.st.borrow_mut().dict_status = Some(DictStatus::Denied);
        let out = w.run(|c, d| c.create_dictated_note(d));
        assert_eq!(out.toast.as_deref(), Some(DICTATION_DENIED));
        assert_eq!(w.live(), 2);
        assert!(!w.st.borrow().dict_active);
    }

    #[test]
    fn startup_purges_old_deleted_notes() {
        let mut w = W::new(vec![note("keep", "k", 1), note("old", "o", 1), note("fresh", "f", 1)]);
        w.notes[1].deleted_at = Some(1000 - (notes::TRASH_DAYS + 1) * 86_400);
        w.notes[2].deleted_at = Some(1000 - 86_400);
        let out = w.run(|c, d| c.startup(d));
        assert!(out.save_notes);
        let ids: Vec<&str> = w.notes.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, ["keep", "fresh"]);
        assert!(!w.run(|c, d| c.startup(d)).save_notes, "nothing left to purge");
    }

    #[test]
    fn new_note_event_creates_and_opens_a_note() {
        let mut w = W::new(vec![note("a", "A", 1)]);
        w.open("a");
        w.ev(Event::Browse);
        let out = w.ev(Event::New);
        assert!(out.save_notes);
        assert_eq!(w.live(), 2);
        assert_ne!(w.cur(), Some("a"));
        assert!(!w.st.borrow().palette_on);
    }

    #[test]
    fn shown_loads_a_note_when_the_window_was_opened_by_someone_else() {
        let mut w = W::new(vec![note("a", "A", 1)]);
        w.ev(Event::Shown);
        assert_eq!(w.cur(), Some("a"));
    }
}
