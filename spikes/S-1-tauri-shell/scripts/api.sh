#!/usr/bin/env bash
# POST a test-control action to one instance. Usage: api.sh ACTION [QUERY]
# Needs S1_ADDR (host:port from instance.sh). With S1_PID set, first checks that
# the address still belongs to that PID. Fails on HTTP errors.
set -uo pipefail
unset LD_LIBRARY_PATH
: "${S1_ADDR:?set S1_ADDR (see scripts/instance.sh)}"
if [[ -n ${S1_PID:-} ]]; then
  got=$(curl -fsS -m 5 "http://$S1_ADDR/api/state" | jq -r .pid) || exit 1
  [[ $got == "$S1_PID" ]] || { echo "$S1_ADDR answers as pid $got, not $S1_PID" >&2; exit 1; }
fi
curl -fsS -m 10 -X POST "http://$S1_ADDR/api/test/$1${2:+?$2}" || exit 1
echo
