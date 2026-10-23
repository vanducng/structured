# Complete and delete

Complete flips the checkbox. Delete asks, then removes the task on `y`. Both write to Structured as soon as the confirming key is pressed.

## Sub-features

- `complete-on` sets the box to `x`.
- `complete-off` clears an `x` back to blank.
- `complete-hold` keeps the local box if a refresh still has the old server state.
- `delete-ask` opens the confirm line.
- `delete-yes` removes the task.
- `delete-no` leaves the task in place.

## How to get to it (user POV)

- Select a task and press `x` or Space to flip the box.
- Press `d`, then `y` to delete, or any other key to keep it.

## Driving it with verify.sh

Preconditions:

- The user authorized a write on a named task and `STRUCTURED_VERIFY_WRITES=1` is set.
- `verify.sh doctor` reports `status=ready`.
- Find is closed and the authorized task is selected.
- Without that authorization, stop. Report the missing precondition. Do not press `x`, Space, `d`, or `y`.

- **Ask.** Press `d`. Run `verify.sh keys "$RUN_ID" -- d` and capture `delete-ask`. The detail pane reads `Delete this task?  y / n`.
- **Decline.** Press `n`. Run `verify.sh keys "$RUN_ID" -- n` and capture `delete-kept`. The confirm line is gone and the task row is still present. This step does not write.
- **Flip.** Press `x`. Run `verify.sh keys "$RUN_ID" -- x` and capture `complete-flip`. The row's box changes between `[x]` and `[ ]`, and the status becomes `saving`.
- **Flip back.** Press `x` again and capture `complete-back`. The box returns. This second press is also a write.
- **Delete.** Press `d`, then `y`. Run `verify.sh keys "$RUN_ID" -- d y` and capture `delete-yes`. The row is gone and the status becomes `saving`. Confirm with a JSON read that the id is absent.

## Gotchas

- `x` and Space write immediately. There is no confirm.
- `d` alone does not write. `y` does. Any other key on the confirm line cancels.
- A later refresh must not put the `x` back while the server still has the old completion state. `cargo test` covers that pin. A screen proof needs a refresh after the write.
- Deleting a recurring task removes that day only. Deleting a calendar-imported task hides it in Structured and leaves the external event.
- Two verification sessions share one cloud account. A write here changes the person's real schedule.
