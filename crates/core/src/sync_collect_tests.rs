//! Complete deletion clean-up (S6, owner-decisions-2 L26): the clock and acknowledgements survive a
//! lost or restored Mac, and a collected deletion cannot be brought back by a stale copy.
use super::*;
use serde_json::json;

const D: i64 = 24 * 3600 * 1000;
const H: i64 = 3600 * 1000;
const T0: i64 = 1_000_000_000_000;
const DOMAIN: &str = "themes";

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ferrocast-col-{tag}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

struct Mac {
    id: String,
    data: PathBuf,
    io: SyncIo,
    items: Vec<(String, Value)>,
    skew: i64,
}

/// What a backup of a Mac's local data holds (the sync folder is not part of it).
struct Backup {
    items: Vec<(String, Value)>,
    tracker: Vec<u8>,
}

impl Mac {
    fn new(id: &str, shared: &Path) -> Mac {
        let data = tmp(id);
        let io = SyncIo::new(shared.to_path_buf(), id.to_string(), data.join("images"));
        Mac { id: id.into(), data, io, items: vec![], skew: 0 }
    }
    fn sync(&mut self, now: i64) {
        let o = sync_domain(&self.io, DOMAIN, &self.items, &load_tracker(&self.data, DOMAIN), now + self.skew).unwrap();
        save_tracker(&self.data, DOMAIN, &o.tracker_merged).unwrap();
        if let Some(m) = o.merged {
            self.items = m;
            self.io.write_doc(DOMAIN, o.applied_doc.as_ref().unwrap()).unwrap();
        }
    }
    /// A run whose result is never applied or saved (the app quit right after it published).
    fn sync_lost(&mut self, now: i64) {
        let _ = sync_domain(&self.io, DOMAIN, &self.items, &load_tracker(&self.data, DOMAIN), now + self.skew).unwrap();
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
    fn value(&self, id: &str) -> Option<Value> {
        self.items.iter().find(|(i, _)| i == id).map(|(_, v)| v.clone())
    }
    fn doc_path(&self) -> PathBuf {
        self.io.dir.join("devices").join(&self.id).join("themes.json")
    }
    fn doc(&self) -> DeviceDoc {
        serde_json::from_slice(&std::fs::read(self.doc_path()).unwrap()).unwrap()
    }
    fn raw(&self) -> Value {
        serde_json::from_slice(&std::fs::read(self.doc_path()).unwrap()).unwrap()
    }
    fn tomb(&self, id: &str) -> bool {
        self.doc().items.get(id).is_some_and(|r| r.deleted)
    }
    fn tracker(&self) -> Tracker {
        load_tracker(&self.data, DOMAIN)
    }
    fn tracker_path(&self) -> PathBuf {
        self.data.join("sync-state").join("themes.json")
    }
    fn backup(&self) -> Backup {
        Backup { items: self.items.clone(), tracker: std::fs::read(self.tracker_path()).unwrap() }
    }
    fn restore(&mut self, b: &Backup) {
        self.items = b.items.clone();
        std::fs::write(self.tracker_path(), &b.tracker).unwrap();
    }
}

fn settled(shared: &Path, ids: &[&str]) -> Vec<Mac> {
    let mut macs: Vec<Mac> = ids.iter().map(|i| Mac::new(i, shared)).collect();
    for m in macs.iter_mut() {
        m.put("x", json!({"v": 1}));
        m.put("keep", json!({"v": 2}));
    }
    rounds(&mut macs, T0, 3);
    macs
}

/// `n` rounds in which every Mac runs once, a day after the previous round.
fn rounds(macs: &mut [Mac], t: i64, n: i64) {
    for round in 0..n {
        for (k, m) in macs.iter_mut().enumerate() {
            m.sync(t + round * D + k as i64 * H);
        }
    }
}

fn cleanup(shared: &Path, macs: &[&Mac]) {
    let _ = std::fs::remove_dir_all(shared);
    for m in macs {
        let _ = std::fs::remove_dir_all(&m.data);
    }
}

fn zombie(shared: &Path, id: &str, modified: i64) {
    let dir = shared.join("devices").join(id);
    std::fs::create_dir_all(&dir).unwrap();
    let doc = format!(r#"{{"device":"{id}","items":{{"x":{{"modified":{modified},"value":{{"v":1}}}}}}}}"#);
    std::fs::write(dir.join("themes.json"), doc).unwrap();
}

#[test]
fn defect_a_a_lost_own_document_and_a_clock_set_back_do_not_make_old_acknowledgements_count() {
    let shared = tmp("shared");
    let mut v = settled(&shared, &["aa", "bb"]);
    let (mut b, mut a) = (v.pop().unwrap(), v.pop().unwrap());
    let t = T0 + 20 * D;
    a.put("y", json!({"v": 5}));
    a.sync(t);
    b.sync(t + H);
    a.sync(t + 2 * H);
    b.sync(t + 3 * H); // B has acknowledged A's latest documents
    let acked = b.doc().acks["aa"];
    std::fs::remove_file(a.doc_path()).unwrap(); // A's own document is lost
    a.skew = -30 * D; // and its clock is a month behind
    a.del("y");
    a.sync(t + 4 * H);
    assert!(a.doc().written_at.unwrap() > acked, "written_at stays above what the peers acknowledged");
    a.sync(t + 5 * H);
    assert!(a.tomb("y"), "B's acknowledgement belongs to an older document: no early collection");
    b.sync(t + 6 * H);
    assert!(!b.has("y"));
    a.sync(t + 7 * H);
    assert!(!a.tomb("y"), "collected once B really acknowledged the new document");
    cleanup(&shared, &[&a, &b]);
}

/// Run the named Macs once each, an hour apart starting at `t`.
fn run(macs: &mut [Mac], order: &[usize], t: i64) {
    for (k, i) in order.iter().enumerate() {
        macs[*i].sync(t + k as i64 * H);
    }
}

#[test]
fn defect_b_a_mac_restored_from_an_older_backup_does_not_publish_the_acknowledgements_of_its_newer_document() {
    let shared = tmp("shared");
    let mut m = settled(&shared, &["aa", "bb", "cc"]); // 0 = A, 1 = B, 2 = C
    let t = T0 + 20 * D;
    run(&mut m, &[0, 1, 2], t);
    let backup = m[2].backup(); // Time Machine copy of C's local data
    m[0].del("x");
    run(&mut m, &[0, 1, 2], t + D); // A deletes, B and C read it and acknowledge it
    m[2].restore(&backup); // C comes back from the backup: older data and tracker, newer document in the folder
    m[2].sync_lost(t + 2 * D); // it publishes once more, nothing of that run is applied or saved
    m[0].sync(t + 2 * D + H);
    assert!(m[0].tomb("x"), "C's older state still holds x: A must not collect");
    run(&mut m, &[2, 0, 1, 2, 0], t + 3 * D);
    assert!(!m[2].has("x") && !m[0].has("x") && !m[1].has("x"));
    let all: Vec<&Mac> = m.iter().collect();
    cleanup(&shared, &all);
}

/// A, B, C all hold `x`, A deletes it, and everybody runs until all of them have collected it.
fn collected_everywhere(shared: &Path, ids: &[&str], t: i64) -> Vec<Mac> {
    let mut m = settled(shared, ids);
    m[0].del("x");
    let all: Vec<usize> = (0..m.len()).collect();
    for k in 0..4 {
        run(&mut m, &all, t + k * D);
    }
    assert!(m.iter().all(|x| !x.tomb("x") && !x.has("x")), "everybody collected it");
    m
}

#[test]
fn defect_c_a_new_mac_does_not_adopt_a_stale_copy_of_a_collected_item_and_the_returning_mac_drops_it() {
    let shared = tmp("shared");
    let mut m = settled(&shared, &["aa", "bb", "cc"]);
    let stale = std::fs::read(m[2].doc_path()).unwrap(); // C's document while it still holds x
    let t = T0 + 20 * D;
    m[0].del("x");
    run(&mut m, &[0, 1], t);
    m[0].io.forget_device("cc").unwrap();
    run(&mut m, &[0, 1, 0, 1], t + D);
    assert!(m[0].tracker().collected.contains_key("x"), "the others collect without C");
    // C comes back and publishes its old copy before it has read anything
    std::fs::create_dir_all(shared.join("devices/cc")).unwrap();
    std::fs::write(m[2].doc_path(), &stale).unwrap();
    let mut d = Mac::new("dd", &shared);
    d.sync(t + 2 * D);
    assert!(!d.has("x"), "the new Mac does not adopt the stale copy");
    assert!(d.has("keep"));
    m[2].sync(t + 2 * D + H);
    assert!(!m[2].has("x"), "the returning Mac drops it");
    assert!(!m[2].doc().items.contains_key("x"), "and does not publish it");
    run(&mut m, &[0, 1], t + 3 * D);
    d.sync(t + 3 * D + H);
    assert!(!m[0].has("x") && !m[1].has("x") && !d.has("x"));
    cleanup(&shared, &[&m[0], &m[1], &m[2], &d]);
}

#[test]
fn forgetting_a_mac_publishes_its_last_acknowledged_document_in_forgotten() {
    let shared = tmp("shared");
    let m = settled(&shared, &["aa", "bb"]);
    let written = m[1].doc().written_at.unwrap();
    m[0].io.forget_device("bb").unwrap();
    assert_eq!(m[0].raw()["forgotten"]["bb"].as_i64(), Some(written));
    cleanup(&shared, &[&m[0], &m[1]]);
}

#[test]
fn a_mac_restored_from_an_older_backup_drops_a_collected_item() {
    let shared = tmp("shared");
    let mut m = settled(&shared, &["aa", "bb", "cc"]);
    let backup = m[2].backup();
    let t = T0 + 20 * D;
    m[0].del("x");
    let all = [0, 1, 2];
    for k in 0..4 {
        run(&mut m, &all, t + k * D);
    }
    assert!(m.iter().all(|x| !x.tomb("x")));
    m[2].restore(&backup);
    assert!(m[2].has("x"));
    m[2].sync(t + 5 * D);
    assert!(!m[2].has("x"), "restored data is brought up to date, not republished");
    assert!(!m[2].doc().items.contains_key("x"));
    let mut d = Mac::new("dd", &shared);
    d.sync(t + 5 * D + H);
    assert!(!d.has("x"), "nobody hands the item to a newcomer");
    run(&mut m, &all, t + 6 * D);
    assert!(m.iter().all(|x| !x.has("x")));
    cleanup(&shared, &[&m[0], &m[1], &m[2], &d]);
}

#[test]
fn a_mac_that_lost_its_tracker_and_its_collected_memory_relearns_from_the_others() {
    let shared = tmp("shared");
    let mut m = collected_everywhere(&shared, &["aa", "bb", "cc"], T0 + 20 * D);
    let stamp = m[1].tracker().collected["x"];
    // A loses its tracker (and with it what it collected) and its own document
    std::fs::remove_file(m[0].tracker_path()).unwrap();
    std::fs::remove_file(m[0].doc_path()).unwrap();
    zombie(&shared, "zz", stamp - 5);
    let t = T0 + 40 * D;
    m[0].sync(t);
    assert!(!m[0].has("x"), "B and C still publish what was collected: A relearns it");
    assert!(m[0].tracker().collected.contains_key("x"));
    run(&mut m, &[1, 2, 0], t + H);
    assert!(m.iter().all(|x| !x.has("x")));
    cleanup(&shared, &[&m[0], &m[1], &m[2]]);
}

#[test]
fn an_edit_made_after_the_deletion_still_wins_everywhere() {
    let shared = tmp("shared");
    let mut m = collected_everywhere(&shared, &["aa", "bb", "cc"], T0 + 20 * D);
    m[1].put("x", json!({"v": 99}));
    let t = T0 + 40 * D;
    for k in 0..3 {
        run(&mut m, &[1, 0, 2], t + k * D);
    }
    for x in &m {
        assert_eq!(x.value("x"), Some(json!({"v": 99})), "{} has the edit", x.id);
    }
    cleanup(&shared, &[&m[0], &m[1], &m[2]]);
}

#[test]
fn notes_a_forgotten_mac_never_uploaded_survive_its_return_and_the_deleted_item_goes() {
    let shared = tmp("shared");
    let mut m = settled(&shared, &["aa", "bb", "ee"]);
    let t = T0 + 20 * D;
    m[2].put("only-on-e", json!({"v": 9})); // made while E was offline for the others, never uploaded
    m[0].del("x");
    run(&mut m, &[0, 1], t);
    m[0].io.forget_device("ee").unwrap();
    run(&mut m, &[0, 1, 0, 1], t + D);
    assert!(!m[0].tomb("x"));
    m[2].sync(t + 5 * D); // E comes back
    assert!(m[2].has("only-on-e") && !m[2].has("x"));
    run(&mut m, &[0, 1, 2, 0, 1], t + 6 * D);
    assert!(m[0].has("only-on-e") && m[1].has("only-on-e"), "its notes reach the others");
    assert!(m.iter().all(|x| !x.has("x")));
    cleanup(&shared, &[&m[0], &m[1], &m[2]]);
}

#[test]
fn v0_8_0_documents_stay_compatible_and_a_stale_copy_from_a_v0_8_0_mac_is_ignored() {
    #[derive(serde::Deserialize)]
    struct Old {
        items: std::collections::BTreeMap<String, sync::Record>,
    }
    let shared = tmp("shared");
    let mut m = settled(&shared, &["aa", "bb"]);
    // a v0.8.0 Mac: a document without written_at or acknowledgements
    let dir = shared.join("devices/oldmac");
    std::fs::create_dir_all(&dir).unwrap();
    let legacy = r#"{"device":"oldmac","items":{"x":{"modified":0,"value":{"v":1}}},"first_sync_at":5,"run_at":99}"#;
    std::fs::write(dir.join("themes.json"), legacy).unwrap();
    let t = T0 + 20 * D;
    m[0].del("x");
    for k in 0..3 {
        run(&mut m, &[0, 1], t + k * D);
    }
    assert!(m[0].tomb("x") && m[1].tomb("x"), "a Mac on 0.8.0 blocks the clean-up");
    m[0].io.forget_device("oldmac").unwrap();
    run(&mut m, &[0, 1, 0, 1], t + 4 * D);
    assert!(!m[0].tomb("x"), "forgetting it lets the clean-up proceed");
    // the 0.8.0 Mac is still running and publishes its stale copy again
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("themes.json"), legacy).unwrap();
    run(&mut m, &[0, 1, 0, 1], t + 5 * D);
    assert!(!m[0].has("x") && !m[1].has("x"), "the newer Macs ignore it");
    assert_eq!(std::fs::read_to_string(dir.join("themes.json")).unwrap(), legacy, "it is told nothing new");
    // and what the newer Macs write is still readable by a 0.8.0 reader
    let raw = std::fs::read(m[0].doc_path()).unwrap();
    let old: Old = serde_json::from_slice(&raw).unwrap();
    assert!(old.items.contains_key("keep"));
    // while it is back it blocks again
    m[0].del("keep");
    for k in 0..3 {
        run(&mut m, &[0, 1], t + (6 + k) * D);
    }
    assert!(m[0].tomb("keep"));
    cleanup(&shared, &[&m[0], &m[1]]);
}
