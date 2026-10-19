#!/usr/bin/env bash
# Crash-loop fallback proof: start with a clean state dir and no --compat-level,
# SIGKILL the app before it reports first paint, repeat. Each restart must move
# one level up (0 -> 1 -> 2 -> 3 -> 4) and then fall back to the browser UI.
# Usage: crashloop.sh STATE_DIR [KILL_AFTER_S=0.1]
set -uo pipefail
unset LD_LIBRARY_PATH
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/.." && pwd)
state=$(realpath -m "$1"); after=${2:-0.1}
rm -rf "$state"; mkdir -p "$state"
for i in 1 2 3 4 5; do
  r="$root/runs/crashloop/run$i"
  pid=$("$here/launch.sh" "$r" -- --state-dir "$state" --test-control --no-open-browser)
  sleep "$after"
  kids=$(ps --ppid "$pid" -o pid= | tr '\n' ' ')
  kill -9 "$pid"
  sleep 1
  lvl=$(jq -c --argjson p "$pid" 'select(.pid==$p and .event=="startup") | {level, level_name, source}' "$state/events.jsonl")
  painted=$(jq -c --argjson p "$pid" 'select(.pid==$p and .event=="first_paint")' "$state/events.jsonl" | wc -l)
  orphans=0; for k in $kids; do kill -0 "$k" 2>/dev/null && orphans=$((orphans+1)); done
  echo "run $i pid=$pid killed after ${after}s: $lvl first_paint_events=$painted marker=$([[ -f $state/starting.json ]] && echo present || echo absent) orphaned_children=$orphans"
done
# Run 6: should come up in browser-fallback mode (no webview at all).
r="$root/runs/crashloop/run6"
pid=$("$here/launch.sh" "$r" -- --state-dir "$state" --test-control --no-open-browser)
sleep 2
echo "run 6 pid=$pid: $(jq -c --argjson p "$pid" 'select(.pid==$p and (.event=="startup" or .event=="browser_fallback" or .event=="crash_loop_bump"))' "$state/events.jsonl" | tr '\n' ' ')"
echo "run 6 windows: $("$here/win.sh" "$pid" | wc -l), children: $(ps --ppid "$pid" -o comm= | tr '\n' ' ')"
echo "run 6 GET / -> $(curl -s -o /dev/null -w '%{http_code} %{content_type}' http://127.0.0.1:47811/)"
echo "run 6 GET /api/state -> $(curl -s http://127.0.0.1:47811/api/state | jq -c '{level, level_name, browser_fallback, ticks}')"
echo "run 6 SSE (3 s): $(timeout 3 curl -s -N http://127.0.0.1:47811/api/ticks | grep -c '^data:') tick events"
echo "run 6 Host guard: evil Host -> $(curl -s -o /dev/null -w '%{http_code}' -H 'Host: evil.example:47811' http://127.0.0.1:47811/api/state), cross-origin POST -> $(curl -s -o /dev/null -w '%{http_code}' -X POST -H 'Origin: http://evil.example' http://127.0.0.1:47811/api/test/info)"
echo "run 6 tray: $("$here/tray.sh" "$pid" | jq -c '{status, labels: [.labels[].label]}')"
"$here/api.sh" quit >/dev/null 2>&1; sleep 1.5; kill "$pid" 2>/dev/null
echo "level file now: $(cat "$state/compat-level")"
