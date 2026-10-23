# Development guidelines

## Setup

Rust 1.88 or newer. The lockfile is committed.

```bash
git clone https://github.com/vanducng/structured
cd structured
cargo build
cargo run -- --help
```

`cargo run -- tui` against your own account needs `structured login` first. For anything that writes, use the fixture below instead.

## Checks

CI runs these, and a change is not done until they pass:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Layout

[System architecture](system-architecture.md) lists every module. In short:

- `src/main.rs` parses and reports errors. `src/lib.rs` dispatches.
- `src/cli.rs` is the clap surface and the agent contract. Help text changes are interface changes.
- `src/api.rs` turns commands into Structured tool calls. `src/mcp.rs` and `src/auth.rs` talk to Structured Cloud.
- `src/agenda.rs` and `src/output.rs` decide what a day looks like, for both `--text` and the TUI.
- `src/tui/app/` is state and keys, `src/tui/view/` is drawing, `src/tui/worker.rs` is the network thread.

## Conventions

- Default to no comments. Write one only when a reader would be surprised: a server quirk, a hidden constraint, a non-obvious invariant.
- CLI output is a contract. New fields are fine. Renaming or removing a JSON field, an error code, or a flag is a breaking change.
- Errors carry a code, a message that names the bad value, and `next_steps` an agent can run.
- Recurring days are explicit. A write on one must say `--scope day` or `--scope series`, or be a change that only ever touches that day.
- Use generic sample titles in tests, fixtures, docs, and recordings. Never commit real task titles, `auth.json`, or tokens.
- Commits follow Conventional Commits, because release-please reads them. See [Deployment](deployment.md#releases).

## Tests

| Kind | Where | Runs |
| --- | --- | --- |
| Unit | `#[cfg(test)]` modules next to the code, and `src/tui/app/tests.rs` for TUI state | Parsers, agenda rows, scope rules, optimistic writes, stale reloads |
| CLI | `tests/cli.rs` with the in-process MCP mock in `tests/mock/` | The real binary, its JSON, exit codes, and the exact tool arguments it sends |

The mock records every tool call, so a test asserts on the request, not just the output. `Mock::config_toml` writes a `config.toml` for a run.

`clock.rs` keeps the time zone in a process-wide slot. Unit tests run in parallel threads, so a unit test must not switch it. Put zone behavior in a pure function, as `config::next_zone` is, or test it through the binary in `tests/cli.rs`.

## TUI screen check

`cargo test` does not draw. `.cursor/skills/verify-structured/` drives the real TUI in its own tmux session and captures text, ANSI, and PNG frames:

```bash
RUN_ID=verify-$(date +%Y%m%d%H%M%S)
.cursor/skills/verify-structured/scripts/verify.sh launch "$RUN_ID" --fixture
.cursor/skills/verify-structured/scripts/verify.sh keys "$RUN_ID" -- w z
.cursor/skills/verify-structured/scripts/verify.sh shot "$RUN_ID" after-zone
.cursor/skills/verify-structured/scripts/verify.sh cleanup "$RUN_ID"
```

`--fixture` starts `scripts/fixture-server.py`, a local stand-in for Structured Cloud with generic tasks around today. It answers like the real server across zones: a task keeps its filed day, a zoned time counts from that day's midnight in the zone asked for, and recurring tasks float. That makes `z` and edits after `z` testable. `--config FILE` gives the run a `config.toml`. The skill's `SKILL.md` lists the stable strings to assert on, and `features/` has one script per feature.

## Demo recording

`scripts/record-demo.sh` in the same skill records the README screencast with asciinema against the fixture, drives the TUI through tmux, and fails if the cast holds a home path, a token, or a local address. With `agg` installed it also renders `docs/assets/demo.gif`, which the README shows because GitHub cannot load asciinema's preview images. When the UI changes, record again, upload with `asciinema upload --visibility public`, and point the README link at the new recording.
