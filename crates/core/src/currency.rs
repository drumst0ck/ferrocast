//! Currency conversion from ECB reference rates (frankfurter.dev JSON).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const RATES_URL: &str = "https://api.frankfurter.dev/v1/latest";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Rates {
    pub base: String,
    pub date: String,
    pub rates: HashMap<String, f64>,
    /// Unix seconds when fetched.
    pub fetched: i64,
}

impl Rates {
    /// Parse `{"base":"EUR","date":"…","rates":{"USD":1.08,…}}`.
    pub fn parse(json: &str, now: i64) -> Option<Rates> {
        let v: serde_json::Value = serde_json::from_str(json).ok()?;
        let base = v.get("base")?.as_str()?.to_uppercase();
        let rates: HashMap<String, f64> = v
            .get("rates")?
            .as_object()?
            .iter()
            .filter_map(|(k, r)| Some((k.to_uppercase(), r.as_f64()?)))
            .filter(|(_, r)| *r > 0.0)
            .collect();
        if rates.is_empty() {
            return None;
        }
        Some(Rates { base, date: v.get("date").and_then(|d| d.as_str()).unwrap_or("").to_string(), rates, fetched: now })
    }

    pub fn is_stale(&self, now: i64) -> bool {
        self.rates.is_empty() || now - self.fetched > 12 * 3600
    }

    pub fn knows(&self, code: &str) -> bool {
        self.rate(code).is_some()
    }

    fn rate(&self, code: &str) -> Option<f64> {
        let c = code.to_uppercase();
        if c == self.base {
            Some(1.0)
        } else {
            self.rates.get(&c).copied()
        }
    }

    pub fn convert(&self, value: f64, from: &str, to: &str) -> Option<f64> {
        Some(value / self.rate(from)? * self.rate(to)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rates() -> Rates {
        Rates::parse(r#"{"amount":1.0,"base":"EUR","date":"2026-10-14","rates":{"USD":1.25,"GBP":0.8,"bad":0}}"#, 100).unwrap()
    }

    #[test]
    fn converts_through_base() {
        let r = rates();
        assert!((r.convert(100.0, "usd", "eur").unwrap() - 80.0).abs() < 1e-9);
        assert!((r.convert(100.0, "EUR", "GBP").unwrap() - 80.0).abs() < 1e-9);
        assert!((r.convert(125.0, "USD", "GBP").unwrap() - 80.0).abs() < 1e-9);
        assert!(r.convert(1.0, "USD", "XXX").is_none());
    }

    #[test]
    fn rejects_garbage_and_zero_rates() {
        assert!(Rates::parse("not json", 0).is_none());
        assert!(Rates::parse(r#"{"base":"EUR","rates":{}}"#, 0).is_none());
        assert!(!rates().knows("BAD"));
    }

    #[test]
    fn staleness() {
        let r = rates();
        assert!(!r.is_stale(100 + 3600));
        assert!(r.is_stale(100 + 13 * 3600));
        assert!(Rates::default().is_stale(0));
    }
}
