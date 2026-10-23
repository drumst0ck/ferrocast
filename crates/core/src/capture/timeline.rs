//! Source time <-> output time through trims, splits and speed segments, EDL timeline
//! validation, and the undo/redo history over [`Project`].
//!
//! `trims` are the kept source ranges (empty = the whole recording), `speeds` play parts of
//! the kept ranges at a factor. The map is piecewise linear and monotonic; times in a cut
//! gap snap to the next kept start and times outside clamp, so both directions are total.

use super::project::{AudioTrack, Project, Speed, Trim};
use super::MIN_SEGMENT;
use crate::videoplan::SPEED_MIN;
use std::collections::VecDeque;

/// Fastest speed the EDL accepts (the video trimmer's own tool stops at 4x).
pub const SPEED_MAX: f64 = 16.0;

/// One kept source span played at a constant factor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Piece {
    pub src_start: f64,
    pub src_end: f64,
    pub factor: f64,
    pub out_start: f64,
}

impl Piece {
    fn out_end(&self) -> f64 {
        self.out_start + (self.src_end - self.src_start) / self.factor
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Timeline {
    pieces: Vec<Piece>,
    total: f64,
}

fn ok(v: f64) -> bool {
    v.is_finite()
}

/// Kept ranges of a project: sanitized trims, or `0..duration` when there are none.
fn kept_ranges(p: &Project) -> Vec<(f64, f64)> {
    if p.trims.is_empty() {
        return if ok(p.duration) && p.duration > 0.0 { vec![(0.0, p.duration)] } else { vec![] };
    }
    let mut r: Vec<(f64, f64)> =
        p.trims.iter().filter(|t| ok(t.start) && ok(t.end)).map(|t| (t.start.max(0.0), t.end)).filter(|(s, e)| e > s).collect();
    r.sort_by(|a, b| a.0.total_cmp(&b.0));
    r
}

impl Timeline {
    /// Tolerant of invalid projects (see [`validate`] for diagnostics): bad entries are skipped.
    pub fn new(p: &Project) -> Self {
        let mut speeds: Vec<&Speed> =
            p.speeds.iter().filter(|s| ok(s.start) && ok(s.end) && s.end > s.start && ok(s.factor) && s.factor > 0.0).collect();
        speeds.sort_by(|a, b| a.start.total_cmp(&b.start));
        let mut pieces: Vec<Piece> = vec![];
        let mut out = 0.0;
        let mut push = |pieces: &mut Vec<Piece>, s: f64, e: f64, factor: f64| {
            if e > s {
                let piece = Piece { src_start: s, src_end: e, factor, out_start: out };
                out = piece.out_end();
                pieces.push(piece);
            }
        };
        for (ks, ke) in kept_ranges(p) {
            let mut cur = ks;
            for sp in &speeds {
                let (s, e) = (sp.start.max(cur), sp.end.min(ke));
                if e > s {
                    push(&mut pieces, cur, s, 1.0);
                    push(&mut pieces, s, e, sp.factor);
                    cur = e;
                }
            }
            push(&mut pieces, cur, ke, 1.0);
        }
        Self { pieces, total: out }
    }

    pub fn pieces(&self) -> &[Piece] {
        &self.pieces
    }

    /// Length of the edited video: the sum of kept spans divided by their speed.
    pub fn output_duration(&self) -> f64 {
        self.total
    }

    pub fn is_kept(&self, t: f64) -> bool {
        self.pieces.iter().any(|c| t >= c.src_start && t <= c.src_end)
    }

    /// Playback factor at source time `t` (1 outside any speed segment).
    pub fn factor_at(&self, t: f64) -> f64 {
        self.pieces.iter().find(|c| t >= c.src_start && t < c.src_end).map_or(1.0, |c| c.factor)
    }

    /// Output time of source time `t`. A cut gap maps to the output time where it was removed.
    pub fn to_output(&self, t: f64) -> f64 {
        let Some(first) = self.pieces.first() else { return 0.0 };
        if !ok(t) || t <= first.src_start {
            return 0.0;
        }
        for c in &self.pieces {
            if t < c.src_start {
                return c.out_start;
            }
            if t < c.src_end {
                return c.out_start + (t - c.src_start) / c.factor;
            }
        }
        self.total
    }

    /// Source time of output time `o` (clamped to `0..=output_duration`).
    pub fn to_source(&self, o: f64) -> f64 {
        let Some(first) = self.pieces.first() else { return 0.0 };
        if !ok(o) || o <= 0.0 {
            return first.src_start;
        }
        for c in &self.pieces {
            if o < c.out_end() {
                return c.src_start + (o - c.out_start) * c.factor;
            }
        }
        self.pieces.last().map_or(0.0, |c| c.src_end)
    }
}

/// Problems with the timeline part of the EDL (trims, speeds, total duration).
pub fn validate(p: &Project) -> Vec<String> {
    let mut out = vec![];
    if !(ok(p.duration) && p.duration > 0.0) {
        out.push("duration must be greater than 0".into());
    }
    let mut prev_end = 0.0;
    for (i, t) in p.trims.iter().enumerate() {
        if !(ok(t.start) && ok(t.end)) {
            out.push(format!("trims[{i}]: start and end must be numbers"));
            continue;
        }
        if t.start < prev_end {
            out.push(format!("trims[{i}]: overlaps or is not sorted"));
        }
        if t.start < 0.0 || t.end > p.duration {
            out.push(format!("trims[{i}]: outside 0..{}", p.duration));
        }
        if t.end - t.start < MIN_SEGMENT {
            out.push(format!("trims[{i}]: shorter than {MIN_SEGMENT} s"));
        }
        prev_end = t.end;
    }
    let kept = kept_ranges(p);
    let mut prev_end = f64::NEG_INFINITY;
    for (i, s) in p.speeds.iter().enumerate() {
        if !(ok(s.start) && ok(s.end) && ok(s.factor)) {
            out.push(format!("speeds[{i}]: start, end and factor must be numbers"));
            continue;
        }
        if !(SPEED_MIN..=SPEED_MAX).contains(&s.factor) {
            out.push(format!("speeds[{i}]: factor {} outside {SPEED_MIN}..={SPEED_MAX}", s.factor));
        }
        if s.start < prev_end {
            out.push(format!("speeds[{i}]: overlaps or is not sorted"));
        }
        if s.end - s.start < MIN_SEGMENT {
            out.push(format!("speeds[{i}]: shorter than {MIN_SEGMENT} s"));
        }
        if !kept.iter().any(|&(ks, ke)| s.start >= ks && s.end <= ke) {
            out.push(format!("speeds[{i}]: not inside one kept range"));
        }
        prev_end = s.end;
    }
    if out.is_empty() && Timeline::new(p).output_duration() <= 0.0 {
        out.push("output duration must be greater than 0".into());
    }
    out
}

/// Effective kept ranges as `Trim`s.
fn kept_trims(p: &Project) -> Vec<Trim> {
    kept_ranges(p).into_iter().map(|(start, end)| Trim { start, end }).collect()
}

/// Intersect sorted ranges with `keep`, dropping pieces shorter than `MIN_SEGMENT`.
fn clip_speeds(speeds: &[Speed], kept: &[Trim]) -> Vec<Speed> {
    let mut out = vec![];
    for s in speeds {
        for k in kept {
            let (a, b) = (s.start.max(k.start), s.end.min(k.end));
            if b - a >= MIN_SEGMENT {
                out.push(Speed { start: a, end: b, factor: s.factor });
            }
        }
    }
    out
}

/// Cut `a..b` out of the video (the kept ranges lose it; speeds are clipped to what remains).
/// Remainders shorter than `MIN_SEGMENT` disappear. Returns false when nothing changes or
/// the cut would leave nothing.
pub fn remove_range(p: &mut Project, a: f64, b: f64) -> bool {
    if !(ok(a) && ok(b)) || b <= a {
        return false;
    }
    let mut next = vec![];
    for k in kept_trims(p) {
        if b <= k.start || a >= k.end {
            next.push(k);
            continue;
        }
        for (s, e) in [(k.start, a.min(k.end)), (b.max(k.start), k.end)] {
            if e - s >= MIN_SEGMENT {
                next.push(Trim { start: s, end: e });
            }
        }
    }
    if next.is_empty() || next == kept_trims(p) {
        return false;
    }
    p.speeds = clip_speeds(&p.speeds, &next);
    p.trims = next;
    true
}

/// Play `a..b` (clipped to the kept ranges) at `factor`, replacing speeds it overlaps.
/// A factor of 1 clears the range. Returns false for a factor outside range or no change.
pub fn set_speed(p: &mut Project, a: f64, b: f64, factor: f64) -> bool {
    if !(ok(a) && ok(b) && ok(factor)) || b <= a || !(SPEED_MIN..=SPEED_MAX).contains(&factor) {
        return false;
    }
    let kept = kept_trims(p);
    let mut speeds: Vec<Speed> = vec![];
    for s in &p.speeds {
        for (x, y) in [(s.start, s.end.min(a)), (s.start.max(b), s.end)] {
            if y - x >= MIN_SEGMENT {
                speeds.push(Speed { start: x, end: y, factor: s.factor });
            }
        }
    }
    if (factor - 1.0).abs() > f64::EPSILON {
        speeds.extend(clip_speeds(&[Speed { start: a, end: b, factor }], &kept));
    }
    speeds.sort_by(|x, y| x.start.total_cmp(&y.start));
    if speeds == p.speeds {
        return false;
    }
    p.speeds = speeds;
    true
}

/// What an edit touched; consecutive continuous edits of one kind merge into one undo step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditKind {
    Trim,
    Split,
    Speed,
    Zoom,
    Cursor,
    Style,
    Audio,
    Export,
    Other,
}

impl EditKind {
    /// Drag-like kinds coalesce; discrete ones (split, anything else) never do.
    fn coalesces(self) -> bool {
        !matches!(self, Self::Split | Self::Other)
    }
}

/// Snapshot undo/redo over the whole `Project` (every EDL field is covered by construction).
#[derive(Debug, Clone)]
pub struct History {
    undo: VecDeque<Project>,
    redo: Vec<Project>,
    depth: usize,
    window: f64,
    last: Option<(EditKind, f64)>,
}

impl History {
    /// `depth` bounds the undo stack; `window` is the coalescing window in seconds.
    pub fn new(depth: usize, window: f64) -> Self {
        Self { undo: VecDeque::new(), redo: vec![], depth: depth.max(1), window, last: None }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    /// Run `f` on the project at time `now` (seconds). Returns false, recording nothing,
    /// when `f` left the project unchanged.
    pub fn edit(&mut self, p: &mut Project, kind: EditKind, now: f64, f: impl FnOnce(&mut Project)) -> bool {
        let before = p.clone();
        f(p);
        if *p == before {
            return false;
        }
        let merge = kind.coalesces()
            && self.last.is_some_and(|(k, t)| k == kind && (0.0..=self.window).contains(&(now - t)))
            && !self.undo.is_empty();
        if !merge {
            self.undo.push_back(before);
            while self.undo.len() > self.depth {
                self.undo.pop_front();
            }
        }
        self.redo.clear();
        self.last = Some((kind, now));
        true
    }

    pub fn undo(&mut self, p: &mut Project) -> bool {
        let Some(prev) = self.undo.pop_back() else { return false };
        self.redo.push(std::mem::replace(p, prev));
        self.last = None;
        true
    }

    pub fn redo(&mut self, p: &mut Project) -> bool {
        let Some(next) = self.redo.pop() else { return false };
        self.undo.push_back(std::mem::replace(p, next));
        self.last = None;
        true
    }
}

/// One contiguous run of an audio FILE placed on the output timeline.
///
/// Times are seconds. `src_*` is the position inside the audio file (file time 0 = its first
/// sample), `out_start` is where the run starts in the exported video and `speed` is the
/// playback factor: the run lasts `(src_end - src_start) / speed` seconds in the output.
///
/// Decision (pitch): a speed change is a plain rate change of the audio by the same factor, so
/// pitch shifts with it (0.25x = two octaves down, 16x = four up). Time-stretching without a
/// pitch shift is NOT required. Using the video's exact factor is what keeps the audio inside
/// the +-40 ms sync budget (S16).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioSegment {
    pub src_start: f64,
    pub src_end: f64,
    pub out_start: f64,
    pub speed: f64,
}

impl AudioSegment {
    pub fn out_end(&self) -> f64 {
        self.out_start + (self.src_end - self.src_start) / self.speed
    }

