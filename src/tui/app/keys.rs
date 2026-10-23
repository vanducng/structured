use std::io::{self, Write};
use std::process::Command;
use std::sync::mpsc::Sender;

use crossterm::ExecutableCommand;
use crossterm::event::{KeyCode, KeyModifiers};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};

use crate::error::Result;

use super::schedule::{self, Spec};
use super::{App, Prompt};
use crate::tui::worker::Request;

pub(in crate::tui) fn handle(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    tx: &Sender<Request>,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> Result<bool> {
    if matches!(app.mode, super::Mode::Input { .. }) {
        return input_key(app, tx, code);
    }
    if matches!(app.mode, super::Mode::Filter) {
        return filter_key(app, code);
    }
    if matches!(app.mode, super::Mode::Help) {
        app.mode = super::Mode::Normal;
        return Ok(false);
    }
    if matches!(app.mode, super::Mode::Confirm) {
        if matches!(code, KeyCode::Char('y')) {
            send(tx, app.delete_selected());
        }
        app.mode = super::Mode::Normal;
        return Ok(false);
    }
    match code {
        KeyCode::Char('q') => return Ok(true),
        KeyCode::Char('?') => app.mode = super::Mode::Help,
        KeyCode::Char('j') | KeyCode::Down => move_by(app, 1),
        KeyCode::Char('k') | KeyCode::Up => move_by(app, -1),
        KeyCode::Char('G') => {
            app.selected = app.visible().len().saturating_sub(1);
            app.detail_scroll = 0;
        }
        KeyCode::Char('g') if modifiers.contains(KeyModifiers::CONTROL) => {
            edit_note(terminal, app, tx)?
        }
        KeyCode::Char('g') => {
            if app.g_pending {
                app.selected = 0;
                app.detail_scroll = 0;
                app.g_pending = false;
            } else {
                app.g_pending = true;
            }
        }
        KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => move_by(app, 8),
        KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => move_by(app, -8),
        KeyCode::Char('e') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.detail_scroll = app.detail_scroll.saturating_add(1);
        }
        KeyCode::Char('y') if modifiers.contains(KeyModifiers::CONTROL) => {
            app.detail_scroll = app.detail_scroll.saturating_sub(1);
        }
        KeyCode::Char('d') => app.mode = super::Mode::Confirm,
        KeyCode::Char('t') => show_today(app),
        KeyCode::Char('i') => show_inbox(app),
        KeyCode::Tab | KeyCode::Char('h') | KeyCode::Char('l') => {
            app.inbox = !app.inbox;
            app.reveal();
        }
        KeyCode::Char('[') | KeyCode::Left => {
            app.inbox = false;
            app.offset -= 1;
            app.reveal();
        }
        KeyCode::Char(']') | KeyCode::Right => {
            app.inbox = false;
            app.offset += 1;
            app.reveal();
        }
        KeyCode::Char('/') => {
            app.filter_literal = true;
            app.mode = super::Mode::Filter;
        }
        KeyCode::Char('f') => find(app),
        KeyCode::Char('r') => app.arm_refresh(),
        KeyCode::Char('a') => {
            app.mode = super::Mode::Input {
                draft: String::new(),
                prompt: Prompt::NewTitle,
            }
        }
        KeyCode::Char('e') => open_prompt(app, Prompt::Title),
        KeyCode::Char('D') => open_prompt(app, Prompt::Day),
        KeyCode::Char('T') => open_prompt(app, Prompt::Time),
        KeyCode::Char('m') => open_prompt(app, Prompt::Duration),
        KeyCode::Char(',') => send(tx, app.shift_day(-1)),
        KeyCode::Char('.') => send(tx, app.shift_day(1)),
        KeyCode::Char('x') | KeyCode::Char(' ') => send(tx, app.toggle()),
        _ => {}
    }
    if !matches!(code, KeyCode::Char('g')) || modifiers.contains(KeyModifiers::CONTROL) {
        app.g_pending = false;
    }
    Ok(false)
}

fn show_today(app: &mut App) {
    if app.inbox || app.offset != 0 {
        app.inbox = false;
        app.offset = 0;
        app.reveal();
    }
}

fn show_inbox(app: &mut App) {
    if !app.inbox {
        app.inbox = true;
        app.reveal();
    }
}

fn find(app: &mut App) {
    if app.query.is_empty() {
        app.filter_literal = false;
        app.mode = super::Mode::Filter;
        return;
    }
    find_next(app);
}

fn find_next(app: &mut App) {
    let count = app.visible().len();
    if count == 0 {
        app.selected = 0;
        return;
    }
    app.selected = if app.selected >= count {
        0
    } else {
        (app.selected + 1) % count
    };
    app.detail_scroll = 0;
}

fn move_by(app: &mut App, delta: isize) {
    let count = app.visible().len();
    if count == 0 {
        return;
    }
    let next = app.selected as isize + delta;
    app.selected = next.clamp(0, count as isize - 1) as usize;
    app.detail_scroll = 0;
}

