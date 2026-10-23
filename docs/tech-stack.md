# Tech stack

Versions are the ones locked in `Cargo.lock`. `Cargo.toml` holds the ranges.

## Language and build

| Item | Version | Notes |
| --- | --- | --- |
| Rust | edition 2024, MSRV 1.88 | `rust-version` in `Cargo.toml` |
| Release profile | `strip = true`, `lto = "thin"` | Smaller release binaries |
| Targets | `aarch64-apple-darwin`, `x86_64-unknown-linux-gnu` | Built by `.github/workflows/release.yml` |

## Crates

| Crate | Locked | Used for |
| --- | --- | --- |
| clap | 4.6.7 | Arguments, help, and errors (`derive`, `wrap_help`) |
| clap_complete | 4.6.11 | `structured completions SHELL` |
| ratatui | 0.30.2 | TUI drawing, with crossterm 0.29.0 as the terminal backend |
| tui-input | 0.15.5 | One-line prompts with a real cursor |
| unicode-width | 0.2.2 | Column widths for titles in the list |
| reqwest | 0.12.28 | Blocking HTTP with rustls, for MCP and OAuth |
| serde, serde_json | 1.0.229, 1.0.154 | Tool arguments, results, and JSON output |
| chrono | 0.4.45 | Days, times, and durations |
| chrono-tz | 0.10.4 | IANA zones for `timezones` and `--tz` |
| iana-time-zone | 0.1.65 | The system zone when none is configured |
| sha2, base64, rand | 0.10.9, 0.22.1, 0.8.8 | PKCE verifier and challenge |
| urlencoding | 2.1.3 | OAuth query strings |
| thiserror | 2.0.21 | Error types |

Tests add assert_cmd 2.2.2 and predicates 3.1.4.

## Services

| Service | Address | Used for |
| --- | --- | --- |
| Structured Cloud MCP | `https://mcp.structured.app/mcp` | Every read and write |
| Structured OAuth | discovered from `/.well-known/oauth-protected-resource` | Sign-in and token refresh |
| GitHub Actions | `.github/workflows/` | CI and releases |
| GitHub Releases | `vanducng/structured` | Archives and `SHA256SUMS` |
