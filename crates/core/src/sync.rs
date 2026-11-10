//! Sync between Macs through a shared folder (iCloud Drive): pure merge rules, no I/O.
//!
//! Each Mac writes only its own document per domain (`devices/<device>/<domain>.json`), so no file is
//! ever written by two Macs. Every Mac reads all documents and merges them item by item: for each id
//! the record with the newest `modified` wins (ties broken by device id), and a winning tombstone
//! removes the item. A local tracker remembers each item's hash and time so that only real local edits
//! get a new time, and items that arrived from another Mac are not sent back as new edits.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};

/// One item as a device last saw it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    /// Unix milliseconds of the edit (or deletion) this record describes.
    pub modified: i64,
    /// A tombstone: the item was deleted at `modified`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deleted: bool,
    /// The item itself; `None` for tombstones.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    /// "This value is the built-in default and was never edited on the Mac that published it."
    /// A record that is not pristine beats a pristine one whatever their times, so a Mac that was
    /// customised before it ever synced is not overwritten by a fresh Mac that synced first.
    /// Documents without the flag (older versions) count as not pristine.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pristine: bool,
    /// The publisher took this record over from another Mac (it did not originate there). Such a
    /// record is never re-stamped by `restamp_baseline`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub adopted: bool,
}

impl Record {
    /// Rank before time: a live pristine record is 0, everything else (edited items, deletions) is 1.
    fn rank(&self) -> u8 {
        u8::from(self.deleted || !self.pristine)
    }
}

/// What one device publishes for one domain.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct DeviceDoc {
    pub device: String,
    pub items: BTreeMap<String, Record>,
    /// Time of the publisher's first sync of this domain; lets it rebuild its tracker from this
    /// document if the tracker is lost.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_sync_at: Option<i64>,
    /// Name of the publishing Mac, for a conflict copy to say whose text it holds. Older builds do
    /// not write it and ignore it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// When the publisher last ran a sync of this domain that had merged every other Mac's document
    /// (unix ms, refreshed at most every few hours so the document is not rewritten each run). A
    /// tombstone stamped before every live Mac's `run_at` has been seen by all of them. Older builds
    /// do not write it and ignore it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_at: Option<i64>,
    /// Read for compatibility, never written: deletions are not collected (a Mac that returns later
    /// must still learn about them). Reserved for an acknowledgement-based clean-up.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gc_at: Option<i64>,
    /// This Mac's own clock when it wrote this document; strictly increasing across its documents of
    /// a domain. Only ever compared with other values of the same Mac's clock. Older builds do not
    /// write it and ignore it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub written_at: Option<i64>,
    /// Per peer Mac: the `written_at` of the peer document this Mac had read cleanly, merged,
    /// applied and saved before it wrote this document. A peer that sees `acks[peer] >= W` knows this
    /// Mac holds every deletion the peer published in documents written up to `W`. Older builds do
    /// not write it and ignore it.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub acks: BTreeMap<String, i64>,
}

/// Local memory of the last published state, per item id: (content hash, modified, deleted).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Tracker {
    pub items: BTreeMap<String, Tracked>,
    /// False until the first `publish`: a Mac that never synced this domain (or lost its tracker)
    /// has no history to compare with.
    #[serde(default)]
    pub seeded: bool,
    /// Time of this Mac's first sync of the domain. Records this Mac published at 0 (the baseline of
    /// that first sync) are re-stamped once to this time by `restamp_baseline`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_sync_at: Option<i64>,
    /// Per tombstone this Mac publishes: the `written_at` of an own document that contained it when
    /// it was recorded (never earlier than the first document that did), the `W` of the
    /// acknowledgement rule. Local only, never published.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub first_pub: BTreeMap<String, i64>,
    /// Deletions this Mac collected (see `PublishOpts::collect_upto`): item id -> stamp of the
    /// tombstone. A record of the item that another Mac publishes with a stamp at or below it is
    /// ignored, so a stale copy cannot bring the item back. Local only, never published.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub collected: BTreeMap<String, i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tracked {
    pub hash: String,
    pub modified: i64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deleted: bool,
    /// A deletion this Mac only learned from another Mac (it did not originate here).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub remote: bool,
    /// The record is pristine (see `Record::pristine`): set on the first publish when the value is a
    /// built-in default, lost with the first edit, carried over from the record that was adopted.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pristine: bool,
    /// Taken over from another Mac's record (see `Record::adopted`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub adopted: bool,
    /// This Mac holds the item's value in its tracker but could not read it into local data (a newer
    /// build wrote it). It is absent from the local items on purpose, so `publish` neither tombstones
    /// nor writes it, and the record the other Mac published stays the one that counts.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unparsed: bool,
    /// For a tombstone stamped in the future (a Mac with a fast clock): when this Mac first saw it,
    /// so it still ages out. Kept only in the local tracker; the published stamp is never touched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seen: Option<i64>,
    /// Time of the last version this Mac shared with another Mac (adopted from it, or the first
    /// stamp of a seeded item). Own publishes leave it alone: a record another Mac wrote after it is
    /// a concurrent edit of this Mac's, whatever this Mac published since. Local only, never
    /// published; trackers without it compare with `modified` as before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<i64>,
}

