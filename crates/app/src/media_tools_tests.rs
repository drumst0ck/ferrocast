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

mod pdf_compress {
    use crate::route::compress_pdf;
    use ferrocast_core::compress::Preset;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    fn names(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        v.sort();
        v
    }

    /// Three noisy JPEGs -> one three-page PDF (sips), so there is real image weight to shrink.
    fn noisy_pdf(dir: &Path) -> PathBuf {
        let mut pages = vec![];
        for i in 0..3u32 {
            let mut img = image::RgbImage::new(900, 900);
            let mut x = 12345u32 + i;
            for px in img.pixels_mut() {
                x = x.wrapping_mul(1664525).wrapping_add(1013904223);
                *px = image::Rgb([(x >> 8) as u8, (x >> 16) as u8, (x >> 24) as u8]);
            }
            let png = dir.join(format!("n{i}.png"));
            img.save(&png).unwrap();
            let jpg = dir.join(format!("n{i}.jpg"));
            assert!(Command::new("/usr/bin/sips")
                .args(["-s", "format", "jpeg"])
                .arg(&png)
                .arg("--out")
                .arg(&jpg)
                .output()
                .unwrap()
                .status
                .success());
            let pdf = dir.join(format!("p{i}.pdf"));
            assert!(Command::new("/usr/bin/sips")
                .args(["-s", "format", "pdf"])
                .arg(&jpg)
                .arg("--out")
                .arg(&pdf)
                .output()
                .unwrap()
                .status
                .success());
            pages.push(pdf);
        }
        let out = dir.join("doc.pdf");
        let join = "/System/Library/Automator/Combine PDF Pages.action/Contents/MacOS/join";
        assert!(Command::new(join).arg("-o").arg(&out).args(&pages).status().unwrap().success());
        for p in pages {
            std::fs::remove_file(p).unwrap();
        }
        for i in 0..3 {
            std::fs::remove_file(dir.join(format!("n{i}.png"))).unwrap();
            std::fs::remove_file(dir.join(format!("n{i}.jpg"))).unwrap();
        }
        out
    }

