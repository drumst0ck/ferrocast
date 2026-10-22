#!/bin/bash
# Smoke test for the ferrocast-capture helper. Usage: scripts/capture-smoke.sh [path-to-helper]
# Default path: dist/Ferrocast.app/Contents/MacOS/ferrocast-capture. Permission-dependent checks
# print SKIP when Screen Recording is not granted. Exit 0 on pass, 1 on failure.
set -uo pipefail
cd "$(dirname "$0")/.."
helper=${1:-dist/Ferrocast.app/Contents/MacOS/ferrocast-capture}
[ -x "$helper" ] || { echo "FAIL: helper not found or not executable: $helper"; exit 1; }
fail=0
bad() { echo "FAIL: $*"; fail=1; }

# Run one request; stdout to $out, exit code to $rc. Stdout must be only JSON object lines.
run() {
  out=$(printf '%s\n' "$1" | "$helper" 2>/dev/null); rc=$?
  if [ -n "$out" ] && ! printf '%s\n' "$out" | python3 -c '
import sys, json
for l in sys.stdin:
    assert isinstance(json.loads(l), dict)' 2>/dev/null; then
    bad "stdout holds non-protocol output: $out"
  fi
}
field() { printf '%s\n' "$out" | head -1 | python3 -c "import sys,json; print(json.loads(sys.stdin.readline()).get('$1',''))"; }

run '{"id":1,"cmd":"hello"}'
if [ "$rc" = 0 ] && [ "$(field ok)" = True ] && [ "$(field protocol)" = 1 ]; then
  echo "PASS: hello ($out)"
else
  bad "hello rc=$rc out=$out"
fi

if [ "$(field sck)" != True ]; then
  echo "SKIP: shareable (ScreenCaptureKit unavailable on this macOS)"
else
  run '{"id":2,"cmd":"shareable"}'
  if [ "$rc" = 0 ] && [ "$(field ok)" = True ]; then
    echo "PASS: shareable"
  elif [ "$rc" = 1 ] && [ "$(field error)" = permission-screen ]; then
    echo "SKIP: shareable (Screen Recording permission not granted; structured permission-screen, exit 1)"
  else
    bad "shareable rc=$rc out=$out"
  fi
fi

# Unknown command: structured error, still protocol-only stdout.
run '{"id":3,"cmd":"nope"}'
[ "$(field error)" = invalid ] && echo "PASS: unknown command rejected" || bad "unknown command rc=$rc out=$out"

[ "$fail" = 0 ] && echo "capture-smoke: OK"
exit "$fail"
