//! Vertical stitching of scrolling-capture strips (pure, RGBA8 buffers).
//!
//! Each strip is one capture of the same area after scrolling. For a pair of
//! consecutive strips the shift `s >= 1` is found so that row `r` of the next
//! strip shows what row `r + s` of the previous one showed. Candidates are
//! pre-filtered by row hashes, then verified with a sum of absolute
//! differences (SAD). Rows that never change between strips (sticky header or
//! footer) are kept once and excluded from the search.

use std::fmt;

/// Steps (strips) a scrolling capture may take before it is stopped.
/// The app shows `Scrolling capture stopped at 60 screens` for the cap.
pub const MAX_STEPS: usize = 60;

/// Largest mean absolute per-byte difference accepted for an overlap.
const MAX_MEAN_DIFF: f64 = 2.0;
/// Smallest overlap, in rows, that is trusted.
const MIN_OVERLAP_ROWS: usize = 4;

/// One RGBA8 capture, `width * height * 4` bytes, row-major, no padding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Strip {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

impl Strip {
    pub fn new(width: usize, height: usize, rgba: Vec<u8>) -> Result<Self, StitchError> {
        if width == 0 || height == 0 || rgba.len() != width * height * 4 {
            return Err(StitchError::BadBuffer);
        }
        Ok(Self { width, height, rgba })
    }

    fn row(&self, y: usize) -> &[u8] {
        let n = self.width * 4;
        &self.rgba[y * n..(y + 1) * n]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StitchError {
    /// No strips given.
    Empty,
    /// Buffer length does not match the dimensions, or a zero dimension.
    BadBuffer,
    /// A strip differs in size from the first one.
    SizeMismatch { index: usize },
}

impl fmt::Display for StitchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "no strips to stitch"),
            Self::BadBuffer => write!(f, "strip buffer does not match its size"),
            Self::SizeMismatch { index } => write!(f, "strip {index} has a different size"),
        }
    }
}

impl std::error::Error for StitchError {}

