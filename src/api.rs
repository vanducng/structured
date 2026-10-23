use chrono::{Duration, NaiveDate};
use serde_json::{Value, json};

use crate::clock;
use crate::error::{Error, Result};
use crate::mcp::Session;
use crate::model::{self, DEFAULT_SYMBOL, Subtask, Task, TaskRef};

#[derive(Default)]
pub struct NewTask {
    pub title: String,
    pub note: Option<String>,
    pub day: Option<String>,
    pub start_time: Option<f64>,
    pub duration: Option<i64>,
    pub all_day: bool,
    pub symbol: Option<String>,
    pub subtasks: Vec<String>,
}

#[derive(Default)]
pub struct Patch {
    pub title: Option<String>,
    pub note: Option<String>,
    pub day: Option<String>,
    pub start_time: Option<f64>,
    pub duration: Option<i64>,
    pub all_day: Option<bool>,
    pub inbox: bool,
    /// Send the zone on screen with the new day or time; set by `anchor`.
    pub zoned: bool,
}

#[derive(Default)]
pub struct Filter<'a> {
    pub from: Option<&'a str>,
    pub to: Option<&'a str>,
    pub completed: Option<bool>,
    pub search: Option<&'a str>,
    pub limit: Option<u8>,
}

pub fn account(session: &mut Session) -> Result<Value> {
    payload(session.call("get_account", json!({}))?)
}

pub fn today(session: &mut Session) -> Result<Vec<Task>> {
    let today = clock::today();
    span(session, today, today)
}

pub fn day(session: &mut Session, day: &str) -> Result<Vec<Task>> {
    let day = clock::parse_stamp(day).ok_or_else(|| Error::usage(format!("bad day {day}")))?;
    span(session, day, day)
}

/// Yesterday, today, and tomorrow, so the rolling window can span midnight in either direction.
pub fn around_now(session: &mut Session) -> Result<Vec<Task>> {
    let today = clock::today();
    span(
        session,
        today - Duration::days(1),
        today + Duration::days(1),
    )
}

fn span(session: &mut Session, from: NaiveDate, to: NaiveDate) -> Result<Vec<Task>> {
    let (from, to) = (clock::stamp(from), clock::stamp(to));
    let filter = Filter {
        from: Some(&from),
        to: Some(&to),
        ..Filter::default()
    };
    Ok(list(session, &filter)?.0)
}

pub fn inbox(session: &mut Session) -> Result<Vec<Task>> {
    session.tasks("get_inbox", json!({}))
}

/// Tasks on `from..=to` in the zone on screen, and whether the server stopped at the limit.
/// A settled task can sit one day off the day it is filed under, so ask one day wider each side.
pub fn list(session: &mut Session, filter: &Filter) -> Result<(Vec<Task>, bool)> {
    let limit = filter.limit.unwrap_or(200);
    let mut args = json!({"timezone": model::timezone(), "limit": limit});
    let from = filter.from.and_then(clock::parse_stamp);
    let to = filter.to.and_then(clock::parse_stamp);
    if let Some(from) = from {
        args["day_from"] = json!(clock::stamp(from - Duration::days(1)));
    }
    if let Some(to) = to {
        args["day_to"] = json!(clock::stamp(to + Duration::days(1)));
    }
    if let Some(completed) = filter.completed {
        args["is_completed"] = json!(completed);
    }
    if let Some(search) = filter.search {
        args["title_contains"] = json!(search);
    }
    let mut tasks = session.tasks("list_tasks", args)?;
    let truncated = tasks.len() >= usize::from(limit);
    tasks.retain(|task| {
        let Some(day) = task.day.as_deref().and_then(clock::parse_stamp) else {
            return true;
        };
        from.is_none_or(|from| day >= from) && to.is_none_or(|to| day <= to)
    });
    Ok((tasks, truncated))
}

/// The server has no get-by-id, so look where a task can live: its day, today, the inbox, then nearby days.
pub fn find(session: &mut Session, task: &TaskRef) -> Result<Task> {
    if let TaskRef::Occurrence { day: on, .. } = task {
        let args =
            json!({"day_from": on, "day_to": on, "timezone": model::timezone(), "limit": 200});
        let filed = session.tasks("list_tasks", args)?;
        return pick(filed, task).unwrap_or_else(|| Err(not_found(task)));
    }
    if let Some(found) = pick(today(session)?, task) {
        return found;
    }
    if let Some(found) = pick(inbox(session)?, task) {
        return found;
    }
    if let Some(found) = pick(list(session, &Filter::default())?.0, task) {
        return found;
    }
    let today = clock::today();
    let from = clock::stamp(today - chrono::Duration::days(60));
    let to = clock::stamp(today - chrono::Duration::days(1));
    let past = Filter {
        from: Some(&from),
        to: Some(&to),
        ..Filter::default()
    };
    pick(list(session, &past)?.0, task).unwrap_or_else(|| Err(not_found(task)))
}

