use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, List, ListItem, ListState};

use super::{fit, marked, placeholder, width};
use crate::agenda::{self, Row};
use crate::clock;
use crate::model::Task;
use crate::tui::app::{App, Focus, Mode};
use crate::tui::theme;

pub(super) fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let focused = app.focus == Focus::List
        && matches!(app.mode, Mode::Normal | Mode::Filter | Mode::Input { .. });
    let visible = app.visible();
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(if focused {
            theme::ACCENT
        } else {
            theme::BORDER
        }))
        .title(Line::from(title(app, &visible)))
        .title(Line::from(summary(app, &visible)).right_aligned());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if !app.loaded || visible.is_empty() {
        let lines = empty_state(app);
        let (paragraph, rect) = placeholder(lines, inner);
        frame.render_widget(paragraph, rect);
        return;
    }
    let clock = agenda::Clock::now();
    let rows = if app.query.is_empty() || app.layout() == agenda::Layout::Window {
        agenda::rows(&visible, app.layout(), &clock)
    } else {
        (0..visible.len())
            .map(|index| Row::Task {
                index,
                overlap: false,
                late: clock.late(visible[index]),
            })
            .collect()
    };
    let view = RowView {
        inbox: app.inbox,
        total: inner.width as usize,
        query: &app.query,
    };
    let items: Vec<ListItem> = rows
        .iter()
        .map(|row| ListItem::new(render(row, &visible, &view)))
        .collect();
    let selected = rows
        .iter()
        .position(|row| matches!(row, Row::Task { index, .. } if *index == app.selected));
    let mut state = ListState::default()
        .with_offset(app.list_offset.get())
        .with_selected(selected);
    let highlight = if focused {
        theme::SELECTED
    } else {
        theme::SELECTED_DIM
    };
    let list = List::new(items)
        .highlight_style(Style::new().bg(highlight))
        .scroll_padding(2);
    frame.render_stateful_widget(list, inner, &mut state);
    app.list_offset.set(state.offset());
}

fn title(app: &App, visible: &[&Task]) -> Vec<Span<'static>> {
    let place = if app.inbox {
        "Inbox".to_owned()
    } else if app.layout() == agenda::Layout::Window {
        "Now".to_owned()
    } else {
        clock::relative(app.viewed_day())
            .filter(|word| matches!(word.as_str(), "today" | "tomorrow" | "yesterday"))
            .map(|word| super::capitalize(&word))
            .unwrap_or_else(|| app.viewed_day().format("%a %-d %b").to_string())
    };
    let noun = if visible.len() == 1 { "task" } else { "tasks" };
    let mut spans = vec![
        Span::styled(format!(" {place} "), theme::bold()),
        Span::styled(
            if app.loaded {
                format!("· {} {noun} ", visible.len())
            } else {
                "· loading… ".to_owned()
            },
            theme::muted(),
        ),
    ];
    if !app.query.is_empty() {
        spans.push(Span::styled(
            format!("· /{} ", app.query),
            Style::new().fg(theme::ACCENT),
        ));
    }
    spans
}

fn summary(app: &App, visible: &[&Task]) -> Vec<Span<'static>> {
    let done = visible.iter().filter(|task| task.done()).count();
    let clock = agenda::Clock::now();
    let late = visible.iter().filter(|task| clock.late(task)).count();
    let mut spans = Vec::new();
    if late > 0 && !app.inbox {
        spans.push(Span::styled(
            format!(" {late} late "),
            Style::new().fg(theme::WARN),
        ));
        spans.push(Span::styled("·", theme::muted()));
    }
    let mut text = format!(" {done} done ");
    let planned = clock::duration(agenda::planned_minutes(visible));
    if !app.inbox && !planned.is_empty() {
        text = format!(" {done} done · {planned} planned ");
    }
    spans.push(Span::styled(text, theme::muted()));
    spans
}

fn empty_state(app: &App) -> Vec<Line<'static>> {
    if !app.loaded {
        return vec![Line::styled("Loading…", theme::muted())];
    }
    if !app.query.is_empty() {
        return vec![
            Line::styled(format!("No tasks match “{}”", app.query), theme::text()),
            Line::styled("esc clears the search", theme::muted()),
        ];
    }
    let what = if app.inbox {
        "The inbox is empty"
    } else {
        "Nothing planned"
    };
    vec![
        Line::styled(what, theme::text()),
        Line::styled("a adds a task", theme::muted()),
    ]
}

