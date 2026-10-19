//! SRT ↔ WebVTT subtitle conversion.

fn normalise(src: &str) -> String {
    src.trim_start_matches('\u{feff}').replace("\r\n", "\n").replace('\r', "\n")
}

fn is_timing(line: &str) -> bool {
    line.contains("-->")
}

/// Split on blank lines into blocks of lines.
fn blocks(text: &str) -> Vec<Vec<&str>> {
    let mut out: Vec<Vec<&str>> = Vec::new();
    let mut cur: Vec<&str> = Vec::new();
    for line in text.split('\n') {
        if line.trim().is_empty() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(line);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Convert SRT text to WebVTT.
pub fn srt_to_vtt(src: &str) -> String {
    let text = normalise(src);
    let mut out = String::from("WEBVTT\n");
    for block in blocks(&text) {
        let Some(t) = block.iter().position(|l| is_timing(l)) else {
            continue;
        };
        out.push('\n');
        out.push_str(&block[t].replace(',', "."));
        out.push('\n');
        for l in &block[t + 1..] {
            out.push_str(l);
            out.push('\n');
        }
    }
    out
}

/// Normalise one VTT timestamp to `HH:MM:SS,mmm`.
fn srt_time(t: &str) -> String {
    let t = t.trim().replace('.', ",");
    if t.matches(':').count() == 1 {
        format!("00:{t}")
    } else {
        t
    }
}

/// Remove `<00:01.000>`-style inline timestamp tags, keeping other tags.
fn strip_inline_timestamps(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        let after = &rest[i..];
        match after.find('>') {
            Some(j) => {
                let inner = &after[1..j];
                let is_ts = !inner.is_empty() && inner.chars().all(|c| c.is_ascii_digit() || c == ':' || c == '.');
                if !is_ts {
                    out.push_str(&after[..=j]);
                }
                rest = &after[j + 1..];
            }
            None => {
                out.push_str(after);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// Convert WebVTT text to SRT.
pub fn vtt_to_srt(src: &str) -> String {
    let text = normalise(src);
    let mut out = String::new();
    let mut n = 0;
    for block in blocks(&text) {
        let first = block[0].trim();
        let head = first.split([' ', '\t']).next().unwrap_or("");
        if !is_timing(first) && matches!(head, "WEBVTT" | "NOTE" | "STYLE" | "REGION") {
            continue;
        }
        let Some(t) = block.iter().position(|l| is_timing(l)) else {
            continue;
        };
        let (a, b) = block[t].split_once("-->").unwrap_or(("", ""));
        let end = b.split_whitespace().next().unwrap_or("");
        n += 1;
        if n > 1 {
            out.push('\n');
        }
        out.push_str(&format!("{n}\n{} --> {}\n", srt_time(a), srt_time(end)));
        for l in &block[t + 1..] {
            out.push_str(&strip_inline_timestamps(l));
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRT: &str = "1\n00:00:01,000 --> 00:00:02,500\nHello\nworld\n\n2\n00:00:03,000 --> 00:00:04,000\n<i>Second</i>\n\n3\n01:00:00,100 --> 01:00:01,200\nThird, with comma\nline two\n";

    #[test]
    fn round_trip() {
        let vtt = srt_to_vtt(SRT);
        assert!(vtt.starts_with("WEBVTT\n\n00:00:01.000 --> 00:00:02.500\nHello\nworld\n"));
        assert!(vtt.contains("Third, with comma"));
        assert_eq!(vtt_to_srt(&vtt), SRT);
    }

    #[test]
    fn vtt_short_timestamps_settings_note() {
        let vtt = "\u{feff}WEBVTT - title\n\nNOTE a comment\nover lines\n\nSTYLE\n::cue { color: red }\n\nintro\n00:01.000 --> 00:02.500 align:start position:10%\nHi <00:01.500>there <b>bold</b>\n\n00:03.000 --> 01:00:04.000\nEnd\n";
        assert_eq!(vtt_to_srt(vtt), "1\n00:00:01,000 --> 00:00:02,500\nHi there <b>bold</b>\n\n2\n00:00:03,000 --> 01:00:04,000\nEnd\n");
    }

    #[test]
    fn crlf_and_bom() {
        let src = format!("\u{feff}{}", SRT.replace('\n', "\r\n"));
        assert_eq!(srt_to_vtt(&src), srt_to_vtt(SRT));
        assert_eq!(vtt_to_srt(&srt_to_vtt(&src).replace('\n', "\r\n")), SRT);
    }

    #[test]
    fn empty() {
        assert_eq!(srt_to_vtt(""), "WEBVTT\n");
        assert_eq!(vtt_to_srt(""), "");
        assert_eq!(vtt_to_srt("WEBVTT\n"), "");
    }
}
