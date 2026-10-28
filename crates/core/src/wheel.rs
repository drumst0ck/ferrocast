//! Drag + key "wheel": the state machine that turns raw mouse/keyboard/pasteboard samples
//! into wheel events, and the hit-testing of wheel segments. Pure; the app feeds samples.

/// Which wheel to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WheelKind {
    /// Conversions and image tools.
    Convert,
    /// File actions (copy, move, terminal, AirDrop…).
    Actions,
    /// File tools (compress, crop, trim, split, merge…).
    Tools,
    /// Target languages for dragged text.
    Translate,
}

/// One sample of the world, taken every few milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Sample {
    pub left_down: bool,
    /// changeCount of the system drag pasteboard.
    pub drag_count: i64,
    /// The drag pasteboard holds file URLs.
    pub has_files: bool,
    /// The drag pasteboard holds plain text (and no files).
    pub has_text: bool,
    pub convert_key: bool,
    /// The translate key: only meaningful during a text drag (Shift would extend the selection).
    pub translate_key: bool,
    pub actions_key: bool,
    pub tools_key: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WheelEvent {
    /// A key was tapped during a file drag: open this wheel.
    Open(WheelKind),
    /// Another wheel key was tapped while a wheel was open.
    Switch(WheelKind),
    /// The mouse was released while a wheel was open.
    Release,
}

/// Remembers the drag in progress between samples.
#[derive(Debug, Default)]
pub struct Tracker {
    down_count: Option<i64>,
    convert_was: bool,
    actions_was: bool,
    tools_was: bool,
    translate_was: bool,
    open: Option<WheelKind>,
}

impl Tracker {
    /// Feed one sample; returns an event when something happened.
    pub fn step(&mut self, s: Sample) -> Option<WheelEvent> {
        let mut ev = None;
        if s.left_down {
            // the drag pasteboard changes when a drag starts after the press
            let base = *self.down_count.get_or_insert(s.drag_count);
            let dragging_files = s.drag_count != base && s.has_files;
            let dragging_text = s.drag_count != base && !s.has_files && s.has_text;
            let translate_tapped = s.translate_key && !self.translate_was;
            let tapped = if s.convert_key && !self.convert_was {
                Some(WheelKind::Convert)
            } else if s.tools_key && !self.tools_was {
                Some(WheelKind::Tools)
            } else if s.actions_key && !self.actions_was {
                Some(WheelKind::Actions)
            } else {
                None
            };
            if let Some(k) = tapped {
                match self.open {
                    // a Translate wheel has nothing to switch to
                    Some(WheelKind::Translate) => {}
                    Some(_) => ev = Some(WheelEvent::Switch(k)),
                    None if dragging_files => {
                        ev = Some(WheelEvent::Open(k));
                        self.open = Some(k);
                    }
                    None => {}
                }
            }
            // text drags have their own key: Shift would extend the selection being dragged
            if self.open.is_none() && dragging_text && translate_tapped {
                ev = Some(WheelEvent::Open(WheelKind::Translate));
                self.open = Some(WheelKind::Translate);
            }
        } else {
            if self.open.is_some() {
                ev = Some(WheelEvent::Release);
            }
            self.down_count = None;
            self.open = None;
        }
        self.convert_was = s.convert_key;
        self.actions_was = s.actions_key;
        self.tools_was = s.tools_key;
        self.translate_was = s.translate_key;
        ev
    }
}

/// Arrow-key step around a ring of `n` segments; starts at 0 when nothing is chosen.
pub fn step_sel(sel: Option<usize>, n: usize, forward: bool) -> usize {
    match (sel, forward) {
        (None, _) => 0,
        (Some(i), true) => (i + 1) % n,
        (Some(i), false) => (i + n - 1) % n,
    }
}

/// `NSEventModifierFlags` bit for a key name used in Settings; 0 (never held) when unknown.
pub fn key_mask(name: &str) -> usize {
    match name {
        "Shift" => 1 << 17,
        "Control" => 1 << 18,
        "Option" => 1 << 19,
        "Command" => 1 << 20,
        _ => 0,
    }
}

