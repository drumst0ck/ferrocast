//! Small fuzzy matcher tuned for launcher queries (`msg` → Messages,
//! `vsc` → Visual Studio Code).

/// Sensitivity of the fuzzy matcher (mirrors Raycast's Root Search Sensitivity).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Sensitivity {
    /// Any subsequence matches.
    Low,
    /// Subsequence, but gaps are penalised and very sparse matches are dropped.
    #[default]
    Medium,
    /// Every query char must start a word or continue a run.
    High,
}

/// Result of a successful match.
#[derive(Debug, Clone, PartialEq)]
pub struct Match {
    /// Higher is better.
    pub score: i64,
    /// Char indices (in the candidate) that matched.
    pub indices: Vec<usize>,
}

fn is_word_start(chars: &[char], i: usize) -> bool {
    if i == 0 {
        return true;
    }
    let prev = chars[i - 1];
    let cur = chars[i];
    !prev.is_alphanumeric() || (prev.is_lowercase() && cur.is_uppercase()) || (prev.is_alphabetic() && cur.is_numeric())
}

fn fold(c: char) -> char {
    // Lowercase and strip the most common Latin accents so `cafe` matches `Café`.
    let c = c.to_lowercase().next().unwrap_or(c);
    match c {
        'á' | 'à' | 'ä' | 'â' | 'ã' | 'å' => 'a',
        'é' | 'è' | 'ë' | 'ê' => 'e',
        'í' | 'ì' | 'ï' | 'î' => 'i',
        'ó' | 'ò' | 'ö' | 'ô' | 'õ' => 'o',
        'ú' | 'ù' | 'ü' | 'û' => 'u',
        'ñ' => 'n',
        'ç' => 'c',
        _ => c,
    }
}

/// Fuzzy-match `query` against `candidate`. Whitespace in the query is ignored
/// except that each whitespace-separated token must match in order.
pub fn fuzzy_match(query: &str, candidate: &str, sensitivity: Sensitivity) -> Option<Match> {
    let q: Vec<char> = query.chars().filter(|c| !c.is_whitespace()).map(fold).collect();
    if q.is_empty() {
        return Some(Match { score: 0, indices: vec![] });
    }
    let orig: Vec<char> = candidate.chars().collect();
    let c: Vec<char> = orig.iter().copied().map(fold).collect();
    if q.len() > c.len() {
        return None;
    }

    // Fast path: contiguous substring.
    let cand_lower: String = c.iter().collect();
    let q_str: String = q.iter().collect();
    if let Some(byte_pos) = cand_lower.find(&q_str) {
        let start = cand_lower[..byte_pos].chars().count();
        let mut score = 1000 - start as i64 * 5 + (q.len() as i64 * 20);
        if start == 0 {
            score += 500;
        } else if is_word_start(&orig, start) {
            score += 250;
        }
        if q.len() == c.len() {
            score += 1000;
        }
        score -= (c.len() as i64 - q.len() as i64).max(0);
        return Some(Match { score, indices: (start..start + q.len()).collect() });
    }

    // Greedy subsequence that prefers word starts: for each query char, look for
    // the next word-start occurrence before falling back to the nearest one.
    let mut indices = Vec::with_capacity(q.len());
    let mut pos = 0usize;
    for (qi, &qc) in q.iter().enumerate() {
        let remaining = q.len() - qi - 1;
        let limit = c.len().saturating_sub(remaining);
        let mut chosen = None;
        // continuing a run is best
        if let Some(&last) = indices.last() {
            if last + 1 < limit && c[last + 1] == qc {
                chosen = Some(last + 1);
            }
        }
        if chosen.is_none() {
            chosen = (pos..limit).find(|&i| c[i] == qc && is_word_start(&orig, i));
        }
        if chosen.is_none() {
            if sensitivity == Sensitivity::High {
                return None;
            }
            chosen = (pos..limit).find(|&i| c[i] == qc);
        }
        let i = chosen?;
        indices.push(i);
        pos = i + 1;
    }

    let mut score: i64 = 0;
    let mut gaps = 0i64;
    for (k, &i) in indices.iter().enumerate() {
        score += 10;
        if is_word_start(&orig, i) {
            score += 60;
        }
        if k > 0 {
            let prev = indices[k - 1];
            if i == prev + 1 {
                score += 40;
            } else {
                gaps += (i - prev - 1) as i64;
            }
        } else {
            score -= i as i64 * 3;
            if i == 0 {
                score += 80;
            }
        }
    }
    score -= gaps;
    score -= (c.len() as i64 - q.len() as i64).max(0) / 2;

    if sensitivity == Sensitivity::Medium {
        // Reject matches where most chars land mid-word far apart.
        let word_starts = indices.iter().filter(|&&i| is_word_start(&orig, i)).count();
        if q.len() >= 3 && word_starts == 0 && gaps > (q.len() as i64 * 3) {
            return None;
        }
    }
    Some(Match { score, indices })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(q: &str, c: &str) -> Option<i64> {
        fuzzy_match(q, c, Sensitivity::Medium).map(|m| m.score)
    }

    #[test]
    fn matches_prefix_and_subsequence() {
        assert!(m("saf", "Safari").is_some());
        assert!(m("msg", "Messages").is_some());
        assert!(m("vsc", "Visual Studio Code").is_some());
        assert!(m("xyz", "Safari").is_none());
    }

    #[test]
    fn prefix_beats_middle() {
        assert!(m("term", "Terminal").unwrap() > m("term", "Determine").unwrap());
    }

    #[test]
    fn exact_beats_prefix() {
        assert!(m("notes", "Notes").unwrap() > m("notes", "Notes Helper").unwrap());
    }

    #[test]
    fn acronym_beats_scatter() {
        assert!(m("vsc", "Visual Studio Code").unwrap() > m("vsc", "Ovuscular").unwrap_or(i64::MIN));
    }

    #[test]
    fn accents_fold() {
        assert!(m("cafe", "Café").is_some());
    }

    #[test]
    fn high_sensitivity_requires_word_starts() {
        assert!(fuzzy_match("msg", "Messages", Sensitivity::High).is_none());
        assert!(fuzzy_match("vsc", "Visual Studio Code", Sensitivity::High).is_some());
    }

    #[test]
    fn indices_are_reported() {
        let r = fuzzy_match("vsc", "Visual Studio Code", Sensitivity::Low).unwrap();
        assert_eq!(r.indices, vec![0, 7, 14]);
    }
}
