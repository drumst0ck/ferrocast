use super::*;

fn overlaps(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
}

fn inside(r: [f64; 4], w: f64, h: f64) -> bool {
    r[0] >= 0.0 && r[1] >= 0.0 && r[0] + r[2] <= w + 1e-9 && r[1] + r[3] <= h + 1e-9 && r[2] > 0.0 && r[3] > 0.0
}

/// The artboard's notch: 200 x 32.
const NOTCH: Notch = Notch { width: 200.0, height: 32.0 };

#[test]
fn a_notch_is_found_from_the_safe_area() {
    // 14-inch MacBook Pro: 1512 pt wide, 32 pt safe-area top, 653.5 pt of menu bar each side
    let n = notch_from_screen(1512.0, 32.0, 653.5, 653.5).unwrap();
    assert_eq!((n.width, n.height), (205.0, 32.0));
}

#[test]
fn no_notch_without_a_safe_area_or_with_nonsense() {
    assert_eq!(notch_from_screen(1920.0, 0.0, 0.0, 0.0), None);
    assert_eq!(notch_from_screen(1920.0, 24.0, 960.0, 960.0), None, "no gap between the areas");
    assert_eq!(notch_from_screen(1512.0, 32.0, 0.0, 0.0), None, "no auxiliary areas reported");
    assert_eq!(notch_from_screen(1512.0, 32.0, 900.0, 900.0), None, "areas overlap");
}

// ---------------------------------------------------------------- the artboard's sizes

#[test]
fn the_sizes_are_the_artboards() {
    assert_eq!(collapsed_size(Some(NOTCH)), (280.0, 32.0), "Wc 280 x Hc 32 around a 200 pt notch");
    assert_eq!(expanded_size(), (430.0, 216.0), "We 430 x He 216");
    let (w, h) = collapsed_size(None);
    assert!(w > 120.0 && w < 260.0 && (24.0..=34.0).contains(&h), "a plain capsule without a notch: {w}x{h}");
    assert_eq!((RC, RE), (16.0, 36.0), "bottom radius 16 to 36");
}

#[test]
fn the_panel_hangs_flush_from_the_top_centre_with_room_below_for_the_light() {
    // Cocoa frame of a display: x, y(bottom), w, h
    let f = panel_frame([1440.0, -480.0, 1920.0, 1080.0], (400.0, 100.0));
    assert_eq!(f, [1440.0 + 760.0, -480.0 + 1080.0 - 100.0 - BLEED_PAD, 400.0, 100.0 + BLEED_PAD]);
}

#[test]
fn the_expanded_layout_is_the_artboards_and_nothing_overlaps() {
    for lines in [1, 2] {
        let e = expanded_layout(lines);
        assert_eq!(e.disc, [28.0, 56.0, 128.0, 128.0]);
        assert_eq!(e.ring, [16.0, 44.0, 152.0, 152.0]);
        assert_eq!(e.eq, [382.0, 9.0, 30.0, 14.0]);
        assert_eq!((e.prev[2], e.play[2], e.next[2]), (40.0, 52.0, 40.0), "buttons 40 / 52 / 40");
        assert_eq!(e.title[0..3], [192.0, 48.0, 214.0]);
        let all = [e.disc, e.ring, e.eq, e.chip_icon, e.chip, e.title, e.artist, e.prev, e.play, e.next, e.elapsed, e.bar, e.remaining];
        for r in all {
            assert!(inside(r, CARD_W, CARD_H), "{r:?} outside the card");
        }
        let pairs = [
            (e.title, e.artist),
            (e.artist, e.prev),
            (e.artist, e.play),
            (e.prev, e.play),
            (e.play, e.next),
            (e.next, e.elapsed),
            (e.play, e.bar),
            (e.elapsed, e.bar),
            (e.bar, e.remaining),
            (e.chip, e.eq),
            (e.disc, e.title),
            (e.ring, e.title),
            (e.ring, e.prev),
        ];
        for (a, b) in pairs {
            assert!(!overlaps(a, b), "{lines} lines: {a:?} overlaps {b:?}");
        }
        // the 32 pt strip at the top is the notch zone: only the chip and the equalizer live there
        for r in [e.disc, e.ring, e.title, e.artist, e.prev, e.play, e.next, e.elapsed, e.bar, e.remaining] {
            assert!(r[1] >= 32.0, "{r:?} reaches into the notch strip");
        }
        // and the middle of the strip stays free for the physical notch
        let free = [(CARD_W - 200.0) / 2.0, (CARD_W + 200.0) / 2.0];
        for r in [e.chip_icon, e.chip, e.eq] {
            assert!(r[0] + r[2] <= free[0] || r[0] >= free[1], "{r:?} sits behind the notch");
        }
    }
}

