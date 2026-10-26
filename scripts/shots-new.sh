#!/bin/bash
# Regenerates the pictures of the screen recorder and the Notes window, made by the app itself
# (never drawn): the recording editor, the Recordings list and the floating Notes window.
#
# usage: scripts/shots-new.sh            # all pictures
#        scripts/shots-new.sh editor     # only these ids: editor | list | notes
#
# Outputs
#   web/public/img/record-editor.jpg   1500x954 JPEG q82   FERROCAST_VIEW=editor on a fixture recording
#   web/public/img/record-list.jpg     1500x950 JPEG q82   FERROCAST_VIEW=recordings
#   web/public/img/notes.jpg           1500x950 JPEG q82   crates/macos/examples/notes_demo.rs --shot
#   resources/tour/record.png          540x342 PNG         same picture as record-editor.jpg (welcome tour)
#   resources/tour/notes.png           540x342 PNG         same picture as notes.jpg (welcome tour)
#
# How: scripts/make_recording_fixture.sh builds a `.ferrocast` project (screen.mov from
# web/public/img/home.jpg with ffmpeg, events.json with three clicks, project.json, thumb.png).
# The editor shot runs the app's own screenshot mode (FERROCAST_SCREENSHOT, see dev_screenshot in
# crates/app/src/draw.rs) with the playhead at FERROCAST_EDITOR_T seconds; the preview frames come
# from the capture helper, which is copied next to a copy of the debug binary in the temp dir (the
# helper from dist/Ferrocast.app when present, else built with swiftc from crates/macos/swift/capture).
# The Notes shot snapshots the real Notes window holding a Markdown note.
#
# Needs (macOS): cargo, ffmpeg (brew install ffmpeg), python3, sips; swiftc only without dist/Ferrocast.app.
# Dark theme everywhere. Every shot runs with a private FERROCAST_DATA_DIR in the temp dir, so a
# running Ferrocast and your real data are untouched. No Screen Recording permission is needed.
set -u
cd "$(dirname "$0")/.." || exit 1
ROOT=$PWD
IMG=$ROOT/web/public/img
TOUR=$ROOT/resources/tour
EDITOR_T=2.0          # playhead of the editor shot: inside the first auto zoom
NOTES_SIZE=620x392    # points of the Notes window (same aspect as the 540x342 tour pictures)
JPG_Q=82
PNG_LIMIT=120000

need() { command -v "$1" >/dev/null 2>&1 || { echo "shots-new.sh: '$1' is required but not found. $2" >&2; exit 2; }; }
need cargo "Install Rust (https://rustup.rs)."
need ffmpeg "Install it with: brew install ffmpeg"
need python3 "Needed by make_recording_fixture.sh."
need sips "This script needs macOS."
need perl "Needed for the per-shot time limit."

