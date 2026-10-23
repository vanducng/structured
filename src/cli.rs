use clap::{Parser, Subcommand};
use clap_complete::Shell;

#[derive(Debug, Parser)]
#[command(
    name = "structured",
    version,
    about = "Unofficial CLI and TUI for the Structured day planner. With no command in a terminal, opens the task manager."
)]
pub struct Cli {
    /// Print machine-readable JSON on stdout and errors on stderr.
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Sign in to Structured Cloud. Opens a browser.
    Login,
    /// Forget the saved sign-in.
    Logout,
    /// Show the signed-in plan and limits.
    Account,
    /// List today's tasks.
    Today,
    /// List inbox tasks.
    Inbox,
    /// List tasks in a date range. Defaults to today onward.
    List {
        /// First day, YYYY-MM-DD.
        #[arg(long)]
        from: Option<String>,
        /// Last day, YYYY-MM-DD.
        #[arg(long)]
        to: Option<String>,
    },
    /// Create a task. With no day, it goes to the inbox.
    Add {
        title: String,
        /// YYYY-MM-DD. Makes the task all-day unless --time is set.
        #[arg(long)]
        day: Option<String>,
        /// Start time as HH:MM.
        #[arg(long, conflicts_with = "all_day")]
        time: Option<String>,
        /// Duration in minutes.
        #[arg(long)]
        duration: Option<i64>,
        #[arg(long)]
        note: Option<String>,
        /// Structured symbol code. Defaults to text.badge.checkmark.
        #[arg(long)]
        symbol: Option<String>,
        #[arg(long)]
        all_day: bool,
        #[arg(long, conflicts_with_all = ["day", "time", "all_day"])]
        inbox: bool,
    },
    /// Mark a task done. Pass --day when the id is a recurring id.
    Done {
        id: String,
        #[arg(long)]
        day: Option<String>,
    },
    /// Mark a task not done.
    Undo {
        id: String,
        #[arg(long)]
        day: Option<String>,
    },
    /// Change fields on a task. Pass --day when the id is a recurring id.
    Edit {
        id: String,
        #[arg(long)]
        day: Option<String>,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        note: Option<String>,
        #[arg(long)]
        move_day: Option<String>,
        #[arg(long)]
        time: Option<String>,
        #[arg(long)]
        duration: Option<i64>,
        #[arg(long)]
        all_day: bool,
        #[arg(long)]
        inbox: bool,
    },
    /// Delete a task. Requires --yes. Recurring tasks delete that day only.
    Delete {
        id: String,
        #[arg(long)]
        day: Option<String>,
        #[arg(long)]
        yes: bool,
    },
    /// Open the task manager.
    Tui,
    /// Print a shell completion script.
    Completions { shell: Shell },
}
