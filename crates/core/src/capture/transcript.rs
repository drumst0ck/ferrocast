//! Transcript model and pure editing helpers (P5): words in SOURCE time (seconds on the
//! screen take), filler/pause detection, and conversion of cut ranges into one undoable EDL edit.

use super::project::Project;
use super::timeline::{remove_range, EditKind, History, Timeline};
use super::MIN_SEGMENT;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Word {
    pub text: String,
    pub start: f64,
    pub end: f64,
    #[serde(default = "full_confidence")]
    pub confidence: f64,
}

fn full_confidence() -> f64 {
    1.0
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Transcript {
    pub language: String,
    pub words: Vec<Word>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// Tunables shared by the helpers; `Default` holds the S18 values. All times are seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// Padding added on each side of a filler cut (clamped so it never eats a kept word).
    pub guard: f64,
    /// Shortest cut ever returned (one frame at 25 fps, equal to `MIN_SEGMENT`).
    pub min_cut: f64,
    /// Length of the screen take; ranges are clamped to it. `None` = unbounded
    /// (and trailing silence is left alone, it cannot be measured).
    pub duration: Option<f64>,
    /// A filler next to a pause at least this long is a candidate.
    pub isolation: f64,
    /// A filler whose confidence is below this is a candidate (0 counts as low).
    pub low_confidence: f64,
    /// Silences longer than this are tightened.
    pub pause_threshold: f64,
    /// Silence left after tightening.
    pub keep_pause: f64,
    /// Silence kept before the first / after the last word.
    pub lead_in: f64,
    /// A pause at least this long ends a sentence.
    pub sentence_pause: f64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            guard: 0.02,
            min_cut: MIN_SEGMENT,
            duration: None,
            isolation: 0.2,
            low_confidence: 0.5,
            pause_threshold: 0.7,
            keep_pause: 0.25,
            lead_in: 0.25,
            sentence_pause: 1.0,
        }
    }
}

/// Tolerance for comparing times produced by float subtraction.
const EPS: f64 = 1e-9;

impl Transcript {
    /// Strict decode: unknown fields are kept, structural problems name the offending word.
    pub fn from_json(json: &str) -> Result<Self, String> {
        let t: Self = serde_json::from_str(json).map_err(|e| format!("transcript is not valid: {e}"))?;
        t.validate()?;
        Ok(t)
    }

    pub fn validate(&self) -> Result<(), String> {
        let mut prev = f64::NEG_INFINITY;
        for (i, w) in self.words.iter().enumerate() {
            if !(w.start.is_finite() && w.end.is_finite()) {
                return Err(format!("transcript word {i}: start and end must be finite numbers"));
            }
            if w.start < 0.0 {
                return Err(format!("transcript word {i}: start is negative"));
            }
            if w.end < w.start {
                return Err(format!("transcript word {i}: end is before start"));
            }
            if !(0.0..=1.0).contains(&w.confidence) {
                return Err(format!("transcript word {i}: confidence must be between 0 and 1"));
            }
            if w.start < prev {
                return Err(format!("transcript word {i}: out of order (starts before word {})", i.saturating_sub(1)));
            }
            prev = w.start;
        }
        Ok(())
    }

    fn usable(w: &Word) -> bool {
        w.start.is_finite() && w.end.is_finite() && w.end >= w.start
    }
}

/// Lowercase, strip accents (precomposed and combining) and punctuation around the word.
pub fn normalize(s: &str) -> String {
    let mapped: String = s
        .chars()
        .filter(|c| !('\u{300}'..='\u{36f}').contains(c))
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' | 'ã' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' | 'õ' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            c => c,
        })
        .collect();
    mapped.trim_matches(|c: char| !c.is_alphanumeric()).to_string()
}

const EN: &[&str] = &["um", "uh", "uhm", "er", "erm", "like", "you know"];
const EN_SURE: &[&str] = &["um", "uh", "uhm", "er", "erm"];
const ES: &[&str] = &["eh", "em", "este", "o sea"];
const ES_SURE: &[&str] = &["eh", "em"];

