#!/bin/bash
# Reproducible performance baseline for Ferrocast (odd/plans/performance-plan.md, P0). macOS only.
#
#   scripts/perf.sh              # measure target/release/ferrocast (built if missing)
#   PERF_BUILD=1 scripts/perf.sh # rebuild first
#   PERF_RUNS=7 PERF_IDLE=10 scripts/perf.sh
#
# Everything runs against a private FERROCAST_DATA_DIR with a generated fixture (1000 text
# clipboard entries + 50 image entries), so a running Ferrocast and your real data are untouched.
# Prints one markdown table; save two runs and `diff` them for before/after.
set -u
cd "$(dirname "$0")/.."
BIN=target/release/ferrocast
RUNS=${PERF_RUNS:-5}   # hidden starts for the startup medians
SETTLE=${PERF_SETTLE:-15}  # seconds to let a hidden start settle before the idle window
IDLE=${PERF_IDLE:-10}  # idle window in seconds (top samples + hidden-frame counter)

[ "${PERF_BUILD:-0}" = 1 ] || [ ! -x "$BIN" ] && cargo build --release -p ferrocast >&2
[ -x "$BIN" ] || { echo "no $BIN" >&2; exit 1; }
command -v python3 >/dev/null || { echo "python3 is needed for the fixture" >&2; exit 1; }

WORK=$(mktemp -d)
trap 'kill $(jobs -p) 2>/dev/null; rm -rf "$WORK"' EXIT

# ---- fixture: 1000 text entries + 50 image entries (real 800x600 PNGs) ----
FIX=$WORK/fixture
mkdir -p "$FIX/clipboard-images"
python3 - "$FIX" <<'EOF'
import json, struct, sys, time, uuid, zlib
d = sys.argv[1]; now = int(time.time())
def png(w, h, seed):
    raw = b"".join(b"\0" + bytes(((x * 3 + seed * 17) % 256, (y * 2 + seed * 5) % 256, (x + y + seed) % 256) [i % 3] for x in range(w) for i in range(3)) for y in range(h))
    def chunk(t, b): return struct.pack(">I", len(b)) + t + b + struct.pack(">I", zlib.crc32(t + b) & 0xffffffff)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, 6)) + chunk(b"IEND", b"")
ents = [{"id": str(uuid.uuid4()), "kind": "Text",
         "content": ("line %d of a fairly typical clipboard text entry with some words " % i) * 4,
         "copied_at": now - i * 60, "pinned": False, "source_app": "Safari", "times_copied": 1} for i in range(1000)]
for i in range(50):
    name = "img%02d.png" % i
    open("%s/clipboard-images/%s" % (d, name), "wb").write(png(800, 600, i))
    ents.append({"id": str(uuid.uuid4()), "kind": "Image", "content": name, "copied_at": now - i * 90,
                 "pinned": False, "source_app": "Preview", "times_copied": 1, "width": 800, "height": 600})
json.dump(ents, open(d + "/clipboard.json", "w"))
EOF

fresh_dir() { local d; d=$(mktemp -d "$WORK/d.XXXXXX"); cp -R "$FIX/." "$d"; echo "$d"; }
median() { sort -n | awk '{a[NR]=$1} END{ if (NR==0) print "n/a"; else printf "%.1f", (NR%2)?a[(NR+1)/2]:(a[NR/2]+a[NR/2+1])/2 }'; }
# "12.3ms" / "456.7µs" / "1.2s" / "528.2ms" -> milliseconds
to_ms() { awk '{ v=$0; if (v ~ /µs$/) { sub(/µs$/,"",v); print v/1000 } else if (v ~ /ms$/) { sub(/ms$/,"",v); print v+0 } else if (v ~ /ns$/) { sub(/ns$/,"",v); print v/1e6 } else { sub(/s$/,"",v); print v*1000 } }'; }
field() { grep -E "$2" "$1" | head -1 | sed -E "$3" | to_ms; }

# ---- 1. hidden start, N runs ----
: > "$WORK/gl"; : > "$WORK/new"; : > "$WORK/apps"
for _ in $(seq "$RUNS"); do
  d=$(fresh_dir)
  FERROCAST_DATA_DIR=$d FERROCAST_PROFILE=1 "$BIN" --hidden 2> "$d/log" & pid=$!
  sleep 3; kill $pid 2>/dev/null; wait $pid 2>/dev/null
  field "$d/log" 'window\+gl ready' 's/.*after //' >> "$WORK/gl"
  field "$d/log" '^App::new' 's/App::new //' >> "$WORK/new"
  field "$d/log" '^apps scan' 's/apps scan ([^ ]+) .*/\1/' >> "$WORK/apps"
done

