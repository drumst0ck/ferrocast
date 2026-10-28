//! iCloud sync wiring (opt-in): which domains changed, the background run, and applying a merge.
//!
//! Threading: only the main thread touches app state. A run is one worker thread that does all the
//! reads and writes in the sync folder on a snapshot taken on the main thread and sends the result
//! back as `Msg::SyncDone`. At most one run is in flight; saves that happen meanwhile are queued in
//! `pending` and start the next run when the result arrives. The result is applied only when the
//! local data still equals the snapshot (otherwise the tracker keeps only what was published and the
//! next run merges again), so a merge never overwrites an edit made while the run was away.

use super::*;
use ferrocast_core::sync::Tracker;
use ferrocast_core::sync_io::{self, Outcome, SyncIo};
use serde_json::Value;
use std::collections::BTreeSet;
use std::sync::atomic::AtomicBool;

type Items = Vec<(String, Value)>;

/// How often the timer pulls and merges.
pub const SYNC_EVERY: Duration = Duration::from_secs(30);

#[derive(Default)]
pub struct SyncCtl {
    pub(crate) in_flight: bool,
    pub(crate) pending: BTreeSet<&'static str>,
    /// Unix seconds of the last finished run.
    last_sync: Option<i64>,
    peers: usize,
    error: Option<String>,
    /// Set while a merge is applied so its saves are not queued as local edits.
    applying: bool,
    last_start: Option<Instant>,
    /// Shared with the worker: cleared when sync is turned off so a run in progress stops writing.
    pub(crate) flag: Arc<AtomicBool>,
    /// Files of other Macs still being downloaded by iCloud Drive in the last run.
    pub(crate) downloading: usize,
    /// When a notes merge that waited for typing to stop may run again.
    notes_retry: Option<Instant>,
}

/// Result of one background run.
pub struct SyncDone {
    pub(crate) results: Vec<(&'static str, Items, Result<Outcome, String>)>,
    pub(crate) unavailable: Option<String>,
    /// The worker's handle on the sync folder, to write the own document once a merge is applied.
    pub(crate) io: Option<SyncIo>,
}

impl App {
    pub(crate) fn sync_on(&self) -> bool {
        self.settings.sync_enabled
    }

    /// Text of the status line in Settings.
    pub(crate) fn sync_status(&self) -> String {
        if !self.sync_on() {
            return "Off. Ferrocast does not read or write anything in iCloud Drive.".into();
        }
        if let Some(e) = &self.sync.error {
            return e.clone();
        }
        match self.sync.last_sync {
            Some(t) => {
                let n = self.sync.peers;
                let macs = match n {
                    0 => "no other Mac seen yet".to_string(),
                    1 => "1 other Mac seen".to_string(),
                    n => format!("{n} other Macs seen"),
                };
                let wait = match self.sync.downloading {
                    0 => String::new(),
                    n => format!(" · {n} file{} still downloading from iCloud", if n == 1 { "" } else { "s" }),
                };
                format!("Last sync {} · {macs}{wait}", services::ago((services::now_secs() - t).max(0)))
            }
            None if self.sync.downloading > 0 => format!("Waiting for iCloud to download {} file(s)…", self.sync.downloading),
            None => "Waiting for the first sync…".into(),
        }
    }

    /// A data file was saved locally: queue its domain.
    pub(crate) fn sync_mark(&mut self, file: &str) {
        if !self.sync_on() || self.sync.applying {
            return;
        }
        if let Some(d) = sync_io::domain_of_file(file) {
            self.sync.pending.insert(d);
        }
    }

    /// Start a run for everything queued (and the clipboard when the recorder changed it).
    pub(crate) fn sync_flush(&mut self) {
        if self.sync.notes_retry.is_some_and(|t| t <= Instant::now()) {
            self.sync.notes_retry = None;
            self.sync.pending.insert("notes");
        }
        let dirty = self.shared.clip_dirty.swap(false, Ordering::Relaxed);
        if !self.sync_on() {
            self.sync.pending.clear();
            return;
        }
        if dirty {
            self.sync.pending.insert("clipboard");
        }
        if self.sync.pending.is_empty() || self.sync.in_flight {
            return;
        }
        let domains: Vec<&'static str> = std::mem::take(&mut self.sync.pending).into_iter().collect();
        self.sync_start(domains);
    }

    /// Before the first publish: every domain file exists with the ids the app holds (a fresh install
    /// only has defaults in memory), so what is published is what is on disk on the next launch.
    pub(crate) fn sync_persist_all(&mut self) {
        let store = self.store.clone();
        let missing = |name: &str| !store.path(name).exists();
        if missing("snippets.json") {
            let _ = store.save("snippets.json", &self.snippets);
        }
        if missing("quicklinks.json") {
            let _ = store.save("quicklinks.json", &self.quicklinks);
        }
        if missing("notes.json") {
            let _ = store.save("notes.json", &self.notes);
        }
        if missing("chats.json") {
            let _ = store.save("chats.json", &self.chats);
        }
        if missing("clipboard.json") {
            let _ = store.save("clipboard.json", &*self.shared.clipboard.lock().unwrap());
        }
        if missing("themes.json") {
            let _ = store.save("themes.json", &ferrocast_core::themes::ThemeFile { themes: self.custom_themes.clone() });
        }
        if missing("settings.json") {
            let _ = store.save("settings.json", &self.settings);
        }
    }

