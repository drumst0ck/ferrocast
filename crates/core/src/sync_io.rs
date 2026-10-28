//! Sync between Macs: the file side of `sync` (iCloud Drive folder, device id, trackers, documents,
//! clipboard images) and the helpers that turn each persisted file into sync items.
//!
//! Nothing here runs unless the user turned sync on: callers never build a `SyncIo` while it is off.
//! Each Mac writes only `devices/<device-id>/<domain>.json` (and its own `clipboard-images/`), so no
//! file is ever written by two Macs and iCloud never needs to create conflict copies.

use crate::models::{ClipEntry, ClipKind, Settings};
use crate::sync::{self, DeviceDoc, Tracker};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Every synced file, as the domain name used in the sync folder and in the tracker file names.
pub const DOMAINS: [&str; 7] = ["settings", "snippets", "quicklinks", "notes", "clipboard", "chats", "themes"];

/// The local data file that holds a domain.
pub fn domain_file(domain: &str) -> &'static str {
    match domain {
        "settings" => "settings.json",
        "snippets" => "snippets.json",
        "quicklinks" => "quicklinks.json",
        "notes" => "notes.json",
        "clipboard" => "clipboard.json",
        "chats" => "chats.json",
        "themes" => "themes.json",
        _ => "",
    }
}

/// Domain of a data file name (`None` for files that never sync).
pub fn domain_of_file(name: &str) -> Option<&'static str> {
    DOMAINS.iter().copied().find(|d| domain_file(d) == name)
}

/// Why the sync folder cannot be used.
#[derive(Debug, Clone, PartialEq)]
pub enum Unavailable {
    /// iCloud Drive is off or not signed in: its folder does not exist.
    ICloudMissing,
    /// The folder could not be created.
    Folder(String),
}

impl Unavailable {
    pub fn message(&self) -> String {
        match self {
            Unavailable::ICloudMissing => "iCloud Drive folder not found: turn on iCloud Drive in System Settings".into(),
            Unavailable::Folder(e) => format!("Cannot use the sync folder: {e}"),
        }
    }
}

/// The sync folder: `FERROCAST_SYNC_DIR` when set, else `Ferrocast` inside iCloud Drive. Created when
/// missing, except when iCloud Drive itself is missing (nothing is made up in that case).
pub fn resolve_dir() -> Result<PathBuf, Unavailable> {
    if let Some(p) = std::env::var_os("FERROCAST_SYNC_DIR") {
        let p = PathBuf::from(p);
        return std::fs::create_dir_all(&p).map(|_| p).map_err(|e| Unavailable::Folder(e.to_string()));
    }
    let home = dirs::home_dir().ok_or_else(|| Unavailable::Folder("no home folder".into()))?;
    resolve_icloud_under(&home)
}

/// `~/Library/Mobile Documents/com~apple~CloudDocs/Ferrocast` under `home`.
pub fn resolve_icloud_under(home: &Path) -> Result<PathBuf, Unavailable> {
    let drive = home.join("Library").join("Mobile Documents").join("com~apple~CloudDocs");
    if !drive.is_dir() {
        return Err(Unavailable::ICloudMissing);
    }
    let dir = drive.join("Ferrocast");
    std::fs::create_dir_all(&dir).map(|_| dir).map_err(|e| Unavailable::Folder(e.to_string()))
}

/// The hardware UUID of this Mac (`IOPlatformUUID`), `None` when `ioreg` cannot be read.
pub fn hardware_id() -> Option<String> {
    static ID: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    ID.get_or_init(|| {
        let text =
            run_with_timeout(std::process::Command::new("/usr/sbin/ioreg").args(["-rd1", "-c", "IOPlatformExpertDevice"]), TOOL_TIMEOUT)?;
        let line = text.lines().find(|l| l.contains("IOPlatformUUID"))?;
        line.split('"').nth(3).map(str::to_string).filter(|s| !s.is_empty())
    })
    .clone()
}

/// Stable id of this Mac's data folder, bound to this Mac's hardware.
pub fn device_id(data_dir: &Path) -> String {
    device_id_with(data_dir, hardware_id().as_deref())
}

fn short_hash(s: &str) -> String {
    sync::hash(&Value::String(s.to_string())).chars().take(16).collect()
}

/// The id file holds the id and a hash of the hardware UUID it was made on. A data folder copied to
/// another Mac (Migration Assistant, a restored backup) carries another Mac's id: the hardware does
/// not match, so this Mac gets its own. A file without a hardware line (older version) or an
/// unreadable hardware UUID keeps the stored id. When the id file cannot be written, a fallback
/// derived from the hardware (or the folder path) is returned, so the id never changes between runs.
pub fn device_id_with(data_dir: &Path, hardware: Option<&str>) -> String {
    let p = data_dir.join("sync-device-id");
    let hw = hardware.map(short_hash);
    let valid = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    let stored = std::fs::read_to_string(&p).ok().and_then(|t| {
        let mut lines = t.lines();
        let id = lines.next()?.trim().to_string();
        valid(&id).then(|| (id, lines.next().map(|l| l.trim().to_string()).filter(|l| !l.is_empty())))
    });
    let write = |id: &str| {
        let text = match &hw {
            Some(h) => format!("{id}\n{h}\n"),
            None => format!("{id}\n"),
        };
        write_atomic(&p, text.as_bytes())
    };
    if let Some((id, stored_hw)) = &stored {
        match (&hw, stored_hw) {
            (Some(h), Some(sh)) if h != sh => eprintln!("ferrocast: sync: data folder was made on another Mac; new device id"),
            (Some(_), None) => {
                let _ = write(id); // adopt: remember this Mac's hardware
                return id.clone();
            }
            _ => return id.clone(),
        }
    }
    let id = uuid::Uuid::new_v4().to_string();
    match write(&id) {
        Ok(()) => id,
        Err(e) => {
            let fallback = format!("fb-{}", hw.unwrap_or_else(|| short_hash(&data_dir.display().to_string())));
            eprintln!("ferrocast: sync: cannot save the device id ({e}); using {fallback}");
            fallback
        }
    }
}

fn tracker_path(data_dir: &Path, domain: &str) -> PathBuf {
    data_dir.join("sync-state").join(format!("{domain}.json"))
}

/// The tracker of a domain (kept in the local data dir, never in iCloud). Missing or unreadable = empty.
pub fn load_tracker(data_dir: &Path, domain: &str) -> Tracker {
    std::fs::read(tracker_path(data_dir, domain)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn save_tracker(data_dir: &Path, domain: &str, t: &Tracker) -> anyhow::Result<()> {
    write_atomic(&tracker_path(data_dir, domain), &serde_json::to_vec(t)?)
}

/// Temp file + rename in the same folder.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Where and as whom this Mac reads and writes.
#[derive(Debug, Clone)]
pub struct SyncIo {
    pub dir: PathBuf,
    pub device: String,
    /// Local `clipboard-images/`.
    pub images_dir: PathBuf,
    /// Cleared when the user turns sync off: a run in progress writes nothing more.
    pub enabled: Arc<AtomicBool>,
    /// Asks iCloud to download an evicted file (`brctl download`).
    pub download: fn(&Path),
}

/// How long `ioreg` and `brctl` may run before they are killed.
const TOOL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Run a tool and return its stdout; `None` when it could not start or was still running after
/// `limit` (then it is killed), so a hung tool can never stop sync.
pub fn run_with_timeout(cmd: &mut std::process::Command, limit: std::time::Duration) -> Option<String> {
    use std::io::Read;
    let mut child =
        cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).spawn().ok()?;
    let mut out = child.stdout.take()?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = out.read_to_end(&mut buf);
        let _ = tx.send(buf);
    });
    let deadline = std::time::Instant::now() + limit;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < deadline => std::thread::sleep(std::time::Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    rx.recv_timeout(std::time::Duration::from_secs(1)).ok().map(|b| String::from_utf8_lossy(&b).to_string())
}

/// Ask iCloud Drive to download a file that is only a placeholder here.
pub fn brctl_download(path: &Path) {
    let _ = run_with_timeout(std::process::Command::new("/usr/bin/brctl").arg("download").arg(path), TOOL_TIMEOUT);
}

/// `dir/name` has not been downloaded when iCloud left `dir/.name.icloud` in its place.
fn is_placeholder(path: &Path) -> bool {
    match (path.parent(), path.file_name()) {
        (Some(dir), Some(name)) => !path.exists() && dir.join(format!(".{}.icloud", name.to_string_lossy())).exists(),
        _ => false,
    }
}

/// Parse a device document; the old settings document (all settings as the one item `settings`)
/// is read as one record per field.
fn parse_doc(domain: &str, bytes: &[u8]) -> serde_json::Result<DeviceDoc> {
    let mut doc: DeviceDoc = serde_json::from_slice(bytes)?;
    if domain == "settings" {
        expand_legacy_settings(&mut doc);
    }
    Ok(doc)
}

/// Replace the single `settings` item of an old document by one record per field, each with the old
/// record's time, pristine and adopted flags. An old deletion of the whole item is dropped.
pub fn expand_legacy_settings(doc: &mut DeviceDoc) {
    let Some(rec) = doc.items.remove(SETTINGS_ID) else { return };
    let Some(value) = rec.value.filter(|_| !rec.deleted) else { return };
    for (id, v) in flatten_settings(&value) {
        doc.items.entry(id).or_insert(sync::Record {
            modified: rec.modified,
            deleted: false,
            value: Some(v),
            pristine: rec.pristine,
            adopted: rec.adopted,
        });
    }
}

/// What reading the other Macs' documents found.
pub struct ReadDocs {
    pub docs: Vec<DeviceDoc>,
    /// Documents that could not be parsed.
    pub corrupt: Vec<String>,
    /// Documents still being downloaded from iCloud.
    pub downloading: usize,
}

impl SyncIo {
    pub fn new(dir: PathBuf, device: String, images_dir: PathBuf) -> SyncIo {
        SyncIo { dir, device, images_dir, enabled: Arc::new(AtomicBool::new(true)), download: brctl_download }
    }

