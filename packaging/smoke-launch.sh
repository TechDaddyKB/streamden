#!/usr/bin/env bash
# Container-only probe: isolated X server, software rendering, no host display.
set -euo pipefail
probe_dir=$(mktemp -d)
probe_log="$probe_dir/launch.log"
trap 'rm -rf "$probe_dir"' EXIT
set +e
env -u WAYLAND_DISPLAY GDK_BACKEND=x11 LIBGL_ALWAYS_SOFTWARE=1 \
  XDG_CONFIG_HOME="$probe_dir/config" XDG_CACHE_HOME="$probe_dir/cache" \
  timeout --signal=TERM --kill-after=5s 30s dbus-run-session -- xvfb-run -a bash -c '
    set -eu
    streamden &
    desktop_pid=$!
    sleep 15 &
    timer_pid=$!
    trap '\''kill "$desktop_pid" "$timer_pid" 2>/dev/null || true; wait 2>/dev/null || true'\'' EXIT
    # Wait on the actual desktop, not D-Bus/portal helpers that may outlive it.
    status=0
    wait -n -p finished "$desktop_pid" "$timer_pid" || status=$?
    if [[ ${finished:-} != "$timer_pid" || $status != 0 ]] || ! kill -0 "$desktop_pid" 2>/dev/null; then
      echo "Desktop exited before the 15-second launch probe: $status" >&2
      exit 1
    fi
  ' >"$probe_log" 2>&1
probe_status=$?
set -e
cat "$probe_log"
if [[ $probe_status != 0 ]]; then
  echo "Desktop launch probe failed: $probe_status" >&2
  exit 1
fi
echo 'Desktop stayed running for 15 seconds under Xvfb (rendering not asserted).'
