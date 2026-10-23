use crate::api;
use crate::cli::SubtaskCommand;
use crate::error::{Error, Result};
use crate::mcp::Session;
use crate::model::{Subtask, TaskRef};
use crate::output::{self, Written};

pub fn run(command: &SubtaskCommand, json: bool, dry_run: bool) -> Result<()> {
    let target = match command {
        SubtaskCommand::List { target }
        | SubtaskCommand::Add { target, .. }
        | SubtaskCommand::Done { target, .. }
        | SubtaskCommand::Undo { target, .. }
        | SubtaskCommand::Edit { target, .. }
        | SubtaskCommand::Delete { target, .. } => target,
    };
    let task_ref = target.task_ref()?;
    if let SubtaskCommand::Delete { yes: false, .. } = command {
        return Err(Error::usage_with(
            "pass --yes to delete a subtask",
            &["structured subtask delete REF SUB --yes"],
        ));
    }
    for title in titles(command) {
        if title.trim().is_empty() {
            return Err(Error::usage("the subtask title is empty"));
        }
    }
    let mut session = Session::connect()?;
    session.dry_run = dry_run;
    let mut task = api::find(&mut session, &task_ref)?;
    let subtasks = &mut task.subtasks;
    let (operation, structural) = match command {
        SubtaskCommand::List { .. } => return output::subtasks(json, &task),
        SubtaskCommand::Add { titles, .. } => {
            subtasks.extend(titles.iter().map(|title| Subtask::new(title.trim())));
            ("subtasks added", true)
        }
        SubtaskCommand::Done { sub, .. } | SubtaskCommand::Undo { sub, .. } => {
            let done = matches!(command, SubtaskCommand::Done { .. });
            let at = pick(subtasks, sub, &task_ref)?;
            let operation = if done {
                "subtask done"
            } else {
                "subtask reopened"
            };
            if subtasks[at].done() == done {
                let scope = task_ref.scope();
                let write = Written {
                    operation,
                    scope,
                    target: Some(&task_ref),
                };
                return output::written(json, write, serde_json::json!({ "task": task }));
            }
            subtasks[at].set_done(done);
            (operation, false)
        }
        SubtaskCommand::Edit { sub, title, .. } => {
            let at = pick(subtasks, sub, &task_ref)?;
            subtasks[at].title = title.trim().to_owned();
            ("subtask renamed", true)
        }
        SubtaskCommand::Delete { sub, .. } => {
            let at = pick(subtasks, sub, &task_ref)?;
            subtasks.remove(at);
            ("subtask deleted", true)
        }
    };
    let result = api::set_subtasks(&mut session, &task_ref, &task.subtasks, structural)?;
    let scope = task_ref.scope();
    let write = Written {
        operation,
        scope,
        target: Some(&task_ref),
    };
    output::written(json, write, result)
}

fn titles(command: &SubtaskCommand) -> Vec<&String> {
    match command {
        SubtaskCommand::Add { titles, .. } => titles.iter().collect(),
        SubtaskCommand::Edit { title, .. } => vec![title],
        _ => Vec::new(),
    }
}

pub fn pick(subtasks: &[Subtask], sub: &str, task: &TaskRef) -> Result<usize> {
    let list = format!("structured subtask list {task} --json");
    if let Ok(number) = sub.parse::<usize>() {
        if (1..=subtasks.len()).contains(&number) {
            return Ok(number - 1);
        }
        return Err(Error::Usage {
            message: format!(
                "subtask {number} is out of range, the task has {}",
                subtasks.len()
            ),
            steps: vec![list],
        });
    }
    subtasks
        .iter()
        .position(|subtask| subtask.id.as_deref() == Some(sub))
        .ok_or_else(|| Error::not_found(format!("subtask {sub} not found"), vec![list]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_by_number_or_id() {
        let mut second = Subtask::new("b");
        second.id = Some("s2".into());
        let list = [Subtask::new("a"), second];
        let task = TaskRef::Id("t".into());
        assert_eq!(pick(&list, "1", &task).unwrap(), 0);
        assert_eq!(pick(&list, "s2", &task).unwrap(), 1);
        assert!(matches!(pick(&list, "3", &task), Err(Error::Usage { .. })));
        assert!(matches!(pick(&list, "0", &task), Err(Error::Usage { .. })));
        let missing = pick(&list, "nope", &task).unwrap_err();
        assert_eq!(missing.next_steps(), ["structured subtask list t --json"]);
    }
}
