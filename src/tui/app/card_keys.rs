use std::sync::mpsc::Sender;

use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use tui_input::backend::crossterm::EventHandler;
use tui_input::{Input, InputRequest};

use super::keys::send;
use super::{App, Card, Field, Mode, Tone};
use crate::error::Result;
use crate::tui::worker::Request;
use crate::tui::wrap;

#[derive(Clone, Copy)]
enum Go {
    Close,
    Next,
    Prev,
    Up,
    Down,
    Below,
    Note,
}

/// Every field saves when the cursor leaves it, so esc never throws typed work away.
pub(super) fn key(app: &mut App, tx: &Sender<Request>, key: KeyEvent) {
    let Mode::Card(mut card) = std::mem::replace(&mut app.mode, Mode::Normal) else {
        return;
    };
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let step = match key.code {
        KeyCode::Esc => Go::Close,
        KeyCode::Tab => Go::Next,
        KeyCode::BackTab => Go::Prev,
        KeyCode::Char('c') if ctrl => return undo(app, card),
        KeyCode::Enter if card.field == Field::Note => {
            card.input.handle(InputRequest::InsertChar('\n'));
            app.mode = Mode::Card(card);
            return;
        }
        KeyCode::Enter if card.field == Field::Title => Go::Next,
        KeyCode::Enter => Go::Below,
        KeyCode::Up | KeyCode::Down => {
            let up = key.code == KeyCode::Up;
            if let Some(cursor) = row_move(app, &card, up) {
                card.input = card.input.with_cursor(cursor);
                app.mode = Mode::Card(card);
                return;
            }
            if up { Go::Up } else { Go::Down }
        }
        KeyCode::Backspace
            if matches!(card.field, Field::NewSubtask(_)) && card.input.value().is_empty() =>
        {
            Go::Up
        }
        _ => {
            if app.status.tone == Tone::Bad {
                app.say(Tone::Quiet, "");
            }
            if card.field != Field::Note || !line_edit(&mut card.input, key) {
                card.input.handle_event(&Event::Key(key));
            }
            app.mode = Mode::Card(card);
            return;
        }
    };
    go(app, tx, card, step);
}

/// ctrl-g leaves the field for the note and opens it in $EDITOR; saving there saves the note.
pub(super) fn edit_note(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    tx: &Sender<Request>,
) -> Result<()> {
    let Mode::Card(card) = std::mem::replace(&mut app.mode, Mode::Normal) else {
        return Ok(());
    };
    if card.field == Field::Note {
        app.mode = Mode::Card(card);
    } else {
        go(app, tx, card, Go::Note);
    }
    if let Some(card) = from_editor(terminal, app)? {
        go(app, tx, card, Go::Note);
    }
    Ok(())
}

/// Up and down walk the wrapped rows of the title and note, and stop at their ends.
fn row_move(app: &App, card: &Card, up: bool) -> Option<usize> {
    let text = card.input.value();
    let moved = wrap::vertical(
        text,
        card.input.cursor(),
        app.text_width.get(),
        if up { -1 } else { 1 },
    );
    match (card.field, up) {
        (Field::Title, true) => Some(moved.unwrap_or(0)),
        (Field::Note, false) => Some(moved.unwrap_or(text.chars().count())),
        (Field::Title | Field::Note, _) => moved,
        _ => None,
    }
}

/// The note has many lines, so Home, End, ctrl-u and ctrl-k stop at the line, not the whole note.
fn line_edit(input: &mut Input, key: KeyEvent) -> bool {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let chars: Vec<char> = input.value().chars().collect();
    let cursor = input.cursor();
    let start = chars[..cursor]
        .iter()
        .rposition(|ch| *ch == '\n')
        .map_or(0, |at| at + 1);
    let end = chars[cursor..]
        .iter()
        .position(|ch| *ch == '\n')
        .map_or(chars.len(), |at| cursor + at);
    let without =
        |from: usize, to: usize| -> String { chars[..from].iter().chain(&chars[to..]).collect() };
    *input = match (key.code, ctrl) {
        (KeyCode::Home, _) | (KeyCode::Char('a'), true) => input.clone().with_cursor(start),
        (KeyCode::End, _) | (KeyCode::Char('e'), true) => input.clone().with_cursor(end),
        (KeyCode::Char('u'), true) => Input::new(without(start, cursor)).with_cursor(start),
        (KeyCode::Char('k'), true) => Input::new(without(cursor, end)).with_cursor(cursor),
        _ => return false,
    };
    true
}