/// Why stitching stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    /// All strips were merged.
    Exhausted,
    /// Two identical strips in a row: the content stopped scrolling.
    EndOfScroll,
    /// [`MAX_STEPS`] strips were consumed and more were offered.
    StoppedAtCap,
    /// The overlap with strip `at` was not trustworthy; the result holds the
    /// strips before it (partial result).
    LowConfidence { at: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stitched {
    pub image: Strip,
    /// Strips merged into `image`.
    pub used: usize,
    pub stop: StopReason,
}

/// Shift (in rows) between two strips, searching only rows `header..height-footer`.
/// `None` when no overlap passes the hash prefilter and SAD verification.
pub fn detect_overlap(prev: &Strip, next: &Strip, header: usize, footer: usize) -> Option<usize> {
    if prev.width != next.width || prev.height != next.height {
        return None;
    }
    let end = prev.height.checked_sub(footer)?;
    let region = end.checked_sub(header)?;
    let min_overlap = MIN_OVERLAP_ROWS.min(region);
    if region < 2 || min_overlap == 0 {
        return None;
    }
    let ha: Vec<u64> = (0..prev.height).map(|y| hash_row(prev.row(y))).collect();
    let hb: Vec<u64> = (0..next.height).map(|y| hash_row(next.row(y))).collect();

    let mut best: Option<(f64, usize)> = None;
    for s in 1..=(region - min_overlap) {
        let overlap = region - s;
        // Prefilter: at least half of the overlapping rows hash-equal.
        let equal = (0..overlap).filter(|&r| ha[header + s + r] == hb[header + r]).count();
        if equal * 2 < overlap {
            continue;
        }
        let mut sad = 0u64;
        for r in 0..overlap {
            sad += row_sad(prev.row(header + s + r), next.row(header + r));
        }
        let mean = sad as f64 / (overlap * prev.width * 4) as f64;
        if mean > MAX_MEAN_DIFF {
            continue;
        }
        if best.is_none_or(|(m, _)| mean < m) {
            best = Some((mean, s));
        }
    }
    best.map(|(_, s)| s)
}

/// Rows at the top and bottom that are identical in every changing pair of strips.
pub fn detect_sticky(strips: &[Strip]) -> (usize, usize) {
    let Some(first) = strips.first() else {
        return (0, 0);
    };
    let cap = first.height / 3;
    let (mut header, mut footer) = (cap, cap);
    let mut seen = false;
    for pair in strips.windows(2) {
        if pair[0] == pair[1] {
            continue;
        }
        seen = true;
        header = header.min((0..cap).take_while(|&y| pair[0].row(y) == pair[1].row(y)).count());
        let h = first.height;
        footer = footer.min((0..cap).take_while(|&i| pair[0].row(h - 1 - i) == pair[1].row(h - 1 - i)).count());
    }
    if seen {
        (header, footer)
    } else {
        (0, 0)
    }
}

/// Merge `strips` (in capture order) into one tall image.
pub fn stitch(strips: &[Strip]) -> Result<Stitched, StitchError> {
    let first = strips.first().ok_or(StitchError::Empty)?;
    for (index, s) in strips.iter().enumerate() {
        if s.rgba.len() != s.width * s.height * 4 || s.width == 0 || s.height == 0 {
            return Err(StitchError::BadBuffer);
        }
        if s.width != first.width || s.height != first.height {
            return Err(StitchError::SizeMismatch { index });
        }
    }
    let considered = &strips[..strips.len().min(MAX_STEPS)];
    let (header, footer) = detect_sticky(considered);
    let region_end = first.height - footer;

    // Everything but the footer of the first strip.
    let mut data: Vec<u8> = first.rgba[..region_end * first.width * 4].to_vec();
    let mut used = 1;
    let mut stop = StopReason::Exhausted;
    for i in 1..considered.len() {
        let (prev, next) = (&considered[i - 1], &considered[i]);
        if prev == next {
            stop = StopReason::EndOfScroll;
            break;
        }
        let Some(s) = detect_overlap(prev, next, header, footer) else {
            stop = StopReason::LowConfidence { at: i };
            break;
        };
        for y in (region_end - s)..region_end {
            data.extend_from_slice(next.row(y));
        }
        used = i + 1;
    }
    if stop == StopReason::Exhausted && strips.len() > MAX_STEPS {
        stop = StopReason::StoppedAtCap;
    }
    // Footer of the last merged strip.
    let last = &considered[used - 1];
    for y in region_end..last.height {
        data.extend_from_slice(last.row(y));
    }
    let height = data.len() / (first.width * 4);
    Ok(Stitched { image: Strip { width: first.width, height, rgba: data }, used, stop })
}

fn hash_row(row: &[u8]) -> u64 {
    row.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

fn row_sad(a: &[u8], b: &[u8]) -> u64 {
    a.iter().zip(b).map(|(&x, &y)| u64::from(x.abs_diff(y))).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: usize = 6;

    fn page_row(i: usize) -> Vec<u8> {
        let mut v = Vec::with_capacity(W * 4);
        for x in 0..W {
            let k = (i * 31 + x * 7 + 3) as u64;
            let h = k.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 56;
            v.extend_from_slice(&[h as u8, (h as u8).wrapping_add(x as u8 * 13), (i % 251) as u8, 255]);
        }
        v
    }

    fn solid_row(v: u8) -> Vec<u8> {
        vec![v; W * 4]
    }

    fn page(rows: usize) -> Vec<Vec<u8>> {
        (0..rows).map(page_row).collect()
    }

    fn strip(rows: &[Vec<u8>]) -> Strip {
        Strip::new(W, rows.len(), rows.concat()).unwrap()
    }

    /// Strips of `h` rows taken every `step` rows over `page`, `n` of them.
    fn scroll(page: &[Vec<u8>], h: usize, step: usize, n: usize) -> Vec<Strip> {
        (0..n).map(|k| strip(&page[k * step..k * step + h])).collect()
    }

    #[test]
    fn exact_overlap_merges_to_the_page() {
        let p = page(120);
        let out = stitch(&scroll(&p, 40, 25, 4)).unwrap();
        assert_eq!(out.stop, StopReason::Exhausted);
        assert_eq!(out.used, 4);
        assert_eq!(out.image, strip(&p[..25 * 3 + 40]));
    }

    #[test]
    fn one_row_step_has_no_duplicates_or_gaps() {
        let p = page(60);
        let out = stitch(&scroll(&p, 20, 1, 10)).unwrap();
        assert_eq!(out.image, strip(&p[..9 + 20]));
    }

    #[test]
    fn fixed_header_is_kept_once() {
        let body = page(120);
        let hdr: Vec<Vec<u8>> = (0..5).map(|i| solid_row(10 + i)).collect();
        let strips: Vec<Strip> = (0..4)
            .map(|k| {
                let mut rows = hdr.clone();
                rows.extend_from_slice(&body[k * 20..k * 20 + 35]);
                strip(&rows)
            })
            .collect();
        assert_eq!(detect_sticky(&strips), (5, 0));
        let mut expected = hdr.clone();
        expected.extend_from_slice(&body[..60 + 35]);
        let out = stitch(&strips).unwrap();
        assert_eq!(out.image, strip(&expected));
    }

    #[test]
    fn fixed_footer_is_kept_once_at_the_end() {
        let body = page(120);
        let ftr: Vec<Vec<u8>> = (0..4).map(|i| solid_row(200 + i)).collect();
        let strips: Vec<Strip> = (0..4)
            .map(|k| {
                let mut rows = body[k * 20..k * 20 + 36].to_vec();
                rows.extend_from_slice(&ftr);
                strip(&rows)
            })
            .collect();
        assert_eq!(detect_sticky(&strips), (0, 4));
        let mut expected = body[..60 + 36].to_vec();
        expected.extend_from_slice(&ftr);
        assert_eq!(stitch(&strips).unwrap().image, strip(&expected));
    }

    #[test]
    fn end_of_scroll_on_two_identical_strips() {
        let p = page(100);
        let mut strips = scroll(&p, 40, 20, 3);
        strips.push(strips[2].clone());
        strips.push(strips[2].clone());
        let out = stitch(&strips).unwrap();
        assert_eq!(out.stop, StopReason::EndOfScroll);
        assert_eq!(out.used, 3);
        assert_eq!(out.image, strip(&p[..40 + 40]));
    }

    #[test]
    fn cap_stops_at_sixty_steps() {
        assert_eq!(MAX_STEPS, 60);
        let p = page(300);
        let out = stitch(&scroll(&p, 30, 3, 61)).unwrap();
        assert_eq!(out.stop, StopReason::StoppedAtCap);
        assert_eq!(out.used, 60);
        assert_eq!(out.image, strip(&p[..59 * 3 + 30]));
    }

    #[test]
    fn low_confidence_overlap_returns_partial() {
        let p = page(200);
        let mut strips = scroll(&p, 40, 20, 2);
        strips.push(strip(&p[150..190])); // jumped: no overlap with strip 1
        let out = stitch(&strips).unwrap();
        assert_eq!(out.stop, StopReason::LowConfidence { at: 2 });
        assert_eq!(out.used, 2);
        assert_eq!(out.image, strip(&p[..20 + 40]));
    }

    #[test]
    fn mismatched_sizes_are_an_error() {
        let a = strip(&page(10));
        let b = Strip::new(W + 1, 10, vec![0; (W + 1) * 10 * 4]).unwrap();
        assert_eq!(stitch(&[a.clone(), b]), Err(StitchError::SizeMismatch { index: 1 }));
        assert_eq!(stitch(&[]), Err(StitchError::Empty));
        assert_eq!(Strip::new(W, 2, vec![0; 3]), Err(StitchError::BadBuffer));
    }

    #[test]
    fn single_strip_is_returned_as_is() {
        let a = strip(&page(10));
        let out = stitch(std::slice::from_ref(&a)).unwrap();
        assert_eq!(out.image, a);
        assert_eq!(out.stop, StopReason::Exhausted);
    }
}
