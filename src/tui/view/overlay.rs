use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};

use super::{centered, fit};
use crate::clock;
use crate::tui::app::{App, Doomed};
use crate::tui::theme;

type Pair = (&'static str, &'static str);

const KEYS: &[(&str, Pair, Pair)] = &[
    ("Move", ("j k ↑ ↓", "move"), ("gg  G", "top, bottom")),
    ("", ("ctrl-d/u", "page"), ("", "")),
    ("Views", ("t", "today"), ("i", "inbox")),
    ("", ("w", "day or 24h view"), ("tab", "today or inbox")),
    ("", ("[ ]  ← →", "prev, next day"), ("z", "next time zone")),
    ("Find", ("/  f", "search"), ("n  N", "next, previous")),
    ("", ("esc", "clear the search"), ("", "")),
    ("Task", ("x  space", "done or not"), ("a", "add")),
    ("", ("e", "edit"), ("d", "delete")),
    ("Schedule", ("D", "date"), ("T", "time")),
    ("", ("m", "minutes"), (",  .", "move a day")),
    ("Subtasks", ("enter  l", "open"), ("A", "add from list")),
    ("", ("x", "tick"), ("a", "add")),
    ("", ("e", "edit"), ("d", "delete")),
    ("", ("J  K", "move down, up"), ("h  esc", "back")),
    ("Editing", ("tab", "next section"), ("esc", "save, close")),
    (
        "More",
        ("ctrl-g", "note in $EDITOR"),
        ("J  K", "scroll details"),
    ),
    ("", ("r", "refresh"), ("q", "quit")),
];

pub(super) fn help(frame: &mut Frame, area: Rect) {
    let mut lines = vec![Line::default()];
    for (group, left, right) in KEYS {
        let mut spans = vec![Span::styled(
            format!("  {group:<10}"),
            Style::new().fg(theme::ACCENT).add_modifier(Modifier::BOLD),
        )];
        for (key, what) in [left, right] {
            spans.push(Span::styled(format!("{key:<10}"), theme::bold()));
            spans.push(Span::styled(format!("{what:<18}"), theme::muted()));
        }
        lines.push(Line::from(spans));
    }
    lines.push(Line::default());
    lines.push(Line::styled(
        "  Saves run in the background. A failed save puts the old value back.",
        theme::muted(),
    ));
    lines.push(Line::default());
    let height = lines.len() as u16 + 2;
    let rect = if height + 2 >= area.height {
        area
    } else {
        centered(area, 76, height)
    };
    modal(frame, area, rect, " Keys ", theme::ACCENT, lines);
}

pub(super) fn confirm(frame: &mut Frame, area: Rect, app: &App, doomed: Doomed) {
    let Some(task) = app.selected_task() else {
        return;
    };
    let rect = centered(area, 56, 8);
    let room = rect.width.saturating_sub(6) as usize;
    let (title, name, detail) = match doomed {
        Doomed::Task => {
            let mut detail = task
                .day
                .as_deref()
                .and_then(clock::parse_stamp)
                .map(clock::long_date)
                .unwrap_or_else(|| "Inbox".into());
            if task.is_recurring {
                detail.push_str(" · this day only");
            } else if task
                .extra
                .get("external_calendar_id")
                .is_some_and(|id| !id.is_null())
            {
                detail.push_str(" · hides it here, the calendar event stays");
            }
            (" Delete task ", task.title.clone(), detail)
        }
        Doomed::Subtask => {
            let name = task
                .subtasks
                .get(app.sub_selected)
                .map(|sub| sub.title.clone())
                .unwrap_or_default();
            (" Delete subtask ", name, format!("from {}", task.title))
        }
    };
    let lines = vec![
        Line::default(),
        Line::styled(format!("  {}", fit(&name, room)), theme::bold()),
        Line::styled(format!("  {}", fit(&detail, room)), theme::muted()),
        Line::default(),
        Line::from(vec![
            Span::styled(
                "  y",
                Style::new().fg(theme::BAD).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" delete    ", theme::muted()),
            Span::styled("any other key", theme::bold()),
            Span::styled(" keeps it", theme::muted()),
        ]),
        Line::default(),
    ];
    modal(frame, area, rect, title, theme::BAD, lines);
}

/// Dims everything behind the modal so the fragments around it read as background.
fn modal(
    frame: &mut Frame,
    area: Rect,
    rect: Rect,
    title: &str,
    color: Color,
    lines: Vec<Line<'static>>,
) {
    let buffer = frame.buffer_mut();
    for position in area.positions() {
        if let Some(cell) = buffer.cell_mut(position) {
            cell.set_style(
                Style::new()
                    .fg(theme::FAINT)
                    .bg(Color::Reset)
                    .remove_modifier(Modifier::all().difference(Modifier::CROSSED_OUT)),
            );
        }
    }
    frame.render_widget(Clear, rect);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(color))
        .title(Line::styled(
            title.to_owned(),
            Style::new().fg(color).add_modifier(Modifier::BOLD),
        ));
    frame.render_widget(Paragraph::new(lines).block(block), rect);
}