/// Segment under `p` for a ring of `n` equal segments starting at 12 o'clock, clockwise.
/// `p` is relative to the centre, y down. `None` outside the ring or in the hole.
pub fn segment_at(px: f32, py: f32, inner: f32, outer: f32, n: usize) -> Option<usize> {
    if n == 0 {
        return None;
    }
    let r = (px * px + py * py).sqrt();
    if r < inner || r > outer {
        return None;
    }
    // angle from 12 o'clock, clockwise, in [0, 2π)
    let a = px.atan2(-py).rem_euclid(std::f32::consts::TAU);
    let step = std::f32::consts::TAU / n as f32;
    // segment i is centred on i * step
    Some(((a + step / 2.0) / step) as usize % n)
}

/// Ring geometry in points from the wheel centre. One source for hit-testing and drawing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Geometry {
    pub r_in: f32,
    pub r_out: f32,
    /// Angular gap left between neighbouring segments, in degrees.
    pub gap_deg: f32,
    /// Radial inset of a drawn sector from the ring edges.
    pub inset: f32,
}

impl Geometry {
    pub const DEFAULT: Geometry = Geometry { r_in: 70.0, r_out: 176.0, gap_deg: 2.5, inset: 2.0 };
}

/// Centre of segment `i` of `n` at radius `r`, relative to the wheel centre (y down).
pub fn segment_center(i: usize, n: usize, r: f32) -> [f32; 2] {
    if n == 0 {
        return [0.0, 0.0];
    }
    let a = i as f32 * std::f32::consts::TAU / n as f32;
    [a.sin() * r, -a.cos() * r]
}

/// Outline of sector `i` of `n`: the outer arc clockwise, then the inner arc back. Empty for an
/// impossible request. `steps` is the tessellation of each arc (at least 1).
pub fn sector_points(i: usize, n: usize, g: &Geometry, steps: usize) -> Vec<[f32; 2]> {
    if n == 0 || i >= n {
        return vec![];
    }
    let step = std::f32::consts::TAU / n as f32;
    let gap = g.gap_deg.to_radians().min(step * 0.9);
    let (a0, a1) = (i as f32 * step - step / 2.0 + gap / 2.0, i as f32 * step + step / 2.0 - gap / 2.0);
    let (ro, ri) = (g.r_out - g.inset, g.r_in + g.inset);
    let k = steps.max(1);
    let at = |a: f32, r: f32| [a.sin() * r, -a.cos() * r];
    let mut v: Vec<[f32; 2]> = (0..=k).map(|j| at(a0 + (a1 - a0) * j as f32 / k as f32, ro)).collect();
    v.extend((0..=k).rev().map(|j| at(a0 + (a1 - a0) * j as f32 / k as f32, ri)));
    v
}

/// Length of one segment's arc at radius `r`.
pub fn arc_len(n: usize, r: f32) -> f32 {
    if n == 0 {
        0.0
    } else {
        std::f32::consts::TAU * r / n as f32
    }
}

/// The text drawn under a segment's icon and its font size, or `None` when the ring has no
/// segments. Every segment is labelled so the wheel is never icon-only: up to eight segments use
/// 11 pt with room for 17 characters; from nine on the sector is narrower, so the font is 10 pt and
/// the label is cut with an ellipsis to the arc width (the hub still shows the full name of the
/// hovered segment).
pub fn ring_label(label: &str, n: usize, r: f32) -> Option<(String, f32)> {
    if n == 0 {
        return None;
    }
    let (size, max_chars) = if n <= 8 { (11.0, 17) } else { (10.0, (((arc_len(n, r) - 8.0) / 5.4) as usize).clamp(6, 17)) };
    let text = if label.chars().count() > max_chars {
        format!("{}\u{2026}", label.chars().take(max_chars - 1).collect::<String>())
    } else {
        label.to_string()
    };
    Some((text, size))
}

/// Ease-out cubic, clamped to 0..=1.
pub fn ease_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// Progress of an animation of `duration_ms` after `elapsed_ms`, eased (ease-out cubic) and
/// clamped to 0..=1. A zero or negative duration is finished immediately (1.0).
pub fn anim_progress(elapsed_ms: f32, duration_ms: f32) -> f32 {
    if duration_ms <= 0.0 {
        return 1.0;
    }
    ease_out_cubic(elapsed_ms / duration_ms)
}

