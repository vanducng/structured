# Navigate

Navigate changes which task is selected and which day or inbox is on screen. Switching view loads from Structured and does not edit a task.

## Sub-features

- `nav-move` moves the highlight with `j`, `k`, `gg`, and `G`.
- `nav-today` shows Today with `t`.
- `nav-inbox` shows Inbox with `i`.
- `nav-day` moves the viewed day with `[` and `]`.
- `nav-switch` flips between the current day and Inbox with Tab.
- `nav-week` highlights the viewed day in the header's week strip.

## How to get to it (user POV)

- Press `j` or `k` on the list.
- Press `g` twice for the top, or `G` for the bottom.
- Press `t` for Today, `i` for Inbox, `[` or `]` for another day, or Tab to flip.

## Driving it with verify.sh

Preconditions:

- `verify.sh doctor` reports `status=ready`.
- Find is closed. If the hint contains `/`, press Escape first.

- **Down.** `keys -- j`, `shot nav-down`. The highlighted row moves down one task and skips section, gap, and now rows. The details band shows the new task.
- **Top.** `keys -- G g g`, `shot nav-top`. The first task row is highlighted.
- **Inbox.** `keys -- i`, `capture nav-inbox`. The header starts with `Inbox` and `N open`, the `Inbox N` chip is highlighted, and the list title reads `Inbox · N tasks`. Press `t` afterward.
- **Today.** `keys -- t`, `capture nav-today`. The header ends the date with `Today` and the list title starts with `Today`.
- **Next day.** `keys -- ]`, `shot nav-next-day`. The header reads `Tomorrow`, the week strip highlights the next day, and there is no now-line. Press `[` to return.

## Gotchas

- The first visit to a day or Inbox that is not cached shows `loading…` in the list title and `Loading…` in the list. A live inbox can take a couple of seconds. Wait for `N tasks` before asserting the list.
- `t` on Today and `i` on Inbox do not reload by themselves. `r` does.
- Left and Right change the day when find is closed. They move the selection when find is open.
- `gg` is two `g` keys. A single `g` does nothing visible.
- `h` and `l` move focus between the list and the checklist. They no longer flip Inbox; Tab does.
