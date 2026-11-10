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

/// Converts a copy of a domain's data taken on the main thread into sync items (worker thread).
type Capture = Box<dyn FnOnce() -> Items + Send>;

/// A run that has not answered after this long is given up: sync is not stuck behind a file iCloud
/// Drive never delivers.
pub const RUN_TIMEOUT: Duration = Duration::from_secs(120);

/// How often the timer pulls and merges.
pub const SYNC_EVERY: Duration = Duration::from_secs(30);

#[derive(Default)]
pub struct SyncCtl {
    pub(crate) in_flight: bool,
    pub(crate) pending: BTreeSet<&'static str>,
    /// Unix seconds of the last finished run.
    pub(crate) last_sync: Option<i64>,
    pub(crate) peers: usize,
    /// The sync folder cannot be used (iCloud Drive off, folder not creatable).
    pub(crate) error: Option<String>,
    /// Domains whose last run failed, with the reason; a domain's entry goes when it syncs again.
    pub(crate) errors: std::collections::BTreeMap<&'static str, String>,
    /// Bumped by every save of a domain's data file (the clipboard has `Shared::clip_rev`): a run
    /// applies its merge only when the revision it started from is still the current one.
    rev: std::collections::BTreeMap<&'static str, u64>,
    /// Number of the run in flight; a result of an earlier, abandoned run is ignored.
    pub(crate) run: u64,
    /// The own documents handed to the run in flight, put back when it never answers.
    pub(crate) sent_docs: std::collections::BTreeMap<&'static str, ferrocast_core::sync::DeviceDoc>,
    /// Chats a cap removed from this Mac only (not deleted by the user): never published as deleted.
    pub(crate) capped_chats: BTreeSet<String>,
    /// Replaces `sync_io::resolve_dir` (tests).
    pub(crate) resolve: Option<fn() -> Result<std::path::PathBuf, sync_io::Unavailable>>,
    /// Set while a merge is applied so its saves are not queued as local edits.
    applying: bool,
    pub(crate) last_start: Option<Instant>,
    /// Shared with the worker: cleared when sync is turned off so a run in progress stops writing.
    pub(crate) flag: Arc<AtomicBool>,
    /// Files of other Macs still being downloaded by iCloud Drive in the last run.
    pub(crate) downloading: usize,
    /// Merged own documents of applied merges, written by the next background run (iCloud Drive is
    /// never touched on the main thread).
    pub(crate) pending_docs: std::collections::BTreeMap<&'static str, ferrocast_core::sync::DeviceDoc>,
    /// Domains whose last run started over because their data file was unreadable (cleared by the
    /// next run of the domain that is not).
    pub(crate) restored: BTreeSet<&'static str>,
    /// When a notes merge that waited for typing to stop may run again.
    notes_retry: Option<Instant>,
    /// Last time unlinked note images were looked for.
    last_gc: Option<Instant>,
}

/// Result of one background run.
#[derive(Default)]
pub struct SyncDone {
    pub(crate) results: Vec<(&'static str, Items, Result<Outcome, String>)>,
    pub(crate) unavailable: Option<String>,
    /// Revision of each domain's data when the run took its copy (see `SyncCtl::rev`).
    pub(crate) revs: std::collections::BTreeMap<&'static str, u64>,
    /// Which run this answers.
    pub(crate) run: u64,
    /// Own documents the run could not write: kept for the next run.
    pub(crate) docs: std::collections::BTreeMap<&'static str, ferrocast_core::sync::DeviceDoc>,
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
        if !self.sync.errors.is_empty() {
            let shown: Vec<String> = if self.sync.errors.len() == 1 {
                self.sync.errors.values().cloned().collect()
            } else {
                self.sync.errors.iter().map(|(d, e)| format!("{d}: {e}")).collect()
            };
            return trf("Sync failed: {e}", &[("e", &shown.join("; "))]);
        }
        match self.sync.last_sync {
            Some(t) => {
                let n = self.sync.peers;
                let macs = match n {
                    0 => tr("no other Mac seen yet").into_owned(),
                    n => trn("{n} other Mac seen", "{n} other Macs seen", n as u64),
                };
                let wait = match self.sync.downloading {
                    0 => String::new(),
                    n => {
                        format!(" · {}", trn("{n} file still downloading from iCloud", "{n} files still downloading from iCloud", n as u64))
                    }
                };
                let note = if self.sync.restored.is_empty() {
                    String::new()
                } else {
                    let names: Vec<&str> = self.sync.restored.iter().copied().collect();
                    format!(" · {}", trf("restored {names} from your other Macs after an unreadable file", &[("names", &names.join(", "))]))
                };
                trf(
                    "Last sync {ago} · {macs}{wait}{note}",
                    &[("ago", &services::ago((services::now_secs() - t).max(0))), ("macs", &macs), ("wait", &wait), ("note", &note)],
                )
            }
            None if self.sync.downloading > 0 => {
                trf("Waiting for iCloud to download {n} file(s)…", &[("n", &self.sync.downloading.to_string())])
            }
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
            *self.sync.rev.entry(d).or_default() += 1;
        }
    }

