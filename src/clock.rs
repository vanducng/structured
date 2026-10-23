use chrono::{Datelike, Duration, Local, NaiveDate};

use crate::error::{Error, Result};

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

pub fn now_hours() -> f64 {
    let now = Local::now().time();
    f64::from(chrono::Timelike::num_seconds_from_midnight(&now)) / 3600.0
}

pub fn stamp(day: NaiveDate) -> String {
    day.format("%Y-%m-%d").to_string()
}

pub fn parse_stamp(text: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(text.trim(), "%Y-%m-%d").ok()
}

/// Words are relative to the real today; `+N`/`-N` are relative to `anchor`.
pub fn parse_day(text: &str, anchor: NaiveDate) -> Option<NaiveDate> {
    let word = text.trim().to_ascii_lowercase();
    match word.as_str() {
        "today" => return Some(today()),
        "tomorrow" => return Some(today() + Duration::days(1)),
        "yesterday" => return Some(today() - Duration::days(1)),
        _ => {}
    }
    if let Some(rest) = word.strip_prefix('+').or_else(|| word.strip_prefix('-')) {
        let days: i64 = rest.parse().ok()?;
        let days = if word.starts_with('-') { -days } else { days };
        return Some(anchor + Duration::days(days));
    }
    parse_stamp(&word)
}

pub fn day_arg(text: &str) -> Result<String> {
    parse_day(text, today()).map(stamp).ok_or_else(|| {
        Error::usage(format!(
            "day must be YYYY-MM-DD, today, tomorrow, yesterday, +N, or -N, got {text}"
        ))
    })
}

pub fn long_date(day: NaiveDate) -> String {
    day.format("%a %-d %b %Y").to_string()
}

pub fn relative(day: NaiveDate) -> Option<String> {
    match (day - today()).num_days() {
        0 => Some("today".into()),
        1 => Some("tomorrow".into()),
        -1 => Some("yesterday".into()),
        n if n > 0 => Some(format!("in {n} days")),
        n => Some(format!("{} days ago", -n)),
    }
}

pub fn week_of(day: NaiveDate) -> [NaiveDate; 7] {
    let monday = day - Duration::days(i64::from(day.weekday().num_days_from_monday()));
    std::array::from_fn(|i| monday + Duration::days(i as i64))
}

/// Server start times may run from -12 to 36 hours; wrap them onto a 24h clock.
pub fn clock(hours: f64) -> String {
    let total = ((hours * 60.0).round() as i64).rem_euclid(24 * 60);
    format!("{:02}:{:02}", total / 60, total % 60)
}

pub fn span(start: f64, minutes: i64) -> String {
    if minutes <= 0 {
        return clock(start);
    }
    format!("{}-{}", clock(start), clock(start + minutes as f64 / 60.0))
}

pub fn duration(minutes: i64) -> String {
    match (minutes / 60, minutes % 60) {
        (_, _) if minutes <= 0 => String::new(),
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    }
}

pub fn parse_clock(text: &str) -> Result<f64> {
    let bad = || {
        Error::usage(format!(
            "time must be HH:MM or HH, like 09:30 or 14, got {text}"
        ))
    };
    let (hour, minute) = text.trim().split_once(':').unwrap_or((text.trim(), "00"));
    let digits = |part: &str, widths: &[usize]| {
        widths.contains(&part.len()) && part.bytes().all(|b| b.is_ascii_digit())
    };
    if !digits(hour, &[1, 2]) || !digits(minute, &[2]) {
        return Err(bad());
    }
    let (hour, minute): (u32, u32) = (
        hour.parse().map_err(|_| bad())?,
        minute.parse().map_err(|_| bad())?,
    );
    if hour >= 24 || minute >= 60 {
        return Err(bad());
    }
    Ok(f64::from(hour) + f64::from(minute) / 60.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_clock_and_durations() {
        assert_eq!(clock(8.5), "08:30");
        assert_eq!(clock(14.0), "14:00");
        assert_eq!(clock(-1.0), "23:00");
        assert_eq!(clock(25.25), "01:15");
        assert_eq!(clock(9.999), "10:00");
        assert_eq!(span(13.0, 210), "13:00-16:30");
        assert_eq!(duration(45), "45m");
        assert_eq!(duration(60), "1h");
        assert_eq!(duration(210), "3h 30m");
        assert_eq!(duration(0), "");
    }

    #[test]
    fn parses_clock_and_days() {
        assert_eq!(parse_clock("08:30").unwrap(), 8.5);
        assert!(parse_clock("24:00").is_err());
        assert_eq!(parse_clock("9").unwrap(), 9.0);
        assert!(parse_clock("9:5").is_err());
        assert!(parse_clock("9.5").is_err());
        let anchor = NaiveDate::from_ymd_opt(2026, 10, 23).unwrap();
        assert_eq!(
            parse_day("+2", anchor).map(stamp).as_deref(),
            Some("2026-10-25")
        );
        assert_eq!(
            parse_day("2026-11-01", anchor).map(stamp).as_deref(),
            Some("2026-11-01")
        );
        assert_eq!(parse_day("today", anchor), Some(today()));
        assert!(parse_day("monday", anchor).is_none());
        assert_eq!(week_of(anchor)[0].to_string(), "2026-10-19");
    }
}
