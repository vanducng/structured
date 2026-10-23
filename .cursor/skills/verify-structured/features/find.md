# Find

Find narrows the visible list by title or note and moves the highlight between matches. It does not change tasks.

## Sub-features

- `find-slash` opens find with `/` and shows the query in the hint.
- `find-type` appends letters, including `f`, and narrows the visible count.
- `find-arrows` move the highlight while the prompt is open.
- `find-empty` shows a zero count for a query that matches nothing.
- `find-escape` clears the query and restores the count.
- `find-f` opens find with `f`. A second `f` on an empty query closes it. A second `f` on a typed query jumps to the next match.
- `find-enter` closes the prompt and keeps the query on the hint.

## How to get to it (user POV)

- Press `/` on the list.
- Press `f` on the list when no query is active.
- Press `f` again while a query is already applied to jump to the next match.

## Driving it with verify.sh

Preconditions:

- `verify.sh doctor` reports `status=ready` on Today.
- Note the visible count in the hint before typing.

- **Open.** Press `/`. Run `verify.sh keys "$RUN_ID" -- /` and `verify.sh capture "$RUN_ID" find-open`. The hint contains `/_`.
- **Miss.** Type `zzqq`. Run `verify.sh keys "$RUN_ID" -- z z q q` and capture `find-miss`. The hint contains `/zzqq_` and the visible count is `0`.
- **Arrows.** Press Down, then Up. Run `verify.sh keys "$RUN_ID" -- Down Up`. The hint still contains `/zzqq_` and the count stays `0`.
- **Clear.** Press Escape. Run `verify.sh keys "$RUN_ID" -- Escape` and capture `find-cleared`. The hint has no `/zzqq` and the count matches the count from before find.
- **`f` toggle.** Press `f` twice. Run `verify.sh keys "$RUN_ID" -- f f` and capture `find-f`. The first `f` would show `/_`; after the second `f` the hint has no `/_`.
- **Keep.** Open with `/`, type `zzqq`, press Enter. Run `verify.sh keys "$RUN_ID" -- / z z q q Enter` and capture `find-kept`. The hint contains `/zzqq` and does not contain `/zzqq_`. Press Escape afterward so the next recipe starts unfiltered.

## Gotchas

- `q` inside find is a letter. It does not quit.
- `j` and `k` inside find are letters. Arrows move. Left and Right also move while find is open. Outside find, Left and Right change the day.
- `f` inside a find opened with `/` is a letter. `f` inside a find opened with `f` is the repeat key: close when the query is empty, next match when it is not.
- Typing or Backspace resets the highlight to the first match.
- Escape clears the query. Enter keeps it.
