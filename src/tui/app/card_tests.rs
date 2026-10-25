use std::sync::mpsc::{self, Receiver, Sender};

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::*;
use crate::config::UiConfig;
use crate::model::Subtask;
use crate::tui::worker::Reply;

fn parent(subtasks: &[&str], note: &str) -> App {
    let task = Task {
        id: Some("p".into()),
        title: "alpha".into(),
        note: note.into(),
        subtasks: subtasks.iter().map(|title| Subtask::new(title)).collect(),
        ..Task::default()
    };
    let mut app = App::new(UiConfig::default());
    app.tasks = vec![task];
    app.loaded = true;
    app.text_width.set(20);
    app
}

fn press(app: &mut App, tx: &Sender<Request>, code: KeyCode) {
    card_keys::key(app, tx, KeyEvent::new(code, KeyModifiers::NONE));
}

fn ctrl(app: &mut App, tx: &Sender<Request>, ch: char) {
    card_keys::key(
        app,
        tx,
        KeyEvent::new(KeyCode::Char(ch), KeyModifiers::CONTROL),
    );
}

fn typed(app: &mut App, tx: &Sender<Request>, text: &str) {
    for ch in text.chars() {
        press(app, tx, KeyCode::Char(ch));
    }
}

fn at(app: &App) -> (Field, String, usize) {
    let Mode::Card(card) = &app.mode else {
        panic!("card closed");
    };
    (card.field, card.input.value().into(), card.input.cursor())
}

fn sent(rx: &Receiver<Request>) -> Vec<Request> {
    rx.try_iter().collect()
}

#[test]
fn each_field_saves_as_the_cursor_leaves_it() {
    let (tx, rx) = mpsc::channel();
    let mut app = parent(&["one"], "old");
    app.open_card(Field::Title);
    typed(&mut app, &tx, " 2");
    press(&mut app, &tx, KeyCode::Tab);
    assert_eq!(at(&app).0, Field::Subtask(0));
    assert!(
        matches!(&sent(&rx)[..], [Request::Edit { patch, .. }] if patch.title.as_deref() == Some("alpha 2"))
    );
    press(&mut app, &tx, KeyCode::Tab);
    assert_eq!(at(&app).0, Field::Note);
    assert!(sent(&rx).is_empty(), "an untouched field does not write");
    typed(&mut app, &tx, " new");
    press(&mut app, &tx, KeyCode::Esc);
    assert!(matches!(app.mode, Mode::Normal));
    assert_eq!(app.tasks[0].note, "old new");
    assert!(
        matches!(&sent(&rx)[..], [Request::Edit { patch, .. }] if patch.note.as_deref() == Some("old new"))
    );
}

#[test]
fn tab_cycles_sections_and_arrows_cross_them() {
    let (tx, _rx) = mpsc::channel();
    let mut app = parent(&[], "");
    app.open_card(Field::Title);
    press(&mut app, &tx, KeyCode::Tab);
    assert_eq!(at(&app).0, Field::NewSubtask(0));
    press(&mut app, &tx, KeyCode::Tab);
    assert_eq!(at(&app).0, Field::Note);
    press(&mut app, &tx, KeyCode::Tab);
    assert_eq!(at(&app).0, Field::Title);
    press(&mut app, &tx, KeyCode::BackTab);
    assert_eq!(at(&app).0, Field::Note);
    press(&mut app, &tx, KeyCode::Up);
    assert_eq!(at(&app).0, Field::NewSubtask(0));
    press(&mut app, &tx, KeyCode::Up);
    assert_eq!(at(&app).0, Field::Title);
}

#[test]
fn enter_adds_a_row_below_and_an_empty_row_moves_on() {
    let (tx, rx) = mpsc::channel();
    let mut app = parent(&["a", "b"], "");
    app.open_card(Field::Subtask(0));
    press(&mut app, &tx, KeyCode::Enter);
    assert_eq!(at(&app).0, Field::NewSubtask(1));
    typed(&mut app, &tx, "mid");
    press(&mut app, &tx, KeyCode::Enter);
    let titles: Vec<_> = app.tasks[0]
        .subtasks
        .iter()
        .map(|sub| sub.title.as_str())
        .collect();
    assert_eq!(titles, ["a", "mid", "b"]);
    assert_eq!(at(&app).0, Field::NewSubtask(2));
    assert_eq!(sent(&rx).len(), 1);
    press(&mut app, &tx, KeyCode::Down);
    assert_eq!(at(&app).0, Field::Subtask(2));
    press(&mut app, &tx, KeyCode::Enter);
    press(&mut app, &tx, KeyCode::Backspace);
    assert_eq!(
        at(&app).0,
        Field::Subtask(2),
        "backspace on an empty row goes up"
    );
    press(&mut app, &tx, KeyCode::Down);
    press(&mut app, &tx, KeyCode::Enter);
    assert_eq!(at(&app).0, Field::Note);
    assert_eq!(app.tasks[0].subtasks.len(), 3);
}

#[test]
fn a_refused_save_keeps_the_draft() {
    let (tx, rx) = mpsc::channel();
    let mut app = parent(&[], "");
    app.open_card(Field::Title);
    ctrl(&mut app, &tx, 'u');
    press(&mut app, &tx, KeyCode::Esc);
    assert_eq!(at(&app), (Field::Title, String::new(), 0));
    assert_eq!(app.status.text, "a title is required");
    app.mode = Mode::Normal;

    let mut day = app.tasks[0].clone();
    day.recurring_id = Some("rid".into());
    day.is_recurring = true;
    day.day = Some("2026-10-23".into());
    app.tasks = vec![day];
    app.open_card(Field::NewSubtask(0));
    typed(&mut app, &tx, "Milk");
    press(&mut app, &tx, KeyCode::Enter);
    typed(&mut app, &tx, "Eggs");
    press(&mut app, &tx, KeyCode::Enter);
    assert_eq!(at(&app).1, "Eggs", "a split day holds the next write");
    assert_eq!(sent(&rx).len(), 1);
}

