use std::sync::mpsc;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::input::{filter_key, prompt_key};
use super::keys::list_key_for_test as keys_for_test;
use super::schedule;
use super::*;
use crate::config::UiConfig;
use crate::model::Subtask;

fn task(title: &str, done: bool) -> Task {
    let mut task = Task {
        id: Some(title.into()),
        title: title.into(),
        ..Task::default()
    };
    if done {
        task.set_done(true);
    }
    task
}

fn app_with(tasks: Vec<Task>) -> App {
    let mut app = App::new(UiConfig::default());
    app.tasks = tasks;
    app.loaded = true;
    app
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn cached_inbox_paints_without_waiting() {
    let mut app = app_with(vec![task("today", false)]);
    app.cache
        .insert(CacheKey::Inbox, vec![task("later", false)]);
    app.inbox = true;
    app.reveal();
    assert_eq!(app.tasks[0].title, "later");
    assert_eq!(app.want, Some(CacheKey::Inbox));
    assert_ne!(app.status.tone, Tone::Busy);
}

#[test]
fn missing_cache_shows_loading() {
    let mut app = app_with(vec![task("today", false)]);
    app.inbox = true;
    app.reveal();
    assert!(app.tasks.is_empty());
    assert!(!app.loaded);
    assert_eq!(app.status.text, "loading…");
}

#[test]
fn tick_does_not_schedule_a_reload() {
    let mut app = app_with(vec![task("a", false)]);
    assert!(app.toggle().is_some());
    app.finish_write(Ok(None), "done".into());
    assert!(app.tasks[0].done());
    assert!(app.want.is_none());
    assert_eq!(app.status.tone, Tone::Good);
}

#[test]
fn stale_reload_keeps_the_tick() {
    let mut app = app_with(vec![task("a", false)]);
    assert!(app.toggle().is_some());
    app.apply_load(0, CacheKey::Today, vec![task("a", false)], true);
    assert!(app.tasks[0].done());
    app.finish_write(Ok(None), "done".into());
    assert_eq!(app.want, Some(CacheKey::Today));
}

#[test]
fn uncheck_stays_blank_when_the_server_still_has_a_stamp() {
    let stamped = task("a", true);
    let mut app = app_with(vec![stamped.clone()]);
    assert!(app.toggle().is_some());
    assert!(!app.tasks[0].done());
    let epoch = app.epoch;
    app.apply_load(epoch, CacheKey::Today, vec![stamped], true);
    assert!(!app.tasks[0].done());
    assert!(app.tasks[0].completed_at.is_none());
    assert!(app.toggle().is_some());
    assert!(app.tasks[0].done());
}

#[test]
fn failed_write_reverts_and_shows_the_error() {
    let mut app = app_with(vec![task("a", false)]);
    assert!(app.rename("b".into()).is_some());
    assert_eq!(app.tasks[0].title, "b");
    app.finish_write(Err(()), "HTTP 500".into());
    assert_eq!(app.tasks[0].title, "a");
    assert_eq!(app.status.tone, Tone::Bad);
}

#[test]
fn subtask_edits_are_optimistic_and_revert_on_failure() {
    let mut parent = task("parent", false);
    parent.subtasks = vec![Subtask {
        id: Some("s1".into()),
        title: "one".into(),
        completed_at: None,
    }];
    let mut app = app_with(vec![parent]);
    app.focus_subtasks();
    assert_eq!(app.focus, Focus::Subtasks);

    let Some(Request::Subtasks {
        subtasks,
        structural,
        ..
    }) = app.toggle_subtask()
    else {
        panic!("toggle sends the whole list");
    };
    assert!(!structural);
    assert!(subtasks[0].done());
    app.finish_write(Ok(None), "subtasks saved".into());

    let Some(Request::Subtasks {
        subtasks,
        structural,
        ..
    }) = app.add_subtask("two".into())
    else {
        panic!("add sends the whole list");
    };
    assert!(structural);
    assert_eq!(subtasks.len(), 2);
    assert_eq!(subtasks[1].id, None);
    assert_eq!(app.sub_selected, 1);
    app.finish_write(Err(()), "HTTP 500".into());
    assert_eq!(app.tasks[0].subtasks.len(), 1);
    assert_eq!(app.sub_selected, 0);
}

#[test]
fn server_subtask_ids_replace_local_drafts() {
    let mut app = app_with(vec![task("parent", false)]);
    assert!(app.add_subtask("new".into()).is_some());
    let mut reply = task("parent", false);
    reply.subtasks = vec![Subtask {
        id: Some("server-id".into()),
        title: "new".into(),
        completed_at: None,
    }];
    app.finish_write(Ok(Some(reply)), "subtasks saved".into());
    assert_eq!(app.tasks[0].subtasks[0].id.as_deref(), Some("server-id"));
}

#[test]
fn delete_subtask_asks_then_removes() {
    let mut parent = task("parent", false);
    parent.subtasks = vec![Subtask::new("one"), Subtask::new("two")];
    let mut app = app_with(vec![parent]);
    app.focus = Focus::Subtasks;
    app.sub_selected = 1;
    assert!(app.delete_subtask().is_some());
    assert_eq!(app.tasks[0].subtasks.len(), 1);
    assert_eq!(app.sub_selected, 0);
}

#[test]
fn adding_while_filtered_selects_the_new_task() {
    let mut app = app_with(vec![task("alpha", false), task("beta", false)]);
    app.query = "beta".into();
    app.queue_add("gamma".into());
    assert!(app.query.is_empty());
    assert_eq!(app.selected_task().unwrap().title, "gamma");
}

#[test]
fn prompts_edit_with_a_cursor_and_ctrl_c_cancels() {
    let (tx, _rx) = mpsc::channel();
    let mut app = app_with(vec![task("alpha", false)]);
    app.mode = input::prompt(Prompt::Title, "alpha".into());
    prompt_key(&mut app, &tx, key(KeyCode::Home));
    prompt_key(&mut app, &tx, key(KeyCode::Char('>')));
    let Mode::Input { input, .. } = &app.mode else {
        panic!("still editing")
    };
    assert_eq!(input.value(), ">alpha");
    prompt_key(
        &mut app,
        &tx,
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
    );
    assert!(matches!(app.mode, Mode::Normal));
    assert_eq!(app.tasks[0].title, "alpha");
}

#[test]
fn filter_arrows_move_and_ctrl_keys_do_not_type() {
    let mut app = app_with(vec![
        task("alpha", false),
        task("beta notes", false),
        task("gamma notes", false),
    ]);
    app.mode = Mode::Filter;
    app.query = "notes".into();
    filter_key(&mut app, key(KeyCode::Down));
    assert_eq!(app.selected_task().unwrap().title, "gamma notes");
    filter_key(&mut app, key(KeyCode::Up));
    assert_eq!(app.selected_task().unwrap().title, "beta notes");
    filter_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
    );
    assert_eq!(app.query, "notes");
    filter_key(&mut app, key(KeyCode::Char('k')));
    assert_eq!(app.query, "notesk");
}

