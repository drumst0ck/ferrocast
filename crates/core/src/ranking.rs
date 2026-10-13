//! Root Search ranking: alias exact > alias prefix > fuzzy title > subtitle /
//! keywords, then frecency and learned query→item choices.

use crate::fuzzy::{fuzzy_match, Sensitivity};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Anything that can be ranked in Root Search.
#[derive(Debug, Clone, Default)]
pub struct Rankable<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub subtitle: &'a str,
    pub keywords: &'a [String],
    pub alias: Option<&'a str>,
}

/// Usage history of a single item.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    pub count: u32,
    /// Unix seconds of the most recent uses (newest last, capped).
    pub recent: Vec<i64>,
}

const MAX_RECENT: usize = 10;

/// Persisted frecency data.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Frecency {
    pub items: HashMap<String, Usage>,
    /// query (lowercased, trimmed) → item id → times chosen.
    pub choices: HashMap<String, HashMap<String, u32>>,
}

impl Frecency {
    /// Record that `id` was picked for `query` at `now` (unix seconds).
    pub fn record(&mut self, id: &str, query: &str, now: i64) {
        let u = self.items.entry(id.to_string()).or_default();
        u.count = u.count.saturating_add(1);
        u.recent.push(now);
        if u.recent.len() > MAX_RECENT {
            let drop = u.recent.len() - MAX_RECENT;
            u.recent.drain(..drop);
        }
        let q = query.trim().to_lowercase();
        if !q.is_empty() {
            // Learn every prefix so shorter queries benefit too (`c`, `ca`, `cal`).
            let chars: Vec<char> = q.chars().collect();
            for end in 1..=chars.len() {
                let p: String = chars[..end].iter().collect();
                *self.choices.entry(p).or_default().entry(id.to_string()).or_default() += 1;
            }
        }
    }

    /// Clear learned data for an item ("Reset Ranking").
    pub fn reset(&mut self, id: &str) {
        self.items.remove(id);
        for m in self.choices.values_mut() {
            m.remove(id);
        }
        self.choices.retain(|_, m| !m.is_empty());
    }

    /// Frequency weighted by recency buckets (Mozilla-style frecency).
    pub fn score(&self, id: &str, now: i64) -> f64 {
        let Some(u) = self.items.get(id) else { return 0.0 };
        if u.recent.is_empty() {
            return 0.0;
        }
        let weight: f64 = u
            .recent
            .iter()
            .map(|&t| {
                let age_h = ((now - t).max(0) as f64) / 3600.0;
                match age_h {
                    a if a < 4.0 => 100.0,
                    a if a < 24.0 => 80.0,
                    a if a < 24.0 * 7.0 => 60.0,
                    a if a < 24.0 * 30.0 => 40.0,
                    a if a < 24.0 * 90.0 => 20.0,
                    _ => 10.0,
                }
            })
            .sum();
        u.count as f64 * weight / u.recent.len() as f64
    }

    pub fn choice_count(&self, query: &str, id: &str) -> u32 {
        let q = query.trim().to_lowercase();
        self.choices.get(&q).and_then(|m| m.get(id)).copied().unwrap_or(0)
    }

    /// Last time an item was used.
    pub fn last_used(&self, id: &str) -> Option<i64> {
        self.items.get(id).and_then(|u| u.recent.last().copied())
    }
}

/// Score an item for a query, or `None` if it does not match.
pub fn score(item: &Rankable, query: &str, frecency: &Frecency, now: i64, sens: Sensitivity) -> Option<f64> {
    let q = query.trim();
    let mut base: f64;
    if q.is_empty() {
        base = 0.0;
    } else if let Some(alias) = item.alias.filter(|a| !a.is_empty()) {
        let al = alias.to_lowercase();
        let ql = q.to_lowercase();
        if al == ql {
            base = 1_000_000.0;
        } else if al.starts_with(&ql) {
            base = 500_000.0;
        } else {
            base = match_text(item, q, sens)?;
        }
    } else {
        base = match_text(item, q, sens)?;
    }
    let f = frecency.score(item.id, now);
    base += f.min(5000.0) * 0.6;
    base += frecency.choice_count(q, item.id).min(50) as f64 * 400.0;
    Some(base)
}

fn match_text(item: &Rankable, q: &str, sens: Sensitivity) -> Option<f64> {
    if let Some(m) = fuzzy_match(q, item.title, sens) {
        return Some(10_000.0 + m.score as f64 * 4.0);
    }
    // Per-word match on title (e.g. "code" in "Visual Studio Code" handled above,
    // but "studio vis" style token reorder here).
    let tokens: Vec<&str> = q.split_whitespace().collect();
    if tokens.len() > 1 && tokens.iter().all(|t| fuzzy_match(t, item.title, sens).is_some()) {
        return Some(8_000.0);
    }
    let mut best: Option<i64> = None;
    for k in item.keywords.iter().map(String::as_str).chain(std::iter::once(item.subtitle)) {
        if k.is_empty() {
            continue;
        }
        if let Some(m) = fuzzy_match(q, k, sens) {
            best = Some(best.map_or(m.score, |b| b.max(m.score)));
        }
    }
    best.map(|s| 2_000.0 + s as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item<'a>(id: &'a str, title: &'a str, alias: Option<&'a str>, kw: &'a [String]) -> Rankable<'a> {
        Rankable { id, title, subtitle: "", keywords: kw, alias }
    }

    #[test]
    fn alias_exact_wins() {
        let f = Frecency::default();
        let kw = vec![];
        let chrome = item("chrome", "Google Chrome", Some("gc"), &kw);
        let gcal = item("gcal", "gcal", None, &kw);
        let a = score(&chrome, "gc", &f, 0, Sensitivity::Medium).unwrap();
        let b = score(&gcal, "gc", &f, 0, Sensitivity::Medium).unwrap();
        assert!(a > b);
    }

    #[test]
    fn learns_choice_for_query() {
        let mut f = Frecency::default();
        let kw = vec![];
        let calendar = item("calendar", "Calendar", None, &kw);
        let calculator = item("calculator", "Calculator", None, &kw);
        let now = 1_700_000_000;
        let before_a = score(&calendar, "cal", &f, now, Sensitivity::Medium).unwrap();
        let before_b = score(&calculator, "cal", &f, now, Sensitivity::Medium).unwrap();
        assert!(before_a >= before_b); // shorter title wins initially
        for _ in 0..3 {
            f.record("calculator", "cal", now);
        }
        let a = score(&calendar, "ca", &f, now, Sensitivity::Medium).unwrap();
        let b = score(&calculator, "ca", &f, now, Sensitivity::Medium).unwrap();
        assert!(b > a);
        f.reset("calculator");
        assert_eq!(f.choice_count("ca", "calculator"), 0);
    }

    #[test]
    fn keyword_match() {
        let f = Frecency::default();
        let kw = vec!["preferences".to_string()];
        let s = item("settings", "System Settings", None, &kw);
        assert!(score(&s, "prefer", &f, 0, Sensitivity::Medium).is_some());
    }

    #[test]
    fn recency_decays() {
        let mut f = Frecency::default();
        f.record("a", "", 0);
        f.record("b", "", 1_000_000);
        assert!(f.score("b", 1_000_000) > f.score("a", 1_000_000));
    }
}
