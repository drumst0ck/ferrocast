//! Planning for the Compress window (target size, percentage or preset, optional resize): bitrate
//! arithmetic and size estimates. Pure, so every rule is testable without media or a window.

/// Lowest audio bitrate the planner will offer, in kbit/s.
pub const MIN_AUDIO_KBPS: u32 = 32;
/// Highest audio bitrate the planner will offer, in kbit/s.
pub const MAX_AUDIO_KBPS: u32 = 320;
/// Smallest video bitrate worth encoding, in kbit/s.
pub const MIN_VIDEO_KBPS: u32 = 100;

/// What the user asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Goal {
    /// Reach about this many bytes.
    TargetBytes(u64),
    /// Make the file this percent smaller (clamped to 1..=95).
    Percent(u8),
    Preset(Preset),
}

/// One-step strengths, from the lightest to the strongest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    Light,
    Balanced,
    Strong,
}

impl Preset {
    /// Rough share of the original size the result keeps.
    fn keep_percent(self) -> u64 {
        match self {
            Preset::Light => 70,
            Preset::Balanced => 50,
            Preset::Strong => 30,
        }
    }
}

/// Optional downscale applied while compressing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Resize {
    /// Longest edge, in pixels.
    pub max_edge: Option<u32>,
    /// Scale, in percent of the original (1..=100).
    pub percent: Option<u8>,
}

pub fn clamp_percent(p: u8) -> u8 {
    p.clamp(1, 95)
}

fn total_kbps(target_bytes: u64, duration_s: f64) -> Option<f64> {
    (target_bytes > 0 && duration_s.is_finite() && duration_s > 0.0).then(|| target_bytes as f64 * 8.0 / 1000.0 / duration_s)
}

/// Audio bitrate that fits `target_bytes` over `duration_s`, clamped to the supported range.
/// `None` when the target is impossible (below the minimum) or the numbers are unusable.
pub fn audio_kbps(target_bytes: u64, duration_s: f64) -> Option<u32> {
    let kbps = total_kbps(target_bytes, duration_s)?.floor();
    if kbps < MIN_AUDIO_KBPS as f64 {
        return None;
    }
    Some(kbps.min(MAX_AUDIO_KBPS as f64) as u32)
}

/// Video bitrate left for `target_bytes` after `audio_kbps` is taken off. `None` when too little
/// remains to be worth encoding.
pub fn video_kbps(target_bytes: u64, duration_s: f64, audio_kbps: u32) -> Option<u32> {
    let left = total_kbps(target_bytes, duration_s)?.floor() - audio_kbps as f64;
    (left >= MIN_VIDEO_KBPS as f64).then_some(left as u32)
}

/// Expected size after compressing a file of `orig` bytes. Never larger than the original.
pub fn estimate_bytes(orig: u64, goal: Goal) -> u64 {
    let est = match goal {
        Goal::TargetBytes(t) => t,
        Goal::Percent(p) => orig / 100 * (100 - clamp_percent(p) as u64) + orig % 100 * (100 - clamp_percent(p) as u64) / 100,
        Goal::Preset(pr) => orig / 100 * pr.keep_percent() + orig % 100 * pr.keep_percent() / 100,
    };
    est.min(orig)
}

