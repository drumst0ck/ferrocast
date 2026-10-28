#!/bin/bash
# Checks the Info.plist that scripts/bundle.sh writes, without building anything: the heredoc is
# cut out of the script, expanded with a made-up version and linted.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
fail() { echo "FAIL: $*" >&2; exit 1; }

body=$(awk '/<<PLIST$/{on=1; next} /^PLIST$/{on=0} on' "$root/scripts/bundle.sh")
[ -n "$body" ] || fail "no Info.plist heredoc found in bundle.sh"
version=9.9.9
plist=$(eval "cat <<PLIST_TEST
$body
PLIST_TEST")

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
echo "$plist" > "$tmp/Info.plist"
if command -v plutil >/dev/null 2>&1; then
  plutil -lint "$tmp/Info.plist" >/dev/null || fail "Info.plist does not lint"
fi

# the seven interface languages of the app (docs: Settings > General > Language)
for loc in en es de fr pt-BR ja zh-Hans; do
  echo "$plist" | grep -q "<string>$loc</string>" || fail "CFBundleLocalizations misses $loc"
done
echo "$plist" | grep -q "<key>CFBundleLocalizations</key>" || fail "no CFBundleLocalizations key"
echo "$plist" | grep -q "<key>CFBundleDevelopmentRegion</key><string>en</string>" || fail "development region is not en"
echo "$plist" | grep -q "<string>9.9.9</string>" || fail "the version is not in the plist"
# no locale beyond the seven
count=$(echo "$plist" | awk '/CFBundleLocalizations/{print}' | grep -o '<string>[^<]*</string>' | wc -l | tr -d ' ')
[ "$count" = "7" ] || fail "expected 7 localizations, found $count"
echo "bundle_plist: ok"
