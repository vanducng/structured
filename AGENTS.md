# structured

Unofficial Structured day-planner CLI and TUI. The binary is `structured`.

Use [skills/structured/SKILL.md](skills/structured/SKILL.md) to operate the installed CLI. `--help` is the command contract. Do not treat the Rust library API as stable.

## Layout

- `src/lib.rs` dispatches commands. `src/main.rs` only parses and reports errors.
- `src/cli.rs` is the clap surface.
- `src/mcp.rs` and `src/auth.rs` talk to Structured Cloud.
- `src/tui.rs` is the terminal UI.
- `tests/cli.rs` runs the binary.

## Checks

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Do not commit `target/`, auth files, or tokens. `publish = false`. Releases are GitHub tags named `v*`, built by `.github/workflows/release.yml`.
