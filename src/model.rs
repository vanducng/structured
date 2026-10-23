use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

use crate::clock;
use crate::error::{Error, Result};

pub const DEFAULT_SYMBOL: &str = "text.badge.checkmark";

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct Subtask {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub completed_at: Option<String>,
}

impl Subtask {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_owned(),
            ..Self::default()
        }
    }

    pub fn done(&self) -> bool {
        self.completed_at.as_ref().is_some_and(|at| !at.is_empty())
    }

    pub fn set_done(&mut self, done: bool) {
        self.completed_at = done.then(|| chrono::Utc::now().to_rfc3339());
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct Task {
    pub id: Option<String>,
    pub recurring_id: Option<String>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub note: String,
    pub day: Option<String>,
    pub start_time: Option<f64>,
    pub duration: Option<i64>,
    #[serde(default)]
    pub is_all_day: bool,
    #[serde(default)]
    pub is_in_inbox: bool,
    #[serde(default)]
    pub is_completed: bool,
    #[serde(default)]
    pub is_recurring: bool,
    pub completed_at: Option<String>,
    pub symbol: Option<String>,
    pub color: Option<String>,
    pub energy_level: Option<i64>,
    #[serde(default, deserialize_with = "null_as_empty")]
    pub subtasks: Vec<Subtask>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
    /// The day the server files this task under, when `settle` moved it to another.
    #[serde(skip)]
    pub listed_day: Option<String>,
}

fn null_as_empty<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Vec<Subtask>, D::Error> {
    Ok(Option::deserialize(deserializer)?.unwrap_or_default())
}

impl Task {
    /// The server keeps a task's day in every zone and counts start_time from that day's midnight
    /// in the zone asked for, so a time can fall outside 0..24; move it onto the day it lands on.
    pub fn settle(&mut self) {
        let (Some(start), Some(day)) = (
            self.start_time,
            self.day.as_deref().and_then(clock::parse_stamp),
        ) else {
            return;
        };
        let shift = (start / 24.0).floor();
        if shift == 0.0 || self.is_all_day {
            return;
        }
        self.listed_day = self.day.take();
        self.day = Some(clock::stamp(day + chrono::Duration::days(shift as i64)));
        self.start_time = Some(start - shift * 24.0);
    }

    pub fn listed_day(&self) -> Option<&str> {
        self.listed_day.as_deref().or(self.day.as_deref())
    }

    /// The task's own zone. None means it floats: the same clock time wherever it is read.
    pub fn zone(&self) -> Option<chrono_tz::Tz> {
        self.extra
            .get("timezone")
            .and_then(Value::as_str)
            .and_then(|name| name.parse().ok())
    }

    pub fn done(&self) -> bool {
        self.is_completed || self.completed_at.as_ref().is_some_and(|v| !v.is_empty())
    }

    pub fn set_done(&mut self, done: bool) {
        self.is_completed = done;
        self.completed_at = done.then(|| chrono::Utc::now().to_rfc3339());
    }

    pub fn timed(&self) -> Option<f64> {
        if self.is_all_day || (self.is_in_inbox && self.day.is_none()) {
            return None;
        }
        self.start_time
    }

    pub fn minutes(&self) -> i64 {
        self.duration.unwrap_or(0).max(0)
    }

    pub fn end(&self) -> Option<f64> {
        self.timed()
            .map(|start| start + self.minutes() as f64 / 60.0)
    }

    pub fn subtask_progress(&self) -> Option<(usize, usize)> {
        if self.subtasks.is_empty() {
            return None;
        }
        let done = self.subtasks.iter().filter(|sub| sub.done()).count();
        Some((done, self.subtasks.len()))
    }

    /// Recurring occurrences must be addressed by recurring_id + day, never by their occurrence id.
    pub fn task_ref(&self) -> Result<TaskRef> {
        let id = self.id.as_deref().filter(|id| !id.is_empty());
        let recurring_id = self.recurring_id.as_deref().filter(|id| !id.is_empty());
        let day = self.listed_day().filter(|day| !day.is_empty());
        match (id, recurring_id, day) {
            (Some(id), _, _) if !self.is_recurring || recurring_id.is_none() => {
                Ok(TaskRef::Id(id.to_owned()))
            }
            (_, Some(recurring_id), Some(day)) => Ok(TaskRef::Occurrence {
                recurring_id: recurring_id.to_owned(),
                day: day.to_owned(),
            }),
            (_, Some(_), None) => Err(Error::usage("recurring task needs a day")),
            _ => Err(Error::Api("task has no id yet".into())),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TaskRef {
    Id(String),
    Occurrence { recurring_id: String, day: String },
}

impl TaskRef {
    /// `RECURRING_ID@YYYY-MM-DD` names one occurrence; `--day` keeps the older two-argument form working.
    pub fn parse(text: &str, day: Option<&str>) -> Result<Self> {
        let text = text.trim();
        let (id, inline_day) = match text.split_once('@') {
            Some((id, day)) => (id, Some(day)),
            None => (text, None),
        };
        if id.is_empty() {
            return Err(Error::usage("task ref is empty"));
        }
        match inline_day.or(day) {
            Some(day) => {
                let day = clock::parse_stamp(day).ok_or_else(|| {
                    Error::usage(format!("occurrence day must be YYYY-MM-DD, got {day}"))
                })?;
                Ok(Self::Occurrence {
                    recurring_id: id.to_owned(),
                    day: clock::stamp(day),
                })
            }
            None => Ok(Self::Id(id.to_owned())),
        }
    }

    pub fn is_occurrence(&self) -> bool {
        matches!(self, Self::Occurrence { .. })
    }

    /// The `scope` a write on this ref reports when it cannot reach the whole series.
    pub fn scope(&self) -> &'static str {
        if self.is_occurrence() { "day" } else { "task" }
    }
}

impl fmt::Display for TaskRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Id(id) => f.write_str(id),
            Self::Occurrence { recurring_id, day } => write!(f, "{recurring_id}@{day}"),
        }
    }
}

