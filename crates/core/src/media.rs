//! Video and audio tools (Tangerine S11) as ffmpeg argument lists. Pure: no process is run here.
//! Every builder returns the arguments between `ffmpeg -y -loglevel error` and nothing else;
//! input(s) and the output path are included so the caller only prepends the binary.

/// Visualizer canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Canvas {
    Landscape,
    Portrait,
    Square,
}

impl Canvas {
    pub fn size(self) -> (u32, u32) {
        match self {
            Canvas::Landscape => (1280, 720),
            Canvas::Portrait => (720, 1280),
            Canvas::Square => (1080, 1080),
        }
    }
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

/// Keep `start`..`end` seconds; stream copy (no re-encode) when `exact` is false, otherwise
/// re-encode for frame-accurate cuts.
pub fn trim(input: &str, start: f64, end: f64, exact: bool, out: &str) -> Vec<String> {
    let mut v = s(&["-ss", &start.to_string(), "-to", &end.to_string(), "-i", input]);
    if exact {
        v.extend(s(&["-c:v", "libx264", "-c:a", "aac"]));
    } else {
        v.extend(s(&["-c", "copy"]));
    }
    v.push(out.into());
    v
}

/// Crop to `w`×`h` at (`x`,`y`), keeping the original audio.
pub fn crop_video(input: &str, x: u32, y: u32, w: u32, h: u32, out: &str) -> Vec<String> {
    let (w, h) = (w & !1, h & !1);
    s(&["-i", input, "-vf", &format!("crop={w}:{h}:{x}:{y}"), "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "copy", out])
}

/// Play at `factor`× (0.25..=4) and keep the pitch; silent videos work too (audio filter is
/// only attached when `has_audio`).
pub fn speed(input: &str, factor: f64, has_audio: bool, out: &str) -> Vec<String> {
    let f = factor.clamp(0.25, 4.0);
    let mut v = s(&["-i", input, "-filter:v", &format!("setpts={:.4}*PTS", 1.0 / f)]);
    if has_audio {
        // atempo accepts 0.5..=2 per stage: chain stages for larger changes
        let mut stages = vec![];
        let mut rest = f;
        while rest > 2.0 {
            stages.push("atempo=2.0".to_string());
            rest /= 2.0;
        }
        while rest < 0.5 {
            stages.push("atempo=0.5".to_string());
            rest /= 0.5;
        }
        stages.push(format!("atempo={rest:.4}"));
        v.extend(s(&["-filter:a", &stages.join(",")]));
    } else {
        v.push("-an".into());
    }
    v.extend(s(&["-c:v", "libx264", "-pix_fmt", "yuv420p", out]));
    v
}

/// Join clips into one MP4 using the first clip as the canvas (others are scaled and padded).
/// Clips with `has_audio[i] == false` get `durations[i]` seconds of generated stereo silence so
/// `concat` always sees one audio stream per clip.
pub fn join(inputs: &[String], has_audio: &[bool], durations: &[f64], width: u32, height: u32, out: &str) -> Vec<String> {
    let mut v = vec![];
    for i in inputs {
        v.extend(s(&["-i", i]));
    }
    let n = inputs.len();
    let mut fc = String::new();
    for i in 0..n {
        fc.push_str(&format!("[{i}:v]scale={width}:{height}:force_original_aspect_ratio=decrease,pad={width}:{height}:(ow-iw)/2:(oh-ih)/2,setsar=1,fps=30[v{i}];"));
        if has_audio.get(i).copied().unwrap_or(true) {
            fc.push_str(&format!("[{i}:a]aresample=48000,aformat=channel_layouts=stereo[a{i}];"));
        } else {
            let d = durations.get(i).copied().unwrap_or(0.0).max(0.0);
            fc.push_str(&format!("anullsrc=r=48000:cl=stereo,atrim=duration={d}[a{i}];"));
        }
    }
    for i in 0..n {
        fc.push_str(&format!("[v{i}][a{i}]"));
    }
    fc.push_str(&format!("concat=n={n}:v=1:a=1[v][a]"));
    v.extend(s(&[
        "-filter_complex",
        &fc,
        "-map",
        "[v]",
        "-map",
        "[a]",
        "-c:v",
        "libx264",
        "-pix_fmt",
        "yuv420p",
        "-c:a",
        "aac",
        "-movflags",
        "+faststart",
        out,
    ]));
    v
}

/// One full-resolution PNG frame at `at` seconds.
pub fn snapshot(input: &str, at: f64, out: &str) -> Vec<String> {
    s(&["-ss", &at.to_string(), "-i", input, "-frames:v", "1", out])
}

/// Drop the audio track.
pub fn mute(input: &str, out: &str) -> Vec<String> {
    s(&["-i", input, "-c:v", "copy", "-an", out])
}

/// Cover a region for `from`..`to` seconds with a solid box (or a pixelated blur when `blur`).
pub fn redact_video(input: &str, region: (u32, u32, u32, u32), range: (f64, f64), blur: bool, out: &str) -> Vec<String> {
    let (x, y, w, h) = region;
    let (from, to) = range;
    let en = format!("between(t\\,{from}\\,{to})");
    let vf = if blur {
        format!("[0:v]crop={w}:{h}:{x}:{y},boxblur=luma_radius=min(20\\,min(w\\,h)/2):luma_power=5:chroma_radius=min(10\\,min(cw\\,ch)/2)[b];[0:v][b]overlay={x}:{y}:enable='{en}'")
    } else {
        format!("drawbox=x={x}:y={y}:w={w}:h={h}:color=black@1:t=fill:enable='{en}'")
    };
    if blur {
        s(&["-i", input, "-filter_complex", &vf, "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "copy", out])
    } else {
        s(&["-i", input, "-vf", &vf, "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "copy", out])
    }
}

/// Video rate control for [`compress_video`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoRate {
    /// Average video bitrate in kbit/s (a size target).
    Kbps(u32),
    /// Constant quality (x264 CRF; lower is better).
    Crf(u8),
}

/// Audio encoder for a compressed file, chosen from the OUTPUT extension. Only encoders this
/// crate already uses elsewhere; anything else falls back to aac.
pub fn compress_audio_encoder(out: &str) -> &'static str {
    match out.rsplit('.').next().unwrap_or("").to_ascii_lowercase().as_str() {
        "mp3" => "libmp3lame",
        "ogg" | "opus" => "libopus",
        _ => "aac",
    }
}

/// Re-encode audio at `kbps` (the encoder follows the output extension). Cover art is dropped.
pub fn compress_audio(input: &str, out: &str, kbps: u32) -> Vec<String> {
    s(&["-i", input, "-vn", "-c:a", compress_audio_encoder(out), "-b:a", &format!("{kbps}k"), out])
}

/// Re-encode a video to H.264 MP4: `rate` (bitrate target or CRF), AAC audio at `audio_kbps` (or
/// no audio at all for a silent source), an optional even `scale` (w, h) and faststart.
pub fn compress_video(input: &str, out: &str, rate: VideoRate, audio_kbps: Option<u32>, scale: Option<(u32, u32)>) -> Vec<String> {
    let mut v = s(&["-i", input, "-map", "0:v:0"]);
    if audio_kbps.is_some() {
        v.extend(s(&["-map", "0:a:0"]));
    }
    if let Some((w, h)) = scale {
        v.extend(s(&["-vf", &format!("scale={w}:{h}")]));
    }
    v.extend(s(&["-c:v", "libx264", "-pix_fmt", "yuv420p"]));
    match rate {
        VideoRate::Kbps(k) => {
            v.extend(s(&["-b:v", &format!("{k}k"), "-maxrate", &format!("{}k", k * 3 / 2), "-bufsize", &format!("{}k", k * 2)]))
        }
        VideoRate::Crf(c) => v.extend(s(&["-crf", &c.to_string()])),
    }
    match audio_kbps {
        Some(a) => v.extend(s(&["-c:a", "aac", "-b:a", &format!("{a}k")])),
        None => v.push("-an".into()),
    }
    v.extend(s(&["-movflags", "+faststart", out]));
    v
}

/// EBU R128 loudness normalisation: target integrated loudness, loudness range and true peak.
pub fn normalize(input: &str, lufs: f64, lra: f64, tp: f64, out: &str) -> Vec<String> {
    s(&["-i", input, "-af", &format!("loudnorm=I={lufs}:LRA={lra}:TP={tp}"), "-ar", "48000", out])
}

/// Audio → MP4 with an animated waveform, or a still image when `image` is given.
pub fn visualizer(input: &str, image: Option<&str>, canvas: Canvas, out: &str) -> Vec<String> {
    let (w, h) = canvas.size();
    match image {
        Some(img) => s(&[
            "-loop",
            "1",
            "-i",
            img,
            "-i",
            input,
            "-vf",
            &format!("scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2,format=yuv420p"),
            "-c:v",
            "libx264",
            "-tune",
            "stillimage",
            "-c:a",
            "aac",
            "-shortest",
            "-movflags",
            "+faststart",
            out,
        ]),
        None => s(&[
            "-i",
            input,
            "-filter_complex",
            &format!("[0:a]showwaves=s={w}x{h}:mode=cline:colors=white,format=yuv420p[v]"),
            "-map",
            "[v]",
            "-map",
            "0:a",
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
            "-movflags",
            "+faststart",
            out,
        ]),
    }
}

/// Keep `start`..`end` of a recording, or strip silence at both ends when `start`/`end` are None.
pub fn trim_audio(input: &str, range: Option<(f64, f64)>, out: &str) -> Vec<String> {
    match range {
        Some((a, b)) => s(&["-ss", &a.to_string(), "-to", &b.to_string(), "-i", input, out]),
        // Trim the start, reverse, trim the (former) end, reverse back: interior pauses survive.
        None => s(&[
            "-i",
            input,
            "-af",
            "areverse,silenceremove=start_periods=1:start_threshold=-50dB,areverse,silenceremove=start_periods=1:start_threshold=-50dB",
            out,
        ]),
    }
}

/// Mono or stereo with independent channel gains (1.0 = unchanged). `in_channels` is the source
/// channel count: a mono source only has `c0`, so it feeds both outputs (or is scaled by the
/// average gain for mono output); sources with 2+ channels use `c0`/`c1`.
pub fn channels(input: &str, in_channels: u32, stereo: bool, left: f64, right: f64, out: &str) -> Vec<String> {
    let mono_src = in_channels <= 1;
    let af = match (stereo, mono_src) {
        (true, true) => format!("pan=stereo|c0={left}*c0|c1={right}*c0"),
        (true, false) => format!("pan=stereo|c0={left}*c0|c1={right}*c1"),
        (false, true) => format!("pan=mono|c0={}*c0", (left + right) / 2.0),
        (false, false) => format!("pan=mono|c0={}*c0+{}*c1", left / 2.0, right / 2.0),
    };
    s(&["-i", input, "-af", &af, out])
}

/// Replace `from`..`to` seconds with a 1 kHz bleep.
pub fn bleep(input: &str, from: f64, to: f64, out: &str) -> Vec<String> {
    let fc = format!(
        "[0:a]volume=0:enable='between(t\\,{from}\\,{to})'[a];sine=frequency=1000:duration={}[t];[t]adelay={}|{},volume=0.5[b];[a][b]amix=inputs=2:duration=first:normalize=0[out]",
        to - from,
        (from * 1000.0) as u64,
        (from * 1000.0) as u64
    );
    s(&["-i", input, "-filter_complex", &fc, "-map", "[out]", out])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_copy_or_exact() {
        assert_eq!(trim("a.mp4", 1.5, 4.0, false, "o.mp4"), s(&["-ss", "1.5", "-to", "4", "-i", "a.mp4", "-c", "copy", "o.mp4"]));
        assert!(trim("a.mp4", 1.0, 2.0, true, "o.mp4").contains(&"libx264".to_string()));
    }

    #[test]
    fn crop_forces_even_sizes_and_keeps_audio() {
        let v = crop_video("a.mp4", 10, 20, 301, 201, "o.mp4");
        assert!(v.contains(&"crop=300:200:10:20".to_string()));
        assert!(v.windows(2).any(|w| w == ["-c:a", "copy"]));
    }

    #[test]
    fn speed_chains_atempo_and_handles_silent() {
        let v = speed("a.mp4", 4.0, true, "o.mp4");
        assert!(
            v.contains(&"atempo=2.0,atempo=2.0000".to_string()) || v.contains(&"atempo=2.0,atempo=2.0,atempo=1.0000".to_string()),
            "{v:?}"
        );
        let v = speed("a.mp4", 0.25, true, "o.mp4");
        assert!(v.iter().any(|a| a.starts_with("atempo=0.5,atempo=0.5")));
        let v = speed("a.mp4", 2.0, false, "o.mp4");
        assert!(v.contains(&"-an".to_string()) && !v.contains(&"-filter:a".to_string()));
        assert!(speed("a", 99.0, false, "o").contains(&"setpts=0.2500*PTS".to_string()), "clamped to 4x");
    }

    #[test]
    fn join_concats_all_clips_on_first_canvas_with_generated_silence() {
        let v = join(&["a.mp4".into(), "b.mov".into(), "c.mkv".into()], &[true, false, true], &[1.0, 2.5, 3.0], 1920, 1080, "o.mp4");
        let fc = v.iter().find(|a| a.contains("concat=")).unwrap();
        assert!(fc.contains("concat=n=3:v=1:a=1"));
        assert!(fc.contains("scale=1920:1080"));
        assert_eq!(v.iter().filter(|a| *a == "-i").count(), 3);
        assert!(fc.contains("[0:a]aresample=48000") && fc.contains("[2:a]aresample=48000"));
        assert!(fc.contains("anullsrc=r=48000:cl=stereo,atrim=duration=2.5[a1]"), "{fc}");
        assert!(!fc.contains("[1:a]"), "silent clip has no audio stream to reference");
    }

    #[test]
    fn trim_audio_strips_only_edges() {
        let v = trim_audio("a", None, "o");
        let af = &v[v.iter().position(|a| a == "-af").unwrap() + 1];
        assert!(af.starts_with("areverse,silenceremove=start_periods=1"), "{af}");
        assert_eq!(af.matches("areverse").count(), 2);
        assert!(af.ends_with("areverse") || af.contains("areverse,silenceremove"), "{af}");
        assert!(!af.contains("stop_periods"), "{af}");
        assert_eq!(trim_audio("a", Some((1.0, 2.0)), "o"), s(&["-ss", "1", "-to", "2", "-i", "a", "o"]));
    }

    #[test]
    fn channels_use_explicit_input_layout() {
        let af = |v: Vec<String>| v[v.iter().position(|a| a == "-af").unwrap() + 1].clone();
        assert_eq!(af(channels("a", 1, true, 1.0, 0.5, "o")), "pan=stereo|c0=1*c0|c1=0.5*c0");
        assert_eq!(af(channels("a", 2, true, 0.5, 1.0, "o")), "pan=stereo|c0=0.5*c0|c1=1*c1");
        assert_eq!(af(channels("a", 1, false, 1.0, 0.5, "o")), "pan=mono|c0=0.75*c0");
        assert_eq!(af(channels("a", 2, false, 1.0, 1.0, "o")), "pan=mono|c0=0.5*c0+0.5*c1");
    }

    #[test]
    fn redact_bleep_channels_visualizer_shapes() {
        assert!(redact_video("a", (0, 0, 10, 10), (1.0, 2.0), false, "o")
            .iter()
            .any(|a| a.contains("drawbox") && a.contains("between(t\\,1\\,2)")));
        assert!(redact_video("a", (0, 0, 10, 10), (1.0, 2.0), true, "o").iter().any(|a| a.contains("boxblur")));
        assert!(bleep("a", 2.0, 3.5, "o").iter().any(|a| a.contains("duration=1.5") && a.contains("adelay=2000|2000")));
        assert!(visualizer("a.mp3", None, Canvas::Square, "o.mp4").iter().any(|a| a.contains("showwaves=s=1080x1080")));
        assert!(visualizer("a.mp3", Some("i.png"), Canvas::Portrait, "o.mp4").iter().any(|a| a.contains("pad=720:1280")));
        assert_eq!(snapshot("a", 3.25, "f.png"), s(&["-ss", "3.25", "-i", "a", "-frames:v", "1", "f.png"]));
        assert!(trim_audio("a", None, "o").iter().any(|a| a.contains("silenceremove")));
        assert_eq!(normalize("a", -16.0, 11.0, -1.5, "o")[3], "loudnorm=I=-16:LRA=11:TP=-1.5");
        assert!(
            normalize("a", -16.0, 11.0, -1.5, "o").windows(2).any(|w| w == ["-ar", "48000"]),
            "loudnorm upsamples to 192 kHz otherwise"
        );
    }

    // ---- real ffmpeg tests (cargo test -p ferrocast-core media -- --ignored) ----
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    const FFMPEG: &str = "/opt/homebrew/bin/ffmpeg";
    const FFPROBE: &str = "/opt/homebrew/bin/ffprobe";

    struct Scratch(PathBuf);
    impl Scratch {
        fn new(name: &str) -> Self {
            let p = std::env::temp_dir().join(format!("ferrocast-media-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            Scratch(p)
        }
        fn path(&self, f: &str) -> String {
            self.0.join(f).to_string_lossy().into_owned()
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Run a binary with a hard 60 s deadline; returns (success, stdout, stderr).
    fn run(bin: &str, args: &[String]) -> (bool, String, String) {
        let mut child = Command::new(bin)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| panic!("spawn {bin}: {e}"));
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("{bin} exceeded the 60 s deadline: {args:?}");
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let o = child.wait_with_output().unwrap();
        (o.status.success(), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
    }

    fn ff(args: &[String]) {
        let mut a = s(&["-y", "-loglevel", "error"]);
        a.extend(args.iter().cloned());
        let (ok, _, err) = run(FFMPEG, &a);
        assert!(ok, "ffmpeg failed: {err}\nargs: {a:?}");
    }

    fn probe(path: &str, entries: &str, extra: &[&str]) -> String {
        let mut a = s(&["-v", "error"]);
        a.extend(extra.iter().map(|x| x.to_string()));
        a.extend(s(&["-show_entries", entries, "-of", "csv=p=0", path]));
        let (ok, out, err) = run(FFPROBE, &a);
        assert!(ok, "ffprobe failed: {err}");
        out.trim().to_string()
    }

    fn duration(path: &str) -> f64 {
        probe(path, "format=duration", &[]).parse().unwrap()
    }

    /// Per-channel RMS level (dB) via astats.
    fn rms_db(path: &str) -> Vec<f64> {
        let a = s(&["-hide_banner", "-i", path, "-af", "astats", "-f", "null", "-"]);
        let (ok, _, err) = run(FFMPEG, &a);
        assert!(ok, "astats failed: {err}");
        let mut v: Vec<f64> = err
            .lines()
            .filter_map(|l| l.split("RMS level dB:").nth(1))
            .map(|x| x.trim().parse::<f64>().unwrap_or(f64::NEG_INFINITY))
            .collect();
        // astats prints one value per channel, then the overall value (only when 2+ channels)
        if v.len() > 1 {
            v.pop();
        }
        v
    }

    fn assert_exists(p: &str) {
        assert!(Path::new(p).exists(), "missing output {p}");
    }

    #[test]
    #[ignore = "needs /opt/homebrew/bin/ffmpeg"]
    fn real_trim_audio_keeps_interior_pause_and_trims_edges() {
        let d = Scratch::new("trim");
        let src = d.path("src.wav");
        // 1 s silence, 1 s tone, 1 s silence (interior), 1 s tone, 1 s silence = 5 s
        ff(&s(&[
            "-filter_complex",
            "anullsrc=r=44100:cl=mono,atrim=duration=1[s0];sine=frequency=440:sample_rate=44100:duration=1[t0];anullsrc=r=44100:cl=mono,atrim=duration=1[s1];sine=frequency=440:sample_rate=44100:duration=1[t1];anullsrc=r=44100:cl=mono,atrim=duration=1[s2];[s0][t0][s1][t1][s2]concat=n=5:v=0:a=1[a]",
            "-map",
            "[a]",
            &src,
        ]));
        assert!((duration(&src) - 5.0).abs() < 0.1);
        let out = d.path("out.wav");
        ff(&trim_audio(&src, None, &out));
        let dur = duration(&out);
        assert!((2.8..3.3).contains(&dur), "expected ~3 s (tone+pause+tone), got {dur}");
    }

    #[test]
    #[ignore = "needs /opt/homebrew/bin/ffmpeg"]
    fn real_channels_levels_for_mono_and_stereo_sources() {
        let d = Scratch::new("chan");
        let mono = d.path("mono.wav");
        let stereo = d.path("stereo.wav");
        ff(&s(&["-f", "lavfi", "-i", "sine=frequency=440:sample_rate=44100:duration=1", &mono]));
        ff(&s(&["-f", "lavfi", "-i", "sine=frequency=440:sample_rate=44100:duration=1", "-af", "pan=stereo|c0=c0|c1=c0", &stereo]));
        let near = |a: f64, b: f64| (a - b).abs() < 0.3;
        let base = rms_db(&mono)[0];

        // mono source -> stereo with right at half gain: left unchanged, right -6 dB
        let o = d.path("m2s.wav");
        ff(&channels(&mono, 1, true, 1.0, 0.5, &o));
        let r = rms_db(&o);
        assert_eq!(r.len(), 2, "{r:?}");
        assert!(near(r[0], base) && near(r[1], base - 6.02), "{r:?} vs {base}");

        // stereo source -> stereo with left at half gain
        let o = d.path("s2s.wav");
        ff(&channels(&stereo, 2, true, 0.5, 1.0, &o));
        let r = rms_db(&o);
        assert!(near(r[0], base - 6.02) && near(r[1], base), "{r:?} vs {base}");

        // stereo source -> mono, unity gains: level preserved
        let o = d.path("s2m.wav");
        ff(&channels(&stereo, 2, false, 1.0, 1.0, &o));
        let r = rms_db(&o);
        assert_eq!(r.len(), 1, "{r:?}");
        assert!(near(r[0], base), "{r:?} vs {base}");

        // mono source -> mono, unity gains: level preserved
        let o = d.path("m2m.wav");
        ff(&channels(&mono, 1, false, 1.0, 1.0, &o));
        assert!(near(rms_db(&o)[0], base));
    }

    #[test]
    #[ignore = "needs /opt/homebrew/bin/ffmpeg"]
    fn real_join_mixed_silent_and_sound_clips() {
        let d = Scratch::new("join");
        let (a, b, c) = (d.path("a.mp4"), d.path("b.mp4"), d.path("c.mp4"));
        // a: 64x48 with sound, b: 32x32 silent, c: 48x64 with sound
        ff(&s(&[
            "-f",
            "lavfi",
            "-i",
            "testsrc=s=64x48:d=1:r=30",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=1",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
            &a,
        ]));
        ff(&s(&["-f", "lavfi", "-i", "testsrc=s=32x32:d=1.5:r=30", "-pix_fmt", "yuv420p", &b]));
        ff(&s(&[
            "-f",
            "lavfi",
            "-i",
            "testsrc=s=48x64:d=1:r=30",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=880:duration=1",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
            &c,
        ]));
        let out = d.path("joined.mp4");
        ff(&join(&[a.clone(), b.clone(), c.clone()], &[true, false, true], &[1.0, 1.5, 1.0], 64, 48, &out));
        assert_exists(&out);
        assert_eq!(probe(&out, "stream=width,height", &["-select_streams", "v:0"]), "64,48");
        assert_eq!(probe(&out, "stream=codec_type", &["-select_streams", "a:0"]), "audio");
        let dur = duration(&out);
        assert!((3.2..3.9).contains(&dur), "expected ~3.5 s, got {dur}");
        // the silent middle clip contributes silence, the sound clips do not
        let whole = rms_db(&out)[0];
        assert!(whole > -60.0, "sound clips should be audible: {whole}");
    }
}

#[cfg(test)]
mod compress_arg_tests {
    use super::*;

    #[test]
    fn audio_encoder_follows_the_output_extension() {
        assert_eq!(compress_audio("a.wav", "o.mp3", 128), s(&["-i", "a.wav", "-vn", "-c:a", "libmp3lame", "-b:a", "128k", "o.mp3"]));
        assert_eq!(compress_audio("a.wav", "o.m4a", 96), s(&["-i", "a.wav", "-vn", "-c:a", "aac", "-b:a", "96k", "o.m4a"]));
        assert!(compress_audio("a.wav", "o.mp4", 96).contains(&"aac".to_string()));
        assert!(compress_audio("a.wav", "o.ogg", 64).contains(&"libopus".to_string()));
        assert!(compress_audio("a.wav", "o.OPUS", 64).contains(&"libopus".to_string()));
        // an extension without an encoder of ours falls back to aac, never an unknown encoder
        assert!(compress_audio("a.wav", "o.wma", 64).contains(&"aac".to_string()));
    }

    #[test]
    fn video_with_a_bitrate_target_and_audio() {
        let v = compress_video("a.mp4", "o.mp4", VideoRate::Kbps(1200), Some(128), None);
        assert_eq!(
            v,
            s(&[
                "-i",
                "a.mp4",
                "-map",
                "0:v:0",
                "-map",
                "0:a:0",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-b:v",
                "1200k",
                "-maxrate",
                "1800k",
                "-bufsize",
                "2400k",
                "-c:a",
                "aac",
                "-b:a",
                "128k",
                "-movflags",
                "+faststart",
                "o.mp4"
            ])
        );
        assert!(!v.contains(&"-vf".to_string()), "no scale filter without a resize");
    }

    #[test]
    fn video_with_crf_presets_and_a_scale() {
        for crf in [23u8, 28, 32] {
            let v = compress_video("a.mp4", "o.mp4", VideoRate::Crf(crf), Some(96), Some((320, 180)));
            let at = v.iter().position(|x| x == "-crf").expect("crf flag");
            assert_eq!(v[at + 1], crf.to_string());
            assert!(!v.contains(&"-b:v".to_string()));
            let vf = v.iter().position(|x| x == "-vf").expect("scale flag");
            assert_eq!(v[vf + 1], "scale=320:180");
            assert!(v.contains(&"libx264".to_string()) && v.contains(&"+faststart".to_string()) && v.contains(&"yuv420p".to_string()));
        }
    }

    #[test]
    fn silent_video_has_no_audio_flags() {
        let v = compress_video("a.mp4", "o.mp4", VideoRate::Crf(28), None, None);
        assert!(v.contains(&"-an".to_string()));
        assert!(!v.contains(&"aac".to_string()) && !v.contains(&"-b:a".to_string()) && !v.contains(&"0:a:0".to_string()));
        assert_eq!(v.last().map(String::as_str), Some("o.mp4"));
    }
}