/// Flag the items of `ids` that this Mac could not read into its local data (see `Tracked::unparsed`).
/// Ids the tracker does not know, or knows as deleted, are left alone.
pub fn mark_unparsed(tracker: &mut Tracker, ids: &[String]) {
    for id in ids {
        if let Some(t) = tracker.items.get_mut(id).filter(|t| !t.deleted) {
            t.unparsed = true;
        }
    }
}

/// Default tombstone lifetime of `publish`/`publish_with` (90 days). The app keeps tombstones until
/// never (they are kept so a Mac that returns later still learns of them); the clipboard ages them, see `sync_io`.
pub const TOMBSTONE_TTL_MS: i64 = 90 * 24 * 3600 * 1000;

pub fn hash(value: &Value) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// The time of a new edit or deletion of an item whose last known record is stamped `last`: the wall
/// clock, but never at or before `last`. A Mac whose clock runs behind (or that saw a record from a
/// Mac running ahead, even one stamped years in the future) still orders what it does after seeing
/// that record after it, so an edit made after receiving a version is never lost to it. Nothing is
/// ignored or clamped: a record stamped far ahead keeps its place, later edits just follow it.
fn logical_now(now: i64, last: i64) -> i64 {
    now.max(last.saturating_add(1))
}

/// Record the current local items: changed or new items get `now`, missing ones become tombstones at
/// `now`, unchanged ones keep their time. Returns this device's document and the new tracker.
pub fn publish(device: &str, items: &[(String, Value)], prev: &Tracker, now: i64) -> (DeviceDoc, Tracker) {
    publish_with(device, items, prev, now, now)
}

/// Like `publish`, but when `prev` was never seeded (first sync of this domain on this Mac) the local
/// items are a baseline stamped `first_stamp` instead of `now`: pass 0 when other Macs already
/// published, so what they published wins and only items nobody has are added.
pub fn publish_with(device: &str, items: &[(String, Value)], prev: &Tracker, now: i64, first_stamp: i64) -> (DeviceDoc, Tracker) {
    publish_ttl(device, items, prev, now, first_stamp, Some(TOMBSTONE_TTL_MS))
}

/// Like `publish_with`, with the lifetime of tombstones chosen by the caller: `None` keeps them for
/// ever. Without that, a device document that was not touched for longer than the lifetime (an old
/// Mac, a folder left by a migration) would bring a deleted item back, because nothing remembers
/// the deletion any more. A tombstone is only an id and a time, so keeping them costs little.
pub fn publish_ttl(
    device: &str,
    items: &[(String, Value)],
    prev: &Tracker,
    now: i64,
    first_stamp: i64,
    ttl: Option<i64>,
) -> (DeviceDoc, Tracker) {
    let opts = PublishOpts { first_stamp, ttl, relay_tombstones: true, ..Default::default() };
    publish_opts(device, items, prev, now, &opts)
}

/// How `publish_opts` treats the first sync and tombstones.
#[derive(Debug, Clone, Default)]
pub struct PublishOpts {
    /// Time given to local items on this Mac's first sync of the domain (0 = baseline: any real time
    /// beats it).
    pub first_stamp: i64,
    /// Lifetime of tombstones; `None` = for ever.
    pub ttl: Option<i64>,
    /// Ids that exist on every fresh install (built-in defaults). On a first sync those missing
    /// locally are published as deletions (at `first_stamp`), so a default the user removed does not
    /// come back when another Mac is added.
    pub baseline_deleted: Vec<String>,
    /// Republish deletions learned from other Macs. Off keeps the document small: every Mac reads
    /// every other Mac's document, so only the deleting Mac needs to publish its deletion.
    pub relay_tombstones: bool,
    /// Keep at most this many tombstones (the newest).
    pub max_tombstones: Option<usize>,
    /// Tells whether a value is the built-in default of its item. Called for items this Mac publishes
    /// for the first time on its first sync; `None` = nothing is pristine.
    pub pristine: Option<fn(&str, &Value) -> bool>,
    /// Ids that another Mac's document already carries (see `carried_elsewhere`). `Some` turns on
    /// the own-entries rule: an item this Mac took over from another Mac and has not changed is kept
    /// in the tracker but not written into this Mac's document while its id is in the set. `None`
    /// publishes everything (every domain but the clipboard).
    pub carried: Option<HashSet<String>>,
    /// `written_at` of the document to publish (`None`: the clipboard, which has no acknowledgements).
    pub written_at: Option<i64>,
    /// `acks` of the document to publish.
    pub acks: BTreeMap<String, i64>,
    /// Acknowledgement-based collection: `Some(m)` when every Mac that is not forgotten has a clean
    /// document whose `acks` for this Mac are at least `m` (the smallest of them). A tombstone whose
    /// recorded `first_pub` is at or below `m` has been merged by every such Mac and is dropped, its
    /// stamp remembered in `Tracker::collected`. `None` collects nothing.
    pub collect_upto: Option<i64>,
    /// Ids whose deletions are never collected (built-in defaults).
    pub keep: HashSet<String>,
}

