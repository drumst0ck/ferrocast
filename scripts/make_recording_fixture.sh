#!/bin/bash
# Builds a small, valid `.ferrocast` recording project for the dev screenshot modes
# (FERROCAST_VIEW=editor and FERROCAST_VIEW=recordings, see dev_screenshot in crates/app/src/draw.rs).
# The "screen" is the launcher picture web/public/img/home.jpg as a 12 s H.264 clip; the clicks and
# cursor path in events.json move over the real rows of that picture, so auto zoom finds three zooms.
#
# usage: scripts/make_recording_fixture.sh <output-folder>
#   creates <output-folder>/Ferrocast launcher demo.ferrocast/{screen.mov,events.json,project.json,thumb.png}
#   and prints the project path on stdout.
#
# Needs: ffmpeg (brew install ffmpeg), python3 (built in). Writes nothing outside <output-folder>.
set -eu
cd "$(dirname "$0")/.."
out=${1:?usage: scripts/make_recording_fixture.sh <output-folder>}
command -v ffmpeg >/dev/null 2>&1 || { echo "make_recording_fixture.sh: ffmpeg is required (brew install ffmpeg)" >&2; exit 2; }
command -v python3 >/dev/null 2>&1 || { echo "make_recording_fixture.sh: python3 is required" >&2; exit 2; }
src=web/public/img/home.jpg
[ -f "$src" ] || { echo "make_recording_fixture.sh: $src is missing" >&2; exit 1; }

proj=$out/Ferrocast\ launcher\ demo.ferrocast
mkdir -p "$proj"
# the picture is 1300x823 with rounded, transparent-white corners: crop 8 px off every side (even size for yuv420p)
W=1284; H=806; INSET=8
DUR=12

# screen.mov: a still picture as a constant-rate H.264 clip
ffmpeg -v error -y -loop 1 -framerate 30 -i "$src" -t "$DUR" -vf "crop=$W:$H:$INSET:$INSET,format=yuv420p" \
  -c:v libx264 -preset medium -crf 18 -movflags +faststart "$proj/screen.mov"
# thumb.png: the first frame, 480 px wide (the Recordings browser picture)
ffmpeg -v error -y -i "$proj/screen.mov" -frames:v 1 -vf scale=480:-2 "$proj/thumb.png"

python3 - "$proj" "$W" "$H" "$DUR" "$INSET" <<'PY'
import json, sys
proj, w, h, dur, inset = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), float(sys.argv[4]), int(sys.argv[5])

# the cursor rests, then glides to the next target in 0.8 s and clicks 0.1 s after arriving:
# Ask AI, Clipboard History, the Actions key (points of the original picture; the crop moves them by the inset). The long rests keep
# the three auto zooms apart.
start = (700 - inset, 520 - inset)
legs = [(0.5, 200 - inset, 190 - inset), (4.6, 180 - inset, 698 - inset), (8.6, 1245 - inset, 791 - inset)]   # (glide start, x, y)
clicks_at = [(t + 0.9, x, y) for t, x, y in legs]

def smooth(a):
    return a * a * (3 - 2 * a)

def at(t):
    x, y = start
    for t0, x1, y1 in legs:
        if t >= t0:
            a = smooth(min(1.0, (t - t0) / 0.8))
            x, y = x + (x1 - x) * a, y + (y1 - y) * a
    return x, y

cursor = []
t = 0.0
while t <= dur + 1e-9:
    x, y = at(t)
    cursor.append([round(t, 4), round(x, 2), round(y, 2), "arrow"])
    t += 1 / 30
clicks = []
for t, x, y in clicks_at:
    clicks.append({"t": t, "x": x, "y": y, "button": "left", "down": True})
    clicks.append({"t": round(t + 0.09, 3), "x": x, "y": y, "button": "left", "up": True})

events = {"version": 1, "screen": {"size": [w, h], "scale": 1, "display_frame": [0, 0, w, h]},
          "cursor": cursor, "clicks": clicks, "keys": [], "scroll": [], "windows": []}
# the same shape as crates/core/fixtures/render_zoom_project.json; no zooms, so the editor's auto zoom
# (capture.auto_zoom, on by default) derives them from the clicks
project = {"version": 1, "name": "Ferrocast launcher demo", "created": "2026-10-23T14:05:22Z", "duration": dur,
           "sources": {"screen": {"file": "screen.mov", "offset": 0.0}},
           "trims": [{"start": 0.0, "end": dur}], "speeds": [], "zooms": [],
           "cursor": {"style": "arrow", "size": 1.4, "smoothing": 0.5, "click_effect": "ring", "hide_idle_after": 3.0, "motion_blur": 0.5},
           "style": {"aspect": "16:9", "padding": 0.08, "radius": 18, "shadow": {"blur": 40, "opacity": 0.5, "dy": 16},
                     "background": {"kind": "gradient", "stops": ["#1f1c2c", "#928dab"], "angle": 135}},
           "audio": {"mic_gain": 1.0, "system_gain": 0.6, "mute": False},
           "export": {"format": "mp4", "codec": "h264", "size": [1920, 1080], "fps": 30}}
json.dump(events, open(proj + "/events.json", "w"), separators=(",", ":"))
json.dump(project, open(proj + "/project.json", "w"), indent=1)
PY
echo "$proj"
