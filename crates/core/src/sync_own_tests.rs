//! Own-entries clipboard documents (T3) and Forget this Mac / tombstone GC (T4).
use super::*;
use crate::models::{ClipEntry, ClipKind, Snippet};
use std::collections::BTreeSet;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ferrocast-own-{tag}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn clip(id: &str, at: i64) -> ClipEntry {
    ClipEntry {
        id: id.into(),
        kind: ClipKind::Text,
        content: format!("text of {id}"),
        title: None,
        copied_at: at,
        pinned: false,
        source_app: None,
        times_copied: 1,
        width: 0,
        height: 0,
    }
}

const NOW: i64 = 1_000_000_000; // ms

/// One simulated Mac with a clipboard.
struct Mac {
    data: PathBuf,
    io: SyncIo,
    list: Vec<ClipEntry>,
    snips: Vec<Snippet>,
}

impl Mac {
    fn new(id: &str, shared: &Path) -> Mac {
        let data = tmp(id);
        let io = SyncIo::new(shared.to_path_buf(), id.to_string(), data.join("images"));
        Mac { data, io, list: vec![], snips: vec![] }
    }
    fn sync(&mut self, now: i64) {
        let recover = needs_recovery(&self.data, "clipboard");
        self.sync_recovering(now, recover);
    }
    fn sync_recovering(&mut self, now: i64, recover: bool) {
        let o = sync_domain_recovering(&self.io, "clipboard", &to_items(&self.list), &load_tracker(&self.data, "clipboard"), now, recover)
            .unwrap();
        save_tracker(&self.data, "clipboard", &o.tracker_merged).unwrap();
        if recover {
            clear_recovery(&self.data, "clipboard");
        }
        if let Some(m) = o.merged {
            self.list = clipboard_normalize(from_items(m), now / 1000, Some(30 * 86_400));
            if let Some(d) = &o.applied_doc {
                self.io.write_doc("clipboard", d).unwrap();
            }
        }
    }
    fn named(mut self, name: &str) -> Mac {
        self.io = self.io.with_name(Some(name.into()));
        self
    }
    fn sync_snips(&mut self, now: i64) {
        let o = sync_domain(&self.io, "snippets", &to_items(&self.snips), &load_tracker(&self.data, "snippets"), now).unwrap();
        save_tracker(&self.data, "snippets", &o.tracker_merged).unwrap();
        if let Some(m) = o.merged {
            self.snips = from_items(m);
            self.io.write_doc("snippets", o.applied_doc.as_ref().unwrap()).unwrap();
        }
    }
    fn snip_doc(&self) -> DeviceDoc {
        read_json(&self.io.dir.join("devices").join(&self.io.device).join("snippets.json"))
    }
    fn snip_ids(&self) -> BTreeSet<String> {
        self.snips.iter().map(|s| s.id.clone()).collect()
    }
    fn ids(&self) -> BTreeSet<String> {
        self.list.iter().map(|e| e.id.clone()).collect()
    }
    fn doc(&self) -> DeviceDoc {
        read_json(&self.io.dir.join("devices").join(&self.io.device).join("clipboard.json"))
    }
    fn live_in_doc(&self) -> BTreeSet<String> {
        self.doc().items.into_iter().filter(|(_, r)| !r.deleted).map(|(k, _)| k).collect()
    }
    fn dead_in_doc(&self) -> BTreeSet<String> {
        self.doc().items.into_iter().filter(|(_, r)| r.deleted).map(|(k, _)| k).collect()
    }
}

fn read_json(p: &Path) -> DeviceDoc {
    serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap()
}

fn set(v: &[&str]) -> BTreeSet<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn settle(macs: &mut [&mut Mac], from: i64) -> i64 {
    let mut t = from;
    for _ in 0..4 {
        for m in macs.iter_mut() {
            t += 1_000;
            m.sync(t);
        }
    }
    t
}

fn cleanup(macs: &[&Mac], shared: &Path) {
    for m in macs {
        let _ = std::fs::remove_dir_all(&m.data);
    }
    let _ = std::fs::remove_dir_all(shared);
}

/// The way a v0.7.0 Mac merges: every document, item by item.
fn old_mac_view(shared: &Path, own: &str) -> BTreeSet<String> {
    let io = SyncIo::new(shared.to_path_buf(), own.to_string(), shared.join("none"));
    let docs = io.read_docs("clipboard").docs;
    let (items, _) = sync::merge(&[], &docs, &Tracker::default());
    items.into_iter().map(|(id, _)| id).collect()
}