/// The general form of `publish`: see `PublishOpts`.
pub fn publish_opts(device: &str, items: &[(String, Value)], prev: &Tracker, now: i64, opts: &PublishOpts) -> (DeviceDoc, Tracker) {
    let unseen = if prev.seeded { now } else { opts.first_stamp };
    let first_sync_at = if prev.seeded { prev.first_sync_at } else { Some(now) };
    let mut doc = DeviceDoc {
        device: device.to_string(),
        items: BTreeMap::new(),
        first_sync_at,
        name: None,
        run_at: None,
        gc_at: None,
        written_at: opts.written_at,
        acks: opts.acks.clone(),
    };
    let mut next = Tracker { items: BTreeMap::new(), seeded: true, first_sync_at, collected: prev.collected.clone(), ..Default::default() };
    let mut seen = HashSet::new();
    for (id, value) in items {
        if !seen.insert(id.clone()) {
            continue; // duplicate id: the first one wins
        }
        let h = hash(value);
        // an edit (a changed hash, even back to the default) is a real change: not pristine, now
        let (modified, pristine, adopted, base) = match prev.items.get(id) {
            Some(t) if !t.deleted && t.hash == h => (t.modified, t.pristine, t.adopted, t.base),
            // the shared version stays the one before the first own edit
            Some(t) if !t.deleted => (logical_now(now, t.modified), false, false, t.base.or(Some(t.modified))),
            Some(t) => (logical_now(now, t.modified), false, false, None),
            // an id this Mac collected must not be re-stamped at or below the collected tombstone
            None if prev.seeded && prev.collected.contains_key(id) => (logical_now(unseen, prev.collected[id]), false, false, Some(unseen)),
            None => (unseen, !prev.seeded && opts.pristine.is_some_and(|f| f(id, value)), false, Some(unseen)),
        };
        // own-entries rule: an unchanged item taken over from another Mac stays out of this document
        // while another document carries it (the tracker still knows it, so it is not a new edit)
        let held_elsewhere = adopted && opts.carried.as_ref().is_some_and(|c| c.contains(id));
        if !held_elsewhere {
            doc.items.insert(id.clone(), Record { modified, deleted: false, value: Some(value.clone()), pristine, adopted });
        }
        next.items.insert(
            id.clone(),
            Tracked { hash: h, modified, deleted: false, remote: false, pristine, adopted, unparsed: false, seen: None, base },
        );
    }
    for (id, t) in &prev.items {
        if seen.contains(id) {
            continue;
        }
        if t.unparsed && !t.deleted {
            // not readable here, so absent from the local items without being deleted: keep
            // remembering it, publish nothing (the Mac that wrote it still carries its value)
            next.items.insert(id.clone(), t.clone());
            continue;
        }
        if t.deleted && t.remote && !opts.relay_tombstones {
            continue; // someone else's deletion: their document carries it
        }
        let modified = if t.deleted { t.modified } else { logical_now(now, t.modified) };
        if t.deleted && collectable(prev, opts, id) {
            let c = next.collected.entry(id.clone()).or_insert(modified);
            *c = (*c).max(modified);
            continue; // every Mac has merged it: this Mac stops publishing and tracking it
        }
        // a tombstone stamped ahead of this Mac's clock ages from when this Mac first saw it
        let seen = (modified > now).then(|| t.seen.unwrap_or(now));
        if opts.ttl.is_some_and(|ttl| now - seen.map_or(modified, |s| s.min(now)) > ttl) {
            continue;
        }
        doc.items.insert(id.clone(), Record { modified, deleted: true, value: None, pristine: false, adopted: false });
        next.items.insert(
            id.clone(),
            Tracked {
                hash: t.hash.clone(),
                modified,
                deleted: true,
                remote: t.remote,
                pristine: false,
                adopted: false,
                unparsed: false,
                seen,
                base: None,
            },
        );
    }
    if !prev.seeded {
        for id in &opts.baseline_deleted {
            if seen.contains(id) || prev.items.contains_key(id) {
                continue;
            }
            doc.items
                .insert(id.clone(), Record { modified: opts.first_stamp, deleted: true, value: None, pristine: false, adopted: false });
            next.items.insert(
                id.clone(),
                Tracked {
                    hash: String::new(),
                    modified: opts.first_stamp,
                    deleted: true,
                    remote: false,
                    pristine: false,
                    adopted: false,
                    unparsed: false,
                    seen: None,
                    base: None,
                },
            );
        }
    }
    if let Some(max) = opts.max_tombstones {
        let mut dead: Vec<(i64, String)> = doc.items.iter().filter(|(_, r)| r.deleted).map(|(id, r)| (r.modified, id.clone())).collect();
        if dead.len() > max {
            dead.sort();
            for (_, id) in dead.into_iter().take(doc.items.values().filter(|r| r.deleted).count() - max) {
                doc.items.remove(&id);
                next.items.remove(&id);
            }
        }
    }
    next.first_pub =
        prev.first_pub.iter().filter(|(id, _)| next.items.get(*id).is_some_and(|t| t.deleted)).map(|(k, v)| (k.clone(), *v)).collect();
    (doc, next)
}

/// True when the tombstone of `id` in `prev` may be dropped: acknowledged by every Mac up to its
/// recorded `first_pub`, and not a built-in default's deletion.
fn collectable(prev: &Tracker, opts: &PublishOpts, id: &str) -> bool {
    let Some(upto) = opts.collect_upto else { return false };
    !opts.keep.contains(id) && prev.first_pub.get(id).is_some_and(|w| *w <= upto)
}

