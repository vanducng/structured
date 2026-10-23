use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::width;
use crate::clock;
use crate::tui::app::App;
use crate::tui::theme;

pub(super) fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let day = app.viewed_day();
    let mut left = vec![Span::raw(" ")];
    if app.inbox {
        left.push(Span::styled("Inbox", theme::bold()));
        if let Some(open) = app.inbox_open() {
            left.push(Span::styled(format!("  {open} open"), theme::muted()));
        }
    } else {
        let long = if area.width >= 110 {
            "%A %-d %B %Y"
        } else {
            "%a %-d %b"
        };
        left.push(Span::styled(day.format(long).to_string(), theme::bold()));
        if let Some(word) = clock::relative(day) {
            left.push(Span::styled(
                format!("  {}", super::capitalize(&word)),
                Style::new().fg(theme::ACCENT),
            ));
        }
    }
    let budget = (area.width as usize).saturating_sub(width(&left) + 2);
    let right = [true, false]
        .into_iter()
        .map(|names| strip(app, day, names))
        .find(|spans| width(spans) <= budget)
        .unwrap_or_default();
    let gap = (area.width as usize).saturating_sub(width(&left) + width(&right));
    left.push(Span::raw(" ".repeat(gap)));
    left.extend(right);
    frame.render_widget(Paragraph::new(Line::from(left)), area);
}

fn strip(app: &App, viewed: chrono::NaiveDate, names: bool) -> Vec<Span<'static>> {
    let today = clock::today();
    let mut spans = Vec::new();
    for day in clock::week_of(viewed) {
        let label = if names {
            day.format("%a %-d").to_string()
        } else {
            day.format("%-d").to_string()
        };
        let style = if day == viewed && !app.inbox {
            theme::chip(theme::ACCENT)
        } else if day == today {
            Style::new().fg(theme::ACCENT).add_modifier(Modifier::BOLD)
        } else {
            theme::muted()
        };
        spans.push(Span::styled(format!(" {label} "), style));
    }
    spans.push(Span::styled("  │ ", theme::faint()));
    let inbox = match app.inbox_open() {
        Some(open) => format!(" Inbox {open} "),
        None => " Inbox ".into(),
    };
    spans.push(Span::styled(
        inbox,
        if app.inbox {
            theme::chip(theme::ACCENT)
        } else {
            theme::muted()
        },
    ));
    spans.push(Span::raw(" "));
    spans
}
