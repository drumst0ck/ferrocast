//! Explicit real-file checks through the public file-tool executor.
use crate::services::run_file_tool;
use ferrocast_core::convert::{FileTool, Tools};
use ferrocast_core::media::Canvas;
use std::path::Path;
use std::process::Command;

fn command(ff: &str, args: &[&str], output: &Path) {
    crate::route::run_limited(Command::new(ff).args(["-nostdin", "-n", "-loglevel", "error"]).args(args).arg(output), 30).unwrap();
}

#[test]
#[ignore = "requires ffmpeg/ffprobe; creates isolated real media"]
fn media_tools_real_files() {
    let tools = Tools::detect();
    let ff = tools.ffmpeg.expect("ffmpeg is required for this explicit check");
    let root = std::env::temp_dir().join(format!("fc-media-check-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let video = root.join("source.mp4");
    let audio = root.join("source.wav");
    command(
        ff,
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=160x120:rate=10",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=2",
            "-t",
            "2",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
        ],
        &video,
    );
    command(ff, &["-f", "lavfi", "-i", "sine=frequency=440:duration=2"], &audio);
    let cover = root.join("cover art.png");
    command(ff, &["-f", "lavfi", "-i", "testsrc=size=200x200:rate=1", "-frames:v", "1"], &cover);
    let original_video = std::fs::read(&video).unwrap();
    let original_audio = std::fs::read(&audio).unwrap();
    let cases = [
        (&video, FileTool::Mute),
        (&video, FileTool::TrimExact(0.5, 1.5)),
        (&video, FileTool::Speed(2.0)),
        (&video, FileTool::Snapshot(0.5)),
        (&video, FileTool::CropVideo(0, 0, 80, 60)),
        (&video, FileTool::RedactVideo(0, 0, 64, 64, 0.25, 1.0, false)),
        (&video, FileTool::RedactVideo(0, 0, 64, 64, 0.25, 1.0, true)),
        (&audio, FileTool::Normalize(-16.0, 11.0, -1.5)),
        (&audio, FileTool::Visualize(Canvas::Landscape)),
        (&audio, FileTool::TrimSilence),
        (&audio, FileTool::VisualizeImage(Canvas::Square, cover.to_string_lossy().to_string())),
        (&audio, FileTool::Channels(false, 1.0, 1.0)),
        (&audio, FileTool::Channels(true, 1.0, 1.0)),
        (&audio, FileTool::Bleep(0.25, 1.0)),
    ];
    for (input, tool) in cases {
        let (made, errors) = run_file_tool(std::slice::from_ref(input), &tool, Some(ff));
        assert!(errors.is_empty(), "{tool:?}: {errors:?}");
        assert_eq!(made.len(), 1, "{tool:?}");
        assert!(std::fs::metadata(&made[0]).unwrap().len() > 0, "{tool:?}");
        if !matches!(tool, FileTool::Snapshot(_)) {
            crate::route::run_limited(Command::new(ff).args(["-nostdin", "-v", "error", "-i"]).arg(&made[0]).args(["-f", "null", "-"]), 30)
                .unwrap();
        }
    }
    // normalize report: the measured loudness moves to the target
    let (norm, e) = run_file_tool(std::slice::from_ref(&audio), &FileTool::Normalize(-16.0, 11.0, -1.5), Some(ff));
    assert!(e.is_empty(), "{e:?}");
    let before = crate::media_tools::loudness(ff, &audio).expect("loudness of the source");
    let after = crate::media_tools::loudness(ff, &norm[0]).expect("loudness of the result");
    assert!((after - -16.0).abs() < 2.0, "normalized to about -16 LUFS, got {after} (was {before})");
    let rep = crate::media_tools::normalize_report(ff, &audio, &norm[0]).expect("report");
    assert!(rep.contains("LUFS") && rep.contains('→'), "{rep}");
    // exact trim: the written clip lasts the requested second (re-encoded, so not keyframe-bound)
    let (tx, e) = run_file_tool(std::slice::from_ref(&video), &FileTool::TrimExact(0.55, 1.45), Some(ff));
    assert!(e.is_empty(), "{e:?}");
    let probe = Path::new(ff).with_file_name("ffprobe");
    let out = Command::new(probe)
        .args(["-v", "error", "-show_entries", "format=duration", "-of", "default=nw=1:nk=1"])
        .arg(&tx[0])
        .output()
        .unwrap();
    let dur: f64 = String::from_utf8_lossy(&out.stdout).trim().parse().unwrap();
    assert!((dur - 0.9).abs() < 0.15, "exact trim lasts about 0.9 s, got {dur}");
    let (joined, errors) = run_file_tool(&[video.clone(), video.clone()], &FileTool::Join, Some(ff));
    assert!(errors.is_empty(), "join: {errors:?}");
    assert_eq!(joined.len(), 1);
    // A repeated tool invocation must leave the previous result unchanged.
    let (a, e) = run_file_tool(std::slice::from_ref(&video), &FileTool::Mute, Some(ff));
    assert!(e.is_empty());
    let saved = std::fs::read(&a[0]).unwrap();
    let (b, e) = run_file_tool(std::slice::from_ref(&video), &FileTool::Mute, Some(ff));
    assert!(e.is_empty());
    assert_ne!(a, b);
    assert_eq!(std::fs::read(&a[0]).unwrap(), saved);
    let missing = root.join("nope.png").to_string_lossy().to_string();
    let before = std::fs::read_dir(&root).unwrap().count();
    let (made, errors) = run_file_tool(std::slice::from_ref(&audio), &FileTool::VisualizeImage(Canvas::Square, missing), Some(ff));
    assert!(made.is_empty() && errors.len() == 1 && errors[0].contains("not a readable image"), "{errors:?}");
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), before, "a rejected image leaves nothing behind");
    let count = std::fs::read_dir(&root).unwrap().count();
    let (made, errors) = run_file_tool(std::slice::from_ref(&video), &FileTool::CropVideo(0, 0, 9999, 1), Some(ff));
    assert!(made.is_empty());
    assert_eq!(errors.len(), 1);
    assert!(errors[0].ends_with("invalid parameters for this media tool"));
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), count);
    assert_eq!(std::fs::read(&video).unwrap(), original_video);
    assert_eq!(std::fs::read(&audio).unwrap(), original_audio);
    std::fs::remove_dir_all(&root).unwrap();
}

