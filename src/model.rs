use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, Result};

pub const DEFAULT_SYMBOL: &str = "text.badge.checkmark";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
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
    pub completed_at: Option<String>,
    pub symbol: Option<String>,
}

impl Task {
    pub fn done(&self) -> bool {
        self.is_completed || self.completed_at.as_ref().is_some_and(|v| !v.is_empty())
    }

    pub fn when(&self) -> String {
        if self.is_in_inbox && self.day.is_none() {
            return "inbox".into();
        }
        if self.is_all_day || self.start_time.is_none() {
            return "all".into();
        }
        clock(self.start_time.unwrap_or(0.0))
    }

    pub fn duration_label(&self) -> String {
        match self.duration {
            Some(mins) if mins > 0 => format!("{mins}m"),
            _ => String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskRef {
    pub id: Option<String>,
    pub recurring_id: Option<String>,
    pub day: Option<String>,
}

impl Task {
    pub fn task_ref(&self) -> Result<TaskRef> {
        if let Some(id) = self.id.clone().filter(|id| !id.is_empty()) {
            return Ok(TaskRef {
                id: Some(id),
                recurring_id: None,
                day: None,
            });
        }
        let recurring_id = self
            .recurring_id
            .clone()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| Error::Api("task has no id".into()))?;
        let day = self
            .day
            .clone()
            .filter(|day| !day.is_empty())
            .ok_or_else(|| Error::Usage("recurring task needs a day".into()))?;
        Ok(TaskRef {
            id: None,
            recurring_id: Some(recurring_id),
            day: Some(day),
        })
    }
}

pub fn parse_ref(id: &str, day: Option<String>) -> TaskRef {
    match day {
        Some(day) => TaskRef {
            id: None,
            recurring_id: Some(id.to_owned()),
            day: Some(day),
        },
        None => TaskRef {
            id: Some(id.to_owned()),
            recurring_id: None,
            day: None,
        },
    }
}

pub fn clock(hours: f64) -> String {
    let hour = hours.floor() as i64;
    let minute = ((hours - hour as f64) * 60.0).round() as i64;
    let (hour, minute) = if minute == 60 {
        (hour + 1, 0)
    } else {
        (hour, minute)
    };
    format!("{hour:02}:{minute:02}")
}

pub fn parse_clock(text: &str) -> Result<f64> {
    if let Ok(value) = text.parse::<f64>() {
        if (0.0..24.0).contains(&value) {
            return Ok(value);
        }
    }
    let (hour, minute) = text
        .split_once(':')
        .ok_or_else(|| Error::Usage(format!("time must be HH:MM, got {text}")))?;
    let hour: f64 = hour
        .parse()
        .map_err(|_| Error::Usage(format!("time must be HH:MM, got {text}")))?;
    let minute: f64 = minute
        .parse()
        .map_err(|_| Error::Usage(format!("time must be HH:MM, got {text}")))?;
    if !(0.0..24.0).contains(&hour) || !(0.0..60.0).contains(&minute) {
        return Err(Error::Usage(format!("time must be HH:MM, got {text}")));
    }
    Ok(hour + minute / 60.0)
}

pub fn tasks_from_tool(result: &Value) -> Result<Vec<Task>> {
    if result
        .get("isError")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err(Error::Api(tool_text(result)));
    }
    if let Some(value) = result.get("structuredContent").filter(|v| !v.is_null()) {
        if let Some(tasks) = tasks_from_value(value) {
            return Ok(tasks);
        }
    }
    let text = tool_text(result);
    let value = extract_json(&text)?;
    tasks_from_value(&value).ok_or_else(|| Error::Api(text))
}

fn tasks_from_value(value: &Value) -> Option<Vec<Task>> {
    if let Some(list) = value.as_array() {
        return serde_json::from_value(Value::Array(list.clone())).ok();
    }
    for key in ["tasks", "items", "inbox", "results", "data"] {
        if let Some(list) = value.get(key).and_then(Value::as_array) {
            if let Ok(tasks) = serde_json::from_value(Value::Array(list.clone())) {
                return Some(tasks);
            }
        }
    }
    None
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
    iana_time_zone::get_timezone().unwrap_or_else(|_| "UTC".into())
}

pub fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_half_hours() {
        assert_eq!(clock(8.5), "08:30");
        assert_eq!(clock(14.0), "14:00");
        assert_eq!(parse_clock("08:30").unwrap(), 8.5);
    }

    #[test]
    fn parses_wrapped_task_list() {
        let raw = r#"Tasks for today:
[{"id":"abc","title":"Standup","start_time":9.0,"duration":30}]"#;
        let value = extract_json(raw).unwrap();
        let tasks = tasks_from_value(&value).unwrap();
        assert_eq!(tasks[0].title, "Standup");
        assert_eq!(tasks[0].when(), "09:00");
    }

    #[test]
    fn recurring_ref_needs_a_day() {
        let task = Task {
            id: None,
            recurring_id: Some("rid".into()),
            title: "Rise".into(),
            note: String::new(),
            day: Some("2026-10-23".into()),
            start_time: None,
            duration: None,
            is_all_day: true,
            is_in_inbox: false,
            is_completed: false,
            completed_at: None,
            symbol: None,
        };
        let task_ref = task.task_ref().unwrap();
        assert_eq!(task_ref.recurring_id.as_deref(), Some("rid"));
        assert_eq!(task_ref.day.as_deref(), Some("2026-10-23"));
    }
}