/// A recurring day's plain id is a common slip; point at the ref that works instead.
fn pick(tasks: Vec<Task>, task: &TaskRef) -> Option<Result<Task>> {
    tasks.into_iter().find_map(|candidate| {
        let candidate_ref = candidate.task_ref().ok()?;
        if &candidate_ref == task {
            return Some(Ok(candidate));
        }
        match task {
            TaskRef::Id(id)
                if candidate.id.as_deref() == Some(id) && candidate_ref.is_occurrence() =>
            {
                Some(Err(Error::not_found(
                    format!("{id} is one day of a recurring task; its ref is {candidate_ref}"),
                    vec![format!("use {candidate_ref} as REF")],
                )))
            }
            _ => None,
        }
    })
}

fn not_found(task: &TaskRef) -> Error {
    Error::not_found(
        format!("task {task} not found in the inbox, upcoming days, or the last 60 days"),
        refresh_steps(),
    )
}

fn refresh_steps() -> Vec<String> {
    vec![
        "structured today --json".into(),
        "structured list --from YYYY-MM-DD --to YYYY-MM-DD --json".into(),
    ]
}

/// Rewrites a day or time typed in the zone on screen into update_task's terms: start_time counts
/// from midnight of the day the task is filed under. A one-off task is filed again under the day on
/// screen; a recurring day keeps its day and gets the time in its own zone, so no series is re-zoned.
pub fn anchor(current: &Task, task: &TaskRef, patch: &mut Patch) {
    let occurrence = task.is_occurrence();
    if patch.all_day == Some(true) {
        if !occurrence && patch.day.is_none() && current.listed_day.is_some() {
            patch.day = current.day.clone();
        }
        return;
    }
    if patch.day.is_none() && patch.start_time.is_none() {
        return;
    }
    let Some(start) = patch.start_time.or(current.timed()) else {
        return;
    };
    if occurrence && patch.day.is_none() {
        let shown = current.day.as_deref().and_then(clock::parse_stamp);
        let filed = current.listed_day().and_then(clock::parse_stamp);
        let rebased = match (current.zone(), clock::screen_zone(), shown, filed) {
            (Some(own), Some(screen), Some(shown), Some(filed)) => {
                clock::rebase(shown, start, screen, filed, own)
            }
            _ => None,
        };
        patch.start_time = Some(rebased.unwrap_or(start));
        return;
    }
    patch.zoned = current.zone().is_some() || (current.timed().is_none() && !occurrence);
    patch.day = patch.day.take().or_else(|| current.day.clone());
    patch.start_time = Some(start);
}

/// How far an edit on one day of a recurring task reaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Day,
    Series,
}

pub fn add(session: &mut Session, draft: NewTask) -> Result<Value> {
    let mut args = json!({
        "title": draft.title,
        "symbol": draft.symbol.unwrap_or_else(|| DEFAULT_SYMBOL.to_owned()),
        "timezone": model::timezone(),
    });
    if let Some(note) = draft.note {
        args["note"] = json!(note);
    }
    if let Some(day) = draft.day {
        args["day"] = json!(day);
    }
    if draft.all_day {
        args["is_all_day"] = json!(true);
    }
    if let Some(start) = draft.start_time {
        args["start_time"] = json!(start);
    }
    if let Some(duration) = draft.duration {
        args["duration"] = json!(duration);
    }
    if !draft.subtasks.is_empty() {
        let subtasks: Vec<Value> = draft
            .subtasks
            .iter()
            .map(|title| json!({"title": title}))
            .collect();
        args["subtasks"] = json!(subtasks);
    }
    payload(session.call("create_task", args)?)
}

pub fn set_completed(session: &mut Session, task: &TaskRef, completed: bool) -> Result<Value> {
    let value = if completed {
        session.call("complete_task", ident(task))?
    } else {
        let mut args = ident(task);
        args["is_completed"] = json!(false);
        session.call("update_task", args)?
    };
    payload(value)
}

pub fn edit(
    session: &mut Session,
    task: &TaskRef,
    patch: Patch,
    scope: Option<Scope>,
) -> Result<Value> {
    payload(session.call("update_task", edit_args(task, patch, scope)?)?)
}