fn input_key(app: &mut App, tx: &Sender<Request>, code: KeyCode) -> Result<bool> {
    let super::Mode::Input { mut draft, prompt } =
        std::mem::replace(&mut app.mode, super::Mode::Normal)
    else {
        return Ok(false);
    };
    let anchor = app
        .selected_task()
        .and_then(|task| task.day.as_deref())
        .and_then(|day| chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d").ok())
        .unwrap_or_else(|| chrono::Local::now().date_naive());
    match code {
        KeyCode::Esc => {}
        KeyCode::Enter => submit_prompt(app, tx, &draft, prompt),
        KeyCode::Backspace => {
            draft.pop();
            app.mode = super::Mode::Input { draft, prompt };
        }
        KeyCode::Up | KeyCode::Down => {
            let step: i64 = if matches!(code, KeyCode::Up) { 1 } else { -1 };
            draft = match prompt {
                Prompt::Day => schedule::shift_date(&draft, anchor, step),
                Prompt::Time => schedule::shift_clock(&draft, step * 15),
                Prompt::Duration => schedule::shift_duration(&draft, step * 15),
                Prompt::NewTitle | Prompt::Title => draft,
            };
            app.mode = super::Mode::Input { draft, prompt };
        }
        KeyCode::Char(ch) => {
            draft.push(ch);
            app.mode = super::Mode::Input { draft, prompt };
        }
        _ => app.mode = super::Mode::Input { draft, prompt },
    }
    Ok(false)
}

fn submit_prompt(app: &mut App, tx: &Sender<Request>, draft: &str, prompt: Prompt) {
    let draft = draft.trim();
    let anchor = app
        .selected_task()
        .and_then(|task| task.day.as_deref())
        .and_then(|day| chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d").ok())
        .unwrap_or_else(|| chrono::Local::now().date_naive());
    match prompt {
        Prompt::NewTitle | Prompt::Title => {
            if draft.is_empty() {
                app.status = "title required".into();
                return;
            }
            let title = draft.to_owned();
            if matches!(prompt, Prompt::Title) {
                send(tx, app.rename(title));
            } else {
                send(tx, Some(app.queue_add(title)));
            }
        }
        Prompt::Day => match schedule::parse_date(draft, anchor) {
            Ok(choice) => send(tx, app.reschedule(Spec::Date(choice))),
            Err(err) => app.status = err,
        },
        Prompt::Time => match schedule::parse_clock(draft) {
            Ok(choice) => send(tx, app.reschedule(Spec::Clock(choice))),
            Err(err) => app.status = err,
        },
        Prompt::Duration => match schedule::parse_duration(draft) {
            Ok(mins) => send(tx, app.reschedule(Spec::Duration(mins))),
            Err(err) => app.status = err,
        },
    }
}

fn open_prompt(app: &mut App, prompt: Prompt) {
    let Some(task) = app.selected_task() else {
        app.status = "no task".into();
        return;
    };
    let draft = match prompt {
        Prompt::Title => task.title.clone(),
        Prompt::NewTitle => String::new(),
        Prompt::Day => task.day.clone().unwrap_or_default(),
        Prompt::Time => match task.when().as_str() {
            "all" | "inbox" => String::new(),
            clock => clock.to_owned(),
        },
        Prompt::Duration => task
            .duration
            .filter(|mins| *mins > 0)
            .map(|mins| mins.to_string())
            .unwrap_or_default(),
    };
    app.mode = super::Mode::Input { draft, prompt };
}

fn filter_key(app: &mut App, code: KeyCode) -> Result<bool> {
    match code {
        KeyCode::Esc => {
            app.query.clear();
            app.mode = super::Mode::Normal;
            app.selected = 0;
        }
        KeyCode::Enter => app.mode = super::Mode::Normal,
        KeyCode::Char('f') if !app.filter_literal && !app.query.is_empty() => find_next(app),
        KeyCode::Char('f') if !app.filter_literal => app.mode = super::Mode::Normal,
        KeyCode::Up | KeyCode::Left => move_by(app, -1),
        KeyCode::Down | KeyCode::Right => move_by(app, 1),
        KeyCode::Backspace => {
            app.query.pop();
            app.selected = 0;
        }
        KeyCode::Char(ch) => {
            app.query.push(ch);
            app.selected = 0;
        }
        _ => {}
    }
    Ok(false)
}

fn send(tx: &Sender<Request>, request: Option<Request>) {
    if let Some(request) = request {
        let _ = tx.send(request);
    }
}

fn edit_note(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    tx: &Sender<Request>,
) -> Result<()> {
    let Some(index) = app.selected_index() else {
        app.status = "no task".into();
        return Ok(());
    };
    if app
        .tasks
        .get(index)
        .and_then(|task| task.task_ref().ok())
        .is_none()
    {
        app.status = "task has no id".into();
        return Ok(());
    }
    let previous = app.tasks[index].note.clone();
    let path = std::env::temp_dir().join(format!("structured-note-{}.md", std::process::id()));
    std::fs::write(&path, &previous)?;
    suspend(terminal)?;
    let edited = run_editor(&path);
    restore(terminal)?;
    let read = std::fs::read_to_string(&path);
    let _ = std::fs::remove_file(&path);
    if !edited? {
        app.status = "editor closed".into();
        return Ok(());
    }
    let note = match read {
        Ok(note) => note,
        Err(_) => {
            app.status = "could not read the note".into();
            return Ok(());
        }
    };
    let next = note.trim_end_matches('\n').to_owned();
    let prev = previous.trim_end_matches('\n');
    if next == prev {
        app.status = "note unchanged".into();
        return Ok(());
    }
    send(tx, app.update_note(next, prev.to_owned()));
    Ok(())
}

fn editor_program() -> String {
    std::env::var("VISUAL")
        .ok()
        .filter(|value| !value.is_empty())
        .or_else(|| std::env::var("EDITOR").ok())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "nvim".into())
}

fn run_editor(path: &std::path::Path) -> Result<bool> {
    let editor = editor_program();
    let mut parts = editor.split_whitespace();
    let program = parts.next().unwrap_or("nvim");
    let status = Command::new(program).args(parts).arg(path).status()?;
    Ok(status.success())
}

fn suspend(terminal: &mut ratatui::DefaultTerminal) -> Result<()> {
    disable_raw_mode()?;
    io::stdout().execute(LeaveAlternateScreen)?;
    let _ = terminal.show_cursor();
    let _ = io::stdout().flush();
    Ok(())
}

fn restore(terminal: &mut ratatui::DefaultTerminal) -> Result<()> {
    enable_raw_mode()?;
    io::stdout().execute(EnterAlternateScreen)?;
    terminal.clear()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::{App, Mode};
    use super::{filter_key, find};
    use crate::config::UiConfig;
    use crate::model::Task;
    use crossterm::event::KeyCode;

    fn task(title: &str) -> Task {
        Task {
            id: Some(title.into()),
            recurring_id: None,
            title: title.into(),
            note: String::new(),
            day: None,
            start_time: None,
            duration: None,
            is_all_day: false,
            is_in_inbox: false,
            is_completed: false,
            is_recurring: false,
            completed_at: None,
            symbol: None,
            color: None,
            energy_level: None,
        }
    }

    #[test]
    fn filter_arrows_move_between_matches() {
        let mut app = App::new(UiConfig::default());
        app.tasks = vec![task("alpha"), task("beta notes"), task("gamma notes")];
        app.mode = Mode::Filter;
        app.query = "notes".into();

        filter_key(&mut app, KeyCode::Down).unwrap();
        assert_eq!(app.selected, 1);
        assert!(matches!(app.mode, Mode::Filter));
        assert_eq!(app.query, "notes");
        assert_eq!(app.selected_task().unwrap().title, "gamma notes");

        filter_key(&mut app, KeyCode::Up).unwrap();
        assert_eq!(app.selected_task().unwrap().title, "beta notes");
        filter_key(&mut app, KeyCode::Right).unwrap();
        assert_eq!(app.selected_task().unwrap().title, "gamma notes");
        filter_key(&mut app, KeyCode::Left).unwrap();
        assert_eq!(app.selected_task().unwrap().title, "beta notes");

        filter_key(&mut app, KeyCode::Char('k')).unwrap();
        assert_eq!(app.query, "notesk");
        assert_eq!(app.selected, 0);
        assert!(matches!(app.mode, Mode::Filter));
    }

    #[test]
    fn f_opens_find_and_repeats_when_it_is_active() {
        let mut app = App::new(UiConfig::default());
        app.tasks = vec![task("alpha"), task("beta notes"), task("gamma notes")];

        find(&mut app);
        assert!(matches!(app.mode, Mode::Filter));
        assert!(app.query.is_empty());

        filter_key(&mut app, KeyCode::Char('f')).unwrap();
        assert!(matches!(app.mode, Mode::Normal));
        assert!(app.query.is_empty());

        app.mode = Mode::Filter;
        app.query = "notes".into();
        filter_key(&mut app, KeyCode::Char('f')).unwrap();
        assert!(matches!(app.mode, Mode::Filter));
        assert_eq!(app.query, "notes");
        assert_eq!(app.selected_task().unwrap().title, "gamma notes");
        filter_key(&mut app, KeyCode::Char('f')).unwrap();
        assert_eq!(app.selected_task().unwrap().title, "beta notes");

        app.mode = Mode::Normal;
        find(&mut app);
        assert_eq!(app.selected_task().unwrap().title, "gamma notes");
        find(&mut app);
        assert_eq!(app.selected_task().unwrap().title, "beta notes");

        app.filter_literal = true;
        app.mode = Mode::Filter;
        app.query = "notes".into();
        app.selected = 0;
        filter_key(&mut app, KeyCode::Char('f')).unwrap();
        assert_eq!(app.query, "notesf");
        assert_eq!(app.selected, 0);
        assert!(matches!(app.mode, Mode::Filter));
    }
}
