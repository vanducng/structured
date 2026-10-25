use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph};

use super::{fit, marked, pad, placeholder, wrap};
use crate::clock;
use crate::model::{Subtask, Task};
use crate::tui::app::{App, Card, Field, Focus, Mode};
use crate::tui::theme;
use crate::tui::wrap::locate;

const MARGIN: &str = "  ";
const LABEL: usize = 10;

/// Where the field being edited has its cursor, so the terminal cursor can be placed there.
struct Draft {
    line: usize,
    column: usize,
}

pub(super) fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let card = match &app.mode {
        Mode::Card(card) => Some(card),
        _ => None,
    };
    let focused = card.is_some()
        || (app.focus == Focus::Subtasks && !matches!(app.mode, Mode::Help | Mode::Confirm(_)));
    let mut title = vec![Span::styled(" Details ", theme::bold())];
    if card.is_some() {
        title.push(Span::styled("· editing ", Style::new().fg(theme::ACCENT)));
    }
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(if focused {
            theme::ACCENT
        } else {
            theme::BORDER
        }))
        .title(Line::from(title));
    let inner = block.inner(area);
    let Some(task) = app.selected_task() else {
        frame.render_widget(block, area);
        let what = if app.query.is_empty() {
            "Select a task"
        } else {
            "No match"
        };
        let (paragraph, rect) = placeholder(vec![Line::styled(what, theme::muted())], inner);
        frame.render_widget(paragraph, rect);
        return;
    };
    let total = inner.width as usize;
    app.detail_width.set(total);
    let mut draft = None;
    let title = card.filter(|card| card.field == Field::Title);
    let mut lines = band(task, title, total, &mut draft);
    lines.push(Line::default());
    properties(&mut lines, task, total);
    lines.push(Line::default());
    let selected = subtasks(&mut lines, task, app, card, total, &mut draft);
    lines.push(Line::default());
    note(&mut lines, task, &app.query, card, total, &mut draft);
    if let Ok(task_ref) = task.task_ref() {
        lines.push(Line::default());
        let room = total.saturating_sub(2 * MARGIN.len());
        let labelled = format!("ref {task_ref}");
        let text = if labelled.len() <= room {
            labelled
        } else {
            task_ref.to_string()
        };
        for text in wrap(&text, room) {
            lines.push(Line::styled(format!("{MARGIN}{text}"), theme::faint()));
        }
    }
    let height = inner.height as usize;
    let mut scroll = app.detail_scroll as usize;
    if let Some(line) = draft.as_ref().map(|draft| draft.line).or(selected) {
        if line >= scroll + height {
            scroll = line + 1 - height;
        } else if line < scroll {
            scroll = line;
        }
    }
    let scroll = scroll.min(lines.len().saturating_sub(1));
    let block = if lines.len() > scroll + height {
        block.title_bottom(Line::styled(" ↓ more · J ", theme::muted()).right_aligned())
    } else {
        block
    };
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(lines).scroll((scroll as u16, 0)), inner);
    if let Some(draft) = draft
        && draft.line >= scroll
        && draft.line < scroll + height
    {
        let x = inner.x + (draft.column as u16).min(inner.width.saturating_sub(1));
        frame.set_cursor_position(Position::new(x, inner.y + (draft.line - scroll) as u16));
    }
}

fn band(
    task: &Task,
    editing: Option<&Card>,
    total: usize,
    draft: &mut Option<Draft>,
) -> Vec<Line<'static>> {
    let base = theme::chip(theme::task_color(task.color.as_deref()));
    let title = if task.done() && editing.is_none() {
        base.add_modifier(Modifier::CROSSED_OUT)
    } else {
        base
    };
    let blank = Line::styled(" ".repeat(total), base);
    let mut lines = vec![blank.clone()];
    let room = Field::Title.width(total);
    let text = editing.map_or(task.title.as_str(), |card| card.input.value());
    if let Some(card) = editing {
        let (row, column) = locate(text, card.input.cursor(), room);
        *draft = Some(Draft {
            line: lines.len() + row,
            column: MARGIN.len() + column,
        });
    }
    for (row, text) in wrap(text, room).into_iter().enumerate() {
        let lead = if editing.is_some() && row == 0 {
            "› "
        } else {
            MARGIN
        };
        let mut spans = vec![Span::styled(lead, base), Span::styled(text, title)];
        pad(&mut spans, total);
        let last = spans.len() - 1;
        spans[last].style = base;
        lines.push(Line::from(spans));
    }
    lines.push(blank);
    lines
}

fn property(lines: &mut Vec<Line<'static>>, label: &str, value: Vec<Span<'static>>) {
    let mut spans = vec![Span::styled(
        format!("{MARGIN}{label:<LABEL$}"),
        theme::muted(),
    )];
    spans.extend(value);
    lines.push(Line::from(spans));
}

