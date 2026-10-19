#!/usr/bin/env bash
# One row of the compat matrix. Launches the app N times (first-paint timing),
# keeps the last run for screenshot + stderr scan + idle/animation CPU, quits.
# Usage: matrix.sh TAG RUNS IDLE_S ANIM_S [VAR=value ...] -- [app args ...]
set -uo pipefail
unset LD_LIBRARY_PATH
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/.." && pwd)
tag=$1 runs=$2 idle=$3 anim=$4; shift 4
out="$root/runs/matrix/$tag"
rm -rf "$out"; mkdir -p "$out"
fps=() ; exec_fps=() ; status=ok
for i in $(seq 1 "$runs"); do
  r="$out/run$i"
  pid=$("$here/launch.sh" "$r" "$@" --state-dir "$r/state" --test-control)
  if ev=$("$here/wait-event.sh" "$r/state" "$pid" first_paint 20); then
    fps+=("$(jq -r '.process_ms|floor' <<<"$ev")")
    exec_fps+=("$(( $(jq -r .t_unix_ms <<<"$ev") - $(cat "$r/launch_ms") ))")
  else
    status="no-first-paint"
  fi
  if (( i < runs )); then
    "$here/api.sh" quit >/dev/null 2>&1; sleep 1.5
    kill "$pid" 2>/dev/null
  fi
done
sleep 2
win=$("$here/win.sh" "$pid" level | head -1)
shot=$("$here/shot.sh" "$pid" level "$out/$tag.png" 2>&1)
state=$(curl -s -m 5 "http://127.0.0.1:${S1_PORT:-47811}/api/state")
"$here/api.sh" animation on=1 >/dev/null; sleep 2
anim_json=$(python3 -I "$here/measure.py" "$pid" "$anim" 5)
"$here/api.sh" animation on=0 >/dev/null; sleep 3
idle_json=$(python3 -I "$here/measure.py" "$pid" "$idle" 5)
"$here/api.sh" quit >/dev/null 2>&1; sleep 1.5
kill "$pid" 2>/dev/null
# stderr: drop Mesa's per-fd "pci id" chatter, keep everything else, counted
errs=$(cat "$out"/run*/stderr.log | grep -v '^pci id for fd\|^\[s1\]\|^$' | sed -E 's/[0-9]{2}:[0-9]{2}:[0-9]{2}\.[0-9]+//; s/\(s1-tauri-shell:[0-9]+\)/(app)/; s/pci id for fd [0-9]+/pci id for fd N/' | sort | uniq -c | sort -rn | jq -R -s 'split("\n") | map(select(length>0) | ltrimstr(" ") )')
jq -n -c --arg tag "$tag" --arg status "$status" --argjson fps "$(printf '%s\n' "${fps[@]:-}" | jq -s -c 'map(select(.!=null))')" \
  --argjson exec_fps "$(printf '%s\n' "${exec_fps[@]:-}" | jq -s -c 'map(select(.!=null))')" \
  --arg shot "$shot" --argjson win "${win:-null}" --argjson state "${state:-null}" \
  --argjson anim "$anim_json" --argjson idle "$idle_json" --argjson stderr "$errs" \
  '{tag:$tag,status:$status,first_paint_process_ms:$fps,first_paint_exec_ms:$exec_fps,shot:$shot,window:$win,
    quirk:$state.quirk_status,env:$state.env_applied,session:$state.session,anim:$anim,idle:$idle,stderr:$stderr}' \
  | tee -a "$root/runs/matrix/results.jsonl"
