//! Multimedia tool execution. Work is staged privately and published without replacing files.
use ferrocast_core::convert::{family_of, output_with_suffix, Family, FileTool};
use ferrocast_core::media;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static SEQ: AtomicU64 = AtomicU64::new(0);

fn probe(ffmpeg: &str, file: &Path) -> Result<serde_json::Value, String> {
    let ffprobe = Path::new(ffmpeg).with_file_name("ffprobe");
    let mut child = Command::new(ffprobe)
        .args(["-v", "quiet", "-show_streams", "-show_format", "-of", "json"])
        .arg(file)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    // The bounded stream metadata is drained concurrently, so large tag blocks cannot fill a pipe.
    let stdout = child.stdout.take().ok_or("missing probe output")?;
    let reader = std::thread::spawn(move || {
        use std::io::Read;
        let mut bytes = Vec::new();
        stdout.take(4 * 1024 * 1024).read_to_end(&mut bytes).map(|_| bytes)
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let data = reader.join().map_err(|_| "probe reader failed")?.map_err(|e| e.to_string())?;
                if !status.success() {
                    return Err("could not inspect media".into());
                }
                return serde_json::from_slice(&data).map_err(|e| e.to_string());
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err("media inspection timed out or failed".into());
            }
        }
    }
}

fn streams(info: &serde_json::Value) -> impl Iterator<Item = &serde_json::Value> {
    info["streams"].as_array().into_iter().flatten()
}

/// Parse the integrated loudness (LUFS) from the summary ffmpeg's `ebur128` filter prints last.
pub(crate) fn parse_integrated(stderr: &str) -> Option<f64> {
    let summary = stderr.rsplit("Integrated loudness:").next().filter(|_| stderr.contains("Integrated loudness:"))?;
    let line = summary.lines().find(|l| l.trim_start().starts_with("I:"))?;
    line.trim_start().strip_prefix("I:")?.split_whitespace().next()?.parse().ok()
}

/// Integrated loudness of a file in LUFS, measured with a hard deadline.
pub(crate) fn loudness(ff: &str, file: &Path) -> Option<f64> {
    let mut child = Command::new(ff)
        .args(["-nostdin", "-hide_banner", "-i"])
        .arg(file)
        .args(["-af", "ebur128", "-f", "null", "-"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let mut err = child.stderr.take()?;
    let reader = std::thread::spawn(move || {
        use std::io::Read;
        let mut text = String::new();
        let mut buf = Vec::new();
        let _ = (&mut err).take(8 << 20).read_to_end(&mut buf);
        text.push_str(&String::from_utf8_lossy(&buf));
        text
    });
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break;
            }
        }
    }
    parse_integrated(&reader.join().ok()?)
}

/// "-31.0 → -16.0 LUFS" for a normalize result, when both measurements work.
pub(crate) fn normalize_report(ff: &str, before: &Path, after: &Path) -> Option<String> {
    Some(format!("{:.1} → {:.1} LUFS", loudness(ff, before)?, loudness(ff, after)?))
}

