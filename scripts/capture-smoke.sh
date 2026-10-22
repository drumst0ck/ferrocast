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

sck=$(field sck)
if [ "$sck" != True ]; then
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

# Screenshot: PNG at the requested path when granted; SKIP (permission-screen, exit 1, no file) when not.
if [ "$sck" != True ]; then
  echo "SKIP: screenshot (ScreenCaptureKit unavailable on this macOS)"
else
  shot=$(mktemp -d)/smoke.png
  run "{\"id\":4,\"cmd\":\"screenshot\",\"target\":{\"kind\":\"display\",\"display\":$(python3 -c 'import Quartz; print(Quartz.CGMainDisplayID())' 2>/dev/null || echo 1)},\"out\":\"$shot\"}"
  if [ "$rc" = 0 ] && [ "$(field ok)" = True ] && [ "$(head -c 8 "$shot" | xxd -p)" = 89504e470d0a1a0a ] && [ ! -e "$shot.partial" ]; then
    echo "PASS: screenshot ($(field size))"
  elif [ "$rc" = 1 ] && [ "$(field error)" = permission-screen ] && [ ! -e "$shot" ] && [ ! -e "$shot.partial" ]; then
    echo "SKIP: screenshot (Screen Recording permission not granted; structured permission-screen, exit 1, no file left)"
  else
    bad "screenshot rc=$rc out=$out"
  fi
  rm -rf "$(dirname "$shot")"
fi

# Window screenshot as the FIRST command of a fresh helper: `SCContentFilter(desktopIndependentWindow:)` in a
# process that never touched CoreGraphics aborted the helper with `CGS_REQUIRE_INIT` (found live: Capture Window
# and Record Window saved nothing). A listing process first picks a visible window id; the process under test
# must then answer the screenshot without any earlier command. SKIP without a grant or without a window.
if [ "$sck" != True ]; then
  echo "SKIP: window screenshot (ScreenCaptureKit unavailable on this macOS)"
else
  wdir=$(mktemp -d)
  python3 - "$helper" "$wdir/win.png" <<'PY'
import json, os, subprocess, sys
helper, out = sys.argv[1], sys.argv[2]
def session(reqs):
    p = subprocess.Popen([helper], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1)
    replies = []
    for o in reqs:
        try:
            p.stdin.write(json.dumps(o) + "\n"); p.stdin.flush()
        except BrokenPipeError:
            break
        r = None
        for line in p.stdout:
            x = json.loads(line)
            if x.get("id") == o["id"]:
                r = x; break
        replies.append(r)
        if r is None: break
    try: p.stdin.close()
    except Exception: pass
    try: p.wait(timeout=10)
    except Exception: p.kill()
    return replies, p.returncode, p.stderr.read()
r, _, _ = session([{"id": 1, "cmd": "shareable"}])
if not r or not r[0] or r[0].get("error"):
    print("SKIP: window screenshot (Screen Recording permission not granted or listing failed)"); sys.exit(3)
wins = [w for w in r[0]["windows"] if w.get("on_screen") and w.get("layer", 0) == 0 and w["frame"][2] > 100 and w["frame"][3] > 100]
if not wins:
    print("SKIP: window screenshot (no visible window to capture)"); sys.exit(3)
w = wins[0]
replies, code, err = session([{"id": 2, "cmd": "screenshot", "target": {"kind": "window", "window": w["id"]}, "out": out, "cursor": False}])
ok = replies and replies[0] and replies[0].get("ok") and code == 0 and open(out, "rb").read(8) == b"\x89PNG\r\n\x1a\n" and not os.path.exists(out + ".partial")
if not ok:
    print("FAIL: window screenshot as the first command (reply %r, exit %r, stderr %r)" % (replies, code, err[:160])); sys.exit(1)
print("PASS: window screenshot as the first command (%s, %s)" % (w.get("app"), replies[0].get("size")))
PY
  wrc=$?
  if [ "$wrc" = 1 ]; then fail=1; fi
  rm -rf "$wdir"
fi

# Unknown command: structured error, still protocol-only stdout.
run '{"id":3,"cmd":"nope"}'
[ "$(field error)" = invalid ] && echo "PASS: unknown command rejected" || bad "unknown command rc=$rc out=$out"

[ "$fail" = 0 ] && echo "capture-smoke: OK"
exit "$fail"
