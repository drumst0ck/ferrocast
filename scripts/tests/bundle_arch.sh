#!/bin/bash
# Structural check, no build: every Swift helper bundle.sh compiles must target the archs list (host
# only, or arm64 + x86_64 with UNIVERSAL=1) like the app does, never a hard-coded one (an arm64 helper
# breaks Intel source builds), and the host-only default must stay.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
fail() { echo "FAIL: $*" >&2; exit 1; }
grep -n 'swiftc' "$root/scripts/bundle.sh" | grep -q -- '-target arm64' && fail "bundle.sh hard-codes -target arm64 for a helper"
grep -q -- '-target "$a-apple-macos13.0"' "$root/scripts/bundle.sh" || fail "swiftc is not run once per arch in archs"
grep -q 'archs=("$(uname -m)")' "$root/scripts/bundle.sh" || fail "default build is not host-only"
grep -q 'UNIVERSAL:-0' "$root/scripts/bundle.sh" || fail "UNIVERSAL is not opt-in"
grep -q 'xcrun swiftc\|swiftc_archs' "$root/scripts/bundle.sh" || fail "no swiftc call"
[ "$(grep -c 'xcrun swiftc' "$root/scripts/bundle.sh")" -eq 1 ] || fail "swiftc must only be called from swiftc_archs"
echo "bundle arch: ok"