#[test]
fn a_clipboard_document_holds_only_the_entries_copied_on_that_mac() {
    let shared = tmp("shared");
    let (mut a, mut b, mut c) = (Mac::new("aa", &shared), Mac::new("bb", &shared), Mac::new("cc", &shared));
    let s = NOW / 1000;
    a.list = vec![clip("a1", s - 30), clip("a2", s - 20)];
    b.list = vec![clip("b1", s - 25)];
    settle(&mut [&mut a, &mut b, &mut c], NOW);
    for m in [&a, &b, &c] {
        assert_eq!(m.ids(), set(&["a1", "a2", "b1"]), "every Mac holds the whole history");
    }
    assert_eq!(a.live_in_doc(), set(&["a1", "a2"]), "A publishes only what it copied");
    assert_eq!(b.live_in_doc(), set(&["b1"]));
    assert!(c.live_in_doc().is_empty(), "a Mac that copied nothing publishes no entries");
    cleanup(&[&a, &b, &c], &shared);
}

#[test]
fn documents_do_not_grow_with_the_number_of_macs() {
    let shared = tmp("shared");
    let mut macs: Vec<Mac> = ["m1", "m2", "m3", "m4"].iter().map(|id| Mac::new(id, &shared)).collect();
    let s = NOW / 1000;
    for (i, m) in macs.iter_mut().enumerate() {
        m.list = (0..50).map(|k| clip(&format!("{}-{k}", i), s - 100 + k)).collect();
    }
    let mut t = NOW;
    for _ in 0..4 {
        for m in macs.iter_mut() {
            t += 1_000;
            m.sync(t);
        }
    }
    let one = std::fs::metadata(shared.join("devices/m1/clipboard.json")).unwrap().len();
    assert!(macs.iter().all(|m| m.list.len() == 200));
    for m in &macs {
        let size = std::fs::metadata(shared.join("devices").join(&m.io.device).join("clipboard.json")).unwrap().len();
        assert!(size < one * 3 / 2, "{}: {size} vs {one}", m.io.device);
        assert_eq!(m.live_in_doc().len(), 50, "{}", m.io.device);
    }
    for m in &macs {
        let _ = std::fs::remove_dir_all(&m.data);
    }
    let _ = std::fs::remove_dir_all(&shared);
}

#[test]
fn a_v0_7_0_shaped_peer_still_merges_and_still_reads_the_new_documents() {
    let shared = tmp("shared");
    let mut a = Mac::new("bb", &shared);
    let s = NOW / 1000;
    // an old Mac: its document holds its own entry and an adopted copy of an entry nobody publishes now
    let old = shared.join("devices").join("aa");
    std::fs::create_dir_all(&old).unwrap();
    let rec = |e: &ClipEntry, adopted: bool| sync::Record {
        modified: e.copied_at * 1000,
        deleted: false,
        value: Some(serde_json::to_value(e).unwrap()),
        pristine: false,
        adopted,
    };
    let (own, other) = (clip("old-own", s - 50), clip("old-adopted", s - 40));
    let mut doc = DeviceDoc { device: "aa".into(), ..Default::default() };
    doc.items.insert(own.id.clone(), rec(&own, false));
    doc.items.insert(other.id.clone(), rec(&other, true));
    std::fs::write(old.join("clipboard.json"), serde_json::to_vec(&doc).unwrap()).unwrap();
    a.list = vec![clip("new-1", s - 10)];
    a.sync(NOW);
    a.sync(NOW + 1_000);
    assert_eq!(a.ids(), set(&["new-1", "old-own", "old-adopted"]), "the new Mac merges the old document");
    assert_eq!(a.live_in_doc(), set(&["new-1"]));
    assert_eq!(old_mac_view(&shared, "zz"), set(&["new-1", "old-own", "old-adopted"]), "an old Mac still gets everything");
    cleanup(&[&a], &shared);
}