    fn on(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    fn device_dir(&self, device: &str) -> PathBuf {
        self.dir.join("devices").join(device)
    }

    fn own_images(&self) -> PathBuf {
        self.device_dir(&self.device).join("clipboard-images")
    }

    /// Number of other Macs that have a folder here.
    pub fn peers(&self) -> usize {
        std::fs::read_dir(self.dir.join("devices"))
            .map(|rd| rd.flatten().filter(|e| e.path().is_dir() && e.file_name().to_string_lossy() != self.device).count())
            .unwrap_or(0)
    }

    /// Documents of the other Macs for a domain; unreadable or corrupt ones are skipped and named in
    /// the second value.
    pub fn read_docs(&self, domain: &str) -> ReadDocs {
        let mut docs = Vec::new();
        let mut bad = Vec::new();
        let mut downloading = 0;
        let Ok(rd) = std::fs::read_dir(self.dir.join("devices")) else { return ReadDocs { docs, corrupt: bad, downloading } };
        let mut names: Vec<_> = rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        names.sort();
        for dev in names {
            if dev == self.device {
                continue;
            }
            let p = self.device_dir(&dev).join(format!("{domain}.json"));
            let Ok(bytes) = std::fs::read(&p) else {
                if is_placeholder(&p) {
                    (self.download)(&p);
                    downloading += 1;
                }
                continue;
            };
            match parse_doc(domain, &bytes) {
                Ok(mut doc) => {
                    doc.device = dev; // the folder name is the authority
                    docs.push(doc);
                }
                Err(e) => {
                    eprintln!("ferrocast: sync: ignoring {}: {e}", p.display());
                    bad.push(p.display().to_string());
                }
            }
        }
        ReadDocs { docs, corrupt: bad, downloading }
    }

    /// This Mac's own published document of a domain, if it exists and can be read.
    pub fn read_own_doc(&self, domain: &str) -> Option<DeviceDoc> {
        let bytes = std::fs::read(self.device_dir(&self.device).join(format!("{domain}.json"))).ok()?;
        parse_doc(domain, &bytes).ok()
    }

    /// Write this Mac's document; an identical one is left alone (no needless iCloud uploads).
    pub fn write_doc(&self, domain: &str, doc: &DeviceDoc) -> anyhow::Result<()> {
        anyhow::ensure!(self.on(), "sync turned off");
        let p = self.device_dir(&self.device).join(format!("{domain}.json"));
        let bytes = serde_json::to_vec_pretty(doc)?;
        if std::fs::read(&p).is_ok_and(|old| old == bytes) {
            return Ok(());
        }
        write_atomic(&p, &bytes)
    }
}

/// The clipboard keeps its deletions for 400 days, longer than the longest retention (a year);
/// everything else keeps them for ever (see `sync::publish_ttl`).
const CLIPBOARD_HORIZON_MS: i64 = 400 * 24 * 3600 * 1000;

fn tombstone_ttl(domain: &str) -> Option<i64> {
    (domain == "clipboard").then_some(CLIPBOARD_HORIZON_MS)
}

/// What one sync of one domain produced.
#[derive(Debug, Clone)]
pub struct Outcome {
    /// The merged items when they differ from the snapshot; `None` = nothing to apply.
    pub merged: Option<Vec<(String, Value)>>,
    /// Tracker after publishing the snapshot: persist it when `merged` is not applied.
    pub tracker_published: Tracker,
    /// Tracker that already knows the merged items: persist it once `merged` is applied
    /// (equal to `tracker_published` when there is nothing to apply).
    pub tracker_merged: Tracker,
    pub peers: usize,
    /// Documents that could not be read.
    pub corrupt: Vec<String>,
    /// Files (documents or images) still being downloaded from iCloud.
    pub downloading: usize,
    /// This Mac's document holding the merged state. It is not written by `sync_domain` when there is
    /// something to apply: the caller writes it (`SyncIo::write_doc`) only once the merge was applied
    /// to the local data. Until then the own document keeps the published snapshot, so a tracker
    /// rebuilt from it cannot turn an older local value into a new edit. `None` for the clipboard
    /// (its document keeps to what was copied here) and when nothing is to be applied.
    pub applied_doc: Option<DeviceDoc>,
    /// The run started over from this Mac's data as if it had never synced (see [`needs_recovery`]).
    pub recovered: bool,
}

/// Publish the snapshot of local items, read the other Macs' documents and merge. Writes only this
/// Mac's document and images in the sync folder. Pure with respect to local data files: the caller
/// applies `merged` and persists a tracker.
pub fn sync_domain(io: &SyncIo, domain: &str, snapshot: &[(String, Value)], tracker: &Tracker, now: i64) -> anyhow::Result<Outcome> {
    sync_domain_recovering(io, domain, snapshot, tracker, now, false)
}

fn recovery_marker(data_dir: &Path, domain: &str) -> PathBuf {
    data_dir.join("sync-state").join(format!("{domain}.recover"))
}

/// Record that this Mac's data file of `domain` could not be loaded (unreadable or unparsable, set
/// aside as `<file>.corrupt` when possible). What is loaded instead is not what this Mac had synced,
/// so comparing it with the tracker would publish every missing item as a deletion on all Macs: the
/// next sync of the domain starts over instead (see [`sync_domain_recovering`]). An explicit marker,
/// not file times, so copying a data folder (with an old `.corrupt` file in it) triggers nothing.
pub fn mark_recovery(data_dir: &Path, domain: &str) {
    let p = recovery_marker(data_dir, domain);
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = std::fs::write(&p, b"") {
        eprintln!("ferrocast: could not write {}: {e}", p.display());
    }
}

/// [`mark_recovery`] for a data file name (`None` of the files that never sync does nothing).
pub fn mark_recovery_for_file(data_dir: &Path, file: &str) {
    if let Some(domain) = domain_of_file(file) {
        mark_recovery(data_dir, domain);
    }
}

/// True while the marker of [`mark_recovery`] exists.
pub fn needs_recovery(data_dir: &Path, domain: &str) -> bool {
    recovery_marker(data_dir, domain).exists()
}

/// Remove the marker once the recovered state is saved (or there was nothing to apply).
pub fn clear_recovery(data_dir: &Path, domain: &str) {
    let _ = std::fs::remove_file(recovery_marker(data_dir, domain));
}

/// A setting this Mac changed because another Mac's data said so (not an edit): record the new
/// value as the one already synced, keeping its time, so the next run does not stamp it as a local
/// edit that could beat a newer value from another Mac. Does nothing when the field was never synced.
pub fn adopt_setting(data_dir: &Path, key: &str, settings: &Settings) {
    let mut tracker = load_tracker(data_dir, "settings");
    let Some(value) = settings_items(settings).into_iter().find(|(k, _)| k == key).map(|(_, v)| v) else { return };
    if let Some(t) = tracker.items.get_mut(key) {
        t.hash = sync::hash(&value);
        let _ = save_tracker(data_dir, "settings", &tracker);
    }
}

/// [`sync_domain`]; with `recover` the run ignores the tracker and this Mac's own document and
/// treats the local data as a first sync: nothing is published as a deletion, what the other Macs
/// have is adopted, and the saved tracker is newer than the `.corrupt` file afterwards.
pub fn sync_domain_recovering(
    io: &SyncIo,
    domain: &str,
    snapshot: &[(String, Value)],
    tracker: &Tracker,
    now: i64,
    recover: bool,
) -> anyhow::Result<Outcome> {
    anyhow::ensure!(io.on(), "sync turned off");
    let fresh = Tracker::default();
    let tracker = if recover { &fresh } else { tracker };
    if domain == "clipboard" {
        export_images(io, snapshot);
    }
    // First-sync rule (see `sync::restamp_baseline`):
    //  1. On this Mac's first sync of a domain (also after a missing or corrupt tracker) its items,
    //     and deletions of built-in defaults it lacks, are published at modified 0, whether or not
    //     iCloud has listed the other Macs' folders yet. Any real time beats 0.
    //  2. On every later run, after merging, records still at 0 (they are the winner or nobody else
    //     has them) are moved once to the time of this Mac's first sync and published.
    //  3. A tie at 0 (two Macs that first-sync before either has run twice) goes to the greater
    //     device id on both Macs, so they still converge.
    // Net effect: the Mac that synced first keeps its data, a Mac that joins later adopts it, items
    // only the newcomer has are added, and genuine edits made afterwards win by their time.
    // A lost or corrupt tracker (and the tracker of the old one-item settings) is rebuilt from this
    // Mac's own document, so edits and deletions made since the last sync are not forgotten.
    let rebuilt;
    let tracker = if !recover && (!tracker.seeded || (domain == "settings" && tracker.items.contains_key(SETTINGS_ID))) {
        match io.read_own_doc(domain) {
            Some(own) => {
                rebuilt = sync::tracker_from_own_doc(&own);
                &rebuilt
            }
            None if tracker.seeded => {
                rebuilt = Tracker::default();
                &rebuilt
            }
            None => tracker,
        }
    } else {
        tracker
    };
    let first = !tracker.seeded;
    let clipboard = domain == "clipboard";
    let opts = sync::PublishOpts {
        first_stamp: 0,
        ttl: tombstone_ttl(domain),
        baseline_deleted: if first { default_ids(domain) } else { vec![] },
        relay_tombstones: !clipboard,
        max_tombstones: clipboard.then_some(MAX_CLIPBOARD_TOMBSTONES),
        pristine: pristine_check(domain),
    };
    // A recovering run reads this Mac's own document before anything is written: it is the intact
    // copy of what this Mac had published (items only it had, edits not pulled yet), and it takes
    // part in the merge like another Mac's document. It is not overwritten with the loaded data;
    // the merged document is written once the merge is applied (`Outcome::applied_doc`).
    let own_copy = if recover { io.read_own_doc(domain) } else { None };
    let (doc, t1) = sync::publish_opts(&io.device, snapshot, tracker, now, &opts);
    if !recover {
        io.write_doc(domain, &doc)?;
    }
    let ReadDocs { mut docs, corrupt, mut downloading } = io.read_docs(domain);
    // deletions recorded in the own copy (the merge skips a tombstone for an id this Mac no longer
    // has, and a tombstone that wins over a loaded item is recorded as someone else's)
    let own_tombstones: Vec<(String, i64)> =
        own_copy.iter().flat_map(|o| o.items.iter().filter(|(_, r)| r.deleted).map(|(id, r)| (id.clone(), r.modified))).collect();
    if let Some(mut own) = own_copy {
        own.device = io.device.clone();
        docs.push(own);
    }
    if clipboard {
        // Clipboard tombstones expire (after the longest retention), so entries from before that
        // horizon cannot be told from deleted ones and are not adopted from other Macs.
        let horizon = CLIPBOARD_HORIZON_MS / 1000;
        for d in docs.iter_mut() {
            d.items.retain(|_, r| {
                r.deleted
                    || r.value.as_ref().and_then(|v| v.get("copied_at")).and_then(Value::as_i64).is_none_or(|at| now / 1000 - at <= horizon)
            });
        }
    }
    let (merged, t2) = sync::merge_as(&io.device, snapshot, &docs, &t1);
    let mut t2 = t2;
    // Own deletions survive a recovery: published again as this Mac's own (not `remote`, so the
    // clipboard, which does not relay others' deletions, still carries them). Done after the merge,
    // never before: a tombstone in the tracker for an id the reloaded defaults contain would look
    // like a new edit and bring the item back.
    let kept: std::collections::HashSet<&str> = merged.iter().map(|(id, _)| id.as_str()).collect();
    for (id, modified) in own_tombstones.iter().filter(|(id, _)| !kept.contains(id.as_str())) {
        match t2.items.get_mut(id) {
            Some(t) if t.deleted => t.remote = false,
            Some(_) => {}
            None => {
                t2.items.insert(
                    id.clone(),
                    sync::Tracked {
                        hash: String::new(),
                        modified: *modified,
                        deleted: true,
                        remote: false,
                        pristine: false,
                        adopted: false,
                    },
                );
            }
        }
    }
    if !first {
        sync::restamp_baseline(&mut t2);
    }
    if clipboard {
        downloading += import_images(io, &merged, &docs);
    }
    // what this Mac publishes now: the merged state with the re-stamped baseline
    let (doc2, t3) = sync::publish_opts(&io.device, &merged, &t2, now, &opts);
    // Own document = the merged state (unchanged bytes are not rewritten), but only once it matches
    // the local data, which is what a tracker rebuilt from it needs: with nothing to apply it is
    // written now; otherwise it goes out as `applied_doc` and the caller writes it after applying.
    // (The clipboard keeps its document to what was copied here: entries taken over from other Macs
    // are in their documents, and republishing them would double its size.)
    if merged == snapshot {
        io.write_doc(domain, &doc2)?;
        return Ok(Outcome {
            merged: None,
            tracker_published: t3.clone(),
            tracker_merged: t3,
            peers: io.peers(),
            corrupt,
            downloading,
            applied_doc: None,
            recovered: recover,
        });
    }
    let applied_doc = (!clipboard || recover).then_some(doc2);
    Ok(Outcome {
        merged: Some(merged),
        tracker_published: t1,
        tracker_merged: t3,
        peers: io.peers(),
        corrupt,
        downloading,
        applied_doc,
        recovered: recover,
    })
}

/// Pristine = the value is the built-in default of its item (see `sync::Record::pristine`): the
/// shared part of the settings (local-only fields removed) equals `Settings::default()`, a built-in
/// snippet or quicklink equals its default content. Everything else is never pristine.
fn pristine_check(domain: &str) -> Option<fn(&str, &Value) -> bool> {
    match domain {
        "settings" => Some(|id, v| default_settings_items().get(id) == Some(v)),
        "snippets" => {
            Some(|id, v| crate::storage::default_snippets().iter().any(|d| d.id == id && serde_json::to_value(d).ok().as_ref() == Some(v)))
        }
        "quicklinks" => Some(|id, v| {
            crate::storage::default_quicklinks().iter().any(|d| d.id == id && serde_json::to_value(d).ok().as_ref() == Some(v))
        }),
        _ => None,
    }
}

fn default_settings_items() -> &'static std::collections::BTreeMap<String, Value> {
    static D: std::sync::OnceLock<std::collections::BTreeMap<String, Value>> = std::sync::OnceLock::new();
    D.get_or_init(|| settings_items(&Settings::default()).into_iter().collect())
}

/// Ids of the built-in default items of a domain (present on every fresh install).
fn default_ids(domain: &str) -> Vec<String> {
    match domain {
        "snippets" => crate::storage::default_snippets().into_iter().map(|s| s.id).collect(),
        "quicklinks" => crate::storage::default_quicklinks().into_iter().map(|q| q.id).collect(),
        _ => vec![],
    }
}

/// The clipboard document keeps at most this many deletions (the newest): heavy copying would
/// otherwise grow it for ever. An entry whose deletion was forgotten can come back only from a
/// device that was offline longer than the oldest kept deletion, and the retention and cap remove it
/// again.
const MAX_CLIPBOARD_TOMBSTONES: usize = 3000;

fn safe_name(name: &str) -> bool {
    !name.is_empty() && !name.contains('/') && !name.contains('\\') && !name.contains("..")
}

fn image_names(items: &[(String, Value)]) -> Vec<String> {
    items
        .iter()
        .filter(|(_, v)| v.get("kind").and_then(Value::as_str) == Some("Image"))
        .filter_map(|(_, v)| v.get("content").and_then(Value::as_str))
        .filter(|n| safe_name(n))
        .map(str::to_string)
        .collect()
}