WANT=("$@"); [ ${#WANT[@]} -eq 0 ] && WANT=(editor list notes)

echo "== building the app and the Notes example"
cargo build -p ferrocast >&2 || { echo "shots-new.sh: cargo build failed" >&2; exit 1; }
cargo build -p ferrocast-macos --example notes_demo >&2 || { echo "shots-new.sh: example build failed" >&2; exit 1; }

T=$(mktemp -d /tmp/ferrocast-shots.XXXXXX)
trap 'rm -rf "$T"' EXIT
mkdir -p "$T/bin" "$T/png" "$T/data"

# the app next to its capture helper: the editor renders its preview through the helper
cp "$ROOT/target/debug/ferrocast" "$T/bin/ferrocast"
if [ -x "$ROOT/dist/Ferrocast.app/Contents/MacOS/ferrocast-capture" ]; then
  cp "$ROOT/dist/Ferrocast.app/Contents/MacOS/ferrocast-capture" "$T/bin/ferrocast-capture"
else
  need swiftc "Install the Xcode command line tools."
  swiftc -O -target arm64-apple-macos13.0 -Xlinker -weak_framework -Xlinker ScreenCaptureKit \
    "$ROOT"/crates/macos/swift/capture/*.swift -o "$T/bin/ferrocast-capture" || { echo "shots-new.sh: helper build failed" >&2; exit 1; }
fi

echo "== fixture recording"
PROJ=$("$ROOT/scripts/make_recording_fixture.sh" "$T/recordings") || exit 1

# shot <id> <png> VAR=value ...   (launcher/editor screenshot, three tries: a launch occasionally ends without a picture)
shot() {
  local id=$1 png=$2; shift 2
  local try pid i
  for try in 1 2 3; do
    rm -f "$png"
    mkdir -p "$T/data/$id.$try"   # the app needs an existing data dir
    # onboarded: a first launch would open the welcome tour with its intro animation instead of the shot
    printf '{"onboarded":true}' > "$T/data/$id.$try/settings.json"
    # backgrounded like scripts/shot.sh (a foreground launch never gets its frames), 60 s limit
    env FERROCAST_DATA_DIR="$T/data/$id.$try" FERROCAST_SCREENSHOT="$png" FERROCAST_THEME=dark "$@" "$T/bin/ferrocast" >/dev/null 2>&1 &
    pid=$!
    for i in $(seq 1 120); do kill -0 $pid 2>/dev/null || break; sleep 0.5; done
    kill $pid 2>/dev/null
    [ -s "$png" ] && return 0
  done
  echo "shots-new.sh: no screenshot for $id" >&2
  return 1
}

jpeg() { # png jpg width height
  sips -s format jpeg -s formatOptions $JPG_Q -z "$4" "$3" "$1" --out "$2" >/dev/null 2>&1 \
    || { command -v magick >/dev/null && magick "$1" -background white -flatten -resize "$3x$4!" -quality $JPG_Q "$2"; }
}
tourpng() { # png out
  sips -s format png -z 342 540 "$1" --out "$2" >/dev/null 2>&1
}

fail=0
for id in "${WANT[@]}"; do
  case $id in
    editor)
      shot editor "$T/png/editor.png" FERROCAST_VIEW=editor FERROCAST_EDITOR_T=$EDITOR_T FERROCAST_FILES="$PROJ" \
        && jpeg "$T/png/editor.png" "$IMG/record-editor.jpg" 1500 954 && tourpng "$T/png/editor.png" "$TOUR/record.png" || fail=1 ;;
    list)
      # two more recordings, so the list is not a single row (same clip, other names and dates)
      for extra in "Quarterly review walkthrough|2026-10-21T09:30:00Z" "Clipboard history bug repro|2026-10-17T16:45:00Z"; do
        name=${extra%%|*}; when=${extra##*|}
        cp -R "$PROJ" "$T/recordings/$name.ferrocast"
        python3 - "$T/recordings/$name.ferrocast/project.json" "$name" "$when" <<'PY'
import json, sys
f, name, when = sys.argv[1:]
p = json.load(open(f)); p["name"] = name; p["created"] = when
json.dump(p, open(f, "w"), indent=1)
PY
      done
      shot list "$T/png/list.png" FERROCAST_VIEW=recordings FERROCAST_FILES="$PROJ" \
        && jpeg "$T/png/list.png" "$IMG/record-list.jpg" 1500 950 || fail=1 ;;
    notes)
      cat > "$T/note.md" <<'NOTE'
# Call Ana about the offer

Ask for the revised numbers before Friday. The **start date** is still open and `offer-v2.pdf` has the new salary band.

- Budget and equity
- Start date
- Remote days

- [x] Book a slot in her calendar
- [ ] Send the signed offer back
NOTE
      rm -f "$T/png/notes.png"
      perl -e 'alarm 90; exec @ARGV' "$ROOT/target/debug/examples/notes_demo" --shot "$T/png/notes.png" --note-file "$T/note.md" --size "$NOTES_SIZE" >/dev/null 2>&1
      [ -s "$T/png/notes.png" ] || { echo "shots-new.sh: no Notes snapshot" >&2; fail=1; continue; }
      jpeg "$T/png/notes.png" "$IMG/notes.jpg" 1500 950 && tourpng "$T/png/notes.png" "$TOUR/notes.png" || fail=1 ;;
    *) echo "shots-new.sh: unknown id $id (editor | list | notes)" >&2; fail=1 ;;
  esac
done

echo; printf '%-34s %12s %10s\n' file size bytes
for f in "$IMG/record-editor.jpg" "$IMG/record-list.jpg" "$IMG/notes.jpg" "$TOUR/record.png" "$TOUR/notes.png"; do
  [ -f "$f" ] || continue
  printf '%-34s %12s %10d\n' "${f#"$ROOT"/}" "$(sips -g pixelWidth -g pixelHeight "$f" | awk '/pixel/{a=a (a?"x":"") $2} END{print a}')" "$(stat -f %z "$f")"
done
for f in "$TOUR/record.png" "$TOUR/notes.png"; do
  [ -f "$f" ] && [ "$(stat -f %z "$f")" -gt $PNG_LIMIT ] && { echo "shots-new.sh: $f is over $PNG_LIMIT bytes" >&2; fail=1; }
done
[ $fail -eq 0 ] || { echo "shots-new.sh: some pictures failed" >&2; exit 1; }
echo "ok"
