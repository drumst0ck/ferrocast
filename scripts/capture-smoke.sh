#!/bin/bash
# Smoke test for the ferrocast-capture helper. Usage: scripts/capture-smoke.sh [path-to-helper]
# Default path: dist/Ferrocast.app/Contents/MacOS/ferrocast-capture. Permission-dependent checks
# print SKIP when Screen Recording is not granted. Exit 0 on pass, 1 on failure.
set -uo pipefail
cd "$(dirname "$0")/.."
helper=${1:-dist/Ferrocast.app/Contents/MacOS/ferrocast-capture}
[ -x "$helper" ] || { echo "FAIL: helper not found or not executable: $helper"; exit 1; }
fail=0
main_display=
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
    main_display=$(printf '%s\n' "$out" | head -1 | python3 -c 'import sys,json; print(next((d["id"] for d in json.loads(sys.stdin.readline())["displays"] if d.get("main")), ""))')
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

# Recording: 2 s of the main display; the duration of screen.mov must be within 0.1 s of the wall
# clock between the start reply and the stop request, static screen included. SKIP (structured
# permission-screen, exit 1, no file) when Screen Recording is not granted. The duration comes from
# the movie header (mvhd) parsed here, so neither ffprobe nor AVFoundation is needed.
if [ "$sck" != True ]; then
  echo "SKIP: record (ScreenCaptureKit unavailable on this macOS)"
