#![forbid(unsafe_code)]

mod agenda;
mod api;
mod auth;
mod cli;
mod clock;
mod config;
mod error;
mod mcp;
mod model;
mod output;
mod subtask;
mod tui;
mod update;

pub use cli::Cli;
pub use error::Error;
pub use output::report;

use std::io::{IsTerminal, Write, stderr, stdin, stdout};

use clap::CommandFactory;
use clap_complete::generate;

use crate::api::{Filter, NewTask, Patch, Scope};
use crate::cli::Command;
use crate::error::Result;
use crate::mcp::Session;
use crate::output::{View, Written};

pub fn wants_json(cli: &Cli) -> bool {
    cli.json || (!cli.text && !stdout().is_terminal())
}

/// For errors raised before clap finishes: decide JSON from the raw arguments.
pub fn wants_json_raw(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--json")
        || (!args.iter().any(|arg| arg == "--text") && !stdout().is_terminal())
}

fn connect(cli: &Cli) -> Result<Session> {
    let mut session = Session::connect()?;
    session.dry_run = cli.dry_run;
    Ok(session)
}

/// When the server cannot find a plain id, check whether it was one day of a recurring task.
fn explain(session: &mut Session, task_ref: &model::TaskRef, err: Error) -> Error {
    if !matches!(err, Error::NotFound { .. }) || task_ref.is_occurrence() {
        return err;
    }
    match api::find(session, task_ref) {
        Err(hint @ Error::NotFound { .. }) if hint.to_string().contains("recurring") => hint,
        _ => err,
    }
}

fn title(text: &str) -> Result<String> {
    let text = text.trim();
    if text.is_empty() {
        return Err(Error::usage("the title is empty"));
    }
    Ok(text.to_owned())
}

