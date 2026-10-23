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

- **Duration.** Press `m`. Run `verify.sh keys "$RUN_ID" -- m` and capture `duration-open`. The detail pane contains `duration:` and ends the draft with `_`.
- **Step.** Press Up. Run `verify.sh keys "$RUN_ID" -- Up` and capture `duration-step`. The minutes number is 15 higher, or `15` when the draft was empty. The status is not `saving`.
- **Cancel.** Press Escape. Run `verify.sh keys "$RUN_ID" -- Escape` and capture `duration-cancel`. The `duration:` prompt is gone and the status is not `saving`.
- **Date.** Press `D`, capture `date-open`, and expect `date:`. Press Escape.
- **Time.** Press `T`, capture `time-open`, and expect `time:`. Press Escape.
- **Save.** Only with the write precondition. Press `m`, type the authorized minutes, press Enter, and capture `duration-save`. The status becomes `saving` and then a saved line. Read the task back with `structured today --json` or `structured inbox --json` and confirm that duration. This capture is a write proof.

## Gotchas

- Enter on `D`, `T`, or `m` calls `update_task`. Escape does not.
- Empty date reports `date required` and does not save. Empty time means all day. Empty duration means 0 minutes.
- Up and Down step the date by one day, the time by 15 minutes, and the duration by 15 minutes.
- `,` and `.` save a one-day move immediately. They are not a prompt. Do not send them in a read-only run.
- A recurring edit is `only_this`. The proof still needs write authorization.