/// Picks the longest value form that fits, so narrow panes never cut a time or date mid-token.
fn fitted(options: Vec<Vec<Span<'static>>>, room: usize) -> Vec<Span<'static>> {
    let last = options.len().saturating_sub(1);
    for (index, spans) in options.into_iter().enumerate() {
        if super::width(&spans) <= room || index == last {
            return spans;
        }
    }
    Vec::new()
}

fn properties(lines: &mut Vec<Line<'static>>, task: &Task, total: usize) {
    let room = total.saturating_sub(2 * MARGIN.len() + LABEL);
    let day = task.day.as_deref().and_then(clock::parse_stamp);
    let date = match day {
        Some(day) => {
            let word =
                clock::relative(day).map(|word| Span::styled(format!(" · {word}"), theme::muted()));
            let long = Span::styled(clock::long_date(day), theme::text());
            let short = Span::styled(day.format("%a %-d %b").to_string(), theme::text());
            let mut options = Vec::new();
            if let Some(word) = &word {
                options.push(vec![long.clone(), word.clone()]);
                options.push(vec![short.clone(), word.clone()]);
            }
            options.push(vec![long]);
            options.push(vec![short]);
            fitted(options, room)
        }
        None => vec![Span::styled("Inbox", theme::text())],
    };
    property(lines, "Date", date);
    if day.is_some() {
        let time = match task.timed() {
            Some(start) => {
                let span = Span::styled(clock::span(start, task.minutes()), theme::text());
                let minutes = clock::duration(task.minutes());
                let mut options = Vec::new();
                if !minutes.is_empty() {
                    options.push(vec![
                        span.clone(),
                        Span::styled(format!(" · {minutes}"), theme::muted()),
                    ]);
                    options.push(vec![
                        span.clone(),
                        Span::styled(format!(" · {}", minutes.replace(' ', "")), theme::muted()),
                    ]);
                }
                options.push(vec![span]);
                fitted(options, room)
            }
            None => vec![Span::styled("All day", theme::text())],
        };
        property(lines, "Time", time);
    }
    if task.is_recurring {
        property(
            lines,
            "Repeats",
            vec![Span::styled(
                format!("{} Recurring", theme::REPEAT),
                theme::text(),
            )],
        );
    }
    if let Some(level) = task.energy_level.filter(|level| *level != 0) {
        property(lines, "Energy", energy(level));
    }
    if task.done() {
        property(
            lines,
            "Status",
            vec![Span::styled(
                format!("{} Done", theme::DONE),
                Style::new().fg(theme::GOOD),
            )],
        );
    } else if crate::agenda::Clock::now().late(task) {
        property(
            lines,
            "Status",
            vec![Span::styled(
                "Late, still open",
                Style::new().fg(theme::WARN),
            )],
        );
    }
    if task
        .extra
        .get("external_calendar_id")
        .is_some_and(|id| !id.is_null())
    {
        property(
            lines,
            "Calendar",
            vec![Span::styled("synced event", theme::text())],
        );
    }
}

fn energy(level: i64) -> Vec<Span<'static>> {
    if level < 0 {
        return vec![Span::styled("rest", theme::text())];
    }
    let filled = level.clamp(0, 3) as usize;
    let word = ["", "low", "medium", "high"][filled];
    vec![
        Span::styled("━━".repeat(filled), Style::new().fg(theme::WARN)),
        Span::styled("━━".repeat(3 - filled), Style::new().fg(theme::BORDER)),
        Span::styled(format!("  {word}"), theme::text()),
    ]
}