    fn root(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ferrocast-pdfc-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn missing_ghostscript_returns_the_hint_and_creates_nothing() {
        let d = root("none");
        let f = d.join("a.pdf");
        std::fs::write(&f, b"%PDF-1.4").unwrap();
        let before = names(&d);
        let (outs, errs) = compress_pdf(None, std::slice::from_ref(&f), Preset::Strong);
        assert!(outs.is_empty());
        assert_eq!(errs, ["Ghostscript (brew install ghostscript)"]);
        assert_eq!(names(&d), before);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    #[ignore = "requires ghostscript; builds a real image PDF"]
    fn compress_pdf_strong_is_smaller_keeps_pages_and_never_touches_the_original() {
        let gs = ferrocast_core::convert::Tools::detect().gs.expect("ghostscript is required for this explicit check");
        let d = root("real");
        let src = noisy_pdf(&d);
        let orig = std::fs::read(&src).unwrap();
        let pages = ferrocast_macos::pdf::page_count(&src).unwrap();
        assert_eq!(pages, 3);
        let (outs, errs) = compress_pdf(Some(gs), std::slice::from_ref(&src), Preset::Strong);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(outs, [d.join("doc compressed.pdf")]);
        let out = std::fs::read(&outs[0]).unwrap();
        assert!(out.starts_with(b"%PDF"));
        assert!(out.len() < orig.len(), "{} vs {}", out.len(), orig.len());
        assert_eq!(ferrocast_macos::pdf::page_count(&outs[0]).unwrap(), pages);
        assert_eq!(std::fs::read(&src).unwrap(), orig, "original untouched");
        let (again, errs) = compress_pdf(Some(gs), std::slice::from_ref(&src), Preset::Strong);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(again, [d.join("doc compressed 2.pdf")]);
        assert!(names(&d).iter().all(|n| !n.contains("ferrocast")), "{:?}", names(&d));
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    #[ignore = "requires ghostscript"]
    fn corrupt_pdf_fails_cleanly_and_leaves_nothing() {
        let gs = ferrocast_core::convert::Tools::detect().gs.expect("ghostscript is required for this explicit check");
        let d = root("bad");
        let f = d.join("fake.pdf");
        std::fs::write(&f, b"this is not a pdf at all").unwrap();
        let before = names(&d);
        let (outs, errs) = compress_pdf(Some(gs), std::slice::from_ref(&f), Preset::Balanced);
        assert!(outs.is_empty(), "{outs:?}");
        assert_eq!(errs.len(), 1, "{errs:?}");
        assert_eq!(names(&d), before);
        std::fs::remove_dir_all(&d).unwrap();
    }
}

mod metadata_real {
    use crate::media_tools::inspect_metadata;
    use crate::services::run_file_tool;
    use ferrocast_core::convert::{FileTool, Tools};
    use ferrocast_core::metaform::{Field, Group, MetaEdit};
    use std::path::{Path, PathBuf};
    use std::process::Command;

    fn ffmpeg() -> &'static str {
        Tools::detect().ffmpeg.expect("ffmpeg is required for this explicit check")
    }

    fn make(args: &[&str], out: &Path) {
        let o = Command::new(ffmpeg()).args(["-nostdin", "-y", "-loglevel", "error"]).args(args).arg(out).output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    }

    fn root(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("fc-meta-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir(&d).unwrap();
        d
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut v: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        v.sort();
        v
    }

    fn probe(f: &Path) -> serde_json::Value {
        let ffprobe = Path::new(ffmpeg()).with_file_name("ffprobe");
        let o = Command::new(ffprobe)
            .args(["-v", "quiet", "-show_format", "-show_streams", "-show_chapters", "-of", "json"])
            .arg(f)
            .output()
            .unwrap();
        serde_json::from_slice(&o.stdout).unwrap()
    }

    fn file_tag(f: &Path, key: &str) -> Option<String> {
        probe(f)["format"]["tags"][key].as_str().map(String::from)
    }

    /// Matroska stores some keys upper-case (ARTIST); look a tag up ignoring case.
    fn file_tag_ci(f: &Path, key: &str) -> Option<String> {
        let j = probe(f);
        j["format"]["tags"].as_object()?.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).and_then(|(_, v)| v.as_str().map(String::from))
    }

    fn codecs(f: &Path) -> Vec<String> {
        probe(f)["streams"].as_array().unwrap().iter().map(|s| s["codec_name"].as_str().unwrap().to_string()).collect()
    }

    fn duration(f: &Path) -> f64 {
        probe(f)["format"]["duration"].as_str().unwrap().parse().unwrap()
    }

    fn set(key: &str, v: &str) -> MetaEdit {
        MetaEdit { group: Group::File, key: key.into(), value: Some(v.into()) }
    }

    fn unset(key: &str) -> MetaEdit {
        MetaEdit { group: Group::File, key: key.into(), value: None }
    }

    fn mp4(d: &Path) -> PathBuf {
        let p = d.join("clip.mp4");
        make(
            &[
                "-f",
                "lavfi",
                "-i",
                "testsrc=size=160x120:rate=10",
                "-f",
                "lavfi",
                "-i",
                "sine=duration=2",
                "-t",
                "2",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
                "-metadata",
                "title=Título ñ",
                "-metadata",
                "artist=Bob",
                "-metadata",
                "comment=hi",
                "-metadata:s:a:0",
                "language=spa",
            ],
            &p,
        );
        p
    }

    fn run(f: &Path, edits: Vec<MetaEdit>, all: bool) -> (Vec<PathBuf>, Vec<String>) {
        run_file_tool(std::slice::from_ref(&f.to_path_buf()), &FileTool::EditMetadata(edits, all), Some(ffmpeg()))
    }

    #[test]
    #[ignore = "requires ffmpeg/ffprobe; creates isolated real media"]
    fn mp4_edit_keeps_the_rest_and_the_streams() {
        let d = root("mp4");
        let src = mp4(&d);
        let before = std::fs::read(&src).unwrap();
        let (outs, errs) = run(&src, vec![set("title", "Nuevo = ñ"), unset("artist")], false);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(outs, [d.join("clip edited.mp4")]);
        let out = &outs[0];
        assert_eq!(file_tag(out, "title").as_deref(), Some("Nuevo = ñ"));
        assert_eq!(file_tag(out, "artist"), None);
        assert_eq!(file_tag(out, "comment").as_deref(), Some("hi"));
        assert_eq!(codecs(out), codecs(&src));
        assert!((duration(out) - duration(&src)).abs() < 0.1);
        assert_eq!(probe(out)["streams"][1]["tags"]["language"], "spa", "track tags survive");
        assert_eq!(std::fs::read(&src).unwrap(), before, "original untouched");
        // a second run never overwrites
        let (again, errs) = run(&src, vec![set("title", "x")], false);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(again, [d.join("clip edited 2.mp4")]);
        // a track edit goes to that stream only
        let track = Group::Track { index: 1, kind: "audio".into() };
        let (t, errs) = run(&src, vec![MetaEdit { group: track, key: "language".into(), value: Some("fra".into()) }], false);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(probe(&t[0])["streams"][1]["tags"]["language"], "fra");
        assert_eq!(file_tag(&t[0], "artist").as_deref(), Some("Bob"));
        // the reader shows what the editor changes
        let fields: Vec<Field> = inspect_metadata(ffmpeg(), out).unwrap();
        assert!(fields.iter().any(|f| f.group == Group::File && f.key == "title" && f.value == "Nuevo = ñ" && f.editable));
        assert!(fields.iter().any(|f| matches!(&f.group, Group::Track { index: 1, kind } if kind == "audio") && f.key == "language"));
        assert!(fields.iter().all(|f| f.key != "artist"));
        assert!(names(&d).iter().all(|n| !n.contains("ferrocast")));
    }

    #[test]
    #[ignore = "requires ffmpeg/ffprobe; creates isolated real media"]
    fn remove_all_leaves_no_user_tags_and_no_chapters() {
        let d = root("all");
        // an mkv with chapters made through an ffmetadata file
        let meta = d.join("m.txt");
        std::fs::write(
            &meta,
            ";FFMETADATA1\ntitle=Movie\nartist=Bob\n[CHAPTER]\nTIMEBASE=1/1000\nSTART=0\nEND=1000\ntitle=Intro\n[CHAPTER]\nTIMEBASE=1/1000\nSTART=1000\nEND=2000\ntitle=Outro\n",
        )
        .unwrap();
        let mkv = d.join("c.mkv");
        make(
            &[
                "-f",
                "lavfi",
                "-i",
                "testsrc=size=64x64:rate=5",
                "-i",
                meta.to_str().unwrap(),
                "-t",
                "2",
                "-map",
                "0",
                "-map_metadata",
                "1",
                "-map_chapters",
                "1",
            ],
            &mkv,
        );
        let fields = inspect_metadata(ffmpeg(), &mkv).unwrap();
        let chapters: Vec<_> = fields.iter().filter(|f| matches!(f.group, Group::Chapter(_))).collect();
        assert_eq!(chapters.len(), 2, "{fields:?}");
        assert_eq!((chapters[0].value.as_str(), chapters[1].value.as_str()), ("Intro", "Outro"));
        assert!(chapters.iter().all(|f| !f.editable));
        assert!(fields.iter().any(|f| f.group == Group::File && f.key == "title" && f.value == "Movie"));

        // a title edit keeps the chapters
        let (e, errs) = run(&mkv, vec![set("title", "Z")], false);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(file_tag(&e[0], "title").as_deref(), Some("Z"));
        assert_eq!(probe(&e[0])["chapters"].as_array().unwrap().len(), 2);
        assert_eq!(file_tag_ci(&e[0], "artist").as_deref(), Some("Bob"));

        // remove all: no title/artist/comment, no chapters
        let (r, errs) = run(&mkv, vec![], true);
        assert!(errs.is_empty(), "{errs:?}");
        let j = probe(&r[0]);
        assert_eq!(j["chapters"].as_array().unwrap().len(), 0);
        for k in ["title", "artist", "comment"] {
            assert_eq!(file_tag_ci(&r[0], k), None, "{k}");
        }
        assert_eq!(codecs(&r[0]), codecs(&mkv));

        let src = mp4(&d);
        let (r, errs) = run(&src, vec![set("title", "ignored")], true);
        assert!(errs.is_empty(), "{errs:?}");
        let tags = probe(&r[0])["format"]["tags"].clone();
        let keys: Vec<&str> = tags.as_object().unwrap().keys().map(String::as_str).collect();
        // only the container's own brand fields remain; the encoder tag is gone with +bitexact
        assert!(keys.iter().all(|k| ["major_brand", "minor_version", "compatible_brands"].contains(k)), "{keys:?}");
        let s = probe(&r[0])["streams"].clone();
        assert!(s.as_array().unwrap().iter().all(|st| st["tags"].get("encoder").is_none()));
        assert_eq!(s[1]["tags"]["language"], "und", "track language reset");
        assert_eq!(codecs(&r[0]), codecs(&src));
        assert_eq!(r[0], d.join("clip edited.mp4"));
    }

    #[test]
    #[ignore = "requires ffmpeg/ffprobe; creates isolated real media"]
    fn mp3_id3_edit_and_remove_all() {
        let d = root("mp3");
        let src = d.join("a.mp3");
        make(
            &["-f", "lavfi", "-i", "sine=duration=2", "-metadata", "title=Canción", "-metadata", "artist=Ann", "-metadata", "comment=c"],
            &src,
        );
        let (o, errs) = run(&src, vec![set("title", "Otra ñ"), unset("artist")], false);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(o, [d.join("a edited.mp3")]);
        assert_eq!(file_tag(&o[0], "title").as_deref(), Some("Otra ñ"));
        assert_eq!(file_tag(&o[0], "artist"), None);
        assert_eq!(file_tag(&o[0], "comment").as_deref(), Some("c"));
        assert_eq!(codecs(&o[0]), ["mp3"]);
        let (r, errs) = run(&src, vec![], true);
        assert!(errs.is_empty(), "{errs:?}");
        for k in ["title", "artist", "comment", "encoder"] {
            assert_eq!(file_tag(&r[0], k), None, "{k}");
        }
    }

    #[test]
    #[ignore = "requires ffmpeg/ffprobe; creates isolated real media"]
    fn gif_cannot_store_tags_but_can_be_stripped() {
        let d = root("gif");
        let src = d.join("g.gif");
        make(&["-f", "lavfi", "-i", "testsrc=size=64x64:rate=5", "-t", "1"], &src);
        let (o, errs) = run(&src, vec![set("title", "x")], false);
        assert!(o.is_empty() && errs.len() == 1 && errs[0].contains("GIF"), "{o:?} {errs:?}");
        assert_eq!(names(&d), ["g.gif"]);
        let (r, errs) = run(&src, vec![], true);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(r, [d.join("g edited.gif")]);
        assert_eq!(codecs(&r[0]), ["gif"]);
    }

    #[test]
    #[ignore = "requires ffmpeg/ffprobe; creates isolated real media"]
    fn bad_input_fails_cleanly_and_leaves_nothing() {
        let d = root("bad");
        let fake = d.join("fake.mp4");
        std::fs::write(&fake, b"this is not media").unwrap();
        let txt = d.join("note.txt");
        std::fs::write(&txt, b"hello").unwrap();
        let before = names(&d);
        let (o, errs) = run(&fake, vec![set("title", "x")], false);
        assert!(o.is_empty() && errs.len() == 1, "{o:?} {errs:?}");
        let (o, errs) = run(&txt, vec![set("title", "x")], false);
        assert!(o.is_empty() && errs[0].contains("requires"), "{errs:?}");
        let src = mp4(&d);
        let after_src = names(&d);
        for edits in
            [vec![], vec![MetaEdit { group: Group::Chapter(0), key: "title".into(), value: Some("x".into()) }], vec![set("a=b", "x")]]
        {
            let (o, errs) = run(&src, edits, false);
            assert!(o.is_empty() && errs.len() == 1, "{o:?} {errs:?}");
        }
        assert_eq!(names(&d), after_src);
        assert_eq!(before.len() + 1, after_src.len());
        assert!(inspect_metadata(ffmpeg(), &fake).is_err());
        // without ffmpeg the install hint comes back and nothing is written
        let (o, errs) = run_file_tool(std::slice::from_ref(&src), &FileTool::EditMetadata(vec![set("title", "x")], false), None);
        assert!(o.is_empty() && errs[0].contains("brew install ffmpeg"), "{errs:?}");
        assert_eq!(names(&d), after_src);
    }

    #[test]
    #[ignore = "requires PDFKit through osascript; creates isolated real PDFs"]
    fn pdf_edit_routes_to_pdfkit_without_ffmpeg() {
        let d = root("pdf");
        let src = d.join("doc.pdf");
        let script = r#"ObjC.import('AppKit'); ObjC.import('Quartz');
function run(argv) { var doc = $.PDFDocument.alloc.init;
  for (var i = 0; i < 2; i++) { var tv = $.NSTextView.alloc.initWithFrame($.NSMakeRect(0,0,300,300)); tv.string = 'p' + i;
    doc.insertPageAtIndex($.PDFDocument.alloc.initWithData(tv.dataWithPDFInsideRect(tv.bounds)).pageAtIndex(0).copy, i); }
  doc.writeToFile(argv[0]); return 'ok'; }"#;
        assert!(Command::new("/usr/bin/osascript").args(["-l", "JavaScript", "-e", script]).arg(&src).output().unwrap().status.success());
        let before = std::fs::read(&src).unwrap();
        let tool = FileTool::EditMetadata(vec![set("Title", "Informe ñ"), set("Author", "Ann")], false);
        let (o, errs) = run_file_tool(std::slice::from_ref(&src), &tool, None);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(o, [d.join("doc edited.pdf")]);
        let m = ferrocast_macos::pdf::metadata(&o[0]).unwrap();
        assert!(m.contains(&("Title".into(), "Informe ñ".into())) && m.contains(&("Author".into(), "Ann".into())), "{m:?}");
        assert_eq!(ferrocast_macos::pdf::page_count(&o[0]).unwrap(), 2);
        let (z, errs) = run_file_tool(&[o[0].clone()], &FileTool::EditMetadata(vec![], true), None);
        assert!(errs.is_empty(), "{errs:?}");
        assert!(ferrocast_macos::pdf::metadata(&z[0]).unwrap().is_empty());
        assert_eq!(std::fs::read(&src).unwrap(), before);
        let (o, errs) = run_file_tool(std::slice::from_ref(&src), &FileTool::EditMetadata(vec![unset("Title")], false), None);
        assert_eq!(o, [d.join("doc edited 2.pdf")], "{errs:?}");
    }
}
