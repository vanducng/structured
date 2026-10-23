use std::fs;

use chrono_tz::Tz;

use crate::auth;
use crate::clock;
use crate::error::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaneLayout {
    Side,
    Stack,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Zone {
    pub label: String,
    pub tz: Tz,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UiConfig {
    pub layout: PaneLayout,
    pub detail_percent: u16,
    pub window: bool,
    pub zones: Vec<Zone>,
    pub warnings: Vec<String>,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            layout: PaneLayout::Side,
            detail_percent: 46,
            window: false,
            zones: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

pub fn load() -> UiConfig {
    let path = auth::config_dir().join("config.toml");
    fs::read_to_string(path)
        .map(|text| parse(&text))
        .unwrap_or_default()
}

pub fn parse(text: &str) -> UiConfig {
    let mut config = UiConfig::default();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim().trim_matches('"');
        match key {
            "layout" if value == "stack" => config.layout = PaneLayout::Stack,
            "layout" => config.layout = PaneLayout::Side,
            "view" => config.window = value == "now",
            "detail_percent" => {
                if let Ok(percent) = value.parse::<u16>() {
                    config.detail_percent = percent.clamp(30, 70);
                }
            }
            "timezones" => {
                for entry in value.trim_matches(['[', ']']).split(',') {
                    let entry = entry.trim().trim_matches('"').trim();
                    if entry.is_empty() {
                        continue;
                    }
                    match zone(entry) {
                        Ok(zone) => config.zones.push(zone),
                        Err(err) => config.warnings.push(format!("config.toml: {err}")),
                    }
                }
            }
            _ => {}
        }
    }
    config
}

/// `LABEL=Area/City`, or a bare IANA name labelled by its city.
fn zone(entry: &str) -> Result<Zone> {
    let (label, name) = match entry.split_once('=') {
        Some((label, name)) => (label.trim().to_owned(), name.trim()),
        None => (city(entry), entry),
    };
    Ok(Zone {
        label,
        tz: clock::parse_zone(name)?,
    })
}

fn city(name: &str) -> String {
    name.rsplit('/').next().unwrap_or(name).replace('_', " ")
}

/// `--tz` takes a configured label in any case, or an IANA name.
pub fn pick_zone(zones: &[Zone], text: &str) -> Result<Tz> {
    if let Some(zone) = zones
        .iter()
        .find(|zone| zone.label.eq_ignore_ascii_case(text.trim()))
    {
        return Ok(zone.tz);
    }
    clock::parse_zone(text).map_err(|_| {
        let labels: Vec<&str> = zones.iter().map(|zone| zone.label.as_str()).collect();
        let hint = if labels.is_empty() {
            String::new()
        } else {
            format!(" or one of {}", labels.join(", "))
        };
        Error::usage(format!(
            "unknown time zone {text}; use an IANA name like America/New_York{hint}"
        ))
    })
}

/// The zone after `current` in the list, wrapping; the first when `current` is not listed.
pub fn next_zone(zones: &[Zone], current: Option<Tz>) -> Option<&Zone> {
    let at = current.and_then(|tz| zones.iter().position(|zone| zone.tz == tz));
    zones.get(at.map_or(0, |at| (at + 1) % zones.len()))
}

/// The short name for a zone on screen: its configured label, else its city.
pub fn label(zones: &[Zone], tz: Tz) -> String {
    zones
        .iter()
        .find(|zone| zone.tz == tz)
        .map(|zone| zone.label.clone())
        .unwrap_or_else(|| city(tz.name()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_layout_view_and_clamps_percent() {
        let config = parse("layout = \"stack\"\ndetail_percent = 90\nview = \"now\"\n");
        assert_eq!(config.layout, PaneLayout::Stack);
        assert_eq!(config.detail_percent, 70);
        assert!(config.window);
        assert!(!parse("").window);
        assert!(!parse("view = \"day\"").window);
    }

    #[test]
    fn parses_zones_with_labels_and_reports_bad_ones() {
        let config =
            parse("timezones = [\"ET=America/New_York\", \"Asia/Ho_Chi_Minh\", \"Mars/Base\"]\n");
        let labels: Vec<&str> = config.zones.iter().map(|z| z.label.as_str()).collect();
        assert_eq!(labels, ["ET", "Ho Chi Minh"]);
        assert_eq!(config.zones[0].tz, chrono_tz::America::New_York);
        assert_eq!(config.warnings.len(), 1);
        assert!(config.warnings[0].contains("Mars/Base"));
        assert_eq!(
            pick_zone(&config.zones, "et").unwrap(),
            chrono_tz::America::New_York
        );
        assert_eq!(
            pick_zone(&config.zones, "Europe/Paris").unwrap(),
            chrono_tz::Europe::Paris
        );
        assert!(pick_zone(&config.zones, "nowhere").is_err());
        assert_eq!(label(&config.zones, chrono_tz::Europe::Paris), "Paris");
        let after = |tz| next_zone(&config.zones, tz).map(|zone| zone.label.as_str());
        assert_eq!(
            after(Some(chrono_tz::America::New_York)),
            Some("Ho Chi Minh")
        );
        assert_eq!(after(Some(chrono_tz::Asia::Ho_Chi_Minh)), Some("ET"));
        assert_eq!(after(Some(chrono_tz::Europe::Paris)), Some("ET"));
        assert_eq!(after(None), Some("ET"));
        assert_eq!(next_zone(&[], None), None);
    }
}
