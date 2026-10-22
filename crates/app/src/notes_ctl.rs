//! Controller of the floating Notes window (T6): which note is open, the history, the palette
//! mode, debounced saving and what every action does.
//!
//! Native callbacks (editor change, window events, palette query / pick / close, zoom) only send
//! a [`NMsg`] on the bus; the work happens in `App::drain` through [`NotesCtl::handle`]. All
//! AppKit and file-system effects go through [`NotesFx`], so tests use a fake and never open a
//! window. The controller never persists anything itself: every call returns an [`Out`] telling
//! the app what to save and which toast to show.
//!
//! Later steps (settings screen, menu-bar toggle, URL scheme, AI commands, dictation) are one
//! more match arm each: see `execute` and `handle`.

use crate::services::{Bus, Msg, Shared};
use ferrocast_core::models::{Note, Quicklink, Settings};
use ferrocast_core::note_actions::{self, BrowseRow, Context as ActCtx, Entry, NoteAction};
use ferrocast_core::notes::{self, History};
use ferrocast_macos::notes::Event;
use ferrocast_macos::notes_format::Format;
use ferrocast_macos::notes_palette::PaletteItem;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Saving to notes.json while typing happens at most once per this many milliseconds.
pub const DEBOUNCE_MS: u64 = 500;
/// Toast of the actions whose feature is a later step (AI commands, dictation).
pub const NOT_WIRED: &str = "Coming in the next step";

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
            section: Some(if r.pinned { "Pinned" } else { "Notes" }.to_string()),
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
            title: e.title.clone(),
            subtitle: e.subtitle.clone(),
            glyph: None,
            accessory: e.shortcut.map(str::to_string),
            section: Some(e.section.to_string()),
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

// ---------------------------------------------------------------- controller

pub struct NotesCtl {
    fx: Box<dyn NotesFx>,
    /// The note shown in the editor (kept while the window is hidden).
    pub current: Option<String>,
    pub history: History,
    pub mode: Mode,
    ready: bool,
    dirty: bool,
    last_save_ms: Option<u64>,
    tick_pending: bool,
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
            tick_pending: false,
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

    /// Writes pending text now (app quit).
    pub fn flush(&mut self, d: &mut Data) -> Out {
        let mut out = Out::default();
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
        self.fx.palette_show("Browse notes…", browse_items(&rows, true));
        self.fx.palette_empty_text("No notes found");
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
            // AI commands and dictation are a later step: listed, but they only show a toast
            dictating: false,
            ai_available: true,
            quicklinks_available: true,
            format_bar: d.settings.notes_format_bar,
        }
    }

    fn open_actions(&mut self, d: &Data) {
        let items = action_items(&note_actions::top_level(&self.act_ctx(d)));
        self.close_palette();
        self.fx.palette_show("Search actions…", items);
        self.fx.palette_empty_text("No actions found");
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
                    self.fx.palette_push("Recently deleted…", items);
                    self.fx.palette_empty_text("Nothing was deleted");
                    self.mode = Mode::RecentlyDeleted;
                    return;
                }
                let kids = note_actions::children(action, &ctx);
                if !kids.is_empty() {
                    self.fx.palette_push("Search…", action_items(&kids));
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
                    out.toast = Some(format!("Copied as {}", f.title()));
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
            // not wired yet (later steps): visible but harmless
            Dictate | Ai(_) => out.toast = Some(NOT_WIRED.into()),
            // menus are opened by `on_pick`; reaching them here means there is nothing to do
            FormatMenu | CopyNoteAs | ExportNote | AiCommands | ChangeTone | RecentlyDeleted => {}
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
                format!("Exported {name} to Downloads")
            }
            Err(e) => format!("Export failed: {e}"),
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
}

/// Route every native callback to the bus (the shelf pattern): the callbacks only send.
pub fn install_callbacks(bus: &Bus) {
    use ferrocast_macos::{notes, notes_palette};
    let b = bus.clone();
    notes::set_on_change(Box::new(move |t| b.send(Msg::Notes(NMsg::Changed(t)))));
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
    }

    /// A window that is only a record of what was asked of it.
    pub struct FakeFx(pub Rc<RefCell<St>>);

    impl FakeFx {
        pub fn new() -> (FakeFx, Rc<RefCell<St>>) {
            let st = Rc::new(RefCell::new(St { zoom: 1.0, downloads: Some(PathBuf::from("/dl")), ..Default::default() }));
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
        fn set_markdown(&mut self, md: &str) {
            self.0.borrow_mut().markdown = md.into();
            self.log(format!("markdown:{md}"));
        }
        fn apply_format(&mut self, f: Format) {
            self.log(format!("format:{f:?}"));
        }
        fn has_selection(&self) -> bool {
            self.0.borrow().selection
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
        w.settings.notes_format_bar = true;
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
    fn format_bar_action_flips_the_setting_and_the_window() {
        let mut w = W::new(vec![note("a", "A", 1)]);
        w.open("a");
        w.pick_action("format-bar");
        assert!(w.settings.notes_format_bar);
        assert!(w.log().contains(&"bar:true".to_string()));
        w.ev(Event::Actions);
        assert!(w.st.borrow().levels[0].1.iter().any(|i| i.title == "Hide Format Bar"));
        let out = w.pick("format-bar");
        assert!(out.save_settings && !w.settings.notes_format_bar);
        assert!(w.log().contains(&"bar:false".to_string()));
    }

    #[test]
    fn ai_and_dictation_are_visible_but_harmless() {
        let mut w = W::new(vec![note("a", "A", 1)]);
        w.open("a");
        w.ev(Event::Actions);
        let out = w.pick("dictate");
        assert_eq!(out.toast.as_deref(), Some(NOT_WIRED));
        w.pick_action("ai");
        assert_eq!(w.ctl.mode, Mode::Submenu(NoteAction::AiCommands));
        let leaf = w.level_ids().into_iter().find(|i| i.starts_with("ai:fix")).unwrap();
        let out = w.pick(&leaf);
        assert_eq!(out, Out { toast: Some(NOT_WIRED.into()), ..Default::default() });
        assert_eq!(w.by_id("a").text, "A");
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
