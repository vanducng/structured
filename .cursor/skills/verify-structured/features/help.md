# Help

Help opens the key list in a centered modal over the screen. It does not change the list or any task.

## Sub-features

- `help-open` shows the `Keys` modal and a `HELP` chip in the footer.
- `help-close` closes it on the next key.

## How to get to it (user POV)

- Press `?` on the list or in the checklist.

## Driving it with verify.sh

Preconditions:

- `verify.sh doctor` reports `status=ready`.
- Find is closed and no prompt is open.

- **Open.** `keys -- '?'`, then `shot help-open`. The modal title is `Keys`, it lists `next, previous`, `minutes`, and `add from list`, its last line, `Saves run in the background`, is fully visible, and everything behind it is dimmed except the footer. At 80x24 the modal still fits between the header and the footer.
- **Close.** `keys -- '?'`, then `capture help-closed`. `Keys` is gone and the footer shows `? help`.

## Gotchas

- Any key closes help, including a second `?`. It does not reopen.
- `q` closes help. It does not quit.
- `ctrl-c` quits even while help is open.
