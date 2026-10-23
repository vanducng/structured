#![forbid(unsafe_code)]

mod api;
mod auth;
mod cli;
mod error;
mod mcp;
mod model;
mod output;
mod tui;

pub use cli::Cli;
pub use error::Error;
pub use output::report;

use std::io::{IsTerminal, Write, stderr, stdout};

use clap::CommandFactory;
use clap_complete::generate;

use crate::api::{NewTask, Patch};
use crate::error::Result;
use crate::mcp::Session;
use crate::model::parse_ref;

pub fn wants_json(cli: &Cli) -> bool {
    cli.json || !stdout().is_terminal()
}

pub fn execute(cli: &Cli) -> Result<()> {
    let Some(command) = &cli.command else {
        if stdout().is_terminal() {
            tui::run()?;
            return Ok(());
        }
        let mut spec = Cli::command();
        spec.write_help(&mut stderr())?;
        writeln!(stderr())?;
        return Err(Error::Help);
    };
    match command {
        cli::Command::Login => {
            Session::login()?;
            output::done(cli.json, "signed in")?;
        }
        cli::Command::Logout => {
            auth::logout()?;
            output::done(cli.json, "signed out")?;
        }
        cli::Command::Account => {
            let mut session = Session::connect()?;
            output::value(cli.json, "account", api::account(&mut session)?)?;
        }
        cli::Command::Today => {
            let mut session = Session::connect()?;
            let tasks = api::today(&mut session)?;
            output::tasks(
                wants_json(cli),
                &format!("Today  {}", model::today()),
                &tasks,
            )?;
        }
        cli::Command::Inbox => {
            let mut session = Session::connect()?;
            output::tasks(wants_json(cli), "Inbox", &api::inbox(&mut session)?)?;
        }
        cli::Command::List { from, to } => {
            let mut session = Session::connect()?;
            let tasks = api::list(&mut session, from.as_deref(), to.as_deref())?;
            output::tasks(wants_json(cli), "Tasks", &tasks)?;
        }
        cli::Command::Add {
            title,
            day,
            time,
            duration,
            note,
            symbol,
            all_day,
            inbox,
        } => {
            let mut session = Session::connect()?;
            let start_time = match time {
                Some(text) => Some(crate::model::parse_clock(text)?),
                None => None,
            };
            let (day, all_day) = if *inbox || day.is_none() {
                (None, false)
            } else if start_time.is_none() {
                (day.clone(), true)
            } else {
                (day.clone(), *all_day)
            };
            let result = api::add(
                &mut session,
                NewTask {
                    title: title.clone(),
                    note: note.clone(),
                    day,
                    start_time,
                    duration: *duration,
                    all_day,
                    symbol: symbol.clone(),
                },
            )?;
            output::value(wants_json(cli), "created", result)?;
        }
        cli::Command::Done { id, day } => {
            let mut session = Session::connect()?;
            let result = api::set_completed(&mut session, &parse_ref(id, day.clone()), true)?;
            output::value(wants_json(cli), "done", result)?;
        }
        cli::Command::Undo { id, day } => {
            let mut session = Session::connect()?;
            let result = api::set_completed(&mut session, &parse_ref(id, day.clone()), false)?;
            output::value(wants_json(cli), "reopened", result)?;
        }
        cli::Command::Edit {
            id,
            day,
            title,
            note,
            move_day,
            time,
            duration,
            all_day,
            inbox,
        } => {
            let mut session = Session::connect()?;
            let result = api::edit(
                &mut session,
                &parse_ref(id, day.clone()),
                Patch {
                    title: title.clone(),
                    note: note.clone(),
                    day: move_day.clone(),
                    start_time: match time {
                        Some(text) => Some(model::parse_clock(text)?),
                        None => None,
                    },
                    duration: *duration,
                    all_day: if *all_day { Some(true) } else { None },
                    inbox: *inbox,
                },
            )?;
            output::value(wants_json(cli), "updated", result)?;
        }
        cli::Command::Delete { id, day, yes } => {
            if !yes {
                return Err(Error::Usage("pass --yes to delete".into()));
            }
            let mut session = Session::connect()?;
            let result = api::delete(&mut session, &parse_ref(id, day.clone()))?;
            output::value(wants_json(cli), "deleted", result)?;
        }
        cli::Command::Tui => tui::run()?,
        cli::Command::Completions { shell } => {
            let mut command = Cli::command();
            generate(*shell, &mut command, "structured", &mut stdout());
        }
    }
    Ok(())
}
