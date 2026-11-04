use super::*;

fn flat(side: usize, c: [u8; 4]) -> Vec<u8> {
    c.iter().copied().cycle().take(side * side * 4).collect()
}

fn px(img: &[u8], side: usize, x: usize, y: usize) -> [u8; 4] {
    let i = (y * side + x) * 4;
    [img[i], img[i + 1], img[i + 2], img[i + 3]]
}

// ---------------------------------------------------------------- the tint

#[test]
fn a_red_cover_gives_a_bright_red_tint() {
    let t = dominant_tint(&flat(8, [200, 30, 40, 255]));
    assert!(t[0] >= 216 && t[1] < 130 && t[2] < 130, "{t:?}");
}

#[test]
fn grey_black_white_and_empty_covers_fall_back_to_the_accent() {
    for c in [[128, 128, 128, 255], [0, 0, 0, 255], [255, 255, 255, 255], [10, 10, 12, 255]] {
        assert_eq!(dominant_tint(&flat(8, c)), ACCENT, "{c:?}");
    }
    assert_eq!(dominant_tint(&[]), ACCENT);
    assert_eq!(dominant_tint(&flat(8, [255, 0, 0, 0])), ACCENT, "transparent pixels do not count");
}

#[test]
fn the_hue_family_with_the_most_colour_wins_not_the_muddy_average() {
    // 3/4 blue, 1/4 red: the average would be purple, the answer is blue
    let mut img = flat(8, [20, 60, 220, 255]);
    for p in img.chunks_exact_mut(4).take(16) {
        p.copy_from_slice(&[220, 30, 30, 255]);
    }
    let t = dominant_tint(&img);
    assert!(t[2] > t[0] && t[2] > t[1], "{t:?}");
}

#[test]
fn a_small_saturated_area_beats_a_large_grey_one() {
    let mut img = flat(8, [120, 120, 124, 255]);
    for p in img.chunks_exact_mut(4).take(10) {
        p.copy_from_slice(&[40, 200, 170, 255]);
    }
    let t = dominant_tint(&img);
    assert!(t[1] > t[0] && t[2] > t[0], "teal, not grey: {t:?}");
}

#[test]
fn a_dull_dark_colour_is_lifted_to_read_on_black() {
    let t = dominant_tint(&flat(8, [60, 20, 90, 255]));
    assert!(t.iter().copied().max().unwrap() >= 216, "bright: {t:?}");
    assert!(t[2] > t[1], "still purple: {t:?}");
}

// ---------------------------------------------------------------- image helpers

#[test]
fn downscaling_averages_blocks_and_keeps_flat_images_flat() {
    let mut img = vec![0u8; 4 * 4 * 4];
    for (i, p) in img.chunks_exact_mut(4).enumerate() {
        let v = if (i % 4) < 2 { 0 } else { 200 };
        p.copy_from_slice(&[v, v, v, 255]);
    }
    let d = downscale(&img, 4, 4, 2, 2);
    assert_eq!(px(&d, 2, 0, 0), [0, 0, 0, 255]);
    assert_eq!(px(&d, 2, 1, 1), [200, 200, 200, 255]);
    assert_eq!(downscale(&flat(10, [9, 8, 7, 255]), 10, 10, 3, 3), flat(3, [9, 8, 7, 255]));
    assert_eq!(downscale(&[], 4, 4, 2, 2), vec![0u8; 16], "bad input: transparent, not a panic");
}

#[test]
fn blurring_spreads_a_point_and_leaves_a_flat_image_alone() {
    let mut a = flat(9, [50, 50, 50, 255]);
    let before = a.clone();
    box_blur(&mut a, 9, 9, 2, 2);
    assert_eq!(a, before);
    let mut b = flat(9, [0, 0, 0, 255]);
    b[(4 * 9 + 4) * 4..(4 * 9 + 4) * 4 + 3].copy_from_slice(&[255, 255, 255]);
    box_blur(&mut b, 9, 9, 1, 1);
    assert!(px(&b, 9, 4, 4)[0] < 255 && px(&b, 9, 4, 4)[0] > 0, "the point is dimmer");
    assert!(px(&b, 9, 3, 4)[0] > 0 && px(&b, 9, 4, 3)[0] > 0, "and its neighbours are lit");
    assert_eq!(px(&b, 9, 0, 0)[0], 0, "far away stays dark");
    box_blur(&mut [], 0, 0, 1, 1);
}

#[test]
fn saturating_moves_colour_away_from_grey_and_leaves_grey_alone() {
    let mut grey = vec![100, 100, 100, 255];
    saturate(&mut grey, 1.5);
    assert_eq!(grey, vec![100, 100, 100, 255]);
    let mut c = vec![200, 100, 100, 255];
    saturate(&mut c, 1.5);
    assert!(c[0] > 200 && c[1] < 100 && c[3] == 255);
    let mut same = vec![200, 100, 100, 255];
    saturate(&mut same, 1.0);
    assert_eq!(same, vec![200, 100, 100, 255]);
}

// ---------------------------------------------------------------- the disc

const S: usize = 128;

#[test]
fn the_disc_is_a_circle_with_a_hole_and_a_tinted_ring_in_the_middle() {
    let tint = [240, 83, 95];
    let d = compose_disc(&flat(S, [30, 90, 200, 255]), S, tint);
    assert_eq!(px(&d, S, 0, 0)[3], 0, "the corners are transparent");
    assert_eq!(px(&d, S, S - 1, 0)[3], 0);
    assert_eq!(px(&d, S, S / 2, S / 2), [5, 5, 5, 255], "the hole is #050505");
    // 15 % wide hole: radius 9.6 px, then a 2 px ring of tint
    assert_eq!(px(&d, S, S / 2 + 10, S / 2)[..3], tint, "the ring");
    let rim = px(&d, S, S / 2, 1);
    assert!(rim[3] == 255, "inside the rim the disc is solid: {rim:?}");
    let cover = px(&d, S, S / 2 + 30, S / 2);
    assert!(cover[2] > cover[0], "the cover shows between the grooves and the hole: {cover:?}");
}

