#!/usr/bin/env bash
# Resolve the HTTP address of ONE launched instance. The server falls back to an
# ephemeral port when the requested one is taken, so never assume --port: read
# the address from PID's own http_listening event, then check that /api/state
# answers with the same PID. Prints host:port.
# Usage: instance.sh STATE_DIR PID [TIMEOUT_S=10]
set -uo pipefail
unset LD_LIBRARY_PATH
here=$(cd "$(dirname "$0")" && pwd)
dir=$1 pid=$2 timeout=${3:-10}
ev=$("$here/wait-event.sh" "$dir" "$pid" http_listening "$timeout") || exit 1
addr=$(jq -r .addr <<<"$ev")
got=$(curl -fsS -m 5 "http://$addr/api/state" | jq -r .pid) || { echo "GET $addr/api/state failed" >&2; exit 1; }
[[ $got == "$pid" ]] || { echo "$addr answers as pid $got, not $pid" >&2; exit 1; }
echo "$addr"
