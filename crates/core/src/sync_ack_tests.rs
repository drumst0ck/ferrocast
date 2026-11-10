//! Acknowledgement-based clean-up of deletions (S6): scenarios with simulated Macs, each with its
//! own explicit clock.
use super::*;
use serde_json::json;
use std::collections::BTreeMap;

const D: i64 = 24 * 3600 * 1000;
const H: i64 = 3600 * 1000;
const T0: i64 = 1_000_000_000_000;
const DOMAIN: &str = "themes";

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ferrocast-ack-{tag}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// One simulated Mac. Its clock is `skew` ahead of the shared test time.
struct Mac {
    data: PathBuf,
    io: SyncIo,
    items: Vec<(String, Value)>,
    skew: i64,
}

impl Mac {
    fn new(id: &str, shared: &Path) -> Mac {
        let data = tmp(id);
        let io = SyncIo::new(shared.to_path_buf(), id.to_string(), data.join("images"));
        Mac { data, io, items: vec![], skew: 0 }
    }
    fn skewed(mut self, skew: i64) -> Mac {
        self.skew = skew;
        self
    }
    /// A run as the app does it: apply a merge, write the own document after applying it, save the tracker.
    fn sync(&mut self, now: i64) {
        let now = now + self.skew;
        let o = sync_domain(&self.io, DOMAIN, &self.items, &load_tracker(&self.data, DOMAIN), now).unwrap();
        save_tracker(&self.data, DOMAIN, &o.tracker_merged).unwrap();
        if let Some(m) = o.merged {
            self.items = m;
            self.io.write_doc(DOMAIN, o.applied_doc.as_ref().unwrap()).unwrap();
        }
    }
    fn put(&mut self, id: &str, v: Value) {
        self.items.retain(|(i, _)| i != id);
        self.items.push((id.into(), v));
    }
    fn del(&mut self, id: &str) {
        self.items.retain(|(i, _)| i != id);
    }
    fn has(&self, id: &str) -> bool {
        self.items.iter().any(|(i, _)| i == id)
    }
    fn doc(&self) -> DeviceDoc {
        serde_json::from_slice(&std::fs::read(self.io.dir.join("devices").join(&self.io.device).join("themes.json")).unwrap()).unwrap()
    }
    fn tomb(&self, id: &str) -> bool {
        self.doc().items.get(id).is_some_and(|r| r.deleted)
    }
    fn tracker(&self) -> Tracker {
        load_tracker(&self.data, DOMAIN)
    }
}

/// A, B and C (and D when asked) all hold item `x`, settled.
fn settled(shared: &Path, ids: &[&str]) -> Vec<Mac> {
    let mut macs: Vec<Mac> = ids.iter().map(|i| Mac::new(i, shared)).collect();
    for m in macs.iter_mut() {
        m.put("x", json!({"v": 1}));
        m.put("keep", json!({"v": 2}));
    }
    for round in 0..3 {
        for (k, m) in macs.iter_mut().enumerate() {
            m.sync(T0 + round * D + k as i64 * H);
        }
    }
    macs
}

fn cleanup(shared: &Path, macs: &[&Mac]) {
    let _ = std::fs::remove_dir_all(shared);
    for m in macs {
        let _ = std::fs::remove_dir_all(&m.data);
    }
}

#[test]
fn a_deletion_is_collected_only_after_every_mac_has_acknowledged_it() {
    let shared = tmp("shared");
    let mut v = settled(&shared, &["aa", "bb", "cc"]);
    let (mut c, mut b, mut a) = (v.pop().unwrap(), v.pop().unwrap(), v.pop().unwrap());
    let t = T0 + 10 * D;
    a.del("x");
    a.sync(t);
    assert!(a.tomb("x"));
    b.sync(t + H);
    assert!(!b.has("x"), "B received the deletion");
    a.sync(t + 2 * H);
    assert!(a.tomb("x"), "only B has acknowledged: C still blocks");
    c.sync(t + 3 * H);
    assert!(!c.has("x"));
    a.sync(t + 4 * H);
    assert!(!a.tomb("x"), "everybody acknowledged: A drops its tombstone");
    assert!(a.tracker().collected.contains_key("x") && !a.tracker().items.contains_key("x"));
    assert!(a.doc().items.contains_key("keep") && !a.doc().items.contains_key("x"));
    assert!(!a.has("x") && !b.has("x") && !c.has("x"));
    cleanup(&shared, &[&a, &b, &c]);
}

