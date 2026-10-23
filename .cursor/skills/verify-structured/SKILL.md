---
name: verify-structured
description: "Drive the structured TUI in an isolated tmux session and capture the screen. Use after a TUI change, to prove find, navigation, help, or schedule prompts, or when cargo test is green but the frame is unverified. Reads are safe. Writes stay off unless the user authorizes them."
---

# Verify structured

Prove the `structured` terminal UI by launching it the way a person does and reading the frame. `cargo test` checks list state, parsers, and key decisions. It does not draw. This skill is the screen loop.

The CLI is the other surface. `cargo test` and `structured --help` cover it. Do not use this skill to prove command flags.

## Launch

From the repo root:

```bash
RUN_ID=verify-$(date +%Y%m%d%H%M%S)
.cursor/skills/verify-structured/scripts/verify.sh launch "$RUN_ID"
```

Launch builds `target/debug/structured`, copies the existing `auth.json` into an empty `XDG_CONFIG_HOME` for this run, and starts `structured tui` in a new tmux session `sverify-$RUN_ID` at 120x32. It waits until the hint contains `f find` and `m min` and a status line ends in `N tasks`.

An empty config with no auth file cannot reach that screen. `Session::connect` returns before the first draw, and the process exits. Sign in once with `structured login` in a terminal. Launch copies that file. It never prints it.

The session is not the user's open TUI. Refuse to drive any tmux session this run did not create.

## Doctor

Run this first whenever the frame looks wrong:

```bash
.cursor/skills/verify-structured/scripts/verify.sh doctor "$RUN_ID"
```

Doctor passes only when all of these are true:

- tmux session `sverify-$RUN_ID` exists
- the pane command is `structured`
- the pane's executable is this run's `target/debug/structured`
- the hint contains `f find` and `m min`
- a status line matches ` N tasks`

Doctor prints those facts and no pane body. A missing hint, a different binary, or a dead pane is a failed doctor, not a reason to reuse another `structured` process.

## Drive

Send keys only through the helper. One argument is one key. Words `Escape`, `Enter`, `Up`, `Down`, `Left`, `Right`, `BSpace`, `Tab`, `Space`, `C-d`, `C-u`, `C-g`, `C-y`, and `C-e` are special. Every other argument is one literal character, including `/`, `?`, and `q`.

```bash
.cursor/skills/verify-structured/scripts/verify.sh keys "$RUN_ID" -- / z z q q
.cursor/skills/verify-structured/scripts/verify.sh capture "$RUN_ID" find-open
```

Stable handles are the text the UI draws, not coordinates:

- Hint: `t today`, `i inbox`, `f find`, `D date`, `T time`, `m min`, the visible count, and `/query_` while find is open
- Status: a line ending in `N tasks`, or `loading`, or `saving`
- Detail prompts: `add:`, `title:`, `date:`, `time:`, `duration:`, each ending in `_`
- Help detail: `f again next match` and `m duration`
- Confirm detail: `Delete this task?`

Read [features/README.md](features/README.md) and drive the feature file that matches the change. A proof of one entry point does not cover the others listed there.

Keys that write to Structured Cloud, as soon as they are pressed or as soon as Enter confirms a prompt: `x`, Space, `a` then Enter, `e` then Enter, `D` then Enter, `T` then Enter, `m` then Enter, `,`, `.`, and `d` then `y`. Do not send them unless the user has authorized that write and `STRUCTURED_VERIFY_WRITES=1` is set. Opening `D`, `T`, or `m` and pressing Escape does not write.

## Evidence

Evidence stays under the `evidence_dir` printed by launch, normally `$TMPDIR/structured-verify/$RUN_ID/evidence/`. Capture writes `<name>.txt` there. The pane transcript is the proof. It includes task titles, so do not commit it, paste it into chat, or copy it into the repo.

For each step record the feature id, the keys, and the capture. The proof is the action plus the next frame, not the last frame alone. Assert on stable strings (`/zzqq_`, `duration:`, `N tasks`). Do not treat a title match as the oracle.

`cargo test` remains the check for state that the screen does not show: checkbox pins, stale reloads, and duration parsing. A green test run is not screen proof. A capture is not parser proof.

## Cleanup

```bash
.cursor/skills/verify-structured/scripts/verify.sh cleanup "$RUN_ID"
```

Cleanup kills tmux session `sverify-$RUN_ID` and no other session. It does not delete the evidence directory. After cleanup, `tmux has-session -t sverify-$RUN_ID` must fail and the capture files must still be readable.

## Helpers

`.cursor/skills/verify-structured/scripts/verify.sh` is the harness.

```bash
.cursor/skills/verify-structured/scripts/verify.sh launch <run-id>
.cursor/skills/verify-structured/scripts/verify.sh doctor <run-id>
.cursor/skills/verify-structured/scripts/verify.sh keys <run-id> -- <key>...
.cursor/skills/verify-structured/scripts/verify.sh capture <run-id> <name>
.cursor/skills/verify-structured/scripts/verify.sh cleanup <run-id>
```

`<name>` is letters, numbers, dots, or dashes. Launch fails if the run id or the tmux session already exists.

Keep the feature map honest as keys change. The maintenance entry is `/maintain-verification-skill`.