#[test]
fn deleting_an_entry_from_another_mac_reaches_everyone_old_and_new() {
    let shared = tmp("shared");
    let (mut a, mut b, mut c) = (Mac::new("aa", &shared), Mac::new("bb", &shared), Mac::new("cc", &shared));
    let s = NOW / 1000;
    a.list = vec![clip("x", s - 30), clip("y", s - 20)];
    let t = settle(&mut [&mut a, &mut b, &mut c], NOW);
    assert_eq!(b.ids(), set(&["x", "y"]));
    b.list.retain(|e| e.id != "x"); // B deletes A's entry
    let t = settle(&mut [&mut b, &mut a, &mut c], t);
    assert_eq!(a.ids(), set(&["y"]), "the origin Mac loses it");
    assert_eq!(c.ids(), set(&["y"]), "a third Mac loses it");
    assert!(b.dead_in_doc().contains("x"), "the deleting Mac publishes the tombstone");
    assert!(!b.live_in_doc().contains("y"), "and still no foreign entries");
    assert_eq!(old_mac_view(&shared, "zz"), set(&["y"]), "an old Mac merging every document drops it too");
    // nothing comes back later
    settle(&mut [&mut a, &mut b, &mut c], t);
    assert_eq!(c.ids(), set(&["y"]));
    cleanup(&[&a, &b, &c], &shared);
}

#[test]
fn pinning_a_foreign_entry_is_published_by_the_mac_that_pinned_it() {
    let shared = tmp("shared");
    let (mut a, mut b, mut c) = (Mac::new("aa", &shared), Mac::new("bb", &shared), Mac::new("cc", &shared));
    a.list = vec![clip("x", NOW / 1000 - 30)];
    let t = settle(&mut [&mut a, &mut b, &mut c], NOW);
    b.list[0].pinned = true;
    settle(&mut [&mut b, &mut a, &mut c], t);
    assert!(a.list[0].pinned && c.list[0].pinned, "the edit travels");
    assert!(b.live_in_doc().contains("x"), "an edited entry is published by the editor");
    cleanup(&[&a, &b, &c], &shared);
}

#[test]
fn a_lost_local_history_is_recovered_from_the_peers_documents() {
    let shared = tmp("shared");
    let (mut a, mut b) = (Mac::new("aa", &shared), Mac::new("bb", &shared));
    let s = NOW / 1000;
    a.list = vec![clip("a1", s - 30)];
    b.list = vec![clip("b1", s - 20), clip("b2", s - 10)];
    let t = settle(&mut [&mut a, &mut b], NOW);
    assert_eq!(b.ids(), set(&["a1", "b1", "b2"]));
    // B's data file is lost and its tracker too: only adopted a1 is not in its own document
    b.list.clear();
    mark_recovery(&b.data, "clipboard");
    b.sync(t + 1_000);
    assert_eq!(b.ids(), set(&["a1", "b1", "b2"]), "own entries from its own document, A's from A's");
    b.sync(t + 2_000);
    a.sync(t + 3_000);
    assert_eq!(a.ids(), set(&["a1", "b1", "b2"]), "nothing was published as deleted");
    assert_eq!(b.live_in_doc(), set(&["b1", "b2"]));
    cleanup(&[&a, &b], &shared);
}