else
  recdir=$(mktemp -d)
  rec_out=$(python3 - "$helper" "${main_display:-1}" "$recdir/Smoke.ferrocast" <<'PY'
import json, os, struct, subprocess, sys, time

helper, display, project = sys.argv[1], int(sys.argv[2]), sys.argv[3]

def mov_duration(path):
    """Movie duration in seconds from moov/mvhd."""
    with open(path, "rb") as f:
        def find(pos, end, names):
            while pos + 8 <= end:
                f.seek(pos)
                size, kind = struct.unpack(">I4s", f.read(8))
                head = 8
                if size == 1:
                    size, head = struct.unpack(">Q", f.read(8))[0], 16
                elif size == 0:
                    size = end - pos
                if kind == names[0]:
                    return (pos + head) if len(names) == 1 else find(pos + head, pos + size, names[1:])
                pos += max(size, 8)
            return None
        at = find(0, os.fstat(f.fileno()).st_size, [b"moov", b"mvhd"])
        if at is None:
            raise ValueError("no moov/mvhd box")
        f.seek(at)
        version = f.read(1)[0]
        f.read(3)
        if version == 1:
            f.read(16)
            scale, dur = struct.unpack(">IQ", f.read(12))
        else:
            f.read(8)
            scale, dur = struct.unpack(">II", f.read(8))
        return dur / scale

p = subprocess.Popen([helper], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, bufsize=1)
def send(o):
    p.stdin.write(json.dumps(o) + "\n"); p.stdin.flush()
def read_until(rid):
    events = []
    for line in p.stdout:
        o = json.loads(line)  # every stdout line must be a protocol object
        assert isinstance(o, dict)
        if "event" in o:
            events.append(o); continue
        if o.get("id") == rid:
            return o, events
    return None, events

send({"id": 1, "cmd": "record.start", "project": project, "file": "screen.mov",
      "target": {"kind": "display", "display": display}, "fps": 30, "cursor": False})
r, ev = read_until(1)
if r is None or r.get("error"):
    p.stdin.close(); code = p.wait()
    if r and r.get("error") == "permission-screen" and code == 1 and not os.path.exists(project):
        print("SKIP: record (Screen Recording permission not granted; structured permission-screen, exit 1, no file left)"); sys.exit(3)
    print("FAIL: record.start -> %r exit=%s" % (r, code)); sys.exit(1)
t_ok = time.monotonic()
time.sleep(2.0)
send({"id": 2, "cmd": "record.stop"})
r2, ev2 = read_until(2)
# Wall clock = from the start reply to the stop reply: the file ends when the stop is processed, so the
# stop latency (15-45 ms measured) belongs to the reference. Measuring to the moment the stop was SENT made
# the check fail by ~5 ms on a cold start (live run, 2.110 s file vs 2.005 s).
t_stop = time.monotonic()
p.stdin.close(); code = p.wait(timeout=10)
wall = t_stop - t_ok
ticks = [e for e in ev + ev2 if e["event"] == "tick"]
if not r2 or r2.get("error"):
    print("FAIL: record.stop -> %r" % (r2,)); sys.exit(1)
path = os.path.join(project, "screen.mov")
if not os.path.isfile(path) or os.path.exists(path + ".partial"):
    print("FAIL: screen.mov missing or .partial left"); sys.exit(1)
dur = mov_duration(path)
msg = "file %.3f s, wall %.3f s, helper %.3f s, %d ticks, %s frames, size %s" % (dur, wall, r2["duration"], len(ticks), r2.get("frames"), r2.get("size"))
# Sanity against the nominal 2 s (loose: start/stop latency is included) and the real S11 rule against wall time.
if abs(dur - 2.0) > 0.25 or abs(dur - wall) > 0.1 or code != 0 or len(ticks) < 1:
    print("FAIL: record duration/ticks/exit (%s, exit %s)" % (msg, code)); sys.exit(1)
print("PASS: record (%s)" % msg)

# events.json: written next to screen.mov (atomically, no .partial), version 1, shaped as
# crates/core/src/capture/events.rs decodes it (the Rust fixture test covers the strict decoder).
ev_path = os.path.join(project, "events.json")
if not os.path.isfile(ev_path) or os.path.exists(ev_path + ".partial"):
    print("FAIL: events.json missing or .partial left"); sys.exit(1)
try:
    ev = json.load(open(ev_path))
    assert ev["version"] == 1 and len(ev["screen"]["size"]) == 2 and ev["screen"]["scale"] > 0
    assert len(ev["screen"]["display_frame"]) == 4
    for name in ("cursor", "clicks", "keys", "scroll", "windows"):
        assert isinstance(ev[name], list), name
    assert len(ev["cursor"]) >= 1 and all(len(c) == 4 for c in ev["cursor"])
    ts = [c[0] for c in ev["cursor"]]
    why = None
    if ts != sorted(ts):
        why = "unsorted at %r" % [(i, ts[i - 1], ts[i]) for i in range(1, len(ts)) if ts[i] < ts[i - 1]][:3]
    elif ts[0] < 0:
        why = "negative first time %r" % ts[0]
    elif ts[-1] > dur + 0.2:
        why = "last sample %.3f after the file end %.3f" % (ts[-1], dur)
    assert why is None, "cursor times: %s" % why
    for k in ev["keys"]:  # privacy: contents only for shortcuts
        assert any(m != "shift" for m in k.get("mods", [])) or not (k.get("code") or k.get("char")), "plain key stored"
except Exception as e:
    print("FAIL: events.json invalid (%r)" % (e,)); sys.exit(1)
print("PASS: events.json (version %d, %d cursor samples, %d clicks, %d keys, %d scroll, %d windows)" % (
    ev["version"], len(ev["cursor"]), len(ev["clicks"]), len(ev["keys"]), len(ev["scroll"]), len(ev["windows"])))
PY
  ); rec_rc=$?
  echo "$rec_out"
  case $rec_rc in 0) ;; 3) ;; *) fail=1 ;; esac
  rm -rf "$recdir"
fi

# render.frame (T24): needs no Screen Recording grant. Generates a 30-frame 320x200 screen.mov with
# AVAssetWriter (swiftc), pairs it with crates/core/fixtures/render_project.json (checked by a Rust
# test), renders a 960 px frame and checks: PNG of the expected size, written atomically, cache hit
# bytes equal, a second helper process gives identical bytes (determinism), and a project that also
# carries P6 fields (loupes/looks/split, S19) renders the same bytes as one without. The sha256 is
# printed and compared with $RENDER_GOLDEN (default target/capture-spike/render-golden.sha256) when
# that file exists, else stored there. Delete it after an intended change of the look.
if ! command -v swiftc >/dev/null 2>&1 || ! xcrun --find swiftc >/dev/null 2>&1; then
  echo "SKIP: render.frame (swiftc not available)"
