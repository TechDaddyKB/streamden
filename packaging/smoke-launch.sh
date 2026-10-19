#!/usr/bin/env bash
# Container-only probe: isolated X server, software rendering, no host display.
set -euo pipefail
probe_dir=$(mktemp -d)
probe_log="$probe_dir/launch.log"
trap 'rm -rf "$probe_dir"' EXIT
set +e
env -u WAYLAND_DISPLAY GDK_BACKEND=x11 LIBGL_ALWAYS_SOFTWARE=1 \
  XDG_CONFIG_HOME="$probe_dir/config" XDG_CACHE_HOME="$probe_dir/cache" \
  timeout --signal=TERM 15s dbus-run-session -- xvfb-run -a streamden >"$probe_log" 2>&1
probe_status=$?
set -e
cat "$probe_log"
if [[ $probe_status != 124 ]]; then
  echo "Desktop exited before the 15-second launch probe: $probe_status" >&2
  exit 1
fi
if grep -Eiq 'error while loading shared libraries|symbol lookup error|panicked at|failed to open display|cannot open display|Failed to create.*(WebView|webview)|WebKitWebProcess.*(crash|died)' "$probe_log"; then
  echo 'Desktop launch diagnostics contain a startup failure' >&2
  exit 1
fi
echo 'Desktop stayed running for 15 seconds under Xvfb (rendering not asserted).'
