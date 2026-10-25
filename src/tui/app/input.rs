use std::sync::mpsc::Sender;

use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use tui_input::Input;
use tui_input::backend::crossterm::EventHandler;

use super::keys::{move_by, send};
use super::schedule::{self, Spec};
use super::{App, Mode, Prompt, Tone};
use crate::tui::worker::Request;

pub(super) fn prompt(prompt: Prompt, draft: String) -> Mode {
    Mode::Input {
        input: Input::new(draft),
        prompt,
    }
}

/// Prompts never quit the app: Ctrl-C cancels, and a rejected value keeps the draft open.
pub(super) fn prompt_key(app: &mut App, tx: &Sender<Request>, key: KeyEvent) -> bool {
    let Mode::Input { mut input, prompt } = std::mem::replace(&mut app.mode, Mode::Normal) else {
        return false;
    };
    let ctrl_c = key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c');
    match key.code {
        _ if ctrl_c || key.code == KeyCode::Esc => app.say(Tone::Quiet, "cancelled"),
        KeyCode::Enter => {
            let draft = input.value().trim().to_owned();
            match submit(app, tx, &draft, prompt) {
                Ok(()) => {}
                Err(message) => {
                    if message != app.status.text {
                        app.say(Tone::Bad, message);
                    }
                    app.mode = Mode::Input { input, prompt };
                }
            }
        }
        KeyCode::Up | KeyCode::Down if prompt.steps() => {
            let step: i64 = if key.code == KeyCode::Up { 1 } else { -1 };
            let draft = match prompt {
                Prompt::Day => schedule::shift_date(input.value(), anchor(app), step),
                Prompt::Time => schedule::shift_clock(input.value(), step * 15),
                _ => schedule::shift_duration(input.value(), step * 15),
            };
            app.mode = Mode::Input {
                input: Input::new(draft),
                prompt,
            };
        }
        _ => {
            if app.status.tone == Tone::Bad {
                app.say(Tone::Quiet, "");
            }
            input.handle_event(&Event::Key(key));
            app.mode = Mode::Input { input, prompt };
        }
    }
    false
}

fn anchor(app: &App) -> chrono::NaiveDate {
    app.selected_task()
        .and_then(|task| task.day.as_deref())
        .and_then(crate::clock::parse_stamp)
        .unwrap_or_else(|| app.viewed_day())
}

fn submit(app: &mut App, tx: &Sender<Request>, draft: &str, prompt: Prompt) -> Result<(), String> {
    let request = match prompt {
        Prompt::NewTitle if draft.is_empty() => return Err("a title is required".into()),
        Prompt::NewTitle => Some(app.queue_add(draft.to_owned())),
        Prompt::Day => app.reschedule(Spec::Date(schedule::parse_date(draft, anchor(app))?)),
        Prompt::Time => app.reschedule(Spec::Clock(schedule::parse_clock(draft)?)),
        Prompt::Duration => app.reschedule(Spec::Duration(schedule::parse_duration(draft)?)),
    };
    if request.is_none() {
        return Err(app.refusal());
    }
    send(tx, request);
    Ok(())
}

pub(super) fn filter_key(app: &mut App, key: KeyEvent) -> bool {
    let typed = !key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
    match key.code {
        KeyCode::Esc => {
            app.clear_query();
            app.mode = Mode::Normal;
        }
        KeyCode::Char('c') if !typed => {
            app.clear_query();
            app.mode = Mode::Normal;
        }
        KeyCode::Char('u') | KeyCode::Char('w') if !typed => app.set_query(String::new()),
        KeyCode::Enter => app.mode = Mode::Normal,
        KeyCode::Up | KeyCode::Left => move_by(app, -1),
        KeyCode::Down | KeyCode::Right => move_by(app, 1),
        KeyCode::Backspace => {
            let mut query = app.query.clone();
            query.pop();
            app.set_query(query);
        }
        KeyCode::Char(ch) if typed => {
            let query = format!("{}{ch}", app.query);
            app.set_query(query);
        }
        _ => {}
    }
    false
}
