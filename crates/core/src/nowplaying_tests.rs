use super::*;

fn snap(state: PlayState, title: &str) -> Snapshot {
    Snapshot {
        bundle_id: "com.example.player".into(),
        state,
        title: title.into(),
        artist: "Artist".into(),
        album: "Album".into(),
        duration: 200.0,
        position: 10.0,
        artwork: None,
    }
}

fn spotify(state: PlayState, title: &str) -> Snapshot {
    snap(state, title)
}

// ---------------------------------------------------------------- progress

#[test]
fn playing_position_advances_and_clamps_to_the_duration() {
    let s = spotify(PlayState::Playing, "a");
    assert_eq!(position_at(&s, 5.0), 15.0);
    assert_eq!(position_at(&s, 500.0), 200.0);
    assert_eq!(position_at(&s, -3.0), 10.0);
}

#[test]
fn paused_position_stays_and_unknown_duration_does_not_clamp() {
    assert_eq!(position_at(&spotify(PlayState::Paused, "a"), 30.0), 10.0);
    let mut s = spotify(PlayState::Playing, "a");
    s.duration = 0.0;
    assert_eq!(position_at(&s, 30.0), 40.0);
}

#[test]
fn fraction_is_clamped_and_zero_without_a_duration() {
    assert_eq!(fraction(50.0, 200.0), 0.25);
    assert_eq!(fraction(250.0, 200.0), 1.0);
    assert_eq!(fraction(-1.0, 200.0), 0.0);
    assert_eq!(fraction(5.0, 0.0), 0.0);
    assert_eq!(fraction(f64::NAN, 10.0), 0.0);
}

#[test]
fn clock_is_m_ss_and_h_mm_ss() {
    assert_eq!(format_clock(0.0), "0:00");
    assert_eq!(format_clock(65.9), "1:05");
    assert_eq!(format_clock(3725.0), "1:02:05");
    assert_eq!(format_clock(-5.0), "0:00");
    assert_eq!(format_clock(f64::NAN), "0:00");
}

// ---------------------------------------------------------------- the controller

fn on() -> Ctl {
    let mut c = Ctl::new();
    c.set_enabled(true, 0.0);
    c
}

fn media(c: &mut Ctl, now: f64, s: Snapshot) -> Out {
    c.on_media(now, Some(s))
}

#[test]
fn a_disabled_controller_shows_nothing() {
    let mut c = Ctl::new();
    assert_eq!(media(&mut c, 1.0, spotify(PlayState::Playing, "a")).render, None);
    assert_eq!(c.set_hover(true, 2.0).render, None);
    assert_eq!(c.on_unavailable(3.0), Out::default());
}

#[test]
fn playing_shows_the_collapsed_pill() {
    let mut c = on();
    assert!(!c.can_command());
    let m = media(&mut c, 1.0, spotify(PlayState::Playing, "a")).render.expect("first track renders");
    assert!(m.visible && !m.expanded);
    assert_eq!(m.snapshot.unwrap().title, "a");
    assert!(c.can_command());
}

#[test]
fn an_unchanged_update_does_not_redraw() {
    let mut c = on();
    media(&mut c, 1.0, spotify(PlayState::Playing, "a"));
    // two seconds later the position moved by the elapsed time: nothing changed
    let mut s = spotify(PlayState::Playing, "a");
    s.position = 12.0;
    assert_eq!(media(&mut c, 3.0, s).render, None);
}

#[test]
fn a_new_track_state_source_or_artwork_redraws() {
    let mut c = on();
    media(&mut c, 1.0, spotify(PlayState::Playing, "a"));
    assert!(media(&mut c, 3.0, spotify(PlayState::Playing, "b")).render.is_some());
    assert!(media(&mut c, 5.0, spotify(PlayState::Paused, "b")).render.is_some());
    let mut other = spotify(PlayState::Paused, "b");
    other.bundle_id = "com.apple.Music".into();
    assert!(media(&mut c, 6.0, other.clone()).render.is_some());
    other.artwork = Some(Arc::new(vec![1, 2, 3]));
    assert!(media(&mut c, 7.0, other.clone()).render.is_some());
    assert_eq!(media(&mut c, 8.0, other).render, None, "same bytes in a new Arc is not a change");
}

