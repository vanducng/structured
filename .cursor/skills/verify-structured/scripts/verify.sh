#!/usr/bin/env bash
set -euo pipefail

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/../../../.." && pwd)
state_root=${STRUCTURED_VERIFY_STATE:-${TMPDIR:-/tmp}/structured-verify}
chrome=${STRUCTURED_VERIFY_CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}

die() {
  printf 'verify: %s\n' "$1" >&2
  exit 1
}

check_id() {
  [[ "$1" =~ ^[A-Za-z0-9._-]+$ ]] || die "$2 must be letters, numbers, dots, or dashes"
}

need_run() {
  check_id "$1" "run id"
  state_dir=$state_root/$1
  [[ -f "$state_dir/run.env" ]] || die "no run $1; launch it first"
  # shellcheck disable=SC1090
  source "$state_dir/run.env"
}

launch() {
  local run_id=$1
  shift
  local fixture=0 size=120x32 config_file=""
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --fixture) fixture=1 ;;
      --size) size=$2; shift ;;
      --config) config_file=$2; shift ;;
      *) die "unknown launch flag $1" ;;
    esac
    shift
  done
  [[ "$size" =~ ^[0-9]+x[0-9]+$ ]] || die "size must look like 120x32"
  check_id "$run_id" "run id"
  command -v tmux >/dev/null || die "tmux is not installed"
  if [[ -x "$HOME/.cargo/bin/cargo" ]]; then
    export PATH="$HOME/.cargo/bin:$PATH"
  fi
  state_dir=$state_root/$run_id
  [[ ! -e "$state_dir" ]] || die "run $run_id already exists"
  local session=sverify-$run_id
  tmux has-session -t "$session" 2>/dev/null && die "tmux session $session already exists"
  mkdir -p "$state_dir/evidence"
  local mcp_url="" fixture_pid="" config_home=$state_dir/config
  if [[ $fixture -eq 1 ]]; then
    python3 -I "$script_dir/fixture-server.py" "$state_dir/port" "$state_dir/evidence/calls.jsonl" >/dev/null 2>&1 &
    fixture_pid=$!
    for _ in $(seq 1 40); do [[ -s "$state_dir/port" ]] && break; sleep 0.1; done
    [[ -s "$state_dir/port" ]] || die "fixture server did not start"
    mcp_url="http://127.0.0.1:$(cat "$state_dir/port")/mcp"
    mkdir -p "$config_home/structured"
    printf '{"client_id":"fixture","access_token":"fixture","refresh_token":"","expires_at":4102444800}\n' \
      >"$config_home/structured/auth.json"
    chmod 600 "$config_home/structured/auth.json"
    [[ -z "$config_file" ]] || cp "$config_file" "$config_home/structured/config.toml"
  else
    [[ -z "$config_file" ]] || die "--config is for fixture runs; live runs read the person's own config"
    # Live runs use the person's own config: a refresh in a copy would rotate and orphan their token.
    config_home=${XDG_CONFIG_HOME:-$HOME/.config}
    [[ -f "$config_home/structured/auth.json" ]] || die "missing auth.json; sign in with structured login in a terminal"
  fi
  (cd "$repo_root" && cargo build --manifest-path Cargo.toml)
  local binary=$repo_root/target/debug/structured
  [[ -x "$binary" ]] || die "debug binary was not built"
  local evidence_dir=$state_dir/evidence
  {
    printf '#!/usr/bin/env bash\n'
    printf 'export XDG_CONFIG_HOME=%q\n' "$config_home"
    [[ -n "$mcp_url" ]] && printf 'export STRUCTURED_MCP_URL=%q\n' "$mcp_url"
    printf 'exec %q tui\n' "$binary"
  } >"$state_dir/run.sh"
  chmod 700 "$state_dir/run.sh"
  {
    printf 'session=%q\n' "$session"
    printf 'binary=%q\n' "$binary"
    printf 'config_home=%q\n' "$config_home"
    printf 'evidence_dir=%q\n' "$evidence_dir"
    printf 'fixture_pid=%q\n' "$fixture_pid"
    printf 'mcp_url=%q\n' "$mcp_url"
  } >"$state_dir/run.env"
  local width=${size%x*} height=${size#*x}
  tmux new-session -d -s "$session" -x "$width" -y "$height" "$state_dir/run.sh"
  tmux set-option -t "$session" status off
  tmux set-window-option -t "$session" remain-on-exit on
  tmux resize-window -t "$session" -x "$width" -y "$height"
  for _ in $(seq 1 40); do
    if ready_now; then
      printf 'run_id=%s\nsession=%s\nbinary=%s\nmode=%s\nevidence_dir=%s\n' \
        "$run_id" "$session" "$binary" "$([[ $fixture -eq 1 ]] && echo fixture || echo live)" "$evidence_dir"
      return 0
    fi
    sleep 0.5
  done
  capture_to "$evidence_dir/launch-failed"
  die "TUI did not show a task count; see $evidence_dir/launch-failed.txt"
}

