#!/usr/bin/env bash
set -euo pipefail

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH= cd -- "$script_dir/../../../.." && pwd)
state_root=${STRUCTURED_VERIFY_STATE:-${TMPDIR:-/tmp}/structured-verify}

die() {
  printf 'verify: %s\n' "$1" >&2
  exit 1
}

need_run() {
  local run_id=$1
  [[ "$run_id" =~ ^[A-Za-z0-9._-]+$ ]] || die "run id must be letters, numbers, dots, or dashes"
  state_dir=$state_root/$run_id
  [[ -f "$state_dir/run.env" ]] || die "no run $run_id; launch it first"
  # shellcheck disable=SC1090
  source "$state_dir/run.env"
}

launch() {
  local run_id=$1
  [[ "$run_id" =~ ^[A-Za-z0-9._-]+$ ]] || die "run id must be letters, numbers, dots, or dashes"
  command -v tmux >/dev/null || die "tmux is not installed"
  if [[ -x "$HOME/.cargo/bin/cargo" ]]; then
    export PATH="$HOME/.cargo/bin:$PATH"
  fi
  state_dir=$state_root/$run_id
  [[ ! -e "$state_dir" ]] || die "run $run_id already exists"
  local session=sverify-$run_id
  tmux has-session -t "$session" 2>/dev/null && die "tmux session $session already exists"
  local auth_src=${XDG_CONFIG_HOME:-$HOME/.config}/structured/auth.json
  [[ -f "$auth_src" ]] || die "missing auth.json; sign in with structured login in a terminal"
  mkdir -p "$state_dir/config/structured" "$state_dir/evidence"
  cp "$auth_src" "$state_dir/config/structured/auth.json"
  chmod 600 "$state_dir/config/structured/auth.json"
  (
    cd "$repo_root"
    cargo build --manifest-path Cargo.toml
  )
  local binary=$repo_root/target/debug/structured
  [[ -x "$binary" ]] || die "debug binary was not built"
  local config_home=$state_dir/config
  local evidence_dir=$state_dir/evidence
  cat >"$state_dir/run.sh" <<EOF
#!/usr/bin/env bash
export XDG_CONFIG_HOME=$(printf '%q' "$config_home")
exec $(printf '%q' "$binary") tui
EOF
  chmod 700 "$state_dir/run.sh"
  {
    printf 'session=%q\n' "$session"
    printf 'binary=%q\n' "$binary"
    printf 'config_home=%q\n' "$config_home"
    printf 'evidence_dir=%q\n' "$evidence_dir"
  } >"$state_dir/run.env"
  tmux new-session -d -s "$session" -x 120 -y 32 "$state_dir/run.sh"
  tmux set-option -t "$session" status off
  tmux set-window-option -t "$session" remain-on-exit on
  tmux resize-window -t "$session" -x 120 -y 32
  local attempt
  for attempt in $(seq 1 40); do
    if ready_now; then
      printf 'run_id=%s\n' "$run_id"
      printf 'session=%s\n' "$session"
      printf 'binary=%s\n' "$binary"
      printf 'evidence_dir=%s\n' "$evidence_dir"
      return 0
    fi
    sleep 0.5
  done
  capture_to "$evidence_dir/launch-failed.txt"
  die "TUI did not show a task count; see $evidence_dir/launch-failed.txt"
}

ready_now() {
  tmux has-session -t "$session" 2>/dev/null || return 1
  local pane
  pane=$(tmux capture-pane -p -t "$session") || return 1
  grep -q 'f find' <<<"$pane" || return 1
  grep -q 'm min' <<<"$pane" || return 1
  grep -Eq '[[:space:]][0-9]+ tasks$' <<<"$pane"
}

capture_to() {
  local path=$1
  tmux capture-pane -p -t "$session" >"$path"
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
  local pane
  pane=$(tmux capture-pane -p -t "$session")
  grep -q 'f find' <<<"$pane" || die "hint line is missing"
  grep -q 'm min' <<<"$pane" || die "duration hint is missing"
  grep -Eq '[[:space:]][0-9]+ tasks$' <<<"$pane" || die "task count is not on screen"
  printf 'session=%s\n' "$session"
  printf 'command=structured\n'
  printf 'binary=%s\n' "$binary"
  printf 'hint=ok\n'
  printf 'status=ready\n'
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
      Escape | Enter | Up | Down | Left | Right | BSpace | Tab | Space | C-d | C-u | C-g | C-y | C-e)
        tmux send-keys -t "$session" "$key"
        ;;
      '?')
        tmux send-keys -t "$session" "?"
        ;;
      *)
        [[ ${#key} -eq 1 ]] || die "unknown key $key"
        tmux send-keys -t "$session" -l "$key"
        ;;
    esac
    sleep 0.12
  done
}

capture() {
  local run_id=$1 name=$2
  [[ "$name" =~ ^[A-Za-z0-9._-]+$ ]] || die "capture name must be letters, numbers, dots, or dashes"
  need_run "$run_id"
  tmux has-session -t "$session" 2>/dev/null || die "session $session is not running"
  local path=$evidence_dir/$name.txt
  capture_to "$path"
  printf 'evidence=%s\n' "$path"
  printf 'bytes=%s\n' "$(wc -c <"$path" | tr -d ' ')"
}

cleanup() {
  need_run "$1"
  if tmux has-session -t "$session" 2>/dev/null; then
    tmux kill-session -t "$session"
  fi
  printf 'stopped=%s\n' "$session"
  printf 'evidence_dir=%s\n' "$evidence_dir"
}

usage() {
  die "usage: verify.sh launch|doctor|keys|capture|cleanup <run-id> ..."
}

[[ $# -ge 2 ]] || usage
action=$1
shift
case "$action" in
  launch) launch "$1" ;;
  doctor) doctor "$1" ;;
  keys) keys "$@" ;;
  capture) [[ $# -eq 2 ]] || usage; capture "$1" "$2" ;;
  cleanup) cleanup "$1" ;;
  *) usage ;;
esac
