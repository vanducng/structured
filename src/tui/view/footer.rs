use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthChar;

use super::width;
use crate::tui::app::{App, Field, Focus, Mode, Prompt, Tone};
use crate::tui::theme;

type Hints = &'static [(&'static str, &'static str)];

const LIST: Hints = &[
    ("⏎", "subtasks"),
    ("x", "done"),
    ("a", "add"),
    ("f", "find"),
    ("e", "edit"),
    ("D", "date"),
    ("T", "time"),
    ("m", "min"),
    ("[ ]", "day"),
    ("?", "help"),
];
const SEARCHED: Hints = &[
    ("n N", "next"),
    ("esc", "clear"),
    ("⏎", "subtasks"),
    ("x", "done"),
    ("a", "add"),
    ("e", "edit"),
    ("?", "help"),
];
const SUBTASKS: Hints = &[
    ("x", "tick"),
    ("a", "add"),
    ("e", "edit"),
    ("d", "delete"),
    ("J K", "move"),
    ("h", "back"),
    ("?", "help"),
];
const NO_SUBTASKS: Hints = &[("a", "add a subtask"), ("h", "back"), ("?", "help")];
const FIND: Hints = &[("↑↓", "move"), ("⏎", "keep"), ("esc", "clear")];
const CONFIRM: Hints = &[("y", "delete"), ("any other key", "keeps it")];
const HELP: Hints = &[("any key", "closes")];
const PROMPT: Hints = &[("⏎", "save"), ("esc", "cancel")];
const STEP: Hints = &[("⏎", "save"), ("↑↓", "step"), ("esc", "cancel")];
const CARD_TITLE: Hints = &[
    ("tab", "next"),
    ("⏎", "subtasks"),
    ("ctrl-c", "undo"),
    ("esc", "done"),
];
const CARD_SUBTASK: Hints = &[
    ("tab", "next"),
    ("⏎", "new row"),
    ("↑↓", "row"),
    ("ctrl-c", "undo"),
    ("esc", "done"),
];
const CARD_NEW_SUBTASK: Hints = &[
    ("tab", "next"),
    ("⏎", "add, then the next"),
    ("↑↓", "row"),
    ("esc", "done"),
];
const CARD_NOTE: Hints = &[
    ("tab", "next"),
    ("⏎", "new line"),
    ("ctrl-g", "$EDITOR"),
    ("ctrl-c", "undo"),
    ("esc", "done"),
];

pub(super) fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let total = area.width as usize;
    match &app.mode {
        Mode::Input { input, prompt } => {
            let series = match prompt {
                Prompt::Time | Prompt::Duration => Some(true),
                Prompt::Day => Some(false),
                Prompt::NewTitle => None,
            };
            let mut left = chip(app, prompt.label(), series);
            left.push(Span::raw("  "));
            let right =
                rejected(app).unwrap_or_else(|| keys(if prompt.steps() { STEP } else { PROMPT }));
            let room = total
                .saturating_sub(width(&left) + width(&right) + 1)
                .max(8);
            let cursor = width(&left);
            if input.value().is_empty() {
                left.push(Span::styled(prompt.hint().to_owned(), theme::faint()));
                return render(frame, area, left, right, Some(cursor as u16));
            }
            let (shown, offset) = window(
                input.value(),
                input.visual_scroll(room),
                input.visual_cursor(),
                room,
            );
            left.push(Span::styled(shown, theme::text()));
            render(frame, area, left, right, Some((cursor + offset) as u16));
        }
        Mode::Card(card) => {
            let mut left = chip(app, card.field.label(), Some(card.field.series()));
            if let Some(right) = rejected(app) {
                return render(frame, area, left, right, None);
            }
            left.push(status(app));
            let hints = match card.field {
                Field::Title => CARD_TITLE,
                Field::Subtask(_) => CARD_SUBTASK,
                Field::NewSubtask(_) => CARD_NEW_SUBTASK,
                Field::Note => CARD_NOTE,
            };
            let room = total.saturating_sub(width(&left) + 2);
            render(frame, area, left, fitted(hints, room), None);
        }
        Mode::Filter => {
            let left = vec![
                Span::styled(" FIND ", theme::chip(theme::ACCENT)),
                Span::styled(format!(" /{}", app.query), theme::text()),
            ];
            let cursor = width(&left) as u16;
            let room = total.saturating_sub(width(&left) + 2);
            render(frame, area, left, fitted(FIND, room), Some(cursor));
        }
        Mode::Confirm(_) => {
            let left = vec![Span::styled(" DELETE ", theme::chip(theme::BAD))];
            render(frame, area, left, keys(CONFIRM), None);
        }
        Mode::Help => render(
            frame,
            area,
            vec![Span::styled(" HELP ", theme::chip(theme::MUTED))],
            keys(HELP),
            None,
        ),
        Mode::Normal => {
            let mut left = Vec::new();
            if app.focus == Focus::Subtasks {
                left.push(Span::styled(" SUBTASKS ", theme::chip(theme::ACCENT)));
            }
            left.push(status(app));
            let hints = match app.focus {
                Focus::Subtasks if app.subtask_count() == 0 => NO_SUBTASKS,
                Focus::Subtasks => SUBTASKS,
                Focus::List if !app.query.is_empty() => SEARCHED,
                Focus::List => LIST,
            };
            let room = total.saturating_sub(width(&left) + 2);
            render(frame, area, left, fitted(hints, room), None);
        }
    }
}

