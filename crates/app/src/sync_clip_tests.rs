//! Sync apply must not touch the clipboard dirty flag for other domains (child module of `tests`).
use super::*;
use std::sync::atomic::Ordering;

fn applied(app: &mut App, domain: &'static str) {
    let snap = app.sync_snapshot(domain);
    let merged = ferrocast_core::sync_io::to_items::<ferrocast_core::models::Snippet>(&[]);
    let merged = if domain == "settings" { ferrocast_core::sync_io::settings_items(&app.settings) } else { merged };
    assert!(app.sync_apply(domain, merged, ferrocast_core::sync::Tracker::default(), &snap).is_some(), "{domain} applied");
}

#[test]
fn applying_another_domain_keeps_a_fresh_copy_marked_for_sync() {
    for domain in ["snippets", "quicklinks", "notes", "settings"] {
        let mut app = fixture();
        app.settings.sync_enabled = true;
        // a copy landed after the last flush
        app.shared.clip_dirty.store(true, Ordering::Relaxed);
        applied(&mut app, domain);
        assert!(app.shared.clip_dirty.load(Ordering::Relaxed), "{domain}: the copy's dirty flag survived");
        std::fs::remove_dir_all(&app.store.root).ok();
    }
}

#[test]
fn a_domain_that_is_not_applied_leaves_the_dirty_flag_alone() {
    let mut app = fixture();
    app.settings.sync_enabled = true;
    app.shared.clip_dirty.store(true, Ordering::Relaxed);
    let snap = app.sync_snapshot("snippets");
    app.snippets.push(ferrocast_core::models::Snippet::new("a", "b", "c")); // local edit after the snapshot
    assert!(app.sync_apply("snippets", vec![], ferrocast_core::sync::Tracker::default(), &snap).is_none());
    assert!(app.shared.clip_dirty.load(Ordering::Relaxed));
    std::fs::remove_dir_all(&app.store.root).ok();
}

/// Holds the clipboard save turn for a while from another thread; returns once it is held.
fn hold_save_turn(ms: u64) -> std::thread::JoinHandle<()> {
    let (tx, rx) = std::sync::mpsc::channel();
    let h = std::thread::spawn(move || {
        let _turn = crate::services::CLIPBOARD_SAVE.lock().unwrap_or_else(|e| e.into_inner());
        tx.send(()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(ms));
    });
    rx.recv().unwrap();
    h
}

fn clip(content: &str, at: i64) -> ferrocast_core::models::ClipEntry {
    ferrocast_core::models::ClipEntry {
        id: content.into(),
        kind: ferrocast_core::models::ClipKind::Text,
        content: content.into(),
        title: None,
        copied_at: at,
        pinned: false,
        source_app: None,
        times_copied: 1,
        width: 0,
        height: 0,
    }
}

#[test]
fn a_sync_apply_clipboard_save_waits_for_the_save_turn_and_the_newest_list_wins() {
    let mut app = fixture();
    app.settings.sync_enabled = true;
    let snap = app.sync_snapshot("clipboard");
    let merged = ferrocast_core::sync_io::to_items(&[clip("synced", crate::services::now_secs())]);
    let holder = hold_save_turn(300);
    let t = Instant::now();
    assert!(app.sync_apply("clipboard", merged, ferrocast_core::sync::Tracker::default(), &snap).is_some());
    assert!(t.elapsed() >= Duration::from_millis(250), "the save took its turn behind the other writer");
    holder.join().unwrap();
    let on_disk: Vec<ferrocast_core::models::ClipEntry> = app.store.load("clipboard.json");
    assert_eq!(on_disk, *app.shared.clipboard.lock().unwrap(), "the file holds the newest list");
    assert_eq!(on_disk.len(), 1);
    std::fs::remove_dir_all(&app.store.root).ok();
}

#[test]
fn first_publish_persist_of_the_clipboard_waits_for_the_save_turn() {
    let mut app = fixture();
    app.shared.clipboard.lock().unwrap().push(clip("a", 5));
    let holder = hold_save_turn(300);
    let t = Instant::now();
    app.sync_persist_all();
    assert!(t.elapsed() >= Duration::from_millis(250));
    holder.join().unwrap();
    let on_disk: Vec<ferrocast_core::models::ClipEntry> = app.store.load("clipboard.json");
    assert_eq!(on_disk.len(), 1);
    std::fs::remove_dir_all(&app.store.root).ok();
}
