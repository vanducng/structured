use std::fs;

use crate::auth;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaneLayout {
    Side,
    Stack,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UiConfig {
    pub layout: PaneLayout,
    pub detail_percent: u16,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            layout: PaneLayout::Side,
            detail_percent: 46,
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
            "detail_percent" => {
                if let Ok(percent) = value.parse::<u16>() {
                    config.detail_percent = percent.clamp(30, 70);
                }
            }
            _ => {}
        }
    }
    config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_layout_and_clamps_percent() {
        let config = parse("layout = \"stack\"\ndetail_percent = 90\n");
        assert_eq!(config.layout, PaneLayout::Stack);
        assert_eq!(config.detail_percent, 70);
    }
}
