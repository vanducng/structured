# Configuration

Everything works with no configuration. Settings live in `config.toml` next to the sign-in file, in `$XDG_CONFIG_HOME/structured/`, or `~/.config/structured/` when that is unset.

## config.toml

```toml
layout = "side"
detail_percent = 46
view = "now"
timezones = ["ET=America/New_York", "Saigon=Asia/Ho_Chi_Minh"]
```

| Key | Values | Default | Effect |
| --- | --- | --- | --- |
| `layout` | `side`, `stack` | `side` | Details beside the list, or below it. Side by side needs 72 columns, and narrower terminals stack. |
| `detail_percent` | 30 to 70 | 46 | Width of the details pane beside the list, in percent. Capped at 40 under 100 columns. |
| `view` | `day`, `now` | `day` | The view the TUI opens today on: the calendar day, or 24 hours around now. `w` switches. |
| `timezones` | list of zones | none | Zones to work in. The first is the default for the TUI and the CLI. `z` cycles through them. |

The file is read line by line as `key = value`. Lines starting with `#` are comments. Unknown keys are ignored.

## Time zones

Each `timezones` entry is `LABEL=Area/City`, or a bare IANA name such as `Europe/Paris`, which is labelled by its city. The label is what the TUI header and `--tz` use.

Which zone a run uses:

1. `--tz ZONE`, a configured label in any case (`saigon`) or an IANA name.
2. The first entry in `timezones`.
3. The system zone.

An unknown name in the list is skipped. The TUI shows the problem in red at startup. An unknown `--tz` is a `USAGE` error that lists the configured labels.

## Files

| Path | Holds |
| --- | --- |
| `auth.json` | The Structured Cloud sign-in from `structured login`. Mode 600. The tool never prints it. `structured logout` removes it. |
| `config.toml` | The settings above |

## Environment

| Variable | Effect |
| --- | --- |
| `XDG_CONFIG_HOME` | Moves the config directory to `$XDG_CONFIG_HOME/structured` |
| `STRUCTURED_MCP_URL` | Points the binary at another MCP endpoint. Tests and the TUI screen check use it to reach a local stand-in. |
| `VISUAL`, `EDITOR` | The editor `ctrl-g` opens for a note |