#[test]
fn the_title_takes_one_or_two_lines_and_the_artist_follows_it() {
    let one = expanded_layout(1);
    let two = expanded_layout(2);
    assert!((one.title[3] - 28.84).abs() < 1e-9 && (two.title[3] - 57.68).abs() < 1e-9);
    assert!((one.artist[1] - (48.0 + 28.84 + 7.0)).abs() < 1e-9, "7 pt under the title");
    assert!(two.artist[1] > one.artist[1]);
    assert_eq!(expanded_layout(0).title, one.title, "at least one line");
    assert_eq!(expanded_layout(9).title, two.title, "at most two lines");
}

#[test]
fn the_time_row_keeps_the_seek_bar_between_the_times() {
    let e = expanded_layout(1);
    assert_eq!((e.elapsed[0], e.elapsed[1]), (192.0, 196.0), "the artboard's time row");
    assert!((e.remaining[0] + e.remaining[2] - (192.0 + 214.0)).abs() < 1e-9, "ends at the title column's edge");
    assert!(e.bar[0] >= e.elapsed[0] + e.elapsed[2] && e.bar[0] + e.bar[2] <= e.remaining[0]);
    assert!((e.bar[1] + e.bar[3] / 2.0 - (e.elapsed[1] + e.elapsed[3] / 2.0)).abs() < 0.6, "on the row's centre line");
}

#[test]
fn the_parts_travel_from_the_pill_to_the_card() {
    let coll = collapsed_size(Some(NOTCH));
    let o = (CARD_W - coll.0) / 2.0; // the content frame is 430 wide, centred on the pill
    let c = parts_at(0.0, coll);
    assert_eq!((c.disc[0] - o, c.disc[1], c.disc[2]), (9.0, 5.0, 22.0), "artboard: cover 22 pt at 9,5");
    assert_eq!((c.ring[0] - o, c.ring[1], c.ring[2]), (8.0, 4.0, 24.0));
    assert_eq!((c.eq[0] - o, c.eq[1], c.eq[2], c.eq[3]), (249.0, 9.0, 22.0, 14.0), "equalizer 22 x 14 on the right");
    let e = parts_at(1.0, coll);
    assert_eq!((e.disc, e.ring, e.eq), ([28.0, 56.0, 128.0, 128.0], [16.0, 44.0, 152.0, 152.0], [382.0, 9.0, 30.0, 14.0]));
    let mid = parts_at(0.5, coll);
    assert!(mid.disc[2] > 22.0 && mid.disc[2] < 128.0);
    let over = parts_at(1.2, coll);
    assert!(over.disc[2] > 128.0, "the spring's overshoot is not clamped");
    assert!(disc_radius(&c) < disc_radius(&e));
}

