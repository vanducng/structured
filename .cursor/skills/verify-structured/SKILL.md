---
name: verify-structured
description: "Drive the structured TUI in an isolated tmux session, against a local fixture server or the real account, and capture text, ANSI, and PNG frames. Use after a TUI change, to prove find, navigation, help, schedule prompts, or subtasks, or when cargo test is green but the frame is unverified. Fixture writes are safe. Live writes stay off unless the user authorizes them."
---

# Verify structured

Prove the `structured` terminal UI by launching it the way a person does and reading the frame. `cargo test` checks list state, parsers, and key decisions. It does not draw. This skill is the screen loop.

The CLI is the other surface. `cargo test` and `structured --help` cover it. Do not use this skill to prove command flags.

## Launch

From the repo root:

```bash
RUN_ID=verify-$(date +%Y%m%d%H%M%S)
.cursor/skills/verify-structured/scripts/verify.sh launch "$RUN_ID" --fixture
```

Launch builds `target/debug/structured` and starts `structured tui` in a new tmux session `sverify-$RUN_ID` at 120x32 (`--size 80x24` for a small terminal). It waits until the footer shows `? help` and a list title shows `N tasks`.

Two modes:

- `--fixture` starts `scripts/fixture-server.py`, a local stand-in for Structured Cloud with generic tasks dated around today, and points the binary at it through `STRUCTURED_MCP_URL`. Writes are safe here and every tool call lands in `evidence/calls.jsonl`. Like the real server, the fixture keeps each task's filed day and counts a zoned time from that day's midnight in the `timezone` each call names; recurring tasks float. Add `--config FILE` to give the run a `config.toml`, for example to open on the 24h view or to list time zones. Use this for screenshots, critic reviews, and every write proof.
- Without `--fixture` the run uses the person's own config in place and reads their real account. Use it only to prove that real data renders. It never copies `auth.json`: a token refresh inside a copy would rotate the refresh token and could sign the person out. The person signs in once with `structured login`. Launch never prints the auth file.

The session is not the user's open TUI. Refuse to drive any tmux session this run did not create.

## Doctor

Run this first whenever the frame looks wrong:

```bash
.cursor/skills/verify-structured/scripts/verify.sh doctor "$RUN_ID"
```

Doctor passes only when the session `sverify-$RUN_ID` exists, the pane command is this run's `target/debug/structured`, the footer shows `? help`, and a list title shows `N tasks` with no `Loading…`. It prints those facts and the mode, never the pane body.

## Drive

Send keys only through the helper. One argument is one key. `Escape`, `Enter`, `Up`, `Down`, `Left`, `Right`, `BSpace`, `Tab`, `Space`, `Home`, `End`, `C-c`, `C-d`, `C-u`, `C-g`, `C-y`, `C-e`, `C-w`, `C-a`, and `C-k` are special. Every other argument is one literal character, including `/`, `?`, and `q`. `type` sends a whole string into an open prompt.

```bash
.cursor/skills/verify-structured/scripts/verify.sh keys "$RUN_ID" -- / z z q q
.cursor/skills/verify-structured/scripts/verify.sh type "$RUN_ID" "Dry run"
.cursor/skills/verify-structured/scripts/verify.sh capture "$RUN_ID" find-open
.cursor/skills/verify-structured/scripts/verify.sh shot "$RUN_ID" find-open
.cursor/skills/verify-structured/scripts/verify.sh calls "$RUN_ID"
```

`capture` writes `<name>.txt` and `<name>.ansi`. `shot` also renders the ANSI frame through `scripts/render.py` and headless Chrome into `<name>.png`, which is what a visual reviewer reads. `calls` prints the fixture's tool-call log.

Stable handles are the text the UI draws, not coordinates:

