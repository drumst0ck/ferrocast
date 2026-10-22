#!/bin/bash
# Render the DMG window background from resources/dmg/background.html into
# resources/dmg/background.png (660x400) and background@2x.png (1320x800); dmgbuild (scripts/dmg.sh)
# picks up the @2x file next to the PNG for Retina screens. Dev aid: the rendered files are
# committed, so releases do not need a browser.
#   scripts/dmg_background.sh            (needs Google Chrome, Chromium or Brave)
set -euo pipefail
cd "$(dirname "$0")/.."
browser=""
for b in "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
         "/Applications/Chromium.app/Contents/MacOS/Chromium" \
         "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser"; do
  [ -x "$b" ] && { browser=$b; break; }
done
[ -n "$browser" ] || { echo "no Chrome, Chromium or Brave found" >&2; exit 1; }
port=8766
python3 -m http.server "$port" --bind 127.0.0.1 >/dev/null 2>&1 &
server=$!
disown "$server"
profile=$(mktemp -d)
trap 'kill $server 2>/dev/null; rm -rf "$profile"' EXIT
for _ in $(seq 1 20); do curl -sf -o /dev/null "http://127.0.0.1:$port/resources/dmg/background.html" && break; sleep 0.2; done
url="http://127.0.0.1:$port/resources/dmg/background.html"
for scale in 1 2; do
  out="$PWD/resources/dmg/background.png"
  [ "$scale" = 2 ] && out="$PWD/resources/dmg/background@2x.png"
  "$browser" --headless=new --disable-gpu --hide-scrollbars --user-data-dir="$profile" \
    --window-size=660,400 --force-device-scale-factor="$scale" --virtual-time-budget=3000 \
    --screenshot="$out" "$url" >/dev/null 2>&1
done
sips -g pixelWidth -g pixelHeight resources/dmg/background.png resources/dmg/background@2x.png | grep pixel
echo "background ready: resources/dmg/background.png (+ @2x)"