/// Size after the optional resize: aspect kept, never upscaled, width and height even (video
/// encoders need that) and at least 2. `None` for a degenerate source or a zero limit.
pub fn resized_dims(w: u32, h: u32, r: &Resize) -> Option<(u32, u32)> {
    if w < 2 || h < 2 || r.max_edge == Some(0) {
        return None;
    }
    let mut scale: f64 = 1.0;
    if let Some(e) = r.max_edge {
        scale = scale.min(e as f64 / w.max(h) as f64);
    }
    if let Some(p) = r.percent {
        scale = scale.min(p.clamp(1, 100) as f64 / 100.0);
    }
    let even = |v: f64| ((v.floor() as u32) & !1).max(2);
    Some((even(w as f64 * scale), even(h as f64 * scale)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_bitrate_comes_from_size_over_duration() {
        // 1 MB over 60 s is about 133 kbit/s in total
        assert_eq!(audio_kbps(1_000_000, 60.0), Some(133));
        assert_eq!(audio_kbps(1_000_000, 1.0), Some(MAX_AUDIO_KBPS), "a roomy target is clamped to the maximum");
    }

    #[test]
    fn impossible_audio_targets_are_refused() {
        assert_eq!(audio_kbps(100_000, 60.0), None, "13 kbit/s is below the minimum");
        for (t, d) in [(0, 60.0), (1_000_000, 0.0), (1_000_000, -3.0), (1_000_000, f64::NAN), (1_000_000, f64::INFINITY)] {
            assert_eq!(audio_kbps(t, d), None, "{t} {d}");
        }
    }

    #[test]
    fn video_bitrate_is_what_remains_after_the_audio() {
        assert_eq!(video_kbps(10_000_000, 60.0, 128), Some(1205));
        assert_eq!(video_kbps(1_000_000, 60.0, 128), None, "5 kbit/s left is not a usable video");
        assert_eq!(video_kbps(10_000_000, 0.0, 128), None);
        assert_eq!(video_kbps(0, 60.0, 128), None);
        assert_eq!(video_kbps(10_000_000, 60.0, 0), Some(1333), "a silent video keeps the whole budget");
    }

    #[test]
    fn percent_is_clamped_and_estimated() {
        assert_eq!(clamp_percent(0), 1);
        assert_eq!(clamp_percent(200), 95);
        assert_eq!(clamp_percent(40), 40);
        assert_eq!(estimate_bytes(1000, Goal::Percent(40)), 600);
        assert_eq!(estimate_bytes(1000, Goal::Percent(0)), 990, "0 % is raised to 1 %");
        assert_eq!(estimate_bytes(1000, Goal::Percent(200)), 50, "200 % is lowered to 95 %");
    }

    #[test]
    fn presets_get_smaller_and_never_grow_the_file() {
        let (a, b, c) = (
            estimate_bytes(10_000, Goal::Preset(Preset::Light)),
            estimate_bytes(10_000, Goal::Preset(Preset::Balanced)),
            estimate_bytes(10_000, Goal::Preset(Preset::Strong)),
        );
        assert!(a > b && b > c && c > 0 && a < 10_000, "{a} {b} {c}");
    }

    #[test]
    fn a_size_target_is_the_estimate_unless_the_file_is_already_smaller() {
        assert_eq!(estimate_bytes(1000, Goal::TargetBytes(500)), 500);
        assert_eq!(estimate_bytes(100, Goal::TargetBytes(500)), 100, "compressing never grows a file");
        assert_eq!(estimate_bytes(0, Goal::Percent(50)), 0);
        assert_eq!(estimate_bytes(0, Goal::TargetBytes(5)), 0);
    }

    #[test]
    fn resize_keeps_the_aspect_never_upscales_and_stays_even() {
        let r = |e, p| Resize { max_edge: e, percent: p };
        assert_eq!(resized_dims(4000, 3000, &r(Some(2000), None)), Some((2000, 1500)));
        assert_eq!(resized_dims(100, 100, &r(Some(400), None)), Some((100, 100)), "no upscaling");
        assert_eq!(resized_dims(1000, 600, &r(None, Some(50))), Some((500, 300)));
        assert_eq!(resized_dims(1000, 600, &r(None, None)), Some((1000, 600)), "no resize requested");
        let (w, h) = resized_dims(101, 51, &r(None, Some(50))).unwrap();
        assert!(w % 2 == 0 && h % 2 == 0 && w >= 2 && h >= 2, "{w}x{h}");
        // both limits: the stricter one wins
        assert_eq!(resized_dims(4000, 3000, &r(Some(2000), Some(25))), Some((1000, 750)));
        assert_eq!(resized_dims(1, 100, &r(None, None)), None, "a degenerate picture has no size");
        assert_eq!(resized_dims(0, 0, &r(Some(10), None)), None);
        assert_eq!(resized_dims(4000, 3000, &r(Some(0), None)), None, "a zero edge is refused");
    }
}