#[test]
fn a_v0_7_0_document_with_mixed_entries_shrinks_to_its_own_without_losing_or_resurrecting_anything() {
    let shared = tmp("shared");
    let (mut a, mut b) = (Mac::new("aa", &shared), Mac::new("bb", &shared));
    let s = NOW / 1000;
    let rec = |e: &ClipEntry, adopted: bool| sync::Record {
        modified: e.copied_at * 1000,
        deleted: false,
        value: Some(serde_json::to_value(e).unwrap()),
        pristine: false,
        adopted,
    };
    let (a1, b1, b2) = (clip("a1", s - 30), clip("b1", s - 20), clip("b2", s - 10));
    // v0.7.0 state: A published a1 plus adopted copies of b1 and b2; B published b1 and b2 plus an
    // adopted copy of a1; b2 was then deleted on B (tombstone) while A still carries its copy
    let mut doc_a = DeviceDoc { device: "aa".into(), first_sync_at: Some(0), ..Default::default() };
    doc_a.items.insert("a1".into(), rec(&a1, false));
    doc_a.items.insert("b1".into(), rec(&b1, true));
    doc_a.items.insert("b2".into(), rec(&b2, true));
    let mut doc_b = DeviceDoc { device: "bb".into(), first_sync_at: Some(0), ..Default::default() };
    doc_b.items.insert("b1".into(), rec(&b1, false));
    doc_b.items.insert("a1".into(), rec(&a1, true));
    doc_b.items.insert("b2".into(), sync::Record { modified: (s - 5) * 1000, deleted: true, value: None, pristine: false, adopted: false });
    for (d, doc) in [("aa", &doc_a), ("bb", &doc_b)] {
        std::fs::create_dir_all(shared.join("devices").join(d)).unwrap();
        std::fs::write(shared.join("devices").join(d).join("clipboard.json"), serde_json::to_vec(doc).unwrap()).unwrap();
    }
    // A's tracker as v0.7.0 left it: it knows what its document says
    save_tracker(&a.data, "clipboard", &sync::tracker_from_own_doc(&doc_a)).unwrap();
    save_tracker(&b.data, "clipboard", &sync::tracker_from_own_doc(&doc_b)).unwrap();
    a.list = vec![a1.clone(), b1.clone(), b2.clone()];
    b.list = vec![b1.clone(), a1.clone()];
    settle(&mut [&mut a, &mut b], NOW);
    assert_eq!(a.ids(), set(&["a1", "b1"]), "the deleted b2 is gone on A and does not come back");
    assert_eq!(b.ids(), set(&["a1", "b1"]));
    assert_eq!(a.live_in_doc(), set(&["a1"]), "A's document shrank to its own entry");
    assert_eq!(b.live_in_doc(), set(&["b1"]));
    assert_eq!(old_mac_view(&shared, "zz"), set(&["a1", "b1"]), "an old Mac sees the same history");
    cleanup(&[&a, &b], &shared);
}

#[test]
fn adopted_entries_are_published_again_when_the_mac_that_has_them_is_gone() {
    let shared = tmp("shared");
    let (mut a, mut b, mut c) = (Mac::new("aa", &shared), Mac::new("bb", &shared), Mac::new("cc", &shared));
    b.list = vec![clip("b1", NOW / 1000 - 30)];
    let t = settle(&mut [&mut a, &mut b, &mut c], NOW);
    assert!(a.live_in_doc().is_empty() && c.live_in_doc().is_empty());
    std::fs::remove_dir_all(shared.join("devices").join("bb")).unwrap(); // B's folder disappears
    settle(&mut [&mut a, &mut c], t);
    // exactly one of the two keeps it available (the lower id), never none and not a growing copy
    assert_eq!(a.live_in_doc(), set(&["b1"]));
    assert!(c.live_in_doc().is_empty());
    assert_eq!(c.ids(), set(&["b1"]));
    cleanup(&[&a, &b, &c], &shared);
}

#[test]
fn images_of_entries_copied_elsewhere_are_not_uploaded_again() {
    let shared = tmp("shared");
    let (mut a, mut b) = (Mac::new("aa", &shared), Mac::new("bb", &shared));
    std::fs::create_dir_all(b.data.join("images")).unwrap();
    std::fs::write(b.data.join("images/pic.png"), b"png bytes").unwrap();
    let mut e = clip("img", NOW / 1000 - 5);
    e.kind = ClipKind::Image;
    e.content = "pic.png".into();
    b.list = vec![e];
    settle(&mut [&mut b, &mut a], NOW);
    assert!(a.data.join("images/pic.png").is_file(), "A received the picture");
    assert!(shared.join("devices/bb/clipboard-images/pic.png").is_file(), "from B's folder");
    assert!(!shared.join("devices/aa/clipboard-images/pic.png").exists(), "and does not upload a second copy");
    cleanup(&[&a, &b], &shared);
}

// ----------------------------------------------------------------------------- T4

const H: i64 = 3_600_000;
const D: i64 = 24 * H;

fn snip(id: &str) -> Snippet {
    Snippet { id: id.into(), name: id.into(), text: format!("text of {id}"), keyword: String::new(), tags: vec![] }
}

/// `own` has a tombstone for `id` in its published snippets document.
fn tombstoned(m: &Mac, id: &str) -> bool {
    m.snip_doc().items.get(id).is_some_and(|r| r.deleted)
}

