use clap::{Args, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

use crate::api::Scope;
use crate::error::{Error, Result};
use crate::model::TaskRef;

const EXAMPLES: &str = "\
Examples:
  structured today --text                     Today as a readable timeline
  structured day tomorrow --json              One day as JSON
  structured add \"Write the note\" --day +1 --time 09:30 --duration 30 --subtask Outline
  structured done REF                         REF is the \"ref\" field from --json output
  structured edit REF --time 10:00 --scope day
  structured subtask add REF \"Draft\" \"Review\"
  structured subtask done REF 2               Subtasks are picked by 1-based number or id
  structured delete REF --yes --dry-run       Print the exact request without sending it

Output: one JSON object on stdout when --json is set or stdout is not a terminal.
Errors: {\"ok\":false,\"error\":{code,message,retryable,next_steps}} on stderr. Usage errors exit 2.";

#[derive(Debug, Parser)]
#[command(
    name = "structured",
    version,
    about = "Unofficial CLI and TUI for the Structured day planner. With no command in a terminal, opens the task manager.",
    after_help = EXAMPLES
)]
pub struct Cli {
    /// Print one JSON object on stdout. This is the default when stdout is not a terminal.
    #[arg(long, global = true, conflicts_with = "text")]
    pub json: bool,
    /// Print the readable view even when stdout is not a terminal.
    #[arg(long, global = true)]
    pub text: bool,
    /// Print the request a write would send, and send nothing. Reads still run.
    #[arg(long, global = true)]
    pub dry_run: bool,
    /// Read and write days and times in this zone: a label from config.toml's `timezones`, or an IANA name. Defaults to the first configured zone, then the system zone.
    #[arg(long, global = true, value_name = "ZONE")]
    pub tz: Option<String>,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Args)]
pub struct Target {
    /// Task ref from --json output: an id, or RECURRING_ID@YYYY-MM-DD for one day of a recurring task.
    #[arg(value_name = "REF")]
    pub task: String,
    /// Day for a bare recurring id. Same as RECURRING_ID@DAY; do not pass both.
    #[arg(long, value_name = "YYYY-MM-DD")]
    pub day: Option<String>,
}

