use std::sync::mpsc::Sender;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::error::Result;

use super::{App, Doomed, Field, Focus, Mode, Prompt, Tone};
use crate::tui::worker::Request;

/// Returns true when the app should quit.
pub(in crate::tui) fn handle(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    tx: &Sender<Request>,
    key: KeyEvent,
) -> Result<bool> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match app.mode {
        Mode::Input { .. } => return Ok(super::input::prompt_key(app, tx, key)),
        Mode::Card(_) if ctrl && key.code == KeyCode::Char('g') => {
            super::card_keys::edit_note(terminal, app, tx)?;
            return Ok(false);
        }
        Mode::Card(_) => {
            super::card_keys::key(app, tx, key);
            return Ok(false);
        }
        Mode::Filter => return Ok(super::input::filter_key(app, key)),
        _ if ctrl && key.code == KeyCode::Char('c') => return Ok(true),
        Mode::Help => {
            app.mode = Mode::Normal;
            return Ok(false);
        }
        Mode::Confirm(doomed) => {
            app.mode = Mode::Normal;
            if key.code == KeyCode::Char('y') {
                send(
                    tx,
                    if doomed == Doomed::Task {
                        app.delete_selected()
                    } else {
                        app.delete_subtask()
                    },
                );
            } else {
                let what = if doomed == Doomed::Task {
                    "task kept"
                } else {
                    "subtask kept"
                };
                app.say(Tone::Quiet, what);
            }
            return Ok(false);
        }
        Mode::Normal => {}
    }
    if ctrl {
        match key.code {
            KeyCode::Char('d') => move_by(app, 8),
            KeyCode::Char('u') => move_by(app, -8),
            KeyCode::Char('e') => app.detail_scroll = app.detail_scroll.saturating_add(1),
            KeyCode::Char('y') => app.detail_scroll = app.detail_scroll.saturating_sub(1),
            KeyCode::Char('g') => super::editor::edit_note(terminal, app, tx)?,
            _ => {}
        }
        app.g_pending = false;
        return Ok(false);
    }
    let quit = match app.focus {
        Focus::List => list_key(app, tx, key.code),
        Focus::Subtasks => subtask_key(app, tx, key.code),
    };
    if key.code != KeyCode::Char('g') {
        app.g_pending = false;
    }
    Ok(quit)
}

fn shared_key(app: &mut App, code: KeyCode) -> Option<bool> {
    match code {
        KeyCode::Char('q') => return Some(true),
        KeyCode::Char('?') => app.mode = Mode::Help,
        KeyCode::Char('r') => {
            app.arm_refresh();
            app.say(Tone::Busy, "refreshing…");
        }
        _ => return None,
    }
    Some(false)
}

#[cfg(test)]
pub(super) fn list_key_for_test(app: &mut App, tx: &Sender<Request>, code: KeyCode) -> bool {
    list_key(app, tx, code)
}