fn lang(locale: &str) -> &str {
    locale.split(['-', '_']).next().unwrap_or("")
}

/// Filler words/phrases for a locale (`en`, `en-US`, `es_MX`, case-insensitive; others: none).
/// Deliberately short: the S18 list (`um, uh, eh, este, o sea, like, you know`) plus variants
/// that are the same sound (`uhm`, `er`, `erm`, `em`). Entries are normalised (see [`normalize`]);
/// multi-word entries match consecutive words.
pub fn filler_words(locale: &str) -> &'static [&'static str] {
    match lang(locale).to_ascii_lowercase().as_str() {
        "en" => EN,
        "es" => ES,
        _ => &[],
    }
}

/// The single-word disfluencies that are never real vocabulary: removed without any other
/// evidence. `like`, `you know`, `este`, `o sea` are NOT here: they are ordinary words.
pub fn unambiguous_fillers(locale: &str) -> &'static [&'static str] {
    match lang(locale).to_ascii_lowercase().as_str() {
        "en" => EN_SURE,
        "es" => ES_SURE,
        _ => &[],
    }
}

/// Merge overlapping or touching ranges (input need not be sorted).
fn merge(mut r: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    r.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out: Vec<(f64, f64)> = vec![];
    for (s, e) in r {
        match out.last_mut() {
            Some(l) if s <= l.1 + EPS => l.1 = l.1.max(e),
            _ => out.push((s, e)),
        }
    }
    out
}

/// Clamp to `[0, duration]` and enforce `min_cut` (widened around the centre, shifted to fit);
/// `None` when nothing usable remains.
fn finish(mut s: f64, mut e: f64, o: &Options) -> Option<(f64, f64)> {
    let hi = o.duration.filter(|d| d.is_finite()).unwrap_or(f64::INFINITY);
    s = s.max(0.0);
    e = e.min(hi);
    if e <= s {
        return None;
    }
    if e - s < o.min_cut {
        let mid = (s + e) / 2.0;
        s = (mid - o.min_cut / 2.0).max(0.0);
        e = s + o.min_cut;
        if e > hi {
            e = hi;
            s = hi - o.min_cut;
            if s < 0.0 {
                return None;
            }
        }
    }
    Some((s, e))
}

/// Source-time ranges to cut so the spoken fillers disappear.
///
/// Rule (simplest that keeps "I like this"): a word or phrase from [`filler_words`] (after
/// [`normalize`]) is removed only when it is one of the [`unambiguous_fillers`], OR its lowest
/// confidence is below `low_confidence`, OR it is isolated by a pause of at least `isolation`
/// before or after it. The start and end of the transcript do not count as pauses. Longest
/// phrase wins at each position. Each cut is the filler span padded by `guard` (never into
/// a kept neighbour or below 0 or past the take), at least `min_cut` long, merged when touching.
pub fn remove_fillers(t: &Transcript, locale: &str, o: &Options) -> Vec<(f64, f64)> {
    let list = filler_words(locale);
    let sure = unambiguous_fillers(locale);
    let v: Vec<&Word> = t.words.iter().filter(|w| Transcript::usable(w)).collect();
    let tokens: Vec<String> = v.iter().map(|w| normalize(&w.text)).collect();
    let mut phrases: Vec<Vec<&str>> = list.iter().map(|p| p.split_whitespace().collect()).collect();
    phrases.sort_by_key(|p| std::cmp::Reverse(p.len()));
    // matched[i] = Some(last index of the run starting at i)
    let mut runs: Vec<(usize, usize)> = vec![];
    let mut i = 0;
    while i < v.len() {
        let hit = phrases.iter().find(|p| i + p.len() <= v.len() && p.iter().enumerate().all(|(k, tok)| tokens[i + k] == *tok));
        let Some(p) = hit else {
            i += 1;
            continue;
        };
        let j = i + p.len() - 1;
        let is_sure = p.len() == 1 && sure.contains(&p[0]);
        let low = v[i..=j].iter().any(|w| w.confidence < o.low_confidence);
        let before = i > 0 && v[i].start - v[i - 1].end >= o.isolation - EPS;
        let after = j + 1 < v.len() && v[j + 1].start - v[j].end >= o.isolation - EPS;
        if is_sure || low || before || after {
            runs.push((i, j));
            i = j + 1;
        } else {
            i += 1;
        }
    }
    let matched = |k: usize| runs.iter().any(|&(a, b)| (a..=b).contains(&k));
    let mut cuts = vec![];
    for &(a, b) in &runs {
        let mut s = v[a].start - o.guard;
        if a > 0 && !matched(a - 1) {
            s = s.max(v[a - 1].end.min(v[a].start));
        }
        let mut e = v[b].end + o.guard;
        if b + 1 < v.len() && !matched(b + 1) {
            e = e.min(v[b + 1].start.max(v[b].end));
        }
        if let Some(c) = finish(s, e, o) {
            cuts.push(c);
        }
    }
    merge(cuts)
}