else
  run '{"id":1,"cmd":"hello"}'
  if ! printf '%s' "$out" | grep -q '"render.frame"'; then
    echo "SKIP: render.frame (helper does not advertise render.frame)"
  else
    mkdir -p target/capture-spike/tmp
    rdir=$(mktemp -d "$PWD/target/capture-spike/tmp/render.XXXXXX")
    cat > "$rdir/gen.swift" <<'SWIFT'
import AVFoundation
import CoreGraphics
import Foundation

// usage: gen <project-dir>: screen.mov with 30 frames, 320x200, 30 fps: gradient, a red square at
// the top left and a white bar that moves right.
let url = URL(fileURLWithPath: CommandLine.arguments[1]).appendingPathComponent("screen.mov")
try? FileManager.default.removeItem(at: url)
let w = 320, h = 200
let srgb = CGColorSpace(name: CGColorSpace.sRGB)!
let writer = try AVAssetWriter(outputURL: url, fileType: .mov)
let input = AVAssetWriterInput(mediaType: .video, outputSettings: [
    AVVideoCodecKey: AVVideoCodecType.h264, AVVideoWidthKey: w, AVVideoHeightKey: h])
let adaptor = AVAssetWriterInputPixelBufferAdaptor(assetWriterInput: input, sourcePixelBufferAttributes: [
    kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA,
    kCVPixelBufferWidthKey as String: w, kCVPixelBufferHeightKey as String: h])