fn list_key(app: &mut App, tx: &Sender<Request>, code: KeyCode) -> bool {
    if let Some(quit) = shared_key(app, code) {
        return quit;
    }
    match code {
        KeyCode::Char('j') | KeyCode::Down => move_by(app, 1),
        KeyCode::Char('k') | KeyCode::Up => move_by(app, -1),
        KeyCode::Char('G') | KeyCode::End => app.select(app.visible().len().saturating_sub(1)),
        KeyCode::Home => app.select(0),
        KeyCode::Char('g') if app.g_pending => {
            app.select(0);
            app.g_pending = false;
        }
        KeyCode::Char('g') => app.g_pending = true,
        KeyCode::Enter | KeyCode::Char('l') => {
            app.focus_subtasks();
        }
        KeyCode::Char('A') => {
            if app.focus_subtasks() {
                app.open_card(Field::NewSubtask(app.subtask_count()));
            }
        }
        KeyCode::Esc if !app.query.is_empty() => app.clear_query(),
        KeyCode::Char('d') if app.selected_task().is_some() => {
            app.mode = Mode::Confirm(Doomed::Task)
        }
        KeyCode::Char('t') => show(app, false, 0),
        KeyCode::Char('w') => {
            app.window = !app.window;
            app.inbox = false;
            app.offset = 0;
            app.reveal();
            app.say(
                Tone::Quiet,
                if app.window {
                    "24h view, 12 hours either side of now"
                } else {
                    "day view, the calendar day"
                },
            );
        }
        KeyCode::Char('z') => app.next_zone(),
        KeyCode::Char('J') => app.detail_scroll = app.detail_scroll.saturating_add(3),
        KeyCode::Char('K') => app.detail_scroll = app.detail_scroll.saturating_sub(3),
        KeyCode::Char('i') => show(app, true, app.offset),
        KeyCode::Tab => show(app, !app.inbox, app.offset),
        KeyCode::Char('[') | KeyCode::Left => show(app, false, app.offset - 1),
        KeyCode::Char(']') | KeyCode::Right => show(app, false, app.offset + 1),
        KeyCode::Char('/') => app.mode = Mode::Filter,
        KeyCode::Char('f') | KeyCode::Char('n') if !app.query.is_empty() => step_match(app, 1),
        KeyCode::Char('N') if !app.query.is_empty() => step_match(app, -1),
        KeyCode::Char('f') => app.mode = Mode::Filter,
        KeyCode::Char('a') => app.mode = super::input::prompt(Prompt::NewTitle, String::new()),
        KeyCode::Char('e') => app.open_card(Field::Title),
        KeyCode::Char('D') => open_prompt(app, Prompt::Day),
        KeyCode::Char('T') => open_prompt(app, Prompt::Time),
        KeyCode::Char('m') => open_prompt(app, Prompt::Duration),
        KeyCode::Char(',') => send(tx, app.shift_day(-1)),
        KeyCode::Char('.') => send(tx, app.shift_day(1)),
        KeyCode::Char('x') | KeyCode::Char(' ') => send(tx, app.toggle()),
        _ => {}
    }
    false
}

fn subtask_key(app: &mut App, tx: &Sender<Request>, code: KeyCode) -> bool {
    if let Some(quit) = shared_key(app, code) {
        return quit;
    }
    let has_one = app.subtask_count() > 0;
    match code {
        KeyCode::Char('j') | KeyCode::Down => app.move_subtask(1),
        KeyCode::Char('k') | KeyCode::Up => app.move_subtask(-1),
        KeyCode::Char('h') | KeyCode::Left | KeyCode::Esc => app.focus = Focus::List,
        KeyCode::Char('x') | KeyCode::Char(' ') if has_one => send(tx, app.toggle_subtask()),
        KeyCode::Char('a') => app.open_card(Field::NewSubtask(app.subtask_count())),
        KeyCode::Char('e') if has_one => app.open_card(Field::Subtask(app.sub_selected)),
        KeyCode::Char('d') if has_one => app.mode = Mode::Confirm(Doomed::Subtask),
        KeyCode::Char('J') if has_one => send(tx, app.reorder_subtask(1)),
        KeyCode::Char('K') if has_one => send(tx, app.reorder_subtask(-1)),
        _ => {}
    }
    false
}

fn show(app: &mut App, inbox: bool, offset: i64) {
    if app.inbox != inbox || (!inbox && app.offset != offset) {
        app.inbox = inbox;
        app.offset = offset;
        app.reveal();
    }
}

fn step_match(app: &mut App, delta: isize) {
    let count = app.visible().len();
    if count > 0 {
        let next = (app.selected as isize + delta).rem_euclid(count as isize) as usize;
        app.select(next);
    }
}

pub(super) fn move_by(app: &mut App, delta: isize) {
    let count = app.visible().len();
    if count > 0 {
        app.select(app.selected.saturating_add_signed(delta).min(count - 1));
    }
}

fn open_prompt(app: &mut App, prompt: Prompt) {
    let Some(task) = app.selected_task() else {
        app.say(Tone::Bad, "no task selected");
        return;
    };
    let draft = match prompt {
        Prompt::Day => task.day.clone().unwrap_or_default(),
        Prompt::Time => task.timed().map(crate::clock::clock).unwrap_or_default(),
        Prompt::Duration => task
            .duration
            .filter(|mins| *mins > 0)
            .map(|mins| mins.to_string())
            .unwrap_or_default(),
        Prompt::NewTitle => String::new(),
    };
    app.mode = super::input::prompt(prompt, draft);
}

pub(super) fn send(tx: &Sender<Request>, request: Option<Request>) {
    if let Some(request) = request {
        let _ = tx.send(request);
    }
}