#[test]
fn search_types_every_letter_and_keeps_the_highlight() {
    let mut app = app_with(vec![
        task("alpha", false),
        task("beta notes", false),
        task("fields", false),
    ]);
    app.mode = Mode::Filter;
    for ch in "fields".chars() {
        filter_key(&mut app, key(KeyCode::Char(ch)));
    }
    assert_eq!(app.query, "fields");
    assert_eq!(app.selected_task().unwrap().title, "fields");
    filter_key(&mut app, key(KeyCode::Esc));
    assert!(app.query.is_empty());
    assert_eq!(app.selected_task().unwrap().title, "fields");
}

#[test]
fn rejected_values_keep_the_prompt_open() {
    let (tx, rx) = mpsc::channel();
    let mut app = app_with(vec![task("alpha", false)]);
    app.mode = input::prompt(Prompt::Duration, "9999".into());
    prompt_key(&mut app, &tx, key(KeyCode::Enter));
    let Mode::Input { input, prompt } = &app.mode else {
        panic!("prompt closed")
    };
    assert_eq!((input.value(), *prompt), ("9999", Prompt::Duration));
    assert_eq!(app.status.tone, Tone::Bad);
    assert!(rx.try_recv().is_err());
}

#[test]
fn new_subtask_prompt_stays_open_for_the_next_item() {
    let (tx, rx) = mpsc::channel();
    let mut app = app_with(vec![task("parent", false)]);
    app.focus = Focus::Subtasks;
    app.mode = input::prompt(Prompt::NewSubtask, "one".into());
    prompt_key(&mut app, &tx, key(KeyCode::Enter));
    assert!(
        matches!(&app.mode, Mode::Input { prompt: Prompt::NewSubtask, input } if input.value().is_empty())
    );
    assert!(rx.try_recv().is_ok());
    prompt_key(&mut app, &tx, key(KeyCode::Enter));
    assert!(matches!(app.mode, Mode::Normal));
    assert_eq!(app.tasks[0].subtasks.len(), 1);
}