#[test]
fn an_offline_mac_blocks_collection_and_still_receives_the_deletion_after_31_days() {
    let shared = tmp("shared");
    let mut v = settled(&shared, &["aa", "bb", "cc"]);
    let (mut c, mut b, mut a) = (v.pop().unwrap(), v.pop().unwrap(), v.pop().unwrap());
    let t = T0 + 10 * D;
    a.del("x");
    for day in 0..45 {
        a.sync(t + day * D);
        b.sync(t + day * D + H);
        assert!(a.tomb("x") && b.tomb("x"), "day {day}: C has not acknowledged, nothing is collected");
    }
    assert!(c.has("x"), "C was away and still holds it, and nothing was purged there");
    c.sync(t + 46 * D);
    assert!(!c.has("x"), "the returning Mac receives the deletion");
    assert!(c.has("keep"), "and keeps everything else, also what it never uploaded");
    a.sync(t + 46 * D + H);
    b.sync(t + 46 * D + 2 * H);
    a.sync(t + 46 * D + 3 * H);
    assert!(!a.tomb("x"), "now that C acknowledged, A collects");
    cleanup(&shared, &[&a, &b, &c]);
}

#[test]
fn a_returning_mac_keeps_notes_it_never_uploaded() {
    let shared = tmp("shared");
    let mut v = settled(&shared, &["aa", "bb"]);
    let (mut b, mut a) = (v.pop().unwrap(), v.pop().unwrap());
    let t = T0 + 10 * D;
    b.put("only-on-b", json!({"v": 9})); // written while B was offline for the others
    a.del("x");
    a.sync(t);
    for day in 1..40 {
        a.sync(t + day * D);
    }
    b.sync(t + 40 * D);
    assert!(b.has("only-on-b") && !b.has("x"));
    a.sync(t + 40 * D + H);
    assert!(a.has("only-on-b"), "published by B, received by A");
    cleanup(&shared, &[&a, &b]);
}

#[test]
fn a_fast_or_slow_clock_cannot_cause_early_collection() {
    let shared = tmp("shared");
    let mut v = settled(&shared, &["aa", "bb", "cc"]);
    // B runs ten days ahead, C ten days behind
    let (mut c, mut b, mut a) = (v.pop().unwrap().skewed(-10 * D), v.pop().unwrap().skewed(10 * D), v.pop().unwrap());
    let t = T0 + 20 * D;
    // both write documents (with their own clocks) before A deletes anything
    b.sync(t);
    c.sync(t + H);
    a.del("x");
    a.sync(t + 2 * H);
    a.sync(t + 3 * H);
    assert!(a.tomb("x"), "their documents were written after A's deletion in A's time, but never read it");
    b.sync(t + 4 * H);
    a.sync(t + 5 * H);
    assert!(a.tomb("x"), "C has not read the deletion yet");
    c.sync(t + 6 * H);
    a.sync(t + 7 * H);
    assert!(!a.tomb("x"), "collected once both really read it, whatever their clocks say");
    cleanup(&shared, &[&a, &b, &c]);
}