#[test]
fn a_seek_redraws_but_normal_drift_does_not() {
    let mut c = on();
    media(&mut c, 1.0, spotify(PlayState::Playing, "a"));
    let mut drift = spotify(PlayState::Playing, "a");
    drift.position = 10.0 + 2.0 + 0.5; // 2 s later, half a second of jitter
    assert_eq!(media(&mut c, 3.0, drift).render, None);
    let mut seek = spotify(PlayState::Playing, "a");
    seek.position = 150.0;
    assert!(media(&mut c, 5.0, seek).render.is_some());
}

#[test]
fn hover_expands_and_leaving_collapses() {
    let mut c = on();
    media(&mut c, 1.0, spotify(PlayState::Playing, "a"));
    assert!(c.set_hover(true, 2.0).render.unwrap().expanded);
    assert_eq!(c.set_hover(true, 2.1).render, None, "already expanded");
    assert!(!c.set_hover(false, 3.0).render.unwrap().expanded);
}

#[test]
fn hover_while_hidden_does_not_expand_later() {
    let mut c = on();
    assert_eq!(c.set_hover(true, 1.0).render, None);
    let m = media(&mut c, 2.0, spotify(PlayState::Playing, "a")).render.unwrap();
    assert!(!m.expanded, "the pointer was not on a visible player");
}

#[test]
fn nothing_playing_hides_at_once() {
    let mut c = on();
    media(&mut c, 1.0, spotify(PlayState::Playing, "a"));
    let m = c.on_media(3.0, None).render.unwrap();
    assert!(!m.visible && !m.expanded);
    assert!(!c.can_command());
}

#[test]
fn full_screen_hides_and_leaving_it_shows_again() {
    let mut c = on();
    media(&mut c, 1.0, spotify(PlayState::Playing, "a"));
    assert!(!c.set_fullscreen(3.0, true).render.unwrap().visible);
    assert_eq!(c.set_fullscreen(3.5, true).render, None, "unchanged");
    assert!(c.set_fullscreen(5.0, false).render.unwrap().visible);
    // a track change while full screen stays hidden
    c.set_fullscreen(6.0, true);
    assert_eq!(media(&mut c, 7.0, spotify(PlayState::Playing, "b")).render, None);
}

#[test]
fn it_hides_after_a_while_paused_and_returns_on_play() {
    let mut c = on();
    media(&mut c, 1.0, spotify(PlayState::Playing, "a"));
    let paused = spotify(PlayState::Paused, "a");
    assert!(media(&mut c, 5.0, paused.clone()).render.unwrap().visible, "a short pause keeps it");
    assert_eq!(media(&mut c, 5.0 + IDLE_HIDE_SECS - 1.0, paused.clone()).render, None);
    let gone = media(&mut c, 5.0 + IDLE_HIDE_SECS + 1.0, paused).render.unwrap();
    assert!(!gone.visible);
    let back = media(&mut c, 100.0, spotify(PlayState::Playing, "a")).render.unwrap();
    assert!(back.visible);
}

#[test]
fn hovering_keeps_a_paused_player_open() {
    let mut c = on();
    media(&mut c, 1.0, spotify(PlayState::Paused, "a"));
    c.set_hover(true, 2.0);
    let out = media(&mut c, 2.0 + IDLE_HIDE_SECS + 10.0, spotify(PlayState::Paused, "a"));
    assert_eq!(out.render, None, "still open under the pointer");
    let m = c.set_hover(false, 100.0).render.unwrap();
    assert!(m.visible, "the idle clock restarts when the pointer leaves");
}

#[test]
fn turning_it_off_hides_and_forgets_the_pointer() {
    let mut c = on();
    media(&mut c, 1.0, spotify(PlayState::Playing, "a"));
    c.set_hover(true, 2.0);
    let m = c.set_enabled(false, 3.0).render.unwrap();
    assert!(!m.visible && !m.expanded);
    assert_eq!(media(&mut c, 4.0, spotify(PlayState::Playing, "a")).render, None);
    c.set_enabled(true, 5.0);
    let m = media(&mut c, 6.0, spotify(PlayState::Playing, "a")).render.unwrap();
    assert!(m.visible && !m.expanded);
}