fn three_with_s1(shared: &Path) -> (Mac, Mac, Mac, i64) {
    let (mut a, mut b, mut c) = (Mac::new("aa", shared).named("Alpha"), Mac::new("bb", shared).named("Beta"), Mac::new("cc", shared));
    a.snips = vec![snip("s1")];
    let mut t = NOW;
    for _ in 0..3 {
        for m in [&mut a, &mut b, &mut c] {
            t += 1_000;
            m.sync_snips(t);
        }
    }
    assert!(b.snip_ids().contains("s1"));
    (a, b, c, t)
}

#[test]
fn a_tombstone_is_dropped_only_once_every_live_mac_has_published_past_it() {
    let shared = tmp("shared");
    let (mut a, mut b, mut c, t) = three_with_s1(&shared);
    let t0 = t + 10 * H;
    a.snips.clear();
    a.sync_snips(t0);
    assert!(tombstoned(&a, "s1"));
    // nobody has run since the deletion: it stays, however often A runs
    for k in 1..4 {
        a.sync_snips(t0 + k * H);
        assert!(tombstoned(&a, "s1"), "kept before anyone has seen it ({k})");
    }
    b.sync_snips(t0 + 12 * H); // B has seen it
    a.sync_snips(t0 + 13 * H);
    assert!(tombstoned(&a, "s1"), "C has not seen it yet");
    c.sync_snips(t0 + 14 * H);
    a.sync_snips(t0 + 15 * H);
    assert!(!a.snip_doc().items.contains_key("s1"), "all live Macs have published past it: dropped");
    // and it does not come back anywhere
    for k in 16..24 {
        for m in [&mut b, &mut c, &mut a] {
            m.sync_snips(t0 + k * H);
        }
    }
    assert!(!a.snip_ids().contains("s1") && !b.snip_ids().contains("s1") && !c.snip_ids().contains("s1"));
    for m in [&a, &b, &c] {
        let _ = std::fs::remove_dir_all(&m.data);
    }
    let _ = std::fs::remove_dir_all(&shared);
}

#[test]
fn a_mac_that_has_been_offline_longer_than_the_live_window_does_not_hold_deletions_back() {
    let shared = tmp("shared");
    let (mut a, b, mut c, t) = three_with_s1(&shared);
    // B goes away; A and C keep running for 40 days
    let t0 = t + 40 * D;
    c.sync_snips(t0 - D);
    a.snips.clear();
    a.sync_snips(t0);
    assert!(tombstoned(&a, "s1"));
    c.sync_snips(t0 + 7 * H);
    a.sync_snips(t0 + 14 * H);
    assert!(!a.snip_doc().items.contains_key("s1"), "dropped although B (offline 40 days) never saw it");
    assert!(shared.join("devices/bb/snippets.json").is_file(), "B's stale document is still in the folder");
    for k in 1..4 {
        a.sync_snips(t0 + 14 * H + k * 7 * H);
        c.sync_snips(t0 + 14 * H + k * 7 * H + H);
    }
    assert!(!a.snip_ids().contains("s1") && !c.snip_ids().contains("s1"), "B's old record does not bring it back");
    assert!(!a.snip_doc().items.contains_key("s1"));
    // the opposite: B back within the window holds the clean-up back
    let (mut a2, mut b2, mut c2, t) = three_with_s1(&tmp("shared2"));
    let t0 = t + 20 * D;
    c2.sync_snips(t0 - D);
    a2.snips.clear();
    a2.sync_snips(t0);
    c2.sync_snips(t0 + 7 * H);
    a2.sync_snips(t0 + 14 * H);
    assert!(tombstoned(&a2, "s1"), "B was seen 20 days ago: still live, has not seen the deletion");
    b2.sync_snips(t0 + 15 * H);
    a2.sync_snips(t0 + 22 * H);
    assert!(!a2.snip_doc().items.contains_key("s1"));
    let _ = (&b, &a2, &b2, &c2);
}

