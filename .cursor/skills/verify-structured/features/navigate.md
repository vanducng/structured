# Navigate

Navigate changes which task is selected and which day or inbox is on screen. Switching view loads from Structured and does not edit a task.

## Sub-features

- `nav-move` moves the highlight with `j`, `k`, `gg`, and `G`.
- `nav-today` shows today with `t`, in whichever view is on.
- `nav-window` switches today between the day view and the 24h view with `w`. The 24h view has headings `YESTERDAY · …` and `TODAY · …`, and `Now` in the header.
- `nav-zone` switches to the next configured time zone with `z` and reloads.
- `nav-detail-scroll` scrolls the details with `J` and `K`.
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
- **Day view.** `keys -- t`, `capture nav-today`. The header shows `Today` and the list title starts with `Today`.
- **24h view.** `keys -- w`, `capture nav-window`. The header shows `Now` and, when it fits, the window edges. The list title starts with `Now`, and a fixture run shows `YESTERDAY` with `Pair on the export job` marked `late`. `keys -- w` returns to the day view.
- **Zone.** Launch with `--config` pointing at a file with `timezones = ["ET=America/New_York", "Saigon=Asia/Ho_Chi_Minh"]`. The header ends with the clock and `ET`. `keys -- z`, `capture nav-zone`. The header ends with `Saigon`, the status reads `times in Saigon`, and the fixture's timed tasks move by the offset between the zones.
- **Next day.** `keys -- ]`, `shot nav-next-day`. The header reads `Tomorrow`, the week strip highlights the next day, and there is no now-line. Press `[` to return.

## Gotchas

- The first visit to a day or Inbox that is not cached shows `loading…` in the list title and `Loading…` in the list. A live inbox can take a couple of seconds. Wait for `N tasks` before asserting the list.
- `t` on Today and `i` on Inbox do not reload by themselves. `r` does.
- Left and Right change the day when find is closed. They move the selection when find is open.
- `gg` is two `g` keys. A single `g` does nothing visible.
- `h` and `l` move focus between the list and the checklist. They no longer flip Inbox; Tab does.
