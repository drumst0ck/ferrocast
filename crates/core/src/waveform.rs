//! Waveform maths for the audio trim window: per-bucket peaks to draw and the span of sound
//! inside a clip with silence at either end. Pure, so it is testable without audio files.

/// Minimum and maximum sample of each of `buckets` equal slices of `samples`, covering every
/// sample. Never more buckets than samples; empty for empty input or zero buckets.
pub fn peaks(samples: &[i16], buckets: usize) -> Vec<(i16, i16)> {
    let n = buckets.min(samples.len());
    if n == 0 {
        return vec![];
    }
    (0..n)
        .map(|k| {
            let (a, b) = (k * samples.len() / n, (k + 1) * samples.len() / n);
            let slice = &samples[a..b];
            (slice.iter().copied().min().unwrap_or(0), slice.iter().copied().max().unwrap_or(0))
        })
        .collect()
}

/// Start and end, in seconds, of the part of the clip louder than `threshold_db` (dBFS, a
/// negative number). `None` when nothing is that loud or the inputs are unusable.
pub fn silence_bounds(samples: &[i16], rate: u32, threshold_db: f32) -> Option<(f64, f64)> {
    if rate == 0 || samples.is_empty() || !threshold_db.is_finite() {
        return None;
    }
    let limit = 32768.0 * 10f64.powf(threshold_db as f64 / 20.0);
    let loud = |v: &i16| (*v as f64).abs() > limit;
    let first = samples.iter().position(loud)?;
    let last = samples.iter().rposition(loud)?;
    Some((first as f64 / rate as f64, (last + 1) as f64 / rate as f64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peaks_give_min_and_max_per_bucket_covering_every_sample() {
        let s: Vec<i16> = vec![0, 10, -20, 5, 100, -100, 7, 7];
        assert_eq!(peaks(&s, 4), vec![(0, 10), (-20, 5), (-100, 100), (7, 7)]);
        assert_eq!(peaks(&s, 1), vec![(-100, 100)]);
        assert_eq!(peaks(&s, 8), s.iter().map(|v| (*v, *v)).collect::<Vec<_>>());
    }

    #[test]
    fn peaks_never_return_more_buckets_than_samples_and_handle_empty_input() {
        let s: Vec<i16> = vec![1, 2, 3];
        assert_eq!(peaks(&s, 100).len(), 3);
        assert!(peaks(&[], 10).is_empty());
        assert!(peaks(&s, 0).is_empty());
        let big: Vec<i16> = (0..10_001).map(|i| (i % 100) as i16).collect();
        let p = peaks(&big, 800);
        assert_eq!(p.len(), 800);
        assert_eq!(p.iter().map(|x| x.1).max(), Some(99), "no sample is skipped");
    }

    #[test]
    fn silence_bounds_find_the_span_of_sound() {
        let mut s = vec![0i16; 1000];
        for v in &mut s[200..700] {
            *v = 8000;
        }
        let (a, b) = silence_bounds(&s, 1000, -40.0).unwrap();
        assert!((a - 0.2).abs() < 1e-9 && (b - 0.7).abs() < 1e-9, "{a} {b}");
    }

    #[test]
    fn silence_at_the_start_only_or_the_end_only() {
        let mut lead = vec![0i16; 500];
        lead.extend(vec![9000i16; 500]);
        let (a, b) = silence_bounds(&lead, 1000, -40.0).unwrap();
        assert!((a - 0.5).abs() < 1e-9 && (b - 1.0).abs() < 1e-9, "start only: {a} {b}");
        let mut tail = vec![9000i16; 500];
        tail.extend(vec![0i16; 500]);
        let (a, b) = silence_bounds(&tail, 1000, -40.0).unwrap();
        assert!(a.abs() < 1e-9 && (b - 0.5).abs() < 1e-9, "end only: {a} {b}");
    }

    #[test]
    fn a_full_scale_square_wave_has_no_silence_and_an_all_zero_clip_has_only_silence() {
        let sq: Vec<i16> = (0..1000).map(|i| if i % 2 == 0 { i16::MAX } else { i16::MIN }).collect();
        assert_eq!(silence_bounds(&sq, 1000, -40.0), Some((0.0, 1.0)));
        assert_eq!(silence_bounds(&vec![0i16; 1000], 1000, -40.0), None);
        assert_eq!(silence_bounds(&[], 1000, -40.0), None);
        assert_eq!(silence_bounds(&sq, 0, -40.0), None, "a zero sample rate is unusable");
        assert_eq!(silence_bounds(&sq, 1000, f32::NAN), None);
    }

    #[test]
    fn the_threshold_decides_what_counts_as_sound() {
        // 100 / 32768 is about -50 dBFS
        let mut s = vec![0i16; 1000];
        for v in &mut s[100..900] {
            *v = 100;
        }
        assert!(silence_bounds(&s, 1000, -40.0).is_none(), "quieter than the threshold is silence");
        assert!(silence_bounds(&s, 1000, -60.0).is_some());
    }
}
