# Edit card

`e` turns the details pane into an editable card. The title, each subtask, and the note are typed in place, and each field writes to `update_task` when the cursor leaves it.

## Sub-features

- `edit-open` opens the card with `e` from the list. The pane title reads `Details · editing`, the border turns accent, a `›` marks the title, and the footer chip reads `TITLE`.
- `edit-tab` moves between sections with `tab`: title, then the first subtask (or the `add a subtask…` row), then the note, then back to the title. `shift-tab` goes the other way.
- `edit-rows` moves with `↑` `↓` across rows and sections. A long title, subtask, or note wraps while it is edited, and the arrows walk its wrapped lines first.
- `edit-insert` opens a draft row below a subtask with Enter. Enter on the draft adds it there and opens the next one. Enter on an empty draft moves to the note. Backspace on an empty draft goes up.
- `edit-note` types the note in place. Enter is a new line. `home` `end` `ctrl-u` `ctrl-k` stop at the line.
- `edit-save` writes the field on `tab`, an arrow out of it, Enter, or `esc`. An unchanged field writes nothing.
- `edit-refuse` keeps an empty title or subtask open with `✗ a title is required`.
- `edit-undo` puts the field back with `ctrl-c` (`· change undone`). A second `ctrl-c` closes the card (`· closed`).
- `edit-scope` adds `every repeat` to `TITLE` and `NOTE`, and `this day only` to `SUBTASK` and `NEW SUBTASK`, on a recurring task.
- `edit-from-checklist` opens the card on the selected row with `e` in the checklist, or on a new row with `a` there or `A` from the list. `esc` returns to the checklist.

## How to get to it (user POV)

- Select a task and press `e`. Type, `tab` to the next section, `esc` when done.

## Driving it with verify.sh

Preconditions:

- A `--fixture` run with `doctor` reporting `status=ready`. Every step below writes, so a live run only opens and closes the card.
- From the ready Today view, `j` six times selects `Finish the billing export (ENG-3442)`, which has three subtasks with one done and a two-line note.

- **Open.** `keys -- j j j j j j e`, `shot edit-open`. Assert `Details · editing`, `› Finish the billing export`, the `TITLE` chip, and `tab next`.
- **Title.** `type " v2"`, `keys -- Tab`. The footer chip is `SUBTASK` and `calls` ends with an `update_task` carrying only `title`.
- **Insert.** `keys -- Enter`, `type "Check the schema"`, `shot edit-insert`: a highlighted draft row sits under `Map the fields` and the trailing `add a subtask…` row is hidden. `keys -- Enter`: the list reads `1/4`, a new empty draft opens under the added row, and `calls` shows four subtasks with the new one second and without an id.
- **Note.** `keys -- Enter`. The chip is `NOTE`. `keys -- End Enter`, `type "Confirm the totals with finance"`, `keys -- Escape`. The footer is back to the list hints with `✓ saved`, and `calls` ends with an `update_task` whose `note` contains a `\n`.
- **Refuse.** `keys -- e Tab C-u Escape`. The chip stays `SUBTASK`, the row shows a faint `type a title…`, and the footer reads `✗ a title is required`. `keys -- C-c C-c` closes it without a write.
- **Cursor.** Captures do not show the terminal cursor. Read it with `tmux display -p -t sverify-<run-id> '#{cursor_x},#{cursor_y}'` on this run's own session.

## Gotchas

- Inside the card every letter types, including `q`, `?`, `x`, and `d`. Leave with `esc` or `ctrl-c` first.
- `verify.sh keys` has no Shift-Tab. Use `↑` from the first subtask to reach the title.
- `verify.sh type` cannot send text that starts with `-`; tmux reads it as a flag.
- On a recurring day a subtask write splits the day off its series and holds the next write until the reload lands. The draft stays open with `● saving the last change to this day…`; press the key again after it clears.
