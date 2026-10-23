# Schedule

Schedule edits the selected task's date, start time, or duration. Opening a prompt is local. Enter sends the change to Structured. Escape drops the draft.

## Sub-features

- `sched-date` opens the date draft with `D`.
- `sched-time` opens the time draft with `T`.
- `sched-duration` opens the duration draft with `m`.
- `sched-step` changes the draft with Up and Down and does not save.
- `sched-cancel` closes the draft with Escape and leaves the task unchanged.
- `sched-save` saves with Enter. This writes.

## How to get to it (user POV)

- Select a task.
- Press `D` for the date, `T` for the time, or `m` for the duration.
- Press Up or Down to step the draft, Enter to save, Escape to cancel.

## Driving it with verify.sh

Preconditions:

- `verify.sh doctor` reports `status=ready` and the list has at least one task.
- Find is closed.
- For `sched-save` only: the user authorized a write on a named task and `STRUCTURED_VERIFY_WRITES=1` is set. Otherwise stop and report that precondition. Do not press Enter.

- **Duration.** `keys -- m`, `capture duration-open`. The footer has a `DURATION` chip, the current minutes, and `↑↓ step`.
- **Step.** `keys -- Up`, `capture duration-step`. The minutes are 15 higher, or `15` when the draft was empty. No `saving`.
- **Cancel.** `keys -- Escape`, `capture duration-cancel`. The chip is gone and the footer reads `cancelled`. No `saving`.
- **Date.** `keys -- D`, `capture date-open`. The chip is `DATE` and the hint lists `today, +1, inbox`. Escape.
- **Time.** `keys -- T`, `capture time-open`. The chip is `TIME`. Escape.
- **Bad input.** `keys -- m C-u`, `type "9999"`, `keys -- Enter`. The `DURATION` prompt stays open with `9999` and the footer shows `✗ duration must be under 24h`. Nothing is sent. Escape.
- **Scope.** Select a recurring row (`↻`), `keys -- T`: the chip reads `TIME` followed by `every repeat` in yellow. `keys -- Escape D`: `DATE` with `this day only`. Escape.
- **Save.** In a fixture run, or live with the write precondition: `keys -- m C-u`, `type "50"`, `keys -- Enter`, `shot duration-save`. The row shows `50m` and the status goes `● saving…` then `✓ saved`. `calls` shows `update_task` with `duration: 50`. A time change re-sorts the timeline and keeps the task selected.

## Gotchas

- Enter on `D`, `T`, or `m` calls `update_task`. Escape does not.
- Empty date reports `date required` and does not save. Empty time means all day. Empty duration means 0 minutes. Durations above 1439 are refused before sending.
- The prompt has a real cursor. `←` `→`, `home`, `end`, `ctrl-w`, and `ctrl-u` edit the draft. `ctrl-c` cancels.
- Up and Down step the date by one day, the time by 15 minutes, and the duration by 15 minutes.
- `,` and `.` save a one-day move immediately. They are not a prompt. Do not send them in a read-only run.
- On a recurring day, a date move (`D`, `,`, `.`) is `only_this` with `new_day`. A time or duration edit is `all` and changes the series. A live proof still needs write authorization.
