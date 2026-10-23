# Complete and delete

Complete flips the checkbox. Delete asks, then removes the task on `y`. Both write to Structured as soon as the confirming key is pressed.

## Sub-features

- `complete-on` turns `○` into a green `✓` and strikes the title.
- `complete-off` turns `✓` back into `○`.
- `complete-hold` keeps the local box if a refresh still has the old server state.
- `delete-ask` opens the `Delete task` modal with the title and date.
- `delete-yes` removes the task.
- `delete-no` leaves the task in place.

## How to get to it (user POV)

- Select a task and press `x` or Space to flip the box.
- Press `d`, then `y` to delete, or any other key to keep it.

## Driving it with verify.sh

Preconditions:

- A `--fixture` run, or a live run where the user authorized a write on a named task and `STRUCTURED_VERIFY_WRITES=1` is set.
- `verify.sh doctor` reports `status=ready`.
- Find is closed and the authorized task is selected.
- In a live run without that authorization, stop. Report the missing precondition. Do not press `x`, Space, `d`, or `y`.

- **Ask.** `keys -- d`, `shot delete-ask`. A red `Delete task` modal shows the title, the date, `y delete`, and `any other key keeps it`. A recurring day adds `this day only`.
- **Decline.** `keys -- n`, `capture delete-kept`. The modal is gone, the footer reads `task kept`, and the row is still there. This step does not write.
- **Flip.** `keys -- x`, `capture complete-flip`. The row flips between `○` and `✓`, and the status shows `● saving…` then `✓ done`.
- **Flip back.** Press `x` again and capture `complete-back`. The box returns. This second press is also a write.
- **Delete.** `keys -- d y`, `capture delete-yes`. The row is gone and the status shows `✓ deleted`. In a fixture run `calls` shows `delete_task`; live, confirm with a JSON read that the ref is absent.

## Gotchas

- `x` and Space write immediately. There is no confirm.
- `d` alone does not write. `y` does. Any other key in the modal keeps the task. `d` on an empty list does nothing.
- A later refresh must not put the `x` back while the server still has the old completion state. `cargo test` covers that pin. A screen proof needs a refresh after the write.
- Deleting a recurring task removes that day only. Deleting a calendar-imported task hides it in Structured and leaves the external event.
- Two verification sessions share one cloud account. A write here changes the person's real schedule.
