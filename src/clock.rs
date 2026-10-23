use std::sync::RwLock;

use chrono::{Datelike, Duration, Local, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;

use crate::error::{Error, Result};

/// The zone every day, time, and API request is read in; None follows the system clock.
static ZONE: RwLock<Option<Tz>> = RwLock::new(None);

pub fn set_zone(zone: Option<Tz>) {
    if let Ok(mut slot) = ZONE.write() {
        *slot = zone;
    }
}

pub fn zone() -> Option<Tz> {
    ZONE.read().ok().and_then(|slot| *slot)
}

pub fn parse_zone(name: &str) -> Result<Tz> {
    name.trim().parse::<Tz>().map_err(|_| {
        Error::usage(format!(
            "unknown time zone {name}, use an IANA name like America/New_York"
        ))
    })
}

/// The IANA name sent with reads and writes: the zone days and times are shown in.
pub fn zone_name() -> String {
    zone()
        .map(|tz| tz.name().to_owned())
        .unwrap_or_else(|| iana_time_zone::get_timezone().unwrap_or_else(|_| "UTC".into()))
}

pub fn screen_zone() -> Option<Tz> {
    zone().or_else(|| parse_zone(&zone_name()).ok())
}

/// Hours from midnight of `filed` in `own` to `start` hours past midnight of `shown` in `screen`.
pub fn rebase(shown: NaiveDate, start: f64, screen: Tz, filed: NaiveDate, own: Tz) -> Option<f64> {
    let at = screen
        .from_local_datetime(&shown.and_hms_opt(0, 0, 0)?)
        .earliest()?
        + Duration::seconds((start * 3600.0).round() as i64);
    let base = own
        .from_local_datetime(&filed.and_hms_opt(0, 0, 0)?)
        .earliest()?;
    Some((at - base).num_seconds() as f64 / 3600.0)
}

pub fn now() -> NaiveDateTime {
    match zone() {
        Some(tz) => Utc::now().with_timezone(&tz).naive_local(),
        None => Local::now().naive_local(),
    }
}

pub fn today() -> NaiveDate {
    now().date()
}

/// Whole minutes, so the now-line reads the same minute as the header clock.
pub fn now_hours() -> f64 {
    f64::from(chrono::Timelike::num_seconds_from_midnight(&now().time()) / 60) / 60.0
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
    fn rebase_moves_a_time_between_zones_and_filed_days() {
        let day = |d| NaiveDate::from_ymd_opt(2026, 10, d).unwrap();
        let (et, saigon) = (chrono_tz::America::New_York, chrono_tz::Asia::Ho_Chi_Minh);
        assert_eq!(rebase(day(23), 8.0, et, day(24), saigon), Some(-5.0));
        assert_eq!(rebase(day(24), 8.0, saigon, day(24), saigon), Some(8.0));
    }

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