#[test]
fn failed_reschedule_restores_only_the_schedule() {
    let mut original = task("a", false);
    original.day = Some(crate::clock::stamp(crate::clock::today()));
    original.start_time = Some(9.0);
    let mut app = app_with(vec![original]);
    assert!(app.reschedule(schedule::Spec::Duration(50)).is_some());
    assert!(app.add_subtask("kept".into()).is_some());
    app.finish_write(Err(()), "HTTP 500".into());
    assert_eq!(app.tasks[0].duration, None);
    assert_eq!(app.tasks[0].subtasks.len(), 1);
}

#[test]
fn split_day_holds_writes_then_adopts_the_new_id() {
    let mut day = task("run", false);
    day.id = Some("occurrence".into());
    day.recurring_id = Some("rid".into());
    day.is_recurring = true;
    day.day = Some("2026-10-23".into());
    let mut app = app_with(vec![day]);
    let Some(Request::Subtasks {
        structural: true, ..
    }) = app.add_subtask("stretch".into())
    else {
        panic!("structural write");
    };
    assert!(app.add_subtask("again".into()).is_none());
    assert_eq!(app.status.tone, Tone::Busy);
    let mut saved = task("run", false);
    saved.id = Some("detached".into());
    saved.day = Some("2026-10-23".into());
    saved.subtasks = vec![Subtask {
        id: Some("s1".into()),
        title: "stretch".into(),
        completed_at: None,
    }];
    app.finish_write(Ok(Some(saved)), "subtasks saved".into());
    let local = &app.tasks[0];
    assert_eq!(
        local.task_ref().unwrap(),
        crate::model::TaskRef::Id("detached".into())
    );
    assert_eq!(local.subtasks[0].id.as_deref(), Some("s1"));
    assert!(app.add_subtask("again".into()).is_some());
}

#[test]
fn added_task_takes_the_server_copy() {
    let mut app = app_with(vec![task("alpha", false)]);
    app.queue_add("gamma".into());
    let mut saved = task("gamma", false);
    saved.id = Some("new-id".into());
    app.finish_write(Ok(Some(saved)), "added".into());
    assert_eq!(app.selected_task().unwrap().id.as_deref(), Some("new-id"));
}

#[test]
fn unsaved_subtask_cannot_be_ticked_and_reorder_moves_it() {
    let mut parent = task("parent", false);
    parent.subtasks = vec![
        Subtask {
            id: Some("s1".into()),
            title: "one".into(),
            completed_at: None,
        },
        Subtask::new("draft"),
    ];
    let mut app = app_with(vec![parent]);
    app.focus = Focus::Subtasks;
    app.sub_selected = 1;
    assert!(app.toggle_subtask().is_none());
    assert!(app.reorder_subtask(-1).is_some());
    assert_eq!(app.tasks[0].subtasks[0].title, "draft");
    assert_eq!(app.sub_selected, 0);
}

#[test]
fn moving_a_recurring_day_is_scoped_to_that_day() {
    let mut day = task("run", false);
    day.recurring_id = Some("rid".into());
    day.is_recurring = true;
    day.day = Some(crate::clock::stamp(crate::clock::today()));
    let mut app = app_with(vec![day]);
    let Some(Request::Edit { scope, patch, .. }) =
        app.reschedule(schedule::Spec::Date(schedule::DateChoice::Inbox))
    else {
        panic!("edit");
    };
    assert_eq!(scope, Some(crate::api::Scope::Day));
    assert!(patch.inbox);
    assert_eq!(app.pending(), 1);
}

#[test]
fn find_matches_subtask_titles() {
    let mut parent = task("parent", false);
    parent.subtasks = vec![Subtask::new("write tests")];
    assert!(matches(&parent, "tests"));
    assert!(!matches(&parent, "missing"));
}