#[test]
fn a_clock_set_back_on_the_deleting_mac_does_not_let_old_acknowledgements_count() {
    let shared = tmp("shared");
    let mut v = settled(&shared, &["aa", "bb"]);
    let (mut b, mut a) = (v.pop().unwrap(), v.pop().unwrap());
    let t = T0 + 20 * D;
    a.sync(t);
    b.sync(t + H); // B acknowledges A's document written at about `t`
    let before = a.doc().written_at.unwrap();
    a.del("x");
    a.skew = -15 * D; // A's clock jumps back half a month
    a.sync(t + 2 * H);
    assert!(a.doc().written_at.unwrap() > before, "A's own clock value never goes back");
    a.sync(t + 3 * H);
    assert!(a.tomb("x"), "B's acknowledgement predates the deletion");
    b.sync(t + 4 * H);
    a.sync(t + 5 * H);
    assert!(!a.tomb("x"));
    cleanup(&shared, &[&a, &b]);
}

fn write_legacy_peer(shared: &Path, id: &str) {
    let dir = shared.join("devices").join(id);
    std::fs::create_dir_all(&dir).unwrap();
    // a v0.8.0 document: no written_at, no acks
    let doc = r#"{"device":"old","items":{"x":{"modified":0,"value":{"v":1}}},"first_sync_at":5,"name":"Old Mac","run_at":99}"#;
    std::fs::write(dir.join("themes.json"), doc).unwrap();
}

#[test]
fn a_legacy_document_without_acks_blocks_until_that_mac_is_forgotten() {
    let shared = tmp("shared");
    let mut v = settled(&shared, &["aa", "bb"]);
    let (mut b, mut a) = (v.pop().unwrap(), v.pop().unwrap());
    write_legacy_peer(&shared, "oldmac");
    let t = T0 + 10 * D;
    a.sync(t);
    b.sync(t + H);
    a.del("x");
    for k in 0..6 {
        a.sync(t + 2 * H + k * 2 * H);
        b.sync(t + 3 * H + k * 2 * H);
    }
    assert!(a.tomb("x") && b.tomb("x"), "a Mac that never acknowledges blocks collection");
    a.io.forget_device("oldmac").unwrap();
    b.sync(t + D);
    a.sync(t + D + H);
    b.sync(t + D + 2 * H);
    a.sync(t + D + 3 * H);
    assert!(!a.tomb("x"), "forgetting the blocker lets the clean-up proceed");
    cleanup(&shared, &[&a, &b]);
}

#[test]
fn a_mac_folder_without_a_document_of_the_domain_blocks_too() {
    let shared = tmp("shared");
    let mut v = settled(&shared, &["aa", "bb"]);
    let (mut b, mut a) = (v.pop().unwrap(), v.pop().unwrap());
    std::fs::create_dir_all(shared.join("devices/empty")).unwrap();
    a.del("x");
    for k in 0..4 {
        a.sync(T0 + 10 * D + k * 2 * H);
        b.sync(T0 + 10 * D + k * 2 * H + H);
    }
    assert!(a.tomb("x"));
    cleanup(&shared, &[&a, &b]);
}

#[test]
fn a_single_mac_collects_nothing() {
    let shared = tmp("shared");
    let mut a = Mac::new("aa", &shared);
    a.put("x", json!(1));
    a.sync(T0);
    a.del("x");
    for k in 1..10 {
        a.sync(T0 + k * D);
    }
    assert!(a.tomb("x"));
    cleanup(&shared, &[&a]);
}