/// Copy the images the local entries reference into this Mac's folder; drop the ones no entry uses.
fn export_images(io: &SyncIo, items: &[(String, Value)]) {
    let names = image_names(items);
    let dir = io.own_images();
    for n in &names {
        let (src, dst) = (io.images_dir.join(n), dir.join(n));
        if io.on() && src.is_file() && !dst.exists() {
            let _ = std::fs::create_dir_all(&dir);
            if let Err(e) = std::fs::copy(&src, &dst) {
                eprintln!("ferrocast: sync: copying {n}: {e}");
            }
        }
    }
    if let Ok(rd) = std::fs::read_dir(&dir).map(|rd| rd.take_while(|_| io.on())) {
        for e in rd.flatten() {
            if !names.contains(&e.file_name().to_string_lossy().to_string()) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

/// Bring images of merged entries that are missing locally from the other Macs' folders. Returns how
/// many are still placeholders being downloaded from iCloud.
fn import_images(io: &SyncIo, items: &[(String, Value)], docs: &[DeviceDoc]) -> usize {
    let mut downloading = 0;
    for n in image_names(items) {
        let dst = io.images_dir.join(&n);
        if dst.exists() {
            continue;
        }
        let mut pending = false;
        for d in docs {
            let src = io.device_dir(&d.device).join("clipboard-images").join(&n);
            if src.is_file() {
                if !io.on() {
                    return downloading;
                }
                let _ = std::fs::create_dir_all(&io.images_dir);
                if std::fs::copy(&src, &dst).is_ok() {
                    pending = false;
                    break;
                }
            } else if is_placeholder(&src) {
                (io.download)(&src);
                pending = true;
            }
        }
        if pending {
            downloading += 1;
        }
    }
    downloading
}

// ------------------------------------------------------------------------- items <-> files

/// Items of a list keyed by their `id` field (entries without one are skipped).
pub fn to_items<T: Serialize>(list: &[T]) -> Vec<(String, Value)> {
    list.iter()
        .filter_map(|x| {
            let v = serde_json::to_value(x).ok()?;
            let id = v.get("id")?.as_str()?.to_string();
            Some((id, v))
        })
        .collect()
}

/// The list back from items; values that do not deserialize are dropped.
pub fn from_items<T: DeserializeOwned>(items: Vec<(String, Value)>) -> Vec<T> {
    items.into_iter().filter_map(|(_, v)| serde_json::from_value(v).ok()).collect()
}

/// Settings fields that stay on each Mac (S6): machine paths, login item, onboarding, update state,
/// UI state, the sync switch itself, and the two tiling fields that depend on the Mac (the switch
/// that moves windows and the layout per display index), and the Raycast extensions switches (Node and
/// the installed extensions exist per Mac).
pub const LOCAL_ONLY: [&str; 16] = [
    "language",
    "script_directories",
    "obsidian_vault",
    "launch_at_login",
    "onboarded",
    "update_check",
    "update_install",
    "update_skipped",
    "update_last_check",
    "notes_last",
    "notes_zoom",
    "sync_enabled",
    "tiling_enabled",
    "tiling_display_layouts",
    "run_raycast_extensions",
    "raycast_extensions_disabled",
];
/// Local-only fields inside `capture`: the devices and the folders differ per Mac.
pub const LOCAL_ONLY_CAPTURE: [&str; 4] = ["camera_id", "mic_id", "folder", "recordings_folder"];

/// Id of the single item the first version synced all settings as; still read from old documents.
pub const SETTINGS_ID: &str = "settings";

/// The shared settings, one sync item per field, without the local-only fields. Top-level fields are
/// items named after the field; the nested `capture` settings and the per-command `commands` map get
/// one item per nested key (`capture.fps`, `commands.<id>`). Per-field items let two Macs that
/// changed different settings keep both.
pub fn settings_items(s: &Settings) -> Vec<(String, Value)> {
    flatten_settings(&serde_json::to_value(s).unwrap_or(Value::Null))
}

fn flatten_settings(v: &Value) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    let Some(o) = v.as_object() else { return out };
    for (k, val) in o {
        if LOCAL_ONLY.contains(&k.as_str()) {
            continue;
        }
        match (k.as_str(), val.as_object()) {
            ("capture", Some(c)) => out.extend(
                c.iter().filter(|(ck, _)| !LOCAL_ONLY_CAPTURE.contains(&ck.as_str())).map(|(ck, cv)| (format!("capture.{ck}"), cv.clone())),
            ),
            ("commands", Some(c)) => out.extend(c.iter().map(|(ck, cv)| (format!("commands.{ck}"), cv.clone()))),
            _ => out.push((k.clone(), val.clone())),
        }
    }
    out
}

/// Merged settings with this Mac's local-only values kept. Fields missing from `merged` keep this
/// Mac's value, except the per-command map, which is exactly what was merged. `None` when the result
/// does not deserialize (then nothing is applied).
pub fn settings_from_items(merged: &[(String, Value)], local: &Settings) -> Option<Settings> {
    let mut o = serde_json::to_value(local).ok()?.as_object()?.clone();
    let mut capture = o.get("capture").and_then(Value::as_object).cloned().unwrap_or_default();
    let mut commands = serde_json::Map::new();
    for (id, v) in merged {
        if let Some(k) = id.strip_prefix("capture.") {
            if !LOCAL_ONLY_CAPTURE.contains(&k) {
                capture.insert(k.to_string(), v.clone());
            }
        } else if let Some(k) = id.strip_prefix("commands.") {
            commands.insert(k.to_string(), v.clone());
        } else if id != SETTINGS_ID && !LOCAL_ONLY.contains(&id.as_str()) {
            o.insert(id.clone(), v.clone());
        }
    }
    o.insert("capture".into(), Value::Object(capture));
    o.insert("commands".into(), Value::Object(commands));
    serde_json::from_value(Value::Object(o)).ok()
}

/// Keep a copy of this Mac's `settings.json` as `settings.before-sync.json` the first time sync is
/// about to merge, as a safety net. An existing copy is never overwritten; nothing is made up when
/// there is no settings file.
pub fn backup_settings_once(data_dir: &Path) {
    let (from, to) = (data_dir.join("settings.json"), data_dir.join("settings.before-sync.json"));
    if from.is_file() && !to.exists() {
        if let Err(e) = std::fs::copy(&from, &to) {
            eprintln!("ferrocast: sync: could not back up settings.json: {e}");
        }
    }
}

/// Clipboard list after a merge: entries past the retention go (pinned stay), Macs that copied the
/// same thing keep one entry (the newest, then the greater id, so every Mac keeps the same one), and
/// the list keeps at most 1000 unpinned entries.
pub fn clipboard_normalize(mut list: Vec<ClipEntry>, now_secs: i64, retention_secs: Option<i64>) -> Vec<ClipEntry> {
    if let Some(secs) = retention_secs {
        list.retain(|e| e.pinned || now_secs - e.copied_at < secs);
    }
    let kind_key = |k: &ClipKind| format!("{k:?}");
    let mut best: std::collections::HashMap<(String, String), (i64, String)> = std::collections::HashMap::new();
    let mut pinned_any: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    for e in &list {
        let key = (kind_key(&e.kind), e.content.clone());
        if e.pinned {
            pinned_any.insert(key.clone());
        }
        let cand = (e.copied_at, e.id.clone());
        if best.get(&key).is_none_or(|b| cand > *b) {
            best.insert(key, cand);
        }
    }
    list.retain(|e| best.get(&(kind_key(&e.kind), e.content.clone())).is_some_and(|b| b.1 == e.id));
    for e in list.iter_mut() {
        // a pin on any of the merged copies is kept
        if pinned_any.contains(&(kind_key(&e.kind), e.content.clone())) {
            e.pinned = true;
        }
    }
    let mut unpinned = 0;
    list.retain(|e| {
        if e.pinned {
            return true;
        }
        unpinned += 1;
        unpinned <= 1000
    });
    list
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Note, Quicklink, Snippet};
    use crate::storage::Store;
    use serde_json::json;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ferrocast-sync-{tag}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Both orders of two device ids: the outcome must not depend on which id is greater.
    const ID_ORDERS: [(&str, &str); 2] = [("aa", "zz"), ("zz", "aa")];

    /// One simulated Mac: its own data dir and device id, sharing the sync dir.
    struct Mac {
        store: Store,
        io: SyncIo,
        snippets: Vec<Snippet>,
        quicklinks: Vec<Quicklink>,
        settings: Settings,
        clipboard: Vec<ClipEntry>,
    }

    impl Mac {
        fn new(data: &Path, shared: &Path) -> Mac {
            let store = Store::new(data);
            let io = SyncIo::new(shared.to_path_buf(), device_id(data), store.images_dir());
            Mac { store, io, snippets: vec![], quicklinks: vec![], settings: Settings::default(), clipboard: vec![] }
        }

        /// Only the clipboard domain, applying the merge the way the app does (retention, dedupe, cap).
        fn sync_clipboard(&mut self, now: i64) {
            let d = &self.store.root;
            let o = sync_domain(&self.io, "clipboard", &to_items(&self.clipboard), &load_tracker(d, "clipboard"), now).unwrap();
            save_tracker(d, "clipboard", &o.tracker_merged).unwrap();
            if let Some(m) = o.merged {
                self.clipboard = clipboard_normalize(from_items(m), now / 1000, Some(30 * 86_400));
            }
        }

        /// A Mac that loads its data like the app does at launch (defaults on a fresh install).
        fn launch(data: &Path, shared: &Path) -> Mac {
            let mut m = Mac::new(data, shared);
            m.restart();
            m
        }

        /// Quit and relaunch: everything comes back from the data folder.
        fn restart(&mut self) {
            let d = self.store.load_all();
            self.snippets = d.snippets;
            self.quicklinks = d.quicklinks;
            self.settings = d.settings;
            self.clipboard = d.clipboard;
        }

        /// One full sync cycle of the three domains the tests use, applying the merge like the app.
        fn sync(&mut self, now: i64) {
            let d = &self.store.root;
            let recover = needs_recovery(d, "snippets");
            let o = sync_domain_recovering(&self.io, "snippets", &to_items(&self.snippets), &load_tracker(d, "snippets"), now, recover)
                .unwrap();
            save_tracker(d, "snippets", &o.tracker_merged).unwrap();
            if o.recovered {
                clear_recovery(d, "snippets");
            }
            if let Some(m) = o.merged {
                self.snippets = from_items(m);
                self.store.save("snippets.json", &self.snippets).unwrap();
                self.io.write_doc("snippets", o.applied_doc.as_ref().unwrap()).unwrap();
            }
            let o = sync_domain(&self.io, "quicklinks", &to_items(&self.quicklinks), &load_tracker(d, "quicklinks"), now).unwrap();
            save_tracker(d, "quicklinks", &o.tracker_merged).unwrap();
            if let Some(m) = o.merged {
                self.quicklinks = from_items(m);
                self.store.save("quicklinks.json", &self.quicklinks).unwrap();
                self.io.write_doc("quicklinks", o.applied_doc.as_ref().unwrap()).unwrap();
            }
            let o = sync_domain(&self.io, "settings", &settings_items(&self.settings), &load_tracker(d, "settings"), now).unwrap();
            save_tracker(d, "settings", &o.tracker_merged).unwrap();
            if let Some(m) = o.merged {
                self.settings = settings_from_items(&m, &self.settings).unwrap();
                self.store.save("settings.json", &self.settings).unwrap();
                self.io.write_doc("settings", o.applied_doc.as_ref().unwrap()).unwrap();
            }
            let o = sync_domain(&self.io, "clipboard", &to_items(&self.clipboard), &load_tracker(d, "clipboard"), now).unwrap();
            save_tracker(d, "clipboard", &o.tracker_merged).unwrap();
            if let Some(m) = o.merged {
                self.clipboard = from_items(m);
            }
        }
    }

    fn snip(id: &str, name: &str) -> Snippet {
        Snippet { id: id.into(), name: name.into(), text: format!("text of {name}"), keyword: String::new(), tags: vec![] }
    }

    #[test]
    fn two_macs_share_edits_deletions_and_concurrent_edits() {
        let (da, db, shared) = (tmp("a"), tmp("b"), tmp("shared"));
        let (mut a, mut b) = (Mac::new(&da, &shared), Mac::new(&db, &shared));
        assert_ne!(a.io.device, b.io.device);

        // each Mac adds its own snippet; both end up with both
        a.snippets = vec![snip("s1", "one"), snip("s2", "two")];
        b.snippets = vec![snip("s3", "three")];
        a.sync(1_000);
        b.sync(1_100);
        a.sync(1_200);
        let names = |m: &Mac| {
            let mut n: Vec<_> = m.snippets.iter().map(|s| s.name.clone()).collect();
            n.sort();
            n
        };
        assert_eq!(names(&a), ["one", "three", "two"]);
        assert_eq!(names(&b), ["one", "three", "two"]);

        // a deletion on B reaches A and nobody gets the snippet back
        b.snippets.retain(|s| s.id != "s2");
        b.sync(2_000);
        a.sync(2_100);
        b.sync(2_200);
        assert_eq!(names(&a), ["one", "three"]);
        assert_eq!(names(&b), ["one", "three"]);

        // the same snippet edited on both before they meet: the newest edit wins on both
        a.snippets.iter_mut().find(|s| s.id == "s1").unwrap().text = "A edit".into();
        a.sync(3_000);
        b.snippets.iter_mut().find(|s| s.id == "s1").unwrap().text = "B edit".into();
        b.sync(3_500); // newer than A's edit
        a.sync(4_000);
        b.sync(4_100);
        for m in [&a, &b] {
            assert_eq!(m.snippets.iter().find(|s| s.id == "s1").unwrap().text, "B edit");
        }
        // merged items are not republished as new edits: the next cycle changes nothing
        a.sync(5_000); // the first cycle after applying republishes the merged state as it is
        let before = std::fs::read(shared.join("devices").join(&a.io.device).join("snippets.json")).unwrap();
        a.sync(9_000);
        assert_eq!(std::fs::read(shared.join("devices").join(&a.io.device).join("snippets.json")).unwrap(), before);

        for d in [da, db, shared] {
            std::fs::remove_dir_all(d).unwrap();
        }
    }

    #[test]
    fn the_tiling_switch_and_display_layouts_stay_on_each_mac_but_tiling_preferences_sync() {
        for (ida, idb) in ID_ORDERS {
            let (da, db, shared) = (tmp("a"), tmp("b"), tmp("shared"));
            let (mut a, mut b) = (Mac::new(&da, &shared), Mac::new(&db, &shared));
            a.io.device = ida.into();
            b.io.device = idb.into();
            a.settings.tiling_enabled = true;
            a.settings.tiling_display_layouts.insert("1".into(), 3);
            a.settings.tiling_layout = 2;
            a.settings.tiling_gap = 14.0;
            a.settings.tiling_excluded = vec!["com.example.app".into()];
            a.settings.tiling_border = true;
            a.settings.tiling_border_color = "#112233".into();
            a.settings.tiling_quick_actions = true;
            a.settings.tiling_hover_activate = true;
            a.sync(1_000);
            a.sync(1_500);
            b.sync(2_000);
            // preferences travel
            assert_eq!(b.settings.tiling_layout, 2);
            assert_eq!(b.settings.tiling_gap, 14.0);
            assert_eq!(b.settings.tiling_excluded, vec!["com.example.app".to_string()]);
            assert!(b.settings.tiling_border && b.settings.tiling_quick_actions && b.settings.tiling_hover_activate);
            assert_eq!(b.settings.tiling_border_color, "#112233");
            // moving windows is switched on per Mac, and display indices differ per Mac
            assert!(!b.settings.tiling_enabled, "tiling is never switched on by another Mac");
            assert!(b.settings.tiling_display_layouts.is_empty(), "display layouts are per Mac");
            a.sync(3_000);
            assert!(a.settings.tiling_enabled && a.settings.tiling_display_layouts.get("1") == Some(&3));
            // the shared item list has neither field
            let names: Vec<String> = settings_items(&a.settings).into_iter().map(|i| i.0).collect();
            assert!(!names.iter().any(|n| n == "tiling_enabled" || n == "tiling_display_layouts"), "{names:?}");
            assert!(names.iter().any(|n| n == "tiling_gap"));
            for d in [da, db, shared] {
                std::fs::remove_dir_all(d).unwrap();
            }
        }
    }

    #[test]
    fn settings_sync_but_local_only_fields_stay_on_each_mac() {
        for (ida, idb) in ID_ORDERS {
            let (da, db, shared) = (tmp("a"), tmp("b"), tmp("shared"));
            let (mut a, mut b) = (Mac::new(&da, &shared), Mac::new(&db, &shared));
            a.io.device = ida.into();
            b.io.device = idb.into();
            a.settings.theme = crate::models::Theme::Dark;
            a.settings.launcher_hotkey = "Ctrl+Space".into();
            a.settings.script_directories = vec!["/Users/a/scripts".into()];
            a.settings.launch_at_login = true;
            a.settings.language = "de".into();
            a.settings.onboarded = true;
            a.settings.capture.camera_id = "cam-a".into();
            a.settings.capture.mic_id = "mic-a".into();
            a.settings.capture.fps = 30;
            a.settings.notes_last = Some("note-a".into());
            a.settings.sync_enabled = true;
            a.settings.capture.folder = "/Users/a/Shots".into();
            a.settings.capture.recordings_folder = "/Users/a/Recs".into();
            b.settings.script_directories = vec!["/Users/b/scripts".into()];
            b.settings.capture.camera_id = "cam-b".into();
            b.settings.capture.folder = "/Users/b/Shots".into();
            b.settings.capture.recordings_folder = "/Users/b/Recs".into();
            b.settings.notes_last = Some("note-b".into());
            // A customises and syncs; B still has the defaults and turns sync on later
            a.sync(1_000);
            a.sync(1_500); // A has run twice before B joins
            b.sync(2_000);
            assert_eq!(b.settings.theme, crate::models::Theme::Dark, "B adopts A's settings on its first sync");
            assert_eq!(b.settings.launcher_hotkey, "Ctrl+Space", "hotkeys sync");
            assert_eq!(b.settings.capture.fps, 30);
            a.sync(3_000);
            assert_eq!(a.settings.launcher_hotkey, "Ctrl+Space", "A keeps its own settings");
            assert_eq!(a.settings.theme, crate::models::Theme::Dark);
            assert_eq!(b.settings.capture.folder, "/Users/b/Shots", "capture folders are per Mac");
            assert_eq!(b.settings.capture.recordings_folder, "/Users/b/Recs");
            // local-only values of B are untouched
            assert_eq!(b.settings.script_directories, vec!["/Users/b/scripts".to_string()]);
            assert_eq!(b.settings.capture.camera_id, "cam-b");
            assert_eq!(b.settings.capture.mic_id, "");
            assert_eq!(b.settings.notes_last.as_deref(), Some("note-b"));
            assert_eq!(b.settings.language, "system", "the interface language is per Mac");
            assert!(!b.settings.launch_at_login && !b.settings.onboarded && !b.settings.sync_enabled);
            // and none of them ever reached the shared folder
            let doc = std::fs::read_to_string(shared.join("devices").join(&a.io.device).join("settings.json")).unwrap();
            for leaked in [
                "/Users/a/scripts",
                "/Users/a/Shots",
                "/Users/a/Recs",
                "cam-a",
                "mic-a",
                "note-a",
                "launch_at_login",
                "sync_enabled",
                "onboarded",
            ] {
                assert!(!doc.contains(leaked), "{leaked} leaked into the sync folder");
            }
            for d in [da, db, shared] {
                std::fs::remove_dir_all(d).unwrap();
            }
        }
    }

    #[test]
    fn first_sync_never_beats_what_other_macs_published_but_keeps_new_items() {
        for (ida, idb) in ID_ORDERS {
            let (da, db, shared) = (tmp("a"), tmp("b"), tmp("shared"));
            let (mut a, mut b) = (Mac::new(&da, &shared), Mac::new(&db, &shared));
            a.io.device = ida.into();
            b.io.device = idb.into();
            a.snippets = vec![snip("s1", "from a")];
            a.sync(5_000);
            a.sync(6_000); // A has run twice before B joins
                           // B has a different version of s1 (older or newer by its own clock) and an item of its own
            let mut theirs = snip("s1", "b version");
            theirs.text = "b edit".into();
            b.snippets = vec![theirs, snip("s9", "only on b")];
            b.sync(9_000_000); // B's clock is far ahead: still a first sync, so A's record wins
            assert_eq!(b.snippets.iter().find(|s| s.id == "s1").unwrap().name, "from a");
            assert!(b.snippets.iter().any(|s| s.id == "s9"), "items nobody else has are kept");
            a.sync(10_000_000);
            assert!(a.snippets.iter().any(|s| s.id == "s9"), "and published");
            assert_eq!(a.snippets.iter().find(|s| s.id == "s1").unwrap().name, "from a");
            // a missing or corrupt tracker is a first sync again
            std::fs::write(db.join("sync-state").join("snippets.json"), "garbage").unwrap();
            a.snippets.iter_mut().find(|s| s.id == "s1").unwrap().name = "a later".into();
            a.sync(20_000_000);
            b.sync(30_000_000);
            assert_eq!(b.snippets.iter().find(|s| s.id == "s1").unwrap().name, "a later");
            for d in [da, db, shared] {
                std::fs::remove_dir_all(d).unwrap();
            }
        }
    }

    #[test]
    fn a_first_sync_before_icloud_lists_the_other_mac_still_adopts_its_settings() {
        for (ida, idb) in ID_ORDERS {
            let (da, db, shared) = (tmp("a"), tmp("b"), tmp("shared"));
            let (mut a, mut b) = (Mac::new(&da, &shared), Mac::new(&db, &shared));
            a.io.device = ida.into();
            b.io.device = idb.into();
            a.settings.launcher_hotkey = "Ctrl+Space".into();
            a.settings.theme = crate::models::Theme::Dark;
            a.sync(1_000);
            a.sync(1_500); // A has run twice before B joins
                           // iCloud has not listed A's folder on B yet
            let hidden = shared.join("hidden");
            std::fs::rename(shared.join("devices").join(ida), &hidden).unwrap();
            b.sync(2_000);
            std::fs::rename(&hidden, shared.join("devices").join(ida)).unwrap();
            b.sync(3_000);
            a.sync(4_000);
            for m in [&a, &b] {
                assert_eq!(m.settings.launcher_hotkey, "Ctrl+Space");
                assert_eq!(m.settings.theme, crate::models::Theme::Dark);
            }
            for d in [da, db, shared] {
                std::fs::remove_dir_all(d).unwrap();
            }
        }
    }

    #[test]
    fn a_later_mac_never_overwrites_an_older_macs_customised_settings_whatever_the_device_ids() {
        for (ida, idb) in ID_ORDERS {
            for idc in ["00", "mm", "zzz"] {
                let (da, db, dc, shared) = (tmp("a"), tmp("b"), tmp("c"), tmp("shared"));
                let (mut a, mut b, mut c) = (Mac::launch(&da, &shared), Mac::launch(&db, &shared), Mac::launch(&dc, &shared));
                (a.io.device, b.io.device, c.io.device) = (ida.into(), idb.into(), idc.into());
                a.settings.launcher_hotkey = "Ctrl+Space".into();
                a.settings.theme = crate::models::Theme::Dark;
                a.sync(1_000);
                a.sync(1_500);
                b.sync(2_000); // fresh Mac, defaults
                a.sync(3_000);
                b.sync(3_500);
                for m in [&a, &b] {
                    assert_eq!(m.settings.launcher_hotkey, "Ctrl+Space", "ids {ida}/{idb}");
                    assert_eq!(m.settings.theme, crate::models::Theme::Dark, "ids {ida}/{idb}");
                }
                // a third Mac with any id and a snippet of its own
                c.snippets.push(snip("only-c", "unique to c"));
                c.sync(4_000);
                a.sync(5_000);
                b.sync(5_500);
                c.sync(6_000);
                for m in [&a, &b, &c] {
                    assert_eq!(m.settings.launcher_hotkey, "Ctrl+Space", "ids {ida}/{idb}/{idc}");
                    assert_eq!(m.settings.theme, crate::models::Theme::Dark);
                    assert!(m.snippets.iter().any(|s| s.id == "only-c"), "C's own snippet is kept");
                }
                // B's genuine edit after joining wins
                b.settings.theme = crate::models::Theme::Light;
                b.sync(9_000);
                a.sync(9_500);
                c.sync(9_600);
                for m in [&a, &b, &c] {
                    assert_eq!(m.settings.theme, crate::models::Theme::Light);
                }
                for d in [da, db, dc, shared] {
                    std::fs::remove_dir_all(d).unwrap();
                }
            }
        }
    }

    #[test]
    fn an_edited_default_snippet_on_the_older_mac_survives_a_new_mac_joining() {
        for (ida, idb) in ID_ORDERS {
            let (da, db, shared) = (tmp("a"), tmp("b"), tmp("shared"));
            let (mut a, mut b) = (Mac::launch(&da, &shared), Mac::launch(&db, &shared));
            (a.io.device, b.io.device) = (ida.into(), idb.into());
            a.snippets.iter_mut().find(|s| s.id == "default-snippet-shrug").unwrap().text = "my shrug".into();
            a.sync(1_000);
            a.sync(1_500);
            b.sync(2_000);
            a.sync(3_000);
            b.sync(3_500);
            for m in [&a, &b] {
                assert_eq!(m.snippets.iter().find(|s| s.id == "default-snippet-shrug").unwrap().text, "my shrug", "ids {ida}/{idb}");
                assert_eq!(m.snippets.len(), 3);
            }
            for d in [da, db, shared] {
                std::fs::remove_dir_all(d).unwrap();
            }
        }
    }

    #[test]
    fn two_macs_that_first_sync_together_converge_on_one_value_and_real_edits_still_win() {
        for (ida, idb) in ID_ORDERS {
            let (da, db, shared) = (tmp("a"), tmp("b"), tmp("shared"));
            let (mut a, mut b) = (Mac::new(&da, &shared), Mac::new(&db, &shared));
            (a.io.device, b.io.device) = (ida.into(), idb.into());
            a.settings.launcher_hotkey = "Ctrl+A".into();
            b.settings.launcher_hotkey = "Ctrl+B".into();
            // neither has run twice before the other starts: a tie at 0
            a.sync(1_000);
            b.sync(1_100);
            a.sync(2_000);
            b.sync(2_100);
            for t in [3_000, 4_000, 5_000] {
                a.sync(t);
                b.sync(t + 50);
            }
            assert_eq!(a.settings.launcher_hotkey, b.settings.launcher_hotkey, "converged, ids {ida}/{idb}");
            let winner = if ida > idb { "Ctrl+A" } else { "Ctrl+B" };
            assert_eq!(a.settings.launcher_hotkey, winner, "the greater device id wins the tie");
            b.settings.launcher_hotkey = "Ctrl+Z".into();
            b.sync(9_000);
            a.sync(9_500);
            assert_eq!(a.settings.launcher_hotkey, "Ctrl+Z");
            for d in [da, db, shared] {
                std::fs::remove_dir_all(d).unwrap();
            }
        }
    }

    #[test]
    fn a_default_the_user_deleted_before_syncing_does_not_come_back_with_a_new_mac() {
        for (ida, idb) in ID_ORDERS {
            let (da, db, shared) = (tmp("a"), tmp("b"), tmp("shared"));
            let (mut a, mut b) = (Mac::launch(&da, &shared), Mac::launch(&db, &shared));
            (a.io.device, b.io.device) = (ida.into(), idb.into());
            a.snippets.retain(|s| s.id != "default-snippet-shrug");
            a.quicklinks.retain(|q| q.id != "default-quicklink-google");
            a.sync(1_000);
            a.sync(1_500);
            b.sync(2_000);
            a.sync(3_000);
            b.sync(3_500);
            for m in [&a, &b] {
                assert!(!m.snippets.iter().any(|s| s.id == "default-snippet-shrug"), "ids {ida}/{idb}");
                assert!(!m.quicklinks.iter().any(|q| q.id == "default-quicklink-google"));
                assert_eq!((m.snippets.len(), m.quicklinks.len()), (2, 5));
            }
            for d in [da, db, shared] {
                std::fs::remove_dir_all(d).unwrap();
            }
        }
    }

    #[test]
    fn a_customised_mac_joining_later_beats_a_fresh_mac_that_synced_first() {
        for (ida, idb) in ID_ORDERS {
            let (da, db, shared) = (tmp("a"), tmp("b"), tmp("shared"));
            let (mut a, mut b) = (Mac::launch(&da, &shared), Mac::launch(&db, &shared));
            (a.io.device, b.io.device) = (ida.into(), idb.into());
            // the old Mac is customised before it ever syncs
            a.settings.launcher_hotkey = "Ctrl+Space".into();
            a.settings.theme = crate::models::Theme::Dark;
            a.snippets.iter_mut().find(|s| s.id == "default-snippet-shrug").unwrap().text = "my shrug".into();
            a.quicklinks.retain(|q| q.id != "default-quicklink-google");
            // the new Mac, still pristine, turns sync on first and syncs twice
            b.sync(1_000);
            b.sync(1_500);
            a.sync(2_000);
            b.sync(3_000);
            a.sync(3_500);
            b.sync(4_000);
            for m in [&a, &b] {
                assert_eq!(m.settings.launcher_hotkey, "Ctrl+Space", "ids {ida}/{idb}");
                assert_eq!(m.settings.theme, crate::models::Theme::Dark, "ids {ida}/{idb}");
                assert_eq!(m.snippets.iter().find(|s| s.id == "default-snippet-shrug").unwrap().text, "my shrug", "ids {ida}/{idb}");
                assert!(!m.quicklinks.iter().any(|q| q.id == "default-quicklink-google"), "ids {ida}/{idb}");
            }
            // a genuine edit on B afterwards wins
            b.settings.theme = crate::models::Theme::Light;
            b.sync(9_000);
            a.sync(9_500);
            for m in [&a, &b] {
                assert_eq!(m.settings.theme, crate::models::Theme::Light, "ids {ida}/{idb}");
            }
            // setting a value back to its default on purpose is a real edit and wins as usual
            a.settings.launcher_hotkey = Settings::default().launcher_hotkey;
            a.sync(10_000);
            b.sync(10_500);
            a.sync(11_000);
            for m in [&a, &b] {
                assert_eq!(m.settings.launcher_hotkey, Settings::default().launcher_hotkey, "ids {ida}/{idb}");
            }
            for d in [da, db, shared] {
                std::fs::remove_dir_all(d).unwrap();
            }
        }
    }

    #[test]
    fn two_pristine_macs_still_end_up_the_same_and_a_default_edited_later_wins() {
        for (ida, idb) in ID_ORDERS {
            let (da, db, shared) = (tmp("a"), tmp("b"), tmp("shared"));
            let (mut a, mut b) = (Mac::launch(&da, &shared), Mac::launch(&db, &shared));
            (a.io.device, b.io.device) = (ida.into(), idb.into());
            a.sync(1_000);
            a.sync(1_500);
            b.sync(2_000);
            b.sync(2_500);
            b.snippets.iter_mut().find(|s| s.id == "default-snippet-today").unwrap().text = "edited on b".into();
            b.sync(3_000);
            a.sync(3_500);
            for m in [&a, &b] {
                assert_eq!(m.snippets.iter().find(|s| s.id == "default-snippet-today").unwrap().text, "edited on b", "ids {ida}/{idb}");
                assert_eq!((m.snippets.len(), m.quicklinks.len()), (3, 6));
            }
            for d in [da, db, shared] {
                std::fs::remove_dir_all(d).unwrap();
            }
        }
    }

    /// All orders in which three Macs can join.
    const JOIN_ORDERS: [[usize; 3]; 6] = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];

    #[test]
    fn a_late_joiner_does_not_claim_another_macs_old_baseline_at_its_own_time() {
        for ids in [["aa", "mm", "zz"], ["zz", "mm", "aa"], ["mm", "zz", "aa"], ["mm", "aa", "zz"], ["aa", "zz", "mm"], ["zz", "aa", "mm"]]
        {
            for hide in [true, false] {
                let (da, db, dc, shared) = (tmp("a"), tmp("b"), tmp("c"), tmp("shared"));
                let (mut a, mut b, mut c) = (Mac::new(&da, &shared), Mac::new(&db, &shared), Mac::new(&dc, &shared));
                (a.io.device, b.io.device, c.io.device) = (ids[0].into(), ids[1].into(), ids[2].into());
                a.snippets = vec![snip("x", "A text")];
                a.sync(1_000); // A syncs once and sleeps
                c.sync(2_000); // C adopts x
                c.sync(2_500);
                c.snippets.iter_mut().find(|s| s.id == "x").unwrap().text = "C edit".into();
                c.sync(2 * DAY);
                let away = shared.join("away");
                if hide {
                    std::fs::rename(shared.join("devices").join(ids[2]), &away).unwrap();
                    // C's document is not visible yet
                }
                b.sync(3 * DAY); // B joins on day 3
                b.sync(3 * DAY + 500);
                if hide {
                    std::fs::rename(&away, shared.join("devices").join(ids[2])).unwrap();
                }
                for round in 0..3 {
                    let t = 4 * DAY + round * 1_000;
                    a.sync(t);
                    b.sync(t + 100);
                    c.sync(t + 200);
                }
                for m in [&a, &b, &c] {
                    assert_eq!(m.snippets.iter().find(|s| s.id == "x").unwrap().text, "C edit", "ids {ids:?} hidden {hide}");
                }
                for d in [da, db, dc, shared] {
                    std::fs::remove_dir_all(d).unwrap();
                }
            }
        }
    }

    #[test]
    fn a_lost_or_corrupt_tracker_keeps_this_macs_unsynced_edits_and_deletions() {
        for (ida, idb) in ID_ORDERS {
            for corrupt in [false, true] {
                let (da, db, shared) = (tmp("a"), tmp("b"), tmp("shared"));
                let (mut a, mut b) = (Mac::new(&da, &shared), Mac::new(&db, &shared));
                (a.io.device, b.io.device) = (ida.into(), idb.into());
                a.snippets = vec![snip("s1", "one"), snip("s2", "two"), snip("s3", "three")];
                a.sync(1_000);
                a.sync(1_500);
                b.sync(2_000);
                b.sync(2_500);
                // edits and a deletion made on A but not synced yet, then the tracker is lost
                a.snippets.iter_mut().find(|s| s.id == "s1").unwrap().text = "A edit".into();
                a.snippets.retain(|s| s.id != "s2");
                let tracker = da.join("sync-state").join("snippets.json");
                if corrupt {
                    std::fs::write(&tracker, "{garbage").unwrap();
                } else {
                    std::fs::remove_file(&tracker).unwrap();
                }
                a.sync(3_000);
                b.sync(3_500);
                a.sync(4_000);
                for m in [&a, &b] {
                    assert_eq!(m.snippets.iter().find(|s| s.id == "s1").unwrap().text, "A edit", "ids {ida}/{idb} corrupt {corrupt}");
                    assert!(!m.snippets.iter().any(|s| s.id == "s2"), "the deletion survives, ids {ida}/{idb}");
                    assert_eq!(m.snippets.len(), 2);
                }
                for d in [da, db, shared] {
                    std::fs::remove_dir_all(d).unwrap();
                }
            }
        }
    }

    #[test]
    fn themes_sync_per_theme_with_renames_edits_and_deletions_in_both_id_orders() {
        use crate::themes::{CustomTheme, ThemeBase};
        let th = |id: &str, name: &str| CustomTheme {
            id: id.into(),
            name: name.into(),
            base: ThemeBase::Dark,
            accent: "#38BDF8".into(),
            background: "#0B1220".into(),
            surface: "#16213A".into(),
            text: "#E6EEFF".into(),
            secondary_text: "#9FB0D0".into(),
            separator: "#26324D".into(),
        };
        let run = |m: &Mac, list: &mut Vec<CustomTheme>, now: i64| {
            let d = &m.store.root;
            let o = sync_domain(&m.io, "themes", &to_items(list), &load_tracker(d, "themes"), now).unwrap();
            save_tracker(d, "themes", &o.tracker_merged).unwrap();
            if let Some(merged) = o.merged {
                *list = from_items(merged);
                m.io.write_doc("themes", o.applied_doc.as_ref().unwrap()).unwrap();
            }
        };
        for (ida, idb) in ID_ORDERS {
            let (da, db, shared) = (tmp("a"), tmp("b"), tmp("shared"));
            let (mut a, mut b) = (Mac::new(&da, &shared), Mac::new(&db, &shared));
            (a.io.device, b.io.device) = (ida.into(), idb.into());
            let (mut ta, mut tb) = (vec![th("t1", "Ocean"), th("t2", "Sunset")], vec![th("t3", "Mint")]);
            for now in [1_000, 1_500] {
                run(&a, &mut ta, now);
            }
            for now in [2_000, 2_500] {
                run(&b, &mut tb, now);
            }
            run(&a, &mut ta, 2_700);
            let names = |l: &Vec<CustomTheme>| {
                let mut n: Vec<_> = l.iter().map(|t| t.name.clone()).collect();
                n.sort();
                n
            };
            assert_eq!(names(&ta), ["Mint", "Ocean", "Sunset"], "ids {ida}/{idb}");
            assert_eq!(names(&tb), ["Mint", "Ocean", "Sunset"], "ids {ida}/{idb}");
            // B renames t1, A deletes t2
            tb.iter_mut().find(|t| t.id == "t1").unwrap().name = "Sea".into();
            run(&b, &mut tb, 3_000);
            ta.retain(|t| t.id != "t2");
            run(&a, &mut ta, 3_100);
            run(&a, &mut ta, 4_000);
            run(&b, &mut tb, 4_500);
            run(&a, &mut ta, 5_000);
            assert_eq!(names(&ta), ["Mint", "Sea"], "ids {ida}/{idb}");
            assert_eq!(names(&tb), ["Mint", "Sea"], "ids {ida}/{idb}");
            for d in [da, db, shared] {
                std::fs::remove_dir_all(d).unwrap();
            }
        }
    }

    #[test]
    fn a_corrupt_local_file_does_not_delete_the_items_on_the_other_macs_and_is_restored_from_them() {
        for (ida, idb) in ID_ORDERS {
            let (da, db, shared) = (tmp("a"), tmp("b"), tmp("shared"));
            let (mut a, mut b) = (Mac::new(&da, &shared), Mac::new(&db, &shared));
            (a.io.device, b.io.device) = (ida.into(), idb.into());
            a.snippets = vec![snip("s1", "one"), snip("s2", "two"), snip("s3", "three")];
            a.sync(1_000);
            a.sync(1_500);
            b.sync(2_000);
            b.sync(2_500);
            assert!(!needs_recovery(&da, "snippets"));
            // A's file becomes unreadable: the next launch sets it aside and starts from the defaults
            std::fs::write(da.join("snippets.json"), "{garbage").unwrap();
            a.restart();
            assert!(da.join("snippets.json.corrupt").exists());
            assert!(needs_recovery(&da, "snippets"), "ids {ida}/{idb}");
            a.sync(3_000);
            assert!(!needs_recovery(&da, "snippets"), "one run clears it");
            b.sync(3_500);
            a.sync(4_000);
            for m in [&a, &b] {
                let mut n: Vec<_> = m.snippets.iter().map(|s| s.id.as_str()).filter(|i| matches!(*i, "s1" | "s2" | "s3")).collect();
                n.sort();
                assert_eq!(n, ["s1", "s2", "s3"], "nothing was deleted, ids {ida}/{idb}");
            }
            for d in [da, db, shared] {
                std::fs::remove_dir_all(d).unwrap();
            }
        }
    }

    /// A Mac whose snippets file became unreadable after it synced `s1..s3` (and, for `peer`, a
    /// second Mac that has or has not synced since).
    fn corrupt_a_after_syncing(ida: &str, idb: &str, b_syncs_first: bool) -> (PathBuf, PathBuf, PathBuf, Mac, Mac) {
        let (da, db, shared) = (tmp("a"), tmp("b"), tmp("shared"));
        let (mut a, mut b) = (Mac::new(&da, &shared), Mac::new(&db, &shared));
        (a.io.device, b.io.device) = (ida.into(), idb.into());
        a.snippets = vec![snip("s1", "one"), snip("s2", "two"), snip("s3", "three")];
        a.sync(1_000);
        a.sync(1_500);
        if b_syncs_first {
            b.sync(2_000);
            b.sync(2_500);
        }
        std::fs::write(da.join("snippets.json"), "{garbage").unwrap();
        a.restart();
        assert!(needs_recovery(&da, "snippets"));
        (da, db, shared, a, b)
    }

    fn custom_ids(m: &Mac) -> Vec<&str> {
        let mut n: Vec<_> = m.snippets.iter().map(|s| s.id.as_str()).filter(|i| matches!(*i, "s1" | "s2" | "s3")).collect();
        n.sort();
        n
    }

    #[test]
    fn items_only_this_mac_had_survive_a_recovery_that_no_other_mac_has_seen() {
        for (ida, idb) in ID_ORDERS {
            let (da, db, shared, mut a, mut b) = corrupt_a_after_syncing(ida, idb, false);
            a.sync(3_000);
            assert_eq!(custom_ids(&a), ["s1", "s2", "s3"], "restored from its own document, ids {ida}/{idb}");
            b.sync(4_000);
            b.sync(4_500);
            a.sync(5_000);
            for m in [&a, &b] {
                assert_eq!(custom_ids(m), ["s1", "s2", "s3"], "ids {ida}/{idb}");
            }
            for d in [da, db, shared] {
                std::fs::remove_dir_all(d).unwrap();
            }
        }
    }

    #[test]
    fn a_single_mac_gets_everything_back_from_its_own_document_after_recovery() {
        for (ida, idb) in ID_ORDERS {
            let (da, db, shared, mut a, _b) = corrupt_a_after_syncing(ida, idb, false);
            a.snippets.iter().for_each(|s| assert!(!matches!(s.id.as_str(), "s1" | "s2" | "s3")));
            a.sync(3_000);
            a.sync(3_500);
            assert_eq!(custom_ids(&a), ["s1", "s2", "s3"], "ids {ida}/{idb}");
            assert!(!needs_recovery(&da, "snippets"));
            // a recovery whose merge was not applied keeps the marker and the own document intact
            std::fs::write(da.join("snippets.json"), "{garbage2").unwrap();
            a.restart();
            let o = sync_domain_recovering(&a.io, "snippets", &to_items(&a.snippets), &load_tracker(&da, "snippets"), 4_000, true).unwrap();
            assert!(o.merged.is_some() && o.applied_doc.is_some());
            let own = a.io.read_own_doc("snippets").unwrap();
            assert!(own.items.contains_key("s1") && !own.items["s1"].deleted, "own document untouched until the merge is applied");
            for d in [da, db, shared] {
                std::fs::remove_dir_all(d).unwrap();
            }
        }
    }

    #[test]
    fn a_deletion_made_here_while_the_other_mac_was_offline_survives_recovery() {
        for (ida, idb) in ID_ORDERS {
            let (da, db, shared) = (tmp("a"), tmp("b"), tmp("shared"));
            let (mut a, mut b) = (Mac::new(&da, &shared), Mac::new(&db, &shared));
            (a.io.device, b.io.device) = (ida.into(), idb.into());
            a.snippets = vec![snip("s1", "one"), snip("s2", "two"), snip("s3", "three")];
            a.sync(1_000);
            a.sync(1_500);
            b.sync(2_000);
            b.sync(2_500);
            // A deletes s2 while B is offline, then A's file is corrupted and recovered
            a.snippets.retain(|s| s.id != "s2");
            a.sync(3_000);
            std::fs::write(da.join("snippets.json"), "{garbage").unwrap();
            a.restart();
            a.sync(4_000);
            a.sync(4_500);
            b.sync(5_000);
            a.sync(5_500);
            b.sync(6_000);
            for m in [&a, &b] {
                assert_eq!(custom_ids(m), ["s1", "s3"], "s2 stays deleted, ids {ida}/{idb}");
            }
            for d in [da, db, shared] {
                std::fs::remove_dir_all(d).unwrap();
            }
        }
    }

    #[test]
    fn a_clipboard_deletion_made_here_survives_recovery_too() {
        let entry = |id: &str, at: i64| ClipEntry {
            id: id.into(),
            kind: ClipKind::Text,
            content: id.into(),
            title: None,
            copied_at: at,
            pinned: false,
            source_app: None,
            times_copied: 1,
            width: 0,
            height: 0,
        };
        for (ida, idb) in ID_ORDERS {
            let (da, db, shared) = (tmp("a"), tmp("b"), tmp("shared"));
            let (mut a, mut b) = (Mac::new(&da, &shared), Mac::new(&db, &shared));
            (a.io.device, b.io.device) = (ida.into(), idb.into());
            let now = 1_000_000_i64;
            a.clipboard = vec![entry("c1", now / 1000 - 30), entry("c2", now / 1000 - 20)];
            a.sync_clipboard(now);
            a.sync_clipboard(now + 500);
            b.sync_clipboard(now + 1_000);
            b.sync_clipboard(now + 1_500);
            a.clipboard.retain(|c| c.id != "c2");
            a.sync_clipboard(now + 2_000);
            std::fs::write(da.join("clipboard.json"), "{garbage").unwrap();
            a.store.load_all();
            a.clipboard = vec![];
            assert!(needs_recovery(&da, "clipboard"));
            for step in 3..8 {
                let recover = needs_recovery(&da, "clipboard");
                let d = da.clone();
                let o = sync_domain_recovering(
                    &a.io,
                    "clipboard",
                    &to_items(&a.clipboard),
                    &load_tracker(&d, "clipboard"),
                    now + step * 1_000,
                    recover,
                )
                .unwrap();
                save_tracker(&d, "clipboard", &o.tracker_merged).unwrap();
                if recover {
                    clear_recovery(&d, "clipboard");
                }
                if let Some(m) = o.merged {
                    a.clipboard = from_items(m);
                    if let Some(doc) = &o.applied_doc {
                        a.io.write_doc("clipboard", doc).unwrap();
                    }
                }
                b.sync_clipboard(now + step * 1_000 + 500);
            }
            for m in [&a, &b] {
                let mut ids: Vec<_> = m.clipboard.iter().map(|c| c.id.as_str()).collect();
                ids.sort();
                assert_eq!(ids, ["c1"], "c2 stays deleted, ids {ida}/{idb}");
            }
            for d in [da, db, shared] {
                std::fs::remove_dir_all(d).unwrap();
            }
        }
    }

    #[test]
    fn a_stale_corrupt_file_without_a_marker_does_not_trigger_recovery() {
        let d = tmp("stale");
        std::fs::write(d.join("snippets.json.corrupt"), "old").unwrap();
        assert!(!needs_recovery(&d, "snippets"), "a copied data folder with an old .corrupt file");
        mark_recovery(&d, "snippets");
        assert!(needs_recovery(&d, "snippets"));
        clear_recovery(&d, "snippets");
        assert!(!needs_recovery(&d, "snippets"));
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn a_merge_that_was_offered_but_not_applied_is_not_written_to_the_own_document_so_a_lost_tracker_keeps_the_remote_edit() {
        for (ida, idb) in ID_ORDERS {
            let (da, db, shared) = (tmp("a"), tmp("b"), tmp("shared"));
            let (mut a, mut b) = (Mac::new(&da, &shared), Mac::new(&db, &shared));
            (a.io.device, b.io.device) = (ida.into(), idb.into());
            a.snippets = vec![snip("s1", "one")];
            a.sync(1_000);
            a.sync(1_500);
            b.sync(2_000);
            b.sync(2_500);
            // B edits; A's run is offered the merge but the app does not apply it (typing, local change)
            b.snippets[0].text = "B edit".into();
            b.sync(3_000);
            let d = &a.store.root;
            let o = sync_domain(&a.io, "snippets", &to_items(&a.snippets), &load_tracker(d, "snippets"), 3_500).unwrap();
            assert!(o.merged.is_some(), "the remote edit is offered");
            save_tracker(d, "snippets", &o.tracker_published).unwrap();
            let own = a.io.read_own_doc("snippets").unwrap();
            assert_eq!(
                own.items["s1"].value.as_ref().unwrap()["text"],
                "text of one",
                "own document still holds what this Mac has, ids {ida}/{idb}"
            );
            // the tracker is lost right after; the older local value must not become a new edit
            std::fs::remove_file(da.join("sync-state").join("snippets.json")).unwrap();
            a.sync(5_000);
            b.sync(5_500);
            a.sync(6_000);
            for m in [&a, &b] {
                assert_eq!(m.snippets[0].text, "B edit", "ids {ida}/{idb}");
            }
            for d in [da, db, shared] {
                std::fs::remove_dir_all(d).unwrap();
            }
        }
    }

    #[test]
    fn a_later_joining_customised_mac_keeps_its_settings_when_the_first_mac_changed_one_setting() {
        for (ida, idb) in ID_ORDERS {
            let (da, db, shared) = (tmp("a"), tmp("b"), tmp("shared"));
            let (mut a, mut b) = (Mac::launch(&da, &shared), Mac::launch(&db, &shared));
            (a.io.device, b.io.device) = (ida.into(), idb.into());
            a.settings.launcher_hotkey = "Ctrl+Space".into();
            a.settings.theme = crate::models::Theme::Dark;
            b.sync(1_000);
            b.sync(1_500);
            b.settings.capture.fps = 30;
            b.sync(3_000);
            a.sync(4_000);
            b.sync(5_000);
            a.sync(6_000);
            for m in [&a, &b] {
                assert_eq!(m.settings.launcher_hotkey, "Ctrl+Space", "ids {ida}/{idb}");
                assert_eq!(m.settings.theme, crate::models::Theme::Dark, "ids {ida}/{idb}");
                assert_eq!(m.settings.capture.fps, 30, "B's later edit of another setting is kept, ids {ida}/{idb}");
            }
            for d in [da, db, shared] {
                std::fs::remove_dir_all(d).unwrap();
            }
        }
    }

    #[test]
    fn two_customised_macs_and_a_fresh_one_keep_every_customisation_in_every_join_order() {
        for ids in [["aa", "mm", "zz"], ["zz", "mm", "aa"]] {
            for order in JOIN_ORDERS {
                let dirs = [tmp("a"), tmp("b"), tmp("c"), tmp("shared")];
                let shared = dirs[3].clone();
                let mut macs: Vec<Mac> = (0..3).map(|i| Mac::launch(&dirs[i], &shared)).collect();
                for (i, m) in macs.iter_mut().enumerate() {
                    m.io.device = ids[i].into();
                }
                macs[0].settings.theme = crate::models::Theme::Dark; // A
                macs[1].settings.launcher_hotkey = "Ctrl+Space".into(); // B; C stays fresh
                for (step, &i) in order.iter().enumerate() {
                    macs[i].sync(1_000 + step as i64 * 1_000);
                    macs[i].sync(1_500 + step as i64 * 1_000);
                }
                for round in 0..3 {
                    for (i, m) in macs.iter_mut().enumerate() {
                        m.sync(10_000 + round * 1_000 + i as i64);
                    }
                }
                for m in &macs {
                    assert_eq!(m.settings.theme, crate::models::Theme::Dark, "ids {ids:?} order {order:?}");
                    assert_eq!(m.settings.launcher_hotkey, "Ctrl+Space", "ids {ids:?} order {order:?}");
                }
                for d in dirs {
                    std::fs::remove_dir_all(d).unwrap();
                }
            }
        }
    }

    /// The settings item of a version that synced all settings as one item.
    fn legacy_settings_value(s: &Settings) -> Value {
        let mut v = serde_json::to_value(s).unwrap();
        let o = v.as_object_mut().unwrap();
        for k in LOCAL_ONLY {
            o.remove(k);
        }
        for k in LOCAL_ONLY_CAPTURE {
            o.get_mut("capture").unwrap().as_object_mut().unwrap().remove(k);
        }
        v
    }

    #[test]
    fn a_document_with_the_old_single_settings_item_is_read_field_by_field_and_old_trackers_do_not_claim_now() {
        for (ida, idb) in ID_ORDERS {
            let (da, db, shared) = (tmp("a"), tmp("b"), tmp("shared"));
            let (mut a, mut b) = (Mac::new(&da, &shared), Mac::new(&db, &shared));
            (a.io.device, b.io.device) = (ida.into(), idb.into());
            // B wrote its document with the old version: one item, customised hotkey, at time 2000
            let theirs = Settings { launcher_hotkey: "Ctrl+B".into(), ..Settings::default() };
            let old =
                format!(r#"{{"device":"{idb}","items":{{"settings":{{"modified":2000,"value":{}}}}}}}"#, legacy_settings_value(&theirs));
            std::fs::create_dir_all(shared.join("devices").join(idb)).unwrap();
            std::fs::write(shared.join("devices").join(idb).join("settings.json"), old).unwrap();
            // A, upgraded, has its own old document and tracker (one item, time 1000) and edited nothing since
            let mine = Settings::default();
            let own_doc =
                format!(r#"{{"device":"{ida}","items":{{"settings":{{"modified":1000,"value":{}}}}}}}"#, legacy_settings_value(&mine));
            std::fs::create_dir_all(shared.join("devices").join(ida)).unwrap();
            std::fs::write(shared.join("devices").join(ida).join("settings.json"), own_doc).unwrap();
            std::fs::create_dir_all(da.join("sync-state")).unwrap();
            std::fs::write(
                da.join("sync-state").join("settings.json"),
                r#"{"items":{"settings":{"hash":"old","modified":1000}},"seeded":true}"#,
            )
            .unwrap();
            a.sync(3_000);
            assert_eq!(
                a.settings.launcher_hotkey, "Ctrl+B",
                "B's newer value is adopted, A's migrated fields do not claim now; ids {ida}/{idb}"
            );
            let doc = std::fs::read_to_string(shared.join("devices").join(ida).join("settings.json")).unwrap();
            assert!(doc.contains("launcher_hotkey") && !doc.contains("\"settings\""), "published field by field: {doc}");
            for d in [da, db, shared] {
                std::fs::remove_dir_all(d).unwrap();
            }
        }
    }

    #[test]
    fn this_macs_settings_file_is_copied_once_before_the_first_merge() {
        let dir = tmp("backup");
        std::fs::write(dir.join("settings.json"), r#"{"theme":"Dark"}"#).unwrap();
        backup_settings_once(&dir);
        std::fs::write(dir.join("settings.json"), r#"{"theme":"Light"}"#).unwrap();
        backup_settings_once(&dir);
        assert_eq!(std::fs::read_to_string(dir.join("settings.before-sync.json")).unwrap(), r#"{"theme":"Dark"}"#);
        // nothing to copy: nothing is made up
        let empty = tmp("nobackup");
        backup_settings_once(&empty);
        assert!(!empty.join("settings.before-sync.json").exists());
        for d in [dir, empty] {
            std::fs::remove_dir_all(d).unwrap();
        }
    }

    #[test]
    fn two_fresh_installs_end_with_three_snippets_and_six_quicklinks() {
        let (da, db, shared) = (tmp("a"), tmp("b"), tmp("shared"));
        let (mut a, mut b) = (Mac::launch(&da, &shared), Mac::launch(&db, &shared));
        assert_eq!((a.snippets.len(), a.quicklinks.len()), (3, 6));
        a.sync(1_000);
        b.sync(2_000);
        a.sync(3_000);
        for m in [&a, &b] {
            assert_eq!((m.snippets.len(), m.quicklinks.len()), (3, 6));
        }
        for d in [da, db, shared] {
            std::fs::remove_dir_all(d).unwrap();
        }
    }

    #[test]
    fn default_items_keep_their_ids_across_launches() {
        let dir = tmp("ids");
        let store = Store::new(&dir);
        let (one, two) = (store.load_all(), store.load_all());
        assert_eq!(one.snippets.iter().map(|s| &s.id).collect::<Vec<_>>(), two.snippets.iter().map(|s| &s.id).collect::<Vec<_>>());
        assert_eq!(one.quicklinks.iter().map(|s| &s.id).collect::<Vec<_>>(), two.quicklinks.iter().map(|s| &s.id).collect::<Vec<_>>());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn old_files_without_ids_get_ids_once_and_a_restart_keeps_the_other_macs_edit() {
        let (da, db, shared) = (tmp("a"), tmp("b"), tmp("shared"));
        let mut b = Mac::new(&db, &shared);
        std::fs::create_dir_all(&db).unwrap();
        std::fs::write(b.store.path("snippets.json"), r#"[{"name":"Old","text":"t"}]"#).unwrap();
        std::fs::write(b.store.path("quicklinks.json"), r#"[{"name":"q","link":"https://x"}]"#).unwrap();
        std::fs::write(b.store.path("notes.json"), r#"[{"text":"n","created":1,"modified":2}]"#).unwrap();
        b.restart();
        let id = b.snippets[0].id.clone();
        b.restart();
        assert_eq!(b.snippets[0].id, id, "ids are saved the first time the file is loaded");
        assert_eq!(b.store.load_all().notes[0].id, b.store.load_all().notes[0].id);
        b.sync(1_000);
        let mut a = Mac::new(&da, &shared);
        a.sync(2_000);
        a.snippets.iter_mut().find(|s| s.id == id).unwrap().text = "A edit".into();
        a.sync(3_000);
        b.restart(); // relaunch before pulling
        b.sync(4_000);
        assert_eq!(b.snippets.len(), 1);
        assert_eq!(b.snippets[0].text, "A edit");
        for d in [da, db, shared] {
            std::fs::remove_dir_all(d).unwrap();
        }
    }

    const DAY: i64 = 24 * 3600 * 1000;

    #[test]
    fn an_item_deleted_on_any_mac_does_not_come_back_from_a_stale_device_folder_after_92_days() {
        let (da, db, dc, shared) = (tmp("a"), tmp("b"), tmp("c"), tmp("shared"));
        let (mut a, mut b, mut c) = (Mac::new(&da, &shared), Mac::new(&db, &shared), Mac::new(&dc, &shared));
        a.snippets = vec![snip("s1", "one"), snip("s2", "two")];
        a.sync(1_000);
        c.sync(1_500); // C holds s1 and then is never opened again
        b.sync(2_000);
        b.snippets.retain(|s| s.id != "s1");
        b.sync(3_000);
        a.sync(4_000);
        assert_eq!(a.snippets.len(), 1);
        for t in [92 * DAY, 200 * DAY, 400 * DAY] {
            b.sync(t);
            a.sync(t + 1);
            b.sync(t + 2);
            for m in [&a, &b] {
                assert_eq!(m.snippets.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["s2"], "day {}", t / DAY);
            }
        }
        // a fresh Mac that meets the stale folder after a year does not get it either
        let (dd, mut d) = (tmp("d"), None);
        d.get_or_insert_with(|| Mac::new(&dd, &shared)).sync(401 * DAY);
        assert_eq!(d.unwrap().snippets.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["s2"]);
        for p in [da, db, dc, dd, shared] {
            std::fs::remove_dir_all(p).unwrap();
        }
    }

    #[test]
    fn clipboard_tombstones_outlive_the_longest_retention_and_older_remote_entries_are_ignored() {
        let (da, shared) = (tmp("a"), tmp("shared"));
        let a = Mac::new(&da, &shared);
        let other = shared.join("devices").join("other");
        std::fs::create_dir_all(&other).unwrap();
        let old = image_entry("old", "x.png", 5);
        let mut doc = DeviceDoc { device: "other".into(), items: Default::default(), ..Default::default() };
        doc.items.insert(
            "old".into(),
            sync::Record {
                modified: 10,
                deleted: false,
                value: Some(serde_json::to_value(&old).unwrap()),
                pristine: false,
                adopted: false,
            },
        );
        std::fs::write(other.join("clipboard.json"), serde_json::to_vec(&doc).unwrap()).unwrap();
        let o = sync_domain(&a.io, "clipboard", &[], &Tracker::default(), 500 * DAY).unwrap();
        assert!(o.merged.is_none(), "an entry last edited before the horizon is not adopted");
        // a deletion made 300 days ago is still published (retention can be a year)
        let (_, t) = sync::publish_ttl(&a.io.device, &[("e".into(), json!(1))], &Tracker::default(), 0, 0, None);
        let o = sync_domain(&a.io, "clipboard", &[], &t, 300 * DAY).unwrap();
        let mine: DeviceDoc =
            serde_json::from_slice(&std::fs::read(shared.join("devices").join(&a.io.device).join("clipboard.json")).unwrap()).unwrap();
        assert!(mine.items["e"].deleted);
        assert!(o.merged.is_some(), "an entry 300 days old is still inside the horizon");
        for p in [da, shared] {
            std::fs::remove_dir_all(p).unwrap();
        }
    }

    #[test]
    fn clipboard_documents_stay_under_a_megabyte_over_400_days_of_heavy_copying() {
        let (da, db, shared) = (tmp("a"), tmp("b"), tmp("shared"));
        let (mut a, mut b) = (Mac::new(&da, &shared), Mac::new(&db, &shared));
        let text = |id: String, at: i64| ClipEntry {
            id: id.clone(),
            kind: ClipKind::Text,
            content: format!("copied text {id}"),
            title: None,
            copied_at: at,
            pinned: false,
            source_app: None,
            times_copied: 1,
            width: 0,
            height: 0,
        };
        let mut n = 0;
        for day in 0..400i64 {
            for m in [&mut a, &mut b] {
                for k in 0..100 {
                    n += 1;
                    m.clipboard.insert(0, text(format!("{:08}-{n}", day), day * 86_400 + k));
                }
                m.clipboard = clipboard_normalize(std::mem::take(&mut m.clipboard), day * 86_400 + 99, Some(30 * 86_400));
            }
            if day % 20 == 19 {
                let now = (day * 86_400 + 100) * 1000;
                a.sync_clipboard(now);
                b.sync_clipboard(now + 1);
            }
        }
        let size = |m: &Mac| std::fs::metadata(shared.join("devices").join(&m.io.device).join("clipboard.json")).unwrap().len();
        eprintln!("clipboard document sizes after 400 days: a = {} bytes, b = {} bytes", size(&a), size(&b));
        assert!(size(&a) < 1_000_000 && size(&b) < 1_000_000, "a = {}, b = {}", size(&a), size(&b));
        for d in [da, db, shared] {
            std::fs::remove_dir_all(d).unwrap();
        }
    }

    #[test]
    fn built_in_defaults_saved_with_random_ids_are_renamed_so_a_fresh_mac_adds_no_duplicates() {
        let (da, db, shared) = (tmp("a"), tmp("b"), tmp("shared"));
        // an existing user: defaults saved by an older version with random ids, plus own items
        let store = Store::new(&da);
        let mut snippets: Vec<Snippet> = crate::storage::default_snippets();
        let mut quicklinks: Vec<Quicklink> = crate::storage::default_quicklinks();
        for s in snippets.iter_mut() {
            s.id = uuid::Uuid::new_v4().to_string();
        }
        for q in quicklinks.iter_mut() {
            q.id = uuid::Uuid::new_v4().to_string();
        }
        snippets.push(snip("mine", "my own"));
        store.save("snippets.json", &snippets).unwrap();
        store.save("quicklinks.json", &quicklinks).unwrap();
        let (mut a, mut b) = (Mac::launch(&da, &shared), Mac::launch(&db, &shared));
        a.sync(1_000);
        b.sync(2_000);
        a.sync(3_000);
        for m in [&a, &b] {
            assert_eq!((m.snippets.len(), m.quicklinks.len()), (4, 6), "3 defaults + 1 own, 6 quicklinks");
        }
        // the renaming was saved: the next launch keeps the ids
        let again = Store::new(&da).load_all();
        assert!(again.snippets.iter().any(|s| s.id == "default-snippet-shrug"));
        // an edited default is not matched (it is the user's own item now)
        let mut edited = crate::storage::default_snippets();
        edited[0].id = "random-edited".into();
        edited[0].text = "changed".into();
        store.save("snippets.json", &edited).unwrap();
        assert!(Store::new(&da).load_all().snippets.iter().any(|s| s.id == "random-edited"));
        for d in [da, db, shared] {
            std::fs::remove_dir_all(d).unwrap();
        }
    }

    #[test]
    fn external_tools_that_hang_are_killed_after_the_timeout() {
        let t = std::time::Instant::now();
        let mut cmd = std::process::Command::new("/bin/sleep");
        cmd.arg("30");
        assert!(run_with_timeout(&mut cmd, std::time::Duration::from_millis(300)).is_none());
        assert!(t.elapsed() < std::time::Duration::from_secs(5));
        let mut echo = std::process::Command::new("/bin/echo");
        echo.arg("hello");
        assert_eq!(run_with_timeout(&mut echo, std::time::Duration::from_secs(5)).unwrap().trim(), "hello");
    }

    #[test]
    fn raycast_extension_settings_are_device_local() {
        let mut s = Settings::default();
        let before = settings_items(&s);
        s.run_raycast_extensions = true;
        s.raycast_extensions_disabled.push("json-format".into());
        assert_eq!(settings_items(&s), before, "Node and the installed extensions are per Mac");
    }

    #[test]
    fn changing_a_local_only_field_is_not_an_edit() {
        let mut s = Settings::default();
        let before = settings_items(&s);
        s.script_directories.push("/x".into());
        s.capture.mic_id = "m".into();
        s.notes_zoom = 2.0;
        s.language = "ja".into();
        assert_eq!(settings_items(&s), before);
        s.theme = crate::models::Theme::Dark;
        assert_ne!(settings_items(&s), before);
    }

    #[test]
    fn old_format_data_files_still_load_and_ids_are_stable() {
        let dir = tmp("old");
        let store = Store::new(&dir);
        // a file written before sync existed: no id, no new fields
        std::fs::write(store.path("snippets.json"), r#"[{"name":"Old","text":"t"}]"#).unwrap();
        std::fs::write(store.path("settings.json"), r#"{"launcher_hotkey":"Alt+Space","theme":"Dark"}"#).unwrap();
        let d = store.load_all();
        assert_eq!(d.snippets.len(), 1);
        assert!(!d.settings.sync_enabled, "sync is off for an old settings file");
        assert_eq!(d.settings.launcher_hotkey, "Alt+Space");
        // a defaulted id is made once and written back by the next save, then stays
        let id = d.snippets[0].id.clone();
        store.save("snippets.json", &d.snippets).unwrap();
        let again: Vec<Snippet> = store.load("snippets.json");
        assert_eq!(again[0].id, id);
        // quicklinks/notes keep loading as before
        std::fs::write(store.path("quicklinks.json"), r#"[{"name":"q","link":"https://x"}]"#).unwrap();
        assert_eq!(store.load::<Vec<Quicklink>>("quicklinks.json").len(), 1);
        std::fs::write(store.path("notes.json"), r#"[{"text":"n","created":1,"modified":2}]"#).unwrap();
        assert_eq!(store.load::<Vec<Note>>("notes.json").len(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn corrupt_and_unreadable_documents_are_ignored() {
        let (da, shared) = (tmp("a"), tmp("shared"));
        let mut a = Mac::new(&da, &shared);
        let bad = shared.join("devices").join("zzz");
        std::fs::create_dir_all(&bad).unwrap();
        std::fs::write(bad.join("snippets.json"), "{not json").unwrap();
        std::fs::create_dir_all(shared.join("devices").join("empty")).unwrap();
        a.snippets = vec![snip("s1", "one")];
        let o = sync_domain(&a.io, "snippets", &to_items(&a.snippets), &Tracker::default(), 1).unwrap();
        assert!(o.merged.is_none());
        assert_eq!(o.corrupt.len(), 1);
        assert_eq!(o.peers, 2);
        for d in [da, shared] {
            std::fs::remove_dir_all(d).unwrap();
        }
    }

    #[test]
    fn device_id_is_stable_and_trackers_live_in_the_data_dir() {
        let dir = tmp("dev");
        let id = device_id(&dir);
        assert_eq!(device_id(&dir), id);
        let (_, t) = sync::publish(&id, &[("1".into(), json!("x"))], &Tracker::default(), 5);
        save_tracker(&dir, "notes", &t).unwrap();
        assert_eq!(load_tracker(&dir, "notes"), t);
        assert_eq!(load_tracker(&dir, "chats"), Tracker::default());
        std::fs::write(dir.join("sync-state").join("chats.json"), "garbage").unwrap();
        assert_eq!(load_tracker(&dir, "chats"), Tracker::default());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sync_dir_comes_from_the_environment_and_icloud_missing_is_reported() {
        let home = tmp("home");
        assert_eq!(resolve_icloud_under(&home), Err(Unavailable::ICloudMissing));
        std::fs::create_dir_all(home.join("Library/Mobile Documents/com~apple~CloudDocs")).unwrap();
        let dir = resolve_icloud_under(&home).unwrap();
        assert!(dir.ends_with("com~apple~CloudDocs/Ferrocast") && dir.is_dir());
        assert!(Unavailable::ICloudMissing.message().contains("iCloud Drive"));
        std::fs::remove_dir_all(home).unwrap();
    }

    fn image_entry(id: &str, name: &str, at: i64) -> ClipEntry {
        ClipEntry {
            id: id.into(),
            kind: ClipKind::Image,
            content: name.into(),
            title: None,
            copied_at: at,
            pinned: false,
            source_app: None,
            times_copied: 1,
            width: 1,
            height: 1,
        }
    }

    #[test]
    fn clipboard_images_travel_with_their_entries() {
        let (da, db, shared) = (tmp("a"), tmp("b"), tmp("shared"));
        let (mut a, mut b) = (Mac::new(&da, &shared), Mac::new(&db, &shared));
        std::fs::create_dir_all(a.store.images_dir()).unwrap();
        std::fs::write(a.store.images_dir().join("abc.png"), b"PNGDATA").unwrap();
        a.clipboard = vec![image_entry("c1", "abc.png", 100)];
        a.sync(1_000);
        assert!(shared.join("devices").join(&a.io.device).join("clipboard-images/abc.png").is_file());
        b.sync(2_000);
        assert_eq!(b.clipboard.len(), 1);
        assert_eq!(std::fs::read(b.store.images_dir().join("abc.png")).unwrap(), b"PNGDATA");
        // the entry is dropped on A: its copy in the sync folder goes away too
        a.clipboard.clear();
        a.sync(3_000);
        assert!(!shared.join("devices").join(&a.io.device).join("clipboard-images/abc.png").exists());
        // a path outside the images folder is never followed
        let evil = vec![("e".to_string(), serde_json::to_value(image_entry("e", "../../secret", 1)).unwrap())];
        assert!(image_names(&evil).is_empty());
        for d in [da, db, shared] {
            std::fs::remove_dir_all(d).unwrap();
        }
    }

    #[test]
    fn clipboard_normalize_applies_retention_dedupes_and_keeps_pins() {
        let text = |id: &str, c: &str, at: i64, pinned: bool| ClipEntry {
            id: id.into(),
            kind: ClipKind::Text,
            content: c.into(),
            title: None,
            copied_at: at,
            pinned,
            source_app: None,
            times_copied: 1,
            width: 0,
            height: 0,
        };
        let list = vec![
            text("a", "same", 600, false),
            text("b", "same", 900, false),
            text("c", "old", 1, false),
            text("d", "oldpinned", 1, true),
            text("e", "tie", 800, false),
            text("f", "tie", 800, false),
        ];
        let out = clipboard_normalize(list.clone(), 1_000, Some(500));
        let ids: Vec<_> = out.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["b", "d", "f"]);
        let all = clipboard_normalize(list.clone(), 1_000, None);
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn a_pin_survives_when_copies_of_the_same_entry_are_merged() {
        let mk = |id: &str, at: i64, pinned: bool| ClipEntry {
            id: id.into(),
            kind: ClipKind::Text,
            content: "same".into(),
            title: None,
            copied_at: at,
            pinned,
            source_app: None,
            times_copied: 1,
            width: 0,
            height: 0,
        };
        let out = clipboard_normalize(vec![mk("old", 10, true), mk("new", 20, false)], 40, Some(50));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "new");
        assert!(out[0].pinned, "pinned on any Mac stays pinned");
        // an old pinned copy is not lost to the retention either
        let out = clipboard_normalize(vec![mk("old", 1, true), mk("new", 90, false)], 1_000, Some(50));
        assert!(out.iter().all(|e| e.pinned) && out.len() == 1);
    }

    static DOWNLOADS: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());
    static KILL_SWITCH: std::sync::Mutex<Option<std::sync::Arc<std::sync::atomic::AtomicBool>>> = std::sync::Mutex::new(None);

    fn record_download(p: &Path) {
        DOWNLOADS.lock().unwrap().push(p.to_path_buf());
    }

    fn download_then_turn_sync_off(p: &Path) {
        record_download(p);
        if let Some(f) = KILL_SWITCH.lock().unwrap().as_ref() {
            f.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    }

    #[test]
    fn icloud_placeholders_request_a_download_and_are_counted() {
        let (da, shared) = (tmp("a"), tmp("shared"));
        let mut a = Mac::new(&da, &shared);
        a.io.download = record_download;
        let other = shared.join("devices").join("other");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join(".snippets.json.icloud"), b"placeholder").unwrap();
        let o = sync_domain(&a.io, "snippets", &[], &Tracker::default(), 1).unwrap();
        assert_eq!(o.downloading, 1);
        assert!(DOWNLOADS.lock().unwrap().iter().any(|p| p.ends_with("other/snippets.json")));
        // an image that is still a placeholder is requested too
        let other_images = other.join("clipboard-images");
        std::fs::create_dir_all(&other_images).unwrap();
        std::fs::write(other_images.join(".pic.png.icloud"), b"placeholder").unwrap();
        let entry = image_entry("c1", "pic.png", 5);
        let doc = DeviceDoc {
            device: "other".into(),
            items: [(
                "c1".to_string(),
                sync::Record {
                    modified: 9,
                    deleted: false,
                    value: Some(serde_json::to_value(&entry).unwrap()),
                    pristine: false,
                    adopted: false,
                },
            )]
            .into(),
            ..Default::default()
        };
        std::fs::write(other.join("clipboard.json"), serde_json::to_vec(&doc).unwrap()).unwrap();
        let o = sync_domain(&a.io, "clipboard", &[], &Tracker::default(), 2).unwrap();
        assert_eq!(o.downloading, 1);
        assert!(DOWNLOADS.lock().unwrap().iter().any(|p| p.ends_with("other/clipboard-images/pic.png")));
        for d in [da, shared] {
            std::fs::remove_dir_all(d).unwrap();
        }
    }

    #[test]
    fn a_run_does_not_write_once_sync_is_turned_off() {
        let (da, shared) = (tmp("a"), tmp("shared"));
        let mut a = Mac::new(&da, &shared);
        a.io.enabled.store(false, std::sync::atomic::Ordering::SeqCst);
        a.snippets = vec![snip("s1", "one")];
        assert!(sync_domain(&a.io, "snippets", &to_items(&a.snippets), &Tracker::default(), 1).is_err());
        assert!(!shared.join("devices").exists(), "nothing written while off");
        // turned off in the middle of a run (while a placeholder download is requested): no image copy
        a.io.enabled.store(true, std::sync::atomic::Ordering::SeqCst);
        a.io.download = download_then_turn_sync_off;
        *KILL_SWITCH.lock().unwrap() = Some(a.io.enabled.clone());
        let other = shared.join("devices").join("other");
        std::fs::create_dir_all(other.join("clipboard-images")).unwrap();
        std::fs::write(other.join("clipboard-images").join("pic.png"), b"PNG").unwrap();
        let other2 = shared.join("devices").join("other2");
        std::fs::create_dir_all(&other2).unwrap();
        std::fs::write(other2.join(".clipboard.json.icloud"), b"x").unwrap();
        let doc = DeviceDoc {
            device: "other".into(),
            items: [(
                "c1".to_string(),
                sync::Record {
                    modified: 9,
                    deleted: false,
                    value: Some(serde_json::to_value(image_entry("c1", "pic.png", 5)).unwrap()),
                    pristine: false,
                    adopted: false,
                },
            )]
            .into(),
            ..Default::default()
        };
        std::fs::write(other.join("clipboard.json"), serde_json::to_vec(&doc).unwrap()).unwrap();
        // the run reads documents (placeholder download turns sync off), then must not copy the image
        let _ = sync_domain(&a.io, "clipboard", &[], &Tracker::default(), 2);
        // the switch was only pulled after the document write; the image copy is the next write
        assert!(!a.store.images_dir().join("pic.png").exists(), "no image copied after sync was turned off");
        *KILL_SWITCH.lock().unwrap() = None;
        for d in [da, shared] {
            std::fs::remove_dir_all(d).unwrap();
        }
    }

    #[test]
    fn a_copied_data_folder_gets_a_new_device_id_and_a_known_one_stays() {
        let (one, two) = (tmp("mac1"), tmp("mac2"));
        let id = device_id_with(&one, Some("HW-1"));
        assert_eq!(device_id_with(&one, Some("HW-1")), id, "stable on the same Mac");
        // Migration Assistant / a restored backup: the folder and its id file arrive on another Mac
        std::fs::copy(one.join("sync-device-id"), {
            std::fs::create_dir_all(&two).unwrap();
            two.join("sync-device-id")
        })
        .unwrap();
        let moved = device_id_with(&two, Some("HW-2"));
        assert_ne!(moved, id);
        assert_eq!(device_id_with(&two, Some("HW-2")), moved);
        // ioreg unreadable: keep what is stored
        assert_eq!(device_id_with(&two, None), moved);
        // an id file from before the hardware binding is adopted, not replaced
        std::fs::write(one.join("sync-device-id"), "legacy-id\n").unwrap();
        assert_eq!(device_id_with(&one, Some("HW-1")), "legacy-id");
        assert_eq!(device_id_with(&one, Some("HW-1")), "legacy-id");
        for d in [one, two] {
            std::fs::remove_dir_all(d).unwrap();
        }
    }

    #[test]
    fn an_id_file_that_cannot_be_written_gives_a_stable_fallback_not_a_new_id_per_run() {
        let dir = tmp("ro");
        let blocker = dir.join("file");
        std::fs::write(&blocker, b"x").unwrap();
        let data = blocker.join("data"); // a folder below a regular file: nothing can be created there
        let first = device_id_with(&data, Some("HW-1"));
        assert_eq!(device_id_with(&data, Some("HW-1")), first);
        assert_eq!(device_id_with(&data, None), device_id_with(&data, None));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn domain_names_map_to_files() {
        for d in DOMAINS {
            assert_eq!(domain_of_file(domain_file(d)), Some(d));
        }
        assert_eq!(domain_file("themes"), "themes.json");
        assert_eq!(domain_of_file("frecency.json"), None);
    }
}
