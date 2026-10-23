# TUI

`structured` with no command in a terminal opens the task manager. So does `structured tui`.

```text
 Sat 24 Oct  Now    Mon 19  Tue 20  Wed 21  Thu 22  Fri 23  Sat 24  Sun 25   │  Inbox 5  │ 02:43 Saigon

╭ Now · 10 tasks ──── 2 late · 4 done · 7h 45m planned ╮╭ Details ─────────────────────────────────────╮
│       YESTERDAY · FRI 23 OCT                         ││                                              │
│       ▍ ○ Send the weekly update                late ││  Send the weekly update                      │
│ 20:30 ▍ ✓ Standup with the US team               30m ││                                              │
│ 21:15 ▍ ○ Pair on the export job        late  1h 30m ││                                              │
│       TODAY · SAT 24 OCT                             ││  Date      Fri 23 Oct 2026 · yesterday       │
│       ▍ ✓ Go for a run                             ↻ ││  Time      All day                           │
│       ▍ ○ Read 20 pages                            ↻ ││  Status    Late, still open                  │
│ 02:43 ────────────────────────────────────────── now ││                                              │
│ 08:00 ▍ ✓ Rise and shine                           ↻ ││  SUBTASKS                                    │
│       ┆   30m free                                   ││  No subtasks · A adds one                    │
│ 08:30 ▍ ○ Team sync                              45m ││                                              │
╰──────────────────────────────────────────────────────╯╰────────────────────────────────── ↓ more · J ╯
 · times in Saigon   ⏎ subtasks  x done  a add  f find  e edit  D date  T time  m min  [ ] day  ? help
```

The frame shows the 24h view in Saigon time. The left pane lists tasks. The right pane shows the selected task, its subtasks, and its note.

## Views

Today has two views. `w` switches between them, and `view` in [config.toml](configuration.md) picks the one the TUI opens on.

| View | Shows | Pick it when |
| --- | --- | --- |
| Day (default) | The calendar day, all-day tasks first, then by start time | Your work fits inside one date |
| 24h (`view = "now"`) | Everything from 12 hours before now to 12 hours after, under one heading per day | Your working hours cross midnight, or you want yesterday's unfinished tasks in sight |

In the 24h view yesterday's open work stays listed and is marked `late` in yellow. A session that starts in the evening and runs past midnight, like US hours seen from Vietnam, stays in one list.

Both views mark free gaps of 30 minutes or more, overlaps, and a red now-line. `[` `]` step to other days, which always show as calendar days. `i` or `tab` opens the inbox: open tasks first, done tasks under a `Done` heading.

## Time zones

With `timezones` set in config.toml, the header ends with the clock in the zone on screen, like `02:39 Saigon`. `z` switches to the next zone in the list and reloads, so every task moves to the day and time it falls on there. Tasks without a zone of their own, like most recurring routines, keep their clock time. Edits made with `T`, `D`, `,` and `.` are read in the zone on screen. `z` waits while a save is still running.

## Keys

`?` shows them all.

| Keys | Action |
| --- | --- |
| `j` `k` `↑` `↓`, `gg` `G`, `ctrl-d` `ctrl-u` | move |
| `t`, `i`, `tab` | today, inbox, flip between the two |
| `[` `]` `←` `→` | day before or after |
| `w` | day or 24h view |
| `z` | next time zone |
| `/` or `f` | find in titles, notes, and subtasks. `n` `N` step through matches, `esc` clears and keeps the highlighted task |
| `x` or space | done or not |
| `a`, `e`, `d` then `y` | add, edit the title, delete |
| `D`, `T`, `m`, `,` `.` | date, time, minutes, move one day |
| `enter` or `l` | open the subtasks. Then `j` `k` move, `x` ticks, `a` adds, `e` renames, `d` deletes, `J` `K` reorder, `h` or `esc` goes back |
| `A` | add a subtask straight from the list |
| `ctrl-g` | edit the note in `$VISUAL` or `$EDITOR` (falls back to `vi`) |
| `J` `K` (or `ctrl-e` `ctrl-y`) | scroll the details of the selected task |
| `r`, `?`, `q` or `ctrl-c` | refresh, help, quit |

## Prompts

Prompts are one line at the bottom with a real cursor. `←` `→`, `home` `end`, `ctrl-w`, and `ctrl-u` edit. `↑` `↓` step a date by a day and a time or duration by 15 minutes. A rejected value keeps the draft and says why. `esc` or `ctrl-c` cancels.

A new subtask is typed in place in the checklist. `⏎` saves it and opens the next one, `esc` stops.

On a recurring task the prompt says how far the edit reaches. A title, time, or duration changes `every repeat`. A date, a move, or a subtask change applies to `this day only`.

## Saving

Every write paints first and saves in the background. A failed save puts the old value back and shows the error in red. `q` waits for pending saves; press `q` again to leave at once.

Today and the inbox stay cached, so switching between them is instant. The view refreshes about every 20 seconds and on `r`.
