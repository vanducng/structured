#!/usr/bin/env bash
# Runs inside `asciinema rec`: hidden fixture setup, then each demo command typed visibly.
set -u
state=$1
export XDG_CONFIG_HOME="$state/cfg" STRUCTURED_MCP_URL="http://127.0.0.1:$(cat "$state/port")/mcp" PATH="$2:$PATH"
note() { printf '\033[2m# %s\033[0m\n' "$1"; sleep 0.6; }
run() {
  printf '\033[1;32m$\033[0m '
  local s=$1 i
  for ((i = 0; i < ${#s}; i++)); do printf '%s' "${s:i:1}"; sleep 0.02; done
  sleep 0.35
  printf '\n'
  eval "$s"
  sleep "${2:-1.8}"
}
clear
note "structured: a CLI and TUI for the Structured day planner"
note "Now: 12 hours either side of the current time, across midnight, late work flagged"
run "structured now --text" 3.5
clear
note "Working across zones: the same tasks in ET"
run "structured now --text --tz et | head -16" 3.2
clear
note "Every task carries a ref. Agents read it from --json and pass it back"
run "REF=\$(structured today --json | jq -r '.tasks[] | select(.title | startswith(\"Finish\")) | .ref')" 0.6
run 'structured subtask list "$REF"'
note "Subtasks: add, then tick by number"
run 'structured subtask add "$REF" "Write the runbook"'
run 'structured subtask done "$REF" 3' 2.2
clear
note "A recurring day must say how far an edit reaches"
run "RUN=\$(structured today --json | jq -r '.tasks[] | select(.title == \"Read 20 pages\") | .ref')" 0.6
run 'structured edit "$RUN" --time 07:00' 2.4
note "--dry-run prints the exact request and sends nothing"
run 'structured edit "$RUN" --time 07:00 --scope day --dry-run --json | jq .result' 3
clear
note "And the TUI: checklist, search, scope-aware prompts, the 24h view, and z for time zones"
run "structured" 0.5
clear
note "Install: github.com/vanducng/structured/releases"
sleep 2
