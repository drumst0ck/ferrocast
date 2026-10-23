#!/bin/bash
# Compiles the live tools into target/dev-live/ when missing or older than the source.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
out="$(cd "$here/../../.." && pwd)/target/dev-live"
mkdir -p "$out"
for t in poster winlist permcheck permask; do
  if [ ! -x "$out/$t" ] || [ "$here/$t.swift" -nt "$out/$t" ]; then
    xcrun swiftc -O "$here/$t.swift" -o "$out/$t" >&2
  fi
  echo "$out/$t"
done