#[test]
fn a_held_write_keeps_the_typed_draft() {
    let (tx, _rx) = mpsc::channel();
    let mut day = task("run", false);
    day.recurring_id = Some("rid".into());
    day.is_recurring = true;
    day.day = Some("2026-10-23".into());
    let mut app = app_with(vec![day]);
    app.focus = Focus::Subtasks;
    app.mode = input::prompt(Prompt::NewSubtask, "Milk".into());
    prompt_key(&mut app, &tx, key(KeyCode::Enter));
    for ch in "Eggs".chars() {
        prompt_key(&mut app, &tx, key(KeyCode::Char(ch)));
    }
    prompt_key(&mut app, &tx, key(KeyCode::Enter));
    let Mode::Input { input, .. } = &app.mode else {
        panic!("prompt closed")
    };
    assert_eq!(input.value(), "Eggs");
}

#[test]
fn queued_writes_still_receive_new_subtask_ids() {
    let mut app = app_with(vec![task("parent", false)]);
    assert!(app.add_subtask("x".into()).is_some());
    assert!(app.rename("renamed".into()).is_some());
    let mut saved = task("parent", false);
    saved.subtasks = vec![Subtask {
        id: Some("sx".into()),
        title: "x".into(),
        completed_at: None,
    }];
    app.finish_write(Ok(Some(saved)), "subtasks saved".into());
    assert_eq!(app.tasks[0].subtasks[0].id.as_deref(), Some("sx"));
}

#[test]
fn a_failed_subtask_write_does_not_undo_a_later_one() {
    let mut app = app_with(vec![task("parent", false)]);
    assert!(app.add_subtask("x".into()).is_some());
    assert!(app.add_subtask("y".into()).is_some());
    app.finish_write(Err(()), "HTTP 500".into());
    assert_eq!(app.tasks[0].subtasks.len(), 2);
}

#[test]
fn holds_are_released_only_by_a_reload_of_their_own_view() {
    let mut day = task("run", false);
    day.recurring_id = Some("rid".into());
    day.is_recurring = true;
    day.day = Some(crate::clock::stamp(crate::clock::today()));
    let mut app = app_with(vec![day.clone()]);
    assert!(app.add_subtask("stretch".into()).is_some());
    app.finish_write(Ok(Some(day.clone())), "subtasks saved".into());
    let epoch = app.epoch;
    app.inbox = true;
    app.apply_load(epoch, CacheKey::Inbox, vec![], true);
    app.inbox = false;
    app.tasks = vec![day.clone()];
    assert!(app.add_subtask("again".into()).is_none());
    app.apply_load(epoch, CacheKey::Today, vec![day], true);
    assert!(app.add_subtask("again".into()).is_some());
}

#[test]
fn the_home_view_is_a_rolling_window_and_w_switches_to_the_day() {
    let today = crate::clock::today();
    let on = |offset: i64, title: &str| Task {
        id: Some(title.into()),
        title: title.into(),
        day: Some(crate::clock::stamp(today + chrono::Duration::days(offset))),
        is_all_day: true,
        ..Task::default()
    };
    let mut app = app_with(vec![
        on(-1, "carried over"),
        on(0, "today"),
        on(5, "next week"),
    ]);
    app.window = true;
    let titles: Vec<&str> = app.visible().iter().map(|t| t.title.as_str()).collect();
    assert_eq!(titles, ["carried over", "today"]);
    assert_eq!(app.layout(), crate::agenda::Layout::Window);
    let (tx, _rx) = mpsc::channel();
    keys_for_test(&mut app, &tx, KeyCode::Char('w'));
    assert!(!app.window);
    assert_eq!(app.want, Some(CacheKey::Today));
    keys_for_test(&mut app, &tx, KeyCode::Char('J'));
    assert_eq!(app.detail_scroll, 3);
    keys_for_test(&mut app, &tx, KeyCode::Char('K'));
    assert_eq!(app.detail_scroll, 0);
}

#[test]
fn zone_key_explains_itself_without_two_zones() {
    let (tx, _rx) = mpsc::channel();
    let mut app = app_with(vec![task("plan", false)]);
    keys_for_test(&mut app, &tx, KeyCode::Char('z'));
    assert!(app.status.text.contains("timezones"));
    assert_eq!(app.tasks.len(), 1);
    assert_eq!(app.want, None);
}
