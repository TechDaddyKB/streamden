#!/usr/bin/env bash
# Control-origin regression (DEN-3 review P2): widget, foreign and null origins must get 403 from /emit and
# /api/chat without publishing an event; the app origin and Origin-less CLI clients must still work.
# Starts its own server on a throwaway port and data dir under /tmp. Needs curl and jq, and a current
# `cargo build --release` (it runs the existing binary). Usage: origin-check.sh
# Set CHROME=<chromium binary> to also drive a real headless Chromium (driver/origin-browser.mjs) with a
# throwaway profile under the same /tmp dir.
set -euo pipefail
PORT=${PORT:-47931}
BIN=${BIN:-$(dirname "$0")/../target/release/s2-obs-overlays}
DATA=$(mktemp -d /tmp/den-s2-origin.XXXXXX)
unset LD_LIBRARY_PATH

"$BIN" --port "$PORT" --data "$DATA" >"$DATA/server.log" 2>&1 &
PID=$!
trap 'kill "$PID" 2>/dev/null || true; wait "$PID" 2>/dev/null || true; rm -rf "$DATA"' EXIT
for _ in $(seq 1 100); do curl -fsS "http://127.0.0.1:$PORT/healthz" >/dev/null 2>&1 && break; sleep 0.05; done

APP="http://127.0.0.1:$PORT"
FAIL=0
events() { [ -f "$DATA/events.jsonl" ] && wc -l <"$DATA/events.jsonl" || echo 0; }
# check <expected-status> <path> <label> [curl header args...]
check() {
  local want=$1 path=$2 label=$3; shift 3
  local before after got
  before=$(events)
  got=$(curl -sS -o /dev/null -w '%{http_code}' -X POST -H 'content-type: text/plain' "$@" \
    --data '{"text":"origin-check '"$label"'"}' "$APP$path")
  after=$(events)
  local emitted=$((after - before)) want_emitted=0
  [ "$want" = 200 ] && want_emitted=1
  if [ "$got" = "$want" ] && [ "$emitted" = "$want_emitted" ]; then
    echo "ok   $path $label -> $got, events +$emitted"
  else
    echo "FAIL $path $label -> $got (want $want), events +$emitted (want +$want_emitted)"
    FAIL=1
  fi
}

for path in /emit /api/chat; do
  check 200 "$path" "cli-no-origin"
  check 200 "$path" "app-127" -H "Origin: $APP" -H 'Sec-Fetch-Site: same-origin'
  check 200 "$path" "app-localhost" -H "Origin: http://localhost:$PORT"
  check 403 "$path" "widget-origin" -H "Origin: http://w-a.localhost:$PORT" -H 'Sec-Fetch-Site: cross-site'
  check 403 "$path" "widget-origin-no-fetch-meta" -H "Origin: http://w-a.localhost:$PORT"
  check 403 "$path" "foreign-origin" -H 'Origin: https://evil.invalid'
  check 403 "$path" "null-origin" -H 'Origin: null'
  check 403 "$path" "cross-site-no-origin" -H 'Sec-Fetch-Site: cross-site'
  check 403 "$path" "widget-host" -H "Host: w-a.localhost:$PORT"
done
REJECTED=$(curl -sS "$APP/report" | jq '[.[] | select(.kind=="server.control_rejected")] | length')
echo "server.control_rejected log entries: $REJECTED (want 12)"
[ "$REJECTED" = 12 ] || FAIL=1

if [ -n "${CHROME:-}" ]; then
  node "$(dirname "$0")/../driver/origin-browser.mjs" "$PORT" "$CHROME" "$DATA/chrome" || FAIL=1
  WIDGET_EVENTS=$(grep -c 'origin-browser widget' "$DATA/events.jsonl" || true)
  DOCK_EVENTS=$(grep -c 'origin-browser dock' "$DATA/events.jsonl" || true)
  WIDGET_REJECTED=$(curl -sS "$APP/report" | jq --arg o "http://w-a.localhost:$PORT" \
    '[.[] | select(.kind=="server.control_rejected" and .data.headers.origin==$o)] | length')
  echo "browser: widget events $WIDGET_EVENTS (want 0), widget rejections $((WIDGET_REJECTED - 4)) (want 2), dock events $DOCK_EVENTS (want 2)"
  [ "$WIDGET_EVENTS" = 0 ] && [ "$((WIDGET_REJECTED - 4))" = 2 ] && [ "$DOCK_EVENTS" = 2 ] || FAIL=1
fi
[ "$FAIL" = 0 ] && echo "origin-check: PASS" || echo "origin-check: FAIL"
exit "$FAIL"
