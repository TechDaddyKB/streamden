#!/usr/bin/env bash
# Kill the spike server, keep it down for <down-seconds>, restart it, and measure how long the OBS overlay
# takes to reconnect. Usage: restart-test.sh <down-seconds> <label>
set -euo pipefail
DOWN=${1:?down seconds}
LABEL=${2:?label}
PORT=${PORT:-47800}
DATA=${DATA:-/tmp/den-s2/data}
BIN=${BIN:-$(dirname "$0")/../target/release/s2-obs-overlays}
PIDFILE=${PIDFILE:-/tmp/den-s2/server.pid}
LOGDIR=${LOGDIR:-/tmp/den-s2/logs}
unset LD_LIBRARY_PATH

pkill -x s2-obs-overlays 2>/dev/null || true
for _ in $(seq 1 100); do curl -fsS "http://127.0.0.1:$PORT/healthz" >/dev/null 2>&1 || break; sleep 0.05; done
sleep "$DOWN"
nohup "$BIN" --port "$PORT" --data "$DATA" ${WIDGET_PORTS:+--widget-ports "$WIDGET_PORTS"} >"$LOGDIR/server-$LABEL.log" 2>&1 &
echo $! >"$PIDFILE"
for _ in $(seq 1 100); do curl -fsS "http://127.0.0.1:$PORT/healthz" >/dev/null 2>&1 && break; sleep 0.05; done
SEQ=$(curl -sS "http://127.0.0.1:$PORT/report" | jq '[.[] | select(.kind=="server.listening")] | last | .seq')
for _ in $(seq 1 120); do
  R=$(curl -sS "http://127.0.0.1:$PORT/report?since=$((SEQ - 1))")
  HELLO=$(echo "$R" | jq -c '[.[] | select(.kind=="ws.client" and .data.msg.type=="hello" and .data.client=="overlay")] | first // empty')
  if [ -n "$HELLO" ]; then
    echo "$R" | jq -c --arg label "$LABEL" --argjson down "$DOWN" '
      (map(select(.kind=="server.listening")) | first | .ts_ms) as $up
      | (map(select(.kind=="ws.client" and .data.msg.type=="hello" and .data.client=="overlay")) | first) as $h
      | {label: $label, down_s: $down, recovery_after_listen_ms: ($h.ts_ms - $up),
         client_outage_ms: $h.data.msg.outage_ms, backoff_attempts: $h.data.msg.attempts}'
    exit 0
  fi
  sleep 0.25
done
echo "{\"label\":\"$LABEL\",\"error\":\"overlay did not reconnect within 30 s\"}"
exit 1
