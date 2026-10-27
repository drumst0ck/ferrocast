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
}

/// What one device publishes for one domain.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct DeviceDoc {
    pub device: String,
    pub items: BTreeMap<String, Record>,
}

/// Local memory of the last published state, per item id: (content hash, modified, deleted).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Tracker {
    pub items: BTreeMap<String, Tracked>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tracked {
    pub hash: String,
    pub modified: i64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deleted: bool,
}

/// Tombstones older than this are dropped from documents (90 days): every Mac that syncs in that
/// time has seen the deletion.
pub const TOMBSTONE_TTL_MS: i64 = 90 * 24 * 3600 * 1000;

pub fn hash(value: &Value) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Record the current local items: changed or new items get `now`, missing ones become tombstones at
/// `now`, unchanged ones keep their time. Returns this device's document and the new tracker.
pub fn publish(device: &str, items: &[(String, Value)], prev: &Tracker, now: i64) -> (DeviceDoc, Tracker) {
    let mut doc = DeviceDoc { device: device.to_string(), items: BTreeMap::new() };
    let mut next = Tracker::default();
    let mut seen = HashSet::new();
    for (id, value) in items {
        if !seen.insert(id.clone()) {
            continue; // duplicate id: the first one wins
        }
        let h = hash(value);
        let modified = match prev.items.get(id) {
            Some(t) if !t.deleted && t.hash == h => t.modified,
            _ => now,
        };
        doc.items.insert(id.clone(), Record { modified, deleted: false, value: Some(value.clone()) });
        next.items.insert(id.clone(), Tracked { hash: h, modified, deleted: false });
    }
    for (id, t) in &prev.items {
        if seen.contains(id) {
            continue;
        }
        let modified = if t.deleted { t.modified } else { now };
        if now - modified > TOMBSTONE_TTL_MS {
            continue;
        }
        doc.items.insert(id.clone(), Record { modified, deleted: true, value: None });
        next.items.insert(id.clone(), Tracked { hash: t.hash.clone(), modified, deleted: true });
    }
    (doc, next)
}

