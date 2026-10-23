use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

use crate::config::PaneLayout;
use crate::model::Task;

use super::app::{self, App, Mode, Prompt};

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let [header_area, body, status_area] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);
    frame.render_widget(Paragraph::new(header(app)), header_area);

    let wide = area.width >= 72;
    let side = app.config.layout == PaneLayout::Side && wide;
    let detail_pct = app.config.detail_percent;
    let list_pct = 100u16.saturating_sub(detail_pct).max(30);
    let (list_area, detail_area) = if side {
        let [list_area, detail_area] = Layout::horizontal([
            Constraint::Percentage(list_pct),
            Constraint::Percentage(detail_pct),
        ])
        .areas(body);
        (list_area, detail_area)
    } else {
        let [list_area, detail_area] =
            Layout::vertical([Constraint::Percentage(55), Constraint::Percentage(45)]).areas(body);
        (list_area, detail_area)
    };

    let visible = app.visible();
    let items: Vec<ListItem> = visible
        .iter()
        .map(|task| ListItem::new(task_line(task)))
        .collect();
    let title = format!("{}  {}", app.heading(), visible.len());
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    let mut state = ListState::default();
    if !visible.is_empty() {
        state.select(Some(app.selected.min(visible.len() - 1)));
    }
    frame.render_stateful_widget(list, list_area, &mut state);

    let detail = Paragraph::new(detail_lines(app))
        .block(Block::default().borders(Borders::ALL).title("Detail"))
        .wrap(Wrap { trim: false })
        .scroll((app.detail_scroll, 0));
    frame.render_widget(detail, detail_area);

    let status = app.status.as_str();
    let style = if status.contains("error")
        || status.contains("HTTP")
        || status.contains("required")
        || status.contains("no id")
    {
        Style::default().fg(Color::Red)
    } else if status.starts_with("saving") {
        Style::default().fg(Color::Yellow)
    } else if matches!(
        status,
        "done" | "reopened" | "deleted" | "saved" | "note saved"
    ) {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    frame.render_widget(Paragraph::new(Span::styled(status, style)), status_area);
}

fn header(app: &App) -> Vec<Line<'static>> {
    let place = if app.inbox {
        "Inbox".to_owned()
    } else {
        app::day_heading(app.offset)
    };
    let filter = if matches!(app.mode, Mode::Filter) {
        format!("/{}_", app.query)
    } else if app.query.is_empty() {
        String::new()
    } else {
        format!("/{}", app.query)
    };
    let shown = app.visible().len();
    vec![
        Line::from(Span::styled(
            place,
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!("t today  i inbox  [ ] day  f find  D date  T time  m min  {shown}  {filter}"),
            Style::default().fg(Color::DarkGray),
        )),
    ]
}

fn task_line(task: &Task) -> Line<'static> {
    let accent = task_color(task);
    let mut title_style = Style::default().fg(if task.done() {
        Color::DarkGray
    } else {
        Color::White
    });
    if task.done() {
        title_style = title_style.add_modifier(Modifier::CROSSED_OUT);
    }
    let mark = if task.done() { "x" } else { " " };
    Line::from(vec![
        Span::styled("● ", Style::default().fg(accent)),
        Span::raw(format!(
            "[{mark}] {:>5} {:>4}  ",
            task.when(),
            task.duration_label()
        )),
        Span::styled(task.title.clone(), title_style),
    ])
}

