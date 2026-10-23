use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::Serialize;
use serde_json::{Value, json};

use crate::agenda::{self, Row};
use crate::clock;
use crate::error::Error;
use crate::model::{Task, TaskRef};

pub enum View {
    Day(chrono::NaiveDate),
    Inbox,
    Range,
}

pub fn report(json: bool, err: &Error) {
    if json {
        let body = json!({
            "ok": false,
            "error": {
                "code": err.code(),
                "message": err.to_string(),
                "retryable": err.retryable(),
                "next_steps": err.next_steps(),
            }
        });
        eprintln!("{body}");
        return;
    }
    eprintln!("error: {err}");
    if let Some(step) = err.next_steps().first() {
        eprintln!("next: {step}");
    }
}

pub fn task_json(task: &Task) -> Value {
    let mut value = serde_json::to_value(task).unwrap_or(Value::Null);
    if let (Some(object), Ok(task_ref)) = (value.as_object_mut(), task.task_ref()) {
        object.insert("ref".into(), json!(task_ref.to_string()));
    }
    value
}

pub fn tasks(json: bool, view: View, tasks: &mut [Task]) -> Result<(), Error> {
    tasks_page(json, view, tasks, None)
}

pub fn tasks_page(
    json: bool,
    view: View,
    tasks: &mut [Task],
    truncated: Option<bool>,
) -> Result<(), Error> {
    if json {
        let list: Vec<Value> = tasks.iter().map(task_json).collect();
        let mut body = json!({"ok": true, "tasks": list});
        if let Some(truncated) = truncated {
            body["truncated"] = json!(truncated);
        }
        return print_json(&body);
    }
    if truncated == Some(true) {
        eprintln!(
            "showing the first {} tasks; raise --limit or narrow the range",
            tasks.len()
        );
    }
    print!("{}", render(view, tasks));
    Ok(())
}

pub fn render(view: View, tasks: &mut [Task]) -> String {
    let mut out = String::new();
    match view {
        View::Day(day) => {
            agenda::sort(tasks, false);
            let refs: Vec<&Task> = tasks.iter().collect();
            let label = clock::relative(day)
                .map(|word| format!(" ({word})"))
                .unwrap_or_default();
            let planned = clock::duration(agenda::planned_minutes(&refs));
            let mut heading = format!("{}{label} · {}", clock::long_date(day), counts(&refs));
            if !planned.is_empty() {
                let _ = write!(heading, " · {planned} planned");
            }
            let now = (day == clock::today()).then(clock::now_hours);
            section(&mut out, &heading, &refs, false, now);
        }
        View::Inbox => {
            agenda::sort(tasks, true);
            let refs: Vec<&Task> = tasks.iter().collect();
            section(
                &mut out,
                &format!("Inbox · {}", counts(&refs)),
                &refs,
                true,
                None,
            );
        }
        View::Range => {
            let mut days: BTreeMap<String, Vec<Task>> = BTreeMap::new();
            for task in tasks.iter() {
                days.entry(task.day.clone().unwrap_or_default())
                    .or_default()
                    .push(task.clone());
            }
            if days.is_empty() {
                out.push_str("No tasks\n");
            }
            for (day, mut group) in days {
                agenda::sort(&mut group, false);
                let refs: Vec<&Task> = group.iter().collect();
                let title =
                    clock::parse_stamp(&day).map_or_else(|| "Inbox".into(), clock::long_date);
                section(
                    &mut out,
                    &format!("{title} · {}", counts(&refs)),
                    &refs,
                    false,
                    None,
                );
            }
        }
    }
    out
}

fn counts(tasks: &[&Task]) -> String {
    let done = tasks.iter().filter(|task| task.done()).count();
    match tasks.len() {
        1 => format!("1 task, {done} done"),
        n => format!("{n} tasks, {done} done"),
    }
}

fn section(out: &mut String, heading: &str, tasks: &[&Task], inbox: bool, now: Option<f64>) {
    let _ = writeln!(out, "{heading}\n");
    if tasks.is_empty() {
        let _ = writeln!(out, "  Nothing here\n");
        return;
    }
    let pad = if inbox { 2 } else { 21 };
    for row in agenda::rows(tasks, inbox, now) {
        match row {
            Row::Section(name) => {
                let _ = writeln!(out, "  {name}");
            }
            Row::Gap(minutes) => {
                let _ = writeln!(out, "{:pad$}    · {} free", "", clock::duration(minutes));
            }
            Row::Now(at) => {
                let _ = writeln!(out, "{:pad$} --- now {} ---", "", clock::clock(at));
            }
            Row::Task { index, overlap } => {
                let task = tasks[index];
                let mut line = if inbox {
                    "  ".to_owned()
                } else {
                    let when = task
                        .timed()
                        .map(|start| clock::span(start, task.minutes()))
                        .unwrap_or_default();
                    format!("  {when:<11} {:>6}  ", clock::duration(task.minutes()))
                };
                let _ = write!(line, "{} {}", check(task.done()), task.title);
                if let Some((done, total)) = task.subtask_progress() {
                    let _ = write!(line, "  ({done}/{total})");
                }
                if task.is_recurring {
                    line.push_str("  repeats");
                }
                if overlap {
                    line.push_str("  ! overlaps");
                }
                let _ = writeln!(out, "{line}");
                for sub in &task.subtasks {
                    let _ = writeln!(out, "{:pad$}    {} {}", "", check(sub.done()), sub.title);
                }
            }
        }
    }
    out.push('\n');
}