/// The winning record for every id across all documents: newest `modified`, then the greater device
/// id, so every Mac picks the same winner.
pub fn winners(docs: &[DeviceDoc]) -> BTreeMap<String, (String, Record)> {
    let mut out: BTreeMap<String, (String, Record)> = BTreeMap::new();
    for doc in docs {
        for (id, rec) in &doc.items {
            let better = match out.get(id) {
                None => true,
                Some((dev, cur)) => (rec.modified, doc.device.as_str()) > (cur.modified, dev.as_str()),
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
    let win = winners(docs);
    let mut out: Vec<(String, Value)> = Vec::new();
    let mut next = tracker.clone();
    let mut placed = HashSet::new();
    for (id, value) in local {
        if !placed.insert(id.clone()) {
            continue;
        }
        match win.get(id) {
            Some((_, rec)) if rec.deleted => {
                // Deleted elsewhere after our last edit.
                if local_is_newer(tracker, id, value, rec.modified) {
                    out.push((id.clone(), value.clone()));
                } else {
                    next.items.insert(id.clone(), Tracked { hash: hash(value), modified: rec.modified, deleted: true });
                }
            }
            Some((_, rec)) => {
                let theirs = rec.value.clone().unwrap_or(Value::Null);
                if local_is_newer(tracker, id, value, rec.modified) || hash(&theirs) == hash(value) {
                    out.push((id.clone(), value.clone()));
                } else {
                    next.items.insert(id.clone(), Tracked { hash: hash(&theirs), modified: rec.modified, deleted: false });
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
            if t.deleted && t.modified >= rec.modified {
                continue;
            }
        }
        let v = rec.value.clone().unwrap_or(Value::Null);
        next.items.insert(id.clone(), Tracked { hash: hash(&v), modified: rec.modified, deleted: false });
        out.push((id.clone(), v));
    }
    (out, next)
}

/// True when this Mac edited `id` after `their_modified`: the local value differs from what the
/// tracker last published (an unpublished edit counts as now) or was published later.
fn local_is_newer(tracker: &Tracker, id: &str, value: &Value, their_modified: i64) -> bool {
    match tracker.items.get(id) {
        Some(t) if t.hash == hash(value) && !t.deleted => t.modified > their_modified,
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
        assert_eq!(doc.items["1"], Record { modified: 200, deleted: true, value: None });
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
    fn the_newest_edit_wins_and_ties_go_to_the_greater_device() {
        let a = DeviceDoc { device: "a".into(), items: [("1".to_string(), Record { modified: 5, deleted: false, value: Some(json!("a")) })].into() };
        let b = DeviceDoc { device: "b".into(), items: [("1".to_string(), Record { modified: 5, deleted: false, value: Some(json!("b")) })].into() };
        let c = DeviceDoc { device: "c".into(), items: [("1".to_string(), Record { modified: 4, deleted: false, value: Some(json!("c")) })].into() };
        assert_eq!(winners(&[a.clone(), b.clone(), c.clone()])["1"].1.value, Some(json!("b")));
        assert_eq!(winners(&[c, b, a])["1"].1.value, Some(json!("b")), "order of documents does not matter");
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
        db.items.insert("1".into(), Record { modified: 200, deleted: false, value: Some(json!("new")) });
        let (merged, t) = merge(&items(&[("1", json!("old"))]), &[da, db], &ta);
        assert_eq!(merged, items(&[("1", json!("new"))]));
        let (doc, _) = publish("a", &merged, &t, 300);
        assert_eq!(doc.items["1"].modified, 200, "the merged value is not republished as a local edit");
    }

    #[test]
    fn an_unpublished_local_edit_beats_an_older_remote_one() {
        let (da, ta) = publish("a", &items(&[("1", json!("v1"))]), &Tracker::default(), 100);
        let db = DeviceDoc { device: "b".into(), items: [("1".to_string(), Record { modified: 150, deleted: false, value: Some(json!("theirs")) })].into() };
        // Local edit made after the last publish, not yet published.
        let (merged, _) = merge(&items(&[("1", json!("mine"))]), &[da, db], &ta);
        assert_eq!(merged, items(&[("1", json!("mine"))]));
    }

    #[test]
    fn a_remote_deletion_newer_than_the_local_edit_removes_the_item() {
        let (da, ta) = publish("a", &items(&[("1", json!("x")), ("2", json!("y"))]), &Tracker::default(), 100);
        let db = DeviceDoc { device: "b".into(), items: [("1".to_string(), Record { modified: 200, deleted: true, value: None })].into() };
        let (merged, t) = merge(&items(&[("1", json!("x")), ("2", json!("y"))]), &[da, db], &ta);
        assert_eq!(merged, items(&[("2", json!("y"))]));
        let (doc, _) = publish("a", &merged, &t, 300);
        assert_eq!(doc.items["1"], Record { modified: 200, deleted: true, value: None }, "the tombstone is kept, not renewed");
    }

    #[test]
    fn a_local_deletion_is_not_undone_by_an_older_remote_copy() {
        let (_, t1) = publish("a", &items(&[("1", json!("x"))]), &Tracker::default(), 100);
        let (da, t2) = publish("a", &[], &t1, 200);
        let db = DeviceDoc { device: "b".into(), items: [("1".to_string(), Record { modified: 100, deleted: false, value: Some(json!("x")) })].into() };
        let (merged, _) = merge(&[], &[da, db], &t2);
        assert!(merged.is_empty());
    }

    #[test]
    fn an_edit_elsewhere_after_a_local_deletion_brings_the_item_back() {
        let (_, t1) = publish("a", &items(&[("1", json!("x"))]), &Tracker::default(), 100);
        let (da, t2) = publish("a", &[], &t1, 200);
        let db = DeviceDoc { device: "b".into(), items: [("1".to_string(), Record { modified: 300, deleted: false, value: Some(json!("x2")) })].into() };
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
}
