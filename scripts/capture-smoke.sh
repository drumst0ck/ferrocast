#!/bin/bash
# Smoke test for the ferrocast-capture helper. Usage: scripts/capture-smoke.sh [path-to-helper]
# Default path: dist/Ferrocast.app/Contents/MacOS/ferrocast-capture. Permission-dependent checks
# print SKIP when Screen Recording is not granted. Exit 0 on pass, 1 on failure.
# --keep (anywhere in the arguments): also leave an editor fixture project under
# target/capture-spike/editor-fixture/Take.ferrocast (the generated 3 s screen.mov, the render
# fixture's project.json with duration 3 and no zooms, and render_events.json) for the editor's
# live test and frame-rate measurement. Nothing else changes.
# --sound: the P4 system-audio check also plays one short system sound (afplay) and checks the RMS.
set -uo pipefail
cd "$(dirname "$0")/.."
keep=0
sound=0
args=()
for a in "$@"; do
  if [ "$a" = --keep ]; then keep=1; elif [ "$a" = --sound ]; then sound=1; else args+=("$a"); fi
done
helper=${args[0]:-dist/Ferrocast.app/Contents/MacOS/ferrocast-capture}
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

t_send = time.monotonic()
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
wall_send = t_stop - t_send
ticks = [e for e in ev + ev2 if e["event"] == "tick"]
if not r2 or r2.get("error"):
    print("FAIL: record.stop -> %r" % (r2,)); sys.exit(1)
path = os.path.join(project, "screen.mov")
if not os.path.isfile(path) or os.path.exists(path + ".partial"):
    print("FAIL: screen.mov missing or .partial left"); sys.exit(1)
dur = mov_duration(path)
msg = "file %.3f s, wall %.3f s (%.3f s from the request), helper %.3f s, %d ticks, %s frames, size %s" % (dur, wall, wall_send, r2["duration"], len(ticks), r2.get("frames"), r2.get("size"))
# The file starts inside startCapture, i.e. somewhere between the moment the request was SENT and the start
# reply, and ends when the stop is processed. So its length must lie in [wall(reply -> stop reply) - 0.1 s,
# wall(request -> stop reply) + 0.1 s]. Locally the file is 10-35 ms longer than the first reference; on the
# CI virtual machine (a static display, 2-3 frames in 2 s) it was 170-240 ms longer because the start reply
# arrives late, which a fixed 0.1 s tolerance around the reply wrongly called a failure. The nominal 2 s
# check stays as a loose sanity band (start latency included).
if abs(dur - 2.0) > 0.6 or dur < wall - 0.1 or dur > wall_send + 0.1 or code != 0 or len(ticks) < 1:
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