pub(crate) fn run(files: &[PathBuf], tool: &FileTool, ffmpeg: Option<&str>) -> (Vec<PathBuf>, Vec<String>) {
    let Some(ff) = ffmpeg else {
        return (vec![], vec!["ffmpeg is not installed (brew install ffmpeg)".into()]);
    };
    let mut made = vec![];
    let mut errors = vec![];
    for (index, file) in files.iter().enumerate() {
        if matches!(tool, FileTool::Join) && index > 0 {
            break;
        }
        let result = (|| -> Result<PathBuf, String> {
            let video = matches!(
                tool,
                FileTool::Mute
                    | FileTool::Speed(_)
                    | FileTool::Snapshot(_)
                    | FileTool::CropVideo(..)
                    | FileTool::RedactVideo(..)
                    | FileTool::Join
            );
            let expected = if video { Family::Video } else { Family::Audio };
            if family_of(file) != expected || (matches!(tool, FileTool::Join) && files.iter().any(|p| family_of(p) != Family::Video)) {
                return Err(if video { "this tool requires video files" } else { "this tool requires audio files" }.into());
            }
            let info = probe(ff, file)?;
            let has_audio = streams(&info).any(|s| s["codec_type"] == "audio");
            let in_channels = streams(&info).find(|s| s["codec_type"] == "audio").and_then(|s| s["channels"].as_u64()).unwrap_or(2) as u32;
            let v = streams(&info).find(|s| s["codec_type"] == "video");
            let (width, height) = (v.and_then(|s| s["width"].as_u64()).unwrap_or(0), v.and_then(|s| s["height"].as_u64()).unwrap_or(0));
            let output_ext = if matches!(tool, FileTool::Snapshot(_)) {
                "png"
            } else if video || matches!(tool, FileTool::Visualize(_) | FileTool::VisualizeImage(..)) {
                "mp4"
            } else {
                "wav"
            };
            let scratch =
                std::env::temp_dir().join(format!("ferrocast-media-{}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed)));
            std::fs::create_dir(&scratch).map_err(|e| e.to_string())?;
            let result = (|| {
                let staged = scratch.join(format!("output.{output_ext}"));
                let input = file.to_str().ok_or("input path is not UTF-8")?;
                let out = staged.to_str().ok_or("output path is not UTF-8")?;
                let finite_range = |a: f64, b: f64| a.is_finite() && b.is_finite() && a >= 0.0 && b > a;
                let args = match *tool {
                    FileTool::Mute => media::mute(input, out),
                    FileTool::Speed(f) if f.is_finite() && (0.25..=4.0).contains(&f) => media::speed(input, f, has_audio, out),
                    FileTool::Snapshot(t) if t.is_finite() && t >= 0.0 => media::snapshot(input, t, out),
                    FileTool::CropVideo(x, y, w, h)
                        if w >= 2 && h >= 2 && u64::from(x) + u64::from(w) <= width && u64::from(y) + u64::from(h) <= height =>
                    {
                        media::crop_video(input, x, y, w, h, out)
                    }
                    FileTool::RedactVideo(x, y, w, h, a, b, blur)
                        if finite_range(a, b)
                            && w >= 2
                            && h >= 2
                            && u64::from(x) + u64::from(w) <= width
                            && u64::from(y) + u64::from(h) <= height =>
                    {
                        media::redact_video(input, (x, y, w, h), (a, b), blur, out)
                    }
                    FileTool::Normalize(i, lra, tp)
                        if i.is_finite()
                            && lra.is_finite()
                            && tp.is_finite()
                            && (-70.0..=-5.0).contains(&i)
                            && (1.0..=50.0).contains(&lra)
                            && (-9.0..=0.0).contains(&tp) =>
                    {
                        media::normalize(input, i, lra, tp, out)
                    }
                    FileTool::Visualize(canvas) => media::visualizer(input, None, canvas, out),
                    FileTool::VisualizeImage(canvas, ref img) => {
                        let p = Path::new(img.as_str());
                        if !p.is_file()
                            || !matches!(ferrocast_core::convert::ext(p).as_str(), "png" | "jpg" | "jpeg" | "webp" | "bmp" | "tiff" | "tif")
                        {
                            return Err(format!("{img} is not a readable image (png, jpg, webp, bmp or tiff)"));
                        }
                        media::visualizer(input, Some(img.as_str()), canvas, out)
                    }
                    FileTool::TrimSilence => media::trim_audio(input, None, out),
                    FileTool::Channels(stereo, l, r)
                        if l.is_finite() && r.is_finite() && (0.0..=4.0).contains(&l) && (0.0..=4.0).contains(&r) =>
                    {
                        media::channels(input, in_channels, stereo, l, r, out)
                    }
                    FileTool::Bleep(a, b) if finite_range(a, b) => media::bleep(input, a, b, out),
                    FileTool::Join if files.len() >= 2 && width >= 2 && height >= 2 => {
                        // Silent clips get generated silence of the same length inside the filter graph.
                        let (mut has, mut durs) = (vec![], vec![]);
                        for f in files {
                            let i = probe(ff, f)?;
                            has.push(streams(&i).any(|s| s["codec_type"] == "audio"));
                            let d =
                                i["format"]["duration"].as_str().and_then(|d| d.parse::<f64>().ok()).filter(|d| d.is_finite() && *d > 0.0);
                            durs.push(d.ok_or("could not read a clip's duration")?);
                        }
                        media::join(
                            &files.iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>(),
                            &has,
                            &durs,
                            width as u32,
                            height as u32,
                            out,
                        )
                    }
                    _ => return Err("invalid parameters for this media tool".into()),
                };
                crate::route::run_limited(Command::new(ff).args(["-nostdin", "-n", "-loglevel", "error"]).args(args), 180)?;
                if std::fs::metadata(&staged).map_err(|e| e.to_string())?.len() == 0 {
                    return Err("tool produced an empty file".into());
                }
                let dest = output_with_suffix(file, &tool.suffix(), output_ext, |p| p.exists());
                // create_new closes the naming race; a concurrent file is never overwritten.
                use std::io::Write;
                let mut src = std::fs::File::open(&staged).map_err(|e| e.to_string())?;
                let mut dst = std::fs::OpenOptions::new().write(true).create_new(true).open(&dest).map_err(|e| e.to_string())?;
                if let Err(e) = std::io::copy(&mut src, &mut dst).and_then(|_| dst.flush()) {
                    let _ = std::fs::remove_file(&dest);
                    return Err(e.to_string());
                }
                Ok(dest)
            })();
            let _ = std::fs::remove_dir_all(scratch);
            result
        })();
        match result {
            Ok(p) => made.push(p),
            Err(e) => errors.push(format!("{}: {e}", file.display())),
        }
    }
    (made, errors)
}

#[cfg(test)]
mod loudness_tests {
    use super::parse_integrated;

    #[test]
    fn reads_the_summary_not_the_running_values() {
        let log = "[Parsed_ebur128_0 @ 0x1] t: 0.4 M: -20.1 S: -120.7 I: -20.1 LUFS LRA: 0.0 LU\n\
            [Parsed_ebur128_0 @ 0x1] Summary:\n\n  Integrated loudness:\n    I:         -31.4 LUFS\n    Threshold: -41.4 LUFS\n\n  Loudness range:\n    LRA:         0.0 LU\n";
        let log = log.replace("\\n", "\n");
        assert_eq!(parse_integrated(&log), Some(-31.4));
        assert_eq!(parse_integrated("no summary here"), None);
        assert_eq!(parse_integrated("Integrated loudness:\n    I:  abc LUFS"), None);
    }
}
