#!/usr/bin/env bash
# Launch the app N times with fresh state and append one JSON line per launch
# to OUT: env and args used, first paint, whether it is still alive after 1 s,
# exec->Error 71 time, the leftover crash marker and the filtered stderr.
# Usage: repro.sh OUT.jsonl TAG N [VAR=value ...] -- [app args ...]
set -uo pipefail
unset LD_LIBRARY_PATH
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/.." && pwd)
out=$(realpath -m "$1") tag=$2 n=$3; shift 3
envs=(); while [[ $# -gt 0 && $1 != -- ]]; do envs+=("$1"); shift; done
[[ ${1:-} == -- ]] && shift
for i in $(seq 1 "$n"); do
  r="$root/runs/repro/$tag/run$i"; rm -rf "$r"
  pid=$("$here/launch.sh" "$r" "${envs[@]}" -- "$@" --state-dir "$r/state" --test-control --no-open-browser)
  fp=$("$here/wait-event.sh" "$r/state" "$pid" first_paint 10 2>/dev/null) || fp=
  sleep 1
  alive=$(kill -0 "$pid" 2>/dev/null && echo true || echo false)
  launch=$(cat "$r/launch_ms")
  e71=$(grep -m1 'Error 71' "$r/stderr.log" | grep -o '[0-9:]\{8\}\.[0-9]\{3\}')
  e71_ms=null; [[ -n $e71 ]] && e71_ms=$(( $(date -d "$(date -d "@$((launch/1000))" +%F) $e71" +%s%3N) - launch ))
  jq -n -c --arg tag "$tag" --argjson run "$i" --argjson pid "$pid" --argjson tiled "$([[ ${S1_TILED:-0} == 1 ]] && echo true || echo false)" --arg env "${envs[*]:-}" --arg args "$*" \
    --argjson fp "$(jq -c '.process_ms' <<<"${fp:-null}" 2>/dev/null || echo null)" \
    --argjson exec_paint "$( [[ -n $fp ]] && echo $(( $(jq -r '.t_unix_ms|floor' <<<"$fp") - launch )) || echo null)" \
    --argjson alive "$alive" --argjson e71 "$e71_ms" \
    --argjson marker "$([[ -f $r/state/starting.json ]] && echo true || echo false)" \
    --arg stderr "$(grep -v '^pci id for fd\|^$' "$r/stderr.log")" \
    '{tag:$tag, run:$run, pid:$pid, launcher_tiled:$tiled, env:$env, args:$args, first_paint_process_ms:$fp, exec_to_paint_ms:$exec_paint,
      alive_after_1s:$alive, exec_to_error71_ms:$e71, starting_marker_left:$marker, stderr:$stderr}' | tee -a "$out"
  kill "$pid" 2>/dev/null; sleep 1; kill -9 "$pid" 2>/dev/null
done
