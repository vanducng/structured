use chrono::{Duration, NaiveDate};

use crate::model;

pub(super) enum Spec {
    Date(DateChoice),
    Clock(ClockChoice),
    Duration(i64),
}

pub(super) enum DateChoice {
    Day(String),
    Inbox,
}

pub(super) enum ClockChoice {
    AllDay,
    At { start: f64, duration: Option<i64> },
}

pub(super) fn parse_date(text: &str, anchor: NaiveDate) -> Result<DateChoice, String> {
    let text = text.trim();
    let word = text.to_ascii_lowercase();
    if word.is_empty() {
        return Err("date required".into());
    }
    if word == "inbox" {
        return Ok(DateChoice::Inbox);
    }
    if let Some(day) = named_day(&word) {
        return Ok(DateChoice::Day(stamp(day)));
    }
    if let Some(day) = relative_day(&word, anchor) {
        return Ok(DateChoice::Day(stamp(day)));
    }
    NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .map(|day| DateChoice::Day(stamp(day)))
        .map_err(|_| "use YYYY-MM-DD, today, +1, -1, or inbox".into())
}

pub(super) fn parse_clock(text: &str) -> Result<ClockChoice, String> {
    let text = text.trim();
    if text.is_empty() || text.eq_ignore_ascii_case("all") {
        return Ok(ClockChoice::AllDay);
    }
    let mut parts = text.split_whitespace();
    let clock = parts.next().unwrap_or("");
    let start = model::parse_clock(clock).map_err(|err| err.to_string())?;
    let duration = match parts.next() {
        Some(mins) => Some(
            mins.parse::<i64>()
                .map_err(|_| "duration is minutes".to_owned())?,
        ),
        None => None,
    };
    if parts.next().is_some() {
        return Err("use HH:MM or HH:MM minutes".into());
    }
    Ok(ClockChoice::At { start, duration })
}

pub(super) fn shift_date(text: &str, anchor: NaiveDate, days: i64) -> String {
    let day = parse_date(text, anchor)
        .ok()
        .and_then(|choice| match choice {
            DateChoice::Day(day) => NaiveDate::parse_from_str(&day, "%Y-%m-%d").ok(),
            DateChoice::Inbox => None,
        })
        .unwrap_or(anchor);
    stamp(day + Duration::days(days))
}

pub(super) fn parse_duration(text: &str) -> Result<i64, String> {
    let text = text.trim().to_ascii_lowercase().replace(' ', "");
    if text.is_empty() {
        return Ok(0);
    }
    let body = text.strip_suffix('m').unwrap_or(&text);
    if let Ok(mins) = body.parse::<i64>() {
        return nonnegative(mins);
    }
    if let Some((hours, mins)) = body.split_once('h') {
        let hours = hours.parse::<i64>().map_err(|_| minutes_err())?;
        let mins = if mins.is_empty() {
            0
        } else {
            mins.parse::<i64>().map_err(|_| minutes_err())?
        };
        return nonnegative(hours * 60 + mins);
    }
    if let Some((hours, mins)) = body.split_once(':') {
        let hours = hours.parse::<i64>().map_err(|_| minutes_err())?;
        let mins = mins.parse::<i64>().map_err(|_| minutes_err())?;
        return nonnegative(hours * 60 + mins);
    }
    Err(minutes_err())
}

pub(super) fn shift_duration(text: &str, minutes: i64) -> String {
    let current = parse_duration(text).unwrap_or(0);
    (current + minutes).max(0).to_string()
}

fn minutes_err() -> String {
    "duration is minutes, like 45 or 1h30".into()
}

fn nonnegative(mins: i64) -> Result<i64, String> {
    if mins < 0 {
        Err(minutes_err())
    } else {
        Ok(mins)
    }
}

pub(super) fn shift_clock(text: &str, minutes: i64) -> String {
    let (start, duration) = match parse_clock(text) {
        Ok(ClockChoice::At { start, duration }) => (start, duration),
        _ => (9.0, None),
    };
    let total = (start * 60.0).round() as i64 + minutes;
    let total = total.rem_euclid(24 * 60);
    let clock = format!("{:02}:{:02}", total / 60, total % 60);
    match duration {
        Some(mins) => format!("{clock} {mins}"),
        None => clock,
    }
}

fn named_day(word: &str) -> Option<NaiveDate> {
    let today = chrono::Local::now().date_naive();
    match word {
        "today" => Some(today),
        "tomorrow" => Some(today + Duration::days(1)),
        "yesterday" => Some(today + Duration::days(-1)),
        _ => None,
    }
}

fn relative_day(word: &str, anchor: NaiveDate) -> Option<NaiveDate> {
    let rest = word.strip_prefix('+').or_else(|| word.strip_prefix('-'))?;
    let magnitude: i64 = rest.parse().ok()?;
    let signed = if word.starts_with('-') {
        -magnitude
    } else {
        magnitude
    };
    Some(anchor + Duration::days(signed))
}

fn stamp(day: NaiveDate) -> String {
    day.format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 23).unwrap()
    }

    #[test]
    fn parses_dates() {
        assert!(matches!(parse_date("inbox", day()), Ok(DateChoice::Inbox)));
        assert!(matches!(
            parse_date("+1", day()),
            Ok(DateChoice::Day(value)) if value == "2026-10-24"
        ));
        assert!(matches!(
            parse_date("2026-11-01", day()),
            Ok(DateChoice::Day(value)) if value == "2026-11-01"
        ));
        assert!(parse_date("monday", day()).is_err());
    }

    #[test]
    fn parses_and_shifts_time() {
        match parse_clock("9:30 45").unwrap() {
            ClockChoice::At { start, duration } => {
                assert!((start - 9.5).abs() < 0.001);
                assert_eq!(duration, Some(45));
            }
            ClockChoice::AllDay => panic!("timed"),
        }
        assert!(matches!(parse_clock(""), Ok(ClockChoice::AllDay)));
        assert_eq!(shift_clock("09:30", 15), "09:45");
        assert_eq!(shift_date("2026-10-23", day(), -1), "2026-10-22");
    }

    #[test]
    fn parses_and_shifts_duration() {
        assert_eq!(parse_duration("45").unwrap(), 45);
        assert_eq!(parse_duration("45m").unwrap(), 45);
        assert_eq!(parse_duration("1h").unwrap(), 60);
        assert_eq!(parse_duration("1h30").unwrap(), 90);
        assert_eq!(parse_duration("1h 30m").unwrap(), 90);
        assert_eq!(parse_duration("1:30").unwrap(), 90);
        assert_eq!(parse_duration("").unwrap(), 0);
        assert!(parse_duration("-5").is_err());
        assert_eq!(shift_duration("45", 15), "60");
        assert_eq!(shift_duration("", 15), "15");
        assert_eq!(shift_duration("0", -15), "0");
    }
}