fn duration_of(ff: &str, file: &Path) -> f64 {
    let out = Command::new(Path::new(ff).with_file_name("ffprobe"))
        .args(["-v", "error", "-show_entries", "format=duration", "-of", "default=nw=1:nk=1"])
        .arg(file)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().parse().unwrap()
}

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    v
}

#[test]
#[ignore = "requires ffmpeg/ffprobe; creates isolated real media"]
fn split_points_real_video() {
    let tools = Tools::detect();
    let ff = tools.ffmpeg.expect("ffmpeg is required for this explicit check");
    let root = std::env::temp_dir().join(format!("fc-split-video-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let video = root.join("movie.mp4");
    command(
        ff,
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=160x120:rate=10",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=10",
            "-t",
            "10",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
        ],
        &video,
    );
    let original = std::fs::read(&video).unwrap();
    let tool = FileTool::SplitPoints(vec![(0.0, 3.0), (3.0, 7.0), (7.0, 10.0)], true);
    let (made, errors) = run_file_tool(std::slice::from_ref(&video), &tool, Some(ff));
    assert!(errors.is_empty(), "{errors:?}");
    let folder = root.join("movie split");
    assert_eq!(names(&folder), ["movie 1.mp4", "movie 2.mp4", "movie 3.mp4"]);
    assert_eq!(made, vec![folder.clone()]);
    for (i, want) in [3.0, 4.0, 3.0].iter().enumerate() {
        let d = duration_of(ff, &folder.join(format!("movie {}.mp4", i + 1)));
        assert!((d - want).abs() < 0.2, "clip {} lasts {d}, wanted {want}", i + 1);
    }
    let first_run = names(&folder);
    let (made, errors) = run_file_tool(std::slice::from_ref(&video), &tool, Some(ff));
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(made, [root.join("movie split 2")]);
    assert_eq!(names(&root.join("movie split 2")).len(), 3);
    assert_eq!(names(&folder), first_run, "the first folder is untouched");
    // an invalid range creates nothing
    let before = names(&root);
    let bad = FileTool::SplitPoints(vec![(0.0, 3.0), (3.0, 99.0)], true);
    let (made, errors) = run_file_tool(std::slice::from_ref(&video), &bad, Some(ff));
    assert!(made.is_empty() && errors.len() == 1, "{errors:?}");
    assert_eq!(names(&root), before, "no folder left behind");
    assert_eq!(std::fs::read(&video).unwrap(), original);
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
#[ignore = "requires ffmpeg/ffprobe; creates isolated real media"]
fn split_points_real_audio() {
    let tools = Tools::detect();
    let ff = tools.ffmpeg.expect("ffmpeg is required for this explicit check");
    let root = std::env::temp_dir().join(format!("fc-split-audio-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let audio = root.join("song.mp3");
    command(ff, &["-f", "lavfi", "-i", "sine=frequency=440:duration=10", "-c:a", "libmp3lame"], &audio);
    let original = std::fs::read(&audio).unwrap();
    let (made, errors) = run_file_tool(
        std::slice::from_ref(&audio),
        &FileTool::SplitPoints(ferrocast_core::videoplan::equal_parts(10.0, 2), false),
        Some(ff),
    );
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(made, [root.join("song split")]);
    assert_eq!(names(&root.join("song split")), ["song 1.mp3", "song 2.mp3"]);
    for i in 1..=2 {
        let d = duration_of(ff, &root.join("song split").join(format!("song {i}.mp3")));
        assert!((d - 5.0).abs() < 0.5, "part {i} lasts {d}");
    }
    assert_eq!(std::fs::read(&audio).unwrap(), original);
    std::fs::remove_dir_all(&root).unwrap();
}

// ---- A6c: audio and video compress

mod compress_real {
    use super::{command, duration_of, names};
    use crate::services::run_file_tool;
    use ferrocast_core::compress::{Goal, Preset, Resize};
    use ferrocast_core::convert::{FileTool, Tools};
    use std::path::{Path, PathBuf};
    use std::process::Command;

    const NONE: Resize = Resize { max_edge: None, percent: None };

    fn size(p: &Path) -> u64 {
        std::fs::metadata(p).unwrap().len()
    }

    fn video_width(ff: &str, file: &Path) -> u32 {
        let out = Command::new(Path::new(ff).with_file_name("ffprobe"))
            .args(["-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width", "-of", "default=nw=1:nk=1"])
            .arg(file)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().parse().unwrap()
    }

    fn has_audio_stream(ff: &str, file: &Path) -> bool {
        let out = Command::new(Path::new(ff).with_file_name("ffprobe"))
            .args(["-v", "error", "-select_streams", "a", "-show_entries", "stream=codec_type", "-of", "default=nw=1:nk=1"])
            .arg(file)
            .output()
            .unwrap();
        !out.stdout.is_empty()
    }

    fn scratch(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("fc-compress-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir(&root).unwrap();
        root
    }

    /// 20 s 1280x720 H.264 with a sine track (or none).
    fn make_video(ff: &str, path: &Path, audio: bool) {
        let mut args = vec!["-f", "lavfi", "-i", "testsrc2=size=1280x720:rate=25:duration=20"];
        if audio {
            args.extend(["-f", "lavfi", "-i", "sine=frequency=440:duration=20", "-c:a", "aac", "-b:a", "192k"]);
        }
        args.extend(["-t", "20", "-c:v", "libx264", "-crf", "18", "-pix_fmt", "yuv420p"]);
        command(ff, &args, path);
    }

    fn compress(ff: &str, file: &Path, goal: Goal, resize: Resize) -> (Vec<PathBuf>, Vec<String>) {
        run_file_tool(std::slice::from_ref(&file.to_path_buf()), &FileTool::Compress(goal, resize), Some(ff))
    }

    #[test]
    #[ignore = "requires ffmpeg/ffprobe; creates isolated real media"]
    fn compress_video_target_percent_and_presets() {
        let ff = Tools::detect().ffmpeg.expect("ffmpeg is required for this explicit check");
        let root = scratch("video");
        let src = root.join("clip.mp4");
        make_video(ff, &src, true);
        let original = std::fs::read(&src).unwrap();
        let target = 1_500_000u64;
        assert!(original.len() as u64 > target * 2, "fixture too small: {}", original.len());
        let (made, errors) = compress(ff, &src, Goal::TargetBytes(target), NONE);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(made, [root.join("clip compressed.mp4")]);
        let got = size(&made[0]);
        assert!(got < original.len() as u64, "smaller than the original");
        let ratio = got as f64 / target as f64;
        eprintln!("target {target} got {got} ratio {ratio:.3} original {}", original.len());
        assert!((0.65..=1.35).contains(&ratio), "size {got} is {ratio:.2} of the target {target}");
        let d = duration_of(ff, &made[0]);
        assert!((d - 20.0).abs() < 0.3, "duration {d}");
        assert!(has_audio_stream(ff, &made[0]));
        // the second run never overwrites: " 2"
        let (made2, errors) = compress(ff, &src, Goal::TargetBytes(target), NONE);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(made2, [root.join("clip compressed 2.mp4")]);
        // percent and presets
        let (p, errors) = compress(ff, &src, Goal::Percent(50), NONE);
        assert!(errors.is_empty(), "{errors:?}");
        assert!(size(&p[0]) < original.len() as u64, "Percent(50) is smaller than the original");
        let (light, e1) = compress(ff, &src, Goal::Preset(Preset::Light), NONE);
        let (strong, e2) = compress(ff, &src, Goal::Preset(Preset::Strong), NONE);
        assert!(e1.is_empty() && e2.is_empty(), "{e1:?} {e2:?}");
        assert!(size(&strong[0]) < size(&light[0]), "strong {} < light {}", size(&strong[0]), size(&light[0]));
        assert_eq!(std::fs::read(&src).unwrap(), original, "the original is untouched");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    #[ignore = "requires ffmpeg/ffprobe; creates isolated real media"]
    fn compress_video_resize_and_silent() {
        let ff = Tools::detect().ffmpeg.expect("ffmpeg is required for this explicit check");
        let root = scratch("resize");
        let src = root.join("clip.mp4");
        make_video(ff, &src, true);
        let (made, errors) = compress(ff, &src, Goal::Preset(Preset::Balanced), Resize { max_edge: Some(320), percent: None });
        assert!(errors.is_empty(), "{errors:?}");
        let w = video_width(ff, &made[0]);
        assert_eq!(w, 320);
        assert_eq!(w % 2, 0);
        // a silent video works and has no audio stream
        let silent = root.join("quiet.mp4");
        make_video(ff, &silent, false);
        let (made, errors) = compress(ff, &silent, Goal::TargetBytes(1_000_000), NONE);
        assert!(errors.is_empty(), "{errors:?}");
        assert!(size(&made[0]) > 0 && !has_audio_stream(ff, &made[0]));
        assert!((duration_of(ff, &made[0]) - 20.0).abs() < 0.3);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    #[ignore = "requires ffmpeg/ffprobe; creates isolated real media"]
    fn compress_audio_extensions_and_sizes() {
        let ff = Tools::detect().ffmpeg.expect("ffmpeg is required for this explicit check");
        let root = scratch("audio");
        let wav = root.join("song.wav");
        command(ff, &["-f", "lavfi", "-i", "sine=frequency=440:duration=20"], &wav);
        let mp3 = root.join("tune.mp3");
        command(ff, &["-f", "lavfi", "-i", "sine=frequency=440:duration=20", "-c:a", "libmp3lame", "-b:a", "256k"], &mp3);
        let wav_bytes = std::fs::read(&wav).unwrap();
        let (made, errors) = compress(ff, &wav, Goal::Preset(Preset::Balanced), NONE);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(made, [root.join("song compressed.m4a")], "a wav becomes m4a");
        assert!(size(&made[0]) < wav_bytes.len() as u64);
        assert!((duration_of(ff, &made[0]) - 20.0).abs() < 0.3);
        let (made, errors) = compress(ff, &mp3, Goal::TargetBytes(240_000), NONE);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(made, [root.join("tune compressed.mp3")], "an mp3 stays mp3");
        assert!(size(&made[0]) < size(&mp3));
        assert_eq!(std::fs::read(&wav).unwrap(), wav_bytes, "the original is untouched");
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    #[ignore = "requires ffmpeg/ffprobe; creates isolated real media"]
    fn compress_impossible_target_creates_nothing() {
        let ff = Tools::detect().ffmpeg.expect("ffmpeg is required for this explicit check");
        let root = scratch("impossible");
        let src = root.join("clip.mp4");
        make_video(ff, &src, true);
        let au = root.join("tune.mp3");
        command(ff, &["-f", "lavfi", "-i", "sine=frequency=440:duration=20", "-c:a", "libmp3lame"], &au);
        let before = names(&root);
        let (made, errors) = compress(ff, &src, Goal::TargetBytes(20_000), NONE);
        assert!(made.is_empty(), "{made:?}");
        assert!(errors.len() == 1 && errors[0].contains("target too small for 20 s of video"), "{errors:?}");
        let (made, errors) = compress(ff, &au, Goal::TargetBytes(20_000), NONE);
        assert!(made.is_empty() && errors[0].contains("target too small for 20 s of audio"), "{errors:?}");
        assert_eq!(names(&root), before, "nothing was created");
        // a missing ffmpeg gives the install hint
        let (made, errors) = run_file_tool(&[src], &FileTool::Compress(Goal::Preset(Preset::Light), NONE), None);
        assert!(made.is_empty() && errors[0].contains("brew install ffmpeg"));
        std::fs::remove_dir_all(&root).unwrap();
    }
}
