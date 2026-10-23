# CLI

Reference for the `structured` commands. `structured --help` and `structured <command> --help` are the contract; this page explains how the pieces fit.

## Output

| Flag | Effect |
| --- | --- |
| `--json` | One JSON object on stdout. This is the default when stdout is not a terminal. |
| `--text` | The readable view, even in a pipe. This is how an agent shows a plan to a person. |
| `--dry-run` | Print the request a write would send, and send nothing. Reads still run. |
| `--tz ZONE` | Read and write days and times in this zone. See [Configuration](configuration.md#time-zones). |

Reads print `{"ok": true, ...}` with a `tasks` array. Every write prints the same envelope:

```json
{"ok": true, "result": {"operation": "updated", "scope": "day", "ref": "RECURRING_ID@2026-10-23", "task": {}}}
```

`scope` is `task`, `day`, or `series`.

Errors go to stderr, argument errors included:

```json
{"ok": false, "error": {"code": "USAGE", "message": "...", "retryable": false, "next_steps": ["structured edit --help"]}}
```

| Code | Meaning | Exit |
| --- | --- | --- |
| `USAGE` | A bad argument, or a write that needs `--yes` or `--scope` | 2 |
| `AUTH` | Not signed in, or the sign-in expired. Run `structured login` in a terminal | 1 |
| `NOT_FOUND` | No task with that ref | 1 |
| `NETWORK` | Structured Cloud did not answer. `retryable` is true | 1 |
| `API`, `DATA`, `IO` | The server refused, sent something unreadable, or a local file failed | 1 |

A pipe with no subcommand prints help to stderr and exits 2.

## Commands

```bash
structured now                        # 12 hours either side of now, across midnight
structured today                      # the calendar day
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
structured update                     # the latest release, in place
structured login | logout | tui | completions zsh
```

`list` returns at most `--limit` tasks (200 by default). JSON says `"truncated": true` when it hit the limit.

In JSON, `late` is true for an open task whose end has passed, or an open all-day task on an earlier day. `start_time` is hours from midnight in the zone in use: `8.5` is 08:30.

## Refs

Every task in `--json` output carries a `ref`. Pass it back to any command that takes `REF`.

| Task | `ref` | Example |
| --- | --- | --- |
| One-off task | its id | `7c0e…-91d2` |
| One day of a recurring task | `RECURRING_ID@YYYY-MM-DD` | `2304…0305@2026-10-23` |

A recurring id plus `--day YYYY-MM-DD` also works. Passing the plain id of a recurring day fails with `NOT_FOUND`, and the message names the right ref.

## Recurring days

On a recurring day, `done`, `undo`, `delete`, and subtask ticks change that day only.

`edit` must say how far it reaches:

- `--scope day` splits that day off the series and changes only it.
- `--scope series` changes every repeat.

Moves (`--move-day`, `--inbox`) only work with `--scope day`. Adding, renaming, or removing a subtask changes that day only, so one day's checklist never leaks into the series.

Deleting a calendar-imported task hides it in Structured. The event stays in the external calendar.

## Time zones

Days and times in the output are in the zone in use, and `start_time` is always within 0 to 24. Structured Cloud keeps each task filed under one day in every zone, so a task filed on Saturday at 08:30 Saigon time shows in ET as Friday 21:30. Its `ref` keeps the filed day. Reads ask one day wider on each side so such tasks land on the right day.

`--time 10:00` means 10:00 in the zone in use, and `--move-day` keeps the time of day as shown. A task with no zone of its own, like most recurring routines, shows the same clock time in every zone. Edits never change a recurring task's zone, or its lack of one, and a title or note edit never touches any task's zone.

```bash
structured now --tz saigon --text            # a label from config.toml
structured today --tz Europe/Paris --json    # or any IANA name
```

## Readable output

`structured today --text` prints the day the way the TUI shows it:

```text
Fri 23 Oct 2026 (today) · 11 tasks, 3 done, 3 late · 9h planned

  All day
                      [x] Go for a run  repeats
                      [ ] Read 20 pages  repeats
  08:00               [x] Rise and shine  repeats
                         · 30m free
  08:30-09:15    45m  [ ] Team sync  ! late
  09:00-09:30    30m  [ ] Review the API pull request  ! late
                         · 1h 45m free
  11:15-12:15     1h  [x] Ship the release notes
                         · 45m free
  13:00-16:30 3h 30m  [ ] Finish the billing export (ENG-3442)  (1/3)  ! late
                         [x] Map the fields
                         [ ] Write the backfill
                         [ ] Dry run on staging
                      --- now 16:42 ---
  16:45-17:30    45m  [ ] Read the hiring brief
  17:45-19:00 1h 15m  [ ] Triage open tickets  (2/2)
```

`structured now --text` prints the same rows under one heading per day, starting with yesterday's unfinished work:

```text
Now · 16:42 · 12h either side · 12 tasks, 3 done, 4 late

  Yesterday · Thu 22 Oct
                      [ ] Send the weekly update  ! late
  Today · Fri 23 Oct
                      [x] Go for a run  repeats
```

Free gaps of 30 minutes or more get a `free` row. `! overlaps` marks a task that starts before the previous one ends.
