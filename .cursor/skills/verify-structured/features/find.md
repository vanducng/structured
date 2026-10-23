# Find

Find narrows the visible list by title or note and moves the highlight between matches. It does not change tasks.

## Sub-features

- `find-slash` opens find with `/`. The footer shows a `FIND` chip and `/` with the query.
- `find-type` appends letters, including `f`, and narrows the visible count.
- `find-arrows` move the highlight while the prompt is open.
- `find-empty` shows a zero count for a query that matches nothing.
- `find-escape` clears the query and restores the count.
- `find-f` opens the same find with `f`. Every letter types, `f` included.
- `find-enter` closes the prompt and keeps the query in the list title. Matches are underlined in titles, subtasks, and notes.
- `find-next` steps through kept matches with `n` and `N` (and `f`).

## How to get to it (user POV)

- Press `/` on the list.
- Press `f` on the list when no query is active.
- Press `n` or `N` while a query is kept to move between matches. `esc` clears it.

## Driving it with verify.sh

Preconditions:

- `verify.sh doctor` reports `status=ready` on Today.
- Note the visible count in the hint before typing.

- **Open.** `keys -- /`, `capture find-open`. The footer has `FIND` and `↑↓ move`.
- **Miss.** `keys -- z z q q`, `capture find-miss`. The footer shows `/zzqq`, the list title reads `0 tasks · /zzqq`, and the list says `No tasks match “zzqq”`.
- **Arrows.** `keys -- Down Up`. The footer still shows `/zzqq` and the count stays `0`.
- **Clear.** `keys -- Escape`, `capture find-cleared`. The `FIND` chip is gone and the count matches the count from before find.
- **`f` types.** `keys -- f`, `type "fields"`, `capture find-f`. The footer shows `/fields` and the `FIND` chip stays. Escape.
- **Keep.** `keys -- / z z q q Enter`, `capture find-kept`. The `FIND` chip is gone and the list title still has `/zzqq`. In the list, Escape clears it so the next recipe starts unfiltered.
- **Subtask match.** In a fixture run, `keys -- / b a c k f i l l Enter`. The list shows `1 task` and it is the billing export, found through its subtask.

## Gotchas

- `q` inside find is a letter. It does not quit.
- `j` and `k` inside find are letters. Arrows move. Left and Right also move while find is open. Outside find, Left and Right change the day.
- Inside find every printable key is a letter. Outside it, with a kept query, `n` `N` and `f` move between matches.
- Typing, Backspace, and Escape keep the highlighted task when it still matches; otherwise the highlight goes to the first match.
- Escape clears the query. Enter keeps it. Escape on the list clears a kept query.
- `ctrl-u` and `ctrl-w` clear the query. Other control keys never type.
