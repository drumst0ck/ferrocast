#!/bin/bash
# Usage: scripts/check-universal.sh <App.app>
# Fails if any Mach-O file in the bundle lacks arm64 or x86_64 (the release must run on both).
set -euo pipefail
app=${1:?usage: check-universal.sh <App.app>}
[ -d "$app" ] || { echo "FAIL: $app is not a directory" >&2; exit 1; }
bad=0 count=0
while IFS= read -r -d '' f; do
  case "$(file -b "$f")" in *Mach-O*) ;; *) continue ;; esac
  count=$((count + 1))
  archs=$(lipo -archs "$f" 2>/dev/null || echo "unreadable")
  if [[ " $archs " == *" arm64 "* && " $archs " == *" x86_64 "* ]]; then
    echo "ok   $archs  ${f#"$app"/}"
  else
    echo "FAIL $archs  ${f#"$app"/} (needs arm64 and x86_64)" >&2; bad=1
  fi
done < <(find "$app" -type f -print0)
[ "$count" -gt 0 ] || { echo "FAIL: no Mach-O files found in $app" >&2; exit 1; }
[ "$bad" -eq 0 ] || exit 1
echo "universal: $count Mach-O files, all arm64 + x86_64"