/// Returns the highlighted checklist row, so the pane can keep it on screen.
fn subtasks(
    lines: &mut Vec<Line<'static>>,
    task: &Task,
    app: &App,
    card: Option<&Card>,
    total: usize,
    draft: &mut Option<Draft>,
) -> Option<usize> {
    let focused = match card.map(|card| card.field) {
        Some(field) => matches!(field, Field::Subtask(_) | Field::NewSubtask(_)),
        None => app.focus == Focus::Subtasks && !matches!(app.mode, Mode::Help | Mode::Confirm(_)),
    };
    let heading = if focused {
        Style::new().fg(theme::ACCENT)
    } else {
        theme::muted()
    };
    let mut head = vec![Span::styled(
        format!("{MARGIN}SUBTASKS"),
        heading.add_modifier(Modifier::BOLD),
    )];
    if let Some((done, all)) = task.subtask_progress() {
        head.push(Span::styled(format!("  {done}/{all}"), theme::muted()));
    }
    lines.push(Line::from(head));
    let count = task.subtasks.len();
    let mut selected = None;
    for (index, sub) in task.subtasks.iter().enumerate() {
        if let Some(card) = card
            && card.field == Field::NewSubtask(index)
        {
            *draft = Some(draft_row(lines, card, None, total));
        }
        if let Some(card) = card
            && card.field == Field::Subtask(index)
        {
            *draft = Some(draft_row(lines, card, Some(sub), total));
            continue;
        }
        let current = card.is_none() && focused && index == app.sub_selected;
        let mut spans = vec![
            Span::styled(
                if current { "  › " } else { "    " },
                Style::new().fg(theme::ACCENT),
            ),
            if sub.done() {
                Span::styled(format!("{} ", theme::TICKED), Style::new().fg(theme::GOOD))
            } else {
                Span::styled(format!("{} ", theme::BOX), theme::muted())
            },
        ];
        let style = if sub.done() {
            theme::done_title()
        } else {
            theme::text()
        };
        spans.extend(marked(
            &fit(&sub.title, total.saturating_sub(7)),
            &app.query,
            style,
        ));
        if current {
            pad(&mut spans, total);
            spans = spans
                .into_iter()
                .map(|span| span.patch_style(Style::new().bg(theme::SELECTED)))
                .collect();
            selected = Some(lines.len());
        }
        lines.push(Line::from(spans));
    }
    match card {
        Some(card) if card.field == Field::NewSubtask(count) => {
            *draft = Some(draft_row(lines, card, None, total));
        }
        Some(card) if !matches!(card.field, Field::NewSubtask(_)) => lines.push(Line::from(vec![
            Span::styled(format!("    {} ", theme::BOX), theme::muted()),
            Span::styled("add a subtask…", theme::faint()),
        ])),
        Some(_) => {}
        None if count == 0 => {
            let hint = if focused {
                "No subtasks yet · a adds one"
            } else {
                "No subtasks · A adds one"
            };
            lines.push(Line::styled(format!("{MARGIN}{hint}"), theme::muted()));
        }
        None => {}
    }
    selected
}

/// The row being typed wraps like the title and note, so a long subtask is never cut off.
fn draft_row(
    lines: &mut Vec<Line<'static>>,
    card: &Card,
    renaming: Option<&Subtask>,
    total: usize,
) -> Draft {
    let done = renaming.is_some_and(|sub| sub.done());
    let lead = format!("  › {} ", if done { theme::TICKED } else { theme::BOX });
    let column = unicode_width::UnicodeWidthStr::width(lead.as_str());
    let room = card.field.width(total);
    let value = card.input.value();
    let (row, cursor) = locate(value, card.input.cursor(), room);
    let draft = Draft {
        line: lines.len() + row,
        column: column + cursor,
    };
    let rows = if value.is_empty() {
        let hint = if renaming.is_some() {
            "type a title…"
        } else {
            "add a subtask…"
        };
        vec![Span::styled(hint, theme::faint())]
    } else {
        wrap(value, room)
            .into_iter()
            .map(|text| Span::styled(text, theme::text()))
            .collect()
    };
    for (index, text) in rows.into_iter().enumerate() {
        let lead = if index == 0 {
            Span::styled(lead.clone(), Style::new().fg(theme::ACCENT))
        } else {
            Span::raw(" ".repeat(column))
        };
        let mut spans = vec![lead, text];
        pad(&mut spans, total);
        let spans = spans
            .into_iter()
            .map(|span| span.patch_style(Style::new().bg(theme::SELECTED)))
            .collect::<Vec<_>>();
        lines.push(Line::from(spans));
    }
    draft
}

fn note(
    lines: &mut Vec<Line<'static>>,
    task: &Task,
    query: &str,
    card: Option<&Card>,
    total: usize,
    draft: &mut Option<Draft>,
) {
    let editing = card.filter(|card| card.field == Field::Note);
    let heading = if editing.is_some() {
        Style::new().fg(theme::ACCENT)
    } else {
        theme::muted()
    };
    lines.push(Line::styled(
        format!("{MARGIN}NOTE"),
        heading.add_modifier(Modifier::BOLD),
    ));
    let room = Field::Note.width(total);
    if let Some(card) = editing {
        let text = card.input.value();
        let (row, column) = locate(text, card.input.cursor(), room);
        *draft = Some(Draft {
            line: lines.len() + row,
            column: MARGIN.len() + column,
        });
        if text.is_empty() {
            lines.push(Line::styled(format!("{MARGIN}add a note…"), theme::faint()));
        } else {
            for row in wrap(text, room) {
                lines.push(Line::styled(format!("{MARGIN}{row}"), theme::text()));
            }
        }
        return;
    }
    if task.note.trim().is_empty() {
        let (hint, style) = if card.is_some() {
            ("add a note…", theme::faint())
        } else {
            ("No note · ctrl-g writes one in $EDITOR", theme::muted())
        };
        lines.push(Line::styled(format!("{MARGIN}{hint}"), style));
        return;
    }
    for text in wrap(task.note.trim_end(), room) {
        let mut spans = vec![Span::raw(MARGIN)];
        spans.extend(marked(&text, query, theme::text()));
        lines.push(Line::from(spans));
    }
}