    /// Pull and merge every domain (timer, opening the launcher, "Sync now", turning sync on).
    pub(crate) fn sync_all(&mut self) {
        if !self.sync_on() {
            return;
        }
        self.sync.pending.extend(sync_io::DOMAINS);
        self.sync_flush();
    }

    /// When the launcher opens: pull unless a run just started.
    pub(crate) fn sync_on_show(&mut self) {
        if self.sync_on() && self.sync.last_start.is_none_or(|t| t.elapsed() > Duration::from_secs(5)) {
            self.sync_all();
        }
    }

    pub(crate) fn sync_snapshot(&self, domain: &str) -> Items {
        match domain {
            "settings" => sync_io::settings_items(&self.settings),
            "snippets" => sync_io::to_items(&self.snippets),
            "quicklinks" => sync_io::to_items(&self.quicklinks),
            "notes" => sync_io::to_items(&self.notes),
            "clipboard" => sync_io::to_items(&self.shared.clipboard.lock().unwrap()),
            "chats" => sync_io::to_items(&self.chats),
            "themes" => sync_io::to_items(&self.custom_themes),
            _ => vec![],
        }
    }

    fn sync_start(&mut self, domains: Vec<&'static str>) {
        self.sync_persist_all();
        sync_io::backup_settings_once(&self.store.root);
        self.sync.in_flight = true;
        self.sync.flag.store(true, Ordering::SeqCst);
        let flag = self.sync.flag.clone();
        self.sync.last_start = Some(Instant::now());
        let jobs: Vec<(&'static str, Items)> = domains.into_iter().map(|d| (d, self.sync_snapshot(d))).collect();
        let (root, bus) = (self.store.root.clone(), self.bus.clone());
        std::thread::spawn(move || {
            let done = match sync_io::resolve_dir() {
                Err(e) => SyncDone { results: vec![], unavailable: Some(e.message()), io: None },
                Ok(dir) => {
                    let mut io = SyncIo::new(dir, sync_io::device_id(&root), Store::new(&root).images_dir());
                    io.enabled = flag;
                    let now = now_ms();
                    let results = jobs
                        .into_iter()
                        .map(|(d, snap)| {
                            let tracker: Tracker = sync_io::load_tracker(&root, d);
                            let out = sync_io::sync_domain(&io, d, &snap, &tracker, now).map_err(|e| format!("{e:#}"));
                            (d, snap, out)
                        })
                        .collect();
                    SyncDone { results, unavailable: None, io: Some(io) }
                }
            };
            bus.send(Msg::SyncDone(Box::new(done)));
        });
    }

    /// Main thread: a run finished. Apply what is still safe to apply and persist the trackers.
    pub(crate) fn sync_done(&mut self, done: SyncDone) {
        self.sync.in_flight = false;
        if !self.sync_on() {
            return; // switched off while the run was away: apply nothing
        }
        if let Some(e) = done.unavailable {
            self.sync.error = Some(e);
            return;
        }
        self.sync.error = None;
        let root = self.store.root.clone();
        let mut downloading = 0;
        for (domain, snapshot, result) in done.results {
            let out = match result {
                Ok(o) => o,
                Err(e) => {
                    self.sync.error = Some(format!("Sync failed: {e}"));
                    instance::log(&root, &format!("sync {domain}: {e}"));
                    continue;
                }
            };
            for bad in &out.corrupt {
                instance::log(&root, &format!("sync: ignored unreadable document {bad}"));
            }
            self.sync.peers = out.peers;
            downloading += out.downloading;
            let applied = match out.merged {
                Some(merged) => self.sync_apply(domain, merged, out.tracker_merged.clone(), &snapshot),
                None => None,
            };
            // The own document holds the merged state only once it is what this Mac has (a tracker
            // rebuilt from it must not take an older local value for a new edit).
            if applied.is_some() {
                if let (Some(doc), Some(io)) = (&out.applied_doc, &done.io) {
                    if let Err(e) = io.write_doc(domain, doc) {
                        instance::log(&root, &format!("sync: writing own document {domain}: {e:#}"));
                    }
                }
            }
            let tracker = applied.unwrap_or(out.tracker_published);
            if let Err(e) = sync_io::save_tracker(&root, domain, &tracker) {
                instance::log(&root, &format!("sync: saving tracker {domain}: {e:#}"));
            }
        }
        self.sync.downloading = downloading;
        self.sync.last_sync = Some(services::now_secs());
        self.sync_flush();
    }

    /// Local data still equals what the run saw (and no Notes text is waiting to be saved).
    fn sync_can_apply(&self, domain: &str, snapshot: &Items) -> bool {
        (domain != "notes" || !self.notes_ctl.typing_recently(now_ms() as u64)) && self.sync_snapshot(domain) == *snapshot
    }

    /// Replace local data with the merge and save it. Returns the tracker to persist, with hashes
    /// taken from the re-exported items so a value that does not round-trip byte for byte is not
    /// mistaken for a local edit (and ping-ponged between Macs).
    pub(crate) fn sync_apply(&mut self, domain: &'static str, merged: Items, mut tracker: Tracker, snapshot: &Items) -> Option<Tracker> {
        // Local data must still be what the run saw. The clipboard (written by the recorder thread
        // too) is compared and replaced under one lock below.
        if domain != "clipboard" && !self.sync_can_apply(domain, snapshot) {
            if domain == "notes" && self.notes_ctl.typing_recently(now_ms() as u64) {
                // try again once the typing has stopped
                self.sync.notes_retry = Some(Instant::now() + Duration::from_millis(crate::notes_ctl::TYPING_IDLE_MS + 200));
                self.ctx.request_repaint_after(Duration::from_millis(crate::notes_ctl::TYPING_IDLE_MS + 300));
            }
            return None;
        }
        self.sync.applying = true;
        let mut again = false;
        match domain {
            "settings" => {
                // safety net: the settings as they were before sync changed them (kept once)
                sync_io::backup_settings_once(&self.store.root);
                let Some(s) = sync_io::settings_from_items(&merged, &self.settings) else {
                    self.sync.applying = false;
                    return None;
                };
                self.commit_settings(s);
            }
            "snippets" => {
                self.snippets = sync_io::from_items(merged);
                self.save_snippets();
            }
            "quicklinks" => {
                self.quicklinks = sync_io::from_items(merged);
                self.save_quicklinks();
            }
            "notes" => {
                // Typing in the open window waits (see `sync_can_apply`); with the window closed or
                // idle a deletion made elsewhere is applied like any other change, and the editor
                // moves off a note that is gone.
                let open_before =
                    self.notes_ctl.current.clone().and_then(|id| self.notes.iter().find(|n| n.id == id).map(|n| n.text.clone()));
                self.notes = sync_io::from_items(merged);
                self.save_notes();
                let previous = open_before.unwrap_or_default();
                self.notes_do(|c, d| c.after_merge(d, &previous));
            }
            "themes" => {
                // the selection is stored by name: when the selected theme was renamed elsewhere,
                // find it again by id and follow the new name (a missing theme draws the default look)
                let active_id = self.active_theme().map(|t| t.id.clone());
                self.custom_themes = sync_io::from_items(merged);
                self.save_themes();
                if let (None, Some(id)) = (self.active_theme(), active_id) {
                    if let Some(name) = self.custom_themes.iter().find(|t| t.id == id).map(|t| t.name.clone()) {
                        self.select_theme(Some(name));
                    }
                }
                self.set_theme();
            }
            "chats" => {
                let mut chats: Vec<Chat> = sync_io::from_items(merged);
                chats.sort_by_key(|c| std::cmp::Reverse(c.updated));
                chats.truncate(ferrocast_core::models::MAX_CHATS);
                self.chats = chats;
                let v = self.chats.clone();
                self.persist("chats.json", &v);
            }
            "clipboard" => {
                let incoming: Vec<ClipEntry> = sync_io::from_items(merged);
                let before = incoming.len();
                let list = sync_io::clipboard_normalize(incoming, services::now_secs(), self.settings.clipboard_retention.secs());
                again = list.len() != before;
                let mut cur = self.shared.clipboard.lock().unwrap();
                if sync_io::to_items(&cur) != *snapshot {
                    // something was recorded while the run was away: merge again next time
                    drop(cur);
                    self.sync.applying = false;
                    return None;
                }
                *cur = list;
                let _ = self.store.save("clipboard.json", &*cur);
                ferrocast_core::storage::prune_images(&self.store.images_dir(), &cur);
            }
            _ => {}
        }
        self.sync.applying = false;
        self.shared.clip_dirty.store(false, Ordering::Relaxed);
        if again {
            self.sync.pending.insert(domain); // entries dropped by retention/dedupe become deletions
        }
        // the applied form of each item is what the next run will see
        for (id, v) in self.sync_snapshot(domain) {
            if let Some(t) = tracker.items.get_mut(&id) {
                if !t.deleted {
                    t.hash = ferrocast_core::sync::hash(&v);
                }
            }
        }
        self.sec_dirty = true;
        Some(tracker)
    }

    /// Ask AI: a finished answer becomes (part of) a persisted chat that can sync.
    pub(crate) fn save_ai_chat(&mut self) {
        let Some(View::Ai { asked, answer, history, .. }) = self.stack.iter().rev().find(|v| matches!(v, View::Ai { .. })) else { return };
        if asked.is_empty() || answer.is_empty() || answer.contains("⚠ ") {
            return;
        }
        let mut turns = history.clone();
        turns.push((asked.clone(), answer.clone()));
        let id = self.ai_chat.get_or_insert_with(|| uuid::Uuid::new_v4().to_string()).clone();
        ferrocast_core::models::upsert_chat(&mut self.chats, Chat::new(id, turns, services::now_secs()));
        let v = self.chats.clone();
        self.persist("chats.json", &v);
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}