    /// Sample-exact version for `rate` Hz. Every boundary is rounded once from its time in
    /// seconds, so consecutive segments share the same rounded edge and never drift or gap.
    pub fn to_samples(&self, rate: u32) -> SampleSegment {
        let r = f64::from(rate);
        let at = |t: f64| (t * r).round().max(0.0) as u64;
        SampleSegment {
            src_start: at(self.src_start),
            src_end: at(self.src_end),
            out_start: at(self.out_start),
            out_end: at(self.out_end()),
            speed: self.speed,
        }
    }
}

/// [`AudioSegment`] in sample frames at a given rate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SampleSegment {
    pub src_start: u64,
    pub src_end: u64,
    pub out_start: u64,
    pub out_end: u64,
    pub speed: f64,
}

/// The audio runs the exporter must place for `track`: the kept pieces of the timeline (trims,
/// splits, speeds) mapped into the audio file through its `offset` (the file starts `offset`
/// seconds after screen time 0; a negative offset means it started earlier). The part of a
/// piece that falls before the file starts is silence, so the segment starts later in the
/// output instead. The file's end is not known here: the exporter stops at end of file.
/// Empty when the project has no such source.
pub fn audio_segments(p: &Project, track: AudioTrack) -> Vec<AudioSegment> {
    let src = match track {
        AudioTrack::Mic => p.sources.mic.as_ref(),
        AudioTrack::System => p.sources.system.as_ref(),
    };
    let Some(off) = src.map(|s| s.offset).filter(|o| o.is_finite()) else { return vec![] };
    Timeline::new(p)
        .pieces()
        .iter()
        .filter_map(|c| {
            let (mut fs, fe) = (c.src_start - off, c.src_end - off);
            let mut out_start = c.out_start;
            if fs < 0.0 {
                out_start += -fs / c.factor;
                fs = 0.0;
            }
            (fe > fs).then_some(AudioSegment { src_start: fs, src_end: fe, out_start, speed: c.factor })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proj(duration: f64, trims: &[(f64, f64)], speeds: &[(f64, f64, f64)]) -> Project {
        let mut p = Project::new("t", "2025-01-01T00:00:00Z");
        p.duration = duration;
        p.trims = trims.iter().map(|&(start, end)| Trim { start, end }).collect();
        p.speeds = speeds.iter().map(|&(start, end, factor)| Speed { start, end, factor }).collect();
        p
    }

    fn cases() -> Vec<Project> {
        vec![
            proj(10.0, &[], &[]),
            proj(10.0, &[(1.0, 4.0), (6.0, 9.0)], &[]),
            proj(10.0, &[], &[(2.0, 4.0, 2.0), (5.0, 8.0, 0.5)]),
            proj(40.0, &[(0.0, 10.0), (12.0, 30.0)], &[(2.0, 6.0, 4.0), (14.0, 20.0, 16.0), (22.0, 30.0, 0.25)]),
        ]
    }

    #[test]
    fn whole_project_maps_identity() {
        let tl = Timeline::new(&proj(10.0, &[], &[]));
        assert_eq!(tl.output_duration(), 10.0);
        assert_eq!(tl.to_output(3.25), 3.25);
        assert_eq!(tl.to_source(3.25), 3.25);
    }

    #[test]
    fn trims_remove_gaps_and_boundaries_are_exact() {
        let tl = Timeline::new(&proj(10.0, &[(1.0, 4.0), (6.0, 9.0)], &[]));
        assert_eq!(tl.output_duration(), 6.0);
        assert_eq!(tl.to_output(1.0), 0.0);
        assert_eq!(tl.to_output(4.0), 3.0);
        assert_eq!(tl.to_output(6.0), 3.0);
        assert_eq!(tl.to_output(9.0), 6.0);
        assert_eq!(tl.to_source(0.0), 1.0);
        assert_eq!(tl.to_source(3.0), 6.0);
        assert_eq!(tl.to_source(6.0), 9.0);
        // inside a gap: snaps to the next kept start; outside: clamps.
        assert_eq!(tl.to_output(5.0), 3.0);
        assert_eq!(tl.to_output(0.2), 0.0);
        assert_eq!(tl.to_output(99.0), 6.0);
        assert_eq!(tl.to_source(-1.0), 1.0);
        assert_eq!(tl.to_source(99.0), 9.0);
        assert!(tl.is_kept(2.0) && !tl.is_kept(5.0));
    }

    #[test]
    fn speed_segments_scale_time() {
        let tl = Timeline::new(&proj(10.0, &[], &[(2.0, 4.0, 2.0), (5.0, 8.0, 0.5)]));
        // 2 + 1 + 1 + 6 + 2 = 12
        assert_eq!(tl.output_duration(), 12.0);
        assert_eq!(tl.to_output(2.0), 2.0);
        assert_eq!(tl.to_output(3.0), 2.5);
        assert_eq!(tl.to_output(4.0), 3.0);
        assert_eq!(tl.to_output(5.0), 4.0);
        assert_eq!(tl.to_output(6.0), 6.0);
        assert_eq!(tl.to_output(8.0), 10.0);
        assert_eq!(tl.to_source(6.0), 6.0);
        assert_eq!(tl.to_source(2.5), 3.0);
        assert_eq!(tl.factor_at(3.0), 2.0);
        assert_eq!(tl.factor_at(1.0), 1.0);
    }

    #[test]
    fn mapping_properties_on_grids() {
        for p in cases() {
            let tl = Timeline::new(&p);
            let kept: f64 = tl.pieces().iter().map(|c| (c.src_end - c.src_start) / c.factor).sum();
            assert!((tl.output_duration() - kept).abs() < 1e-9);
            let (mut prev_o, mut prev_s) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
            let mut t = 0.0;
            while t <= p.duration {
                let o = tl.to_output(t);
                assert!(o >= prev_o, "to_output monotonic at {t}");
                prev_o = o;
                if tl.is_kept(t) {
                    assert!((tl.to_source(o) - t).abs() < 1e-9, "round trip at {t}");
                }
                t += 0.05;
            }
            let mut o = 0.0;
            while o <= tl.output_duration() {
                let s = tl.to_source(o);
                assert!(s >= prev_s, "to_source monotonic at {o}");
                prev_s = s;
                assert!((tl.to_output(s) - o).abs() < 1e-9, "inverse at {o}");
                o += 0.05;
            }
        }
    }

    #[test]
    fn empty_timeline_is_safe() {
        let tl = Timeline::new(&proj(0.0, &[], &[]));
        assert_eq!(tl.output_duration(), 0.0);
        assert_eq!(tl.to_output(1.0), 0.0);
        assert_eq!(tl.to_source(1.0), 0.0);
    }

    #[test]
    fn valid_timelines_have_no_problems() {
        for p in cases() {
            assert_eq!(validate(&p), Vec::<String>::new());
        }
    }

    #[test]
    fn validation_reports_problems() {
        let has = |p: &Project, needle: &str| validate(p).iter().any(|m| m.contains(needle));
        assert!(has(&proj(0.0, &[], &[]), "duration"));
        assert!(has(&proj(10.0, &[(1.0, 5.0), (4.0, 8.0)], &[]), "trims[1]: overlaps"));
        assert!(has(&proj(10.0, &[(6.0, 8.0), (1.0, 2.0)], &[]), "trims[1]: overlaps or is not sorted"));
        assert!(has(&proj(10.0, &[(1.0, 1.01)], &[]), "shorter than"));
        assert!(has(&proj(10.0, &[(1.0, 11.0)], &[]), "outside"));
        assert!(has(&proj(10.0, &[], &[(1.0, 2.0, 0.2)]), "factor"));
        assert!(has(&proj(10.0, &[], &[(1.0, 2.0, 17.0)]), "factor"));
        assert!(has(&proj(10.0, &[], &[(1.0, 3.0, 2.0), (2.0, 4.0, 2.0)]), "speeds[1]: overlaps"));
        assert!(has(&proj(10.0, &[(0.0, 3.0), (5.0, 9.0)], &[(2.0, 6.0, 2.0)]), "kept range"));
        assert!(has(&proj(10.0, &[], &[(3.0, 3.0 + MIN_SEGMENT / 2.0, 2.0)]), "shorter than"));
        assert!(has(&proj(10.0, &[(f64::NAN, 3.0)], &[]), "trims[0]"));
        assert!(!has(&proj(10.0, &[(0.0, 5.0), (5.0, 9.0)], &[(1.0, 2.0, 16.0)]), "trims"));
    }

    #[test]
    fn remove_range_cuts_and_clips_speeds() {
        let mut p = proj(10.0, &[], &[(2.0, 6.0, 2.0)]);
        assert!(remove_range(&mut p, 4.0, 5.0));
        assert_eq!(p.trims, vec![Trim { start: 0.0, end: 4.0 }, Trim { start: 5.0, end: 10.0 }]);
        assert_eq!(p.speeds, vec![Speed { start: 2.0, end: 4.0, factor: 2.0 }, Speed { start: 5.0, end: 6.0, factor: 2.0 }]);
        assert!(validate(&p).is_empty());
        // too-short remainders vanish, nothing-to-do is false, whole removal refused
        assert!(remove_range(&mut p, 0.0, 3.99));
        assert_eq!(p.trims, vec![Trim { start: 5.0, end: 10.0 }]);
        assert!(!remove_range(&mut p, 0.0, 1.0));
        assert!(!remove_range(&mut p, 0.0, 10.0));
        assert_eq!(p.trims.len(), 1);
    }

    #[test]
    fn set_speed_overlays_and_one_clears() {
        let mut p = proj(10.0, &[], &[(1.0, 5.0, 2.0)]);
        assert!(set_speed(&mut p, 3.0, 7.0, 4.0));
        assert_eq!(p.speeds, vec![Speed { start: 1.0, end: 3.0, factor: 2.0 }, Speed { start: 3.0, end: 7.0, factor: 4.0 }]);
        assert!(validate(&p).is_empty());
        assert!(set_speed(&mut p, 0.0, 10.0, 1.0));
        assert!(p.speeds.is_empty());
        assert!(!set_speed(&mut p, 1.0, 2.0, 99.0));
        let mut q = proj(10.0, &[(0.0, 3.0), (5.0, 9.0)], &[]);
        assert!(set_speed(&mut q, 2.0, 6.0, 2.0));
        assert_eq!(q.speeds, vec![Speed { start: 2.0, end: 3.0, factor: 2.0 }, Speed { start: 5.0, end: 6.0, factor: 2.0 }]);
    }

    fn set_name(p: &mut Project, n: &str) {
        p.name = n.into();
    }

    #[test]
    fn undo_redo_restores_identical_json() {
        let mut p = proj(10.0, &[], &[]);
        let mut h = History::new(50, 1.0);
        let j0 = p.to_json();
        assert!(h.edit(&mut p, EditKind::Trim, 0.0, |p| {
            remove_range(p, 2.0, 3.0);
        }));
        let j1 = p.to_json();
        assert!(h.edit(&mut p, EditKind::Speed, 10.0, |p| {
            set_speed(p, 0.0, 1.0, 2.0);
        }));
        let j2 = p.to_json();
        assert!(h.can_undo() && !h.can_redo());
        assert!(h.undo(&mut p));
        assert_eq!(p.to_json(), j1);
        assert!(h.undo(&mut p));
        assert_eq!(p.to_json(), j0);
        assert!(!h.undo(&mut p));
        assert!(h.redo(&mut p));
        assert_eq!(p.to_json(), j1);
        assert!(h.redo(&mut p));
        assert_eq!(p.to_json(), j2);
        assert!(!h.redo(&mut p));
    }

    #[test]
    fn same_kind_edits_inside_window_coalesce() {
        let mut p = proj(10.0, &[], &[]);
        let j0 = p.to_json();
        let mut h = History::new(50, 1.0);
        for (i, n) in ["a", "b", "c"].iter().enumerate() {
            h.edit(&mut p, EditKind::Style, 0.5 * i as f64, |p| set_name(p, n));
        }
        assert_eq!(h.undo_len(), 1);
        assert!(h.undo(&mut p));
        assert_eq!(p.to_json(), j0);
        // different kind, or a gap larger than the window, starts a new entry
        h.edit(&mut p, EditKind::Style, 10.0, |p| set_name(p, "d"));
        h.edit(&mut p, EditKind::Zoom, 10.1, |p| set_name(p, "e"));
        h.edit(&mut p, EditKind::Zoom, 20.0, |p| set_name(p, "f"));
        assert_eq!(h.undo_len(), 3);
        // discrete kinds never coalesce
        h.edit(&mut p, EditKind::Split, 20.1, |p| set_name(p, "g"));
        h.edit(&mut p, EditKind::Split, 20.2, |p| set_name(p, "h"));
        assert_eq!(h.undo_len(), 5);
    }

    #[test]
    fn new_edit_clears_redo_and_noops_record_nothing() {
        let mut p = proj(10.0, &[], &[]);
        let mut h = History::new(50, 1.0);
        h.edit(&mut p, EditKind::Style, 0.0, |p| set_name(p, "a"));
        h.undo(&mut p);
        assert!(h.can_redo());
        assert!(!h.edit(&mut p, EditKind::Style, 5.0, |_| {}));
        assert!(h.can_redo());
        assert!(h.edit(&mut p, EditKind::Style, 6.0, |p| set_name(p, "b")));
        assert!(!h.can_redo());
        // undo then a coalescable edit right away must not merge into the undone entry
        h.undo(&mut p);
        h.edit(&mut p, EditKind::Style, 6.1, |p| set_name(p, "c"));
        h.edit(&mut p, EditKind::Style, 6.2, |p| set_name(p, "d"));
        assert_eq!(h.undo_len(), 1);
    }

    #[test]
    fn depth_is_bounded() {
        let mut p = proj(10.0, &[], &[]);
        let mut h = History::new(3, 0.0);
        for i in 0..10 {
            h.edit(&mut p, EditKind::Split, f64::from(i), |p| set_name(p, &i.to_string()));
        }
        assert_eq!(h.undo_len(), 3);
        while h.undo(&mut p) {}
        assert_eq!(p.name, "6");
    }

    mod audio {
        use super::*;
        use crate::capture::project::Source;

        fn with_mic(mut p: Project, offset: f64) -> Project {
            p.sources.mic = Some(Source { file: "mic.m4a".into(), offset });
            p
        }
        fn seg(src_start: f64, src_end: f64, out_start: f64, speed: f64) -> AudioSegment {
            AudioSegment { src_start, src_end, out_start, speed }
        }

        #[test]
        fn no_source_means_no_segments() {
            assert!(audio_segments(&proj(10.0, &[], &[]), AudioTrack::Mic).is_empty());
            let p = with_mic(proj(10.0, &[], &[]), 0.0);
            assert!(audio_segments(&p, AudioTrack::System).is_empty());
        }

        #[test]
        fn whole_recording_is_one_segment() {
            let p = with_mic(proj(10.0, &[], &[]), 0.0);
            assert_eq!(audio_segments(&p, AudioTrack::Mic), vec![seg(0.0, 10.0, 0.0, 1.0)]);
        }

        #[test]
        fn trims_drop_gaps_and_pack_the_output() {
            let p = with_mic(proj(10.0, &[(1.0, 4.0), (6.0, 9.0)], &[]), 0.0);
            assert_eq!(audio_segments(&p, AudioTrack::Mic), vec![seg(1.0, 4.0, 0.0, 1.0), seg(6.0, 9.0, 3.0, 1.0)]);
        }

        #[test]
        fn speed_segments_split_and_scale_the_rate() {
            let p = with_mic(proj(10.0, &[], &[(2.0, 4.0, 0.25), (6.0, 8.0, 16.0)]), 0.0);
            let s = audio_segments(&p, AudioTrack::Mic);
            assert_eq!(
                s,
                vec![
                    seg(0.0, 2.0, 0.0, 1.0),
                    seg(2.0, 4.0, 2.0, 0.25),
                    seg(4.0, 6.0, 10.0, 1.0),
                    seg(6.0, 8.0, 12.0, 16.0),
                    seg(8.0, 10.0, 12.125, 1.0),
                ]
            );
            assert_eq!(s[1].out_end(), 10.0);
            let tl = Timeline::new(&p);
            assert_eq!(s.last().unwrap().out_end(), tl.output_duration());
        }

        #[test]
        fn mic_recorded_late_is_shifted_and_starts_with_silence() {
            let p = with_mic(proj(10.0, &[], &[]), 0.2);
            let s = audio_segments(&p, AudioTrack::Mic);
            assert_eq!(s.len(), 1);
            assert_eq!((s[0].src_start, s[0].out_start, s[0].speed), (0.0, 0.2, 1.0));
            assert!((s[0].src_end - 9.8).abs() < 1e-12);
            // screen time 5.0 is file time 4.8 and output time 5.0
            let t = 5.0;
            assert!(((t - 0.2) - (s[0].src_start + (t - s[0].out_start))).abs() < 1e-12);
        }

        #[test]
        fn late_offset_inside_a_sped_up_piece_scales_the_silence() {
            // piece 0..4 at 4x lasts 1 s; file starts at screen 2.0 -> silence is 0.5 s of output.
            let p = with_mic(proj(4.0, &[], &[(0.0, 4.0, 4.0)]), 2.0);
            assert_eq!(audio_segments(&p, AudioTrack::Mic), vec![seg(0.0, 2.0, 0.5, 4.0)]);
        }

        #[test]
        fn file_that_starts_after_every_piece_gives_nothing() {
            let p = with_mic(proj(10.0, &[(0.0, 1.0)], &[]), 5.0);
            assert!(audio_segments(&p, AudioTrack::Mic).is_empty());
        }

        #[test]
        fn early_file_skips_into_the_file() {
            let p = with_mic(proj(10.0, &[(1.0, 2.0)], &[]), -0.05);
            let s = audio_segments(&p, AudioTrack::Mic);
            assert_eq!(s.len(), 1);
            assert!((s[0].src_start - 1.05).abs() < 1e-12 && (s[0].src_end - 2.05).abs() < 1e-12 && s[0].out_start == 0.0);
        }

        #[test]
        fn samples_are_exact_and_contiguous() {
            let p = with_mic(proj(10.0, &[(1.0, 4.0)], &[(2.0, 3.0, 4.0), (3.0, 4.0, 0.25)]), 0.2);
            let segs = audio_segments(&p, AudioTrack::Mic);
            let rate = 48_000;
            let s: Vec<_> = segs.iter().map(|a| a.to_samples(rate)).collect();
            assert_eq!((s[0].src_start, s[0].src_end, s[0].out_start, s[0].out_end), (38_400, 86_400, 0, 48_000));
            // contiguous: each segment starts where the previous one ended
            for w in s.windows(2) {
                assert_eq!(w[0].out_end, w[1].out_start);
            }
            let tl = Timeline::new(&p);
            assert_eq!(s.last().unwrap().out_end, (tl.output_duration() * f64::from(rate)).round() as u64);
        }

        #[test]
        fn samples_round_each_edge_once_at_44100() {
            let a = seg(0.0, 1.0 / 3.0, 0.0, 1.0).to_samples(44_100);
            assert_eq!((a.src_end, a.out_end), (14_700, 14_700));
            let b = seg(0.0, 1.0, 1.0 / 3.0, 16.0).to_samples(44_100);
            assert_eq!((b.out_start, b.out_end), (14_700, 14_700 + 2_756));
        }
    }
}
