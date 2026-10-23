use serde::Serialize;
use serde_json::json;

use crate::error::Error;
use crate::model::Task;

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
}

pub fn tasks(json: bool, heading: &str, tasks: &[Task]) -> Result<(), Error> {
    if json {
        println!(
            "{}",
            serde_json::to_string(&json!({"ok": true, "tasks": tasks}))?
        );
        return Ok(());
    }
    println!("{heading}");
    if tasks.is_empty() {
        println!("(none)");
        return Ok(());
    }
    for task in tasks {
        let mark = if task.done() { "x" } else { " " };
        println!(
            "[{mark}] {:>5} {:>4}  {}",
            task.when(),
            task.duration_label(),
            task.title
        );
    }
    Ok(())
}

pub fn value(json: bool, heading: &str, value: impl Serialize) -> Result<(), Error> {
    if json {
        println!(
            "{}",
            serde_json::to_string(&json!({"ok": true, "result": value}))?
        );
        return Ok(());
    }
    println!("{heading}");
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

pub fn done(json: bool, message: &str) -> Result<(), Error> {
    if json {
        println!("{}", serde_json::to_string(&json!({"ok": true}))?);
        return Ok(());
    }
    println!("{message}");
    Ok(())
}