fn render(row: &Row, tasks: &[&Task], view: &RowView) -> Line<'static> {
    let gutter = if view.inbox { 1 } else { 7 };
    match row {
        Row::Section(name) => Line::from(vec![
            Span::raw(" ".repeat(gutter)),
            Span::styled(
                name.to_uppercase(),
                theme::faint().add_modifier(Modifier::BOLD),
            ),
        ]),
        Row::Gap(minutes) => Line::from(vec![
            Span::raw(" ".repeat(gutter)),
            Span::styled(
                format!("┆   {} free", clock::duration(*minutes)),
                theme::faint(),
            ),
        ]),
        Row::Now(at) => {
            let lead = format!("{:>6} ", clock::clock(*at));
            let rule = view.total.saturating_sub(lead.len() + 5);
            Line::from(vec![
                Span::styled(
                    lead,
                    Style::new().fg(theme::BAD).add_modifier(Modifier::BOLD),
                ),
                Span::styled("─".repeat(rule), Style::new().fg(theme::BAD)),
                Span::styled(" now", Style::new().fg(theme::BAD)),
            ])
        }
        Row::Task {
            index,
            overlap,
            late,
        } => task_line(tasks[*index], *overlap, *late, view),
    }
}

/// Rendering facts shared by every row of one frame.
struct RowView<'a> {
    inbox: bool,
    total: usize,
    query: &'a str,
}

impl RowView<'_> {
    fn compact(&self) -> bool {
        self.total < 52
    }
}

fn task_line(task: &Task, overlap: bool, late: bool, view: &RowView) -> Line<'static> {
    let color = theme::task_color(task.color.as_deref());
    let done = task.done();
    let compact = view.compact();
    let mut left = vec![Span::raw(" ")];
    if !view.inbox {
        let when = task.timed().map(clock::clock).unwrap_or_default();
        let style = match () {
            _ if (overlap || late) && compact => Style::new().fg(theme::WARN),
            _ if done => theme::faint(),
            _ => theme::muted(),
        };
        left = vec![Span::styled(format!("{when:>6} "), style)];
    }
    left.push(Span::styled(
        format!("{} ", theme::BAR),
        Style::new().fg(color),
    ));
    left.push(if done {
        Span::styled(format!("{} ", theme::DONE), Style::new().fg(theme::GOOD))
    } else {
        Span::styled(format!("{} ", theme::OPEN), Style::new().fg(color))
    });

    let mut right = Vec::new();
    if late && !compact {
        right.push(Span::styled(" late", Style::new().fg(theme::WARN)));
    } else if overlap && !compact {
        right.push(Span::styled(" overlap", Style::new().fg(theme::WARN)));
    }
    if let Some((finished, all)) = task.subtask_progress() {
        let style = if finished == all {
            Style::new().fg(theme::GOOD)
        } else {
            theme::muted()
        };
        right.push(Span::styled(format!(" {finished}/{all}"), style));
    }
    if task.is_recurring {
        right.push(Span::styled(format!(" {}", theme::REPEAT), theme::muted()));
    }
    let minutes = clock::duration(task.minutes());
    if !minutes.is_empty() {
        let minutes = if compact {
            minutes.replace(' ', "")
        } else {
            minutes
        };
        let width = if compact { 5 } else { 6 };
        right.push(Span::styled(format!("  {minutes:>width$}"), theme::muted()));
    }
    right.push(Span::raw(" "));

    if !view.query.is_empty()
        && !task
            .title
            .to_lowercase()
            .contains(&view.query.to_lowercase())
    {
        let why = if task
            .note
            .to_lowercase()
            .contains(&view.query.to_lowercase())
        {
            " ↳ note"
        } else {
            " ↳ subtask"
        };
        let spare = view.total.saturating_sub(width(&left) + width(&right) + 1);
        if unicode_width::UnicodeWidthStr::width(task.title.as_str()) + why.chars().count() <= spare
        {
            right.insert(0, Span::styled(why, theme::faint()));
        }
    }
    let room = view.total.saturating_sub(width(&left) + width(&right) + 1);
    let title = fit(&task.title, room);
    let style = if done {
        theme::done_title()
    } else {
        theme::text()
    };
    let gap = room.saturating_sub(unicode_width::UnicodeWidthStr::width(title.as_str())) + 1;
    left.extend(marked(&title, view.query, style));
    left.push(Span::raw(" ".repeat(gap)));
    left.extend(right);
    Line::from(left)
}
