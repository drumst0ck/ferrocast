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

/// What probing a media file tells the planner.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MediaInfo {
    pub duration: f64,
    pub has_audio: bool,
    /// Displayed size (rotation applied) of the first video stream.
    pub dims: Option<(u32, u32)>,
}

/// How a media file will be re-encoded.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Plan {
    Audio { kbps: u32 },
    Video { rate: crate::media::VideoRate, audio_kbps: Option<u32>, scale: Option<(u32, u32)> },
}

fn preset_audio_kbps(p: Preset) -> u32 {
    match p {
        Preset::Light => 128,
        Preset::Balanced => 96,
        Preset::Strong => 64,
    }
}

/// Plans an audio (`video == false`, resize ignored) or video compression. Size and percent goals
/// become a bitrate; a target that leaves too little for the stream is refused with a clear text.
pub fn plan(video: bool, orig_bytes: u64, info: &MediaInfo, goal: Goal, resize: &Resize) -> Result<Plan, String> {
    use crate::media::VideoRate;
    if !info.duration.is_finite() || info.duration <= 0.0 {
        return Err("could not read the media duration".into());
    }
    let target = match goal {
        Goal::TargetBytes(t) => Some(t),
        Goal::Percent(_) => Some(estimate_bytes(orig_bytes, goal)),
        Goal::Preset(_) => None,
    };
    let too_small = |what: &str| format!("target too small for {:.0} s of {what}", info.duration);
    if !video {
        let kbps = match (target, goal) {
            (Some(t), _) => audio_kbps(t, info.duration).ok_or_else(|| too_small("audio"))?,
            (None, Goal::Preset(p)) => preset_audio_kbps(p),
            _ => unreachable!("a goal without a target is a preset"),
        };
        return Ok(Plan::Audio { kbps });
    }
    let (w, h) = info.dims.ok_or("could not read the video size")?;
    let (rw, rh) = resized_dims(w, h, resize).ok_or("the video size cannot be resized")?;
    let scale = ((rw, rh) != (w, h)).then_some((rw, rh));
    let (rate, audio) = match (target, goal) {
        (Some(t), _) if info.has_audio => {
            let a = [128, 96, 64, MIN_AUDIO_KBPS].into_iter().find(|&a| video_kbps(t, info.duration, a).is_some());
            let a = a.ok_or_else(|| too_small("video"))?;
            (VideoRate::Kbps(video_kbps(t, info.duration, a).unwrap_or(MIN_VIDEO_KBPS)), Some(a))
        }
        (Some(t), _) => (VideoRate::Kbps(video_kbps(t, info.duration, 0).ok_or_else(|| too_small("video"))?), None),
        (None, Goal::Preset(p)) => {
            let crf = match p {
                Preset::Light => 23,
                Preset::Balanced => 28,
                Preset::Strong => 32,
            };
            (VideoRate::Crf(crf), info.has_audio.then(|| preset_audio_kbps(p)))
        }
        _ => unreachable!("a goal without a target is a preset"),
    };
    Ok(Plan::Video { rate, audio_kbps: audio, scale })
}

/// Extension of the compressed copy: audio keeps mp3, m4a, ogg and opus and becomes m4a otherwise;
/// video is always mp4.
pub fn output_ext(video: bool, source_ext: &str) -> &'static str {
    if video {
        return "mp4";
    }
    match source_ext.to_ascii_lowercase().as_str() {
        "mp3" => "mp3",
        "ogg" => "ogg",
        "opus" => "opus",
        _ => "m4a",
    }
}