/// Source-time ranges that shorten long silences.
///
/// Between consecutive words, a gap longer than `pause_threshold` (0.7 s) is cut to `keep_pause`
/// (0.25 s): the cut is `(prev_end + keep/2, next_start - keep/2)`, so 0.125 s of silence stays
/// on each side. Example: words end at 1.0 and resume at 2.0 (gap 1.0): cut `(1.125, 1.875)`,
/// remaining silence `1.125-1.0 + 2.0-1.875 = 0.25`. Leading silence longer than the threshold
/// is cut from 0 to `first.start - lead_in` (0.25 s lead-in is kept); with `duration` set,
/// trailing silence longer than the threshold is cut from `last.end + lead_in` to the end.
pub fn tighten_pauses(t: &Transcript, o: &Options) -> Vec<(f64, f64)> {
    let v: Vec<&Word> = t.words.iter().filter(|w| Transcript::usable(w)).collect();
    let Some(first) = v.first() else { return vec![] };
    let mut cuts = vec![];
    let mut push = |s: f64, e: f64| {
        if let Some(c) = finish(s, e, &Options { min_cut: 0.0, ..o.clone() }) {
            if c.1 - c.0 >= o.min_cut {
                cuts.push(c);
            }
        }
    };
    if first.start > o.pause_threshold + EPS {
        push(0.0, first.start - o.lead_in);
    }
    let mut end = first.end;
    for w in &v[1..] {
        if w.start - end > o.pause_threshold + EPS {
            push(end + o.keep_pause / 2.0, w.start - o.keep_pause / 2.0);
        }
        end = end.max(w.end);
    }
    if let Some(d) = o.duration.filter(|d| d.is_finite()) {
        if d - end > o.pause_threshold + EPS {
            push(end + o.lead_in, d);
        }
    }
    merge(cuts)
}

fn ends_sentence(text: &str) -> bool {
    text.trim_end_matches(['"', '\'', '”', '’', ')', ']', '»']).ends_with(['.', '!', '?', '…'])
}

/// Word-index ranges, one per sentence: a sentence ends after a word with terminal punctuation
/// (`. ! ? …`, closing quotes allowed) or before a pause of `sentence_pause` (1 s) or more.
/// Abbreviations such as "Mr." split too; the editor treats sentences as selection shortcuts.
pub fn sentences(t: &Transcript, o: &Options) -> Vec<Range<usize>> {
    let n = t.words.len();
    let mut out = vec![];
    let mut start = 0;
    for (i, w) in t.words.iter().enumerate() {
        let pause = t.words.get(i + 1).is_some_and(|nx| nx.start - w.end >= o.sentence_pause - EPS);
        if i + 1 == n || ends_sentence(&w.text) || pause {
            out.push(start..i + 1);
            start = i + 1;
        }
    }
    out
}

