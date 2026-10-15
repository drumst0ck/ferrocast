#!/bin/bash
# Dev aid: render the launcher to a PNG without screen-recording permission.
# usage: scripts/shot.sh out.png [VIEW=settings] [QUERY=text]
out=$1; shift
export FERROCAST_DATA_DIR=${FERROCAST_DATA_DIR:-$(mktemp -d)}
rm -f "$out"
env "$@" FERROCAST_SCREENSHOT="$out" "$(dirname "$0")/../target/debug/ferrocast" >/dev/null 2>&1 &
pid=$!
for _ in $(seq 1 40); do kill -0 $pid 2>/dev/null || break; sleep 0.5; done
kill $pid 2>/dev/null
[ -s "$out" ] && echo "$out" || { echo "no screenshot" >&2; exit 1; }