# ---- 2. hidden idle: top (cpu, context switches/s), probe counters, footprint ----
d=$(fresh_dir)
FERROCAST_DATA_DIR=$d FERROCAST_PROFILE=1 "$BIN" --hidden 2> "$d/log" & pid=$!
sleep "$SETTLE"
footprint_start=$(vmmap -summary $pid 2>/dev/null | awk '/Physical footprint:/ {print $3; exit}')
top -l $((IDLE + 1)) -s 1 -pid $pid -stats pid,cpu,mem,csw 2>/dev/null | awk -v p=$pid '$1==p {print}' > "$WORK/top"
sleep 1
footprint_idle=$(vmmap -summary $pid 2>/dev/null | awk '/Physical footprint:/ {print $3; exit}')
kill $pid 2>/dev/null; wait $pid 2>/dev/null
# top prints cumulative csw: per-second rate = (last - first) / (n - 1); the first line is since-boot noise for cpu
cpu=$(tail -n +2 "$WORK/top" | awk '{s+=$2; n++} END{ if (n) printf "%.2f", s/n; else print "n/a" }')
csw=$(awk '{gsub(/[+-]/,"",$4); v[NR]=$4+0} END{ if (NR>1) printf "%.0f", (v[NR]-v[1])/(NR-1); else print "n/a" }' "$WORK/top")
# the probe prints one line per 10 s window; the first window holds the startup frames, so it is skipped
hidden_frames=$(grep -E '^frames in' "$d/log" | tail -n +2 | sed -E 's/.*hidden=//' | awk '{s+=$1} END{print s+0}')
hidden_startup=$(grep -E '^frames in' "$d/log" | head -1 | sed -E 's/.*hidden=//')
frames_windows=$(grep -E '^frames in' "$d/log" | tail -n +2 | wc -l | tr -d ' ')
slow_hidden=$(grep -cE '^slow frame .* visible=false' "$d/log")

# ---- 3. views: screenshot mode renders a few frames and exits ----
view() { # name, env...
  local name=$1; shift
  local d; d=$(fresh_dir)
  env "$@" FERROCAST_DATA_DIR=$d FERROCAST_PROFILE=1 FERROCAST_SCREENSHOT=$d/x.png "$BIN" 2> "$d/log" >/dev/null & local pid=$!
  for _ in $(seq 80); do kill -0 $pid 2>/dev/null || break; sleep 0.25; done
  kill $pid 2>/dev/null; wait $pid 2>/dev/null
  local sec icons slow
  sec=$(grep -E '^sections ' "$d/log" | sed -E 's/^sections ([^ ]+) .*/\1/' | to_ms | sort -n | tail -1)
  icons=$(grep -E '^icon sync ' "$d/log" | awk '{print $NF}' | to_ms | awk '{s+=$1} END{printf "%.0f", s}')
  slow=$(grep -cE '^slow frame' "$d/log")
  printf '| view: %s | sections max %s ms; icon sync %s ms (%s); slow frames %s |\n' "$name" "${sec:-n/a}" "$icons" "$(grep -cE '^icon sync ' "$d/log") renders" "$slow"
}

echo "## Ferrocast perf baseline — $(git rev-parse --short HEAD 2>/dev/null) — $(sysctl -n machdep.cpu.brand_string 2>/dev/null) — $(sw_vers -productVersion 2>/dev/null)"
echo
echo "| Metric | Result |"
echo "|---|---|"
echo "| hidden start: window+gl ready, median of $RUNS | $(median < "$WORK/gl") ms |"
echo "| hidden start: App::new, median | $(median < "$WORK/new") ms |"
echo "| hidden start: apps scan, median (153 apps on the dev Mac) | $(median < "$WORK/apps") ms |"
echo "| hidden idle (${SETTLE}s after start, ${IDLE}s window): CPU | ${cpu} % |"
echo "| hidden idle: context switches/s | ${csw} |"
echo "| hidden: frames drawn in the first 10 s after start | ${hidden_startup:-n/a} |"
echo "| hidden idle: frames drawn while hidden after that ($frames_windows windows of 10 s) | ${hidden_frames} |"
echo "| hidden idle: slow frames while hidden | ${slow_hidden} |"
echo "| physical footprint after ${SETTLE}s hidden / after idle window | ${footprint_start:-n/a} / ${footprint_idle:-n/a} |"
view "root (empty query)" FERROCAST_QUERY=
view "root query 'saf'" FERROCAST_QUERY=saf
view "clipboard, 1000 text + 50 images" FERROCAST_VIEW=clipboard
view "clipboard, query 'line 5'" FERROCAST_VIEW=clipboard FERROCAST_QUERY="line 5"
view "emoji, empty" FERROCAST_VIEW=emoji
view "emoji, query 'smile'" FERROCAST_VIEW=emoji FERROCAST_QUERY=smile
view "settings" FERROCAST_VIEW=settings
