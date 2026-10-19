#!/usr/bin/env bash
# Started by Hyprland (via launch.sh) so the app gets the real session env,
# not the shell's. Usage: app.sh RUN_DIR [VAR=value ...] -- [app args ...]
set -euo pipefail
here=$(cd "$(dirname "$0")/.." && pwd)
bin="$here/src-tauri/target/release/s1-tauri-shell"
run=$1; shift
envs=()
while [[ $# -gt 0 && $1 != -- ]]; do envs+=("$1"); shift; done
[[ ${1:-} == -- ]] && shift
mkdir -p "$run"
date +%s%3N > "$run/launch_ms"
echo $$ > "$run/pid"
exec env "${envs[@]}" "$bin" "$@" >"$run/stdout.log" 2>"$run/stderr.log"