/// ctrl-c puts the field back as it was; on an untouched field it closes the card.
fn undo(app: &mut App, mut card: Card) {
    let untouched = card.input.value() == card.original;
    if !untouched {
        card.input = Input::new(card.original.clone());
    }
    app.mode = Mode::Card(card);
    if untouched {
        app.close_card();
        app.say(Tone::Quiet, "closed");
    } else {
        app.say(Tone::Quiet, "change undone");
    }
}

fn from_editor(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> Result<Option<Card>> {
    let Mode::Card(mut card) = std::mem::replace(&mut app.mode, Mode::Normal) else {
        return Ok(None);
    };
    let edited = if card.field == Field::Note {
        super::editor::edit_text(terminal, app, card.input.value())?
    } else {
        None
    };
    let Some(text) = edited else {
        app.mode = Mode::Card(card);
        return Ok(None);
    };
    card.input = Input::new(text);
    Ok(Some(card))
}

/// The card goes back into the mode before saving, so the save resolves the card's own task.
fn go(app: &mut App, tx: &Sender<Request>, card: Card, step: Go) {
    let field = card.field;
    let draft = card.input.value().to_owned();
    let original = card.original.clone();
    app.mode = Mode::Card(card);
    let here = match save(app, tx, field, &draft, &original) {
        Ok(true) => match field {
            Field::NewSubtask(at) => Field::Subtask(at),
            field => field,
        },
        Ok(false) => field,
        Err(message) => {
            if message != app.status.text {
                app.say(Tone::Bad, message);
            }
            return;
        }
    };
    let count = app.subtask_count();
    let first = if count > 0 {
        Field::Subtask(0)
    } else {
        Field::NewSubtask(0)
    };
    let next = match (step, here) {
        (Go::Close, _) => return app.close_card(),
        (Go::Note, _) | (Go::Prev, Field::Title) => Field::Note,
        (Go::Next | Go::Down, Field::Title) | (Go::Prev, Field::Note) => first,
        (Go::Next, Field::Note) | (Go::Prev, _) => Field::Title,
        (Go::Next, _) => Field::Note,
        (Go::Below, Field::Subtask(pick)) => Field::NewSubtask(pick + 1),
        (Go::Below, _) => Field::Note,
        (Go::Down, Field::Subtask(pick)) if pick + 1 < count => Field::Subtask(pick + 1),
        (Go::Down, Field::Subtask(_)) => Field::NewSubtask(count),
        (Go::Down, Field::NewSubtask(at)) if at < count => Field::Subtask(at),
        (Go::Down, _) => Field::Note,
        (Go::Up, Field::Subtask(at) | Field::NewSubtask(at)) if at > 0 => Field::Subtask(at - 1),
        (Go::Up, Field::Note) => Field::NewSubtask(count),
        (Go::Up, _) => Field::Title,
    };
    match app.card_at(next) {
        Some(card) => app.mode = Mode::Card(card),
        None => app.close_card(),
    }
}

/// Writes the field when it changed. True when a draft row became a subtask.
fn save(
    app: &mut App,
    tx: &Sender<Request>,
    field: Field,
    draft: &str,
    original: &str,
) -> std::result::Result<bool, String> {
    let title = draft.trim();
    let request = match field {
        Field::Note => {
            let note = draft.trim_end_matches('\n');
            if note == original {
                return Ok(false);
            }
            app.update_note(note.to_owned(), original.to_owned())
        }
        Field::NewSubtask(_) if title.is_empty() => return Ok(false),
        _ if title.is_empty() => return Err("a title is required".into()),
        _ if title == original => return Ok(false),
        Field::Title => app.rename(title.to_owned()),
        Field::Subtask(pick) => app.rename_subtask(pick, title.to_owned()),
        Field::NewSubtask(at) => app.add_subtask(at, title.to_owned()),
    };
    if request.is_none() {
        return Err(app.refusal());
    }
    send(tx, request);
    Ok(matches!(field, Field::NewSubtask(_)))
}