    /// Start a run for everything queued (and the clipboard when the recorder changed it).
    pub(crate) fn sync_flush(&mut self) {
        if self.sync.in_flight && self.sync.last_start.is_some_and(|t| t.elapsed() > RUN_TIMEOUT) {
            self.sync_abandon_run();
        }
        if self.sync.notes_retry.is_some_and(|t| t <= Instant::now()) {
            self.sync.notes_retry = None;
            self.sync.pending.insert("notes");
        }
        let dirty = self.shared.clip_dirty.swap(false, Ordering::Relaxed);
        if !self.sync_on() {
            self.sync.pending.clear();
            self.sync.pending_docs.clear();
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

    /// The run in flight did not answer in time (a file stuck in iCloud Drive): stop waiting for it.
    /// It is told to stop writing, its own documents go back to the queue and its late answer, if
    /// it ever comes, is ignored.
    fn sync_abandon_run(&mut self) {
        self.sync.flag.store(false, Ordering::SeqCst);
        self.sync.flag = Arc::new(AtomicBool::new(true));
        self.sync.run += 1;
        self.sync.in_flight = false;
        for (d, doc) in std::mem::take(&mut self.sync.sent_docs) {
            self.sync.pending_docs.entry(d).or_insert(doc);
        }
        self.sync.error = Some(tr("Sync timed out: iCloud Drive did not answer. It tries again.").into_owned());
        instance::log(&self.store.root, "sync: a run did not finish in time and was given up");
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
            self.shared.write_clipboard();
        }
        if missing("themes.json") {
            let _ = store.save("themes.json", &ferrocast_core::themes::ThemeFile { themes: self.custom_themes.clone() });
        }
        if missing("settings.json") {
            let _ = store.save("settings.json", &self.settings);
        }
    }

    /// The open note was saved: images it links are linked, so their unlink countdown (see
    /// `attachments::cleanup_dir`) starts over the next time they are unlinked. Cheap: it returns
    /// at once unless a countdown is recorded.
    pub(crate) fn forget_unlinked_images(&self) {
        let Some(text) = self.notes_ctl.current.as_ref().and_then(|id| self.notes.iter().find(|n| &n.id == id)).map(|n| n.text.as_str())
        else {
            return;
        };
        let linked = ferrocast_core::attachments::referenced([text]);
        if !linked.is_empty() {
            ferrocast_core::attachments::forget_unlinked(&self.store.root, &linked);
        }
    }

    /// Remove note images nothing links any more (older than a week), at most once an hour and off
    /// the main thread. With sync on, every Mac's published notes count too (`gc_attachments`).
    pub(crate) fn attachments_gc(&mut self) {
        if self.sync.last_gc.is_some_and(|t| t.elapsed() < Duration::from_secs(3600)) {
            return;
        }
        self.sync.last_gc = Some(Instant::now());
        // A pending recovery marker (a data file was set aside) means the notes in memory may be
        // incomplete. Only turning sync on again clears it, so with sync off nothing is cleaned up
        // until then: images are kept, never lost.
        if sync_io::needs_recovery(&self.store.root, "notes") {
            return;
        }
        let (notes, root, sync_on) = (self.notes.clone(), self.store.root.clone(), self.sync_on());
        std::thread::spawn(move || {
            let dir = if sync_on {
                match sync_io::resolve_dir() {
                    Ok(d) => Some(d),
                    Err(_) => return,
                }
            } else {
                None
            };
            sync_io::gc_attachments(&root, &notes, dir.as_deref(), services::now_secs());
        });
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
        self.attachments_gc();
        if self.sync_on() && self.sync.last_start.is_none_or(|t| t.elapsed() > Duration::from_secs(5)) {
            self.sync_all();
        }
    }

    pub(crate) fn sync_snapshot(&self, domain: &str) -> Items {
        self.sync_capture(domain).1()
    }

    /// Revision of a domain's data (see `SyncCtl::rev`).
    pub(crate) fn sync_rev(&self, domain: &str) -> u64 {
        if domain == "clipboard" {
            self.shared.clip_rev.load(Ordering::SeqCst)
        } else {
            self.sync.rev.get(domain).copied().unwrap_or(0)
        }
    }

    /// The revision of a domain together with a copy of its data; turning the copy into items (the
    /// slow part: every entry is serialised) is left to whoever calls the closure, so the main thread
    /// only clones. The clipboard list lock is held for the clone and nothing else.
    fn sync_capture(&self, domain: &str) -> (u64, Capture) {
        let rev = self.sync_rev(domain);
        let cap: Capture = match domain {
            "settings" => {
                let v = self.settings.clone();
                Box::new(move || sync_io::settings_items(&v))
            }
            "snippets" => {
                let v = self.snippets.clone();
                Box::new(move || sync_io::to_items(&v))
            }
            "quicklinks" => {
                let v = self.quicklinks.clone();
                Box::new(move || sync_io::to_items(&v))
            }
            "notes" => {
                let v = self.notes.clone();
                Box::new(move || sync_io::to_items(&v))
            }
            "clipboard" => {
                // the revision is read under the lock, so it names exactly the list that is cloned
                let guard = self.shared.clipboard.lock().unwrap();
                let rev = self.shared.clip_rev.load(Ordering::SeqCst);
                let v = guard.clone();
                drop(guard);
                return (rev, Box::new(move || sync_io::to_items(&v)));
            }
            "chats" => {
                let v = self.chats.clone();
                Box::new(move || sync_io::to_items(&v))
            }
            "themes" => {
                let v = self.custom_themes.clone();
                Box::new(move || sync_io::to_items(&v))
            }
            _ => Box::new(Vec::new),
        };
        (rev, cap)
    }

    fn sync_start(&mut self, domains: Vec<&'static str>) {
        self.sync_persist_all();
        sync_io::backup_settings_once(&self.store.root);
        self.sync.in_flight = true;
        self.sync.flag.store(true, Ordering::SeqCst);
        let flag = self.sync.flag.clone();
        self.sync.last_start = Some(Instant::now());
        // `sync_done` applies the results in this order: "settings" must come before "themes"
        // (`adopt_setting` patches the settings tracker the settings result has already saved)
        debug_assert!(domains.windows(2).all(|w| w[0] < w[1]), "domains run in alphabetical order");
        let docs = std::mem::take(&mut self.sync.pending_docs);
        self.sync.sent_docs = docs.clone();
        self.sync.run += 1;
        let run = self.sync.run;
        let mut revs = std::collections::BTreeMap::new();
        let jobs: Vec<(&'static str, Capture)> = domains
            .into_iter()
            .map(|d| {
                let (rev, cap) = self.sync_capture(d);
                revs.insert(d, rev);
                (d, cap)
            })
            .collect();
        let (root, bus) = (self.store.root.clone(), self.bus.clone());
        let resolve = self.sync.resolve.unwrap_or(sync_io::resolve_dir);
        std::thread::spawn(move || {
            let done = match resolve() {
                // the own documents wait for a folder that works
                Err(e) => SyncDone { unavailable: Some(e.message()), revs, run, docs, ..Default::default() },
                Ok(dir) => {
                    let mut io = SyncIo::new(dir, sync_io::device_id(&root), Store::new(&root).images_dir())
                        .with_attachments(root.join(ferrocast_core::attachments::DIR))
                        .with_name(sync_io::computer_name());
                    io.enabled = flag;
                    let failed = flush_docs(&io, docs, &root);
                    let now = now_ms();
                    let results = jobs
                        .into_iter()
                        .map(|(d, cap)| {
                            let snap = cap();
                            let tracker: Tracker = sync_io::load_tracker(&root, d);
                            // an unreadable data file was set aside since the last sync: start over from
                            // what is loaded now instead of publishing every missing item as deleted
                            let recover = sync_io::needs_recovery(&root, d);
                            let out = sync_io::sync_domain_recovering(&io, d, &snap, &tracker, now, recover).map_err(|e| format!("{e:#}"));
                            (d, snap, out)
                        })
                        .collect();
                    SyncDone { results, unavailable: None, revs, run, docs: failed }
                }
            };
            bus.send(Msg::SyncDone(Box::new(done)));
        });
    }

    /// Main thread: a run finished. Apply what is still safe to apply and persist the trackers.
    pub(crate) fn sync_done(&mut self, done: SyncDone) {
        if done.run != self.sync.run {
            return; // the answer of a run that was given up
        }
        self.sync.in_flight = false;
        self.sync.sent_docs.clear();
        if !self.sync_on() {
            self.sync.pending_docs.clear();
            return; // switched off while the run was away: apply nothing
        }
        // own documents that could not be written (no folder, a write error) are tried again; a newer
        // one queued meanwhile (a merge applied since) stays
        for (d, doc) in done.docs {
            self.sync.pending_docs.entry(d).or_insert(doc);
        }
        if let Some(e) = done.unavailable {
            self.sync.error = Some(e);
            return;
        }
        self.sync.error = None;
        self.attachments_gc();
        let root = self.store.root.clone();
        let mut downloading = 0;
        let mut conflicts = 0;
        let mut succeeded = false;
        for (domain, _, result) in done.results {
            let out = match result {
                Ok(o) => o,
                Err(e) => {
                    self.sync.errors.insert(domain, e.clone());
                    instance::log(&root, &format!("sync {domain}: {e}"));
                    continue;
                }
            };
            succeeded = true;
            self.sync.errors.remove(domain);
            for bad in &out.corrupt {
                instance::log(&root, &format!("sync: ignored unreadable document {bad}"));
            }
            self.sync.peers = out.peers;
            downloading += out.downloading;
            if domain == "notes" && out.attachments_added > 0 {
                // images that were missing (placeholders in the editor) are here now
                self.notes_do(|c, d| c.refresh_images(d));
            }
            let had_merge = out.merged.is_some();
            let had_marker = sync_io::needs_recovery(&root, domain);
            let rev = done.revs.get(domain).copied().unwrap_or(0);
            let applied = match out.merged {
                Some(merged) => self.sync_apply(domain, merged, out.tracker_merged.clone(), rev),
                None => None,
            };
            if applied.is_some() {
                conflicts += out.conflicts;
            }
            // The own document holds the merged state only once it is what this Mac has (a tracker
            // rebuilt from it must not take an older local value for a new edit). The next
            // background run writes it: reading and writing iCloud Drive can block.
            if applied.is_some() {
                if let Some(doc) = out.applied_doc.clone() {
                    self.sync.pending_docs.insert(domain, doc);
                }
            }
            let was_applied = applied.is_some();
            if out.recovered {
                instance::log(
                    &root,
                    &format!("sync {domain}: the data file was unreadable (kept as .corrupt); started over from this Mac's own iCloud copy and the other Macs"),
                );
                // named only once the recovered state is in place (a merge not applied retries)
                if was_applied || !had_merge {
                    self.sync.restored.insert(domain);
                }
            } else {
                self.sync.restored.remove(domain);
            }
            let mut tracker = applied.unwrap_or(out.tracker_published);
            if domain == "chats" {
                // chats a cap removed here only: this Mac keeps their records instead of deleting them
                let capped: Vec<String> = std::mem::take(&mut self.sync.capped_chats).into_iter().collect();
                ferrocast_core::sync::mark_unparsed(&mut tracker, &capped);
            }
            match sync_io::save_tracker(&root, domain, &tracker) {
                Err(e) => instance::log(&root, &format!("sync: saving tracker {domain}: {e:#}")),
                // The merge is on disk and so is the tracker that knows it: the marker `sync_apply`
                // set (or the recovery marker) has done its job. A merge that could not be applied
                // keeps a recovery marker, so the next run recovers again; so does a failed tracker
                // write, because data and tracker then disagree.
                Ok(()) if was_applied && (!had_marker || out.recovered) => sync_io::clear_recovery(&root, domain),
                Ok(()) if out.recovered && !had_merge => sync_io::clear_recovery(&root, domain),
                Ok(()) => {}
            }
        }
        self.sync.downloading = downloading;
        // "last sync" means something was synced: a run in which every domain failed is not one
        if succeeded {
            self.sync.last_sync = Some(services::now_secs());
        }
        if conflicts > 0 {
            self.toast(trn(
                "Note edited on two Macs: the other text was kept as a copy",
                "{n} notes edited on two Macs: the other texts were kept as copies",
                conflicts as u64,
            ));
        }
        self.sync_flush();
    }

    /// Local data is still what the run saw: nothing was saved since (`rev`), no settings or theme
    /// edit is waiting for its delayed save, and no Notes text is waiting to be saved.
    fn sync_can_apply(&self, domain: &str, rev: u64) -> bool {
        let waiting = match domain {
            "notes" => self.notes_ctl.typing_recently(now_ms() as u64),
            "settings" => self.pending_saves.settings.is_some(),
            "themes" => self.pending_saves.themes.is_some(),
            _ => false,
        };
        !waiting && self.sync_rev(domain) == rev
    }

    /// Replace local data with the merge and save it. Returns the tracker to persist, with hashes
    /// taken from the re-exported items so a value that does not round-trip byte for byte is not
    /// mistaken for a local edit (and ping-ponged between Macs).
    pub(crate) fn sync_apply(&mut self, domain: &'static str, merged: Items, tracker: Tracker, rev: u64) -> Option<Tracker> {
        // Data and tracker are two files: a crash between the two writes would leave a tracker that
        // does not know what was applied, and the next run would take the merged items for edits made
        // here. The recovery marker makes that next run start over from the Macs' documents instead;
        // `sync_done` removes it once the tracker is saved.
        let had_marker = sync_io::needs_recovery(&self.store.root, domain);
        sync_io::mark_recovery(&self.store.root, domain);
        let applied = self.sync_apply_marked(domain, merged, tracker, rev);
        if applied.is_none() && !had_marker {
            sync_io::clear_recovery(&self.store.root, domain);
        }
        applied
    }

    fn sync_apply_marked(&mut self, domain: &'static str, merged: Items, mut tracker: Tracker, rev: u64) -> Option<Tracker> {
        // Local data must still be what the run saw. The clipboard (written by the recorder thread
        // too) is checked and replaced under one lock below.
        if domain != "clipboard" && !self.sync_can_apply(domain, rev) {
            if domain == "notes" && self.notes_ctl.typing_recently(now_ms() as u64) {
                // try again once the typing has stopped
                self.sync.notes_retry = Some(Instant::now() + Duration::from_millis(crate::notes_ctl::TYPING_IDLE_MS + 200));
                self.ctx.request_repaint_after(Duration::from_millis(crate::notes_ctl::TYPING_IDLE_MS + 300));
            }
            return None;
        }
        self.sync.applying = true;
        let mut again = false;
        // ids of merged items this build cannot read: flagged below so publish never deletes them
        let mut unreadable: Vec<String> = Vec::new();
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
                (self.snippets, unreadable) = sync_io::from_items_report(merged);
                self.save_snippets();
            }
            "quicklinks" => {
                (self.quicklinks, unreadable) = sync_io::from_items_report(merged);
                self.save_quicklinks();
            }
            "notes" => {
                // Typing in the open window waits (see `sync_can_apply`); with the window closed or
                // idle a deletion made elsewhere is applied like any other change, and the editor
                // moves off a note that is gone.
                let open_before =
                    self.notes_ctl.current.clone().and_then(|id| self.notes.iter().find(|n| n.id == id).map(|n| n.text.clone()));
                (self.notes, unreadable) = sync_io::from_items_report(merged);
                self.save_notes();
                let previous = open_before.unwrap_or_default();
                self.notes_do(|c, d| c.after_merge(d, &previous));
            }
            "themes" => {
                // (this arm runs after "settings" in the same `sync_done`, see `sync_start`)
                // the selection is stored by name: when the selected theme was renamed elsewhere,
                // find it again by id and follow the new name (a missing theme draws the default look)
                let active_id = self.active_theme().map(|t| t.id.clone());
                (self.custom_themes, unreadable) = sync_io::from_items_report(merged);
                self.save_themes();
                if let (None, Some(id)) = (self.active_theme(), active_id) {
                    if let Some(name) = self.custom_themes.iter().find(|t| t.id == id).map(|t| t.name.clone()) {
                        // a received value, not an edit: the settings tracker takes it as already synced
                        let mut s = self.settings.clone();
                        s.custom_theme = Some(name);
                        self.commit_settings(s);
                        sync_io::adopt_setting(&self.store.root, "custom_theme", &self.settings);
                    }
                }
                self.set_theme();
            }
            "chats" => {
                let (mut chats, dropped): (Vec<Chat>, _) = sync_io::from_items_report(merged);
                unreadable = dropped;
                chats.sort_by_key(|c| std::cmp::Reverse(c.updated));
                // the cap keeps the newest chats here; the others stay on the other Macs and are
                // not published as deleted (see `capped_chats`)
                if chats.len() > ferrocast_core::models::MAX_CHATS {
                    let cut = chats.split_off(ferrocast_core::models::MAX_CHATS);
                    self.sync.capped_chats.extend(cut.into_iter().map(|c| c.id));
                }
                self.chats = chats;
                let v = self.chats.clone();
                self.persist("chats.json", &v);
            }
            "clipboard" => {
                let (incoming, dropped): (Vec<ClipEntry>, _) = sync_io::from_items_report(merged);
                unreadable = dropped;
                let before = incoming.len();
                let list = sync_io::clipboard_normalize(incoming, services::now_secs(), self.settings.clipboard_retention.secs());
                again = list.len() != before;
                let mut cur = self.shared.clipboard.lock().unwrap();
                if self.shared.clip_rev.load(Ordering::SeqCst) != rev {
                    // something was recorded while the run was away: merge again next time
                    drop(cur);
                    self.sync.applying = false;
                    return None;
                }
                let old = std::mem::replace(&mut *cur, list);
                self.shared.prune_images_keeping_held(&cur);
                drop(cur);
                drop(old); // freed outside the lock
                           // saved through the shared serialisation (SAVE, then the list), never under the list lock
                self.shared.write_clipboard();
            }
            _ => {}
        }
        self.sync.applying = false;
        // (the clipboard dirty flag is not cleared here: a copy that landed after the last flush
        // while another domain was applied must still be published)
        if again {
            self.sync.pending.insert(domain); // entries dropped by retention/dedupe become deletions
        }
        ferrocast_core::sync::mark_unparsed(&mut tracker, &unreadable);
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
        let before: Vec<String> = self.chats.iter().map(|c| c.id.clone()).collect();
        ferrocast_core::models::upsert_chat(&mut self.chats, Chat::new(id, turns, services::now_secs()));
        // chats the cap pushed out of this Mac's list are not deleted anywhere else
        self.sync.capped_chats.extend(before.into_iter().filter(|b| !self.chats.iter().any(|c| &c.id == b)));
        let v = self.chats.clone();
        self.persist("chats.json", &v);
    }
}

/// Write the own documents of merges the app applied (background thread, before the run's own work).
/// Returns the documents that could not be written, to be tried again by the next run.
pub(crate) fn flush_docs(
    io: &SyncIo,
    docs: std::collections::BTreeMap<&'static str, ferrocast_core::sync::DeviceDoc>,
    root: &std::path::Path,
) -> std::collections::BTreeMap<&'static str, ferrocast_core::sync::DeviceDoc> {
    let mut failed = std::collections::BTreeMap::new();
    for (domain, doc) in docs {
        if let Err(e) = io.write_doc(domain, &doc) {
            instance::log(root, &format!("sync: writing own document {domain}: {e:#}"));
            failed.insert(domain, doc);
        }
    }
    failed
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}
