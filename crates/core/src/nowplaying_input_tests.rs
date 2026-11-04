use super::*;
use crate::nowplaying::{Ctl, PlayState, Snapshot, SEEK_HOLD_SECS};
use crate::nowplaying_view::expanded_layout;

// ---------------------------------------------------------------- cover crop

fn area(r: [f64; 4]) -> f64 {
    r[2] * r[3]
}

#[test]
fn a_wide_image_is_cropped_left_and_right_around_the_centre() {
    assert_eq!(cover_crop(200.0, 100.0), [50.0, 0.0, 100.0, 100.0]);
}

#[test]
fn a_tall_image_is_cropped_top_and_bottom_around_the_centre() {
    assert_eq!(cover_crop(100.0, 300.0), [0.0, 100.0, 100.0, 100.0]);
}

#[test]
fn a_square_image_is_used_whole_and_a_tiny_one_too() {
    assert_eq!(cover_crop(640.0, 640.0), [0.0, 0.0, 640.0, 640.0]);
    assert_eq!(cover_crop(1.0, 1.0), [0.0, 0.0, 1.0, 1.0]);
    assert_eq!(cover_crop(3.0, 1.0), [1.0, 0.0, 1.0, 1.0]);
}

#[test]
fn the_crop_is_always_square_inside_the_image_and_never_stretches() {
    for (w, h) in [(544.0, 306.0), (306.0, 544.0), (1200.0, 1200.0), (2.0, 7.0), (7.0, 2.0)] {
        let r = cover_crop(w, h);
        assert_eq!(r[2], r[3], "square: {r:?}");
        assert!(r[0] >= 0.0 && r[1] >= 0.0 && r[0] + r[2] <= w + 1e-9 && r[1] + r[3] <= h + 1e-9, "inside {w}x{h}: {r:?}");
        assert!((r[2] - w.min(h)).abs() < 1e-9, "uses the full short side");
        assert!(area(r) > 0.0);
    }
}

#[test]
fn an_unknown_or_empty_size_means_the_whole_image() {
    assert_eq!(cover_crop(0.0, 0.0), [0.0; 4]);
    assert_eq!(cover_crop(-5.0, 10.0), [0.0; 4]);
    assert_eq!(cover_crop(f64::NAN, 10.0), [0.0; 4]);
}

// ---------------------------------------------------------------- opening the source app

#[test]
fn only_a_plausible_bundle_id_is_opened() {
    assert_eq!(open_target("com.brave.Browser"), Some("com.brave.Browser"));
    assert_eq!(open_target(" com.spotify.client\n"), Some("com.spotify.client"));
    assert_eq!(open_target("org.videolan.vlc"), Some("org.videolan.vlc"));
    for bad in ["", "   ", "no spaces.allowed", "a/b", "../x", "x;rm", "-flag"] {
        assert_eq!(open_target(bad), None, "{bad:?}");
    }
}

#[test]
fn the_title_and_artist_rows_are_the_clickable_text() {
    for lines in [1, 2] {
        let e = expanded_layout(lines);
        let l = link_rect(&e);
        assert!(l[0] <= e.title[0] && l[0] + l[2] >= e.title[0] + e.title[2]);
        assert!(l[1] <= e.title[1] && l[1] + l[3] >= e.artist[1] + e.artist[3]);
        assert!(contains(l, [e.title[0] + 5.0, e.title[1] + 5.0]));
        assert!(contains(l, [e.artist[0] + 5.0, e.artist[1] + 5.0]));
        assert!(!contains(l, [e.disc[0] + 5.0, e.disc[1] + 5.0]), "the cover is not the link");
        assert!(!contains(l, [e.ring[0] + 20.0, e.ring[1] + 20.0]), "nor the ring");
        assert!(!contains(l, [e.play[0] + 5.0, e.play[1] + 5.0]), "nor the buttons");
        assert!(l[1] + l[3] <= bar_hit_rect(e.bar)[1] - 8.0, "the link area stays clear of the bar's hit area");
    }
}

// ---------------------------------------------------------------- seeking

const BAR: [f64; 4] = [20.0, 100.0, 352.0, 4.0];

#[test]
fn x_maps_to_a_position_clamped_to_the_track() {
    assert_eq!(position_for_x(BAR, 20.0, 200.0), 0.0);
    assert_eq!(position_for_x(BAR, 196.0, 200.0), 100.0);
    assert_eq!(position_for_x(BAR, 372.0, 200.0), 200.0);
    assert_eq!(position_for_x(BAR, -50.0, 200.0), 0.0, "left of the bar");
    assert_eq!(position_for_x(BAR, 900.0, 200.0), 200.0, "right of the bar");
    assert_eq!(position_for_x(BAR, f64::NAN, 200.0), 0.0);
}

#[test]
fn a_track_without_a_duration_is_not_seekable() {
    assert!(seekable(200.0) && seekable(0.5));
    assert!(!seekable(0.0) && !seekable(-1.0) && !seekable(f64::NAN) && !seekable(f64::INFINITY));
    assert!(Scrub::begin(BAR, 100.0, 102.0, 0.0).is_none(), "live streams: nothing to scrub");
}

#[test]
fn the_hit_area_is_taller_than_the_bar_and_centred_on_it() {
    let h = bar_hit_rect(BAR);
    assert!(h[3] >= 14.0 && h[3] <= 16.0, "{h:?}");
    assert_eq!((h[0], h[2]), (BAR[0], BAR[2]));
    assert!((h[1] + h[3] / 2.0 - (BAR[1] + BAR[3] / 2.0)).abs() < 1e-9);
    assert!(contains(h, [100.0, BAR[1] - 5.0]) && contains(h, [100.0, BAR[1] + BAR[3] + 5.0]));
    assert!(!contains(h, [100.0, BAR[1] - 12.0]));
}

