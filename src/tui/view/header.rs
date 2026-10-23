use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::width;
use crate::agenda::{self, Layout};
use crate::clock;
use crate::tui::app::App;
use crate::tui::theme;

/// Narrow terminals give up the 24h edges first, then weekday names, then the zone clock.
pub(super) fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let day = app.viewed_day();
    let total = area.width as usize;
    let (mut left, right) = [
        (true, true, true),
        (false, true, true),
        (false, false, true),
        (false, false, false),
    ]
    .into_iter()
    .map(|(bounds, names, zone)| {
        let mut right = strip(app, day, names);
        if zone {
            right.extend(zone_clock(app));
        }
        (title(app, day, total, bounds), right)
    })
    .find(|(left, right)| width(left) + width(right) + 2 <= total)
    .unwrap_or_else(|| (title(app, day, total, false), Vec::new()));
    let gap = total.saturating_sub(width(&left) + width(&right));
    left.push(Span::raw(" ".repeat(gap)));
    left.extend(right);
    frame.render_widget(Paragraph::new(Line::from(left)), area);
}

fn title(app: &App, day: chrono::NaiveDate, total: usize, bounds: bool) -> Vec<Span<'static>> {
    let mut left = vec![Span::raw(" ")];
    if app.inbox {
        left.push(Span::styled("Inbox", theme::bold()));
        if let Some(open) = app.inbox_open() {
            left.push(Span::styled(format!("  {open} open"), theme::muted()));
        }
        return left;
    }
    let window = app.layout() == Layout::Window;
    let long = if total >= 110 && !window {
        "%A %-d %B %Y"
    } else {
        "%a %-d %b"
    };
    left.push(Span::styled(day.format(long).to_string(), theme::bold()));
    if window {
        left.push(Span::styled(
            "  Now",
            Style::new().fg(theme::ACCENT).add_modifier(Modifier::BOLD),
        ));
        if bounds {
            left.push(Span::styled(
                format!("  {}", window_bounds()),
                theme::muted(),
            ));
        }
    } else if let Some(word) = clock::relative(day) {
        left.push(Span::styled(
            format!("  {}", super::capitalize(&word)),
            Style::new().fg(theme::ACCENT),
        ));
    }
    left
}

/// "14:12 Fri to 14:12 Sat": the edges of the rolling window in local time.
fn window_bounds() -> String {
    let now = clock::now();
    let span = chrono::Duration::minutes((agenda::WINDOW_HOURS * 60.0) as i64);
    let edge = |at: chrono::NaiveDateTime| at.format("%H:%M %a").to_string();
    format!("{} to {}", edge(now - span), edge(now + span))
}

/// "│ 02:32 Saigon": the wall clock in the zone every day and time on screen is read in.
fn zone_clock(app: &App) -> Vec<Span<'static>> {
    let Some(label) = app.zone_label() else {
        return Vec::new();
    };
    vec![
        Span::styled("│ ", theme::faint()),
        Span::styled(clock::now().format("%H:%M ").to_string(), theme::bold()),
        Span::styled(format!("{label} "), Style::new().fg(theme::ACCENT)),
    ]
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
