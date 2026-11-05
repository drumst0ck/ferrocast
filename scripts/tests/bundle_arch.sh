#!/bin/bash
# Structural check, no build: every Swift helper bundle.sh compiles must target the host arch, like
# the app and the other helpers do, never a hard-coded one (an arm64 helper breaks Intel source builds).
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
fail() { echo "FAIL: $*" >&2; exit 1; }
grep -n 'swiftc' "$root/scripts/bundle.sh" | grep -q -- '-target arm64' && fail "bundle.sh hard-codes -target arm64 for a helper"
grep -n 'swiftc' "$root/scripts/bundle.sh" | grep -q -- '-target "$(uname -m)-apple-macos13.0"' || fail "build_swift_helper does not target the host arch"
echo "bundle arch: ok"
