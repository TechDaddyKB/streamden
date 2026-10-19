#!/usr/bin/env bash
# Launch the spike through Hyprland (so it gets the real session env, not this
# shell's) as a floating, opaque 1000x700 window on the active workspace.
# Usage: launch.sh RUN_DIR [VAR=value ...] -- [app args ...]   (prints the PID)
# Set S1_TILED=1 to skip the float/size rules.
set -euo pipefail
unset LD_LIBRARY_PATH
here=$(cd "$(dirname "$0")" && pwd)
run=$(realpath -m "$1"); shift
rm -f "$run/pid"
mkdir -p "$run"
cmd="$here/app.sh$(printf ' %q' "$run" "$@")"
if [[ ${S1_TILED:-0} == 1 ]]; then
  rules='{ opacity = "1 1" }'
else
  rules='{ float = true, size = "1000 700", center = true, opacity = "1 1" }'
fi
hyprctl dispatch "hl.dsp.exec_cmd([[$cmd]], $rules)" >/dev/null
for _ in $(seq 1 100); do [[ -s $run/pid ]] && break; sleep 0.05; done
cat "$run/pid"