ready_now() {
  tmux has-session -t "$session" 2>/dev/null || return 1
  local pane
  pane=$(tmux capture-pane -p -t "$session") || return 1
  grep -q '? help' <<<"$pane" || return 1
  grep -Eq '[0-9]+ tasks? ' <<<"$pane" || return 1
  ! grep -q 'Loading…' <<<"$pane"
}

capture_to() {
  tmux capture-pane -p -t "$session" >"$1.txt"
  tmux capture-pane -p -e -t "$session" >"$1.ansi"
}

doctor() {
  need_run "$1"
  tmux has-session -t "$session" 2>/dev/null || die "session $session is not running"
  local command_name pid txt
  command_name=$(tmux list-panes -t "$session" -F '#{pane_current_command}')
  pid=$(tmux list-panes -t "$session" -F '#{pane_pid}')
  [[ "$command_name" == "structured" ]] || die "pane command is $command_name, want structured"
  txt=$(lsof -a -p "$pid" -d txt 2>/dev/null | awk '/structured$/ { print $NF; exit }')
  [[ "$txt" == "$binary" ]] || die "pane binary is ${txt:-missing}, want $binary"
  ready_now || die "the hint (? help) or the task count is not on screen"
  printf 'session=%s\ncommand=structured\nbinary=%s\nmode=%s\nhint=ok\nstatus=ready\n' \
    "$session" "$binary" "$([[ -n "$mcp_url" ]] && echo fixture || echo live)"
}

keys() {
  local run_id=$1
  shift
  [[ "${1:-}" == "--" ]] || die "usage: verify.sh keys <run-id> -- <key>..."
  shift
  need_run "$run_id"
  tmux has-session -t "$session" 2>/dev/null || die "session $session is not running"
  local key
  for key in "$@"; do
    case "$key" in
      Escape | Enter | Up | Down | Left | Right | BSpace | Tab | Space | Home | End | C-c | C-d | C-u | C-g | C-y | C-e | C-w | C-a | C-k)
        tmux send-keys -t "$session" "$key"
        ;;
      *)
        [[ ${#key} -eq 1 ]] || die "unknown key $key"
        tmux send-keys -t "$session" -l "$key"
        ;;
    esac
    sleep 0.12
  done
}

type_text() {
  need_run "$1"
  tmux send-keys -t "$session" -l "$2"
  sleep 0.2
}

capture() {
  check_id "$2" "capture name"
  need_run "$1"
  tmux has-session -t "$session" 2>/dev/null || die "session $session is not running"
  capture_to "$evidence_dir/$2"
  printf 'evidence=%s.txt\nbytes=%s\n' "$evidence_dir/$2" "$(wc -c <"$evidence_dir/$2.txt" | tr -d ' ')"
}

shot() {
  check_id "$2" "capture name"
  need_run "$1"
  [[ -x "$chrome" ]] || die "Chrome not found; set STRUCTURED_VERIFY_CHROME"
  capture_to "$evidence_dir/$2"
  python3 -I "$script_dir/render.py" "$evidence_dir/$2.ansi" "$evidence_dir/$2.html"
  local cols rows
  cols=$(tmux display -p -t "$session" '#{pane_width}')
  rows=$(tmux display -p -t "$session" '#{pane_height}')
  "$chrome" --headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=2 \
    --window-size="$((cols * 85 / 10 + 32)),$((rows * 35 / 2 + 28))" \
    --screenshot="$evidence_dir/$2.png" "file://$evidence_dir/$2.html" >/dev/null 2>&1
  printf 'png=%s.png\n' "$evidence_dir/$2"
}

calls() {
  need_run "$1"
  [[ -n "$mcp_url" ]] || die "calls are only logged in --fixture runs"
  cat "$evidence_dir/calls.jsonl" 2>/dev/null || true
}

cleanup() {
  need_run "$1"
  if tmux has-session -t "$session" 2>/dev/null; then
    tmux kill-session -t "$session"
  fi
  if [[ -n "${fixture_pid:-}" ]]; then
    kill "$fixture_pid" 2>/dev/null || true
  fi
  printf 'stopped=%s\nevidence_dir=%s\n' "$session" "$evidence_dir"
}

usage() {
  die "usage: verify.sh launch|doctor|keys|type|capture|shot|calls|cleanup <run-id> ..."
}

[[ $# -ge 2 ]] || usage
action=$1
shift
case "$action" in
  launch) launch "$@" ;;
  doctor) doctor "$1" ;;
  keys) keys "$@" ;;
  type) [[ $# -eq 2 ]] || usage; type_text "$1" "$2" ;;
  capture) [[ $# -eq 2 ]] || usage; capture "$1" "$2" ;;
  shot) [[ $# -eq 2 ]] || usage; shot "$1" "$2" ;;
  calls) calls "$1" ;;
  cleanup) cleanup "$1" ;;
  *) usage ;;
esac
