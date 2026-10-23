#!/bin/bash
# Starts the real debug app detached (nohup, never `open`) with an isolated data dir.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../../.." && pwd)"
bin="${CARGO_TARGET_DIR:-$root/target}/debug/ferrocast"
[ -x "$bin" ] || { echo "missing $bin (cargo build -p ferrocast first)" >&2; exit 1; }
live="$root/target/dev-live"
data="${LIVE_DATA:-$live/app-data}"
mkdir -p "$data" "$live/caps" "$live/recs"
# `onboarded: true` is what skips the first-run tour (crates/app/src/tour.rs first_run).
if [ ! -f "$data/settings.json" ]; then
  cat > "$data/settings.json" <<JSON
{"onboarded": true, "capture": {"folder": "$live/caps", "recordings_folder": "$live/recs"}}
JSON
fi
if [ -n "${FERROCAST_HELPER:-}" ]; then
  cp "$FERROCAST_HELPER" "$(dirname "$bin")/$(basename "$FERROCAST_HELPER")"
fi
FERROCAST_DATA_DIR="$data" nohup "$bin" > "$live/app.out" 2>&1 < /dev/null &
echo "$!"
