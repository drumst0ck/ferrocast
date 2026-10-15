//! Calculator time-zone queries: `time in tokyo`, `5pm ldn in sf`.

use chrono::{DateTime, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;

const ALIASES: &[(&str, &str)] = &[
    ("ldn", "Europe/London"),
    ("uk", "Europe/London"),
    ("sf", "America/Los_Angeles"),
    ("la", "America/Los_Angeles"),
    ("nyc", "America/New_York"),
    ("ny", "America/New_York"),
    ("new york", "America/New_York"),
    ("chicago", "America/Chicago"),
    ("utc", "UTC"),
    ("gmt", "UTC"),
    ("cet", "Europe/Paris"),
    ("pst", "America/Los_Angeles"),
    ("est", "America/New_York"),
    ("jfk", "America/New_York"),
    ("lax", "America/Los_Angeles"),
    ("sao paulo", "America/Sao_Paulo"),
    ("são paulo", "America/Sao_Paulo"),
    ("dubai", "Asia/Dubai"),
    ("beijing", "Asia/Shanghai"),
];

/// City or alias → time zone. Matches the last segment of IANA names.
pub fn lookup(name: &str) -> Option<Tz> {
    let n = name.trim().to_lowercase();
    if n.is_empty() {
        return None;
    }
    if let Some((_, id)) = ALIASES.iter().find(|(a, _)| *a == n) {
        return id.parse().ok();
    }
    chrono_tz::TZ_VARIANTS.iter().find(|tz| tz.name().rsplit('/').next().is_some_and(|c| c.replace('_', " ").to_lowercase() == n)).copied()
}

fn parse_clock(t: &str) -> Option<NaiveTime> {
    let t = t.trim().to_lowercase();
    let (body, pm) = if let Some(b) = t.strip_suffix("pm") {
        (b, Some(true))
    } else if let Some(b) = t.strip_suffix("am") {
        (b, Some(false))
    } else {
        (t.as_str(), None)
    };
    let (h, m) = match body.trim().split_once(':') {
        Some((h, m)) => (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?),
        None => (body.trim().parse::<u32>().ok()?, 0),
    };
    let h = match pm {
        Some(true) if h < 12 => h + 12,
        Some(false) if h == 12 => 0,
        _ => h,
    };
    NaiveTime::from_hms_opt(h, m, 0)
}

fn show<T: TimeZone>(t: &DateTime<T>, other_day: bool) -> String
where
    T::Offset: std::fmt::Display,
{
    let clock = t.format("%-I:%M %p").to_string();
    if other_day {
        format!("{clock}, {}", t.format("%a %-d %b"))
    } else {
        clock
    }
}

/// Answer for a time-zone question: `(question, answer)`.
pub fn evaluate(input: &str, now: DateTime<Utc>) -> Option<(String, String)> {
    let s = input.trim().to_lowercase();
    // time in <city>
    if let Some(city) = s.strip_prefix("time in ").or_else(|| s.strip_prefix("now in ")) {
        let tz = lookup(city)?;
        let t = now.with_timezone(&tz);
        let day_differs = t.date_naive() != now.with_timezone(&chrono::Local).date_naive();
        return Some((format!("Time in {}", city.trim()), format!("{} {}", show(&t, day_differs), t.format("%Z"))));
    }
    // <clock> <city> in <city>
    let (lhs, to_city) = s.rsplit_once(" in ")?;
    let (clock, from_city) = lhs.split_once(' ')?;
    let clock = parse_clock(clock)?;
    let (from, to) = (lookup(from_city)?, lookup(to_city)?);
    let local_date = now.with_timezone(&from).date_naive();
    let src = from.from_local_datetime(&local_date.and_time(clock)).single()?;
    let dst = src.with_timezone(&to);
    let day_differs = dst.date_naive() != src.date_naive();
    Some((
        format!("{} {} in {}", clock.format("%-I:%M %p"), from_city.trim(), to_city.trim()),
        format!("{} {}", show(&dst, day_differs), dst.format("%Z")),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, 15, 12, 0, 0).unwrap()
    }

    #[test]
    fn city_lookup() {
        assert_eq!(lookup("Tokyo"), Some(chrono_tz::Asia::Tokyo));
        assert_eq!(lookup("new york"), Some(chrono_tz::America::New_York));
        assert_eq!(lookup("ldn"), Some(chrono_tz::Europe::London));
        assert!(lookup("notacity").is_none());
    }

    #[test]
    fn time_in_city() {
        let (_, a) = evaluate("time in tokyo", now()).unwrap();
        assert!(a.starts_with("9:00 PM"), "{a}");
    }

    #[test]
    fn converts_between_cities() {
        // January: London = UTC, San Francisco = UTC-8
        let (_, a) = evaluate("5pm ldn in sf", now()).unwrap();
        assert!(a.starts_with("9:00 AM"), "{a}");
        let (_, a) = evaluate("17:30 tokyo in utc", now()).unwrap();
        assert!(a.starts_with("8:30 AM"), "{a}");
    }

    #[test]
    fn not_timezone_queries() {
        assert!(evaluate("10ft in m", now()).is_none());
        assert!(evaluate("safari", now()).is_none());
        assert!(evaluate("time in", now()).is_none());
    }
}