#[test]
fn giving_up_hides_the_pill_and_reports_once_until_turned_on_again() {
    let mut c = on();
    media(&mut c, 1.0, spotify(PlayState::Playing, "a"));
    let out = c.on_event(2.0, Event::Unavailable);
    assert!(out.unavailable);
    assert!(!out.render.unwrap().visible, "the pill stays hidden");
    assert!(!c.on_event(3.0, Event::Unavailable).unavailable, "once");
    c.set_enabled(false, 4.0);
    c.set_enabled(true, 5.0);
    assert!(c.on_event(6.0, Event::Unavailable).unavailable);
}

#[test]
fn events_are_routed_to_their_handlers() {
    let mut c = on();
    assert!(c.on_event(1.0, Event::Media(Some(spotify(PlayState::Playing, "a")))).render.unwrap().visible);
    assert!(!c.on_event(2.0, Event::Fullscreen(true)).render.unwrap().visible);
    assert!(c.on_event(3.0, Event::Fullscreen(false)).render.unwrap().visible);
    assert!(!c.on_event(4.0, Event::Media(None)).render.unwrap().visible);
}

// ---------------------------------------------------------------- the setting

#[test]
fn the_setting_is_off_by_default_and_old_files_load_without_it() {
    use crate::models::Settings;
    assert!(!Settings::default().now_playing);
    let old: Settings = serde_json::from_str(r#"{"launcher_hotkey":"Alt+Space"}"#).unwrap();
    assert!(!old.now_playing);
}

#[test]
fn the_setting_is_saved_with_the_others() {
    use crate::models::Settings;
    let s = Settings { now_playing: true, ..Default::default() };
    let back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
    assert!(back.now_playing);
}

// ---------------------------------------------------------------- idle tick

#[test]
fn a_pause_hides_the_pill_on_the_idle_tick_with_no_further_event() {
    let mut c = on();
    media(&mut c, 1.0, spotify(PlayState::Playing, "a"));
    let paused = spotify(PlayState::Paused, "a");
    assert!(media(&mut c, 5.0, paused).render.unwrap().visible);
    // the feed goes quiet: the app asks when to look again
    let due = c.hide_deadline().expect("a paused pill has a deadline");
    assert_eq!(due, 5.0 + IDLE_HIDE_SECS);
    assert_eq!(c.tick(due - 1.0).render, None, "too early: still shown");
    assert!(c.can_command());
    let gone = c.tick(due + 0.01).render.expect("hidden by the tick alone");
    assert!(!gone.visible && !gone.expanded && gone.snapshot.is_none());
    assert!(!c.can_command());
    assert_eq!(c.hide_deadline(), None, "nothing left to wait for");
    assert_eq!(c.tick(due + 5.0).render, None, "a late tick is a no-op");
}

#[test]
fn the_tick_never_hides_a_playing_or_hovered_player_or_one_that_is_off() {
    let mut c = on();
    media(&mut c, 1.0, spotify(PlayState::Playing, "a"));
    assert_eq!(c.hide_deadline(), None, "playing never idles");
    assert_eq!(c.tick(1.0 + IDLE_HIDE_SECS * 3.0).render, None);
    assert!(c.can_command());
    let mut c = on();
    media(&mut c, 1.0, spotify(PlayState::Paused, "a"));
    c.set_hover(true, 2.0);
    assert_eq!(c.hide_deadline(), None, "the pointer keeps it open");
    assert_eq!(c.tick(2.0 + IDLE_HIDE_SECS * 3.0).render, None);
    let mut c = Ctl::new();
    assert_eq!(c.hide_deadline(), None);
    assert_eq!(c.tick(1000.0), Out::default());
}

#[test]
fn the_tick_event_routes_to_the_idle_check() {
    let mut c = on();
    media(&mut c, 1.0, spotify(PlayState::Paused, "a"));
    let out = c.on_event(1.0 + IDLE_HIDE_SECS + 1.0, Event::Tick);
    assert!(!out.render.unwrap().visible);
}
