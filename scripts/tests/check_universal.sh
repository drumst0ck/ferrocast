#!/bin/bash
# Behaviour check, no build: scripts/check-universal.sh must pass an app whose every Mach-O is
# arm64 + x86_64 and fail (naming the file) on a thin one, wherever it sits in the bundle.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
check="$root/scripts/check-universal.sh"
fail() { echo "FAIL: $*" >&2; exit 1; }
[ -x "$check" ] || fail "scripts/check-universal.sh missing or not executable"
tmp=$(mktemp -d); trap 'find "$tmp" -mindepth 1 -delete 2>/dev/null; rmdir "$tmp" 2>/dev/null || true' EXIT
echo 'int main(void){return 0;}' > "$tmp/t.c"
cc -arch arm64 -o "$tmp/arm64" "$tmp/t.c"
cc -arch x86_64 -o "$tmp/x86_64" "$tmp/t.c"
lipo -create "$tmp/arm64" "$tmp/x86_64" -output "$tmp/fat"
mkapp() { # <dir> <app binary> <helper binary>
  mkdir -p "$1/A.app/Contents/MacOS" "$1/A.app/Contents/Frameworks/F.framework/Versions/A" "$1/A.app/Contents/Resources"
  cp "$2" "$1/A.app/Contents/MacOS/app"; cp "$3" "$1/A.app/Contents/MacOS/helper"
  cp "$3" "$1/A.app/Contents/Frameworks/F.framework/Versions/A/F"
  echo text > "$1/A.app/Contents/Resources/x.pl"
}
mkapp "$tmp/ok" "$tmp/fat" "$tmp/fat"
"$check" "$tmp/ok/A.app" >/dev/null || fail "universal app rejected"
for thin in arm64 x86_64; do
  mkapp "$tmp/bad-$thin" "$tmp/fat" "$tmp/$thin"
  out=$("$check" "$tmp/bad-$thin/A.app" 2>&1) && fail "thin $thin helper accepted"
  echo "$out" | grep -q "Contents/MacOS/helper" || fail "output does not name the thin helper: $out"
done
mkapp "$tmp/thinapp" "$tmp/arm64" "$tmp/fat"
"$check" "$tmp/thinapp/A.app" >/dev/null 2>&1 && fail "thin app binary accepted"
"$check" "$tmp/nope" >/dev/null 2>&1 && fail "missing app accepted"
echo "check-universal: ok"