#[test]
fn an_old_copy_cannot_resurrect_a_collected_item_but_a_newer_edit_wins() {
    let shared = tmp("shared");
    let mut v = settled(&shared, &["aa", "bb"]);
    let (mut b, mut a) = (v.pop().unwrap(), v.pop().unwrap());
    let t = T0 + 10 * D;
    a.del("x");
    a.sync(t);
    b.sync(t + H);
    a.sync(t + 2 * H);
    assert!(!a.tomb("x"), "collected");
    let stamp = a.tracker().collected["x"];
    // an old copy comes back: a document restored from a backup, stamped before the deletion
    let dir = shared.join("devices/zombie");
    std::fs::create_dir_all(&dir).unwrap();
    let old = |m: i64| format!(r#"{{"device":"zombie","items":{{"x":{{"modified":{m},"value":{{"v":1}}}}}}}}"#);
    std::fs::write(dir.join("themes.json"), old(stamp - 5)).unwrap();
    a.sync(t + 3 * H);
    assert!(!a.has("x"), "older than the deletion: ignored");
    assert!(!a.doc().items.contains_key("x"));
    std::fs::write(dir.join("themes.json"), old(stamp)).unwrap();
    a.sync(t + 4 * H);
    assert!(!a.has("x"), "equal to the deletion: ignored");
    // an edit made after the deletion is an edit
    std::fs::write(dir.join("themes.json"), old(stamp + 1)).unwrap();
    a.sync(t + 5 * H);
    assert!(a.has("x"), "a newer record wins as an edit");
    b.sync(t + 6 * H);
    assert!(b.has("x"));
    cleanup(&shared, &[&a, &b]);
}

#[test]
fn a_collected_item_recreated_here_is_stamped_after_the_collected_deletion() {
    let shared = tmp("shared");
    let mut v = settled(&shared, &["aa", "bb"]);
    let (mut b, mut a) = (v.pop().unwrap(), v.pop().unwrap());
    let t = T0 + 10 * D;
    a.del("x");
    a.sync(t);
    b.sync(t + H);
    a.sync(t + 2 * H);
    let stamp = a.tracker().collected["x"];
    a.skew = -50 * D; // clock far behind the deletion's stamp
    a.put("x", json!({"v": 7}));
    a.sync(t + 3 * H);
    assert!(a.doc().items["x"].modified > stamp, "never stamped at or before the collected tombstone");
    b.sync(t + 4 * H);
    assert_eq!(b.items.iter().find(|(i, _)| i == "x").map(|(_, v)| v.clone()), Some(json!({"v": 7})));
    cleanup(&shared, &[&a, &b]);
}

#[test]
fn a_relayed_deletion_is_collected_by_the_relaying_mac_by_the_same_rule() {
    let shared = tmp("shared");
    let mut v = settled(&shared, &["aa", "bb", "cc"]);
    let (mut c, mut b, mut a) = (v.pop().unwrap(), v.pop().unwrap(), v.pop().unwrap());
    let t = T0 + 10 * D;
    a.del("x");
    a.sync(t);
    b.sync(t + H);
    assert!(b.tomb("x"), "B relays A's deletion");
    c.sync(t + 2 * H);
    // B is acknowledged by A and C only after they read B's document
    b.sync(t + 3 * H);
    assert!(b.tomb("x"), "A and C have not acknowledged B's document yet");
    a.sync(t + 4 * H);
    c.sync(t + 5 * H);
    b.sync(t + 6 * H);
    assert!(!b.tomb("x"), "every other Mac acknowledged B's relay");
    assert!(b.tracker().collected.contains_key("x"));
    assert!(!b.has("x"));
    cleanup(&shared, &[&a, &b, &c]);
}

#[test]
fn built_in_default_deletions_are_never_collected() {
    let shared = tmp("shared");
    let default_id = crate::storage::default_snippets().remove(0).id;
    let mut a = Mac::new("aa", &shared);
    let mut b = Mac::new("bb", &shared);
    let run = |m: &mut Mac, now: i64| {
        let o = sync_domain(&m.io, "snippets", &[], &load_tracker(&m.data, "snippets"), now).unwrap();
        save_tracker(&m.data, "snippets", &o.tracker_merged).unwrap();
    };
    for k in 0..6 {
        run(&mut a, T0 + k * 2 * H);
        run(&mut b, T0 + k * 2 * H + H);
    }
    let doc: DeviceDoc = serde_json::from_slice(&std::fs::read(shared.join("devices/aa/snippets.json")).unwrap()).unwrap();
    assert!(doc.items[&default_id].deleted, "the deletion of a built-in default stays published");
    cleanup(&shared, &[&a, &b]);
}

#[test]
fn documents_carry_written_at_and_acks_and_unchanged_documents_are_not_rewritten() {
    let shared = tmp("shared");
    let mut v = settled(&shared, &["aa", "bb"]);
    let (mut b, mut a) = (v.pop().unwrap(), v.pop().unwrap());
    let t = T0 + 10 * D;
    a.sync(t);
    b.sync(t + H);
    a.sync(t + 2 * H);
    let doc = a.doc();
    assert!(doc.written_at.is_some());
    assert_eq!(doc.acks.keys().collect::<Vec<_>>(), ["bb"]);
    let path = shared.join("devices/aa/themes.json");
    b.sync(t + 3 * H);
    a.sync(t + 4 * H); // reads B's new document: acks change, written_at moves on
    let after = a.doc();
    assert!(after.written_at.unwrap() >= doc.written_at.unwrap());
    a.sync(t + 5 * H);
    a.sync(t + 6 * H);
    assert_eq!(a.doc().written_at, after.written_at, "nothing changed: written_at stays");
    assert!(path.is_file());
    cleanup(&shared, &[&a, &b]);
}

#[test]
fn the_clipboard_document_carries_no_acknowledgements() {
    let shared = tmp("shared");
    let mut a = Mac::new("aa", &shared);
    let b = Mac::new("bb", &shared);
    let o = sync_domain(&a.io, "clipboard", &[], &Tracker::default(), T0).unwrap();
    save_tracker(&a.data, "clipboard", &o.tracker_merged).unwrap();
    let doc: DeviceDoc = serde_json::from_slice(&std::fs::read(shared.join("devices/aa/clipboard.json")).unwrap()).unwrap();
    assert!(doc.written_at.is_none() && doc.acks.is_empty());
    a.skew = 0;
    cleanup(&shared, &[&a, &b]);
}

#[test]
fn documents_stay_compatible_with_v0_8_0_both_ways() {
    // a v0.8.0 reader (no such fields) reads a new document
    #[derive(serde::Deserialize)]
    struct Old {
        device: String,
        items: BTreeMap<String, sync::Record>,
        #[serde(default)]
        run_at: Option<i64>,
        #[serde(default)]
        gc_at: Option<i64>,
    }
    let shared = tmp("shared");
    let mut v = settled(&shared, &["aa", "bb"]);
    let (b, a) = (v.pop().unwrap(), v.pop().unwrap());
    let raw = std::fs::read(shared.join("devices/aa/themes.json")).unwrap();
    assert!(String::from_utf8_lossy(&raw).contains("written_at"));
    let old: Old = serde_json::from_slice(&raw).unwrap();
    assert!(old.items.contains_key("x") && old.device == "aa" && old.run_at.is_some() && old.gc_at.is_none());
    // a new reader reads a v0.8.0 tracker and document
    let t: Tracker = serde_json::from_str(r#"{"items":{"1":{"hash":"h","modified":5,"deleted":true}},"seeded":true}"#).unwrap();
    assert!(t.first_pub.is_empty() && t.collected.is_empty());
    let d: DeviceDoc = serde_json::from_str(r#"{"device":"o","items":{},"run_at":3,"gc_at":4}"#).unwrap();
    assert!(d.written_at.is_none() && d.acks.is_empty());
    // a tracker from before this change collects nothing at once (no `first_pub`), and works on
    let mut c = Mac::new("cc", &shared);
    c.sync(T0 + 9 * D);
    assert!(c.has("x"));
    cleanup(&shared, &[&a, &b, &c]);
}

#[test]
fn a_tombstone_from_a_v0_8_0_build_is_collected_only_after_acks_of_documents_written_by_this_build() {
    let shared = tmp("shared");
    let mut v = settled(&shared, &["aa", "bb"]);
    let (mut b, mut a) = (v.pop().unwrap(), v.pop().unwrap());
    let t = T0 + 10 * D;
    a.del("x");
    a.sync(t);
    // strip the new fields from A's document and tracker, as a v0.8.0 build left them
    let p = shared.join("devices/aa/themes.json");
    let mut doc: DeviceDoc = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
    doc.written_at = None;
    doc.acks.clear();
    std::fs::write(&p, serde_json::to_vec(&doc).unwrap()).unwrap();
    let mut tr = a.tracker();
    tr.first_pub.clear();
    save_tracker(&a.data, DOMAIN, &tr).unwrap();
    // B's earlier acknowledgements (of the document before the deletion) must not count
    b.sync(t + H);
    a.sync(t + 2 * H);
    assert!(a.tomb("x"), "B read a document without written_at: no acknowledgement yet");
    b.sync(t + 3 * H);
    a.sync(t + 4 * H);
    assert!(!a.tomb("x"));
    cleanup(&shared, &[&a, &b]);
}

#[test]
fn a_conflict_copy_and_the_logical_clock_still_work_with_collection_on() {
    let shared = tmp("shared");
    let mut a = Mac::new("aa", &shared);
    let mut b = Mac::new("bb", &shared);
    let note = |text: &str| json!({"id": "n1", "text": text, "created_at": 1, "updated_at": 2});
    let run = |m: &mut Mac, now: i64| {
        let o = sync_domain(&m.io, "notes", &m.items, &load_tracker(&m.data, "notes"), now + m.skew).unwrap();
        save_tracker(&m.data, "notes", &o.tracker_merged).unwrap();
        if let Some(merged) = o.merged {
            m.items = merged;
            m.io.write_doc("notes", o.applied_doc.as_ref().unwrap()).unwrap();
        }
    };
    a.put("n1", note("start"));
    run(&mut a, T0);
    run(&mut b, T0 + H);
    run(&mut a, T0 + 2 * H);
    // both edit; B's clock runs a year behind but its edit still orders after what it received
    a.put("n1", note("edited on A"));
    b.skew = -365 * D;
    b.put("n1", note("edited on B"));
    run(&mut a, T0 + 3 * H);
    run(&mut b, T0 + 4 * H);
    run(&mut a, T0 + 5 * H);
    run(&mut b, T0 + 6 * H);
    let texts: Vec<String> = a.items.iter().filter_map(|(_, v)| v["text"].as_str().map(String::from)).collect();
    assert!(texts.iter().any(|t| t.contains("conflict copy")), "{texts:?}");
    assert_eq!(a.items.len(), b.items.len());
    // a logical-clock edit made after receiving a record stamped in the future still wins
    let future = T0 + 400 * D;
    a.put("n1", note("from the future"));
    run(&mut a, future);
    b.skew = 0;
    run(&mut b, T0 + 7 * H); // B receives the record stamped in the future
    b.put("n1", note("after the future"));
    run(&mut b, T0 + 8 * H);
    run(&mut a, T0 + 9 * H);
    let last = a.items.iter().find(|(i, _)| i == "n1").unwrap().1["text"].as_str().unwrap().to_string();
    assert_eq!(last, "after the future");
    cleanup(&shared, &[&a, &b]);
}

#[test]
fn a_document_written_later_never_has_a_lower_written_at_and_equal_values_mean_equal_content() {
    let shared = tmp("shared");
    let a = Mac::new("aa", &shared);
    let doc = |w: i64, v: i64| DeviceDoc {
        device: "aa".into(),
        items: [("k".to_string(), sync::Record { modified: 1, deleted: false, value: Some(json!(v)), pristine: false, adopted: false })]
            .into(),
        written_at: Some(w),
        ..Default::default()
    };
    assert_eq!(a.io.write_doc(DOMAIN, &doc(100, 1)).unwrap(), Some(100));
    // a delayed older document (a retried write of an earlier merge) with other content
    assert_eq!(a.io.write_doc(DOMAIN, &doc(50, 2)).unwrap(), Some(101), "moves on instead of going back");
    // the same content again keeps the value, so nobody can have read different content under it
    assert_eq!(a.io.write_doc(DOMAIN, &doc(500, 2)).unwrap(), Some(101));
    assert_eq!(a.doc().written_at, Some(101));
    cleanup(&shared, &[&a]);
}