pub fn execute(cli: &Cli) -> Result<()> {
    if cli.dry_run && matches!(cli.command, None | Some(Command::Tui)) {
        return Err(Error::usage_with(
            "--dry-run previews CLI writes; the TUI always saves",
            &["structured tui"],
        ));
    }
    let config = config::load();
    clock::set_zone(match &cli.tz {
        Some(text) => Some(config::pick_zone(&config.zones, text)?),
        None => config.zones.first().map(|zone| zone.tz),
    });
    let Some(command) = &cli.command else {
        if stdout().is_terminal() {
            return tui::run(config);
        }
        let mut spec = Cli::command();
        spec.write_help(&mut stderr())?;
        writeln!(stderr())?;
        return Err(Error::Help);
    };
    let json = wants_json(cli);
    match command {
        Command::Login => {
            if !stdin().is_terminal() || !stdout().is_terminal() {
                return Err(Error::usage_with(
                    "login opens a browser and needs an interactive terminal",
                    &["ask the person to run `structured login` in their own terminal"],
                ));
            }
            Session::login()?;
            output::done(json, "signed in")?;
        }
        Command::Logout => {
            auth::logout()?;
            output::done(json, "signed out")?;
        }
        Command::Account => {
            let mut session = connect(cli)?;
            output::value(json, "account", api::account(&mut session)?)?;
        }
        Command::Today => {
            let mut tasks = api::today(&mut connect(cli)?)?;
            output::tasks(json, View::Day(clock::today()), &mut tasks)?;
        }
        Command::Now => {
            let mut tasks = api::around_now(&mut connect(cli)?)?;
            let clock = agenda::Clock::now();
            tasks.retain(|task| clock.in_window(task));
            output::tasks(json, View::Window, &mut tasks)?;
        }
        Command::Day { date } => {
            let day = clock::day_arg(date)?;
            let mut tasks = api::day(&mut connect(cli)?, &day)?;
            let view = View::Day(clock::parse_stamp(&day).unwrap_or_else(clock::today));
            output::tasks(json, view, &mut tasks)?;
        }
        Command::Inbox => {
            output::tasks(json, View::Inbox, &mut api::inbox(&mut connect(cli)?)?)?;
        }
        Command::List {
            from,
            to,
            done,
            open,
            search,
            limit,
        } => {
            let from = from.as_deref().map(clock::day_arg).transpose()?;
            let to = to.as_deref().map(clock::day_arg).transpose()?;
            if let (Some(from), Some(to)) = (&from, &to)
                && from > to
            {
                return Err(Error::usage(format!("--from {from} is after --to {to}")));
            }
            let filter = Filter {
                from: from.as_deref(),
                to: to.as_deref(),
                completed: (*done || *open).then_some(*done),
                search: search.as_deref(),
                limit: Some(*limit),
            };
            let (mut tasks, truncated) = api::list(&mut connect(cli)?, &filter)?;
            output::tasks_page(json, View::Range, &mut tasks, Some(truncated))?;
        }
        Command::Add {
            title: text,
            day,
            time,
            duration,
            note,
            symbol,
            all_day,
            inbox,
            subtasks,
        } => {
            let start_time = time.as_deref().map(clock::parse_clock).transpose()?;
            let scheduled = !inbox && (day.is_some() || start_time.is_some() || *all_day);
            let day = match (scheduled, day) {
                (false, _) => None,
                (true, Some(day)) => Some(clock::day_arg(day)?),
                (true, None) => Some(clock::stamp(clock::today())),
            };
            let subtasks = subtasks
                .iter()
                .map(|sub| title(sub))
                .collect::<Result<Vec<_>>>()?;
            let draft = NewTask {
                title: title(text)?,
                note: note.clone(),
                all_day: day.is_some() && start_time.is_none(),
                day,
                start_time,
                duration: *duration,
                symbol: symbol.clone(),
                subtasks,
            };
            let result = api::add(&mut connect(cli)?, draft)?;
            let write = Written {
                operation: "created",
                scope: "task",
                target: None,
            };
            output::written(json, write, result)?;
        }
        Command::Done { target } | Command::Undo { target } => {
            let completed = matches!(command, Command::Done { .. });
            let task_ref = target.task_ref()?;
            let mut session = connect(cli)?;
            let result = api::set_completed(&mut session, &task_ref, completed)
                .map_err(|err| explain(&mut session, &task_ref, err))?;
            let write = Written {
                operation: if completed { "completed" } else { "reopened" },
                scope: task_ref.scope(),
                target: Some(&task_ref),
            };
            output::written(json, write, result)?;
        }
        Command::Edit {
            target,
            title: text,
            note,
            move_day,
            time,
            duration,
            all_day,
            inbox,
            scope,
        } => {
            let start_time = time.as_deref().map(clock::parse_clock).transpose()?;
            let mut patch = Patch {
                title: text.as_deref().map(title).transpose()?,
                note: note.clone(),
                day: move_day.as_deref().map(clock::day_arg).transpose()?,
                start_time,
                duration: *duration,
                all_day: if *all_day {
                    Some(true)
                } else {
                    start_time.map(|_| false)
                },
                inbox: *inbox,
                zoned: false,
            };
            let task_ref = target.task_ref()?;
            let scope = scope.map(Scope::from);
            let mut session = connect(cli)?;
            if patch.day.is_some() || patch.start_time.is_some() || *all_day {
                match api::find(&mut session, &task_ref) {
                    Ok(current) => api::anchor(&current, &task_ref, &mut patch),
                    Err(Error::NotFound { .. }) if !task_ref.is_occurrence() => {}
                    Err(err) => return Err(err),
                }
            }
            let result = api::edit(&mut session, &task_ref, patch, scope)
                .map_err(|err| explain(&mut session, &task_ref, err))?;
            let scope = match scope {
                _ if !task_ref.is_occurrence() => "task",
                Some(Scope::Day) => "day",
                _ => "series",
            };
            let write = Written {
                operation: "updated",
                scope,
                target: Some(&task_ref),
            };
            output::written(json, write, result)?;
        }
        Command::Delete { target, yes } => {
            let task_ref = target.task_ref()?;
            if !yes && !cli.dry_run {
                return Err(Error::usage_with(
                    "pass --yes to delete",
                    &[
                        "structured delete REF --dry-run --json to preview; add --yes once the person approves",
                    ],
                ));
            }
            let mut session = connect(cli)?;
            let result = api::delete(&mut session, &task_ref)
                .map_err(|err| explain(&mut session, &task_ref, err))?;
            let scope = task_ref.scope();
            let write = Written {
                operation: "deleted",
                scope,
                target: Some(&task_ref),
            };
            output::written(json, write, result)?;
        }
        Command::Subtask(action) => subtask::run(action, json, cli.dry_run)?,
        Command::Tui => tui::run(config)?,
        Command::Update => update::run(json, cli.dry_run)?,
        Command::Completions { shell } => {
            let mut command = Cli::command();
            generate(*shell, &mut command, "structured", &mut stdout());
        }
    }
    Ok(())
}
