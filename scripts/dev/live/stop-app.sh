#!/bin/bash
# Kills only the debug app started by start-app.sh (exact binary path).
set -euo pipefail
root="$(cd "$(dirname "$0")/../../.." && pwd)"
pkill -f "${CARGO_TARGET_DIR:-$root/target}/debug/ferrocast" && echo stopped || echo "not running"
