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
