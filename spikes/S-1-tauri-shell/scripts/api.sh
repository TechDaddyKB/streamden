#!/usr/bin/env bash
# POST a test-control action. Usage: api.sh ACTION [QUERY]
unset LD_LIBRARY_PATH
curl -s -m 10 -X POST "http://127.0.0.1:${S1_PORT:-47811}/api/test/$1${2:+?$2}"
echo
