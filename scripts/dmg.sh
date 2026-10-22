#!/bin/bash
# Build dist/Ferrocast-<version>.dmg from dist/Ferrocast.app (run scripts/bundle.sh first, or pass --build).
# With dmgbuild installed (pip install dmgbuild==1.6.7, or DMGBUILD=/path/to/dmgbuild) the image opens
# as a designed window: background resources/dmg/background.png (+ @2x), the app on the left and an
# Applications link on the right (resources/dmg/settings.py). Without it, a plain image is built.
set -euo pipefail
cd "$(dirname "$0")/.."
[ "${1:-}" = "--build" ] && scripts/bundle.sh
version=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
app=dist/Ferrocast.app
[ -d "$app" ] || { echo "dist/Ferrocast.app missing: run scripts/bundle.sh" >&2; exit 1; }
dmg="dist/Ferrocast-${version}.dmg"
rm -f "$dmg"
dmgbuild=${DMGBUILD:-$(command -v dmgbuild || true)}
# releases must ship the designed window: REQUIRE_DESIGNED_DMG=1 turns the plain fallback into an error
if [ -z "$dmgbuild" ] && [ "${REQUIRE_DESIGNED_DMG:-}" = 1 ]; then
  echo "dmgbuild not found and REQUIRE_DESIGNED_DMG=1" >&2
  exit 1
fi
if [ -n "$dmgbuild" ]; then
  "$dmgbuild" -s resources/dmg/settings.py \
    -D app="$app" -D background=resources/dmg/background.png -D volume_icon=resources/Ferrocast.icns \
    "Ferrocast" "$dmg" >/dev/null
  echo "designed DMG (dmgbuild)"
else
  echo "dmgbuild not found: building a plain DMG (pip install dmgbuild==1.6.7 for the designed window)" >&2
  stage=$(mktemp -d)
  trap 'rm -rf "$stage"' EXIT
  cp -R "$app" "$stage/"
  ln -s /Applications "$stage/Applications"
  hdiutil create -volname "Ferrocast" -srcfolder "$stage" -ov -format UDZO "$dmg" >/dev/null
fi
hdiutil verify "$dmg" >/dev/null
echo "DMG ready: $PWD/$dmg"
