#!/usr/bin/env bash
# Records the README demo against the fixture server and drives the TUI segment with tmux keys.
set -euo pipefail
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=$(CDPATH= cd -- "$here/../../../.." && pwd)
out=${STRUCTURED_DEMO_DIR:-${TMPDIR:-/tmp}/structured-demo}
session=structured-demo
if [[ -x "$HOME/.cargo/bin/cargo" ]]; then
  export PATH="$HOME/.cargo/bin:$PATH"
fi
for tool in asciinema tmux jq python3 cargo; do command -v "$tool" >/dev/null || { echo "missing $tool" >&2; exit 1; }; done
tmux has-session -t "$session" 2>/dev/null && { echo "tmux session $session already exists" >&2; exit 1; }
(cd "$repo" && cargo build --quiet)
rm -rf "$out" && mkdir -p "$out/cfg/structured"
printf '{"client_id":"f","access_token":"f","refresh_token":"","expires_at":4102444800}' >"$out/cfg/structured/auth.json"
chmod 600 "$out/cfg/structured/auth.json"
printf 'timezones = ["Saigon=Asia/Ho_Chi_Minh", "ET=America/New_York"]\n' >"$out/cfg/structured/config.toml"
TZ=Asia/Ho_Chi_Minh python3 -I "$here/fixture-server.py" "$out/port" "$out/calls.jsonl" >/dev/null 2>&1 &
fixture=$!
trap 'kill $fixture 2>/dev/null || true; tmux kill-session -t $session 2>/dev/null || true' EXIT
for _ in $(seq 1 30); do [[ -s "$out/port" ]] && break; sleep 0.1; done
tmux new-session -d -s "$session" -x 120 -y 32 \
  "asciinema rec '$out/demo.cast' --overwrite --quiet --window-size 120x32 --idle-time-limit 2 \
   --title 'structured: CLI and TUI for the Structured day planner' \
   --command 'bash $here/demo-session.sh $out $repo/target/debug'"
tmux set-option -t "$session" status off >/dev/null
k() { for key in "$@"; do if [[ ${#key} -eq 1 ]]; then tmux send-keys -t "$session" -l "$key"; else tmux send-keys -t "$session" "$key"; fi; sleep 0.28; done; }
lit() { local s=$1 i; for ((i = 0; i < ${#s}; i++)); do tmux send-keys -t "$session" -l "${s:i:1}"; sleep 0.07; done; }
for _ in $(seq 1 600); do tmux capture-pane -p -t "$session" 2>/dev/null | grep -q '? help' && break; sleep 0.1; done
sleep 2.5
k j j j j j j; sleep 1.2
k Enter; sleep 1.5
k j x; sleep 1.4
k a; lit "Share the notes"; sleep 0.6; k Enter; sleep 1; k Escape; sleep 1.5
k J; sleep 1.2
k h; sleep 0.8
k /; lit "backfill"; k Enter; sleep 2
k Escape; sleep 0.8
k g g j T; sleep 2.2
k Escape D; sleep 1.8
k Escape; sleep 0.6
k w; sleep 2.4; k z; sleep 2.8; k z; sleep 1.6; k w; sleep 1.2
k '?'; sleep 3.2
k Escape; sleep 0.6
k i; sleep 2.2
k t; sleep 0.8
k ']'; sleep 2.2
k t; sleep 1
k q
for _ in $(seq 1 200); do tmux has-session -t "$session" 2>/dev/null || break; sleep 0.1; done
python3 - "$out/demo.cast" <<'PY'
import json, sys
path = sys.argv[1]
lines = open(path).read().splitlines()
header = json.loads(lines[0])
header["command"] = "bash demo-session.sh"
lines[0] = json.dumps(header)
blob = "\n".join(lines)
assert not any(leak in blob for leak in ("/Users/", "/home/", "access_token", "127.0.0.1")), "private data in the cast"
open(path, "w").write(blob + "\n")
PY
if command -v agg >/dev/null; then
  agg --font-size 14 --idle-time-limit 2 --fps-cap 15 --last-frame-duration 3 --theme github-dark \
    "$out/demo.cast" "$repo/docs/assets/demo.gif" >/dev/null 2>&1
  echo "gif=$repo/docs/assets/demo.gif"
fi
echo "cast=$out/demo.cast"