impl Target {
    pub fn task_ref(&self) -> Result<TaskRef> {
        if self.task.contains('@') && self.day.is_some() {
            return Err(Error::usage("the ref already names a day; drop --day"));
        }
        TaskRef::parse(&self.task, self.day.as_deref())
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ScopeArg {
    /// Only this day. The day is split off from the series.
    Day,
    /// Every repeat of the series.
    Series,
}

impl From<ScopeArg> for Scope {
    fn from(scope: ScopeArg) -> Self {
        match scope {
            ScopeArg::Day => Scope::Day,
            ScopeArg::Series => Scope::Series,
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Sign in to Structured Cloud. Opens a browser; needs an interactive terminal.
    Login,
    /// Forget the saved sign-in.
    Logout,
    /// Show the signed-in plan and limits.
    Account,
    /// List today's tasks.
    Today,
    /// List tasks 12 hours either side of now, across yesterday, today, and tomorrow.
    Now,
    /// List one day's tasks.
    Day {
        /// YYYY-MM-DD, today, tomorrow, yesterday, +N, or -N.
        date: String,
    },
    /// List inbox tasks.
    Inbox,
    /// List tasks in a date range. Defaults to today onward.
    List {
        /// First day. Same forms as `day`.
        #[arg(long)]
        from: Option<String>,
        /// Last day. Same forms as `day`.
        #[arg(long)]
        to: Option<String>,
        /// Only completed tasks.
        #[arg(long, conflicts_with = "open")]
        done: bool,
        /// Only open tasks.
        #[arg(long)]
        open: bool,
        /// Case-insensitive title filter.
        #[arg(long)]
        search: Option<String>,
        /// Most tasks to return. JSON says `"truncated": true` when the limit was reached.
        #[arg(long, default_value_t = 200, value_parser = clap::value_parser!(u8).range(1..=200))]
        limit: u8,
    },
    /// Create a task. No day, time, or --all-day puts it in the inbox.
    Add {
        /// Task title.
        title: String,
        /// YYYY-MM-DD, today, tomorrow, +N, or -N. All-day unless --time is set.
        #[arg(long)]
        day: Option<String>,
        /// Start time, HH:MM or HH. Defaults the day to today.
        #[arg(long, conflicts_with = "all_day")]
        time: Option<String>,
        /// Duration in minutes, 0 to 1439.
        #[arg(long, value_parser = clap::value_parser!(i64).range(0..=1439))]
        duration: Option<i64>,
        /// Note text.
        #[arg(long)]
        note: Option<String>,
        /// Structured symbol code. Defaults to text.badge.checkmark.
        #[arg(long)]
        symbol: Option<String>,
        /// All-day on --day, or today.
        #[arg(long)]
        all_day: bool,
        /// Put it in the inbox. This is also the default with no day or time.
        #[arg(long, conflicts_with_all = ["day", "time", "all_day"])]
        inbox: bool,
        /// Add a subtask. Repeat for more.
        #[arg(long = "subtask", value_name = "TITLE")]
        subtasks: Vec<String>,
    },
    /// Mark a task done. On a recurring task, only that day.
    Done {
        #[command(flatten)]
        target: Target,
    },
    /// Mark a task not done. On a recurring task, only that day.
    Undo {
        #[command(flatten)]
        target: Target,
    },
    /// Change fields on a task. A recurring day needs --scope.
    Edit {
        #[command(flatten)]
        target: Target,
        /// New title.
        #[arg(long)]
        title: Option<String>,
        /// New note. Replaces the old one.
        #[arg(long)]
        note: Option<String>,
        /// Move to another day. Same forms as `day`. With a recurring day, needs --scope day.
        #[arg(long, conflicts_with = "inbox")]
        move_day: Option<String>,
        /// Start time, HH:MM or HH. Makes the task timed.
        #[arg(long, conflicts_with_all = ["all_day", "inbox"])]
        time: Option<String>,
        /// Duration in minutes, 0 to 1439.
        #[arg(long, value_parser = clap::value_parser!(i64).range(0..=1439))]
        duration: Option<i64>,
        /// Make it all-day.
        #[arg(long, conflicts_with = "inbox")]
        all_day: bool,
        /// Move it back to the inbox.
        #[arg(long)]
        inbox: bool,
        /// Required for a recurring day: `day` changes only that day, `series` changes every repeat.
        #[arg(long, value_enum)]
        scope: Option<ScopeArg>,
    },
    /// Delete a task. Requires --yes. On a recurring task, only that day.
    Delete {
        #[command(flatten)]
        target: Target,
        /// Confirm the delete.
        #[arg(long)]
        yes: bool,
    },
    /// Read and change a task's checklist.
    #[command(subcommand, visible_alias = "sub")]
    Subtask(SubtaskCommand),
    /// Open the task manager.
    Tui,
    /// Update to the latest release, in place. Runs install.sh from GitHub; needs curl.
    Update,
    /// Print a shell completion script.
    Completions { shell: Shell },
}

#[derive(Debug, Subcommand)]
pub enum SubtaskCommand {
    /// List a task's subtasks with their numbers and ids.
    List {
        #[command(flatten)]
        target: Target,
    },
    /// Append one or more subtasks.
    Add {
        #[command(flatten)]
        target: Target,
        /// One title per subtask.
        #[arg(required = true, value_name = "TITLE")]
        titles: Vec<String>,
    },
    /// Tick a subtask.
    Done {
        #[command(flatten)]
        target: Target,
        /// 1-based number from `subtask list`, or the subtask id. Prefer the id when the list may change.
        sub: String,
    },
    /// Untick a subtask.
    Undo {
        #[command(flatten)]
        target: Target,
        /// 1-based number from `subtask list`, or the subtask id.
        sub: String,
    },
    /// Rename a subtask.
    Edit {
        #[command(flatten)]
        target: Target,
        /// 1-based number from `subtask list`, or the subtask id.
        sub: String,
        /// New title.
        title: String,
    },
    /// Remove a subtask. Requires --yes.
    Delete {
        #[command(flatten)]
        target: Target,
        /// 1-based number from `subtask list`, or the subtask id.
        sub: String,
        /// Confirm the delete.
        #[arg(long)]
        yes: bool,
    },
}
