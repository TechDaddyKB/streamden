#!/usr/bin/env bash
# Which GPU does each process of the app tree use? Prints one JSON line per
# process: open DRM render nodes (with their kernel driver), whether it holds
# /dev/nvidia* (NVIDIA's own GL/EGL stack), and how many llvmpipe threads it
# runs (Mesa software rendering). Usage: gpu.sh PID
set -uo pipefail
root=$1
tree() { echo "$1"; for c in $(ps --ppid "$1" -o pid=); do tree "$c"; done; }
for p in $(tree "$root"); do
  comm=$(cat /proc/$p/comm 2>/dev/null) || continue
  nodes=$(for f in /proc/$p/fd/*; do readlink "$f"; done 2>/dev/null | grep -o '/dev/dri/renderD[0-9]*' | sort -u |
    while read -r n; do echo "$n=$(basename "$(readlink -f /sys/class/drm/${n##*/}/device/driver)")"; done | paste -sd, -)
  nv=$(for f in /proc/$p/fd/*; do readlink "$f"; done 2>/dev/null | grep -c '^/dev/nvidia')
  llvm=$(grep -l '^llvmpipe' /proc/$p/task/*/comm 2>/dev/null | wc -l)
  jq -n -c --argjson pid "$p" --arg comm "$comm" --arg nodes "$nodes" --argjson nv "$nv" --argjson llvm "$llvm" \
    '{pid:$pid, comm:$comm, render_nodes:$nodes, nvidia_fds:$nv, llvmpipe_threads:$llvm}'
done