fn detail_lines(app: &App) -> Vec<Line<'static>> {
    if matches!(app.mode, Mode::Help) {
        return vec![
            Line::from("j/k move    gg / G top and bottom    ctrl-d/u page"),
            Line::from("t today     i inbox     tab or h/l switch"),
            Line::from("[ ] or arrows change day"),
            Line::from("/ or f find    f again next match, or closes    esc clears"),
            Line::from("arrows move the list while find is open"),
            Line::from("x or space toggles the box     a add     e title"),
            Line::from("D date     T time     m duration     , . move one day"),
            Line::from("ctrl-g edit the note in $EDITOR"),
            Line::from("d then y delete     r refresh     q quit"),
            Line::from("ctrl-y / ctrl-e scroll this pane"),
        ];
    }
    if let Mode::Input { draft, prompt } = &app.mode {
        let label = match prompt {
            Prompt::NewTitle => "add",
            Prompt::Title => "title",
            Prompt::Day => "date",
            Prompt::Time => "time",
            Prompt::Duration => "duration",
        };
        return vec![Line::from(format!("{label}: {draft}_"))];
    }
    if matches!(app.mode, Mode::Confirm) {
        return vec![Line::from("Delete this task?  y / n")];
    }
    let Some(task) = app.selected_task() else {
        return vec![Line::from("No task")];
    };
    let accent = task_color(task);
    let mut title_style = Style::default().fg(accent).add_modifier(Modifier::BOLD);
    if task.done() {
        title_style = title_style.add_modifier(Modifier::CROSSED_OUT);
    }
    let state = if task.done() { "done" } else { "open" };
    let mut meta = format!("{}  {}  {}", state, task.when(), task.duration_label());
    if task.is_recurring {
        meta.push_str("  repeats");
    }
    if let Some(energy) = task.energy_level.filter(|level| *level > 0) {
        meta.push_str(&format!("  energy {energy}"));
    }
    let mut lines = vec![
        Line::from(Span::styled(task.title.clone(), title_style)),
        Line::from(meta),
    ];
    if let Some(day) = &task.day {
        lines.push(Line::from(format!("day  {day}")));
    }
    if let Some(color) = &task.color {
        lines.push(Line::from(Span::styled(
            format!("color  {color}"),
            Style::default().fg(accent),
        )));
    }
    lines.push(Line::from(""));
    if task.note.is_empty() {
        lines.push(Line::from(Span::styled(
            "No note",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        lines.push(Line::from(task.note.clone()));
    }
    let id = task
        .id
        .clone()
        .filter(|id| !id.is_empty())
        .or(task.recurring_id.clone())
        .unwrap_or_default();
    if !id.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            id,
            Style::default().fg(Color::DarkGray),
        )));
    }
    lines
}

fn task_color(task: &Task) -> Color {
    task.color.as_deref().map(accent).unwrap_or(Color::Cyan)
}

pub fn accent(name: &str) -> Color {
    match name.trim().to_ascii_lowercase().as_str() {
        "day" => Color::Rgb(240, 147, 67),
        "dawn" => Color::Rgb(232, 164, 154),
        "sunshine" => Color::Rgb(240, 196, 80),
        "nature" | "forest" => Color::Rgb(90, 158, 92),
        "night" => Color::Rgb(120, 136, 196),
        "twilight" => Color::Rgb(156, 112, 176),
        "midnight" => Color::Rgb(72, 88, 148),
        "classic" => Color::Gray,
        _ => parse_hex(name).unwrap_or(Color::Gray),
    }
}

fn parse_hex(text: &str) -> Option<Color> {
    let hex = text.trim().trim_start_matches('#');
    let value = u32::from_str_radix(hex, 16).ok()?;
    let (red, green, blue) = match hex.len() {
        6 => (
            ((value >> 16) & 0xff) as u8,
            ((value >> 8) & 0xff) as u8,
            (value & 0xff) as u8,
        ),
        3 => (
            (((value >> 8) & 0xf) * 17) as u8,
            (((value >> 4) & 0xf) * 17) as u8,
            ((value & 0xf) * 17) as u8,
        ),
        _ => return None,
    };
    Some(Color::Rgb(red, green, blue))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_named_and_hex_colors() {
        assert_eq!(accent("forest"), Color::Rgb(90, 158, 92));
        assert_eq!(accent("#ED732E"), Color::Rgb(0xed, 0x73, 0x2e));
    }
}
