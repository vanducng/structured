# structured

Unofficial command-line and terminal interface for the [Structured](https://structured.app) day planner.

This project is not affiliated with Structured, unorderly, or Leo Mehlig.

`structured` is a CLI for scripts and agents, and a vim-style TUI for a person. Both talk to Structured Cloud. A terminal with no subcommand opens the TUI. A pipe with no subcommand prints help and exits 2.

[![structured demo: text timeline, refs, subtasks, recurring scope, dry run, and the TUI](https://asciinema.org/a/x3Az7oaRX9Bm2jen.svg)](https://asciinema.org/a/x3Az7oaRX9Bm2jen)

The demo runs on generic sample tasks: the text timeline, refs and subtasks from the CLI, the scope check on a recurring day, `--dry-run`, then the TUI checklist, search, scope-aware prompts, help, inbox, and the next day.

## Install

Download the archive for your machine from the [latest release](https://github.com/vanducng/structured/releases/latest), check it against `SHA256SUMS`, and put `structured` on `PATH`. Archives exist for `aarch64-apple-darwin` and `x86_64-unknown-linux-gnu`. Upgrading is the same steps.

```bash
tag=$(gh release view --repo vanducng/structured --json tagName -q .tagName)
target=aarch64-apple-darwin
gh release download "$tag" --repo vanducng/structured -p "structured-$tag-$target.tar.gz" -p SHA256SUMS
shasum -a 256 -c SHA256SUMS --ignore-missing
tar -xzf "structured-$tag-$target.tar.gz"
install -m 755 "structured-$tag-$target/structured" ~/.local/bin/structured
structured --version
```

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

`--help` on any command is the contract. Output is one JSON object on stdout when `--json` is set or stdout is not a terminal. `--text` forces the readable view even in a pipe, which is how an agent shows a plan to a person. `--dry-run` prints the exact request a write would send and sends nothing. Errors, argument errors included, go to stderr as `{ "ok": false, "error": { "code", "message", "retryable", "next_steps" } }`. Usage errors exit 2. Other errors exit 1.

Every write prints the same result: `{ "ok": true, "result": { "operation", "scope", "ref", "task" } }`. `scope` is `task`, `day`, or `series`.

```bash
structured today                      # readable timeline in a terminal
structured today --json               # one JSON object
structured day tomorrow               # YYYY-MM-DD, today, tomorrow, yesterday, +N, -N
structured inbox
structured list --from 2026-10-01 --to +7 --open --search review --limit 50
structured add "Write the note" --day +1 --time 09:30 --duration 30 --subtask Outline --subtask Draft
structured add "Call the bank" --time 16:00   # a time alone means today
structured add "Someday"                      # no day, time, or --all-day goes to the inbox
structured done REF
structured undo REF
structured edit REF --title "New title" --time 10:00
structured edit RECURRING_ID@2026-10-23 --move-day tomorrow --scope day
structured delete REF --yes --dry-run
structured delete REF --yes
structured subtask list REF
structured subtask add REF "Draft" "Review"
structured subtask done REF 2         # SUB is a 1-based number or a subtask id
structured subtask edit REF 2 "Review with Sam"
structured subtask delete REF 2 --yes
structured account
structured login | logout | tui | completions zsh
```

### Refs

Every task in `--json` output carries a `ref`. Pass it back to any command that takes `REF`.

| Task | `ref` | Example |
| --- | --- | --- |
| One-off task | its id | `7c0e…-91d2` |
| One day of a recurring task | `RECURRING_ID@YYYY-MM-DD` | `2304…0305@2026-10-23` |

The older form, a recurring id plus `--day YYYY-MM-DD`, still works.

On a recurring day, `done`, `undo`, `delete`, and subtask ticks change that day only. `edit` must say how far it reaches: `--scope day` splits that day off the series, `--scope series` changes every repeat. Moves (`--move-day`, `--inbox`) only work with `--scope day`. Adding, renaming, or removing a subtask changes that day only, so one day's ticks never leak into the series. Deleting a calendar-imported task hides it in Structured. It does not remove the event from the external calendar.

### Readable output

`structured today --text` prints the day the way the TUI shows it:

```text
Fri 23 Oct 2026 (today) · 11 tasks, 3 done · 9h planned

  All day
                      [x] Go for a run  repeats
                      [ ] Read 20 pages  repeats
  08:30-09:15    45m  [ ] Team sync
  09:00-09:30    30m  [ ] Review the API pull request  ! overlaps
                         · 1h 45m free
  11:15-12:15     1h  [x] Ship the release notes
                         · 45m free
  13:00-16:30 3h 30m  [ ] Finish the billing export (ENG-3442)  (1/3)
                         [x] Map the fields
                         [ ] Write the backfill
                         [ ] Dry run on staging
  16:45-17:30    45m  [ ] Read the hiring brief
                      --- now 17:02 ---
  17:45-19:00 1h 15m  [ ] Triage open tickets  (2/2)
```

## TUI

`structured` with no command in a terminal opens it. So does `structured tui`.

```text
 Friday 23 October 2026  Today         Mon 19 … [Fri 23] Sat 24 Sun 25  │  Inbox 5
╭ Today · 11 tasks ───── 3 done · 9h planned ╮╭ Details ─────────────────────────╮
│       ALL DAY                              ││ ██ Finish the billing export ███ │
│       ▍ ○ Read 20 pages                 ↻  ││                                  │
│ 08:30 ▍ ○ Team sync                   45m  ││ Date      Fri 23 Oct 2026 · today│
│ 09:00 ▍ ○ Review the pull…  overlap   30m  ││ Time      13:00-16:30 · 3h 30m   │
│       ┆  3h 30m free                       ││ SUBTASKS  1/3                    │
│ 13:00 ▍ ○ Finish the billi…  1/3   3h 30m  ││   ☑ Map the fields               │
│ 23:44 ─────────────────────────────── now  ││ › ☐ Write the backfill           │
╰────────────────────────────────────────────╯╰──────────────────────────────────╯
 SUBTASKS ✓ subtasks saved               x tick  a add  e rename  d delete  ? help
```

The list is a timeline: all-day tasks first, then by start time, with free gaps of 30 minutes or more, overlaps, and a red now-line on today. The inbox lists open tasks first and done tasks under a `Done` heading. The right pane shows the selected task, its subtasks, and its note.

| Keys | Action |
| --- | --- |
| `j` `k` `↑` `↓`, `gg` `G`, `ctrl-d` `ctrl-u` | move |
| `t`, `i`, `tab`, `[` `]` `←` `→` | today, inbox, flip the two, day before or after |
| `/` or `f` | find in titles, notes, and subtasks. Matches are underlined. `n` `N` next and previous match, `esc` clears and keeps the highlighted task |
| `x` or space | done or not |
| `a`, `e`, `d` then `y` | add, edit the title, delete |
| `D`, `T`, `m`, `,` `.` | date, time, minutes, move one day |
| `enter` or `l` | open the subtasks. Then `j` `k` move, `x` ticks, `a` adds (typed in place, `⏎` keeps adding, `esc` stops), `e` renames, `d` deletes, `J` `K` reorder, `h` or `esc` goes back |
| `A` | add a subtask straight from the list |
| `ctrl-g` | edit the note in `$VISUAL` or `$EDITOR` (falls back to `vi`) |
| `ctrl-e` `ctrl-y` | scroll the details |
| `r`, `?`, `q` or `ctrl-c` | refresh, help, quit. Quit waits for pending saves; press `q` again to leave at once |

Prompts are one line at the bottom with a real cursor: `←` `→`, `home` `end`, `ctrl-w`, and `ctrl-u` edit. `↑` `↓` step a date by a day and a time or duration by 15 minutes. A rejected value keeps the draft and shows why. `esc` or `ctrl-c` cancels. On a recurring task the prompt says how far the edit reaches: a title, time, or duration changes `every repeat`; a date, a move, or a subtask change applies to `this day only`.

Every write paints first and saves in the background. A failed save puts the old value back and shows the error in red. Today and Inbox stay cached, so switching is instant. The view refreshes about every 20 seconds and on `r`.

`~/.config/structured/config.toml` (or `$XDG_CONFIG_HOME/structured/config.toml`):

```toml
layout = "side"
detail_percent = 46
```

`layout` is `side` or `stack`. `detail_percent` is clamped to 30..=70. Side by side needs at least 72 columns.

## Development

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Rust 1.88 or newer. The lockfile is committed.

Releases are automatic. Conventional commits on `main` feed release-please, which keeps a release PR open with the next version and its `CHANGELOG.md` entry: `fix` bumps the patch, `feat` and breaking changes bump the minor while the version is below 1.0. Merging that PR updates `Cargo.toml`, tags `vX.Y.Z`, and creates the GitHub release; the same workflow run builds the archives and `SHA256SUMS` and attaches them. A manual run of the `release` workflow with a tag rebuilds and re-uploads that release's assets.

`src/lib.rs` dispatches the CLI and `src/subtask.rs` runs the checklist commands. `src/agenda.rs` orders a day and finds gaps and overlaps for both the TUI and `--text`. `src/tui/` is the terminal UI: `app` holds state, keys, and the cache, `worker` talks to Structured off the UI thread, and `view` draws with ratatui. Prompts use `tui-input`.

`STRUCTURED_MCP_URL` points the binary at another MCP endpoint. The CLI tests use it with an in-process mock, and the TUI check in `.cursor/skills/verify-structured` uses it with a local fixture server so writes never touch a real account.

## License

MIT. See [LICENSE](LICENSE).