fn check(done: bool) -> &'static str {
    if done { "[x]" } else { "[ ]" }
}

pub fn value(json: bool, heading: &str, value: impl Serialize) -> Result<(), Error> {
    if json {
        return print_json(&json!({"ok": true, "result": value}));
    }
    println!("{heading}");
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

/// Every write prints the same envelope so an agent can read `ref` and `scope` without knowing the tool.
pub struct Written<'a> {
    pub operation: &'a str,
    pub scope: &'a str,
    pub target: Option<&'a TaskRef>,
}

pub fn written(json: bool, write: Written, result: Value) -> Result<(), Error> {
    let task = crate::api::task_in(&result);
    let task_ref = task
        .as_ref()
        .and_then(|task| task.task_ref().ok())
        .or_else(|| write.target.cloned())
        .map(|task_ref| task_ref.to_string());
    let operation = result
        .get("operation")
        .and_then(Value::as_str)
        .filter(|server| *server != "updated")
        .unwrap_or(write.operation);
    let mut body = json!({"operation": operation, "scope": write.scope, "ref": task_ref});
    if let Some(task) = &task {
        body["task"] = task_json(task);
    }
    if result.get("dry_run").is_some() {
        body["dry_run"] = json!(true);
        body["tool"] = result["tool"].clone();
        body["arguments"] = result["arguments"].clone();
    }
    if json {
        return print_json(&json!({"ok": true, "result": body}));
    }
    if body.get("dry_run").is_some() {
        println!(
            "dry run, nothing sent: {} {}",
            body["tool"].as_str().unwrap_or_default(),
            body["arguments"]
        );
        return Ok(());
    }
    let title = task
        .as_ref()
        .map(|task| format!(" {}", task.title))
        .unwrap_or_default();
    let scope = if write.scope == "task" {
        String::new()
    } else {
        format!(" ({})", write.scope)
    };
    println!(
        "{operation}{scope}:{title}  ref {}",
        task_ref.unwrap_or_default()
    );
    if let Some(task) = task.filter(|task| !task.subtasks.is_empty()) {
        print_subtasks(&task);
    }
    Ok(())
}

pub fn subtasks(json: bool, task: &Task) -> Result<(), Error> {
    if json {
        let task_ref = task.task_ref().map(|r| r.to_string()).ok();
        let list: Vec<Value> = task
            .subtasks
            .iter()
            .enumerate()
            .map(|(index, sub)| json!({"n": index + 1, "id": sub.id, "title": sub.title, "completed_at": sub.completed_at, "done": sub.done()}))
            .collect();
        return print_json(
            &json!({"ok": true, "ref": task_ref, "title": task.title, "subtasks": list}),
        );
    }
    println!("{}", task.title);
    print_subtasks(task);
    Ok(())
}

fn print_subtasks(task: &Task) {
    if task.subtasks.is_empty() {
        println!("  No subtasks");
    }
    for (number, sub) in task.subtasks.iter().enumerate() {
        println!("  {:>2}. {} {}", number + 1, check(sub.done()), sub.title);
    }
}

pub fn done(json: bool, message: &str) -> Result<(), Error> {
    if json {
        return print_json(&json!({"ok": true}));
    }
    println!("{message}");
    Ok(())
}

fn print_json(value: &Value) -> Result<(), Error> {
    println!("{}", serde_json::to_string(value)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Subtask;

    #[test]
    fn day_text_is_a_timeline() {
        let mut sub = Subtask::new("ma");
        sub.completed_at = Some("2026-10-23T02:00:00Z".into());
        let mut tasks = vec![
            Task {
                title: "Billing export".into(),
                day: Some("2026-10-23".into()),
                start_time: Some(13.0),
                duration: Some(210),
                subtasks: vec![sub, Subtask::new("tests")],
                ..Task::default()
            },
            Task {
                title: "Run".into(),
                day: Some("2026-10-23".into()),
                is_all_day: true,
                is_recurring: true,
                ..Task::default()
            },
        ];
        let day = chrono::NaiveDate::from_ymd_opt(2026, 10, 23).unwrap();
        let text = render(View::Day(day), &mut tasks);
        assert!(text.starts_with("Fri 23 Oct 2026"), "{text}");
        assert!(text.contains("2 tasks, 0 done · 3h 30m planned"), "{text}");
        assert!(text.contains("  All day\n"), "{text}");
        assert!(text.contains("[ ] Run  repeats"), "{text}");
        assert!(
            text.contains("  13:00-16:30 3h 30m  [ ] Billing export  (1/2)"),
            "{text}"
        );
        assert!(text.contains("    [x] ma\n"), "{text}");
    }
}