/// Ids that a Mac other than `own` already publishes as live in its document, for the clipboard's
/// own-entries rule. A record counts when its publisher originated it (not `adopted`) or, when
/// the publisher only took it over (documents of v0.7.0 and older Macs, or a Mac that republished it
/// because its origin left), when the publisher's id is lower than `own`: of several Macs that all
/// hold a copy and none is the origin, only the one with the lowest id keeps publishing, so the
/// entry never disappears from every document and is never published by all of them.
pub fn carried_elsewhere(own: &str, docs: &[DeviceDoc]) -> HashSet<String> {
    let mut out = HashSet::new();
    for d in docs.iter().filter(|d| d.device != own) {
        for (id, r) in &d.items {
            if !r.deleted && (!r.adopted || d.device.as_str() < own) {
                out.insert(id.clone());
            }
        }
    }
    out
}

/// Drop from `docs` the live records of an item this Mac collected whose stamp is at or below the
/// collected tombstone's: a stale copy (a Mac that came back, a document restored from a backup)
/// must not bring a deleted item back. A record stamped after the tombstone is an edit and stays.
pub fn ignore_collected(docs: &mut [DeviceDoc], collected: &BTreeMap<String, i64>) {
    if collected.is_empty() {
        return;
    }
    for d in docs.iter_mut() {
        d.items.retain(|id, r| r.deleted || collected.get(id).is_none_or(|stamp| r.modified > *stamp));
    }
}

/// Rebuild a tracker from this Mac's own published document, for a lost or corrupt tracker file:
/// what was published stays the baseline, so edits and deletions made since still count as local
/// changes instead of being lost to a restart as a newcomer.
pub fn tracker_from_own_doc(doc: &DeviceDoc) -> Tracker {
    let mut t = Tracker { items: BTreeMap::new(), seeded: true, first_sync_at: doc.first_sync_at, ..Default::default() };
    for (id, r) in &doc.items {
        if let (true, Some(w)) = (r.deleted, doc.written_at) {
            t.first_pub.insert(id.clone(), w); // the document contains it, so this is never too early
        }
        let hash = r.value.as_ref().map(hash).unwrap_or_default();
        t.items.insert(
            id.clone(),
            Tracked {
                hash,
                modified: r.modified,
                deleted: r.deleted,
                remote: false,
                pristine: r.pristine,
                adopted: r.adopted,
                unparsed: false,
                seen: None,
                base: None,
            },
        );
    }
    t
}

/// First-sync rule, second half. A record this Mac itself published at 0 (the baseline of its first
/// sync; not one it took over from another Mac) and that survived the merge is moved once to the
/// time of this Mac's first sync, not to now. From then on it beats the 0 baseline of every Mac that
/// joins later, whatever the device ids, and still loses to any genuine edit made after that time.
/// Returns true when something changed. Call it after `merge` on every run but the first.
pub fn restamp_baseline(tracker: &mut Tracker) -> bool {
    let Some(first) = tracker.first_sync_at.filter(|f| *f > 0) else { return false };
    let mut changed = false;
    for t in tracker.items.values_mut() {
        if t.modified == 0 && !t.adopted {
            t.modified = first;
            if t.base == Some(0) {
                t.base = Some(first); // the shared version moves with its stamp
            }
            changed = true;
        }
    }
    changed
}

/// The winning record for every id across all documents: a record that is not pristine beats a
/// pristine one, then the newest `modified`, then the greater device id, so every Mac picks the same winner.
pub fn winners(docs: &[DeviceDoc]) -> BTreeMap<String, (String, Record)> {
    let mut out: BTreeMap<String, (String, Record)> = BTreeMap::new();
    for doc in docs {
        for (id, rec) in &doc.items {
            let better = match out.get(id) {
                None => true,
                Some((dev, cur)) => (rec.rank(), rec.modified, doc.device.as_str()) > (cur.rank(), cur.modified, dev.as_str()),
            };
            if better {
                out.insert(id.clone(), (doc.device.clone(), rec.clone()));
            }
        }
    }
    out
}

/// Merge every document into the local items. Existing items keep their order; items new to this Mac
/// are appended oldest first. Returns the merged items and a tracker that already knows the winners,
/// so the next `publish` does not resend them as local edits.
pub fn merge(local: &[(String, Value)], docs: &[DeviceDoc], tracker: &Tracker) -> (Vec<(String, Value)>, Tracker) {
    merge_as("", local, docs, tracker)
}

