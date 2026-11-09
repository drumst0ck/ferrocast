//! Natural timer queries for Root Search: `timer 10m`, `25 min timer`, `pomodoro`,
//! `set a timer for 1h 30m`.

/// A parsed timer request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimerSpec {
    pub secs: u64,
    /// What the user called it ("Pomodoro", or empty).
    pub label: String,
}

fn unit_secs(u: &str) -> Option<f64> {
    Some(match u {
        "s" | "sec" | "secs" | "second" | "seconds" => 1.0,
        "m" | "min" | "mins" | "minute" | "minutes" => 60.0,
        "h" | "hr" | "hrs" | "hour" | "hours" => 3600.0,
        _ => return None,
    })
}

/// Longest allowed timer.
const MAX_SECS: f64 = 24.0 * 3600.0;

/// Parse a query into a timer, or `None` when it is not one. Needs the word `timer`,
/// `pomodoro` or `countdown` so arithmetic like `10m in ft` is left to the calculator.
pub fn parse(query: &str) -> Option<TimerSpec> {
    let q = query.trim().to_lowercase();
    if q == "pomodoro" || q == "pomodoro timer" || q == "timer pomodoro" {
        return Some(TimerSpec { secs: 25 * 60, label: "Pomodoro".into() });
    }
    if !(q.contains("timer") || q.contains("countdown")) {
        return None;
    }
    let mut total = 0f64;
    let mut num: Option<f64> = None;
    for tok in q.replace(',', " ").split_whitespace() {
        if matches!(tok, "timer" | "countdown" | "set" | "a" | "for" | "start" | "in" | "and") {
            continue;
        }
        // "10m" / "1.5h" / "1h30m" style tokens
        let mut rest = tok;
        let mut consumed = false;
        while !rest.is_empty() {
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
            if digits.is_empty() {
                break;
            }
            let after = &rest[digits.len()..];
            let unit: String = after.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
            let n: f64 = digits.parse().ok().filter(|n: &f64| n.is_finite())?;
            // "10 30" or "10 30m" is ambiguous: refuse instead of dropping one of the numbers.
            if num.is_some() {
                return None;
            }
            if unit.is_empty() {
                num = Some(n);
                consumed = true;
                break;
            }
            total += n * unit_secs(&unit)?;
            rest = &after[unit.len()..];
            consumed = true;
        }
        if consumed {
            continue;
        }
        if let (Some(n), Some(u)) = (num.take(), unit_secs(tok)) {
            total += n * u;
            continue;
        }
        return None;
    }
    if let Some(n) = num {
        total += n * 60.0; // a bare number means minutes: "timer 10"
    }
    if !total.is_finite() || total > MAX_SECS {
        return None;
    }
    let secs = total.round() as u64;
    if secs == 0 {
        return None;
    }
    Some(TimerSpec { secs, label: String::new() })
}

/// `1h 30m`, `10 min`, `45 sec` (in the interface language).
pub fn describe(secs: u64) -> String {
    use crate::i18n::trf;
    let (h, m, s) = (secs / 3600, secs % 3600 / 60, secs % 60);
    let mut parts = vec![];
    if h > 0 {
        parts.push(trf("{n}h", &[("n", &h.to_string())]));
    }
    if m > 0 {
        parts.push(trf("{n} min", &[("n", &m.to_string())]));
    }
    if s > 0 && h == 0 {
        parts.push(trf("{n} sec", &[("n", &s.to_string())]));
    }
    parts.join(" ")
}

/// `mm:ss` or `h:mm:ss` for a countdown display.
pub fn clock(secs: u64) -> String {
    let (h, m, s) = (secs / 3600, secs % 3600 / 60, secs % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_phrasings() {
        assert_eq!(parse("timer 10m").unwrap().secs, 600);
        assert_eq!(parse("10 min timer").unwrap().secs, 600);
        assert_eq!(parse("set a timer for 1h 30m").unwrap().secs, 5400);
        assert_eq!(parse("timer 1h30m").unwrap().secs, 5400);
        assert_eq!(parse("timer 10").unwrap().secs, 600);
        assert_eq!(parse("countdown 45 seconds").unwrap().secs, 45);
        let p = parse("pomodoro").unwrap();
        assert_eq!((p.secs, p.label.as_str()), (1500, "Pomodoro"));
    }

    #[test]
    fn rejects_non_timers_and_nonsense() {
        assert_eq!(parse("10m in ft"), None);
        assert_eq!(parse("timer"), None);
        assert_eq!(parse("timer banana"), None);
        assert_eq!(parse("timer 48h"), None);
    }

    #[test]
    fn decimals_and_ambiguous_numbers() {
        assert_eq!(parse("timer 1.5h").unwrap().secs, 5400);
        assert_eq!(parse("timer 0.5 min").unwrap().secs, 30);
        assert_eq!(parse("timer 2.5").unwrap().secs, 150);
        assert_eq!(parse("timer 10 30"), None);
        assert_eq!(parse("timer 10 30m"), None);
        assert_eq!(parse("timer 1.2.3m"), None);
    }

    #[test]
    fn huge_numbers_are_rejected_not_wrapped() {
        assert_eq!(parse("timer 18446744073709551615h"), None);
        assert_eq!(parse("timer 99999999999999999999999m"), None);
        assert_eq!(parse("timer 5124095576030431h"), None);
    }

    #[test]
    fn describe_is_translated() {
        crate::i18n::with_language(crate::i18n::Lang::Ja, || {
            assert_eq!(describe(5400), "1時間 30分");
            assert_eq!(describe(45), "45秒");
        });
    }

    #[test]
    fn formats() {
        assert_eq!(describe(5400), "1h 30 min");
        assert_eq!(describe(45), "45 sec");
        assert_eq!(clock(5400), "1:30:00");
        assert_eq!(clock(65), "1:05");
    }
}