#[test]
fn the_shape_runs_from_the_pill_to_the_card_and_the_height_never_collapses_below_a_bit_less_than_the_notch() {
    let coll = collapsed_size(Some(NOTCH));
    let rest = shape(0.0, 0.0, coll);
    assert_eq!((rest.w, rest.h, rest.radius), (280.0, 32.0, 16.0));
    let full = shape(1.0, 1.0, coll);
    assert_eq!((full.w, full.h, full.radius), (430.0, 216.0, 36.0));
    let under = shape(-0.5, -0.5, coll);
    assert!(under.h >= 32.0 * 0.92 - 1e-9, "the artboard's floor");
    assert!(under.w >= 60.0);
    let mid = shape(1.0, 0.5, coll);
    assert!(mid.w == 430.0 && (mid.h - 124.0).abs() < 1e-9 && mid.radius == 36.0, "the radius follows the larger of the two");
    assert!(shape(0.5, 0.5, coll).radius <= shape(0.5, 0.5, coll).h / 2.0, "never more than half the height");
}

// ---------------------------------------------------------------- springs

fn run_open(fps: f64, rm: bool) -> (Motion, f64, usize) {
    let mut m = Motion::new();
    m.set_open(true, rm);
    let (mut peak, mut frames) = (0.0f64, 0usize);
    while frames < 1200 {
        frames += 1;
        m.step(1.0 / fps, rm);
        peak = peak.max(m.pw.x);
        if m.at_rest() {
            break;
        }
    }
    (m, peak, frames)
}

#[test]
fn the_artboards_defaults() {
    let c = MotionCfg::default();
    assert_eq!((c.k, c.c, c.ov, c.sq, c.stag_ms, c.lead_ms, c.hold_ms, c.pulse), (150.0, 14.0, 0.8, 0.035, 70.0, 90.0, 400.0, 1.0));
}

#[test]
fn opening_springs_past_the_card_and_settles_exactly_on_it() {
    for fps in [60.0, 120.0] {
        let (m, peak, frames) = run_open(fps, false);
        assert!(peak > 1.04 && peak < 1.4, "clear but bounded overshoot at {fps} fps: {peak}");
        let secs = frames as f64 / fps;
        assert!(secs > 0.3 && secs < 1.6, "settles: {secs} s");
        assert_eq!((m.pw.x, m.ph.x), (1.0, 1.0), "snaps exactly on the target at rest");
        assert!(m.items.iter().all(|i| i.x == 1.0));
    }
}

#[test]
fn the_result_does_not_depend_on_the_frame_rate() {
    let at = |fps: f64| {
        let mut m = Motion::new();
        m.set_open(true, false);
        for _ in 0..(fps * 0.3) as usize {
            m.step(1.0 / fps, false);
        }
        (m.pw.x, m.ph.x)
    };
    let (a, b) = (at(60.0), at(120.0));
    assert!((a.0 - b.0).abs() < 0.03 && (a.1 - b.1).abs() < 0.03, "{a:?} {b:?}");
}

#[test]
fn the_height_lags_the_width_by_the_lead_time() {
    let mut m = Motion::new();
    m.set_open(true, false);
    for _ in 0..20 {
        m.step(0.004, false); // 80 ms: before the 90 ms lead
    }
    assert!(m.pw.x > 0.02, "the width has started");
    assert_eq!(m.ph.t, 0.0, "the height has not been told to open yet");
    assert_eq!(m.ph.x, 0.0);
    for _ in 0..10 {
        m.step(0.004, false);
    }
    assert_eq!(m.ph.t, 1.0, "after the lead the height follows");
    for _ in 0..25 {
        m.step(0.004, false);
    }
    assert!(m.ph.x > 0.0 && m.ph.x < m.pw.x, "and trails the width");
}

#[test]
fn the_content_arrives_staggered_one_item_after_another() {
    let mut m = Motion::new();
    m.set_open(true, false);
    let mut start = [None; ITEMS];
    for f in 0..400 {
        m.step(0.002, false);
        for (i, s) in start.iter_mut().enumerate() {
            if s.is_none() && m.items[i].t == 1.0 {
                *s = Some((f + 1) as f64 * 0.002);
            }
        }
    }
    let want = [0.054, 0.124, 0.194, 0.264]; // lead 90 ms x .6 + index x 70 ms
    for i in 0..ITEMS {
        assert!((start[i].unwrap() - want[i]).abs() < 0.004, "item {i}: {:?} vs {}", start[i], want[i]);
    }
}