writer.add(input)
writer.startWriting()
writer.startSession(atSourceTime: .zero)
for i in 0..<30 {
    while !input.isReadyForMoreMediaData { Thread.sleep(forTimeInterval: 0.005) }
    var pb: CVPixelBuffer?
    CVPixelBufferPoolCreatePixelBuffer(nil, adaptor.pixelBufferPool!, &pb)
    let buf = pb!
    CVPixelBufferLockBaseAddress(buf, [])
    let ctx = CGContext(data: CVPixelBufferGetBaseAddress(buf), width: w, height: h, bitsPerComponent: 8,
                        bytesPerRow: CVPixelBufferGetBytesPerRow(buf), space: srgb,
                        bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue)!
    let g = CGGradient(colorsSpace: srgb, colors: [CGColor(red: 0.1, green: 0.6, blue: 0.9, alpha: 1),
                                                   CGColor(red: 0.95, green: 0.8, blue: 0.2, alpha: 1)] as CFArray, locations: nil)!
    ctx.drawLinearGradient(g, start: CGPoint(x: 0, y: Double(h)), end: CGPoint(x: Double(w), y: 0), options: [])
    ctx.setFillColor(CGColor(red: 1, green: 0, blue: 0, alpha: 1))
    ctx.fill(CGRect(x: 0, y: h - 40, width: 40, height: 40))
    ctx.setFillColor(CGColor(red: 1, green: 1, blue: 1, alpha: 1))
    ctx.fill(CGRect(x: 10 + i * 9, y: 90, width: 12, height: 20))
    CVPixelBufferUnlockBaseAddress(buf, [])
    adaptor.append(buf, withPresentationTime: CMTime(value: Int64(i), timescale: 30))
}
input.markAsFinished()
let sem = DispatchSemaphore(value: 0)
writer.finishWriting { sem.signal() }
sem.wait()
if writer.status != .completed { FileHandle.standardError.write(Data("fixture failed\n".utf8)); exit(1) }
SWIFT
    mkdir -p "$rdir/Fixture.ferrocast"
    if swiftc -O "$rdir/gen.swift" -o "$rdir/gen" 2>/dev/null && "$rdir/gen" "$rdir/Fixture.ferrocast" \
       && cp crates/core/fixtures/render_project.json "$rdir/Fixture.ferrocast/project.json"; then
      render_out=$(python3 - "$helper" "$rdir/Fixture.ferrocast" "${RENDER_GOLDEN:-target/capture-spike/render-golden.sha256}" <<'PY'
import hashlib, json, os, struct, subprocess, sys

helper, project, golden = sys.argv[1:4]

def session(reqs):
    """One helper process; returns the replies in order."""
    p = subprocess.Popen([helper], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
    out, _ = p.communicate("".join(json.dumps(r) + "\n" for r in reqs), timeout=60)
    replies = [json.loads(l) for l in out.splitlines()]
    assert len(replies) == len(reqs) and p.returncode == 0, (replies, p.returncode)
    return replies

def req(i, t=0.5, width=960, **kw):
    return dict({"id": i, "cmd": "render.frame", "project": project, "t": t, "width": width}, **kw)

def png_size(path):
    d = open(path, "rb").read(24)
    assert d[:8] == b"\x89PNG\r\n\x1a\n", "not a PNG"
    return list(struct.unpack(">II", d[16:24]))

def digest(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()

try:
    out_a, out_b = project + "/.cache/a.png", project + "/.cache/b.png"
    ra, rb, bad, missing = session([req(1, out=out_a), req(2, out=out_b), req(3, t="x"), req(4, project=project + "/nope")])
    assert ra.get("ok") and not ra["cached"] and ra["size"] == [960, 540], ra
    assert png_size(out_a) == [960, 540] and not os.path.exists(out_a + ".partial"), "size or .partial"
    assert rb.get("ok") and rb["cached"] and digest(out_a) == digest(out_b), ("cache hit differs", rb)
    assert bad.get("error") == "invalid" and missing.get("error") == "not-found", (bad, missing)
    sha = digest(out_a)

    out_c = project + "/.cache/c.png"
    assert session([req(5, out=out_c)])[0].get("ok") and digest(out_c) == sha, "second process differs (not deterministic)"

    # S19: P6 fields (loupes, looks, split) in project.json change nothing about the frame.
    path = project + "/project.json"
    edl = json.load(open(path))
    edl.update({"loupes": [{"start": 0.1, "end": 0.4}], "looks": "warm", "split": {"layout": "left"}})
    json.dump(edl, open(path, "w"))
    out_d = project + "/.cache/d.png"
    assert session([req(6, out=out_d)])[0].get("ok") and digest(out_d) == sha, "P6 fields changed the rendered bytes"

    a = session([req(7, t=0.0, width=480, out=project + "/.cache/e.png")])[0]
    assert a.get("ok") and a["size"] == [480, 270], a

    note = ""
    if os.path.exists(golden):
        want = open(golden).read().strip()
        assert want == sha, "golden mismatch: %s != %s (delete %s after an intended change)" % (sha, want, golden)
        note = ", golden matches"
    else:
        os.makedirs(os.path.dirname(golden), exist_ok=True)
        open(golden, "w").write(sha + "\n")
        note = ", golden stored"
    print("PASS: render.frame (960x540 PNG %d bytes in %.0f ms, cache hit identical, 2nd process identical, P6 fields ignored, 480 px -> 480x270, sha256 %s%s)"
          % (os.path.getsize(out_a), ra["ms"], sha[:16], note))
except Exception as e:
    print("FAIL: render.frame (%s)" % (e,)); sys.exit(1)
PY
      ); render_rc=$?
      echo "$render_out"
      [ "$render_rc" = 0 ] || fail=1
    else
      echo "SKIP: render.frame (fixture movie could not be generated: AVFoundation unavailable?)"
    fi
    rm -rf "$rdir"
  fi
fi

# Unknown command: structured error, still protocol-only stdout.
run '{"id":3,"cmd":"nope"}'
[ "$(field error)" = invalid ] && echo "PASS: unknown command rejected" || bad "unknown command rc=$rc out=$out"

[ "$fail" = 0 ] && echo "capture-smoke: OK"
exit "$fail"
