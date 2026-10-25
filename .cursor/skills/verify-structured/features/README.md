# structured TUI verification map

This directory is the maintained source for verifying the user-facing TUI. Read the index, then use the matching feature file.

## Baseline preconditions

- Launch with `.cursor/skills/verify-structured/scripts/verify.sh launch <run-id> --fixture` from the repo root. Drop `--fixture` only to prove real data renders.
- The run has its own tmux session `sverify-<run-id>` and its own `XDG_CONFIG_HOME`.
- A fixture run serves generic tasks dated around today and logs every tool call. A live run copies an existing `auth.json` and exits before the first frame without one.
- `verify.sh doctor <run-id>` reports `command=structured`, this run's debug binary, the mode, `hint=ok`, and `status=ready`.
- Never drive a tmux session this run did not start. A person may already have `structured` open.

## Driving conventions

- Start every recipe from the ready Today view unless its preconditions say otherwise.
- `verify.sh` means `.cursor/skills/verify-structured/scripts/verify.sh` from the repo root.
- Send keys only with `verify.sh keys`. One argument is one keypress.
- Assert on footer hints, chips, modal titles, and the `N tasks` list title. In a live run, task titles are not oracles. Fixture titles are fixed and may be asserted.
- Capture after the action that should change the frame, then capture the resulting state.
- Do not commit captures. Live captures contain the person's schedule.
- For a visual review, `verify.sh shot` writes a PNG. Review fixture frames only.

## Proof and skip reporting

- Record the feature id, the entry point, the keys, and the evidence path.
- In a live run, a path that needs a cloud write fails unless the user authorized it and `STRUCTURED_VERIFY_WRITES=1` is set. Say which precondition failed. Prefer a fixture run, where `verify.sh calls` is the write proof.
- Do not mark a write feature verified because a read-only prompt opened.
- Do not mark a second entry point verified because a different one passed.

## Feature entry contract

Each feature file has an H1, one short paragraph, then these H2 sections in order: `Sub-features`, `How to get to it (user POV)`, `Driving it with verify.sh`, `Gotchas`.

## Features

- [Find](./find.md) filters the list from `/` or `f` and moves between matches.
- [Navigate](./navigate.md) moves the selection and switches Today, Inbox, and day.
- [Help](./help.md) opens the key list in a modal.
- [Schedule](./schedule.md) edits date, time, and duration. Enter writes. Escape does not.
- [Complete and delete](./complete.md) toggles the box and deletes. Both write immediately.
- [Subtasks](./subtasks.md) opens the checklist and ticks, adds, renames, and deletes subtasks.
- [Edit card](./edit.md) edits the title, subtasks, and note in place from `e`. Each field writes when the cursor leaves it.
