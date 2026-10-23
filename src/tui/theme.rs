use ratatui::style::{Color, Modifier, Style};

pub const ACCENT: Color = Color::Rgb(98, 148, 222);
pub const TEXT: Color = Color::Rgb(226, 228, 233);
pub const MUTED: Color = Color::Rgb(140, 144, 156);
pub const FAINT: Color = Color::Rgb(92, 96, 108);
pub const BORDER: Color = Color::Rgb(64, 68, 80);
pub const SELECTED: Color = Color::Rgb(44, 50, 64);
pub const SELECTED_DIM: Color = Color::Rgb(36, 39, 48);
pub const GOOD: Color = Color::Rgb(120, 196, 132);
pub const WARN: Color = Color::Rgb(232, 184, 92);
pub const BAD: Color = Color::Rgb(236, 104, 104);
pub const INK: Color = Color::Rgb(24, 24, 28);

pub const OPEN: &str = "○";
pub const DONE: &str = "✓";
pub const REPEAT: &str = "↻";
pub const BOX: &str = "☐";
pub const TICKED: &str = "☑";
pub const BAR: &str = "▍";

pub fn text() -> Style {
    Style::new().fg(TEXT)
}

pub fn muted() -> Style {
    Style::new().fg(MUTED)
}

pub fn faint() -> Style {
    Style::new().fg(FAINT)
}

pub fn bold() -> Style {
    Style::new().fg(TEXT).add_modifier(Modifier::BOLD)
}

pub fn done_title() -> Style {
    Style::new().fg(FAINT).add_modifier(Modifier::CROSSED_OUT)
}

pub fn chip(bg: Color) -> Style {
    Style::new().fg(INK).bg(bg).add_modifier(Modifier::BOLD)
}

pub fn task_color(name: Option<&str>) -> Color {
    name.map_or(ACCENT, accent)
}

pub fn accent(name: &str) -> Color {
    match name.trim().to_ascii_lowercase().as_str() {
        "day" => Color::Rgb(240, 147, 67),
        "dawn" => Color::Rgb(232, 140, 128),
        "sunshine" => Color::Rgb(240, 196, 80),
        "nature" | "forest" => Color::Rgb(96, 168, 98),
        "night" => Color::Rgb(108, 142, 206),
        "twilight" => Color::Rgb(156, 112, 176),
        "midnight" => Color::Rgb(86, 102, 168),
        "classic" => Color::Rgb(150, 150, 150),
        _ => parse_hex(name).unwrap_or(ACCENT),
    }
}

fn parse_hex(text: &str) -> Option<Color> {
    let hex = text.trim().trim_start_matches('#');
    let value = u32::from_str_radix(hex, 16).ok()?;
    let (red, green, blue) = match hex.len() {
        6 => ((value >> 16) & 0xff, (value >> 8) & 0xff, value & 0xff),
        3 => (
            ((value >> 8) & 0xf) * 17,
            ((value >> 4) & 0xf) * 17,
            (value & 0xf) * 17,
        ),
        _ => return None,
    };
    Some(Color::Rgb(red as u8, green as u8, blue as u8))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_named_and_hex_colors() {
        assert_eq!(accent("forest"), Color::Rgb(96, 168, 98));
        assert_eq!(accent("#ED732E"), Color::Rgb(0xed, 0x73, 0x2e));
        assert_eq!(accent("#F80"), Color::Rgb(0xff, 0x88, 0x00));
        assert_eq!(accent("nonsense"), ACCENT);
    }
}