#[test]
fn closing_hands_the_content_back_first_then_the_height_then_the_width() {
    let mut m = Motion::new();
    m.set_open(true, true);
    assert_eq!((m.pw.x, m.ph.x), (1.0, 1.0));
    m.set_open(false, false);
    assert_eq!(m.ph.t, 0.0, "the height starts at once");
    assert_eq!(m.pw.t, 1.0, "the width waits for the lead");
    let mut t_items = [None; ITEMS];
    for f in 0..100 {
        m.step(0.002, false);
        for (i, s) in t_items.iter_mut().enumerate() {
            if s.is_none() && m.items[i].t == 0.0 {
                *s = Some((f + 1) as f64 * 0.002);
            }
        }
    }
    assert!(t_items[3].unwrap() < t_items[0].unwrap(), "the last item leaves first: {t_items:?}");
    assert!(t_items[0].unwrap() < 0.06, "10 ms per step");
    assert_eq!(m.pw.t, 0.0, "after the lead the width follows");
}

#[test]
fn closing_is_springy_too_and_ends_exactly_in_the_notch() {
    let mut m = Motion::new();
    m.set_open(true, true);
    m.set_open(false, false);
    let mut lowest = 1.0f64;
    for _ in 0..600 {
        m.step(1.0 / 60.0, false);
        lowest = lowest.min(m.pw.x);
        if m.at_rest() {
            break;
        }
    }
    assert!(m.at_rest() && (m.pw.x, m.ph.x) == (0.0, 0.0));
    assert!(lowest < -0.005, "a small bounce past the notch: {lowest}");
}

#[test]
fn leaving_waits_for_the_hold_and_coming_back_cancels_the_close() {
    let mut m = Motion::new();
    m.set_expanded(true, false);
    for _ in 0..300 {
        m.step(0.004, false);
    }
    m.set_expanded(false, false);
    for _ in 0..95 {
        m.step(0.004, false); // 380 ms
    }
    assert_eq!(m.pw.t, 1.0, "still open inside the 400 ms hold");
    m.set_expanded(true, false);
    for _ in 0..200 {
        m.step(0.004, false);
    }
    assert_eq!(m.pw.t, 1.0, "the pointer came back: no close");
    m.set_expanded(false, false);
    for _ in 0..105 {
        m.step(0.004, false); // 420 ms
    }
    assert_eq!(m.ph.t, 0.0, "the hold passed: closing");
}

#[test]
fn reduce_motion_is_instant_with_no_springs_no_stagger_no_squash_and_no_spin() {
    let (m, peak, frames) = run_open(60.0, true);
    assert_eq!((m.pw.x, m.ph.x, peak, frames), (1.0, 1.0, 1.0, 1), "one frame, no overshoot");
    assert!(m.items.iter().all(|i| i.x == 1.0));
    assert_eq!(m.squash(true), (1.0, 1.0));
    for i in 0..ITEMS {
        assert_eq!(m.item(i, true), (1.0, 1.0, 0.0), "no scale, no slide");
    }
    let mut m = Motion::new();
    m.step(1.0, true);
    assert_eq!(m.angle, 0.0, "the disc does not turn");
    assert!(!m.spinning(true));
}

#[test]
fn items_fade_scale_and_slide_in() {
    let mut m = Motion::new();
    assert_eq!(m.item(1, false), (0.0, 0.9, 10.0), "scale .9 and 10 pt down at the start");
    m.items[1].x = 1.0;
    assert_eq!(m.item(1, false), (1.0, 1.0, 0.0));
    m.items[1].x = 1.2;
    let (a, s, dy) = m.item(1, false);
    assert_eq!((a, s > 1.0, dy < 0.0), (1.0, true, true), "overshoot is allowed, opacity is not");
}

