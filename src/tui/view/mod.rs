mod detail;
mod footer;
mod header;
mod list;
mod overlay;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::config::PaneLayout;

use super::app::{App, Mode};
use super::theme;

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let [header_area, _, body, footer_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .areas(area);
    header::draw(frame, header_area, app);
    let side = app.config.layout == PaneLayout::Side && area.width >= 72;
    let (list_area, detail_area) = if side {
        let detail = if area.width < 100 {
            app.config.detail_percent.min(40)
        } else {
            app.config.detail_percent
        };
        let [list, detail] = Layout::horizontal([
            Constraint::Percentage(100 - detail),
            Constraint::Percentage(detail),
        ])
        .areas(body);
        (list, detail)
    } else {
        let [list, detail] =
            Layout::vertical([Constraint::Percentage(55), Constraint::Percentage(45)]).areas(body);
        (list, detail)
    };
    list::draw(frame, list_area, app);
    detail::draw(frame, detail_area, app);
    footer::draw(frame, footer_area, app);
    let behind = Rect {
        height: area.height.saturating_sub(1),
        ..area
    };
    match app.mode {
        Mode::Help => overlay::help(frame, behind),
        Mode::Confirm(doomed) => overlay::confirm(frame, behind, app, doomed),
        _ => {}
    }
}

/// Splits `text` so every case-insensitive match of `query` is underlined in the warning colour.
pub(super) fn marked(text: &str, query: &str, base: Style) -> Vec<Span<'static>> {
    let lower = text.to_lowercase();
    let needle = query.to_lowercase();
    if needle.is_empty() || lower.len() != text.len() {
        return vec![Span::styled(text.to_owned(), base)];
    }
    let hit = base.fg(theme::WARN).add_modifier(Modifier::UNDERLINED);
    let mut spans = Vec::new();
    let mut at = 0;
    for (start, _) in lower.match_indices(&needle) {
        if start < at {
            continue;
        }
        if start > at {
            spans.push(Span::styled(text[at..start].to_owned(), base));
        }
        spans.push(Span::styled(
            text[start..start + needle.len()].to_owned(),
            hit,
        ));
        at = start + needle.len();
    }
    if at < text.len() {
        spans.push(Span::styled(text[at..].to_owned(), base));
    }
    spans
}

pub(super) fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

pub(super) fn width(spans: &[Span]) -> usize {
    spans.iter().map(|span| span.content.width()).sum()
}

pub(super) fn fit(text: &str, max: usize) -> String {
    if text.width() <= max {
        return text.to_owned();
    }
    let mut out = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let w = ch.width().unwrap_or(0);
        if used + w + 1 > max {
            break;
        }
        out.push(ch);
        used += w;
    }
    if max > 0 {
        out.push('…');
    }
    out
}

pub(super) fn wrap(text: &str, max: usize) -> Vec<String> {
    super::wrap::rows(text, max)
        .into_iter()
        .map(|row| text[row].to_owned())
        .collect()
}

pub(super) fn pad(spans: &mut Vec<Span<'static>>, total: usize) {
    let used = width(spans);
    if used < total {
        spans.push(Span::raw(" ".repeat(total - used)));
    }
}

pub(super) fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width.saturating_sub(2));
    let height = height.min(area.height.saturating_sub(2));
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

pub(super) fn placeholder(
    lines: Vec<Line<'static>>,
    area: Rect,
) -> (ratatui::widgets::Paragraph<'static>, Rect) {
    let height = lines.len() as u16;
    let top = area.y + area.height.saturating_sub(height) / 2;
    let rect = Rect {
        x: area.x,
        y: top,
        width: area.width,
        height: height.min(area.height),
    };
    (ratatui::widgets::Paragraph::new(lines).centered(), rect)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_and_wraps_by_display_width() {
        assert_eq!(fit("hello", 10), "hello");
        assert_eq!(fit("hello world", 6), "hello…");
        assert_eq!(wrap("one two three", 7), ["one two", "three"]);
        assert_eq!(wrap("abcdefghij", 4), ["abcd", "efgh", "ij"]);
        assert_eq!(wrap("a\nb", 10), ["a", "b"]);
    }
}
