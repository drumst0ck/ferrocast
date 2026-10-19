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
