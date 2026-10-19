#!/usr/bin/env bash
# Wait until STATE_DIR/events.jsonl has an event NAME from PID. Prints the line.
# Usage: wait-event.sh STATE_DIR PID NAME [TIMEOUT_S] [EXTRA_JQ_FILTER]
set -uo pipefail
dir=$1 pid=$2 name=$3 timeout=${4:-20} extra=${5:-true}
end=$(( $(date +%s) + timeout ))
while (( $(date +%s) < end )); do
  if [[ -f $dir/events.jsonl ]]; then
    line=$(jq -c --argjson pid "$pid" --arg n "$name" "select(.pid==\$pid and .event==\$n) | select($extra)" "$dir/events.jsonl" 2>/dev/null | tail -1)
    [[ -n $line ]] && { echo "$line"; exit 0; }
  fi
  sleep 0.1
done
echo "timeout waiting for $name from $pid" >&2
exit 1
