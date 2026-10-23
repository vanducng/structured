# structured

Unofficial Structured day-planner CLI and TUI. The binary is `structured`.

Use [skills/structured/SKILL.md](skills/structured/SKILL.md) to operate the installed CLI. `--help` is the command contract. Do not treat the Rust library API as stable.

## Layout

- `src/lib.rs` dispatches commands. `src/main.rs` only parses and reports errors.
- `src/cli.rs` is the clap surface. `src/subtask.rs` runs the checklist commands.
- `src/api.rs` builds tool arguments. `src/mcp.rs` and `src/auth.rs` talk to Structured Cloud. `STRUCTURED_MCP_URL` overrides the endpoint for tests.
- `src/model.rs` holds `Task`, `Subtask`, and `TaskRef` (the `ref` string). `src/clock.rs` parses and formats days and times.
- `src/agenda.rs` orders a day and finds gaps, overlaps, and the now-line for both the TUI and `--text`. `src/output.rs` prints JSON and text.
- `src/tui/` is the terminal UI (ratatui, tui-input): `app/` state and keys, `view/` drawing, `worker.rs` network I/O off the UI thread. `src/config.rs` loads the pane layout.
- `tests/cli.rs` runs the binary against the in-process mock in `tests/mock/`.

## Checks

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

`.cursor/skills/verify-structured/SKILL.md` drives the TUI in its own tmux session, against a local fixture server (`--fixture`) or the real account, and renders PNG frames. `cargo test` does not draw the screen. Do not attach that session to a TUI that is already open.

Do not commit `target/`, auth files, or tokens. `publish = false`. Releases come from release-please: merge its release PR to bump the version and tag `vX.Y.Z`; `.github/workflows/release.yml` then builds and attaches the archives. Do not edit `CHANGELOG.md` or bump `Cargo.toml` by hand.