#[test]
fn a_live_mac_that_publishes_no_run_time_holds_deletions_back_until_it_is_offline_too_long() {
    let shared = tmp("shared");
    let (mut a, mut b, mut c, t) = three_with_s1(&shared);
    // B and C are v0.7.0-like: their documents carry no run time
    let strip = |m: &Mac| {
        let p = shared.join("devices").join(&m.io.device).join("snippets.json");
        let mut d = read_json(&p);
        d.run_at = None;
        std::fs::write(&p, serde_json::to_vec(&d).unwrap()).unwrap();
        let f = std::fs::File::options().write(true).open(&p).unwrap();
        f.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_millis(t as u64)).unwrap();
    };
    strip(&b);
    strip(&c);
    let t0 = t + 10 * H;
    a.snips.clear();
    a.sync_snips(t0);
    for k in 1..6 {
        a.sync_snips(t0 + k * 8 * H);
        assert!(tombstoned(&a, "s1"), "a live Mac without a run time blocks ({k})");
    }
    let later = t0 + 31 * D; // 31 days later their files are old: they no longer count as live
    a.sync_snips(later);
    a.sync_snips(later + H);
    assert!(tombstoned(&a, "s1"), "no live Mac at all: nobody can vouch for it, kept");
    c.sync_snips(later + 2 * H); // C comes back and publishes a run time again
    a.sync_snips(later + 10 * H);
    assert!(!a.snip_doc().items.contains_key("s1"), "B has only an old file (offline): not live; C has published past the deletion");
    let _ = (&mut b, &mut c);
}

#[test]
fn deletions_of_built_in_defaults_are_never_dropped() {
    let shared = tmp("shared");
    let default_id = crate::storage::default_snippets()[0].id.clone();
    let (mut a, mut b) = (Mac::new("aa", &shared), Mac::new("bb", &shared));
    let mut t = NOW;
    for _ in 0..3 {
        for m in [&mut a, &mut b] {
            t += 1_000;
            m.sync_snips(t);
        }
    }
    assert!(a.snip_doc().items[&default_id].deleted, "A has no defaults here: it publishes their deletion");
    for k in 1..10 {
        for m in [&mut b, &mut a] {
            m.sync_snips(t + k * 12 * H);
        }
    }
    assert!(a.snip_doc().items[&default_id].deleted, "a fresh Mac still has the default, so its deletion stays");
    for m in [&a, &b] {
        let _ = std::fs::remove_dir_all(&m.data);
    }
    let _ = std::fs::remove_dir_all(&shared);
}

#[test]
fn clipboard_deletions_keep_their_own_aging_and_are_not_dropped_early() {
    let shared = tmp("shared");
    let (mut a, mut b) = (Mac::new("aa", &shared), Mac::new("bb", &shared));
    a.list = vec![clip("x", NOW / 1000 - 30)];
    let t = settle(&mut [&mut a, &mut b], NOW);
    a.list.clear();
    a.sync(t + H);
    for k in 1..12 {
        b.sync(t + 20 * k * H);
        a.sync(t + 20 * k * H + H);
    }
    assert!(a.dead_in_doc().contains("x"), "still there after every Mac has seen it (the clipboard ages them itself)");
    cleanup(&[&a, &b], &shared);
}

#[test]
fn an_idle_mac_does_not_rewrite_its_documents_on_every_run() {
    let shared = tmp("shared");
    let (mut a, mut b) = (Mac::new("aa", &shared), Mac::new("bb", &shared));
    a.snips = vec![snip("s1")];
    let t = NOW;
    a.sync_snips(t);
    b.sync_snips(t + 1_000);
    a.sync_snips(t + 2_000);
    let p = shared.join("devices/aa/snippets.json");
    let first = std::fs::read(&p).unwrap();
    for k in 1..5 {
        a.sync_snips(t + 2_000 + k * 600_000);
        assert_eq!(std::fs::read(&p).unwrap(), first, "run {k}: unchanged bytes within the refresh interval");
    }
    a.sync_snips(t + 8 * H);
    assert_ne!(std::fs::read(&p).unwrap(), first, "the run time is refreshed after a few hours");
    assert!(read_json(&p).run_at.is_some_and(|r| r >= t + 8 * H));
    cleanup(&[&a, &b], &shared);
}