/// The single cut that deletes words `first..=last`: from the middle of the silence before
/// `first` (0 when it is the first word) to the middle of the silence after `last` (the word's
/// end for the last word), so both gaps are consumed and no sliver remains.
/// `None` for an empty or out-of-range selection or non-finite times.
pub fn word_range_cuts(t: &Transcript, first: usize, last: usize) -> Option<(f64, f64)> {
    let (f, l) = (t.words.get(first)?, t.words.get(last)?);
    if first > last || !Transcript::usable(f) || !Transcript::usable(l) {
        return None;
    }
    let s = match first.checked_sub(1).map(|i| &t.words[i]) {
        Some(p) if Transcript::usable(p) => (p.end.min(f.start) + f.start) / 2.0,
        Some(_) => return None,
        None => 0.0,
    };
    let e = match t.words.get(last + 1) {
        Some(n) if Transcript::usable(n) => (l.end + n.start.max(l.end)) / 2.0,
        Some(_) => return None,
        None => l.end,
    };
    Some((s, e))
}

/// Apply source-time cuts to a copy of the project through [`remove_range`] (kept ranges lose
/// them, speeds are clipped). Ranges are clamped to the take, sorted and merged; ranges
/// shorter than `MIN_SEGMENT`, outside the take or invalid are ignored. Errors, without
/// changing anything, when the cuts would leave nothing to play.
pub fn apply_cuts(p: &Project, cuts: &[(f64, f64)]) -> Result<Project, String> {
    let hi = if p.duration.is_finite() && p.duration > 0.0 { p.duration } else { f64::INFINITY };
    let ranges = merge(
        cuts.iter()
            .filter(|(a, b)| a.is_finite() && b.is_finite())
            .map(|&(a, b)| (a.max(0.0), b.min(hi)))
            .filter(|(a, b)| b - a >= MIN_SEGMENT)
            .collect(),
    );
    let mut q = p.clone();
    for (a, b) in ranges {
        let touches = Timeline::new(&q).pieces().iter().any(|c| c.src_start.max(a) < c.src_end.min(b));
        if touches && !remove_range(&mut q, a, b) {
            return Err("Removing these ranges would delete everything; nothing was changed.".into());
        }
    }
    Ok(q)
}