/// A recurring day must say whether the edit detaches that day (`only_this`) or rewrites the series (`all`).
fn edit_args(task: &TaskRef, patch: Patch, scope: Option<Scope>) -> Result<Value> {
    let mut args = ident(task);
    let before = args.as_object().map_or(0, serde_json::Map::len);
    if let Some(title) = patch.title {
        args["title"] = json!(title);
    }
    if let Some(note) = patch.note {
        args["note"] = json!(note);
    }
    if let Some(day) = patch.day {
        let key = if task.is_occurrence() {
            "new_day"
        } else {
            "day"
        };
        args[key] = json!(day);
    }
    if let Some(start) = patch.start_time {
        args["start_time"] = json!(start);
    }
    if let Some(duration) = patch.duration {
        args["duration"] = json!(duration);
    }
    if let Some(all_day) = patch.all_day {
        args["is_all_day"] = json!(all_day);
    }
    let patch_inbox = patch.inbox;
    if patch.inbox {
        args["is_in_inbox"] = json!(true);
    }
    if args.as_object().map_or(0, serde_json::Map::len) == before {
        return Err(Error::usage("edit needs at least one change"));
    }
    if patch.zoned {
        args["timezone"] = json!(model::timezone());
    }
    if task.is_occurrence() {
        let action = match scope {
            Some(Scope::Day) => "only_this",
            Some(Scope::Series) if args.get("new_day").is_some() || patch_inbox => {
                return Err(Error::usage_with(
                    "a series cannot move to one day or the inbox; move this day with --scope day",
                    &["structured edit REF --move-day DAY --scope day"],
                ));
            }
            Some(Scope::Series) => "all",
            None => {
                return Err(Error::usage_with(
                    format!("{task} is one day of a recurring task; say how far the edit reaches"),
                    &[
                        "add --scope day to change only this day",
                        "add --scope series to change every repeat",
                    ],
                ));
            }
        };
        args["update_action"] = json!(action);
    }
    Ok(args)
}

/// The server replaces the whole list. Ticking on a recurring occurrence is free and per day.
/// Other changes stay on that day (`only_this`) so one day's ticks never leak into the series.
pub fn set_subtasks(
    session: &mut Session,
    task: &TaskRef,
    subtasks: &[Subtask],
    structural: bool,
) -> Result<Value> {
    let mut args = ident(task);
    args["subtasks"] = Value::Array(subtasks.iter().map(subtask_json).collect());
    if structural && task.is_occurrence() {
        args["update_action"] = json!("only_this");
    }
    payload(session.call("update_task", args)?)
}

fn subtask_json(subtask: &Subtask) -> Value {
    let mut value = json!({"title": subtask.title});
    match &subtask.id {
        Some(id) => {
            value["id"] = json!(id);
            value["completed_at"] = json!(subtask.completed_at);
        }
        None => {
            if let Some(at) = &subtask.completed_at {
                value["completed_at"] = json!(at);
            }
        }
    }
    value
}

pub fn delete(session: &mut Session, task: &TaskRef) -> Result<Value> {
    let mut args = ident(task);
    if task.is_occurrence() {
        args["delete_scope"] = json!("this");
    }
    payload(session.call("delete_task", args)?)
}

/// Unwraps the MCP envelope so callers see the tool's own JSON, not `content[].text` copies.
fn payload(value: Value) -> Result<Value> {
    let text = model::tool_text(&value);
    if value
        .get("isError")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        if text.to_ascii_lowercase().contains("not found") {
            return Err(Error::not_found(text, refresh_steps()));
        }
        return Err(Error::Api(if text.is_empty() {
            "request failed".into()
        } else {
            text
        }));
    }
    if let Some(structured) = value.get("structuredContent").filter(|v| !v.is_null()) {
        return Ok(structured.clone());
    }
    if text.is_empty() {
        return Ok(value);
    }
    Ok(model::extract_json(&text).unwrap_or(Value::String(text)))
}

pub fn task_in(result: &Value) -> Option<Task> {
    let task = result
        .get("task")
        .filter(|task| task.is_object())
        .or_else(|| result.get("title").map(|_| result))?;
    let mut task: Task = serde_json::from_value(task.clone()).ok()?;
    task.is_completed = task.done();
    task.settle();
    Some(task)
}