/// The editor's name, and on a recurring day how far the edit reaches. `series` marks series-wide edits.
fn chip(app: &App, label: &str, series: Option<bool>) -> Vec<Span<'static>> {
    let mut spans = vec![Span::styled(
        format!(" {} ", label.to_uppercase()),
        theme::chip(theme::ACCENT),
    )];
    let recurring = app.selected_task().is_some_and(|task| task.is_recurring);
    match series.filter(|_| recurring) {
        Some(true) => spans.push(Span::styled(" every repeat", Style::new().fg(theme::WARN))),
        Some(false) => spans.push(Span::styled(" this day only", theme::muted())),
        None => {}
    }
    spans
}

fn rejected(app: &App) -> Option<Vec<Span<'static>>> {
    (app.status.tone == Tone::Bad).then(|| {
        vec![Span::styled(
            format!("✗ {}  ", app.status.text),
            Style::new().fg(theme::BAD),
        )]
    })
}

/// Slices the draft by display columns so wide characters keep the cursor aligned.
pub(super) fn window(text: &str, scroll: usize, cursor: usize, room: usize) -> (String, usize) {
    let mut used = 0;
    let mut shown = String::new();
    for ch in text.chars() {
        let w = ch.width().unwrap_or(0);
        if used >= scroll && used + w <= scroll + room {
            shown.push(ch);
        }
        used += w;
    }
    (shown, cursor.saturating_sub(scroll))
}

fn status(app: &App) -> Span<'static> {
    let text = &app.status.text;
    match app.status.tone {
        _ if text.is_empty() => Span::raw(""),
        Tone::Busy => Span::styled(format!(" ● {text}"), Style::new().fg(theme::WARN)),
        Tone::Good => Span::styled(
            format!(" {} {text}", theme::DONE),
            Style::new().fg(theme::GOOD),
        ),
        Tone::Bad => Span::styled(format!(" ✗ {text}"), Style::new().fg(theme::BAD)),
        Tone::Quiet => Span::styled(format!(" · {text}"), theme::muted()),
    }
}

fn keys(hints: &[(&str, &str)]) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for (key, label) in hints {
        spans.push(Span::styled((*key).to_owned(), theme::bold()));
        spans.push(Span::styled(format!(" {label}  "), theme::muted()));
    }
    spans
}

/// Drops the least common hints first and always keeps the trailing `? help`.
fn fitted(hints: &[(&str, &str)], room: usize) -> Vec<Span<'static>> {
    let mut chosen: Vec<(&str, &str)> = hints.to_vec();
    while chosen.len() > 1 && width(&keys(&chosen)) > room {
        chosen.remove(chosen.len() - 2);
    }
    if width(&keys(&chosen)) > room {
        return Vec::new();
    }
    keys(&chosen)
}

fn render(
    frame: &mut Frame,
    area: Rect,
    left: Vec<Span<'static>>,
    right: Vec<Span<'static>>,
    cursor: Option<u16>,
) {
    let gap = (area.width as usize).saturating_sub(width(&left) + width(&right));
    let mut spans = left;
    spans.push(Span::raw(" ".repeat(gap)));
    spans.extend(right);
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
    if let Some(x) = cursor {
        frame.set_cursor_position(Position::new(
            area.x + x.min(area.width.saturating_sub(1)),
            area.y,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::window;

    #[test]
    fn window_counts_display_columns() {
        assert_eq!(window("hello", 0, 5, 10), ("hello".into(), 5));
        assert_eq!(window("日本語abc", 2, 9, 4), ("本語".into(), 7));
    }
}