#[test]
fn the_bubble_squashes_with_the_velocity_within_the_limit() {
    let mut m = Motion::new();
    assert_eq!(m.squash(false), (1.0, 1.0));
    m.pw.v = 1000.0;
    let (sx, sy) = m.squash(false);
    assert!((sx - 1.035).abs() < 1e-9 && (sy - 0.965).abs() < 1e-9, "capped at 3.5 %: {sx} {sy}");
    m.pw.v = -1000.0;
    let (sx, sy) = m.squash(false);
    assert!(sx < 1.0 && sy > 1.0);
}

#[test]
fn a_new_track_pulses_the_pill_once_and_restarts_the_title() {
    let mut m = Motion::new();
    m.track_changed(false);
    let mut peak = 0.0f64;
    for _ in 0..600 {
        m.step(1.0 / 120.0, false);
        peak = peak.max(m.pulse.x);
    }
    assert!(peak > 0.1, "it swells: {peak}");
    assert!(m.at_rest() && m.pulse.x == 0.0, "and settles");
    let mut m = Motion::new();
    m.set_open(true, true);
    m.track_changed(false);
    assert_eq!((m.items[1].x, m.items[1].v), (0.1, 0.0), "the title slides in again while open");
    assert_eq!(m.items[0].x, 1.0);
    let mut m = Motion::new();
    m.track_changed(false);
    let (sx, sy) = m.squash(false);
    assert!(sx == 1.0 && sy == 1.0, "no motion yet at the first instant");
    m.step(1.0 / 120.0, false);
    let (sx, sy) = m.squash(false);
    assert!(sx > 1.0 && sy > 1.0 && (sx - 1.0) > (sy - 1.0), "wider than taller (pulse .025 / .015)");
}

// ---------------------------------------------------------------- the disc

fn spin_for(m: &mut Motion, secs: f64) {
    for _ in 0..(secs * 120.0) as usize {
        m.step(1.0 / 120.0, false);
    }
}

#[test]
fn the_disc_turns_about_sixty_degrees_a_second_while_playing() {
    let mut m = Motion::new();
    spin_for(&mut m, 1.0);
    assert!((m.angle - 60.0).abs() < 1.0, "{}", m.angle);
    assert!(m.spinning(false));
    spin_for(&mut m, 10.0);
    assert!(m.angle >= 0.0 && m.angle < 360.0);
}

#[test]
fn pausing_coasts_to_a_stop_instead_of_halting() {
    let mut m = Motion::new();
    spin_for(&mut m, 0.5);
    m.set_playing(false);
    spin_for(&mut m, 0.25);
    assert!(m.om.x > 5.0 && m.om.x < 60.0, "still coasting after a quarter second: {}", m.om.x);
    spin_for(&mut m, 8.0);
    assert!(m.om.x.abs() < 1.0, "stopped: {}", m.om.x);
    m.set_playing(true);
    spin_for(&mut m, 4.0);
    assert!((m.om.x - 60.0).abs() < 3.0, "and it picks up again");
}

#[test]
fn a_new_track_flicks_the_disc_to_seven_times_the_speed_and_it_settles_back() {
    let mut m = Motion::new();
    spin_for(&mut m, 1.0);
    m.track_changed(true);
    assert_eq!(m.om.x, 420.0);
    spin_for(&mut m, 6.0);
    assert!((m.om.x - 60.0).abs() < 6.0, "{}", m.om.x);
}

