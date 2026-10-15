#!/bin/bash
# Build dist/Ferrocast-<version>.dmg from dist/Ferrocast.app (run scripts/bundle.sh first, or pass --build).
set -euo pipefail
cd "$(dirname "$0")/.."
[ "${1:-}" = "--build" ] && scripts/bundle.sh
version=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
app=dist/Ferrocast.app
[ -d "$app" ] || { echo "dist/Ferrocast.app missing: run scripts/bundle.sh" >&2; exit 1; }
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
cp -R "$app" "$stage/"
ln -s /Applications "$stage/Applications"
dmg="dist/Ferrocast-${version}.dmg"
rm -f "$dmg"
hdiutil create -volname "Ferrocast" -srcfolder "$stage" -ov -format UDZO "$dmg" >/dev/null
hdiutil verify "$dmg" >/dev/null
echo "DMG ready: $PWD/$dmg"