#[test]
fn the_edge_is_antialiased_and_the_result_is_premultiplied() {
    let d = compose_disc(&flat(S, [200, 200, 200, 255]), S, ACCENT);
    let mut partial = 0;
    for p in d.chunks_exact(4) {
        if p[3] > 0 && p[3] < 255 {
            partial += 1;
            assert!(p[0] as u32 <= p[3] as u32 + 1, "premultiplied: {p:?}");
        }
        if p[3] == 0 {
            assert_eq!(&p[..3], &[0, 0, 0]);
        }
    }
    assert!(partial > 40, "a soft rim: {partial} edge pixels");
}

#[test]
fn the_grooves_darken_rings_of_the_cover() {
    let d = compose_disc(&flat(S, [200, 200, 200, 255]), S, ACCENT);
    let mut values: Vec<u8> = (0..30).map(|i| px(&d, S, S / 2 + 20 + i, S / 2)[0]).collect();
    values.sort_unstable();
    values.dedup();
    assert!(values.len() == 2 && values[0] < values[1], "light and groove only: {values:?}");
    assert!(values[0] >= 150, "a 20 % black ring: {values:?}");
}

#[test]
fn bad_input_gives_a_transparent_disc_not_a_panic() {
    assert_eq!(compose_disc(&[1, 2, 3], 8, ACCENT), vec![0u8; 8 * 8 * 4]);
    assert!(compose_disc(&[], 0, ACCENT).is_empty());
}

#[test]
fn a_look_has_a_disc_a_blurred_opaque_halo_and_a_tint() {
    let src = flat(DISC_PX, [200, 30, 40, 255]);
    let look = look_from_square(&src);
    assert_eq!((look.disc.len(), look.halo.len()), (DISC_PX * DISC_PX * 4, HALO_PX * HALO_PX * 4));
    let c = (HALO_PX / 2 * HALO_PX + HALO_PX / 2) * 4;
    assert_eq!(look.halo[c + 3], 255, "solid in the middle");
    assert_eq!(look.halo[3], 0, "and gone at the corner");
    assert!(look.tint[0] >= 216);
    assert_eq!(px(&look.disc, DISC_PX, DISC_PX / 2, DISC_PX / 2), [5, 5, 5, 255]);
    let ph = placeholder_look();
    assert_eq!(ph.tint, ACCENT);
    assert_eq!(ph.disc.len(), DISC_PX * DISC_PX * 4);
}

// ---------------------------------------------------------------- the ambient light

#[test]
fn the_halo_fades_in_with_the_card_swells_with_a_new_track_and_drifts() {
    let (a0, _, _) = halo_params(0.0, 0.0, 0.0, false);
    let (a1, _, _) = halo_params(1.0, 0.0, 0.0, false);
    assert_eq!((a0, a1), (0.0, 0.7), "opacity .7 when open");
    assert!((halo_params(0.0, 1.0, 0.0, false).0 - 0.25).abs() < 1e-9, "the pulse adds .25");
    assert_eq!(halo_params(2.0, 2.0, 0.0, false).0, 0.95, "ex and pulse are clamped");
    let (_, dx, dy) = halo_params(1.0, 0.0, 5.0, false);
    assert!(dx.abs() <= 14.0 && dy.abs() <= 10.0 && (dx != 0.0 || dy != 0.0));
    assert_eq!(halo_params(1.0, 0.0, 5.0, true).1, 0.0, "Reduce Motion: no drift");
}

#[test]
fn the_light_under_the_card_follows_its_size_and_the_swell() {
    let (r, a) = bleed_params(430.0, 216.0, 1.0, 0.0);
    assert!(
        (r[0] - 34.4).abs() < 1e-9 && (r[2] - 361.2).abs() < 1e-9 && (r[1] - 75.6).abs() < 1e-9 && (r[3] - 183.6).abs() < 1e-9,
        "{r:?}"
    );
    assert!((a - 0.65).abs() < 1e-9);
    let (_, rest) = bleed_params(280.0, 32.0, 0.0, 0.0);
    assert!((rest - 0.3).abs() < 1e-9, "a faint glow even collapsed");
    assert!((bleed_params(280.0, 32.0, 0.0, 1.0).1 - 0.6).abs() < 1e-9);
    assert!((bleed_params(1.0, 1.0, 5.0, 5.0).1 - 0.95).abs() < 1e-9, "ex and pulse are clamped");
}

#[test]
fn the_halo_edge_dissolves_and_the_colour_is_premultiplied() {
    let mut px = flat(20, [200, 100, 50, 255]);
    fade_edges(&mut px, 20, 20);
    assert_eq!(px[3], 0, "the corner is clear");
    assert_eq!(px_at(&px, 20, 10, 10), [200, 100, 50, 255], "the middle is untouched");
    let (mut last, mut rising) = (0u8, true);
    for x in 0..10 {
        let a = px_at(&px, 20, x, 10)[3];
        rising &= a >= last;
        last = a;
    }
    assert!(rising && last == 255, "a smooth ramp inward");
    for p in px.chunks_exact(4) {
        assert!(p[0] <= p[3] && p[1] <= p[3] && p[2] <= p[3] || p[3] == 255, "premultiplied: {p:?}");
    }
}

fn px_at(img: &[u8], side: usize, x: usize, y: usize) -> [u8; 4] {
    px(img, side, x, y)
}