#[test]
fn the_ring_is_a_clockwise_sweep_from_the_top() {
    assert_eq!(ring_end_degrees(0.0), 0.0);
    assert_eq!(ring_end_degrees(0.25), 90.0);
    assert_eq!(ring_end_degrees(1.0), 360.0);
    assert_eq!(ring_end_degrees(1.7), 360.0);
    assert_eq!(ring_end_degrees(-1.0), 0.0);
    assert_eq!(ring_end_degrees(f64::NAN), 0.0);
    // artboard stroke widths: 2.4 % track and 3 % arc of the ring's diameter
    assert!((ring_stroke(152.0).0 - 3.648).abs() < 1e-9 && (ring_stroke(152.0).1 - 4.56).abs() < 1e-9);
}

#[test]
fn play_pause_morphs_between_a_triangle_and_two_bars() {
    assert_eq!(pp_points(0.0), PP_PLAY);
    assert_eq!(pp_points(1.0), PP_PAUSE);
    let mid = pp_points(0.5);
    for i in 0..16 {
        assert!((mid[i] - (PP_PLAY[i] + PP_PAUSE[i]) / 2.0).abs() < 1e-9);
    }
    assert_eq!(pp_points(-5.0), pp_points(-0.1), "the spring may overshoot a little, not a lot");
    assert_eq!(pp_points(5.0), pp_points(1.1));
}

#[test]
fn the_equalizer_rests_flat_when_paused_and_dances_when_playing() {
    for i in 0..4 {
        assert!((eq_scale(1.234, i, 0.0, false) - 0.16).abs() < 1e-9, "paused: a flat line");
        let v = eq_scale(1.234, i, 1.0, false);
        assert!((0.16..=1.0).contains(&v));
        assert_eq!(eq_scale(0.0, i, 1.0, true), eq_scale(9.0, i, 1.0, true), "Reduce Motion: static");
    }
    assert_ne!(eq_scale(0.0, 0, 1.0, false), eq_scale(0.37, 0, 1.0, false));
    assert!((eq_scale(0.0, 0, 1.0, true) - (0.16 + 0.44)).abs() < 1e-9);
    assert!((eq_scale(0.0, 0, 0.0, true) - 0.16).abs() < 1e-9);
}

#[test]
fn the_timer_runs_at_the_display_rate_while_springs_move_and_calmly_while_the_disc_turns() {
    let t = |a, s, p, e, rm, fps| tick_interval(a, s, p, e, rm, fps);
    assert_eq!(t(false, false, false, false, false, 60.0), None, "idle and paused: no redraws");
    assert!((t(true, false, false, false, false, 120.0).unwrap() - 1.0 / 120.0).abs() < 1e-9);
    assert!((t(true, false, false, false, false, 30.0).unwrap() - 1.0 / 60.0).abs() < 1e-9, "never below 60 Hz");
    assert!((t(true, false, false, false, false, 240.0).unwrap() - 1.0 / 120.0).abs() < 1e-9, "never above 120 Hz");
    let spin = t(false, true, true, false, false, 120.0).unwrap();
    assert!(spin > 1.0 / 61.0 && spin < 0.1, "the disc: a calm rate {spin}");
    assert_eq!(t(false, false, true, false, true, 60.0), None, "Reduce Motion: static bars, no spin");
    assert!(t(false, false, true, true, true, 60.0).is_some(), "the progress still moves while expanded");
}

// ---------------------------------------------------------------- text, item transforms, gloss

/// Ten points per character.
fn ten(s: &str) -> f64 {
    s.chars().count() as f64 * 10.0
}

#[test]
fn a_short_title_stays_on_one_line() {
    assert_eq!(wrap_lines("Nightcall", 214.0, 2, &ten), vec!["Nightcall"]);
    assert_eq!(wrap_lines("", 214.0, 2, &ten), Vec::<String>::new());
    assert_eq!(wrap_lines("  ", 214.0, 2, &ten), Vec::<String>::new());
}

