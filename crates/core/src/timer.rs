//! Natural timer queries for Root Search: `timer 10m`, `25 min timer`, `pomodoro`,
//! `set a timer for 1h 30m`.

/// A parsed timer request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimerSpec {
    pub secs: u64,
    /// What the user called it ("Pomodoro", or empty).
    pub label: String,
}

fn unit_secs(u: &str) -> Option<u64> {
    Some(match u {
        "s" | "sec" | "secs" | "second" | "seconds" => 1,
        "m" | "min" | "mins" | "minute" | "minutes" => 60,
        "h" | "hr" | "hrs" | "hour" | "hours" => 3600,
        _ => return None,
    })
}

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
    let mut total = 0u64;
    let mut num: Option<u64> = None;
    for tok in q.replace(',', " ").split_whitespace() {
        if matches!(tok, "timer" | "countdown" | "set" | "a" | "for" | "start" | "in" | "and") {
            continue;
        }
        // "10m" / "1h30m" style tokens
        let mut rest = tok;
        let mut consumed = false;
        while !rest.is_empty() {
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if digits.is_empty() {
                break;
            }
            let after = &rest[digits.len()..];
            let unit: String = after.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
            let n: u64 = digits.parse().ok()?;
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
        total += n * 60; // a bare number means minutes: "timer 10"
    }
    if total == 0 || total > 24 * 3600 {
        return None;
    }
    Some(TimerSpec { secs: total, label: String::new() })
}

/// `1h 30m`, `10 min`, `45 sec`.
pub fn describe(secs: u64) -> String {
    let (h, m, s) = (secs / 3600, secs % 3600 / 60, secs % 60);
    let mut parts = vec![];
    if h > 0 {
        parts.push(format!("{h}h"));
    }
    if m > 0 {
        parts.push(format!("{m} min"));
    }
    if s > 0 && h == 0 {
        parts.push(format!("{s} sec"));
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
    fn formats() {
        assert_eq!(describe(5400), "1h 30 min");
        assert_eq!(describe(45), "45 sec");
        assert_eq!(clock(5400), "1:30:00");
        assert_eq!(clock(65), "1:05");
    }
}