#[test]
fn the_sync_folder_lists_the_macs_that_published_with_their_names_and_last_seen() {
    let shared = tmp("shared");
    let (mut a, mut b, mut c) = (Mac::new("aa", &shared).named("Alpha"), Mac::new("bb", &shared).named("Beta"), Mac::new("cc", &shared));
    a.snips = vec![snip("s1")];
    a.sync_snips(NOW);
    b.sync_snips(NOW + 5 * H);
    c.sync_snips(NOW + 9 * H);
    std::fs::create_dir_all(shared.join("devices/empty")).unwrap();
    let list = a.io.devices();
    let ids: Vec<&str> = list.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(ids[0], "aa", "this Mac first");
    assert!(list[0].current && list[1..].iter().all(|d| !d.current));
    assert_eq!(list.iter().find(|d| d.id == "bb").unwrap().name.as_deref(), Some("Beta"));
    assert_eq!(list.iter().find(|d| d.id == "cc").unwrap().name, None);
    assert_eq!(list.iter().find(|d| d.id == "bb").unwrap().last_seen, Some(NOW + 5 * H));
    assert_eq!(list.iter().find(|d| d.id == "cc").unwrap().last_seen, Some(NOW + 9 * H));
    assert_eq!(ids[1], "cc", "most recently seen first");
    cleanup(&[&a, &b, &c], &shared);
}

#[test]
fn forgetting_a_mac_removes_only_its_folder_and_it_reappears_when_it_publishes_again() {
    let shared = tmp("shared");
    let (mut a, mut b, mut c) = (Mac::new("aa", &shared), Mac::new("bb", &shared), Mac::new("cc", &shared));
    a.snips = vec![snip("s1")];
    b.snips = vec![snip("only-b")];
    for k in 0..3 {
        for m in [&mut a, &mut b, &mut c] {
            m.sync_snips(NOW + k * 3_000);
        }
    }
    std::fs::write(shared.join("devices/bb/clipboard-images-note.txt"), b"x").unwrap();
    assert!(a.io.forget_device("bb").is_ok());
    assert!(!shared.join("devices/bb").exists(), "its folder is gone, with everything in it");
    assert!(shared.join("devices/aa/snippets.json").is_file() && shared.join("devices/cc/snippets.json").is_file());
    let read: Vec<String> = a.io.read_docs("snippets").docs.into_iter().map(|d| d.device).collect();
    assert_eq!(read, ["cc"], "this Mac stops reading it");
    assert!(a.io.devices().iter().all(|d| d.id != "bb"));
    // B comes back and publishes
    b.sync_snips(NOW + 100_000);
    assert!(a.io.devices().iter().any(|d| d.id == "bb"), "it simply reappears");
    cleanup(&[&a, &b, &c], &shared);
}

#[test]
fn forgetting_refuses_this_mac_and_anything_that_is_not_a_mac_folder() {
    let shared = tmp("shared");
    let mut a = Mac::new("aa", &shared);
    a.snips = vec![snip("s1")];
    a.sync_snips(NOW);
    std::fs::create_dir_all(shared.join("devices/bb")).unwrap();
    std::fs::write(shared.join("keep.txt"), b"x").unwrap();
    std::fs::write(shared.join("devices/plain-file"), b"x").unwrap();
    for bad in ["aa", "", ".", "..", "../aa", "bb/../aa", "/", ".hidden", "plain-file", "missing"] {
        assert!(a.io.forget_device(bad).is_err(), "{bad:?} refused");
    }
    assert!(shared.join("devices/aa/snippets.json").is_file() && shared.join("devices/bb").is_dir() && shared.join("keep.txt").is_file());
    assert!(shared.join("devices/plain-file").is_file());
    cleanup(&[&a], &shared);
}

#[test]
fn documents_of_macs_offline_too_long_are_read_only_when_no_other_mac_is_live() {
    let shared = tmp("shared");
    let mut b = Mac::new("bb", &shared);
    b.snips = vec![snip("only-b")];
    b.sync_snips(NOW);
    // only a stale Mac is left: a new Mac still restores from it
    let mut d = Mac::new("dd", &shared);
    d.sync_snips(NOW + 40 * D);
    assert!(d.snip_ids().contains("only-b"), "the only copy of the data stays readable");
    // with a live Mac around, the stale document is not a source any more
    let mut e = Mac::new("ee", &shared);
    e.sync_snips(NOW + 40 * D + H);
    assert!(e.snip_ids().contains("only-b"), "D carried it on, so E still gets it from D");
    d.snips.retain(|s| s.id != "only-b");
    d.sync_snips(NOW + 40 * D + 2 * H);
    e.sync_snips(NOW + 40 * D + 3 * H);
    assert!(!e.snip_ids().contains("only-b"), "B's old record does not outvote the live Macs' deletion");
    for m in [&b, &d, &e] {
        let _ = std::fs::remove_dir_all(&m.data);
    }
    let _ = std::fs::remove_dir_all(&shared);
}