// usage: gen <project-dir> [frames=30] [first-pts-in-frames=0]: screen.mov with 320x200 frames at 30 fps: gradient, a red
// square at the top left and a white bar that moves right (wrapping at 300 px: frames < 34 are
// unchanged from T24, so its golden stays valid).
let url = URL(fileURLWithPath: CommandLine.arguments[1]).appendingPathComponent("screen.mov")
try? FileManager.default.removeItem(at: url)
let w = 320, h = 200
let frames = CommandLine.arguments.count > 2 ? Int(CommandLine.arguments[2]) ?? 30 : 30
// a real SCStream take does not start at 0 (first frame at ~13 ms): a nonzero value reproduces it
let firstPts = CommandLine.arguments.count > 3 ? Int64(CommandLine.arguments[3]) ?? 0 : 0
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
for i in 0..<frames {
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
    ctx.fill(CGRect(x: 10 + (i * 9) % 290, y: 90, width: 12, height: 20))
    CVPixelBufferUnlockBaseAddress(buf, [])
    adaptor.append(buf, withPresentationTime: CMTime(value: Int64(i) + firstPts, timescale: 30))
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

      # A real recording starts at its first sample (~13 ms), not at 0: the frame at t = 0 must be
      # the first sample, not "Cannot Open" (found on the first live take; the fixture above starts at 0).
      odir="$rdir/Offset.ferrocast"
      mkdir -p "$odir"
      if "$rdir/gen" "$odir" 30 2 && cp crates/core/fixtures/render_project.json "$odir/project.json"; then
        off_out=$(python3 - "$helper" "$odir" <<'PY'
import json, subprocess, sys

helper, project = sys.argv[1:3]
reqs = [{"id": i + 1, "cmd": "render.frame", "project": project, "t": t, "width": 480,
         "out": "%s/.cache/o%d.png" % (project, i)} for i, t in enumerate([0.0, 0.01, 0.2])]
p = subprocess.Popen([helper], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
out, _ = p.communicate("".join(json.dumps(r) + "\n" for r in reqs), timeout=60)
replies = [json.loads(l) for l in out.splitlines()]
bad = [r for r in replies if not r.get("ok")]
if len(replies) != 3 or bad:
    print("FAIL: render.frame before the first sample (%s)" % (bad or replies,)); sys.exit(1)
print("PASS: render.frame at t = 0 of a take whose first sample is after 0 (3 of 3)")
PY
        ); off_rc=$?
        echo "$off_out"
        [ "$off_rc" = 0 ] || fail=1
      else
        echo "FAIL: render.frame offset fixture could not be made"; fail=1
      fi

      # T25: camera crop, cursor sprite, click ring and motion blur from the optional render.frame
      # fields. The fields come from crates/core/fixtures/render_zoom_params.json, which a Rust test
      # (compose_fixture) keeps equal to what capture::compose evaluates: Rust owns the maths, the
      # helper only draws. events.json next to it was written by the Swift EventLog. Frames are kept
      # under target/capture-spike/render-fixture/t25/ for looking at.
      zdir="$rdir/Zoom.ferrocast"
      mkdir -p "$zdir" target/capture-spike/render-fixture/t25
      if "$rdir/gen" "$zdir" 90 && cp crates/core/fixtures/render_zoom_project.json "$zdir/project.json" \
         && cp crates/core/fixtures/render_events.json "$zdir/events.json"; then
        t25_out=$(python3 - "$helper" "$zdir" crates/core/fixtures/render_zoom_params.json target/capture-spike/render-fixture/t25 <<'PY'
import hashlib, json, os, shutil, struct, subprocess, sys, zlib

helper, project, params_file, keep = sys.argv[1:5]
frames = json.load(open(params_file))

def session(reqs):
    p = subprocess.Popen([helper], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
    out, _ = p.communicate("".join(json.dumps(r) + "\n" for r in reqs), timeout=60)
    replies = [json.loads(l) for l in out.splitlines()]
    assert len(replies) == len(reqs) and p.returncode == 0, (replies, p.returncode)
    return replies

n = [100]
def req(t, name, params=None, width=960):
    n[0] += 1
    return dict({"id": n[0], "cmd": "render.frame", "project": project, "t": t, "width": width,
                 "out": "%s/.cache/%s.png" % (project, name)}, **(params or {}))

def digest(name):
    return hashlib.sha256(open("%s/.cache/%s.png" % (project, name), "rb").read()).hexdigest()

def pixels(name):
    """Decode an 8-bit RGB/RGBA non-interlaced PNG into (w, h, rows of bytes, channels)."""
    d = open("%s/.cache/%s.png" % (project, name), "rb").read()
    assert d[:8] == b"\x89PNG\r\n\x1a\n"
    pos, idat, w = 8, b"", None
    while pos < len(d):
        ln, kind = struct.unpack(">I4s", d[pos:pos + 8])
        body = d[pos + 8:pos + 8 + ln]
        if kind == b"IHDR":
            w, h, depth, ctype, _, _, inter = struct.unpack(">IIBBBBB", body)
            assert depth == 8 and ctype in (2, 6) and inter == 0, (depth, ctype, inter)
        elif kind == b"IDAT":
            idat += body
        pos += 12 + ln
    ch = 3 if ctype == 2 else 4
    raw, stride, rows, prev = zlib.decompress(idat), w * ch, [], bytearray(w * ch)
    for y in range(h):
        f, line = raw[y * (stride + 1)], bytearray(raw[y * (stride + 1) + 1:(y + 1) * (stride + 1)])
        for i in range(stride):
            a = line[i - ch] if i >= ch else 0
            b = prev[i]
            c = prev[i - ch] if i >= ch else 0
            if f == 1: line[i] = (line[i] + a) & 255
            elif f == 2: line[i] = (line[i] + b) & 255
            elif f == 3: line[i] = (line[i] + (a + b) // 2) & 255
            elif f == 4:
                pa, pb, pc = abs(b - c), abs(a - c), abs(a + b - 2 * c)
                line[i] = (line[i] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        rows.append(line); prev = line
    return w, h, rows, ch

def count(name, pred):
    w, h, rows, ch = pixels(name)
    return sum(1 for r in rows for x in range(w) if pred(r[x * ch], r[x * ch + 1], r[x * ch + 2]))

red = lambda r, g, b: r > 200 and g < 60 and b < 60

try:
    assert [round(f["t"], 2) for f in frames] == [0.2, 0.85, 1.2, 2.5], frames
    reqs = [req(f["t"], "z%d" % i, f["params"]) for i, f in enumerate(frames)]
    first = session(reqs)
    for r in first:
        assert r.get("ok") and r["size"] == [960, 540], r
    base = [digest("z%d" % i) for i in range(4)]
    assert len(set(base)) == 4, "the four frames are not all different"

    # determinism: a second process renders identical bytes
    session([req(f["t"], "y%d" % i, f["params"]) for i, f in enumerate(frames)])
    assert [digest("y%d" % i) for i in range(4)] == base, "second process differs (not deterministic)"

    # S19: no new fields = the plain T24 frame, whatever the project's zooms say; the T24 check
    # above compares the plain Fixture frame with the golden sha256.
    plain = session([req(0.2, "p0"), req(1.2, "p2")])
    assert all(r.get("ok") for r in plain)
    assert digest("p0") != base[0] and digest("p2") != base[2], "params had no effect"
    ident = session([req(1.2, "i2", {"camera": {"x": 0, "y": 0, "w": 1, "h": 1}})])[0]
    assert ident.get("ok") and digest("i2") == digest("p2"), "identity camera differs from no camera"

    # camera: the red square (top left, 12 % of the source) is visible before and gone at full zoom
    red_before, red_zoom, red_after = count("z0", red), count("z2", red), count("z3", red)
    assert red_before > 200 and red_zoom == 0 and red_after > 200, (red_before, red_zoom, red_after)

    # ablations: each effect changes pixels, so each is really drawn
    p = lambda i: frames[i]["params"]
    ab = {"cursor": (0, "cursor"), "ring": (2, "ring"), "blur": (1, "blur"), "camera": (2, "camera")}
    ab_reqs = [req(frames[i]["t"], "no-" + k, {kk: v for kk, v in p(i).items() if kk != k}) for k, (i, _) in ab.items()]
    session(ab_reqs)
    for k, (i, _) in ab.items():
        assert digest("no-" + k) != base[i], "removing %s changed nothing" % k
    # blur leaves a longer streak than the sharp frame: more non-gradient... compared by byte difference
    sharp, blurred = pixels("no-blur"), pixels("z1")
    diff = sum(1 for ra, rb in zip(sharp[2], blurred[2]) for a, b in zip(ra, rb) if abs(a - b) > 8)
    assert diff > 500, "blur changed too few samples (%d)" % diff

    # malformed fields are an error, not a silent default
    bad = session([req(0.2, "bad", {"camera": {"x": "a", "y": 0, "w": 1, "h": 1}}),
                   req(0.2, "bad2", {"ring": [1]})])
    assert all(b.get("error") == "invalid" for b in bad), bad

    # the cache key includes the params: same t and width, different fields, different bytes
    c = session([req(1.2, "k1", p(2)), req(1.2, "k2"), req(1.2, "k3", p(2))])
    assert not c[1]["cached"] and c[2]["cached"], c
    assert digest("k1") == digest("k3") == base[2] and digest("k2") == digest("p2"), "cache served the wrong frame"

    # a camera far outside the display is clamped to the display (S15), not an error
    out_of = session([req(1.2, "far", {"camera": {"x": 0.9, "y": 0.9, "w": 0.5, "h": 0.5}}),
                      req(1.2, "far-ok", {"camera": {"x": 0.5, "y": 0.5, "w": 0.5, "h": 0.5}})])
    assert all(r.get("ok") for r in out_of) and digest("far") == digest("far-ok"), "viewport left the display"

    # perf budget (plan: <= 45 ms at 960 px): later frames of one session, assets loaded
    warm = session([req(0.2, "w0", p(0)), req(0.85, "w1", p(1)), req(1.2, "w2", p(2)), req(2.5, "w3", p(3))])
    ms = [round(r["ms"]) for r in warm if not r["cached"]]
    for i in range(4):
        shutil.copy("%s/.cache/z%d.png" % (project, i), "%s/frame-%d.png" % (keep, i))
    print("PASS: render.frame T25 (4 frames 960x540 differ, 2nd process identical, params-less = T24 frame, identity camera = none, red square %d -> %d -> %d px, cursor/ring/blur/camera each change pixels, malformed -> invalid, cache keyed by params, camera clamped; frame ms %s, budget 45 ms, PNGs in %s)"
          % (red_before, red_zoom, red_after, ms, keep))
except Exception as e:
    print("FAIL: render.frame T25 (%r)" % (e,)); sys.exit(1)
PY
        ); t25_rc=$?
        echo "$t25_out"
        [ "$t25_rc" = 0 ] || fail=1
      else
        echo "SKIP: render.frame T25 (zoom fixture could not be generated)"
      fi

      # S15/S16 performance work: raw preview frames (`format:"raw"`) and the sequential take decoder
      # (AVAssetReader) used for consecutive times and for every exported frame. Checks: a raw frame
      # equals the PNG frame PIXEL FOR PIXEL (the PNG path keeps the generator and its golden bytes);
      # the `decode` field of the reply shows the decoder policy (first frame and jumps = generator,
      # the second consecutive frame = reopen, then advance, a time inside the same sample = reuse, a
      # repeated key = cache, a jump back or far ahead = generator); an unknown format is `invalid`;
      # a sequential export (with a backward and a forward cut) has the same frame count/duration as the
      # generator export, and 3 decoded frames match it within a tolerance (compared with ffmpeg when installed).
      sdir="$rdir/Seq.ferrocast"
      mkdir -p "$sdir/.cache" target/capture-spike/export-smoke
      if "$rdir/gen" "$sdir" 90 && cp crates/core/fixtures/render_project.json "$sdir/project.json"; then
        seq_out=$(python3 - "$helper" "$sdir" "$PWD/target/capture-spike/export-smoke" <<'PY'
import json, os, shutil, struct, subprocess, sys, zlib

helper, project, outdir = sys.argv[1:4]
p = subprocess.Popen([helper], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, bufsize=1)
_id = [0]

def call(o):
    _id[0] += 1; o = dict(o, id=_id[0])
    p.stdin.write(json.dumps(o) + "\n"); p.stdin.flush()
    while True:
        l = json.loads(p.stdout.readline())
        if l.get("id") == o["id"] and "event" not in l: return l

def png_pixels(path):
    d = open(path, "rb").read(); assert d[:8] == b"\x89PNG\r\n\x1a\n"
    pos, idat = 8, b""
    while pos < len(d):
        n, typ = struct.unpack(">I4s", d[pos:pos + 8]); body = d[pos + 8:pos + 8 + n]; pos += 12 + n
        if typ == b"IHDR": w, h, depth, ct = struct.unpack(">IIBB", body[:10]); assert depth == 8 and ct in (2, 6), (depth, ct)
        if typ == b"IDAT": idat += body
    bpp = 4 if ct == 6 else 3
    raw, stride, rows, prev, q = zlib.decompress(idat), w * bpp, [], bytearray(w * bpp), 0
    for _ in range(h):
        f, line = raw[q], bytearray(raw[q + 1:q + 1 + stride]); q += 1 + stride
        for i in range(stride):
            a = line[i - bpp] if i >= bpp else 0; b = prev[i]; c = prev[i - bpp] if i >= bpp else 0
            if f == 1: line[i] = (line[i] + a) & 255
            elif f == 2: line[i] = (line[i] + b) & 255
            elif f == 3: line[i] = (line[i] + ((a + b) >> 1)) & 255
            elif f == 4:
                pa, pb, pc = abs(b - c), abs(a - c), abs(a + b - 2 * c)
                line[i] = (line[i] + (a if pa <= pb and pa <= pc else (b if pb <= pc else c))) & 255
        rows.append(bytes(line)); prev = line
    px = b"".join(rows)
    if bpp == 3: px = b"".join(px[i:i + 3] + b"\xff" for i in range(0, len(px), 3))
    return w, h, px

def frame(t, width=160, **kw):
    return dict({"cmd": "render.frame", "project": project, "t": t, "width": width}, **kw)

try:
    png, rawf = project + "/.cache/s.png", project + "/.cache/s.rgba"
    # raw == PNG, pixel for pixel, for a consecutive run (sequential decoder) and two single frames
    steps, decodes = [], []
    times = [0.2 + i / 30 for i in range(6)] + [0.5, 0.5, 0.5 + 0.004, 2.5, 0.1, 1.9]
    for t in times:
        r = call(frame(t, out=rawf, format="raw")); assert r.get("ok") and r["format"] == "raw", r
        w, h = r["size"]; assert r["stride"] == w * 4, r
        got = open(rawf, "rb").read(); assert len(got) == w * h * 4, (len(got), r)
        q = call(frame(t, out=png)); assert q.get("ok") and "format" not in q, q
        pw, ph, px = png_pixels(png)
        assert (pw, ph) == (w, h), ((pw, ph), (w, h))
        assert px == got, "raw frame differs from the PNG frame at t=%s (%d bytes differ)" % (t, sum(a != b for a, b in zip(px, got)))
        decodes.append(r["decode"])
    # 0.2 generator, 0.233 reopen, 0.267 .. 0.367 advance, 0.5 advance (4 frames ahead, reader still open),
    # 0.5 again cache, 0.504 reuse (inside the sample of 0.5), 2.5 / 0.1 / 1.9 generator
    # (far ahead of the open reader, backwards, far ahead again)
    want = ["generator", "reopen", "advance", "advance", "advance", "advance", "advance"]
    assert decodes == want + ["cache", "reuse", "generator", "generator", "generator"], decodes
    bad = call(frame(0.5, format="bmp")); assert bad.get("error") == "invalid", bad

    # sequential export (default) vs the generator export, with a backward and a forward cut
    n = 90
    pf = project + "/.cache/seq-params.ndjson"
    with open(pf, "w") as f:
        for i in range(n):
            t = i / 30.0 if i < 30 else (0.5 + (i - 30) / 30.0 if i < 60 else 2.0 + (i - 60) / 30.0 - 0.0)
            f.write(json.dumps({"t": min(t, 2.96)}) + "\n")
    def export(name, **kw):
        out = outdir + "/" + name
        if os.path.exists(out): os.remove(out)
        r = call(dict({"cmd": "render", "project": project, "out": out, "format": "mp4", "codec": "h264", "width": 480, "height": 270,
                       "fps": 30, "quality": "standard", "frames": n,
                       "segments": [{"src_start": 0.0, "src_end": n / 30.0, "speed": 1.0}], "params_file": pf}, **kw))
        assert r.get("ok") and r["frames"] == n and not os.path.exists(out + ".partial"), r
        return out
    a, b = export("seq-fast.mp4"), export("seq-ref.mp4", decode="generator")
    # The pixel buffers handed to the encoder are bit-identical in both modes (checked with an
    # instrumented build: FNV hash of every frame). The hardware H.264 encoder is not deterministic
    # against the pace of its input (the faster sequential export differs run to run), so the MP4s
    # are compared after decoding, on 3 frames (before the cuts, after the backward cut, after the
    # forward cut), within a tolerance: max 40 / mean 2 levels. A wrong frame (off by one: the white bar
    # moves 9 px per frame) differs by 100+ levels over hundreds of pixels.
    note = "frames not compared (ffmpeg not installed)"
    if shutil.which("ffmpeg"):
        raws = []
        for m in (a, b):
            r = subprocess.run(["ffmpeg", "-v", "error", "-i", m, "-f", "rawvideo", "-pix_fmt", "rgb24", "-"], capture_output=True)
            assert r.returncode == 0 and len(r.stdout) == n * 480 * 270 * 3, (r.returncode, len(r.stdout))
            raws.append(r.stdout)
        size, worst, total = 480 * 270 * 3, 0, 0
        for i in (5, 40, 80):
            x, y = raws[0][i * size:(i + 1) * size], raws[1][i * size:(i + 1) * size]
            d = [abs(u - v) for u, v in zip(x, y)]
            worst, total = max(worst, max(d)), total + sum(d)
        mean = total / (3 * size)
        assert worst <= 40 and mean <= 2.0, "sequential export frames differ from the generator export (max %d, mean %.2f)" % (worst, mean)
        note = "3 decoded frames match the generator export (max diff %d, mean %.3f)" % (worst, mean)
    print("PASS: render.frame raw (== PNG pixel for pixel on %d frames, decode %s, bad format invalid) and sequential MP4 export (%d frames, %s)"
          % (len(times), "/".join(sorted(set(decodes))), n, note))
except Exception as e:
    print("FAIL: render.frame raw / sequential export (%s)" % (e,)); sys.exit(1)
finally:
    p.stdin.close()
    try: p.wait(timeout=10)
    except Exception: p.kill()
PY
        ); seq_rc=$?
        echo "$seq_out"
        [ "$seq_rc" = 0 ] || fail=1
      else
        echo "SKIP: render.frame raw / sequential (fixture could not be generated)"
      fi
    else
      echo "SKIP: render.frame (fixture movie could not be generated: AVFoundation unavailable?)"
    fi
    # T28: MP4 export (`render`). The per-frame parameters Rust would compute (source time and camera)
    # are written here by hand: one NDJSON line per output frame in params_file. Checks: `started`,
    # `progress` events and a final `ok`; the MP4 header (parsed here, no ffprobe needed) has the
    # requested codec and size and a duration within one frame of frames/fps; no `.partial` is left; a
    # `cancel` mid-export answers `cancelled` and leaves no output and no `.partial`; bad requests
    # are `invalid`; the helper keeps answering afterwards. Output stays in target/capture-spike/export-smoke/.
    edir="$rdir/Export.ferrocast"
    mkdir -p "$edir/.cache" target/capture-spike/export-smoke
    if "$rdir/gen" "$edir" 90 && cp crates/core/fixtures/render_project.json "$edir/project.json"; then
      export_out=$(python3 - "$helper" "$edir" "$PWD/target/capture-spike/export-smoke" <<'PY'
import json, os, queue, struct, subprocess, sys, threading, time

helper, project, outdir = sys.argv[1:4]
p = subprocess.Popen([helper], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
lines = queue.Queue()
threading.Thread(target=lambda: [lines.put(json.loads(l)) for l in p.stdout], daemon=True).start()

def send(o):
    p.stdin.write(json.dumps(o) + "\n"); p.stdin.flush()

def until(pred, timeout=90):
    """Lines up to and including the first for which pred is true."""
    got, end = [], time.time() + timeout
    while time.time() < end:
        try: l = lines.get(timeout=1)
        except queue.Empty: continue
        got.append(l)
        if pred(l): return got
    raise AssertionError("timeout, got %r" % got[-3:])

def params(n, zoom=True):
    path = project + "/.cache/params-%d.ndjson" % n
    with open(path, "w") as f:
        for i in range(n):
            o = {"t": min(i / 30.0, 2.96)}
            if zoom and 30 <= i < 60: o["camera"] = {"x": 0.25, "y": 0.25, "w": 0.5, "h": 0.5}
            f.write(json.dumps(o) + "\n")
    return path

def request(i, n, out, codec="h264", **kw):
    r = {"id": i, "cmd": "render", "project": project, "out": out, "format": "mp4", "codec": codec, "width": 480,
         "height": 270, "fps": 30, "quality": "standard", "frames": n,
         "segments": [{"src_start": 0.0, "src_end": n / 30.0, "speed": 1.0}], "params_file": params(n)}
    r.update(kw); return r

def boxes(data, start=0, end=None):
    end = len(data) if end is None else end
    while start + 8 <= end:
        size, kind = struct.unpack(">I4s", data[start:start + 8]); hdr = 8
        if size == 1: size = struct.unpack(">Q", data[start + 8:start + 16])[0]; hdr = 16
        if size == 0: size = end - start
        yield kind.decode("latin1"), start + hdr, start + size
        start += size

def probe(path):
    d = open(path, "rb").read()
    info = {}
    def walk(s, e):
        for k, a, b in boxes(d, s, e):
            if k in ("moov", "trak", "mdia", "minf", "stbl"): walk(a, b)
            elif k == "mvhd":
                v = d[a]; ts, dur = struct.unpack(">II", d[a + 12:a + 20]) if v == 0 else (struct.unpack(">I", d[a + 20:a + 24])[0], struct.unpack(">Q", d[a + 24:a + 32])[0])
                info["duration"] = dur / ts
            elif k == "stsd" and "codec" not in info:
                e0 = a + 8; info["codec"] = d[e0 + 4:e0 + 8].decode("latin1")
                info["size"] = list(struct.unpack(">HH", d[e0 + 32:e0 + 36]))
    walk(0, len(d)); return info

try:
    out = outdir + "/smoke.mp4"
    for f in (out, out + ".partial"):
        if os.path.exists(f): os.remove(f)
    send(request(1, 90, out))
    got = until(lambda l: "event" not in l)
    assert got[0].get("event") == "started" and got[0]["total_frames"] == 90, got[0]
    final = got[-1]
    assert final.get("ok") and final["path"] == out and final["frames"] == 90, final
    prog = [l for l in got if l.get("event") == "progress"]
    assert prog and prog[-1]["done_frames"] == 90, prog[-1:]
    assert not os.path.exists(out + ".partial"), ".partial left"
    info = probe(out)
    assert info["codec"] == "avc1" and info["size"] == [480, 270], info
    assert abs(info["duration"] - 3.0) <= 1 / 30.0, info
    hevc = outdir + "/smoke-hevc.mp4"
    send(request(2, 30, hevc, codec="hevc"))
    got = until(lambda l: "event" not in l)
    assert got[-1].get("ok"), got[-1]
    assert probe(hevc)["codec"] in ("hvc1", "hev1"), probe(hevc)

    cancelled = outdir + "/smoke-cancel.mp4"
    for f in (cancelled, cancelled + ".partial"):
        if os.path.exists(f): os.remove(f)
    send(request(3, 600, cancelled))
    until(lambda l: l.get("event") == "progress")
    send({"id": 4, "cmd": "cancel", "target": 3})
    got = until(lambda l: l.get("id") == 3 and "event" not in l)
    assert got[-1].get("error") == "cancelled", got[-1]
    assert not os.path.exists(cancelled) and not os.path.exists(cancelled + ".partial"), "cancel left output"

    send(request(5, 30, "relative.mp4"))
    assert until(lambda l: l.get("id") == 5)[-1].get("error") == "invalid"
    send(request(6, 30, outdir + "/odd.mp4", width=481))
    assert until(lambda l: l.get("id") == 6)[-1].get("error") == "invalid"
    send({"id": 7, "cmd": "cancel", "target": 99})
    assert until(lambda l: l.get("id") == 7)[-1].get("error") == "not-found"
    send({"id": 8, "cmd": "hello"})
    assert until(lambda l: l.get("id") == 8)[-1].get("ok")
    print("PASS: render MP4 (h264 480x270 duration %.3f s for 90 frames @30, hevc ok, progress events, cancel leaves nothing, bad requests invalid)" % info["duration"])
except Exception as e:
    print("FAIL: render MP4 (%s)" % (e,)); sys.exit(1)
finally:
    p.stdin.close()
    try: p.wait(timeout=10)
    except Exception: p.kill()
PY
      ); export_rc=$?
      echo "$export_out"
      [ "$export_rc" = 0 ] || fail=1
    else
      echo "SKIP: render MP4 (fixture movie could not be generated)"
    fi
    # T29: GIF export (`render` with format "gif"), same params file / progress / cancel / .partial
    # rules as MP4. The GIF is parsed here (no ffprobe, no ImageIO needed to read it): the
    # requested size, one frame per output frame, per-frame delays in centiseconds whose TOTAL is
    # round(frames * 100 / fps) (the rounding is spread, not accumulated), the NETSCAPE loop
    # extension with count 0 (forever), no `.partial`. Limits (S16): width 1920, fps 30 and a clip
    # over the memory budget are `invalid`; cancel leaves no output. Output: target/capture-spike/export-smoke/.
    if [ -x "$rdir/gen" ] && [ -d "$edir" ]; then
      gif_out=$(python3 - "$helper" "$edir" "$PWD/target/capture-spike/export-smoke" <<'PY'
import json, os, queue, struct, subprocess, sys, threading, time

helper, project, outdir = sys.argv[1:4]
p = subprocess.Popen([helper], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
lines = queue.Queue()
threading.Thread(target=lambda: [lines.put(json.loads(l)) for l in p.stdout], daemon=True).start()

def send(o):
    p.stdin.write(json.dumps(o) + "\n"); p.stdin.flush()

def until(pred, timeout=90):
    got, end = [], time.time() + timeout
    while time.time() < end:
        try: l = lines.get(timeout=1)
        except queue.Empty: continue
        got.append(l)
        if pred(l): return got
    raise AssertionError("timeout, got %r" % got[-3:])

def params(n, fps):
    path = project + "/.cache/gif-params-%d.ndjson" % n
    with open(path, "w") as f:
        for i in range(n):
            o = {"t": min(i / float(fps), 2.96)}
            if n // 3 <= i < 2 * n // 3: o["camera"] = {"x": 0.25, "y": 0.25, "w": 0.5, "h": 0.5}
            f.write(json.dumps(o) + "\n")
    return path

def request(i, n, out, width=480, fps=15, **kw):
    r = {"id": i, "cmd": "render", "project": project, "out": out, "format": "gif", "codec": "h264", "width": width,
         "height": width * 9 // 16 // 2 * 2, "fps": fps, "quality": "standard", "frames": n,
         "segments": [{"src_start": 0.0, "src_end": n / float(fps), "speed": 1.0}], "params_file": params(n, fps)}
    r.update(kw); return r

def parse_gif(path):
    d = open(path, "rb").read()
    # ImageIO writes the header "GIF87a" yet uses the 89a extensions (delay, loop): decoders accept it
    assert d[:6] in (b"GIF89a", b"GIF87a"), d[:6]
    w, h, flags = struct.unpack("<HHB", d[6:11])
    i = 13 + (3 * 2 ** ((flags & 7) + 1) if flags & 0x80 else 0)
    info = {"size": [w, h], "delays": [], "loop": None, "frames": 0}
    def skip(i):
        while d[i]: i += d[i] + 1
        return i + 1
    while i < len(d):
        b = d[i]
        if b == 0x21:
            if d[i + 1] == 0xF9: info["delays"].append(struct.unpack("<H", d[i + 4:i + 6])[0])
            elif d[i + 1] == 0xFF and d[i + 3:i + 14] == b"NETSCAPE2.0": info["loop"] = struct.unpack("<H", d[i + 16:i + 18])[0]
            i = skip(i + 2)
        elif b == 0x2C:
            info["frames"] += 1
            packed = d[i + 9]
            i += 10 + (3 * 2 ** ((packed & 7) + 1) if packed & 0x80 else 0) + 1
            i = skip(i)
        elif b == 0x3B: break
        else: raise AssertionError("bad GIF byte %x at %d" % (b, i))
    return info

try:
    out = outdir + "/smoke.gif"
    for f in (out, out + ".partial"):
        if os.path.exists(f): os.remove(f)
    send(request(1, 45, out))
    got = until(lambda l: "event" not in l)
    assert got[0].get("event") == "started" and got[0]["total_frames"] == 45, got[0]
    final = got[-1]
    assert final.get("ok") and final["path"] == out and final["frames"] == 45, final
    prog = [l for l in got if l.get("event") == "progress"]
    assert prog and prog[-1]["done_frames"] == 45, prog[-1:]
    assert not os.path.exists(out + ".partial"), ".partial left"
    g = parse_gif(out)
    assert g["size"] == [480, 270] and g["frames"] == 45 and len(g["delays"]) == 45, g
    assert g["loop"] == 0, "no NETSCAPE loop-forever extension"
    assert sum(g["delays"]) == 300 and set(g["delays"]) == {6, 7}, g["delays"]
    assert abs(final["duration"] - 3.0) < 1e-9
    # 960 px at 10 fps (7 cs rounding is not an issue at 10 fps: all delays 10)
    big = outdir + "/smoke-960.gif"
    send(request(2, 30, big, width=960, fps=10))
    got = until(lambda l: "event" not in l)
    assert got[-1].get("ok"), got[-1]
    g2 = parse_gif(big)
    assert g2["size"] == [960, 540] and g2["frames"] == 30 and set(g2["delays"]) == {10}, g2

    cancelled = outdir + "/smoke-cancel.gif"
    for f in (cancelled, cancelled + ".partial"):
        if os.path.exists(f): os.remove(f)
    send(request(3, 300, cancelled, width=960))
    until(lambda l: l.get("event") == "progress")
    send({"id": 4, "cmd": "cancel", "target": 3})
    got = until(lambda l: l.get("id") == 3 and "event" not in l)
    assert got[-1].get("error") == "cancelled", got[-1]
    assert not os.path.exists(cancelled) and not os.path.exists(cancelled + ".partial"), "cancel left output"

    send(request(5, 30, outdir + "/wide.gif", width=962))
    r = until(lambda l: l.get("id") == 5)[-1]
    assert r.get("error") == "invalid" and "960" in r.get("message", ""), r
    send(request(6, 30, outdir + "/fast.gif", fps=30))
    r = until(lambda l: l.get("id") == 6)[-1]
    assert r.get("error") == "invalid" and "15 fps" in r.get("message", ""), r
    send(request(7, 400, outdir + "/long.gif", width=960))
    r = until(lambda l: l.get("id") == 7)[-1]
    assert r.get("error") == "invalid" and "too long" in r.get("message", ""), r
    assert not any(os.path.exists(outdir + "/" + n) for n in ("wide.gif", "fast.gif", "long.gif"))
    send({"id": 8, "cmd": "hello"})
    assert until(lambda l: l.get("id") == 8)[-1].get("ok")
    print("PASS: render GIF (480x270 45 frames, delays %s.. total 3.00 s, loop forever, 960 px at 10 fps, cancel leaves nothing, width/fps/length limits invalid)" % g["delays"][:6])
except Exception as e:
    print("FAIL: render GIF (%s)" % (e,)); sys.exit(1)
finally:
    p.stdin.close()
    try: p.wait(timeout=10)
    except Exception: p.kill()
PY
      ); gif_rc=$?
      echo "$gif_out"
      [ "$gif_rc" = 0 ] || fail=1
    else
      echo "SKIP: render GIF (swiftc/ImageIO fixture not available)"
    fi
    if [ "$keep" = 1 ] && [ -x "$rdir/gen" ]; then
      kd=target/capture-spike/editor-fixture/Take.ferrocast
      mkdir -p "$kd"
      if "$rdir/gen" "$kd" 90 && python3 - "$kd" <<'PY'
import json, shutil, sys
d = sys.argv[1]
p = json.load(open("crates/core/fixtures/render_project.json"))
p["duration"] = 3.0
p["trims"] = [{"start": 0.0, "end": 3.0}]
json.dump(p, open(d + "/project.json", "w"), indent=1)
shutil.copy("crates/core/fixtures/render_events.json", d + "/events.json")
PY
      then
        echo "KEPT: editor fixture project at $kd"
      else
        echo "FAIL: --keep could not write the editor fixture"; fail=1
      fi
    fi
    rm -rf "$rdir"
  fi
fi

# P4 (T31): `devices`, camera / microphone permission errors and the granted paths, system audio.
# - devices: well-formed JSON, needs no grant (names may be empty without one). PASS.
# - camera / microphone NOT authorized (checked with the helper's own `permissions` command, which
#   never prompts unless asked): record.start answers the structured permission error, exit 1 and
#   leaves NO file. When authorized, 2 s are recorded with that source and its duration + offset must
#   match the screen take within 0.1 s; SKIP when Screen Recording is not granted.
# - system audio rides the screen stream and needs only Screen Recording: 3 s are recorded and
#   system.m4a must exist with a duration + offset within 0.1 s of the screen take. With --sound a
#   short system sound plays (afplay) and the file must not be silent (RMS read by a tiny swiftc probe).
if [ "$sck" != True ]; then
  echo "SKIP: P4 sources (ScreenCaptureKit unavailable on this macOS)"
else
  p4dir=$(mktemp -d)
  rms_probe=
  if [ "$sound" = 1 ] && command -v afplay >/dev/null 2>&1 && xcrun --find swiftc >/dev/null 2>&1; then
    cat > "$p4dir/rms.swift" <<'SW'
import AVFoundation
import Foundation
let f = try AVAudioFile(forReading: URL(fileURLWithPath: CommandLine.arguments[1]))
let buf = AVAudioPCMBuffer(pcmFormat: f.processingFormat, frameCapacity: 16384)!
var sum = 0.0, n = 0.0
while f.framePosition < f.length {
    try f.read(into: buf)
    for c in 0..<Int(f.processingFormat.channelCount) { let p = buf.floatChannelData![c]
        for i in 0..<Int(buf.frameLength) { let v = Double(p[i]); sum += v * v; n += 1 } }
}
print(sqrt(sum / max(n, 1)))
SW
    swiftc -O "$p4dir/rms.swift" -o "$p4dir/rms" 2>/dev/null && rms_probe="$p4dir/rms"
  fi
  p4_out=$(python3 - "$helper" "${main_display:-1}" "$p4dir" "$sound" "$rms_probe" <<'PY'
import json, os, struct, subprocess, sys, time

helper, display, base, sound, rms_probe = sys.argv[1], int(sys.argv[2]), sys.argv[3], sys.argv[4] == "1", sys.argv[5]

def duration(path):
    """Duration in seconds from moov/mvhd (mov and m4a)."""
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

class Session:
    def __init__(self):
        self.p = subprocess.Popen([helper], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, bufsize=1)
        self.events = []
    def call(self, o):
        self.p.stdin.write(json.dumps(o) + "\n"); self.p.stdin.flush()
        while True:
            line = self.p.stdout.readline()
            if not line:
                return None
            x = json.loads(line)
            if "event" in x:
                self.events.append(x); continue
            if x.get("id") == o["id"]:
                return x
    def close(self):
        try: self.p.stdin.close()
        except Exception: pass
        try: self.p.wait(timeout=10)
        except Exception: self.p.kill()
        return self.p.returncode

def start(project, **extra):
    o = {"id": 1, "cmd": "record.start", "project": project, "file": "screen.mov",
         "target": {"kind": "display", "display": display}, "fps": 30}
    o.update(extra)
    return o

failed = False
def bad(msg):
    global failed
    failed = True
    print("FAIL: " + msg)

# devices
s = Session()
r = s.call({"id": 1, "cmd": "devices"})
code = s.close()
try:
    assert r and r.get("ok") is True and code == 0, (r, code)
    assert isinstance(r["cameras"], list) and isinstance(r["microphones"], list)
    for c in r["cameras"]:
        assert isinstance(c["id"], str) and c["id"] and isinstance(c["name"], str) and c["position"] in ("front", "back", "unspecified") and isinstance(c["default"], bool)
    for m in r["microphones"]:
        assert isinstance(m["id"], str) and m["id"] and isinstance(m["name"], str) and isinstance(m["default"], bool)
    assert sum(c["default"] for c in r["cameras"]) <= 1 and sum(m["default"] for m in r["microphones"]) <= 1
    print("PASS: devices (%d cameras, %d microphones, names %s)" % (len(r["cameras"]), len(r["microphones"]),
          "present" if all(c["name"] for c in r["cameras"]) and all(m["name"] for m in r["microphones"]) else "empty without a grant"))
except Exception as e:
    bad("devices (%r, exit %r)" % (e, code))

# permission states (never prompts without a `request`)
s = Session()
perm = s.call({"id": 1, "cmd": "permissions"})
s.close()
state = {"camera": perm.get("camera"), "microphone": perm.get("microphone")} if perm and perm.get("ok") else {}
if not state:
    bad("permissions -> %r" % (perm,))

def screen_granted():
    s = Session()
    r = s.call({"id": 1, "cmd": "shareable"})
    s.close()
    return bool(r and r.get("ok"))

can_record = screen_granted()
n = 0
for kind, source, code_name, granted_file in (("camera", "camera", "permission-camera", "camera.mov"), ("microphone", "microphone", "permission-microphone", "mic.m4a")):
    n += 1
    project = os.path.join(base, "%s.ferrocast" % kind)
    if not can_record:
        print("SKIP: %s (Screen Recording is not granted: the helper answers permission-screen first)" % kind)
        continue
    if state.get(kind) != "authorized":
        s = Session()
        r = s.call(start(project, **{source: {}}))
        code = s.close()
        files = sorted(os.listdir(project)) if os.path.isdir(project) else []
        if r and r.get("error") == code_name and code == 1 and not files:
            print("PASS: %s denied (state %s): structured %s, exit 1, no file" % (kind, state.get(kind), code_name))
            print("SKIP: %s recording (not authorized; the granted path needs the user's permission)" % kind)
        else:
            bad("%s denied -> %r, exit %r, files %r" % (kind, r, code, files))
        continue
    s = Session()
    r = s.call(start(project, **{source: {}}))
    if not r or not r.get("ok"):
        bad("%s start -> %r" % (kind, r)); s.close(); continue
    time.sleep(2.0)
    r2 = s.call({"id": 2, "cmd": "record.stop"})
    s.close()
    try:
        src = r2["sources"]
        key = "camera" if kind == "camera" else "mic"
        info = src[key]
        screen_d = duration(os.path.join(project, "screen.mov"))
        d = duration(os.path.join(project, granted_file))
        assert info["file"] == granted_file and abs(d + info["offset"] - screen_d) < 0.1, (d, info, screen_d)
        assert not any(f.endswith(".partial") for f in os.listdir(project))
        print("PASS: %s recording (file %.3f s + offset %.3f s vs screen %.3f s%s)" % (kind, d, info["offset"], screen_d, ", size %s, %s frames" % (info["size"], info["frames"]) if kind == "camera" else ""))
    except Exception as e:
        bad("%s recording (%r, reply %r)" % (kind, e, r2))

# system audio
project = os.path.join(base, "system.ferrocast")
if not can_record:
    print("SKIP: system audio (Screen Recording is not granted)")
else:
    s = Session()
    r = s.call(start(project, system_audio=True))
    if not r or not r.get("ok"):
        bad("system audio start -> %r" % (r,)); s.close()
    else:
        time.sleep(0.8)
        if sound and rms_probe:
            subprocess.Popen(["afplay", "/System/Library/Sounds/Glass.aiff"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(2.2)
        r2 = s.call({"id": 2, "cmd": "record.stop"})
        code = s.close()
        try:
            info = r2["sources"]["system"]
            screen_d = duration(os.path.join(project, "screen.mov"))
            path = os.path.join(project, "system.m4a")
            d = duration(path)
            assert code == 0 and info["file"] == "system.m4a" and r2["sources"]["screen"] == {"file": "screen.mov", "offset": 0}, (code, r2)
            assert abs(d + info["offset"] - screen_d) < 0.1, (d, info, screen_d)
            assert not any(f.endswith(".partial") for f in os.listdir(project))
            extra = ""
            if sound and rms_probe:
                rms = float(subprocess.check_output([rms_probe, path], text=True).strip())
                assert rms > 0.002, "silent system audio (rms %.6f)" % rms
                extra = ", rms %.4f" % rms
            print("PASS: system audio (file %.3f s + offset %.3f s vs screen %.3f s%s)" % (d, info["offset"], screen_d, extra))
            if not (sound and rms_probe):
                print("SKIP: system audio non-silence (pass --sound to play one short system sound and check the RMS)")
        except Exception as e:
            bad("system audio (%r, reply %r)" % (e, r2))
sys.exit(1 if failed else 0)
PY
  ); p4_rc=$?
  echo "$p4_out"
  [ "$p4_rc" = 0 ] || fail=1
  rm -rf "$p4dir"
fi

# Unknown command: structured error, still protocol-only stdout.
run '{"id":3,"cmd":"nope"}'
[ "$(field error)" = invalid ] && echo "PASS: unknown command rejected" || bad "unknown command rc=$rc out=$out"

[ "$fail" = 0 ] && echo "capture-smoke: OK"
exit "$fail"
