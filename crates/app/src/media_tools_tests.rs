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

// ---- A10b: audio sample extractor

mod audio_samples_pure {
    use crate::media_tools::pcm_s16le;
    use crate::route::read_capped;

    #[test]
    fn little_endian_bytes_become_i16_and_an_odd_trailing_byte_is_ignored() {
        assert_eq!(pcm_s16le(&[0x01, 0x00, 0xff, 0xff, 0x00, 0x80, 0xff, 0x7f]), vec![1, -1, i16::MIN, i16::MAX]);
        assert_eq!(pcm_s16le(&[0x34, 0x12, 0x99]), vec![0x1234], "the lone last byte is dropped");
        assert!(pcm_s16le(&[0x55]).is_empty());
        assert!(pcm_s16le(&[]).is_empty());
    }

    #[test]
    fn read_capped_stops_at_the_cap_flags_truncation_and_drains_the_rest() {
        let data: Vec<u8> = (0..100u8).collect();
        let mut src = std::io::Cursor::new(data.clone());
        let (got, truncated) = read_capped(&mut src, 40);
        assert_eq!(got, &data[..40]);
        assert!(truncated);
        assert_eq!(src.position(), 100, "the source was drained to the end so a child never blocks");
    }

    #[test]
    fn read_capped_is_not_truncated_at_or_below_the_cap() {
        let data = vec![7u8; 40];
        assert_eq!(read_capped(std::io::Cursor::new(data.clone()), 40), (data.clone(), false));
        assert_eq!(read_capped(std::io::Cursor::new(data.clone()), 1000), (data, false));
        assert_eq!(read_capped(std::io::Cursor::new(Vec::<u8>::new()), 10), (vec![], false));
    }
}

mod audio_samples_real {
    use crate::media_tools::{audio_samples, AUDIO_CAP, AUDIO_RATE};
    use ferrocast_core::convert::Tools;
    use ferrocast_core::waveform::{peaks, silence_bounds};
    use std::process::Command;

    fn ffmpeg(ff: &str, args: &[&str]) {
        crate::route::run_limited(Command::new(ff).args(["-nostdin", "-y", "-loglevel", "error"]).args(args), 60).unwrap();
    }

