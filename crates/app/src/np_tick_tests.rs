//! The notch player hides itself after a pause even when no further event arrives (child module of `tests`).
use super::*;
use ferrocast_core::nowplaying::{Event, PlayState, Snapshot, IDLE_HIDE_SECS};

fn snap(state: PlayState) -> Snapshot {
    Snapshot {
        bundle_id: "com.brave.Browser".into(),
        state,
        title: "Song".into(),
        artist: "Band".into(),
        album: "Record".into(),
        duration: 200.0,
        position: 10.0,
        artwork: None,
    }
}

fn feed(app: &mut App, m: Msg) {
    app.bus.send(m);
    app.drain();
}

fn on(app: &mut App) {
    let mut s = app.settings.clone();
    s.now_playing = true;
    app.commit_settings(s);
}

#[test]
fn a_pause_schedules_one_wake_up_for_the_idle_deadline_and_a_play_cancels_it() {
    let mut app = fixture();
    on(&mut app);
    assert_eq!(app.np_tick_at, None);
    feed(&mut app, Msg::NowPlaying(Event::Media(Some(snap(PlayState::Playing)))));
    assert_eq!(app.np_tick_at, None, "playing: nothing to wait for");
    feed(&mut app, Msg::NowPlaying(Event::Media(Some(snap(PlayState::Paused)))));
    let at = app.np_tick_at.expect("paused: a wake-up is scheduled");
    let now = ferrocast_core::nowplaying::mono_secs();
    assert!((at - (now + IDLE_HIDE_SECS)).abs() < 2.0, "{at} vs {now}+{IDLE_HIDE_SECS}");
    // an identical pause event keeps the same schedule (no second wake-up)
    feed(&mut app, Msg::NowPlaying(Event::Media(Some(snap(PlayState::Paused)))));
    assert_eq!(app.np_tick_at, Some(at));
    feed(&mut app, Msg::NowPlaying(Event::Media(Some(snap(PlayState::Playing)))));
    assert_eq!(app.np_tick_at, None, "playing again cancels the schedule");
    // a tick that arrives early draws nothing and keeps the pill
    feed(&mut app, Msg::NowPlaying(Event::Tick));
    assert!(app.now_playing.can_command());
    std::fs::remove_dir_all(&app.store.root).ok();
}