/// [`apply_cuts`] as ONE undo step (`EditKind::Other`, never coalesced): a single ⌘Z restores
/// the project byte for byte. `Ok(false)` when nothing changed (nothing is recorded).
pub fn apply_cuts_edit(h: &mut History, p: &mut Project, cuts: &[(f64, f64)], now: f64) -> Result<bool, String> {
    let next = apply_cuts(p, cuts)?;
    Ok(h.edit(p, EditKind::Other, now, |p| *p = next))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(text: &str, start: f64, end: f64) -> Word {
        Word { text: text.into(), start, end, confidence: 0.95 }
    }
    fn wc(text: &str, start: f64, end: f64, c: f64) -> Word {
        Word { text: text.into(), start, end, confidence: c }
    }
    fn tr(words: Vec<Word>) -> Transcript {
        Transcript { language: "en-US".into(), words, ..Default::default() }
    }
    fn opts(d: f64) -> Options {
        Options { duration: Some(d), ..Options::default() }
    }
    fn near(a: (f64, f64), b: (f64, f64)) {
        assert!((a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9, "{a:?} != {b:?}");
    }
    fn project(d: f64) -> Project {
        let mut p = Project::new("t", "2025-01-01T00:00:00Z");
        p.duration = d;
        p
    }

    // ---- model ----
    #[test]
    fn decode_defaults_and_unknown_fields() {
        let t = Transcript::from_json(r#"{"words":[{"text":"hi","start":0.5,"end":0.9,"future":1}],"newer":true}"#).unwrap();
        assert_eq!(t.language, "");
        assert_eq!(t.words[0].confidence, 1.0);
        assert!(t.generated_at.is_none());
        assert!(t.extra.contains_key("newer"));
        assert!(Transcript::from_json("{}").unwrap().words.is_empty());
    }

    #[test]
    fn decode_errors_are_clear() {
        let e = Transcript::from_json(r#"{"words":[{"text":"a","start":2,"end":1}]}"#).unwrap_err();
        assert!(e.contains("word 0") && e.contains("end"), "{e}");
        let e = Transcript::from_json(r#"{"words":[{"text":"a","start":0,"end":1},{"text":"b","start":-1,"end":1}]}"#).unwrap_err();
        assert!(e.contains("word 1") && e.contains("negative"), "{e}");
        let e = Transcript::from_json(r#"{"words":[{"text":"a","start":0,"end":1,"confidence":1.5}]}"#).unwrap_err();
        assert!(e.contains("confidence"), "{e}");
        let e = Transcript::from_json(r#"{"words":[{"text":"a","start":2,"end":3},{"text":"b","start":1,"end":1.5}]}"#).unwrap_err();
        assert!(e.contains("word 1") && e.contains("order"), "{e}");
        let e = Transcript::from_json(r#"{"words":[{"text":"a","end":3}]}"#).unwrap_err();
        assert!(e.contains("start"), "{e}");
        assert!(Transcript::from_json("nope").unwrap_err().contains("transcript"));
    }

    #[test]
    fn roundtrip_omits_empty_generated_at() {
        let t = tr(vec![w("hi", 0.0, 0.5)]);
        let j = serde_json::to_string(&t).unwrap();
        assert!(!j.contains("generated_at"));
        assert_eq!(Transcript::from_json(&j).unwrap(), t);
    }

    // ---- normalisation / lists ----
    #[test]
    fn normalize_case_punctuation_accents() {
        assert_eq!(normalize("Um,"), "um");
        assert_eq!(normalize("“Éh…”"), "eh");
        assert_eq!(normalize("e\u{301}h"), "eh"); // decomposed accent
        assert_eq!(normalize("don't."), "don't");
        assert_eq!(normalize("..."), "");
    }

    #[test]
    fn locale_lists() {
        assert!(filler_words("en-US").contains(&"um") && filler_words("EN").contains(&"you know"));
        assert!(filler_words("es_MX").contains(&"o sea") && filler_words("es-ES").contains(&"este"));
        assert!(!filler_words("en").contains(&"este"));
        assert!(filler_words("fr").is_empty() && filler_words("").is_empty());
        for l in ["en", "es"] {
            for u in unambiguous_fillers(l) {
                assert!(filler_words(l).contains(u));
            }
        }
        assert!(!unambiguous_fillers("en").contains(&"like"));
    }

    // ---- fillers ----
    #[test]
    fn unambiguous_filler_removed_without_pause() {
        // "so um we": no pauses at all, still removed. guard 0.02 each side.
        let t = tr(vec![w("so", 0.0, 0.3), w("um", 0.3, 0.6), w("we", 0.6, 0.9)]);
        let c = remove_fillers(&t, "en", &opts(5.0));
        assert_eq!(c.len(), 1);
        // guard is clamped to the neighbours' edges (no overlap with kept words)
        near(c[0], (0.3, 0.6));
    }

    #[test]
    fn guard_extends_into_silence() {
        let t = tr(vec![w("so", 0.0, 0.3), w("Um,", 0.5, 0.8), w("we", 1.0, 1.3)]);
        near(remove_fillers(&t, "en", &opts(5.0))[0], (0.48, 0.82));
    }

    #[test]
    fn like_in_sentence_is_kept_but_isolated_like_goes() {
        let t = tr(vec![w("I", 0.0, 0.1), w("like", 0.1, 0.4), w("this", 0.4, 0.8)]);
        assert!(remove_fillers(&t, "en", &opts(5.0)).is_empty());
        // pause >= 0.2 s on one side
        let t = tr(vec![w("I", 0.0, 0.1), w("went", 0.1, 0.4), w("like", 0.8, 1.0), w("home", 1.0, 1.4)]);
        assert_eq!(remove_fillers(&t, "en", &opts(5.0)).len(), 1);
        // pause just under 0.2 s on both sides: kept
        let t = tr(vec![w("I", 0.0, 0.1), w("went", 0.1, 0.4), w("like", 0.59, 1.0), w("home", 1.19, 1.4)]);
        assert!(remove_fillers(&t, "en", &opts(5.0)).is_empty());
        // low confidence qualifies without pauses
        let t = tr(vec![w("I", 0.0, 0.1), wc("like", 0.1, 0.4, 0.2), w("this", 0.4, 0.8)]);
        assert_eq!(remove_fillers(&t, "en", &opts(5.0)).len(), 1);
    }

    #[test]
    fn zero_confidence_word_is_low_confidence() {
        let t = tr(vec![w("a", 0.0, 0.1), wc("este", 0.1, 0.4, 0.0), w("libro", 0.4, 0.8)]);
        assert_eq!(remove_fillers(&t, "es", &opts(5.0)).len(), 1);
        let t = tr(vec![w("a", 0.0, 0.1), w("este", 0.1, 0.4), w("libro", 0.4, 0.8)]);
        assert!(remove_fillers(&t, "es", &opts(5.0)).is_empty());
    }

    #[test]
    fn phrases_across_words_and_spanish_accents() {
        let t = tr(vec![w("it", 0.0, 0.3), w("you", 0.8, 1.0), w("know,", 1.0, 1.3), w("works", 1.8, 2.2)]);
        let c = remove_fillers(&t, "en", &opts(5.0));
        assert_eq!(c.len(), 1);
        near(c[0], (0.78, 1.32));
        // "you" alone is not a filler
        let t = tr(vec![w("it", 0.0, 0.3), w("you", 0.8, 1.0), w("works", 1.8, 2.2)]);
        assert!(remove_fillers(&t, "en", &opts(5.0)).is_empty());
        // Spanish with accents, uppercase, trailing punctuation
        let t = tr(vec![w("pero", 0.0, 0.3), w("¡Éh!", 0.3, 0.6), w("O", 1.0, 1.1), w("sea,", 1.1, 1.4), w("sí", 2.0, 2.2)]);
        let c = remove_fillers(&t, "es", &opts(5.0));
        assert_eq!(c.len(), 2);
        near(c[0], (0.3, 0.62));
        near(c[1], (0.98, 1.42));
    }

    #[test]
    fn adjacent_fillers_merge_and_clamp_to_take() {
        let t = tr(vec![w("um", 0.0, 0.3), w("uh", 0.3, 0.6), w("ok", 0.6, 1.0), w("um", 4.9, 5.2)]);
        let c = remove_fillers(&t, "en", &opts(5.0));
        assert_eq!(c.len(), 2);
        near(c[0], (0.0, 0.6)); // never negative
        near(c[1], (4.88, 5.0)); // clamped to take duration
    }

    #[test]
    fn tiny_fillers_are_widened_to_min_cut_and_invalid_words_skipped() {
        let t = tr(vec![w("ok", 0.0, 1.0), w("um", 2.0, 2.001), w("ok", 3.0, 4.0), w("um", f64::NAN, 1.0)]);
        let c = remove_fillers(&t, "en", &Options { guard: 0.0, duration: Some(10.0), ..Options::default() });
        assert_eq!(c.len(), 1);
        assert!(c[0].1 - c[0].0 >= Options::default().min_cut - 1e-12 && c[0].0 <= 2.0 && c[0].1 >= 2.001);
    }

    #[test]
    fn filler_edge_cases() {
        assert!(remove_fillers(&tr(vec![]), "en", &opts(5.0)).is_empty());
        assert_eq!(remove_fillers(&tr(vec![w("um", 0.0, 0.3)]), "en", &Options::default()).len(), 1);
        assert!(remove_fillers(&tr(vec![w("um", 0.0, 0.3)]), "fr", &Options::default()).is_empty());
        // beyond the take: dropped
        let t = tr(vec![w("um", 9.0, 9.3)]);
        assert!(remove_fillers(&t, "en", &opts(5.0)).is_empty());
    }

    // ---- pauses ----
    #[test]
    fn pause_arithmetic_hand_checked() {
        // gap 1.0 .. 2.0 (1.0 s > 0.7). keep 0.125 each side: cut (1.125, 1.875) leaves 0.25 s.
        let t = tr(vec![w("a", 0.0, 1.0), w("b", 2.0, 2.5)]);
        let c = tighten_pauses(&t, &Options::default());
        assert_eq!(c.len(), 1);
        near(c[0], (1.125, 1.875));
        assert!((2.0 - 1.0 - (c[0].1 - c[0].0) - 0.25).abs() < 1e-9);
    }

    #[test]
    fn pause_threshold_is_strict() {
        let t = tr(vec![w("a", 0.0, 1.0), w("b", 1.7, 2.0), w("c", 2.71, 3.0)]);
        // 0.7 exactly (not longer) kept; 0.71 cut to 0.25
        let c = tighten_pauses(&t, &Options::default());
        assert_eq!(c.len(), 1);
        near(c[0], (2.125, 2.585));
    }

    #[test]
    fn leading_and_trailing_silence() {
        let t = tr(vec![w("a", 3.0, 3.5), w("b", 3.6, 4.0)]);
        // leading: keep a 0.25 s lead-in before the first word
        near(tighten_pauses(&t, &Options::default())[0], (0.0, 2.75));
        // trailing needs the take duration; keeps 0.25 s after the last word
        let c = tighten_pauses(&t, &opts(10.0));
        assert_eq!(c.len(), 2);
        near(c[1], (4.25, 10.0));
        // short lead / tail untouched
        let t = tr(vec![w("a", 0.5, 1.0)]);
        assert!(tighten_pauses(&t, &opts(1.5)).is_empty());
    }

    #[test]
    fn pause_edge_cases() {
        assert!(tighten_pauses(&tr(vec![]), &opts(10.0)).is_empty());
        let one = tighten_pauses(&tr(vec![w("a", 0.0, 0.5)]), &opts(0.9));
        assert!(one.is_empty());
        // overlapping words: no negative gap
        assert!(tighten_pauses(&tr(vec![w("a", 0.0, 1.0), w("b", 0.9, 1.5)]), &Options::default()).is_empty());
    }

    // ---- sentences / word ranges ----
    #[test]
    fn sentence_splitting() {
        let t = tr(vec![
            w("Hi", 0.0, 0.2),
            w("there.", 0.2, 0.5),
            w("¿Qué", 0.6, 0.8),
            w("tal?", 0.8, 1.0),
            w("and", 1.0, 1.2),
            w("then", 2.5, 2.8),
            w("\"ok!\"", 2.8, 3.0),
        ]);
        assert_eq!(sentences(&t, &Options::default()), vec![0..2, 2..4, 4..5, 5..7]);
        assert!(sentences(&tr(vec![]), &Options::default()).is_empty());
        assert_eq!(sentences(&tr(vec![w("a", 0.0, 1.0)]), &Options::default()), vec![0..1]);
    }

    #[test]
    fn word_range_cut_joins_gaps() {
        let t = tr(vec![w("a", 0.0, 1.0), w("b", 1.4, 2.0), w("c", 2.2, 3.0), w("d", 3.6, 4.0)]);
        // delete b..c: from midpoint of gap before b (1.2) to midpoint of gap after c (3.3)
        near(word_range_cuts(&t, 1, 2).unwrap(), (1.2, 3.3));
        // first word: from 0; last word: to its end
        near(word_range_cuts(&t, 0, 0).unwrap(), (0.0, 1.2));
        near(word_range_cuts(&t, 3, 3).unwrap(), (3.3, 4.0));
        assert!(word_range_cuts(&t, 2, 1).is_none() && word_range_cuts(&t, 0, 4).is_none());
        assert!(word_range_cuts(&tr(vec![]), 0, 0).is_none());
    }

    // ---- apply ----
    #[test]
    fn apply_cuts_removes_ranges_in_source_time() {
        let p = project(10.0);
        let q = apply_cuts(&p, &[(1.0, 2.0), (1.5, 2.5), (5.0, 6.0)]).unwrap();
        assert_eq!(Timeline::new(&q).output_duration(), 7.5);
        assert_eq!(q.trims.len(), 3);
        assert_eq!(p.trims.len(), 0);
    }

    #[test]
    fn apply_cuts_ignores_tiny_outside_and_invalid() {
        let p = project(10.0);
        assert_eq!(apply_cuts(&p, &[]).unwrap(), p);
        assert_eq!(apply_cuts(&p, &[(1.0, 1.0 + super::super::MIN_SEGMENT / 2.0), (20.0, 30.0), (f64::NAN, 2.0), (5.0, 4.0)]).unwrap(), p);
        // partially outside is clamped
        let q = apply_cuts(&p, &[(9.0, 30.0), (-5.0, 0.5)]).unwrap();
        assert_eq!(Timeline::new(&q).output_duration(), 8.5);
    }

    #[test]
    fn apply_cuts_rejects_removing_everything() {
        let p = project(10.0);
        let e = apply_cuts(&p, &[(0.0, 10.0)]).unwrap_err();
        assert!(e.contains("everything"), "{e}");
        assert!(apply_cuts(&p, &[(0.0, 5.0), (5.0, 10.0)]).is_err());
        assert!(apply_cuts(&p, &[(-1.0, 99.0)]).is_err());
        // leaves only a sliver shorter than MIN_SEGMENT: also nothing
        assert!(apply_cuts(&p, &[(0.0, 9.99)]).is_err());
        // on an already trimmed project
        let mut q = project(10.0);
        q.trims = vec![crate::capture::project::Trim { start: 2.0, end: 4.0 }];
        assert!(apply_cuts(&q, &[(1.0, 5.0)]).is_err());
        assert!(apply_cuts(&q, &[(6.0, 7.0)]).unwrap() == q);
    }

    #[test]
    fn apply_cuts_one_undo_step_byte_identical() {
        let mut p = project(10.0);
        let before = p.to_json();
        let mut h = History::new(50, 0.5);
        let cuts = [(1.0, 1.5), (3.0, 3.5), (6.0, 7.0)];
        assert!(apply_cuts_edit(&mut h, &mut p, &cuts, 0.0).unwrap());
        assert_eq!(h.undo_len(), 1);
        let after = p.to_json();
        assert_ne!(after, before);
        assert!(h.undo(&mut p));
        assert_eq!(p.to_json(), before);
        assert!(!h.can_undo());
        assert!(h.redo(&mut p));
        assert_eq!(p.to_json(), after);
        // a second immediate edit never coalesces (EditKind::Other)
        assert!(apply_cuts_edit(&mut h, &mut p, &[(8.0, 9.0)], 0.1).unwrap());
        assert_eq!(h.undo_len(), 2);
        // no change records nothing; rejection leaves the project alone
        assert!(!apply_cuts_edit(&mut h, &mut p, &[], 0.2).unwrap());
        let snap = p.to_json();
        assert!(apply_cuts_edit(&mut h, &mut p, &[(0.0, 10.0)], 0.3).is_err());
        assert_eq!(p.to_json(), snap);
        assert_eq!(h.undo_len(), 2);
    }
}