#[test]
fn a_click_on_the_bar_starts_a_scrub_at_that_position_and_release_seeks_there() {
    let s = Scrub::begin(BAR, 196.0, 102.0, 200.0).expect("inside the hit area");
    assert_eq!(s.position(), 100.0);
    assert_eq!(s.end(196.0), 100.0);
    assert!(Scrub::begin(BAR, 196.0, 60.0, 200.0).is_none(), "far above the bar");
    assert!(Scrub::begin(BAR, 5.0, 102.0, 200.0).is_none(), "left of the bar");
}

#[test]
fn dragging_only_moves_the_position_and_release_gives_the_one_seek() {
    let mut s = Scrub::begin(BAR, 20.0, 102.0, 200.0).unwrap();
    let mut shown = vec![];
    for x in [60.0, 120.0, 250.0, 372.0, 600.0, 300.0] {
        s.drag(x);
        shown.push(s.position());
    }
    assert!(shown[..4].windows(2).all(|w| w[0] < w[1]), "follows the pointer: {shown:?}");
    assert_eq!(shown[4], 200.0, "clamped at the end");
    assert!(shown[5] < 200.0, "and back");
    // dragging above or below the bar still scrubs: only x matters once it started
    s.drag(196.0);
    assert_eq!(s.position(), 100.0);
    assert_eq!(s.end(300.0), position_for_x(BAR, 300.0, 200.0), "released where the pointer is");
}

// ---------------------------------------------------------------- optimistic elapsed time

fn snap(state: PlayState, pos: f64) -> Snapshot {
    Snapshot {
        bundle_id: "com.brave.Browser".into(),
        state,
        title: "Song".into(),
        artist: "Band".into(),
        album: String::new(),
        duration: 200.0,
        position: pos,
        artwork: None,
    }
}

fn on(s: Snapshot) -> Ctl {
    let mut c = Ctl::new();
    c.set_enabled(true, 0.0);
    c.on_media(1.0, Some(s));
    c
}

#[test]
fn a_seek_moves_the_bar_at_once_and_always_redraws() {
    let mut c = on(snap(PlayState::Paused, 10.0));
    let (secs, out) = c.seek_to(2.0, 150.0).expect("accepted");
    assert_eq!(secs, 150.0);
    let m = out.render.expect("redraw even for a small jump");
    assert_eq!(m.snapshot.unwrap().position, 150.0);
    let (secs, _) = c.seek_to(3.0, 9999.0).unwrap();
    assert_eq!(secs, 200.0, "clamped to the duration");
    assert_eq!(c.seek_to(4.0, -5.0).unwrap().0, 0.0);
}

#[test]
fn a_seek_is_refused_without_a_track_a_duration_or_when_off() {
    let mut off = Ctl::new();
    assert!(off.seek_to(1.0, 5.0).is_none());
    let mut c = on(snap(PlayState::Playing, 10.0));
    c.on_media(2.0, None);
    assert!(c.seek_to(3.0, 5.0).is_none(), "nothing playing");
    let mut s = snap(PlayState::Playing, 10.0);
    s.duration = 0.0;
    let mut live = on(s);
    assert!(live.seek_to(2.0, 5.0).is_none(), "a live stream");
}

#[test]
fn an_old_position_arriving_after_a_seek_does_not_pull_the_bar_back() {
    let mut c = on(snap(PlayState::Playing, 10.0));
    c.seek_to(2.0, 150.0);
    // an update that still carries the position from before the seek
    let out = c.on_media(2.5, Some(snap(PlayState::Playing, 11.0)));
    let shown = out.render.map(|m| m.snapshot.unwrap().position).unwrap_or(150.5);
    assert!(shown > 140.0, "kept near the target: {shown}");
    // the update that confirms the seek is accepted and ends the hold
    c.on_media(3.0, Some(snap(PlayState::Playing, 151.2)));
    let out = c.on_media(4.0, Some(snap(PlayState::Playing, 20.0)));
    assert_eq!(out.render.unwrap().snapshot.unwrap().position, 20.0, "a later real change (the user seeked elsewhere) shows");
}

#[test]
fn the_hold_expires_and_a_new_track_is_never_held() {
    let mut c = on(snap(PlayState::Paused, 10.0));
    c.seek_to(2.0, 150.0);
    let out = c.on_media(2.0 + SEEK_HOLD_SECS + 0.5, Some(snap(PlayState::Paused, 10.0)));
    assert_eq!(out.render.unwrap().snapshot.unwrap().position, 10.0, "the seek never happened: show the truth");
    let mut c = on(snap(PlayState::Paused, 10.0));
    c.seek_to(2.0, 150.0);
    let mut other = snap(PlayState::Paused, 3.0);
    other.title = "Next song".into();
    assert_eq!(c.on_media(2.2, Some(other)).render.unwrap().snapshot.unwrap().position, 3.0);
}

#[test]
fn the_source_of_what_is_shown_is_known_only_while_visible() {
    let mut c = Ctl::new();
    assert_eq!(c.source_bundle(), None);
    c.set_enabled(true, 0.0);
    assert_eq!(c.source_bundle(), None);
    c.on_media(1.0, Some(snap(PlayState::Playing, 1.0)));
    assert_eq!(c.source_bundle().as_deref(), Some("com.brave.Browser"));
    c.on_media(2.0, None);
    assert_eq!(c.source_bundle(), None);
}