/// One line for the window: the bitrates a plan will use.
pub fn describe(plan: &Plan) -> String {
    use crate::media::VideoRate;
    match plan {
        Plan::Audio { kbps } => format!("≈ {kbps} kbit/s audio"),
        Plan::Video { rate, audio_kbps, scale } => {
            let mut l = match rate {
                VideoRate::Kbps(k) if *k >= 1000 => format!("≈ {:.1} Mbit/s video", *k as f64 / 1000.0),
                VideoRate::Kbps(k) => format!("≈ {k} kbit/s video"),
                VideoRate::Crf(c) => format!("quality-based (CRF {c})"),
            };
            if let Some(a) = audio_kbps {
                l.push_str(&format!(" + {a} kbit/s audio"));
            }
            if let Some((w, h)) = scale {
                l.push_str(&format!(" · {w}×{h} px"));
            }
            l
        }
    }
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

#[cfg(test)]
mod media_plan_tests {
    use super::*;
    use crate::media::VideoRate;

    fn info(duration: f64, has_audio: bool, dims: Option<(u32, u32)>) -> MediaInfo {
        MediaInfo { duration, has_audio, dims }
    }
    const NONE: Resize = Resize { max_edge: None, percent: None };

    #[test]
    fn audio_target_uses_the_bitrate_and_refuses_the_impossible() {
        let i = info(20.0, false, None);
        assert_eq!(plan(false, 5_000_000, &i, Goal::TargetBytes(320_000), &NONE), Ok(Plan::Audio { kbps: 128 }));
        let err = plan(false, 5_000_000, &i, Goal::TargetBytes(20_000), &NONE).unwrap_err();
        assert_eq!(err, "target too small for 20 s of audio");
        // percent goes through the estimated size
        assert_eq!(plan(false, 640_000, &i, Goal::Percent(50), &NONE), Ok(Plan::Audio { kbps: 128 }));
    }

    #[test]
    fn audio_presets_are_128_96_64() {
        let i = info(20.0, false, None);
        let k = |p| match plan(false, 1_000_000, &i, Goal::Preset(p), &NONE) {
            Ok(Plan::Audio { kbps }) => kbps,
            other => panic!("{other:?}"),
        };
        assert_eq!((k(Preset::Light), k(Preset::Balanced), k(Preset::Strong)), (128, 96, 64));
    }

    #[test]
    fn video_target_splits_audio_and_video_and_ignores_resize_for_audio() {
        // 1.5 MB over 20 s = 600 kbit/s total: 128 audio leaves 472 video
        let i = info(20.0, true, Some((1280, 720)));
        match plan(true, 9_000_000, &i, Goal::TargetBytes(1_500_000), &NONE).unwrap() {
            Plan::Video { rate: VideoRate::Kbps(v), audio_kbps: Some(a), scale: None } => assert_eq!((a, v), (128, 472)),
            other => panic!("{other:?}"),
        }
        // silent: everything goes to video
        let silent = info(20.0, false, Some((1280, 720)));
        match plan(true, 9_000_000, &silent, Goal::TargetBytes(1_500_000), &NONE).unwrap() {
            Plan::Video { rate: VideoRate::Kbps(v), audio_kbps: None, .. } => assert_eq!(v, 600),
            other => panic!("{other:?}"),
        }
        // audio files ignore a resize
        let r = Resize { max_edge: Some(320), percent: None };
        assert_eq!(plan(false, 1_000_000, &info(20.0, false, None), Goal::Preset(Preset::Light), &r), Ok(Plan::Audio { kbps: 128 }));
    }

    #[test]
    fn video_audio_shrinks_before_the_target_is_refused() {
        // 300 kbit/s total: 128 audio would leave 172 video, fine; 220 total: 96 audio leaves 124
        let i = info(10.0, true, Some((640, 360)));
        match plan(true, 9_000_000, &i, Goal::TargetBytes(275_000), &NONE).unwrap() {
            Plan::Video { audio_kbps: Some(a), rate: VideoRate::Kbps(v), .. } => assert!(a < 128 && v >= MIN_VIDEO_KBPS, "{a} {v}"),
            other => panic!("{other:?}"),
        }
        let err = plan(true, 9_000_000, &i, Goal::TargetBytes(50_000), &NONE).unwrap_err();
        assert_eq!(err, "target too small for 10 s of video");
    }

    #[test]
    fn video_presets_use_crf_and_the_resize_scale() {
        let i = info(20.0, true, Some((1280, 720)));
        let crf = |p| match plan(true, 9_000_000, &i, Goal::Preset(p), &NONE).unwrap() {
            Plan::Video { rate: VideoRate::Crf(c), audio_kbps: Some(a), .. } => (c, a),
            other => panic!("{other:?}"),
        };
        assert_eq!((crf(Preset::Light), crf(Preset::Balanced), crf(Preset::Strong)), ((23, 128), (28, 96), (32, 64)));
        let r = Resize { max_edge: Some(320), percent: None };
        match plan(true, 9_000_000, &i, Goal::Preset(Preset::Light), &r).unwrap() {
            Plan::Video { scale, .. } => assert_eq!(scale, Some((320, 180))),
            other => panic!("{other:?}"),
        }
        // odd source sizes are made even even without a resize
        let odd = info(20.0, true, Some((641, 361)));
        match plan(true, 9_000_000, &odd, Goal::Preset(Preset::Light), &NONE).unwrap() {
            Plan::Video { scale, .. } => assert_eq!(scale, Some((640, 360))),
            other => panic!("{other:?}"),
        }
        assert!(plan(true, 9_000_000, &info(20.0, true, None), Goal::Preset(Preset::Light), &NONE).is_err());
        assert!(plan(true, 9_000_000, &info(0.0, true, Some((640, 360))), Goal::Preset(Preset::Light), &NONE).is_err());
    }

    #[test]
    fn output_extension_rules() {
        for e in ["mp3", "m4a", "ogg", "opus"] {
            assert_eq!(output_ext(false, e), e);
        }
        assert_eq!(output_ext(false, "MP3"), "mp3");
        for e in ["wav", "flac", "aiff", "wma", "aac", ""] {
            assert_eq!(output_ext(false, e), "m4a", "{e}");
        }
        for e in ["mp4", "mov", "mkv", "webm", "avi", "wmv"] {
            assert_eq!(output_ext(true, e), "mp4");
        }
    }

    #[test]
    fn describe_shows_the_bitrates() {
        assert_eq!(describe(&Plan::Audio { kbps: 128 }), "≈ 128 kbit/s audio");
        let v = |rate, a, scale| Plan::Video { rate, audio_kbps: a, scale };
        assert_eq!(describe(&v(VideoRate::Kbps(1200), Some(128), None)), "≈ 1.2 Mbit/s video + 128 kbit/s audio");
        assert_eq!(describe(&v(VideoRate::Kbps(472), None, None)), "≈ 472 kbit/s video");
        assert_eq!(describe(&v(VideoRate::Crf(28), Some(96), Some((640, 360)))), "quality-based (CRF 28) + 96 kbit/s audio · 640×360 px");
    }
}
