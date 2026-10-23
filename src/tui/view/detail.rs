use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph};

use super::{fit, marked, pad, placeholder, wrap};
use crate::clock;
use crate::model::Task;
use crate::tui::app::{App, Focus, Mode, Prompt};
use crate::tui::theme;

const MARGIN: &str = "  ";
const LABEL: usize = 10;

/// Where the inline subtask draft sits, so the terminal cursor can be placed in it.
struct Draft {
    line: usize,
    column: usize,
}

pub(super) fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let focused =
        app.focus == Focus::Subtasks && !matches!(app.mode, Mode::Help | Mode::Confirm(_));
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(if focused {
            theme::ACCENT
        } else {
            theme::BORDER
        }))
        .title(Line::styled(" Details ", theme::bold()));
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
    let mut lines = band(task, total);
    lines.push(Line::default());
    properties(&mut lines, task, total);
    lines.push(Line::default());
    let (focus_line, draft) = subtasks(&mut lines, task, app, focused, total);
    lines.push(Line::default());
    note(&mut lines, task, &app.query, total);
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
    if let Some(line) = focus_line {
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

fn band(task: &Task, total: usize) -> Vec<Line<'static>> {
    let base = theme::chip(theme::task_color(task.color.as_deref()));
    let title = if task.done() {
        base.add_modifier(Modifier::CROSSED_OUT)
    } else {
        base
    };
    let blank = Line::styled(" ".repeat(total), base);
    let mut lines = vec![blank.clone()];
    for text in wrap(&task.title, total.saturating_sub(4)) {
        let mut spans = vec![Span::styled(MARGIN, base), Span::styled(text, title)];
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

/// Returns the line to keep on screen and, while a subtask is being typed, where its cursor goes.
fn subtasks(
    lines: &mut Vec<Line<'static>>,
    task: &Task,
    app: &App,
    focused: bool,
    total: usize,
) -> (Option<usize>, Option<Draft>) {
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
    let editing = match &app.mode {
        Mode::Input { input, prompt } if prompt.inline() => Some((*prompt, input)),
        _ => None,
    };
    let mut focus_line = None;
    let mut draft = None;
    for (index, sub) in task.subtasks.iter().enumerate() {
        let adding = matches!(editing, Some((Prompt::NewSubtask, _)));
        let current = focused && index == app.sub_selected && !adding;
        if let Some((Prompt::SubtaskTitle, input)) = editing
            && current
        {
            focus_line = Some(lines.len());
            draft = Some(draft_row(lines, input, sub.done(), total));
            continue;
        }
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
            focus_line = Some(lines.len());
        }
        lines.push(Line::from(spans));
    }
    if let Some((Prompt::NewSubtask, input)) = editing {
        focus_line = Some(lines.len());
        draft = Some(draft_row(lines, input, false, total));
    } else if task.subtasks.is_empty() {
        let hint = if focused {
            "No subtasks yet · a adds one"
        } else {
            "No subtasks · A adds one"
        };
        lines.push(Line::styled(format!("{MARGIN}{hint}"), theme::muted()));
    }
    (focus_line, draft)
}

fn draft_row(
    lines: &mut Vec<Line<'static>>,
    input: &tui_input::Input,
    done: bool,
    total: usize,
) -> Draft {
    let lead = format!("  › {} ", if done { theme::TICKED } else { theme::BOX });
    let column = unicode_width::UnicodeWidthStr::width(lead.as_str());
    let room = total.saturating_sub(column + 1);
    let (shown, cursor) = super::footer::window(
        input.value(),
        input.visual_scroll(room),
        input.visual_cursor(),
        room,
    );
    let text = if shown.is_empty() {
        Span::styled("add a subtask…", theme::faint())
    } else {
        Span::styled(shown, theme::text())
    };
    let mut spans = vec![Span::styled(lead, Style::new().fg(theme::ACCENT)), text];
    pad(&mut spans, total);
    let spans = spans
        .into_iter()
        .map(|span| span.patch_style(Style::new().bg(theme::SELECTED)))
        .collect::<Vec<_>>();
    let line = lines.len();
    lines.push(Line::from(spans));
    Draft {
        line,
        column: column + cursor,
    }
}

fn note(lines: &mut Vec<Line<'static>>, task: &Task, query: &str, total: usize) {
    lines.push(Line::styled(
        format!("{MARGIN}NOTE"),
        theme::muted().add_modifier(Modifier::BOLD),
    ));
    if task.note.trim().is_empty() {
        lines.push(Line::styled(
            format!("{MARGIN}No note · ctrl-g writes one in $EDITOR"),
            theme::muted(),
        ));
        return;
    }
    for text in wrap(task.note.trim_end(), total.saturating_sub(4)) {
        let mut spans = vec![Span::raw(MARGIN)];
        spans.extend(marked(&text, query, theme::text()));
        lines.push(Line::from(spans));
    }
}