fn ident(task: &TaskRef) -> Value {
    match task {
        TaskRef::Id(id) => json!({"id": id}),
        TaskRef::Occurrence { recurring_id, day } => {
            json!({"recurring_id": recurring_id, "day": day})
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unwraps_structured_content_and_errors() {
        let ok = json!({"content":[{"type":"text","text":"done"}],"structuredContent":{"task":{"id":"a","title":"x"}}});
        let value = payload(ok).unwrap();
        assert_eq!(task_in(&value).unwrap().title, "x");
        let text_only = json!({"content":[{"type":"text","text":"Created: {\"id\":\"b\"}"}]});
        assert_eq!(payload(text_only).unwrap()["id"], "b");
        let missing = json!({"isError":true,"content":[{"type":"text","text":"Task not found"}]});
        assert!(matches!(payload(missing), Err(Error::NotFound { .. })));
        let failed = json!({"isError":true,"content":[{"type":"text","text":"Unknown symbol"}]});
        assert!(matches!(payload(failed), Err(Error::Api(message)) if message == "Unknown symbol"));
    }

    #[test]
    fn recurring_edits_need_an_explicit_scope() {
        let occurrence = TaskRef::Occurrence {
            recurring_id: "rid".into(),
            day: "2026-10-24".into(),
        };
        let title = || Patch {
            title: Some("Standup".into()),
            ..Patch::default()
        };
        let moved = || Patch {
            day: Some("2026-10-25".into()),
            ..Patch::default()
        };
        assert!(matches!(
            edit_args(&occurrence, title(), None),
            Err(Error::Usage { .. })
        ));
        let args = edit_args(&occurrence, title(), Some(Scope::Series)).unwrap();
        assert_eq!(args["update_action"], "all");
        assert_eq!(args["day"], "2026-10-24");
        assert!(args.get("timezone").is_none());
        let args = edit_args(&occurrence, title(), Some(Scope::Day)).unwrap();
        assert_eq!(args["update_action"], "only_this");
        assert!(matches!(
            edit_args(&occurrence, moved(), Some(Scope::Series)),
            Err(Error::Usage { .. })
        ));
        let args = edit_args(&occurrence, moved(), Some(Scope::Day)).unwrap();
        assert_eq!(args["update_action"], "only_this");
        assert_eq!(args["new_day"], "2026-10-25");
        let plain = edit_args(&TaskRef::Id("a".into()), moved(), None).unwrap();
        assert_eq!(plain, json!({"id": "a", "day": "2026-10-25"}));
        assert!(matches!(
            edit_args(&occurrence, Patch::default(), Some(Scope::Day)),
            Err(Error::Usage { .. })
        ));
    }

    #[test]
    fn anchor_turns_screen_times_into_filed_day_terms() {
        let mut shown = Task {
            id: Some("a".into()),
            day: Some("2026-10-24".into()),
            start_time: Some(-11.0),
            ..Task::default()
        };
        shown
            .extra
            .insert("timezone".into(), json!("America/New_York"));
        shown.settle();
        assert_eq!(shown.day.as_deref(), Some("2026-10-23"));
        assert_eq!(shown.start_time, Some(13.0));
        let one_off = TaskRef::Id("a".into());

        let mut patch = Patch {
            start_time: Some(10.0),
            ..Patch::default()
        };
        anchor(&shown, &one_off, &mut patch);
        assert_eq!(patch.day.as_deref(), Some("2026-10-23"));
        assert_eq!(patch.start_time, Some(10.0));
        assert!(patch.zoned);

        let mut patch = Patch {
            day: Some("2026-10-25".into()),
            ..Patch::default()
        };
        anchor(&shown, &one_off, &mut patch);
        assert_eq!(patch.day.as_deref(), Some("2026-10-25"));
        assert_eq!(patch.start_time, Some(13.0));

        let occurrence = TaskRef::Occurrence {
            recurring_id: "r".into(),
            day: "2026-10-24".into(),
        };
        let mut patch = Patch {
            start_time: Some(14.0),
            ..Patch::default()
        };
        anchor(&shown, &occurrence, &mut patch);
        assert_eq!(patch.day, None);
        assert!(!patch.zoned, "a recurring day keeps its own zone");

        let mut all_day = Patch {
            all_day: Some(true),
            ..Patch::default()
        };
        anchor(&shown, &one_off, &mut all_day);
        assert_eq!(all_day.day.as_deref(), Some("2026-10-23"));

        let floating = Task {
            recurring_id: Some("r".into()),
            is_recurring: true,
            day: Some("2026-10-24".into()),
            start_time: Some(8.0),
            ..Task::default()
        };
        let mut patch = Patch {
            start_time: Some(7.0),
            ..Patch::default()
        };
        anchor(&floating, &occurrence, &mut patch);
        assert_eq!(patch.start_time, Some(7.0));
        assert!(!patch.zoned);

        let routine = Task {
            is_all_day: true,
            start_time: None,
            ..floating.clone()
        };
        let mut patch = Patch {
            start_time: Some(7.0),
            ..Patch::default()
        };
        anchor(&routine, &occurrence, &mut patch);
        assert_eq!(patch.start_time, Some(7.0));
        assert!(
            !patch.zoned,
            "an all-day routine given a time stays floating"
        );

        let mut title_only = Patch {
            title: Some("x".into()),
            ..Patch::default()
        };
        anchor(&shown, &one_off, &mut title_only);
        assert!(title_only.day.is_none() && !title_only.zoned);
    }

    #[test]
    fn subtask_json_keeps_ids_and_omits_new_fields() {
        let mut existing = Subtask::new("a");
        existing.id = Some("s1".into());
        assert_eq!(
            subtask_json(&existing),
            json!({"id":"s1","title":"a","completed_at":null})
        );
        assert_eq!(subtask_json(&Subtask::new("b")), json!({"title":"b"}));
    }
}
