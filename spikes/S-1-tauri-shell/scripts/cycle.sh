#!/usr/bin/env bash
# Close the main webview to tray and recreate it N times via the tray menu,
# printing the app's child processes and the reopen latency each time.
# Usage: cycle.sh PID STATE_DIR [N=3]
set -uo pipefail
unset LD_LIBRARY_PATH
here=$(cd "$(dirname "$0")" && pwd)
pid=$1 dir=$2 n=${3:-3}
kids() { ps --ppid "$pid" -o comm= | sort | uniq -c | awk '{printf "%s×%s ", $2, $1}'; }
echo "start: $(kids)"
for i in $(seq 1 "$n"); do
  "$here/tray.sh" "$pid" 'Hide to tray' >/dev/null
  sleep 2
  ticks=$(tail -1 "$dir/engine.log")
  echo "cycle $i closed:   $(kids) | engine: ${ticks#* }"
  t=$(date +%s%3N)
  "$here/tray.sh" "$pid" '^Show$' >/dev/null
  fp=$("$here/wait-event.sh" "$dir" "$pid" first_paint 15 ".label==\"main\" and .t_unix_ms >= $t" | jq -r '.since_open_ms|floor')
  sleep 1
  echo "cycle $i reopened: $(kids) | reopen-to-first-paint ${fp} ms"
done