pub fn tasks_from_tool(result: &Value) -> Result<Vec<Task>> {
    if result
        .get("isError")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err(Error::Api(tool_text(result)));
    }
    let structured = result.get("structuredContent").filter(|v| !v.is_null());
    if let Some(tasks) = structured.and_then(tasks_from_value) {
        return Ok(tasks);
    }
    let text = tool_text(result);
    let value = extract_json(&text)?;
    tasks_from_value(&value).ok_or_else(|| Error::Api(text))
}

pub fn tasks_from_value(value: &Value) -> Option<Vec<Task>> {
    let list = value.as_array().or_else(|| {
        ["tasks", "items", "inbox", "results", "data"]
            .iter()
            .find_map(|key| value.get(key).and_then(Value::as_array))
    })?;
    let mut tasks: Vec<Task> = serde_json::from_value(Value::Array(list.clone())).ok()?;
    for task in &mut tasks {
        task.is_completed = task.done();
        task.settle();
    }
    Some(tasks)
}

pub fn tool_text(result: &Value) -> String {
    result
        .get("content")
        .and_then(Value::as_array)
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|block| block.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

pub fn extract_json(text: &str) -> Result<Value> {
    let start = text
        .find(['[', '{'])
        .ok_or_else(|| Error::Api(format!("no JSON in response: {text}")))?;
    let mut deserializer = serde_json::Deserializer::from_str(&text[start..]);
    Ok(Value::deserialize(&mut deserializer)?)
}

pub fn timezone() -> String {
    clock::zone_name()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settle_moves_out_of_range_times_and_keeps_the_filed_day_in_the_ref() {
        let mut task = Task {
            id: Some("o".into()),
            recurring_id: Some("r".into()),
            is_recurring: true,
            day: Some("2026-10-24".into()),
            start_time: Some(25.5),
            ..Task::default()
        };
        task.settle();
        assert_eq!(task.day.as_deref(), Some("2026-10-25"));
        assert_eq!(task.start_time, Some(1.5));
        assert_eq!(task.task_ref().unwrap().to_string(), "r@2026-10-24");
        let mut plain = Task {
            day: Some("2026-10-24".into()),
            start_time: Some(8.0),
            ..Task::default()
        };
        plain.settle();
        assert_eq!(plain.day.as_deref(), Some("2026-10-24"));
        assert_eq!(plain.listed_day, None);
    }

    #[test]
    fn parses_wrapped_task_list_with_subtasks() {
        let raw = r#"Tasks for today:
[{"id":"abc","title":"Standup","start_time":9.0,"duration":30,"completed_at":"2026-10-23T01:00:00Z",
  "subtasks":[{"id":"s1","title":"notes","completed_at":null}],"order_index":3}]"#;
        let tasks = tasks_from_value(&extract_json(raw).unwrap()).unwrap();
        assert_eq!(tasks[0].timed(), Some(9.0));
        assert!(tasks[0].is_completed);
        assert_eq!(tasks[0].subtask_progress(), Some((0, 1)));
        assert_eq!(tasks[0].extra["order_index"], 3);
        let null_subtasks = r#"[{"id":"a","title":"x","subtasks":null}]"#;
        assert!(
            tasks_from_value(&extract_json(null_subtasks).unwrap()).unwrap()[0]
                .subtasks
                .is_empty()
        );
    }

    #[test]
    fn recurring_occurrence_uses_recurring_id_and_day() {
        let task = Task {
            id: Some("occurrence".into()),
            recurring_id: Some("rid".into()),
            day: Some("2026-10-23".into()),
            is_recurring: true,
            ..Task::default()
        };
        let task_ref = task.task_ref().unwrap();
        assert_eq!(task_ref.to_string(), "rid@2026-10-23");
        let plain = Task {
            id: Some("abc".into()),
            ..Task::default()
        };
        assert_eq!(plain.task_ref().unwrap(), TaskRef::Id("abc".into()));
    }

    #[test]
    fn parses_refs() {
        assert_eq!(
            TaskRef::parse("abc", None).unwrap(),
            TaskRef::Id("abc".into())
        );
        let occurrence = TaskRef::parse("rid@2026-10-23", None).unwrap();
        assert_eq!(
            occurrence,
            TaskRef::parse("rid", Some("2026-10-23")).unwrap()
        );
        assert!(occurrence.is_occurrence());
        assert!(TaskRef::parse("rid@soon", None).is_err());
        assert!(TaskRef::parse("", None).is_err());
    }
}
