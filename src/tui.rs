use std::io::{self, stdout};
use std::time::{Duration, Instant};

use crossterm::ExecutableCommand;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs};

use crate::api::{self, NewTask, Patch};
use crate::error::Result;
use crate::mcp::Session;
use crate::model::Task;

enum Mode {
    Normal,
    Help,
    Confirm,
    Input { title: String, edit: bool },
}

struct App {
    inbox: bool,
    offset: i64,
    tasks: Vec<Task>,
    selected: usize,
    status: String,
    mode: Mode,
    g_pending: bool,
    dirty: bool,
}

pub fn run() -> Result<()> {
    let mut session = Session::connect()?;
    let mut app = App {
        inbox: false,
        offset: 0,
        tasks: Vec::new(),
        selected: 0,
        status: "loading".into(),
        mode: Mode::Normal,
        g_pending: false,
        dirty: true,
    };
    let mut terminal = setup()?;
    let _guard = RawGuard;
    let mut last = Instant::now() - Duration::from_secs(30);
    loop {
        if app.dirty || last.elapsed() >= Duration::from_secs(20) {
            refresh(&mut session, &mut app);
            last = Instant::now();
        }
        terminal.draw(|frame| draw(frame, &app))?;
        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Release {
                    continue;
                }
                if handle(&mut session, &mut app, key.code, key.modifiers)? {
                    break;
                }
            }
        }
    }
    Ok(())
}

fn refresh(session: &mut Session, app: &mut App) {
    let loaded = if app.inbox {
        api::inbox(session)
    } else if app.offset == 0 {
        api::today(session)
    } else {
        api::day(session, &shift_day(app.offset))
    };
    match loaded {
        Ok(tasks) => {
            app.tasks = tasks;
            if app.selected >= app.tasks.len() {
                app.selected = app.tasks.len().saturating_sub(1);
            }
            app.status = format!("{} tasks", app.tasks.len());
        }
        Err(err) => app.status = err.to_string(),
    }
    app.dirty = false;
}

fn handle(
    session: &mut Session,
    app: &mut App,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> Result<bool> {
    if let Mode::Input { title, edit } = &mut app.mode {
        match code {
            KeyCode::Esc => app.mode = Mode::Normal,
            KeyCode::Enter => {
                let title = title.trim().to_owned();
                let edit = *edit;
                app.mode = Mode::Normal;
                if title.is_empty() {
                    app.status = "title required".into();
                    return Ok(false);
                }
                let result = if edit {
                    let Some(task) = app.tasks.get(app.selected) else {
                        return Ok(false);
                    };
                    let task_ref = task.task_ref()?;
                    api::edit(
                        session,
                        &task_ref,
                        Patch {
                            title: Some(title),
                            note: None,
                            day: None,
                            start_time: None,
                            duration: None,
                            all_day: None,
                            inbox: false,
                        },
                    )
                } else if app.inbox {
                    api::add(
                        session,
                        NewTask {
                            title,
                            note: None,
                            day: None,
                            start_time: None,
                            duration: None,
                            all_day: false,
                            symbol: None,
                        },
                    )
                } else {
                    api::add(
                        session,
                        NewTask {
                            title,
                            note: None,
                            day: Some(shift_day(app.offset)),
                            start_time: None,
                            duration: None,
                            all_day: true,
                            symbol: None,
                        },
                    )
                };
                app.status = match result {
                    Ok(_) => "saved".into(),
                    Err(err) => err.to_string(),
                };
                app.dirty = true;
            }
            KeyCode::Backspace => {
                title.pop();
            }
            KeyCode::Char(ch) => title.push(ch),
            _ => {}
        }
        return Ok(false);
    }
    if matches!(app.mode, Mode::Help) {
        app.mode = Mode::Normal;
        return Ok(false);
    }
    if matches!(app.mode, Mode::Confirm) {
        match code {
            KeyCode::Char('y') => {
                app.mode = Mode::Normal;
                if let Some(task) = app.tasks.get(app.selected) {
                    let task_ref = task.task_ref()?;
                    app.status = match api::delete(session, &task_ref) {
                        Ok(_) => "deleted".into(),
                        Err(err) => err.to_string(),
                    };
                    app.dirty = true;
                }
            }
            _ => app.mode = Mode::Normal,
        }
        return Ok(false);
    }
    match code {
        KeyCode::Char('q') => return Ok(true),
        KeyCode::Char('?') => app.mode = Mode::Help,
        KeyCode::Char('j') | KeyCode::Down => {
            app.selected = (app.selected + 1).min(app.tasks.len().saturating_sub(1))
        }
        KeyCode::Char('k') | KeyCode::Up => app.selected = app.selected.saturating_sub(1),
        KeyCode::Char('G') => app.selected = app.tasks.len().saturating_sub(1),
        KeyCode::Char('g') => {
            if app.g_pending {
                app.selected = 0;
                app.g_pending = false;
            } else {
                app.g_pending = true;
            }
        }
        KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.selected = (app.selected + 8).min(app.tasks.len().saturating_sub(1));
        }
        KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.selected = app.selected.saturating_sub(8);
        }
        KeyCode::Char('d') => app.mode = Mode::Confirm,
        KeyCode::Tab | KeyCode::Char('l') | KeyCode::Char('h') => {
            app.inbox = !app.inbox;
            app.selected = 0;
            app.dirty = true;
        }
        KeyCode::Char('[') if !app.inbox => {
            app.offset -= 1;
            app.selected = 0;
            app.dirty = true;
        }
        KeyCode::Char(']') if !app.inbox => {
            app.offset += 1;
            app.selected = 0;
            app.dirty = true;
        }
        KeyCode::Char('r') => app.dirty = true,
        KeyCode::Char('a') => {
            app.mode = Mode::Input {
                title: String::new(),
                edit: false,
            }
        }
        KeyCode::Char('e') => {
            let title = app
                .tasks
                .get(app.selected)
                .map(|t| t.title.clone())
                .unwrap_or_default();
            app.mode = Mode::Input { title, edit: true };
        }
        KeyCode::Char('x') | KeyCode::Char(' ') => toggle(session, app)?,
        _ => app.g_pending = false,
    }
    if !matches!(code, KeyCode::Char('g')) {
        app.g_pending = false;
    }
    Ok(false)
}

