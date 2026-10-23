# structured

Unofficial command-line and terminal interface for the [Structured](https://structured.app) day planner.

This project is not affiliated with Structured, unorderly, or Leo Mehlig.

`structured` is a CLI for scripts and agents, and a vim-style TUI for a person. Both talk to Structured Cloud. A terminal with no subcommand opens the TUI. A pipe with no subcommand prints help and exits 2.

## Install

From a GitHub release, unpack the archive for your machine and put `structured` on `PATH`.

Or build from the tagged source:

```bash
cargo install --git https://github.com/vanducng/structured
```

Sign in once, in a terminal:

```bash
structured login
```

The browser sign-in is stored at `$XDG_CONFIG_HOME/structured/auth.json`, or `~/.config/structured/auth.json`. The file is mode 600. The tool never prints it.

## Commands

`--help` is the command list. `--json` prints one JSON object on stdout. The same JSON is used when stdout is not a terminal. Errors go to stderr as `{ "ok": false, "error": { "code", "message", "retryable", "next_steps" } }`. Usage errors exit 2. Other errors exit 1.

```bash
structured login
structured logout
structured account
structured today
structured inbox
structured list --from 2026-10-01 --to 2026-10-31
structured add "Write the note" --day 2026-10-24 --time 09:30 --duration 30
structured add "Someday" --inbox
structured done TASK_ID
structured undo TASK_ID
structured edit TASK_ID --title "New title"
structured delete TASK_ID --yes
structured tui
structured completions zsh
```

A task with no day goes to the inbox. A day without `--time` is all-day. `--time` is `HH:MM`.

Recurring tasks use the recurring id plus `--day YYYY-MM-DD`. `delete` of a recurring task removes that day only. Deleting a calendar-imported task hides it in Structured. It does not remove the event from the external calendar.

Shell completions:

```bash
structured completions bash
structured completions zsh
structured completions fish
```

## TUI

```text
j/k        move
gg / G     top / bottom
ctrl-d/u   page
h/l, tab   Today and Inbox
[ ]        previous / next day
x, space   tick or untick
a          add
e          edit the title
d, then y  delete
r          refresh
?          help
q          quit
```

The list refreshes about every 20 seconds.

## Development

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Rust 1.85 or newer. The lockfile is committed.

## License

MIT. See [LICENSE](LICENSE).