    #[test]
    #[ignore = "requires ffmpeg/ffprobe; creates isolated real media"]
    fn a_three_second_tone_gives_about_24000_samples_with_a_real_peak() {
        let ff = Tools::detect().ffmpeg.expect("ffmpeg is required for this explicit check");
        let dir = std::env::temp_dir().join(format!("fc-a10b-tone-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("tone.mp3");
        ffmpeg(ff, &["-f", "lavfi", "-i", "sine=f=440:d=3", f.to_str().unwrap()]);
        let a = audio_samples(ff, &f, AUDIO_CAP).unwrap();
        assert_eq!(a.rate, AUDIO_RATE);
        assert!(!a.truncated);
        assert!((a.samples.len() as i64 - 24000).abs() < 1200, "{} samples", a.samples.len());
        assert!((a.duration - 3.0).abs() < 0.1, "duration {}", a.duration);
        let top = peaks(&a.samples, 100).iter().map(|p| p.1.max(-p.0)).max().unwrap();
        // ffmpeg's sine is 1/8 of full scale (about 4096)
        assert!(top > 2000, "the tone must have a clear peak, got {top}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    #[ignore = "requires ffmpeg/ffprobe; creates isolated real media"]
    fn silence_tone_silence_clip_has_bounds_near_one_and_four_seconds() {
        let ff = Tools::detect().ffmpeg.expect("ffmpeg is required for this explicit check");
        let dir = std::env::temp_dir().join(format!("fc-a10b-bounds-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("t.mp3");
        ffmpeg(
            ff,
            &[
                "-f",
                "lavfi",
                "-i",
                "anullsrc=r=44100:cl=mono:d=1",
                "-f",
                "lavfi",
                "-i",
                "sine=f=440:r=44100:d=3",
                "-f",
                "lavfi",
                "-i",
                "anullsrc=r=44100:cl=mono:d=2",
                "-filter_complex",
                "[0][1][2]concat=n=3:v=0:a=1",
                f.to_str().unwrap(),
            ],
        );
        let a = audio_samples(ff, &f, AUDIO_CAP).unwrap();
        assert!((a.duration - 6.0).abs() < 0.2, "duration {}", a.duration);
        let (s, e) = silence_bounds(&a.samples, a.rate, -40.0).expect("there is sound");
        assert!((s - 1.0).abs() < 0.1, "start {s}");
        assert!((e - 4.0).abs() < 0.1, "end {e}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    #[ignore = "requires ffmpeg/ffprobe; creates isolated real media"]
    fn a_missing_or_garbage_file_is_an_error_not_a_panic() {
        let ff = Tools::detect().ffmpeg.expect("ffmpeg is required for this explicit check");
        let dir = std::env::temp_dir().join(format!("fc-a10b-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(audio_samples(ff, &dir.join("missing.mp3"), AUDIO_CAP).is_err());
        let g = dir.join("garbage.mp3");
        std::fs::write(&g, b"this is not audio at all, just text").unwrap();
        assert!(audio_samples(ff, &g, AUDIO_CAP).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    #[ignore = "requires ffmpeg/ffprobe; creates isolated real media"]
    fn a_tiny_cap_truncates_and_still_finishes() {
        let ff = Tools::detect().ffmpeg.expect("ffmpeg is required for this explicit check");
        let dir = std::env::temp_dir().join(format!("fc-a10b-cap-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("tone.wav");
        ffmpeg(ff, &["-f", "lavfi", "-i", "sine=f=440:d=3", f.to_str().unwrap()]);
        let a = audio_samples(ff, &f, 8000).unwrap();
        assert!(a.truncated);
        assert_eq!(a.samples.len(), 4000, "8000 bytes are 4000 samples");
        assert!((a.duration - 3.0).abs() < 0.1, "the duration is still the whole file's");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// A10d/A10e: the pure parameter checks behind the normalize and channels windows.
mod a10de {
    use super::*;
    use crate::media_tools::{channels_params, db_to_gain, normalize_params};

    #[test]
    fn normalize_params_accepts_the_window_ranges_and_builds_the_tool() {
        assert_eq!(normalize_params(-16.0, 11.0, -1.5), Ok(FileTool::Normalize(-16.0, 11.0, -1.5)));
        assert_eq!(normalize_params(-30.0, 1.0, -9.0), Ok(FileTool::Normalize(-30.0, 1.0, -9.0)));
        assert_eq!(normalize_params(-5.0, 20.0, 0.0), Ok(FileTool::Normalize(-5.0, 20.0, 0.0)));
    }

    #[test]
    fn normalize_params_refuses_out_of_range_and_non_numbers() {
        for (t, r, p) in [
            (-30.1, 11.0, -1.5),
            (-4.9, 11.0, -1.5),
            (-16.0, 0.9, -1.5),
            (-16.0, 20.1, -1.5),
            (-16.0, 11.0, -9.1),
            (-16.0, 11.0, 0.1),
            (f64::NAN, 11.0, -1.5),
            (-16.0, f64::NAN, -1.5),
            (-16.0, 11.0, f64::NAN),
            (f64::INFINITY, 11.0, -1.5),
            (-16.0, f64::NEG_INFINITY, -1.5),
        ] {
            let e = normalize_params(t, r, p).expect_err(&format!("{t} {r} {p}"));
            assert!(!e.is_empty());
        }
    }

    #[test]
    fn db_to_gain_is_unity_at_zero_and_half_at_minus_six() {
        assert_eq!(db_to_gain(0.0), 1.0);
        assert!((db_to_gain(-6.0) - 0.5).abs() < 0.01, "{}", db_to_gain(-6.0));
        assert!((db_to_gain(6.0) - 2.0).abs() < 0.02);
        assert!((db_to_gain(-20.0) - 0.1).abs() < 1e-9);
    }

    #[test]
    fn channels_params_converts_db_to_the_gain_the_tool_expects() {
        assert_eq!(channels_params(true, 0.0, 0.0), Ok(FileTool::Channels(true, 1.0, 1.0)));
        assert_eq!(channels_params(false, 0.0, 0.0), Ok(FileTool::Channels(false, 1.0, 1.0)));
        match channels_params(true, -6.0, 12.0).unwrap() {
            FileTool::Channels(true, l, r) => {
                assert!((l - 0.5).abs() < 0.01, "{l}");
                assert!((r - 3.98).abs() < 0.01 && r <= 4.0, "{r}");
            }
            other => panic!("{other:?}"),
        }
        // the lowest setting stays a valid, quiet gain (the executor accepts 0.0..=4.0)
        assert!(matches!(channels_params(false, -30.0, -30.0), Ok(FileTool::Channels(false, l, _)) if l > 0.0 && l < 0.04));
    }

    #[test]
    fn channels_params_refuses_out_of_range_and_non_numbers() {
        for (l, r) in [(-30.1, 0.0), (0.0, -31.0), (12.1, 0.0), (0.0, 13.0), (f64::NAN, 0.0), (0.0, f64::NAN), (f64::INFINITY, 0.0)] {
            assert!(channels_params(true, l, r).is_err(), "{l} {r}");
        }
    }

    #[test]
    #[ignore = "requires ffmpeg/ffprobe; creates isolated real media"]
    fn a10d_measure_and_normalize_a_quiet_tone_and_probe_channels() {
        let ff = Tools::detect().ffmpeg.expect("ffmpeg is required for this explicit check");
        let dir = std::env::temp_dir().join(format!("fc-a10d-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let quiet = dir.join("quiet.wav");
        command(ff, &["-f", "lavfi", "-i", "sine=f=440:d=4", "-af", "volume=-8dB"], &quiet);
        let before = crate::media_tools::loudness(ff, &quiet).expect("measured");
        assert!(before.is_finite() && before < -20.0 && before > -45.0, "plausible quiet tone, got {before}");
        // exactly what the window dispatches
        let tool = normalize_params(-16.0, 11.0, -1.5).unwrap();
        let (out, errors) = run_file_tool(std::slice::from_ref(&quiet), &tool, Some(ff));
        assert!(errors.is_empty(), "{errors:?}");
        let after = crate::media_tools::loudness(ff, &out[0]).expect("measured result");
        assert!((after - -16.0).abs() < 2.0, "normalized to about -16 LUFS, got {after} (was {before})");
        // channels probe: a mono tone and a stereo one
        let stereo = dir.join("stereo.wav");
        command(ff, &["-f", "lavfi", "-i", "sine=f=440:d=1", "-ac", "2"], &stereo);
        assert_eq!(crate::media_tools::audio_channel_count(ff, &quiet), Some(1));
        assert_eq!(crate::media_tools::audio_channel_count(ff, &stereo), Some(2));
        assert_eq!(crate::media_tools::audio_channel_count(ff, &dir.join("missing.wav")), None);
        // -6 dB on both sides of a stereo copy lowers the loudness by about 6 LU
        let tool = channels_params(true, -6.0, -6.0).unwrap();
        let (out, errors) = run_file_tool(std::slice::from_ref(&stereo), &tool, Some(ff));
        assert!(errors.is_empty(), "{errors:?}");
        let (a, b) = (crate::media_tools::loudness(ff, &stereo).unwrap(), crate::media_tools::loudness(ff, &out[0]).unwrap());
        assert!((a - b - 6.0).abs() < 0.5, "{a} -> {b}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Timed redaction of several areas (A11b), checked on decoded pixels.
mod redact_many_real {
    use super::{duration_of, names};
    use crate::services::run_file_tool;
    use ferrocast_core::convert::{FileTool, Tools};
    use ferrocast_core::redactplan::Redaction;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    fn ffmpeg() -> &'static str {
        Tools::detect().ffmpeg.expect("ffmpeg is required for this explicit check")
    }

    fn ff(args: &[&str]) {
        crate::route::run_limited(Command::new(ffmpeg()).args(["-nostdin", "-y", "-loglevel", "error"]).args(args), 60).unwrap();
    }

    fn root(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("fc-redmany-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir(&d).unwrap();
        d
    }

    fn flat(dir: &Path) -> PathBuf {
        let p = dir.join("source.mp4");
        ff(&[
            "-f",
            "lavfi",
            "-i",
            "color=c=0x3070d0:s=320x180:r=25:d=6",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=6",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
            p.to_str().unwrap(),
        ]);
        p
    }

    fn area(x: u32, y: u32, w: u32, h: u32, start: f64, end: f64, blur: bool) -> Redaction {
        Redaction { x, y, w, h, start, end, blur }
    }

    /// RGB of the displayed pixel (x, y) of the frame at `t`.
    fn pixel(file: &Path, t: f64, x: u32, y: u32) -> [u8; 3] {
        let out = Command::new(ffmpeg())
            .args(["-nostdin", "-v", "error", "-ss", &t.to_string(), "-i"])
            .arg(file)
            .args(["-frames:v", "1", "-vf", &format!("format=rgb24,crop=1:1:{x}:{y}"), "-f", "rawvideo", "-pix_fmt", "rgb24", "-"])
            .output()
            .unwrap();
        assert_eq!(out.stdout.len(), 3, "no frame at {t}: {}", String::from_utf8_lossy(&out.stderr));
        [out.stdout[0], out.stdout[1], out.stdout[2]]
    }

    fn black(p: [u8; 3]) -> bool {
        p.iter().map(|&c| u32::from(c)).sum::<u32>() < 45
    }

    fn is_blue(p: [u8; 3]) -> bool {
        let d = |a: u8, b: u8| (i32::from(a) - i32::from(b)).abs();
        d(p[0], 0x30) < 16 && d(p[1], 0x70) < 16 && d(p[2], 0xd0) < 16
    }

    fn run(file: &Path, areas: Vec<Redaction>) -> (Vec<PathBuf>, Vec<String>) {
        run_file_tool(&[file.to_path_buf()], &FileTool::RedactVideoMany(areas), Some(ffmpeg()))
    }

    #[test]
    #[ignore = "requires ffmpeg/ffprobe; creates isolated real media"]
    fn two_solid_areas_at_different_times() {
        let d = root("two");
        let src = flat(&d);
        let original = std::fs::read(&src).unwrap();
        let (made, e) = run(&src, vec![area(10, 10, 60, 40, 1.0, 2.0, false), area(200, 100, 60, 50, 4.0, 5.0, false)]);
        assert!(e.is_empty(), "{e:?}");
        assert_eq!(made.len(), 1);
        assert_eq!(made[0].file_name().unwrap(), "source redacted.mp4");
        let o = &made[0];
        assert!((duration_of(ffmpeg(), o) - 6.0).abs() < 0.2);
        let (a_in, a_out, b_in, b_out) = ((40, 30), (120, 30), (230, 125), (120, 125));
        assert!(black(pixel(o, 1.5, a_in.0, a_in.1)), "A covered at 1.5");
        assert!(is_blue(pixel(o, 1.5, a_out.0, a_out.1)), "outside A untouched");
        assert!(!black(pixel(o, 1.5, b_in.0, b_in.1)), "B not yet active at 1.5");
        assert!(black(pixel(o, 4.5, b_in.0, b_in.1)), "B covered at 4.5");
        assert!(is_blue(pixel(o, 4.5, b_out.0, b_out.1)), "outside B untouched");
        assert!(!black(pixel(o, 4.5, a_in.0, a_in.1)), "A over by 4.5");
        for t in [0.3, 2.6, 3.0, 5.6] {
            assert!(is_blue(pixel(o, t, a_in.0, a_in.1)) && is_blue(pixel(o, t, b_in.0, b_in.1)), "nothing covered at {t}");
        }
        let probe = Path::new(ffmpeg()).with_file_name("ffprobe");
        let streams =
            Command::new(probe).args(["-v", "error", "-show_entries", "stream=codec_name", "-of", "csv=p=0"]).arg(o).output().unwrap();
        assert!(String::from_utf8_lossy(&streams.stdout).contains("aac"), "audio kept");
        assert_eq!(std::fs::read(&src).unwrap(), original, "original untouched");
        // second run: never clobbers, takes " 2"
        let (again, e) = run(&src, vec![area(10, 10, 60, 40, 1.0, 2.0, false)]);
        assert!(e.is_empty(), "{e:?}");
        assert_eq!(again[0].file_name().unwrap(), "source redacted 2.mp4");
        assert_eq!(names(&d), ["source redacted 2.mp4", "source redacted.mp4", "source.mp4"]);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    #[ignore = "requires ffmpeg/ffprobe; creates isolated real media"]
    fn overlapping_areas_compose() {
        let d = root("overlap");
        let src = flat(&d);
        let (made, e) = run(&src, vec![area(10, 10, 100, 60, 0.0, 3.0, false), area(60, 40, 100, 60, 2.0, 5.0, false)]);
        assert!(e.is_empty(), "{e:?}");
        let o = &made[0];
        assert!(black(pixel(o, 2.5, 80, 55)), "overlap");
        assert!(black(pixel(o, 2.5, 20, 20)) && black(pixel(o, 2.5, 150, 90)), "each alone");
        assert!(!black(pixel(o, 1.0, 150, 90)), "B not yet");
        assert!(!black(pixel(o, 4.0, 20, 20)), "A over");
        assert!(black(pixel(o, 4.0, 80, 55)), "B alone covers the overlap");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    #[ignore = "requires ffmpeg/ffprobe; creates isolated real media"]
    fn blur_area_softens_an_edge_and_leaves_the_rest() {
        let d = root("blur");
        let src = d.join("edge.mp4");
        ff(&[
            "-filter_complex",
            "color=c=black:s=160x180:r=25:d=2[l];color=c=white:s=160x180:r=25:d=2[r];[l][r]hstack[v]",
            "-map",
            "[v]",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            src.to_str().unwrap(),
        ]);
        let luma = |p: [u8; 3]| i32::from(p[0]);
        let before = luma(pixel(&src, 1.0, 170, 90)) - luma(pixel(&src, 1.0, 150, 90));
        assert!(before > 220, "sharp source edge: {before}");
        let (made, e) = run(&src, vec![area(100, 40, 120, 100, 0.0, 2.0, true)]);
        assert!(e.is_empty(), "{e:?}");
        let o = &made[0];
        let after = luma(pixel(o, 1.0, 170, 90)) - luma(pixel(o, 1.0, 150, 90));
        assert!(after < before - 40, "contrast across the edge drops inside the area: {before} -> {after}");
        assert_eq!(luma(pixel(o, 1.0, 20, 10)), luma(pixel(&src, 1.0, 20, 10)), "outside untouched");
        assert!(luma(pixel(o, 1.0, 310, 170)) > 220, "outside untouched (white side)");
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    #[ignore = "requires ffmpeg/ffprobe; creates isolated real media"]
    fn rotated_source_uses_displayed_space() {
        let d = root("rot");
        let flat = flat(&d);
        let src = d.join("phone.mp4");
        ff(&["-display_rotation", "90", "-i", flat.to_str().unwrap(), "-c", "copy", src.to_str().unwrap()]);
        let dims = crate::media_tools::video_dims(ffmpeg(), &src).unwrap();
        assert_eq!(dims, (180, 320), "fixture is displayed portrait");
        // x+w = 200 > 180: outside the displayed frame, although inside the raw 320x180
        let (made, e) = run(&src, vec![area(120, 10, 80, 40, 0.0, 1.0, false)]);
        assert!(made.is_empty() && e.len() == 1 && e[0].contains("area 1"), "{e:?}");
        // y = 250 + 50 is only inside the displayed frame
        let (made, e) = run(&src, vec![area(100, 250, 60, 50, 1.0, 2.0, false)]);
        assert!(e.is_empty(), "{e:?}");
        let o = &made[0];
        assert_eq!(crate::media_tools::video_dims(ffmpeg(), o), Some((180, 320)));
        assert!(black(pixel(o, 1.5, 120, 270)), "covered in displayed space");
        assert!(!black(pixel(o, 0.5, 120, 270)), "gate respected");
        assert!(is_blue(pixel(o, 1.5, 20, 20)));
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    #[ignore = "requires ffmpeg/ffprobe; creates isolated real media"]
    fn invalid_areas_create_nothing() {
        let d = root("bad");
        let src = flat(&d);
        let before = names(&d);
        let scratch = |p: &str| {
            std::fs::read_dir(std::env::temp_dir()).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().starts_with(p)).count()
        };
        let prefix = format!("ferrocast-media-{}-", std::process::id());
        let scratch_before = scratch(&prefix);
        let ok = area(10, 10, 20, 20, 0.0, 1.0, false);
        for (areas, needle) in [
            (vec![ok, area(300, 10, 50, 20, 0.0, 1.0, false)], "area 2"),
            (vec![area(10, 10, 20, 20, 2.0, 1.0, false)], "area 1"),
            (vec![area(10, 10, 20, 20, 0.0, 9.0, false)], "ends after"),
            (vec![], "at least one"),
            (vec![ok; 65], "too many"),
        ] {
            let (made, e) = run(&src, areas);
            assert!(made.is_empty() && e.len() == 1 && e[0].contains(needle), "{needle}: {e:?}");
        }
        assert_eq!(names(&d), before, "nothing published");
        assert_eq!(scratch(&prefix), scratch_before, "no scratch left (run with --test-threads=1)");
        std::fs::remove_dir_all(&d).unwrap();
    }
}

/// A10g: the pure job builder behind the audio to video window, and real media runs.
mod a10g {
    use super::*;
    use crate::media_tools::{visualize_job, visualize_line, VisStyle};
    use std::path::PathBuf;

    fn picture(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fc-a10g-pure-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        std::fs::write(&p, b"x").unwrap();
        p
    }

    const ALL: [Canvas; 3] = [Canvas::Landscape, Canvas::Portrait, Canvas::Square];

    #[test]
    fn every_orientation_with_the_waveform_builds_a_visualize_job() {
        for c in ALL {
            assert_eq!(visualize_job(c, VisStyle::Waveform, None), Ok(FileTool::Visualize(c)));
            // a leftover picture is ignored for the waveform
            assert_eq!(visualize_job(c, VisStyle::Waveform, Some(Path::new("/nonexistent/x.png"))), Ok(FileTool::Visualize(c)));
        }
        assert!(ALL.contains(&Canvas::Portrait));
    }

    #[test]
    fn every_orientation_with_a_picture_builds_a_visualize_image_job() {
        for name in ["a.png", "b.JPG", "c.jpeg", "d.webp", "e.bmp", "f.tiff", "g.tif"] {
            let p = picture(name);
            for c in ALL {
                assert_eq!(
                    visualize_job(c, VisStyle::Still, Some(&p)),
                    Ok(FileTool::VisualizeImage(c, p.to_string_lossy().into_owned())),
                    "{name}"
                );
            }
        }
    }

    #[test]
    fn a_missing_nonexistent_empty_or_wrong_picture_is_refused() {
        for c in ALL {
            assert!(visualize_job(c, VisStyle::Still, None).unwrap_err().to_lowercase().contains("picture"));
            assert!(visualize_job(c, VisStyle::Still, Some(Path::new("/nonexistent/gone.png"))).is_err());
            assert!(visualize_job(c, VisStyle::Still, Some(Path::new(""))).is_err());
            assert!(visualize_job(c, VisStyle::Still, Some(&picture("notes.txt"))).is_err(), "not a picture type");
            assert!(visualize_job(c, VisStyle::Still, Some(&std::env::temp_dir())).is_err(), "a folder");
        }
    }

    #[test]
    fn the_summary_line_uses_the_builder_sizes() {
        assert_eq!(visualize_line(Canvas::Landscape, VisStyle::Waveform), "1280×720 MP4 with the waveform");
        assert_eq!(visualize_line(Canvas::Portrait, VisStyle::Waveform), "720×1280 MP4 with the waveform");
        assert_eq!(visualize_line(Canvas::Square, VisStyle::Still), "1080×1080 MP4 with your picture");
        for c in ALL {
            let (w, h) = c.size();
            assert!(visualize_line(c, VisStyle::Waveform).starts_with(&format!("{w}×{h}")));
        }
    }

    fn video_size(ff: &str, file: &Path) -> (u32, u32) {
        let out = Command::new(Path::new(ff).with_file_name("ffprobe"))
            .args(["-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height", "-of", "csv=p=0"])
            .arg(file)
            .output()
            .unwrap();
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let (w, h) = s.split_once(',').unwrap_or_else(|| panic!("ffprobe said {s:?}"));
        (w.parse().unwrap(), h.parse().unwrap())
    }

    #[test]
    #[ignore = "requires ffmpeg/ffprobe; creates isolated real media"]
    fn a10g_portrait_waveform_and_still_image_make_videos_of_the_builder_size() {
        let ff = Tools::detect().ffmpeg.expect("ffmpeg is required for this explicit check");
        let dir = std::env::temp_dir().join(format!("fc-a10g-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let tone = dir.join("tone.mp3");
        command(ff, &["-f", "lavfi", "-i", "sine=f=440:d=3"], &tone);
        let before = std::fs::read(&tone).unwrap();
        // exactly what the window dispatches
        let job = visualize_job(Canvas::Portrait, VisStyle::Waveform, None).unwrap();
        let (out, errors) = run_file_tool(std::slice::from_ref(&tone), &job, Some(ff));
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].extension().and_then(|e| e.to_str()), Some("mp4"));
        assert_eq!(out[0].parent(), tone.parent(), "next to the original");
        assert_eq!(video_size(ff, &out[0]), Canvas::Portrait.size());
        assert!((duration_of(ff, &out[0]) - 3.0).abs() < 0.3, "{}", duration_of(ff, &out[0]));
        assert_eq!(std::fs::read(&tone).unwrap(), before, "original untouched");
        // a second run never overwrites
        let (out2, errors) = run_file_tool(std::slice::from_ref(&tone), &job, Some(ff));
        assert!(errors.is_empty(), "{errors:?}");
        assert_ne!(out2[0], out[0]);
        assert!(out2[0].file_stem().unwrap().to_string_lossy().ends_with(" 2"), "{:?}", out2[0]);
        assert!(out[0].is_file() && out2[0].is_file());
        // a still image: a generated 200x200 PNG on a square canvas
        let png = dir.join("cover.png");
        command(ff, &["-f", "lavfi", "-i", "testsrc=size=200x200:rate=1", "-frames:v", "1"], &png);
        let job = visualize_job(Canvas::Square, VisStyle::Still, Some(&png)).unwrap();
        let (out3, errors) = run_file_tool(std::slice::from_ref(&tone), &job, Some(ff));
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(video_size(ff, &out3[0]), Canvas::Square.size());
        assert!((duration_of(ff, &out3[0]) - 3.0).abs() < 0.3, "{}", duration_of(ff, &out3[0]));
        assert_eq!(std::fs::read(&tone).unwrap(), before);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