fn toggle(session: &mut Session, app: &mut App) -> Result<()> {
    let Some(task) = app.tasks.get(app.selected) else {
        return Ok(());
    };
    let task_ref = task.task_ref()?;
    let completed = !task.done();
    app.status = match api::set_completed(session, &task_ref, completed) {
        Ok(_) => {
            if completed {
                "done".into()
            } else {
                "reopened".into()
            }
        }
        Err(err) => err.to_string(),
    };
    app.dirty = true;
    Ok(())
}

fn draw(frame: &mut Frame, app: &App) {
    let [tabs_area, list_area, detail_area, status_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(5),
        Constraint::Length(5),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    let titles = ["Today", "Inbox"].map(Line::from);
    let tabs = Tabs::new(titles).select(usize::from(app.inbox)).block(
        Block::default()
            .borders(Borders::ALL)
            .title(shift_day(app.offset)),
    );
    frame.render_widget(tabs, tabs_area);

    let items: Vec<ListItem> = app
        .tasks
        .iter()
        .map(|task| {
            let mark = if task.done() { "x" } else { " " };
            ListItem::new(format!(
                "[{mark}] {:>5} {:>4}  {}",
                task.when(),
                task.duration_label(),
                task.title
            ))
        })
        .collect();
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Tasks"))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    let mut state = ListState::default();
    if !app.tasks.is_empty() {
        state.select(Some(app.selected));
    }
    frame.render_stateful_widget(list, list_area, &mut state);

    let detail = match app.tasks.get(app.selected) {
        Some(task) => {
            let id = task
                .id
                .clone()
                .or(task.recurring_id.clone())
                .unwrap_or_default();
            format!("{}\n{}\n{id}", task.title, task.note)
        }
        None => "No task".into(),
    };
    let detail = if matches!(app.mode, Mode::Help) {
        "j/k move  g g top  G bottom  h/l pane  [ ] day\nx or space tick  a add  e edit  d delete  r refresh  q quit".into()
    } else if let Mode::Input { title, edit } = &app.mode {
        format!("{}: {title}_", if *edit { "edit" } else { "add" })
    } else if matches!(app.mode, Mode::Confirm) {
        "Delete this task? y / n".into()
    } else {
        detail
    };
    frame.render_widget(
        Paragraph::new(detail).block(Block::default().borders(Borders::ALL).title("Detail")),
        detail_area,
    );
    let status = Line::from(Span::raw(format!("{}   ? help", app.status)));
    frame.render_widget(Paragraph::new(status), status_area);
}

fn shift_day(offset: i64) -> String {
    (chrono::Local::now().date_naive() + chrono::Duration::days(offset))
        .format("%Y-%m-%d")
        .to_string()
}

fn setup() -> Result<ratatui::DefaultTerminal> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    Ok(ratatui::Terminal::new(
        ratatui::backend::CrosstermBackend::new(stdout()),
    )?)
}

struct RawGuard;

impl Drop for RawGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = io::stdout().execute(LeaveAlternateScreen);
    }
}
