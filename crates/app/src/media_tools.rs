//! Multimedia tool execution. Work is staged privately and published without replacing files.
use ferrocast_core::compress::{self, Goal, Plan, Resize};
use ferrocast_core::convert::{family_of, output_with_suffix, Family, FileTool};
use ferrocast_core::media;
use ferrocast_core::videoplan::MIN_PIECE;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static SEQ: AtomicU64 = AtomicU64::new(0);

fn probe(ffmpeg: &str, file: &Path) -> Result<serde_json::Value, String> {
    serde_json::from_slice(&probe_raw(ffmpeg, file, &["-show_streams", "-show_format"])?).map_err(|e| e.to_string())
}

/// ffprobe JSON bytes for `file` with the given `-show_*` flags: 10 s deadline, 4 MiB cap.
fn probe_raw(ffmpeg: &str, file: &Path, show: &[&str]) -> Result<Vec<u8>, String> {
    let ffprobe = Path::new(ffmpeg).with_file_name("ffprobe");
    let mut child = Command::new(ffprobe)
        .args(["-v", "quiet"])
        .args(show)
        .args(["-of", "json"])
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
                return Ok(data);
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

/// Tags of an audio, video or GIF file grouped by file, track and chapter (one ffprobe under its
/// own deadline; never call it on the UI thread).
#[allow(dead_code)] // consumed by the Edit metadata window (A7d)
pub fn inspect_metadata(ffmpeg: &str, path: &Path) -> Result<Vec<ferrocast_core::metaform::Field>, String> {
    let raw = probe_raw(ffmpeg, path, &["-show_format", "-show_streams", "-show_chapters"])?;
    ferrocast_core::metaform::parse_ffprobe(&String::from_utf8_lossy(&raw))
}

/// Edits of a PDF: only file-level fields named like PDFKit's attributes (Title, Author, Subject,
/// Keywords, Creator, Producer) exist there.
pub(crate) fn edit_pdf_metadata(file: &Path, edits: &[ferrocast_core::metaform::MetaEdit], remove_all: bool) -> Result<PathBuf, String> {
    use ferrocast_core::metaform::Group;
    if let Some(e) = edits.iter().find(|e| e.group != Group::File) {
        return Err(format!("a PDF has no tracks or chapters to edit ({})", e.key));
    }
    let pairs: Vec<_> = edits.iter().map(|e| (e.key.clone(), e.value.clone())).collect();
    if pairs.is_empty() && !remove_all {
        return Err("nothing to change".into());
    }
    ferrocast_macos::pdf::set_metadata(file, &pairs, remove_all)
}

/// Rewrites the tags of one audio, video or GIF file with a stream copy, next to the original and
/// with the same extension ("<name> edited.<ext>", " 2" on a collision). Staged privately and
/// published with `create_new`, so nothing is overwritten and a failure leaves nothing behind.
fn edit_metadata_one(ff: &str, file: &Path, edits: &[ferrocast_core::metaform::MetaEdit], remove_all: bool) -> Result<PathBuf, String> {
    let is_gif = ferrocast_core::convert::ext(file) == "gif";
    if !is_gif && !matches!(family_of(file), Family::Audio | Family::Video) {
        return Err("this tool requires audio, video or GIF files".into());
    }
    ferrocast_core::metaform::validate(edits)?;
    if edits.is_empty() && !remove_all {
        return Err("nothing to change".into());
    }
    if is_gif && !remove_all {
        return Err("GIF files cannot store tags".into());
    }
    let ext = file.extension().and_then(|e| e.to_str()).ok_or("the file has no extension")?.to_string();
    let input = file.to_str().ok_or("input path is not UTF-8")?;
    let scratch = std::env::temp_dir().join(format!("ferrocast-media-{}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed)));
    std::fs::create_dir(&scratch).map_err(|e| e.to_string())?;
    let result = (|| {
        let staged = scratch.join(format!("output.{ext}"));
        let out = staged.to_str().ok_or("output path is not UTF-8")?;
        let args = ferrocast_core::metaform::ffmpeg_args(input, edits, remove_all, out);
        crate::route::run_limited(Command::new(ff).args(["-nostdin", "-n", "-loglevel", "error"]).args(args), 300)?;
        if std::fs::metadata(&staged).map_err(|e| e.to_string())?.len() == 0 {
            return Err("tool produced an empty file".to_string());
        }
        let suffix = FileTool::EditMetadata(vec![], remove_all).suffix();
        let dest = output_with_suffix(file, &suffix, &ext, |p| p.exists());
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
}

fn streams(info: &serde_json::Value) -> impl Iterator<Item = &serde_json::Value> {
    info["streams"].as_array().into_iter().flatten()
}

/// Duration (s) and frame rate of a video, from ffprobe under its own deadline.
pub(crate) fn video_info(ff: &str, file: &Path) -> Option<(f64, f64)> {
    let info = probe(ff, file).ok()?;
    let duration = info["format"]["duration"].as_str().and_then(|d| d.parse::<f64>().ok())?;
    let rate =
        streams(&info).find(|s| s["codec_type"] == "video")?["r_frame_rate"].as_str().and_then(ferrocast_core::videoplan::parse_rate)?;
    Some((duration, rate))
}

/// Size of a video stream as it is displayed: width and height swap when the stream carries a
/// quarter-turn rotation (side data or the legacy `rotate` tag), because ffmpeg autorotates.
fn display_dims(v: &serde_json::Value) -> Option<(u32, u32)> {
    let (w, h) = (v["width"].as_u64()?, v["height"].as_u64()?);
    let rot = v["side_data_list"]
        .as_array()
        .into_iter()
        .flatten()
        .find_map(|d| d["rotation"].as_f64())
        .or_else(|| v["tags"]["rotate"].as_str().and_then(|r| r.parse::<f64>().ok()))
        .unwrap_or(0.0);
    let quarter = (rot.rem_euclid(180.0) - 90.0).abs() < 1.0;
    let (w, h) = if quarter { (h, w) } else { (w, h) };
    (w >= 2 && h >= 2 && w <= 16384 && h <= 16384).then_some((w as u32, h as u32))
}

/// Displayed pixel size of the first video stream.
pub(crate) fn video_dims(ff: &str, file: &Path) -> Option<(u32, u32)> {
    let info = probe(ff, file).ok()?;
    let dims = display_dims(streams(&info).find(|s| s["codec_type"] == "video")?);
    dims
}

/// Duration, audio presence and displayed size of an audio or video file (one ffprobe under its
/// own deadline; never call it on the UI thread).
pub(crate) fn media_info(ff: &str, file: &Path) -> Option<ferrocast_core::compress::MediaInfo> {
    let info = probe(ff, file).ok()?;
    let duration = info["format"]["duration"].as_str().and_then(|d| d.parse::<f64>().ok()).filter(|d| d.is_finite() && *d > 0.0)?;
    let has_audio = streams(&info).any(|s| s["codec_type"] == "audio");
    let dims = streams(&info).find(|s| s["codec_type"] == "video").and_then(display_dims);
    Some(ferrocast_core::compress::MediaInfo { duration, has_audio, dims })
}

/// PNG of the frame at `t` seconds, at most 800 px wide, or None (bounded to 20 s).
pub(crate) fn preview_frame(ff: &str, file: &Path, t: f64) -> Option<Vec<u8>> {
    if !t.is_finite() || t < 0.0 {
        return None;
    }
    let dir = std::env::temp_dir().join(format!("ferrocast-vframe-{}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed)));
    std::fs::create_dir(&dir).ok()?;
    let out = dir.join("f.png");
    let ran = crate::route::run_limited(
        Command::new(ff)
            .args(["-nostdin", "-y", "-loglevel", "error", "-ss", &t.to_string(), "-i"])
            .arg(file)
            .args(["-frames:v", "1", "-vf", "scale='min(800,iw)':-2"])
            .arg(&out),
        20,
    );
    let bytes = ran.ok().and_then(|_| std::fs::read(&out).ok()).filter(|b| !b.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
    bytes
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

/// Checks a split before anything is written: at least two ranges, each finite, starting at or
/// after 0, at least [`MIN_PIECE`] long and ending no later than `duration` (plus 0.05 s slack).
pub(crate) fn validate_split(ranges: &[(f64, f64)], duration: f64) -> Result<(), String> {
    if !duration.is_finite() || duration <= 0.0 {
        return Err("could not read the media duration".into());
    }
    if ranges.len() < 2 {
        return Err("a split needs at least two ranges (one range is just a trim)".into());
    }
    for (i, &(a, b)) in ranges.iter().enumerate() {
        let n = i + 1;
        if !a.is_finite() || !b.is_finite() || a < 0.0 || b <= a {
            return Err(format!("range {n} is not valid (start must be at least 0 and before the end)"));
        }
        if b - a < MIN_PIECE {
            return Err(format!("range {n} is shorter than {MIN_PIECE} s"));
        }
        if b > duration + 0.05 {
            return Err(format!("range {n} ends after the end of the media ({duration:.2} s)"));
        }
    }
    Ok(())
}

/// "<stem> split", then "<stem> split 2", "<stem> split 3"... next to the source.
pub(crate) fn split_folder(source: &Path, exists: impl Fn(&Path) -> bool) -> PathBuf {
    let dir = source.parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = source.file_stem().and_then(|s| s.to_str()).unwrap_or("media");
    (1u32..)
        .map(|n| dir.join(if n == 1 { format!("{stem} split") } else { format!("{stem} split {n}") }))
        .find(|p| !exists(p))
        .expect("an unused name exists")
}

/// Cuts one audio or video file into the ranges, all inside one new folder. The folder is made
/// with `create_dir`, so an existing one is never reused; on any failure it is removed again.
fn split_one(ff: &str, file: &Path, ranges: &[(f64, f64)], exact: bool) -> Result<PathBuf, String> {
    let family = family_of(file);
    if !matches!(family, Family::Audio | Family::Video) {
        return Err("this tool requires audio or video files".into());
    }
    let info = probe(ff, file)?;
    let duration = info["format"]["duration"].as_str().and_then(|d| d.parse::<f64>().ok()).ok_or("could not read the media duration")?;
    validate_split(ranges, duration)?;
    let input = file.to_str().ok_or("input path is not UTF-8")?;
    let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("media").to_string();
    let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("mp4").to_string();
    // Re-encoding with x264/aac is for video; audio files are stream-copied (their cuts are not frame-bound).
    let exact = exact && family == Family::Video;
    let folder = loop {
        let candidate = split_folder(file, |p| p.exists());
        match std::fs::create_dir(&candidate) {
            Ok(()) => break candidate,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    };
    let result = (|| {
        for (i, &(a, b)) in ranges.iter().enumerate() {
            let clip = folder.join(format!("{stem} {}.{ext}", i + 1));
            let out = clip.to_str().ok_or("output path is not UTF-8")?;
            let args = media::trim(input, a, b, exact, out);
            crate::route::run_limited(Command::new(ff).args(["-nostdin", "-n", "-loglevel", "error"]).args(args), 180)?;
            if std::fs::metadata(&clip).map_err(|e| e.to_string())?.len() == 0 {
                return Err(format!("clip {} came out empty", i + 1));
            }
        }
        Ok(())
    })();
    match result {
        Ok(()) => Ok(folder),
        Err(e) => {
            let _ = std::fs::remove_dir_all(&folder);
            Err(e)
        }
    }
}

/// Compresses one audio or video file next to itself. The plan is made (and a hopeless target
/// refused) before anything is written; the copy is staged privately and published with
/// `create_new`, so nothing is ever overwritten and a failure leaves nothing behind.
fn compress_one(ff: &str, file: &Path, goal: Goal, resize: &Resize) -> Result<PathBuf, String> {
    let video = match family_of(file) {
        Family::Video => true,
        Family::Audio => false,
        _ => return Err("this tool requires audio or video files".into()),
    };
    let info = media_info(ff, file).ok_or("could not read the media")?;
    let orig = std::fs::metadata(file).map_err(|e| e.to_string())?.len();
    let plan = compress::plan(video, orig, &info, goal, resize)?;
    let output_ext = compress::output_ext(video, &ferrocast_core::convert::ext(file));
    let input = file.to_str().ok_or("input path is not UTF-8")?;
    let scratch = std::env::temp_dir().join(format!("ferrocast-media-{}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed)));
    std::fs::create_dir(&scratch).map_err(|e| e.to_string())?;
    let result = (|| {
        let staged = scratch.join(format!("output.{output_ext}"));
        let out = staged.to_str().ok_or("output path is not UTF-8")?;
        let args = match plan {
            Plan::Audio { kbps } => media::compress_audio(input, out, kbps),
            Plan::Video { rate, audio_kbps, scale } => media::compress_video(input, out, rate, audio_kbps, scale),
        };
        crate::route::run_limited(Command::new(ff).args(["-nostdin", "-n", "-loglevel", "error"]).args(args), 900)?;
        if std::fs::metadata(&staged).map_err(|e| e.to_string())?.len() == 0 {
            return Err("compression produced an empty file".to_string());
        }
        let dest = output_with_suffix(file, &FileTool::Compress(goal, *resize).suffix(), output_ext, |p| p.exists());
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
        if let FileTool::SplitPoints(ranges, exact) = tool {
            match split_one(ff, file, ranges, *exact) {
                Ok(p) => made.push(p),
                Err(e) => errors.push(format!("{}: {e}", file.display())),
            }
            continue;
        }
        if let FileTool::EditMetadata(edits, remove_all) = tool {
            match edit_metadata_one(ff, file, edits, *remove_all) {
                Ok(p) => made.push(p),
                Err(e) => errors.push(format!("{}: {e}", file.display())),
            }
            continue;
        }
        if let FileTool::Compress(goal, resize) = tool {
            match compress_one(ff, file, *goal, resize) {
                Ok(p) => made.push(p),
                Err(e) => errors.push(format!("{}: {e}", file.display())),
            }
            continue;
        }
        let result = (|| -> Result<PathBuf, String> {
            let video = matches!(
                tool,
                FileTool::Mute
                    | FileTool::TrimExact(..)
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
            let (width, height) = v.and_then(display_dims).map_or((0, 0), |(w, h)| (u64::from(w), u64::from(h)));
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
                    FileTool::TrimExact(a, b) if finite_range(a, b) => media::trim(input, a, b, true, out),
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
    #[test]
    fn rotated_streams_swap_their_dimensions() {
        let v = |j: &str| serde_json::from_str::<serde_json::Value>(j).unwrap();
        assert_eq!(display_dims(&v(r#"{"width":320,"height":180}"#)), Some((320, 180)));
        assert_eq!(display_dims(&v(r#"{"width":320,"height":180,"side_data_list":[{"rotation":-90}]}"#)), Some((180, 320)));
        assert_eq!(display_dims(&v(r#"{"width":320,"height":180,"side_data_list":[{"rotation":90}]}"#)), Some((180, 320)));
        assert_eq!(display_dims(&v(r#"{"width":320,"height":180,"tags":{"rotate":"270"}}"#)), Some((180, 320)));
        assert_eq!(display_dims(&v(r#"{"width":320,"height":180,"side_data_list":[{"rotation":180}]}"#)), Some((320, 180)));
        assert_eq!(display_dims(&v(r#"{"width":1}"#)), None);
    }

    use super::{display_dims, parse_integrated};

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

#[cfg(test)]
mod split_tests {
    use super::{split_folder, validate_split};

    #[test]
    fn rejects_bad_ranges() {
        let ok = vec![(0.0, 3.0), (3.0, 10.0)];
        assert!(validate_split(&ok, 10.0).is_ok());
        assert!(validate_split(&[(0.0, 3.0), (2.0, 1.0)], 10.0).unwrap_err().contains("range"));
        assert!(validate_split(&[(0.0, 3.0), (f64::NAN, 5.0)], 10.0).is_err());
        assert!(validate_split(&[(0.0, 3.0), (3.0, f64::INFINITY)], 10.0).is_err());
        assert!(validate_split(&[(-1.0, 3.0), (3.0, 5.0)], 10.0).is_err());
        assert!(validate_split(&[(0.0, 3.0), (3.0, 3.01)], 10.0).is_err(), "piece under the minimum");
        assert!(validate_split(&[(0.0, 3.0)], 10.0).unwrap_err().contains("at least two"));
        assert!(validate_split(&[], 10.0).is_err());
        assert!(validate_split(&[(0.0, 3.0), (3.0, 10.2)], 10.0).is_err(), "past the duration");
        assert!(validate_split(&[(0.0, 3.0), (3.0, 10.04)], 10.0).is_ok(), "tolerance of 0.05 s");
        assert!(validate_split(&ok, f64::NAN).is_err());
    }

    #[test]
    fn folder_names_never_collide() {
        let dir = std::env::temp_dir().join(format!("fc-split-name-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let src = dir.join("clip.mp4");
        assert_eq!(split_folder(&src, |p| p.exists()), dir.join("clip split"));
        std::fs::create_dir(dir.join("clip split")).unwrap();
        assert_eq!(split_folder(&src, |p| p.exists()), dir.join("clip split 2"));
        std::fs::write(dir.join("clip split 2"), b"a file").unwrap();
        assert_eq!(split_folder(&src, |p| p.exists()), dir.join("clip split 3"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
