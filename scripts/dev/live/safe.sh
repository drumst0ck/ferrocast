#!/bin/bash
# safe.sh <poster args>: posts only while the frontmost app matches $LIVE_APP (default ferrocast).
# Mouse buttons are stricter: `click` goes through gclick.sh and `drag` must START inside an onscreen
# window of $LIVE_APP, so a gesture can never land on another app even while $LIVE_APP is frontmost.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../../.." && pwd)"
pat="${LIVE_APP:-ferrocast}"
front="$(lsappinfo info -only name "$(lsappinfo front)" 2>/dev/null | sed -n '1s/^"\([^"]*\)".*/\1/p')"
if [ "${1:-}" != "move" ] && ! printf '%s' "$front" | grep -qi -- "$pat"; then
  echo "REFUSED: front is $front"
  exit 3
fi
"$here/build.sh" >/dev/null
case "${1:-}" in
  click)
    exec "$here/gclick.sh" "${2:?x}" "${3:?y}" ;;
  drag)
    inside="$("$root/target/dev-live/winlist" "$pat" | awk -v x="${2:?x1}" -v y="${3:?y1}" '
      /onscreen=true/ {
        split($NF, b, "[=,]")
        if (x >= b[2] && x < b[2] + b[4] && y >= b[3] && y < b[3] + b[5]) { print "yes"; exit }
      }')"
    if [ "$inside" != "yes" ]; then
      echo "REFUSED: drag start (${2},${3}) is not inside an onscreen $pat window"
      exit 3
    fi ;;
esac
exec "$root/target/dev-live/poster" "$@"
