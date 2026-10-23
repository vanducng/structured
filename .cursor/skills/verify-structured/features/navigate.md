# Navigate

Navigate changes which task is selected and which day or inbox is on screen. Switching view loads from Structured and does not edit a task.

## Sub-features

- `nav-move` moves the highlight with `j`, `k`, `gg`, and `G`.
- `nav-today` shows Today with `t`.
- `nav-inbox` shows Inbox with `i`.
- `nav-day` moves the viewed day with `[` and `]`.
- `nav-switch` flips between the current day and Inbox with Tab.

## How to get to it (user POV)

- Press `j` or `k` on the list.
- Press `g` twice for the top, or `G` for the bottom.
- Press `t` for Today, `i` for Inbox, `[` or `]` for another day, or Tab to flip.

## Driving it with verify.sh

Preconditions:

- `verify.sh doctor` reports `status=ready`.
- Find is closed. If the hint contains `/`, press Escape first.

- **Down.** Press `j`. Run `verify.sh keys "$RUN_ID" -- j` and capture `nav-down`. The status still ends in `tasks`. With more than one task, the reversed row is not the first row. With one task, the highlight stays put.
- **Top.** Press `G`, then `g` `g`. Run `verify.sh keys "$RUN_ID" -- G g g` and capture `nav-top`. The first task row is reversed.
- **Inbox.** Press `i`. Run `verify.sh keys "$RUN_ID" -- i` and capture `nav-inbox`. The first line is `Inbox`. The status ends in `tasks` or reads `loading` and then `tasks`. Press `t` afterward.
- **Today.** Press `t`. Run `verify.sh keys "$RUN_ID" -- t` and capture `nav-today`. The first line starts with `Today`.
- **Next day.** Press `]`. Run `verify.sh keys "$RUN_ID" -- ]` and capture `nav-next-day`. The first line is not `Inbox`. Press `[` to return.

## Gotchas

- The first visit to a day or Inbox that is not cached shows `loading` before the count. Wait for `N tasks` before asserting the list.
- `t` on Today and `i` on Inbox do not reload by themselves. `r` does.
- Left and Right change the day when find is closed. They move the selection when find is open.
- `gg` is two `g` keys. A single `g` does nothing visible.
