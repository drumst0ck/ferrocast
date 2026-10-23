#!/bin/bash
# gclick.sh X Y: clicks only if $LIVE_APP is frontmost AND (X,Y) is inside one of its onscreen windows.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../../.." && pwd)"
[ $# -eq 2 ] || { echo "usage: gclick.sh X Y" >&2; exit 2; }
pat="${LIVE_APP:-ferrocast}"
front="$(lsappinfo info -only name "$(lsappinfo front)" 2>/dev/null | sed -n '1s/^"\([^"]*\)".*/\1/p')"
if ! printf '%s' "$front" | grep -qi -- "$pat"; then
  echo "REFUSED: front is $front"
  exit 3
fi
"$here/build.sh" >/dev/null
inside="$("$root/target/dev-live/winlist" "$pat" | awk -v x="$1" -v y="$2" '
  /onscreen=true/ {
    split($NF, b, "[=,]")  # bounds=X,Y,W,H
    if (x >= b[2] && x < b[2] + b[4] && y >= b[3] && y < b[3] + b[5]) { print "yes"; exit }
  }')"
if [ "$inside" != "yes" ]; then
  echo "REFUSED: ($1,$2) is not inside an onscreen $pat window"
  exit 3
fi
exec "$root/target/dev-live/poster" click "$1" "$2"
