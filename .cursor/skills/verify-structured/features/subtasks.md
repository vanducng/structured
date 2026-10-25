# Subtasks

The checklist in the details pane. Enter moves focus into it, and its keys tick, add, rename, and delete subtasks. Every change sends the whole list to `update_task`.

## Sub-features

- `sub-open` focuses the checklist with `enter` or `l`. The footer shows a `SUBTASKS` chip.
- `sub-move` moves the `›` marker with `j`, `k`, and the arrows.
- `sub-tick` flips `☐` and `☑` with `x` or Space. The list row's `n/m` count follows. A row still being created cannot be ticked yet.
- `sub-add` opens the [edit card](./edit.md) on a new row (`NEW SUBTASK`) with `a`, or `A` from the list. Enter saves it and opens the next row. Escape saves what is typed and closes.
- `sub-rename` opens the edit card on the selected row (`SUBTASK`) with `e`. Escape or Tab saves the new title.
- `sub-reorder` moves the row with `J` and `K`.
- `sub-delete` asks with `d` (`Delete subtask`) and removes on `y`.
- `sub-back` returns to the list with `h`, `esc`, or `←`.
- `sub-empty` shows `No subtasks yet · a adds one` on a task without subtasks.

## How to get to it (user POV)

- Select a task in the list and press `enter`.
- Use the checklist keys, then `h` to go back.

## Driving it with verify.sh

Preconditions:

- A `--fixture` run with `doctor` reporting `status=ready`. A live run only for `sub-open`, `sub-move`, and `sub-back` unless the user authorized writes.
- From the ready Today view, `j` six times selects `Finish the billing export`, which has three subtasks with one done.

- **Open.** `keys -- j j j j j j Enter`, `shot sub-open`. The footer has `SUBTASKS` and `x tick`. The details pane shows `SUBTASKS  1/3` in the accent color and `›` on the first row.
- **Tick.** `keys -- j x`, wait a beat, `shot sub-tick`. The second row is `☑` and struck, the list row reads `2/3`, the status reads `✓ subtasks saved`. `calls` shows one `update_task` with three subtasks, all with ids, and no `update_action`.
- **Add.** `keys -- a`, `type "Write the runbook"`, `shot sub-add-draft`: the draft is a highlighted row inside the checklist. `keys -- Enter`, `shot sub-add`: the row is saved and an empty draft row with a faint `add a subtask…` opens below it. `keys -- Escape`: the empty draft closes without a write and the status reads `✓ subtasks saved`. `calls` shows a subtask with only a title.
- **Reorder.** `keys -- K`. The row moves up and `calls` shows the reordered list.
- **Rename.** `keys -- e C-u`, `type "Runbook"`, `keys -- Escape`. The row reads `Runbook`.
- **From the list.** `keys -- h A`, `type "Ship it"`, `keys -- Enter Escape`. A row is added without opening the checklist first.
- **Delete.** `keys -- d`, `shot sub-delete-ask` shows `Delete subtask` and `y delete`. `keys -- y` removes the row and `calls` shows three subtasks.
- **Back.** `keys -- h`. The chip is gone and the footer shows `⏎ subtasks`.

## Gotchas

- `x` in the checklist ticks a subtask, not the task. `h` first to tick the task.
- A failed save puts the old checklist back and shows `✗` with the server message.
- Ticks on a recurring day are free and stay on that day. Add, rename, and delete detach that day (`only_this`, Pro plan) and the view reloads.
- Find also matches subtask titles, so `/` then a subtask word selects its task.