/// Linear interpolation from `a` to `b`.
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a * (1.0 - t) + b * t
}

/// Cross-fade of two things at progress `p` (0..=1): `(outgoing alpha, incoming alpha)`.
pub fn crossfade_alpha(p: f32) -> (f32, f32) {
    let p = p.clamp(0.0, 1.0);
    (1.0 - p, p)
}

/// Start delay of segment `i` when a wheel switches: 12 ms per segment, at most 96 ms.
pub fn stagger_ms(i: usize) -> f32 {
    (i.min(8) * 12) as f32
}

/// Hub text for the files being dragged: a count line and a kind line.
pub fn hub_text(files: &[std::path::PathBuf]) -> (String, String) {
    use crate::convert::{ext, family_of, Family};
    let count = crate::i18n::trn("{n} file", "{n} files", files.len() as u64);
    let Some(first) = files.first() else { return (count, String::new()) };
    let fam = family_of(first);
    if files.iter().any(|f| family_of(f) != fam) {
        return (count, crate::i18n::tr("Mixed").into_owned());
    }
    let sub = if fam == Family::Folder {
        crate::i18n::tr("Folder").into_owned()
    } else if files.len() == 1 {
        let e = ext(first).to_ascii_uppercase();
        match fam {
            Family::Image => format!("{e} image"),
            Family::Video => format!("{e} video"),
            Family::Audio => format!("{e} audio"),
            _ if e.is_empty() => "File".to_string(),
            _ => format!("{e} file"),
        }
    } else {
        match fam {
            Family::Image => "images",
            Family::Video => "videos",
            Family::Audio => "audio files",
            Family::Pdf => "PDFs",
            _ => "files",
        }
        .to_string()
    };
    (count, sub)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(left: bool, count: i64, files: bool, ck: bool, ak: bool) -> Sample {
        Sample {
            left_down: left,
            drag_count: count,
            has_files: files,
            has_text: false,
            translate_key: false,
            convert_key: ck,
            actions_key: ak,
            tools_key: false,
        }
    }

    #[test]
    fn command_opens_tools_and_keys_switch_an_open_wheel() {
        let mut t = Tracker::default();
        let tools = |left, count, on| Sample { left_down: left, drag_count: count, has_files: true, tools_key: on, ..Default::default() };
        t.step(tools(true, 1, false));
        t.step(tools(true, 2, false));
        assert_eq!(t.step(tools(true, 2, true)), Some(WheelEvent::Open(WheelKind::Tools)));
        t.step(tools(true, 2, false));
        // tap Control, then Shift while it stays open: switches, never re-opens
        assert_eq!(t.step(s(true, 2, true, false, true)), Some(WheelEvent::Switch(WheelKind::Actions)));
        t.step(s(true, 2, true, false, false));
        assert_eq!(t.step(s(true, 2, true, true, false)), Some(WheelEvent::Switch(WheelKind::Convert)));
        assert_eq!(t.step(s(false, 2, true, false, false)), Some(WheelEvent::Release));
    }

    /// A text drag; `translate` is the translate key (Command by default), the others are the file-wheel keys.
    fn text(left: bool, count: i64, translate: bool, tools: bool, actions: bool) -> Sample {
        Sample {
            left_down: left,
            drag_count: count,
            has_text: true,
            translate_key: translate,
            tools_key: tools,
            actions_key: actions,
            ..Default::default()
        }
    }

    #[test]
    fn text_drag_plus_convert_key_opens_translate() {
        let mut t = Tracker::default();
        assert_eq!(t.step(text(true, 5, false, false, false)), None); // press
        assert_eq!(t.step(text(true, 5, true, false, false)), None); // key without a drag
        assert_eq!(t.step(text(true, 5, false, false, false)), None);
        assert_eq!(t.step(text(true, 6, false, false, false)), None); // text drag started
        assert_eq!(t.step(text(true, 6, true, false, false)), Some(WheelEvent::Open(WheelKind::Translate)));
        assert_eq!(t.step(text(true, 6, true, false, false)), None, "held, not re-opened");
        assert_eq!(t.step(text(false, 6, false, false, false)), Some(WheelEvent::Release));
    }

    #[test]
    fn text_drag_ignores_the_convert_key_because_shift_extends_the_selection() {
        let mut t = Tracker::default();
        let shift = |count, on| Sample { left_down: true, drag_count: count, has_text: true, convert_key: on, ..Default::default() };
        t.step(shift(1, false));
        t.step(shift(2, false));
        assert_eq!(t.step(shift(2, true)), None, "Convert key during a text drag does nothing");
        assert_eq!(t.step(Sample { left_down: false, drag_count: 2, ..Default::default() }), None, "no wheel was open");
    }

    #[test]
    fn the_translate_key_does_not_open_a_file_wheel() {
        let mut t = Tracker::default();
        let f = |count, on| Sample { left_down: true, drag_count: count, has_files: true, translate_key: on, ..Default::default() };
        t.step(f(1, false));
        t.step(f(2, false));
        assert_eq!(t.step(f(2, true)), None, "the translate key means nothing for files");
    }

    #[test]
    fn text_drag_ignores_tools_and_actions_keys() {
        let mut t = Tracker::default();
        t.step(text(true, 1, false, false, false));
        t.step(text(true, 2, false, false, false));
        assert_eq!(t.step(text(true, 2, false, true, false)), None);
        t.step(text(true, 2, false, false, false));
        assert_eq!(t.step(text(true, 2, false, false, true)), None);
        assert_eq!(t.step(text(false, 2, false, false, false)), None, "no wheel was open");
    }

    #[test]
    fn open_translate_wheel_never_switches() {
        let mut t = Tracker::default();
        t.step(text(true, 1, false, false, false));
        t.step(text(true, 2, false, false, false));
        assert_eq!(t.step(text(true, 2, true, false, false)), Some(WheelEvent::Open(WheelKind::Translate)));
        t.step(text(true, 2, false, false, false));
        assert_eq!(t.step(text(true, 2, false, true, false)), None);
        t.step(text(true, 2, false, false, false));
        assert_eq!(t.step(text(true, 2, false, false, true)), None);
        t.step(text(true, 2, false, false, false));
        assert_eq!(t.step(text(true, 2, true, false, false)), None);
        assert_eq!(t.step(text(false, 2, false, false, false)), Some(WheelEvent::Release));
    }

    #[test]
    fn a_drag_with_files_and_text_is_a_file_drag() {
        let mut t = Tracker::default();
        let both = |count, ck| Sample {
            left_down: true,
            drag_count: count,
            has_files: true,
            has_text: true,
            convert_key: ck,
            ..Default::default()
        };
        t.step(both(1, false));
        t.step(both(2, false));
        assert_eq!(t.step(both(2, true)), Some(WheelEvent::Open(WheelKind::Convert)));
    }

    #[test]
    fn opens_only_on_a_key_tap_during_a_file_drag() {
        let mut t = Tracker::default();
        assert_eq!(t.step(s(true, 5, true, false, false)), None); // press, no drag yet
        assert_eq!(t.step(s(true, 5, true, true, false)), None); // shift without a drag
        assert_eq!(t.step(s(true, 5, true, false, false)), None);
        assert_eq!(t.step(s(true, 6, true, false, false)), None); // drag started
        assert_eq!(t.step(s(true, 6, true, true, false)), Some(WheelEvent::Open(WheelKind::Convert)));
        assert_eq!(t.step(s(true, 6, true, true, false)), None); // held, not re-opened
        assert_eq!(t.step(s(false, 6, true, false, false)), Some(WheelEvent::Release));
        assert_eq!(t.step(s(false, 6, true, false, false)), None);
    }

    #[test]
    fn ignores_text_drags_and_holds_from_before_the_press() {
        let mut t = Tracker::default();
        t.step(s(true, 1, false, false, false));
        assert_eq!(t.step(s(true, 2, false, true, false)), None, "text drag");
        t.step(s(false, 2, false, false, false));
        // key already held when the drag starts: needs a fresh tap
        t.step(s(true, 2, true, true, false));
        assert_eq!(t.step(s(true, 3, true, true, false)), None);
        assert_eq!(t.step(s(true, 3, true, false, true)), Some(WheelEvent::Open(WheelKind::Actions)));
        // release without a wheel produces nothing
        let mut u = Tracker::default();
        u.step(s(true, 1, true, false, false));
        assert_eq!(u.step(s(false, 2, true, false, false)), None);
    }

    #[test]
    fn arrow_steps_wrap() {
        assert_eq!(step_sel(None, 8, true), 0);
        assert_eq!(step_sel(Some(7), 8, true), 0);
        assert_eq!(step_sel(Some(0), 8, false), 7);
    }

    #[test]
    fn key_masks() {
        assert_eq!(key_mask("Shift"), 1 << 17);
        assert_eq!(key_mask("Command"), 1 << 20);
        assert_eq!(key_mask("Off"), 0);
    }

    #[test]
    fn segments() {
        // 4 segments: top, right, bottom, left
        assert_eq!(segment_at(0.0, -100.0, 40.0, 160.0, 4), Some(0));
        assert_eq!(segment_at(100.0, 0.0, 40.0, 160.0, 4), Some(1));
        assert_eq!(segment_at(0.0, 100.0, 40.0, 160.0, 4), Some(2));
        assert_eq!(segment_at(-100.0, 0.0, 40.0, 160.0, 4), Some(3));
        assert_eq!(segment_at(10.0, 10.0, 40.0, 160.0, 4), None, "hole");
        assert_eq!(segment_at(200.0, 0.0, 40.0, 160.0, 4), None, "outside");
        // boundary just left of 12 o'clock still belongs to segment 0
        assert_eq!(segment_at(-10.0, -100.0, 40.0, 160.0, 4), Some(0));
    }

    fn angle_of(p: [f32; 2]) -> f32 {
        p[0].atan2(-p[1]).rem_euclid(std::f32::consts::TAU)
    }

    #[test]
    fn every_segment_centre_hits_its_own_segment_for_one_to_twelve() {
        let g = Geometry::DEFAULT;
        for n in 1..=12 {
            for i in 0..n {
                let c = segment_center(i, n, (g.r_in + g.r_out) / 2.0);
                assert_eq!(segment_at(c[0], c[1], g.r_in, g.r_out, n), Some(i), "n={n} i={i}");
            }
        }
    }

    #[test]
    fn sector_points_stay_in_the_ring_and_leave_the_gap() {
        let g = Geometry::DEFAULT;
        let n = 8;
        let step = std::f32::consts::TAU / n as f32;
        let gap = g.gap_deg.to_radians();
        let pts = sector_points(2, n, &g, 16);
        assert!(pts.len() >= 4);
        for p in &pts {
            let r = (p[0] * p[0] + p[1] * p[1]).sqrt();
            assert!(r >= g.r_in - 0.01 && r <= g.r_out + 0.01, "radius {r}");
        }
        let angles: Vec<f32> = pts.iter().map(|p| angle_of(*p)).collect();
        let lo = angles.iter().cloned().fold(f32::MAX, f32::min);
        let hi = angles.iter().cloned().fold(f32::MIN, f32::max);
        assert!((lo - (2.0 * step - step / 2.0 + gap / 2.0)).abs() < 1e-3, "{lo}");
        assert!((hi - (2.0 * step + step / 2.0 - gap / 2.0)).abs() < 1e-3, "{hi}");
    }

    #[test]
    fn sector_points_degenerate_inputs_do_not_panic() {
        let g = Geometry::DEFAULT;
        assert!(sector_points(0, 0, &g, 16).is_empty());
        assert!(sector_points(5, 3, &g, 16).is_empty());
        assert!(!sector_points(0, 1, &g, 0).is_empty());
    }

    #[test]
    fn every_segment_is_labelled_and_long_names_are_cut_to_the_arc() {
        // up to eight segments: unchanged, 11 pt, 17 characters
        assert_eq!(ring_label("Compress", 7, 123.0), Some(("Compress".to_string(), 11.0)));
        assert_eq!(ring_label("PowerPoint (PPTX)", 8, 123.0), Some(("PowerPoint (PPTX)".to_string(), 11.0)));
        assert_eq!(ring_label("Remove all the metadata", 8, 123.0), Some(("Remove all the m\u{2026}".to_string(), 11.0)));
        // nine or more: still labelled (the old rule hid every label), smaller and cut to the arc
        assert_eq!(ring_label("Compress", 9, 123.0), Some(("Compress".to_string(), 10.0)));
        assert_eq!(ring_label("Snapshots", 9, 123.0), Some(("Snapshots".to_string(), 10.0)));
        let (long, size) = ring_label("PowerPoint (PPTX)", 9, 123.0).unwrap();
        assert_eq!(size, 10.0);
        assert!(long.ends_with('\u{2026}') && long.chars().count() <= 14, "{long:?}");
        let (tight, _) = ring_label("Remove metadata", 12, 123.0).unwrap();
        assert!(tight.ends_with('\u{2026}') && tight.chars().count() <= 10, "{tight:?}");
        // a short name is never cut, even on a crowded ring
        assert_eq!(ring_label("Mute", 12, 123.0), Some(("Mute".to_string(), 10.0)));
        // no segments, no label
        assert_eq!(ring_label("Mute", 0, 123.0), None);
        assert!((arc_len(8, 100.0) - 78.539_82).abs() < 1e-3);
    }

    #[test]
    fn easing_is_monotonic_and_pinned() {
        assert_eq!(ease_out_cubic(0.0), 0.0);
        assert_eq!(ease_out_cubic(1.0), 1.0);
        assert_eq!(ease_out_cubic(-3.0), 0.0);
        assert_eq!(ease_out_cubic(7.0), 1.0);
        let mut prev = 0.0;
        for k in 1..=20 {
            let v = ease_out_cubic(k as f32 / 20.0);
            assert!(v >= prev);
            prev = v;
        }
    }

    #[test]
    fn anim_progress_is_eased_clamped_and_instant_for_zero_duration() {
        assert_eq!(anim_progress(0.0, 120.0), 0.0);
        assert_eq!(anim_progress(120.0, 120.0), 1.0);
        assert_eq!(anim_progress(500.0, 120.0), 1.0);
        assert_eq!(anim_progress(-5.0, 120.0), 0.0);
        assert!((anim_progress(60.0, 120.0) - 0.875).abs() < 1e-6, "ease_out_cubic(0.5) = 0.875");
        assert_eq!(anim_progress(0.0, 0.0), 1.0, "zero duration is already finished");
        assert_eq!(anim_progress(0.0, -1.0), 1.0);
    }

    #[test]
    fn lerp_and_crossfade_and_stagger() {
        assert_eq!(lerp(2.0, 4.0, 0.0), 2.0);
        assert_eq!(lerp(2.0, 4.0, 1.0), 4.0);
        assert_eq!(lerp(2.0, 4.0, 0.5), 3.0);
        assert_eq!(crossfade_alpha(0.0), (1.0, 0.0));
        assert_eq!(crossfade_alpha(1.0), (0.0, 1.0));
        assert_eq!(crossfade_alpha(0.25), (0.75, 0.25));
        assert_eq!(crossfade_alpha(9.0), (0.0, 1.0));
        assert_eq!(crossfade_alpha(-9.0), (1.0, 0.0));
        assert_eq!(stagger_ms(0), 0.0);
        assert_eq!(stagger_ms(3), 36.0);
        assert_eq!(stagger_ms(40), 96.0);
    }

    #[test]
    fn hub_text_summarises_the_selection() {
        use std::path::PathBuf;
        let p = |s: &str| PathBuf::from(s);
        assert_eq!(hub_text(&[p("/nonexistent/a.png")]), ("1 file".to_string(), "PNG image".to_string()));
        assert_eq!(hub_text(&[p("/nonexistent/a.mp4")]), ("1 file".to_string(), "MP4 video".to_string()));
        let many: Vec<PathBuf> = (0..12).map(|i| p(&format!("/nonexistent/{i}.png"))).collect();
        assert_eq!(hub_text(&many), ("12 files".to_string(), "images".to_string()));
        assert_eq!(hub_text(&[p("/nonexistent/a.png"), p("/nonexistent/b.mp3")]), ("2 files".to_string(), "Mixed".to_string()));
        assert_eq!(hub_text(&[]), ("0 files".to_string(), String::new()));
        let dir = std::env::temp_dir();
        assert_eq!(hub_text(&[dir]), ("1 file".to_string(), "Folder".to_string()));
    }
}
