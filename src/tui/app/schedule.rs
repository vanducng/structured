use chrono::{Duration, NaiveDate};

use crate::clock;

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
    if text.is_empty() {
        return Err("date required".into());
    }
    if text.eq_ignore_ascii_case("inbox") {
        return Ok(DateChoice::Inbox);
    }
    clock::parse_day(text, anchor)
        .map(|day| DateChoice::Day(clock::stamp(day)))
        .ok_or_else(|| "use YYYY-MM-DD, today, tomorrow, +1, -1, or inbox".into())
}

pub(super) fn parse_clock(text: &str) -> Result<ClockChoice, String> {
    let text = text.trim();
    if text.is_empty() || text.eq_ignore_ascii_case("all") {
        return Ok(ClockChoice::AllDay);
    }
    let mut parts = text.split_whitespace();
    let start = clock::parse_clock(parts.next().unwrap_or("")).map_err(|err| err.to_string())?;
    let duration = match parts.next() {
        Some(mins) => Some(parse_duration(mins)?),
        None => None,
    };
    if parts.next().is_some() {
        return Err("use HH:MM or HH:MM minutes".into());
    }
    Ok(ClockChoice::At { start, duration })
}

pub(super) fn shift_date(text: &str, anchor: NaiveDate, days: i64) -> String {
    let day = match parse_date(text, anchor) {
        Ok(DateChoice::Day(day)) => clock::parse_stamp(&day).unwrap_or(anchor),
        _ => anchor,
    };
    clock::stamp(day + Duration::days(days))
}

pub(super) fn parse_duration(text: &str) -> Result<i64, String> {
    let text = text.trim().to_ascii_lowercase().replace(' ', "");
    if text.is_empty() {
        return Ok(0);
    }
    let body = text.strip_suffix('m').unwrap_or(&text);
    let minutes = if let Ok(mins) = body.parse::<i64>() {
        mins
    } else if let Some((hours, mins)) = body.split_once('h').or_else(|| body.split_once(':')) {
        let hours = hours.parse::<i64>().map_err(|_| minutes_err())?;
        let mins = if mins.is_empty() {
            0
        } else {
            mins.parse::<i64>().map_err(|_| minutes_err())?
        };
        hours * 60 + mins
    } else {
        return Err(minutes_err());
    };
    if (0..=1439).contains(&minutes) {
        Ok(minutes)
    } else {
        Err(minutes_err())
    }
}

pub(super) fn shift_duration(text: &str, minutes: i64) -> String {
    let current = parse_duration(text).unwrap_or(0);
    (current + minutes).clamp(0, 1439).to_string()
}

fn minutes_err() -> String {
    "duration must be under 24h, like 45 or 1h30".into()
}

pub(super) fn shift_clock(text: &str, minutes: i64) -> String {
    let (start, duration) = match parse_clock(text) {
        Ok(ClockChoice::At { start, duration }) => (start, duration),
        _ => (9.0, None),
    };
    let clock = clock::clock(start + minutes as f64 / 60.0);
    match duration {
        Some(mins) => format!("{clock} {mins}"),
        None => clock,
    }
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
        assert!(
            matches!(parse_date("+1", day()), Ok(DateChoice::Day(value)) if value == "2026-10-24")
        );
        assert!(
            matches!(parse_date("2026-11-01", day()), Ok(DateChoice::Day(value)) if value == "2026-11-01")
        );
        assert!(parse_date("monday", day()).is_err());
        assert!(parse_date("", day()).is_err());
    }

    #[test]
    fn parses_and_shifts_time() {
        match parse_clock("9:30 1h") {
            Ok(ClockChoice::At { start, duration }) => {
                assert!((start - 9.5).abs() < 0.001);
                assert_eq!(duration, Some(60));
            }
            _ => panic!("timed"),
        }
        assert!(matches!(parse_clock(""), Ok(ClockChoice::AllDay)));
        assert_eq!(shift_clock("09:30", 15), "09:45");
        assert_eq!(shift_clock("23:50", 15), "00:05");
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
        assert!(parse_duration("1440").is_err());
        assert_eq!(shift_duration("45", 15), "60");
        assert_eq!(shift_duration("", 15), "15");
        assert_eq!(shift_duration("0", -15), "0");
    }
}
