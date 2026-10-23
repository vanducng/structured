use serde_json::{Value, json};

use crate::error::{Error, Result};
use crate::mcp::Session;
use crate::model::{self, DEFAULT_SYMBOL, Task, TaskRef};

pub struct NewTask {
    pub title: String,
    pub note: Option<String>,
    pub day: Option<String>,
    pub start_time: Option<f64>,
    pub duration: Option<i64>,
    pub all_day: bool,
    pub symbol: Option<String>,
}

pub struct Patch {
    pub title: Option<String>,
    pub note: Option<String>,
    pub day: Option<String>,
    pub start_time: Option<f64>,
    pub duration: Option<i64>,
    pub all_day: Option<bool>,
    pub inbox: bool,
    pub new_day: Option<String>,
}

pub fn account(session: &mut Session) -> Result<Value> {
    session.call("get_account", json!({}))
}

pub fn today(session: &mut Session) -> Result<Vec<Task>> {
    session.tasks("get_today", json!({"timezone": model::timezone()}))
}

pub fn day(session: &mut Session, day: &str) -> Result<Vec<Task>> {
    session.tasks(
        "get_tasks_for_day",
        json!({"day": day, "timezone": model::timezone()}),
    )
}

pub fn inbox(session: &mut Session) -> Result<Vec<Task>> {
    session.tasks("get_inbox", json!({}))
}

pub fn list(session: &mut Session, from: Option<&str>, to: Option<&str>) -> Result<Vec<Task>> {
    let mut args = json!({"timezone": model::timezone(), "limit": 200});
    if let Some(from) = from {
        args["day_from"] = json!(from);
    }
    if let Some(to) = to {
        args["day_to"] = json!(to);
    }
    session.tasks("list_tasks", args)
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
    ensure_ok(session.call("create_task", args)?)
}

pub fn set_completed(session: &mut Session, task: &TaskRef, completed: bool) -> Result<Value> {
    let value = if completed {
        session.call("complete_task", ident(task)?)?
    } else {
        let mut args = ident(task)?;
        args["is_completed"] = json!(false);
        if args.get("recurring_id").is_some() {
            args["update_action"] = json!("only_this");
        }
        session.call("update_task", args)?
    };
    ensure_ok(value)
}

pub fn edit(session: &mut Session, task: &TaskRef, patch: Patch) -> Result<Value> {
    let mut args = ident(task)?;
    if let Some(title) = patch.title {
        args["title"] = json!(title);
    }
    if let Some(note) = patch.note {
        args["note"] = json!(note);
    }
    if let Some(day) = patch.day {
        args["day"] = json!(day);
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
    if let Some(new_day) = patch.new_day {
        args["new_day"] = json!(new_day);
    }
    if patch.inbox {
        args["is_in_inbox"] = json!(true);
    }
    let fields = args.as_object().map(serde_json::Map::len).unwrap_or(0);
    if fields == 1 || (args.get("recurring_id").is_some() && fields == 2) {
        return Err(Error::Usage("edit needs at least one change".into()));
    }
    if args.get("recurring_id").is_some() {
        args["update_action"] = json!("only_this");
    }
    ensure_ok(session.call("update_task", args)?)
}

pub fn delete(session: &mut Session, task: &TaskRef) -> Result<Value> {
    let mut args = ident(task)?;
    if args.get("recurring_id").is_some() {
        args["delete_scope"] = json!("this");
    }
    ensure_ok(session.call("delete_task", args)?)
}

fn ensure_ok(value: Value) -> Result<Value> {
    if value
        .get("isError")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let message = model::tool_text(&value);
        return Err(Error::Api(if message.is_empty() {
            "request failed".into()
        } else {
            message
        }));
    }
    Ok(value)
}

fn ident(task: &TaskRef) -> Result<Value> {
    if let Some(id) = &task.id {
        return Ok(json!({"id": id}));
    }
    let recurring_id = task
        .recurring_id
        .as_deref()
        .ok_or_else(|| Error::Usage("task id is required".into()))?;
    let day = task
        .day
        .as_deref()
        .ok_or_else(|| Error::Usage("recurring task needs --day YYYY-MM-DD".into()))?;
    Ok(json!({"recurring_id": recurring_id, "day": day}))
}