- Header: the long date and `Today`, `Now` in the 24h view, or `Inbox` and `N open`, plus the week strip and `Inbox N`. With `timezones` configured it ends with the zone clock, like `02:39 Saigon`.
- List title: `Today · N tasks` (day view), `Now · N tasks` (24h view), `Inbox · N tasks`, or a date, with `/query` while a search is applied. Right side: `N late` in yellow when any, `N done`, and `planned`.
- Footer in the list: `⏎ subtasks`, `x done`, `f find`, `m min`, `? help`. In the checklist: a `SUBTASKS` chip and `x tick`, `h back`.
- Footer chips: `FIND` with `/query`, a prompt name (`NEW TASK`, `DATE`, `TIME`, `DURATION`), an edit card field (`TITLE`, `SUBTASK`, `NEW SUBTASK`, `NOTE`) with `Details · editing` on the pane, either plus `every repeat` or `this day only` on a recurring task, `DELETE`, `HELP`.
- Status: `· times in LABEL` after `z`, `· 24h view, …` and `· day view, …` after `w`, `● saving…`, `✓ saved`, `✓ subtasks saved`, `✗ <error>`, `· task kept`, and `● finishing N saves, q again quits now` after `q` with saves pending.
- Help modal: `Keys`, `next, previous`, `minutes`, `Subtasks`.
- Confirm modal: `Delete task` or `Delete subtask`, and `y delete`.

The cursor of a prompt is a real terminal cursor, so a text capture does not show it. Assert on the chip and the draft text.

Read [features/README.md](features/README.md) and drive the feature file that matches the change. A proof of one entry point does not cover the others listed there.

Keys that write, as soon as they are pressed or as soon as Enter or `y` confirms: `x`, Space, `a`/`D`/`T`/`m` then Enter, `,`, `.`, `d` then `y`, and in the checklist `x`, Space, `d` then `y`. In the edit card (`e`, `A`, or `a`/`e` in the checklist) a changed field writes on `tab`, an arrow out of it, Enter, or Escape. In a live run do not send them unless the user authorized that write and `STRUCTURED_VERIFY_WRITES=1` is set. In a `--fixture` run they are safe. Opening a prompt and pressing Escape never writes. Opening the edit card and pressing Escape without typing never writes.

## Evidence

Evidence stays under the `evidence_dir` printed by launch, normally `$TMPDIR/structured-verify/$RUN_ID/evidence/`. A live run's captures include the person's task titles, so do not commit them, paste them into chat, or hand them to another agent. Fixture captures hold only generic titles and are fine to share with reviewers.

For each step record the feature id, the keys, and the capture. The proof is the action plus the next frame, not the last frame alone. Assert on stable strings (`/zzqq_`, `duration:`, `N tasks`). Do not treat a title match as the oracle.

`cargo test` remains the check for state that the screen does not show: checkbox pins, stale reloads, and duration parsing. A green test run is not screen proof. A capture is not parser proof.

## Cleanup

```bash
.cursor/skills/verify-structured/scripts/verify.sh cleanup "$RUN_ID"
```

Cleanup kills tmux session `sverify-$RUN_ID`, stops this run's fixture server, and touches nothing else. It does not delete the evidence directory. After cleanup, `tmux has-session -t sverify-$RUN_ID` must fail and the capture files must still be readable.

## Helpers

`.cursor/skills/verify-structured/scripts/verify.sh` is the harness.

```bash
.cursor/skills/verify-structured/scripts/verify.sh launch <run-id> [--fixture] [--size WxH]
.cursor/skills/verify-structured/scripts/verify.sh doctor <run-id>
.cursor/skills/verify-structured/scripts/verify.sh keys <run-id> -- <key>...
.cursor/skills/verify-structured/scripts/verify.sh type <run-id> <text>
.cursor/skills/verify-structured/scripts/verify.sh capture <run-id> <name>
.cursor/skills/verify-structured/scripts/verify.sh shot <run-id> <name>
.cursor/skills/verify-structured/scripts/verify.sh calls <run-id>
.cursor/skills/verify-structured/scripts/verify.sh cleanup <run-id>
```

`<name>` is letters, numbers, dots, or dashes. Launch fails if the run id or the tmux session already exists.

Keep the feature map honest as keys change. The maintenance entry is `/maintain-verification-skill`.