#[test]
fn ctrl_c_undoes_the_field_then_closes() {
    let (tx, rx) = mpsc::channel();
    let mut app = parent(&[], "");
    app.open_card(Field::Title);
    typed(&mut app, &tx, "!");
    ctrl(&mut app, &tx, 'c');
    assert_eq!(at(&app).1, "alpha");
    ctrl(&mut app, &tx, 'c');
    assert!(matches!(app.mode, Mode::Normal));
    assert!(sent(&rx).is_empty());
}

#[test]
fn the_note_edits_by_line() {
    let (tx, _rx) = mpsc::channel();
    let mut app = parent(&[], "first line\nsecond");
    app.open_card(Field::Note);
    press(&mut app, &tx, KeyCode::Up);
    assert_eq!(at(&app).2, 6, "same column one row up");
    press(&mut app, &tx, KeyCode::End);
    press(&mut app, &tx, KeyCode::Enter);
    typed(&mut app, &tx, "x");
    assert_eq!(at(&app).1, "first line\nx\nsecond");
    ctrl(&mut app, &tx, 'u');
    assert_eq!(at(&app).1, "first line\n\nsecond");
    press(&mut app, &tx, KeyCode::Down);
    press(&mut app, &tx, KeyCode::Down);
    assert_eq!(at(&app).2, "first line\n\nsecond".chars().count());
    assert_eq!(
        at(&app).0,
        Field::Note,
        "down on the last row stays in the note"
    );
}

#[test]
fn a_reload_without_the_task_closes_the_card() {
    let mut app = parent(&[], "");
    app.open_card(Field::Note);
    app.apply_reply(Reply::Loaded {
        epoch: app.epoch,
        key: CacheKey::Today,
        show: true,
        tasks: vec![],
    });
    assert!(matches!(app.mode, Mode::Normal));
    assert_eq!(app.status.tone, Tone::Bad);
}

#[test]
fn the_card_keeps_its_task_when_the_list_shifts() {
    let (tx, rx) = mpsc::channel();
    let mut app = parent(&[], "");
    let mut beta = app.tasks[0].clone();
    beta.id = Some("b".into());
    beta.title = "beta".into();
    app.tasks.push(beta);
    app.query = "a".into();
    app.open_card(Field::Title);
    ctrl(&mut app, &tx, 'u');
    typed(&mut app, &tx, "zzz");
    press(&mut app, &tx, KeyCode::Tab);
    assert_eq!(
        app.visible().len(),
        1,
        "the renamed task no longer matches the search"
    );
    typed(&mut app, &tx, "milk");
    press(&mut app, &tx, KeyCode::Esc);
    assert_eq!(app.tasks[0].subtasks.len(), 1);
    assert!(app.tasks[1].subtasks.is_empty());
    assert_eq!(sent(&rx).len(), 2);
}

#[test]
fn an_undone_row_does_not_move_the_card_onto_another() {
    let (tx, rx) = mpsc::channel();
    let mut app = parent(&["X", "Z", "W"], "");
    for (sub, id) in app.tasks[0].subtasks.iter_mut().zip(["x", "z", "w"]) {
        sub.id = Some(id.into());
    }
    app.open_card(Field::Subtask(0));
    press(&mut app, &tx, KeyCode::Enter);
    typed(&mut app, &tx, "A");
    press(&mut app, &tx, KeyCode::Enter);
    press(&mut app, &tx, KeyCode::Down);
    assert_eq!(at(&app), (Field::Subtask(2), "Z".into(), 1));
    app.apply_reply(Reply::WriteFailed {
        message: "HTTP 500".into(),
    });
    assert_eq!(at(&app).0, Field::Subtask(1));
    typed(&mut app, &tx, "!");
    press(&mut app, &tx, KeyCode::Esc);
    let titles: Vec<_> = app.tasks[0]
        .subtasks
        .iter()
        .map(|sub| sub.title.as_str())
        .collect();
    assert_eq!(titles, ["X", "Z!", "W"]);
    assert_eq!(sent(&rx).len(), 2);
}

#[test]
fn a_split_day_keeps_the_card_on_the_new_task() {
    let (tx, rx) = mpsc::channel();
    let mut app = parent(&[], "");
    app.tasks[0].recurring_id = Some("rid".into());
    app.tasks[0].is_recurring = true;
    app.tasks[0].day = Some("2026-10-23".into());
    app.open_card(Field::NewSubtask(0));
    typed(&mut app, &tx, "Milk");
    press(&mut app, &tx, KeyCode::Enter);
    let mut saved = app.tasks[0].clone();
    saved.id = Some("detached".into());
    saved.recurring_id = None;
    saved.is_recurring = false;
    app.apply_reply(Reply::Saved {
        message: "subtasks saved",
        task: Some(Box::new(saved)),
    });
    typed(&mut app, &tx, "Eggs");
    press(&mut app, &tx, KeyCode::Enter);
    assert_eq!(at(&app).0, Field::NewSubtask(2));
    let writes = sent(&rx);
    assert!(
        matches!(&writes[..], [_, Request::Subtasks { task_ref, .. }] if *task_ref == crate::model::TaskRef::Id("detached".into()))
    );
}
