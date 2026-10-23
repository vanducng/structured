# Help

Help replaces the detail pane with the key list. It does not change the list or any task.

## Sub-features

- `help-open` shows the key list in the detail pane.
- `help-close` returns to the selected task on the next key.

## How to get to it (user POV)

- Press `?` on the list.

## Driving it with verify.sh

Preconditions:

- `verify.sh doctor` reports `status=ready`.
- Find is closed and no prompt is open.

- **Open.** Press `?`. Run `verify.sh keys "$RUN_ID" -- '?'` and capture `help-open`. The detail pane contains `f again next match` and `m duration`.
- **Close.** Press `?` again. Run `verify.sh keys "$RUN_ID" -- '?'` and capture `help-closed`. Those help lines are gone and the detail pane shows the selected task's open or done state.

## Gotchas

- Any key closes help, including a second `?`. The second `?` does not reopen it.
- Help does not hide the list. A capture still contains task titles. Do not use a title as the oracle.