#[test]
fn a_long_title_wraps_at_words_onto_two_lines() {
    // 21 characters fit in 214 pt at ten points each
    let lines = wrap_lines("Rust sin miedo ahora mismo y siempre", 214.0, 2, &ten);
    assert_eq!(lines.len(), 2);
    assert!(lines.iter().all(|l| ten(l) <= 214.0), "{lines:?}");
    assert_eq!(lines[0], "Rust sin miedo ahora");
}

#[test]
fn the_second_line_ends_with_an_ellipsis_when_there_is_more() {
    let lines = wrap_lines("one two three four five six seven eight nine ten eleven twelve", 214.0, 2, &ten);
    assert_eq!(lines.len(), 2);
    assert!(lines[1].ends_with('…') && ten(&lines[1]) <= 214.0, "{lines:?}");
}

#[test]
fn a_word_longer_than_the_line_is_broken_and_one_line_mode_truncates() {
    let l = wrap_lines("Supercalifragilisticexpialidocious", 100.0, 2, &ten);
    assert!(l.len() == 2 && l.iter().all(|x| ten(x) <= 100.0), "{l:?}");
    let one = wrap_lines("A title that does not fit in one line", 150.0, 1, &ten);
    assert_eq!(one.len(), 1);
    assert!(one[0].ends_with('…') && ten(&one[0]) <= 150.0, "{one:?}");
    assert_eq!(truncate_to_width("Código Fuente · Ep. 87", 100.0, &ten).chars().count(), 10);
    assert_eq!(truncate_to_width("short", 100.0, &ten), "short");
}

#[test]
fn item_transforms_scale_about_their_origin_and_slide_down() {
    let r = [100.0, 50.0, 40.0, 20.0];
    assert_eq!(xf_rect(r, [100.0, 60.0], 1.0, 0.0), r);
    assert_eq!(xf_rect(r, [100.0, 60.0], 0.5, 0.0), [100.0, 55.0, 20.0, 10.0], "left-middle stays put");
    assert_eq!(xf_rect(r, [100.0, 60.0], 1.0, 10.0), [100.0, 60.0, 40.0, 20.0]);
}

#[test]
fn the_gloss_is_two_soft_highlights_on_the_conic_gradient() {
    // conic-gradient(from 30deg, transparent 0 30%, white .17 at 38%, transparent 46% 80%, white .1 at 88%, transparent 95%)
    assert_eq!(gloss_alpha(30.0), 0.0);
    assert_eq!(gloss_alpha(30.0 + 0.2 * 360.0), 0.0);
    assert!((gloss_alpha(30.0 + 0.38 * 360.0) - 0.17).abs() < 1e-9);
    assert_eq!(gloss_alpha(30.0 + 0.6 * 360.0), 0.0);
    assert!((gloss_alpha(30.0 + 0.88 * 360.0) - 0.10).abs() < 1e-9);
    assert!(gloss_alpha(30.0 + 0.34 * 360.0) > 0.0 && gloss_alpha(30.0 + 0.34 * 360.0) < 0.17);
    assert!((gloss_alpha(30.0 + 360.0 + 0.38 * 360.0) - gloss_alpha(30.0 + 0.38 * 360.0)).abs() < 1e-9, "periodic");
    assert!((gloss_alpha(30.0 + 0.38 * 360.0 - 360.0) - gloss_alpha(30.0 + 0.38 * 360.0)).abs() < 1e-9);
}

#[test]
fn a_paused_start_does_not_move_and_a_playing_one_turns() {
    let mut p = Motion::with_playing(false);
    for _ in 0..120 {
        p.step(1.0 / 60.0, false);
    }
    assert_eq!((p.angle, p.pm.x, p.am.x), (0.0, 0.0, 0.0));
    assert!(!p.spinning(false));
    let mut q = Motion::with_playing(true);
    for _ in 0..60 {
        q.step(1.0 / 60.0, false);
    }
    assert!(q.angle > 50.0 && q.spinning(false));
    assert!(!q.is_open());
    q.set_open(true, true);
    assert!(q.is_open());
}