/// `merge` for the Mac `own`: when this Mac's record and the winning remote one have exactly the same
/// time (two Macs that both stamped a first sync with 0), the greater device id wins on both Macs.
pub fn merge_as(own: &str, local: &[(String, Value)], docs: &[DeviceDoc], tracker: &Tracker) -> (Vec<(String, Value)>, Tracker) {
    let win = winners(docs);
    let mut out: Vec<(String, Value)> = Vec::new();
    let mut next = tracker.clone();
    let mut placed = HashSet::new();
    for (id, value) in local {
        if !placed.insert(id.clone()) {
            continue;
        }
        match win.get(id) {
            Some((dev, rec)) if rec.deleted => {
                // Deleted elsewhere after our last edit.
                if local_wins(tracker, id, value, rec, dev, own) {
                    out.push((id.clone(), value.clone()));
                } else {
                    next.items.insert(
                        id.clone(),
                        Tracked {
                            hash: hash(value),
                            modified: rec.modified,
                            deleted: true,
                            remote: true,
                            pristine: false,
                            adopted: false,
                            unparsed: false,
                            seen: None,
                            base: None,
                        },
                    );
                }
            }
            Some((dev, rec)) => {
                let theirs = rec.value.clone().unwrap_or(Value::Null);
                if local_wins(tracker, id, value, rec, dev, own) || hash(&theirs) == hash(value) {
                    out.push((id.clone(), value.clone()));
                } else {
                    next.items.insert(
                        id.clone(),
                        Tracked {
                            hash: hash(&theirs),
                            modified: rec.modified,
                            deleted: false,
                            remote: false,
                            pristine: rec.pristine,
                            adopted: true,
                            unparsed: false,
                            seen: None,
                            base: Some(rec.modified),
                        },
                    );
                    out.push((id.clone(), theirs));
                }
            }
            None => out.push((id.clone(), value.clone())),
        }
    }
    let mut fresh: Vec<(&String, &Record)> =
        win.iter().filter(|(id, (_, rec))| !rec.deleted && !placed.contains(*id)).map(|(id, (_, rec))| (id, rec)).collect();
    fresh.sort_by_key(|(id, rec)| (rec.modified, (*id).clone()));
    for (id, rec) in fresh {
        // Deleted here since the other Mac's edit: our tombstone is newer, keep it deleted.
        if let Some(t) = tracker.items.get(id) {
            if t.deleted && (rec.rank() == 0 || t.modified >= rec.modified) {
                continue;
            }
        }
        let v = rec.value.clone().unwrap_or(Value::Null);
        // an item that was unreadable here stays flagged until the caller finds it still unreadable
        let unparsed = tracker.items.get(id).is_some_and(|t| t.unparsed && !t.deleted);
        next.items.insert(
            id.clone(),
            Tracked {
                hash: hash(&v),
                modified: rec.modified,
                deleted: false,
                remote: false,
                pristine: rec.pristine,
                adopted: true,
                unparsed,
                seen: None,
                base: Some(rec.modified),
            },
        );
        out.push((id.clone(), v));
    }
    // an unreadable item that no Mac has any more (or whose winner is a deletion) is forgotten
    next.items.retain(|id, t| !t.unparsed || t.deleted || win.get(id).is_some_and(|(_, r)| !r.deleted));
    (out, next)
}

