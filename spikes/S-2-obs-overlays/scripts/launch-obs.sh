#!/usr/bin/env bash
# Launch a second, isolated OBS instance for spike S-2.
# Usage: launch-obs.sh <root> <wayland|xcb> <logfile>
# Isolation: XDG_* dirs point into <root>; --multi allows a parallel instance next to the owner's OBS.
set -euo pipefail
ROOT=${1:?root dir}
QPA=${2:?wayland or xcb}
LOG=${3:?log file}
case "$ROOT" in /tmp/*) ;; *) echo "refusing: root must be under /tmp" >&2; exit 1 ;; esac
unset LD_LIBRARY_PATH
mkdir -p "$ROOT/cache" "$ROOT/data" "$ROOT/state"
XDG_CONFIG_HOME="$ROOT/config" XDG_CACHE_HOME="$ROOT/cache" XDG_DATA_HOME="$ROOT/data" XDG_STATE_HOME="$ROOT/state" \
QT_QPA_PLATFORM="$QPA" \
  setsid obs --multi --disable-shutdown-check --disable-missing-files-check >"$LOG" 2>&1 < /dev/null &
echo $!