/// True when this Mac's record of `id` beats the winning remote record `rec` (from `their_device`):
/// not pristine beats pristine, then the later time, then the greater device id. A local value that
/// differs from what the tracker last published is an edit nobody has seen: it always wins.
fn local_wins(tracker: &Tracker, id: &str, value: &Value, rec: &Record, their_device: &str, own: &str) -> bool {
    match tracker.items.get(id) {
        Some(t) if t.hash == hash(value) && !t.deleted => {
            (u8::from(!t.pristine), t.modified, own) > (rec.rank(), rec.modified, their_device)
        }
        // Never published or changed since: an edit nobody else has seen yet.
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn items(v: &[(&str, Value)]) -> Vec<(String, Value)> {
        v.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
    }

    #[test]
    fn unchanged_items_keep_their_time_and_edits_get_now() {
        let (_, t1) = publish("a", &items(&[("1", json!("x")), ("2", json!("y"))]), &Tracker::default(), 100);
        let (doc, t2) = publish("a", &items(&[("1", json!("x")), ("2", json!("y2"))]), &t1, 200);
        assert_eq!(doc.items["1"].modified, 100);
        assert_eq!(doc.items["2"].modified, 200);
        assert_eq!(t2.items["2"].modified, 200);
    }

    #[test]
    fn a_removed_item_becomes_a_tombstone_once() {
        let (_, t1) = publish("a", &items(&[("1", json!("x"))]), &Tracker::default(), 100);
        let (doc, t2) = publish("a", &[], &t1, 200);
        assert_eq!(doc.items["1"], Record { modified: 200, deleted: true, value: None, pristine: false, adopted: false });
        let (doc, _) = publish("a", &[], &t2, 300);
        assert_eq!(doc.items["1"].modified, 200, "the deletion time does not move");
    }

    #[test]
    fn old_tombstones_are_dropped() {
        let (_, t1) = publish("a", &items(&[("1", json!("x"))]), &Tracker::default(), 0);
        let (_, t2) = publish("a", &[], &t1, 10);
        let (doc, t3) = publish("a", &[], &t2, 10 + TOMBSTONE_TTL_MS + 1);
        assert!(doc.items.is_empty());
        assert!(t3.items.is_empty());
    }

    #[test]
    fn a_far_future_tombstone_still_ages_out_from_when_it_was_first_seen_without_changing_its_stamp() {
        let future = 10_000_000_000_000;
        let (_, t1) = publish("a", &items(&[("1", json!("x"))]), &Tracker::default(), future);
        let (doc, t2) = publish("a", &[], &t1, 1_000);
        let stamp = doc.items["1"].modified;
        assert!(stamp > future, "the stamp keeps its place in the merge order");
        let (doc, t3) = publish("a", &[], &t2, 1_000 + TOMBSTONE_TTL_MS / 2);
        assert_eq!(doc.items["1"].modified, stamp, "stamp unchanged while it is kept");
        let (doc, t4) = publish("a", &[], &t3, 1_000 + TOMBSTONE_TTL_MS + 1);
        assert!(doc.items.is_empty() && t4.items.is_empty(), "aged out by the time this Mac first saw it");
    }

    #[test]
    fn the_newest_edit_wins_and_ties_go_to_the_greater_device() {
        let a = DeviceDoc {
            device: "a".into(),
            items: [("1".to_string(), Record { modified: 5, deleted: false, value: Some(json!("a")), pristine: false, adopted: false })]
                .into(),
            ..Default::default()
        };
        let b = DeviceDoc {
            device: "b".into(),
            items: [("1".to_string(), Record { modified: 5, deleted: false, value: Some(json!("b")), pristine: false, adopted: false })]
                .into(),
            ..Default::default()
        };
        let c = DeviceDoc {
            device: "c".into(),
            items: [("1".to_string(), Record { modified: 4, deleted: false, value: Some(json!("c")), pristine: false, adopted: false })]
                .into(),
            ..Default::default()
        };
        assert_eq!(winners(&[a.clone(), b.clone(), c.clone()])["1"].1.value, Some(json!("b")));
        assert_eq!(winners(&[c, b, a])["1"].1.value, Some(json!("b")), "order of documents does not matter");
    }

    #[test]
    fn a_tie_at_the_same_time_goes_to_the_greater_device_on_both_macs() {
        let rec = |v: &str| Record { modified: 0, deleted: false, value: Some(json!(v)), pristine: false, adopted: false };
        let (da, ta) = publish_with("zz", &items(&[("1", json!("zz value"))]), &Tracker::default(), 5, 0);
        let (db, tb) = publish_with("aa", &items(&[("1", json!("aa value"))]), &Tracker::default(), 5, 0);
        assert_eq!(da.items["1"], rec("zz value"));
        let (on_zz, _) = merge_as("zz", &items(&[("1", json!("zz value"))]), std::slice::from_ref(&db), &ta);
        let (on_aa, _) = merge_as("aa", &items(&[("1", json!("aa value"))]), std::slice::from_ref(&da), &tb);
        assert_eq!(on_zz, items(&[("1", json!("zz value"))]));
        assert_eq!(on_aa, items(&[("1", json!("zz value"))]), "both Macs end with the greater device's value");
    }

    #[test]
    fn trackers_written_before_the_first_sync_time_and_the_remote_flag_still_load() {
        let old = r#"{"items":{"1":{"hash":"h","modified":5}},"seeded":true}"#;
        let t: Tracker = serde_json::from_str(old).unwrap();
        assert_eq!(t.first_sync_at, None);
        assert!(!t.items["1"].remote);
    }

    #[test]
    fn restamping_moves_baseline_records_once_and_a_received_deletion_is_not_republished_when_relaying_is_off() {
        let opts = |relay| PublishOpts {
            first_stamp: 0,
            ttl: None,
            baseline_deleted: vec![],
            relay_tombstones: relay,
            max_tombstones: None,
            pristine: None,
            ..Default::default()
        };
        let (_, mut t) = publish_opts("a", &items(&[("1", json!("x")), ("2", json!("y"))]), &Tracker::default(), 500, &opts(true));
        assert_eq!((t.items["1"].modified, t.first_sync_at), (0, Some(500)));
        assert!(restamp_baseline(&mut t));
        assert_eq!(t.items["1"].modified, 500);
        assert!(!restamp_baseline(&mut t), "once");
        // a deletion learned from another Mac
        let theirs = DeviceDoc {
            device: "b".into(),
            items: [("2".to_string(), Record { modified: 900, deleted: true, value: None, pristine: false, adopted: false })].into(),
            ..Default::default()
        };
        let (merged, t2) = merge_as("a", &items(&[("1", json!("x")), ("2", json!("y"))]), &[theirs], &t);
        assert_eq!(merged, items(&[("1", json!("x"))]));
        assert!(publish_opts("a", &merged, &t2, 1_000, &opts(true)).0.items.contains_key("2"));
        assert!(!publish_opts("a", &merged, &t2, 1_000, &opts(false)).0.items.contains_key("2"));
    }

    #[test]
    fn a_record_that_is_not_pristine_beats_a_pristine_one_whatever_the_time_and_old_documents_count_as_not_pristine() {
        let doc = |dev: &str, modified, pristine, v: &str| DeviceDoc {
            device: dev.into(),
            items: [("1".to_string(), Record { modified, deleted: false, value: Some(json!(v)), pristine, adopted: false })].into(),
            ..Default::default()
        };
        let w = winners(&[doc("a", 9_000, true, "default"), doc("b", 1, false, "custom")]);
        assert_eq!(w["1"].1.value, Some(json!("custom")));
        // an old document has no `pristine` key
        let old: DeviceDoc = serde_json::from_str(r#"{"device":"c","items":{"1":{"modified":2,"value":"old"}}}"#).unwrap();
        assert!(!old.items["1"].pristine);
        let w = winners(&[doc("a", 9_000, true, "default"), old]);
        assert_eq!(w["1"].1.value, Some(json!("old")));
        // among equal statuses time decides
        let w = winners(&[doc("a", 5, false, "older"), doc("b", 6, false, "newer")]);
        assert_eq!(w["1"].1.value, Some(json!("newer")));
    }

    #[test]
    fn an_edit_makes_a_pristine_record_not_pristine_even_when_it_goes_back_to_the_default() {
        let opts = PublishOpts {
            first_stamp: 0,
            ttl: None,
            baseline_deleted: vec![],
            relay_tombstones: true,
            max_tombstones: None,
            pristine: Some(|_, v| v == &json!("default")),
            ..Default::default()
        };
        let (doc, t1) = publish_opts("a", &items(&[("1", json!("default"))]), &Tracker::default(), 10, &opts);
        assert!(doc.items["1"].pristine && t1.items["1"].pristine);
        let (doc, t2) = publish_opts("a", &items(&[("1", json!("edited"))]), &t1, 20, &opts);
        assert!(!doc.items["1"].pristine && doc.items["1"].modified == 20);
        let (doc, _) = publish_opts("a", &items(&[("1", json!("default"))]), &t2, 30, &opts);
        assert!(!doc.items["1"].pristine, "back to the default on purpose is an edit");
        assert_eq!(doc.items["1"].modified, 30);
        // a new item on a later run is never pristine
        let (doc, _) = publish_opts("a", &items(&[("1", json!("default")), ("2", json!("default"))]), &t2, 40, &opts);
        assert!(!doc.items["2"].pristine);
    }

    #[test]
    fn tombstones_can_be_kept_without_expiry() {
        let (_, t1) = publish_ttl("a", &items(&[("1", json!("x"))]), &Tracker::default(), 0, 0, None);
        let (doc, _) = publish_ttl("a", &[], &t1, 10, 10, None);
        let (doc2, t3) = publish_ttl("a", &[], &doc_tracker(&doc), 10 + 1000 * TOMBSTONE_TTL_MS, 0, None);
        assert!(doc2.items["1"].deleted && t3.items["1"].deleted);
    }

    fn doc_tracker(doc: &DeviceDoc) -> Tracker {
        Tracker {
            items: doc
                .items
                .iter()
                .map(|(k, r)| {
                    (
                        k.clone(),
                        Tracked {
                            hash: String::new(),
                            modified: r.modified,
                            deleted: r.deleted,
                            remote: false,
                            pristine: false,
                            adopted: false,
                            unparsed: false,
                            seen: None,
                            base: None,
                        },
                    )
                })
                .collect(),
            seeded: true,
            first_sync_at: None,
            ..Default::default()
        }
    }

    #[test]
    fn items_new_on_two_macs_are_both_kept() {
        let (da, ta) = publish("a", &items(&[("x", json!("from a"))]), &Tracker::default(), 100);
        let (db, _) = publish("b", &items(&[("y", json!("from b"))]), &Tracker::default(), 110);
        let (merged, _) = merge(&items(&[("x", json!("from a"))]), &[da, db], &ta);
        assert_eq!(merged, items(&[("x", json!("from a")), ("y", json!("from b"))]));
    }

    #[test]
    fn a_newer_remote_edit_replaces_the_local_value_without_echo() {
        let (da, ta) = publish("a", &items(&[("1", json!("old"))]), &Tracker::default(), 100);
        let mut db = da.clone();
        db.device = "b".into();
        db.items.insert("1".into(), Record { modified: 200, deleted: false, value: Some(json!("new")), pristine: false, adopted: false });
        let (merged, t) = merge(&items(&[("1", json!("old"))]), &[da, db], &ta);
        assert_eq!(merged, items(&[("1", json!("new"))]));
        let (doc, _) = publish("a", &merged, &t, 300);
        assert_eq!(doc.items["1"].modified, 200, "the merged value is not republished as a local edit");
    }

    #[test]
    fn an_unpublished_local_edit_beats_an_older_remote_one() {
        let (da, ta) = publish("a", &items(&[("1", json!("v1"))]), &Tracker::default(), 100);
        let db = DeviceDoc {
            device: "b".into(),
            items: [(
                "1".to_string(),
                Record { modified: 150, deleted: false, value: Some(json!("theirs")), pristine: false, adopted: false },
            )]
            .into(),
            ..Default::default()
        };
        // Local edit made after the last publish, not yet published.
        let (merged, _) = merge(&items(&[("1", json!("mine"))]), &[da, db], &ta);
        assert_eq!(merged, items(&[("1", json!("mine"))]));
    }

    #[test]
    fn a_remote_deletion_newer_than_the_local_edit_removes_the_item() {
        let (da, ta) = publish("a", &items(&[("1", json!("x")), ("2", json!("y"))]), &Tracker::default(), 100);
        let db = DeviceDoc {
            device: "b".into(),
            items: [("1".to_string(), Record { modified: 200, deleted: true, value: None, pristine: false, adopted: false })].into(),
            ..Default::default()
        };
        let (merged, t) = merge(&items(&[("1", json!("x")), ("2", json!("y"))]), &[da, db], &ta);
        assert_eq!(merged, items(&[("2", json!("y"))]));
        let (doc, _) = publish("a", &merged, &t, 300);
        assert_eq!(
            doc.items["1"],
            Record { modified: 200, deleted: true, value: None, pristine: false, adopted: false },
            "the tombstone is kept, not renewed"
        );
    }

    #[test]
    fn a_local_deletion_is_not_undone_by_an_older_remote_copy() {
        let (_, t1) = publish("a", &items(&[("1", json!("x"))]), &Tracker::default(), 100);
        let (da, t2) = publish("a", &[], &t1, 200);
        let db = DeviceDoc {
            device: "b".into(),
            items: [("1".to_string(), Record { modified: 100, deleted: false, value: Some(json!("x")), pristine: false, adopted: false })]
                .into(),
            ..Default::default()
        };
        let (merged, _) = merge(&[], &[da, db], &t2);
        assert!(merged.is_empty());
    }

    #[test]
    fn an_edit_elsewhere_after_a_local_deletion_brings_the_item_back() {
        let (_, t1) = publish("a", &items(&[("1", json!("x"))]), &Tracker::default(), 100);
        let (da, t2) = publish("a", &[], &t1, 200);
        let db = DeviceDoc {
            device: "b".into(),
            items: [("1".to_string(), Record { modified: 300, deleted: false, value: Some(json!("x2")), pristine: false, adopted: false })]
                .into(),
            ..Default::default()
        };
        let (merged, _) = merge(&[], &[da, db], &t2);
        assert_eq!(merged, items(&[("1", json!("x2"))]));
    }

    #[test]
    fn duplicate_local_ids_keep_the_first() {
        let (doc, _) = publish("a", &items(&[("1", json!("first")), ("1", json!("second"))]), &Tracker::default(), 1);
        assert_eq!(doc.items["1"].value, Some(json!("first")));
        let (merged, _) = merge(&items(&[("1", json!("first")), ("1", json!("second"))]), &[doc], &Tracker::default());
        assert_eq!(merged, items(&[("1", json!("first"))]));
    }

    #[test]
    fn documents_round_trip_through_json() {
        let (doc, tracker) = publish("a", &items(&[("1", json!({"k": 1}))]), &Tracker::default(), 7);
        let back: DeviceDoc = serde_json::from_str(&serde_json::to_string(&doc).unwrap()).unwrap();
        assert_eq!(back, doc);
        let back: Tracker = serde_json::from_str(&serde_json::to_string(&tracker).unwrap()).unwrap();
        assert_eq!(back, tracker);
    }

    fn rec(modified: i64, v: Value) -> Record {
        Record { modified, deleted: false, value: Some(v), pristine: false, adopted: false }
    }

    #[test]
    fn an_unparsed_item_missing_from_the_snapshot_is_not_published_as_deleted() {
        let remote = DeviceDoc {
            device: "b".into(),
            items: BTreeMap::from([("x".to_string(), rec(5000, json!({"kind": "RichText"})))]),
            first_sync_at: None,
            name: None,
            run_at: None,
            gc_at: None,
            written_at: None,
            acks: Default::default(),
        };
        let (_, mut t) = merge_as("a", &[], &[remote], &Tracker { seeded: true, ..Default::default() });
        assert!(t.items.contains_key("x"));
        mark_unparsed(&mut t, &["x".to_string()]);
        let (doc, t2) = publish("a", &[], &t, 9000);
        assert!(!doc.items.contains_key("x"), "no tombstone and no rewrite: {:?}", doc.items.get("x"));
        assert!(t2.items["x"].unparsed && !t2.items["x"].deleted, "still remembered as unparsed");
    }

    #[test]
    fn an_item_that_is_parsed_later_publishes_normally_and_a_remote_deletion_forgets_it() {
        let remote = DeviceDoc {
            device: "b".into(),
            items: BTreeMap::from([("x".to_string(), rec(5000, json!(1)))]),
            first_sync_at: None,
            name: None,
            run_at: None,
            gc_at: None,
            written_at: None,
            acks: Default::default(),
        };
        let (_, mut t) = merge_as("a", &[], std::slice::from_ref(&remote), &Tracker { seeded: true, ..Default::default() });
        mark_unparsed(&mut t, &["x".to_string()]);
        // a newer build reads it: it is in the local items, flag gone, the unchanged value keeps its time
        let (doc, t2) = publish("a", &items(&[("x", json!(1))]), &t, 9000);
        assert_eq!(doc.items["x"].modified, 5000);
        assert!(!t2.items["x"].unparsed);
        // the other Mac deletes it: nothing is carried
        let gone = DeviceDoc {
            device: "b".into(),
            items: BTreeMap::from([(
                "x".to_string(),
                Record { modified: 7000, deleted: true, value: None, pristine: false, adopted: false },
            )]),
            first_sync_at: None,
            name: None,
            run_at: None,
            gc_at: None,
            written_at: None,
            acks: Default::default(),
        };
        let (_, t3) = merge_as("a", &[], &[gone], &t);
        let (doc, _) = publish("a", &[], &t3, 9000);
        assert!(!doc.items.contains_key("x"), "A publishes nothing for x: no tombstone of its own, no copy: {:?}", doc.items.get("x"));
        assert!(!t3.items.get("x").is_some_and(|t| t.unparsed && !t.deleted), "a deleted unparsed item is not kept live");
    }

    #[test]
    fn tracked_unparsed_round_trips_and_false_is_not_written() {
        let mut t = Tracked {
            hash: "h".into(),
            modified: 1,
            deleted: false,
            remote: false,
            pristine: false,
            adopted: false,
            unparsed: true,
            seen: None,
            base: None,
        };
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains("\"unparsed\":true"), "{json}");
        assert!(serde_json::from_str::<Tracked>(&json).unwrap().unparsed);
        t.unparsed = false;
        let json = serde_json::to_string(&t).unwrap();
        assert!(!json.contains("unparsed"), "{json}");
        assert!(!serde_json::from_str::<Tracked>(&json).unwrap().unparsed, "an old tracker loads as false");
    }

    #[test]
    fn a_normal_local_deletion_and_edit_still_publish_as_before() {
        let (_, t1) = publish("a", &items(&[("1", json!("x")), ("2", json!("y"))]), &Tracker::default(), 100);
        let (doc, _) = publish("a", &items(&[("2", json!("y2"))]), &t1, 200);
        assert!(doc.items["1"].deleted && doc.items["1"].modified == 200);
        assert_eq!(doc.items["2"].modified, 200);
    }
}
