#!/bin/bash
# Point Formula/ferrocast.rb at a published tag and refresh its sha256.
# The tap repo drumst0ck/homebrew-ferrocast syncs itself; this copy serves taps added from the old URL.
#   scripts/update_formula.sh OWNER/REPO [version]     (version defaults to Cargo.toml's)
# Run it after pushing the tag vX.Y.Z; it downloads GitHub's source archive to hash it.
set -euo pipefail
cd "$(dirname "$0")/.."
repo=${1:?usage: update_formula.sh OWNER/REPO [version]}
version=${2:-$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')}
url="https://github.com/$repo/archive/refs/tags/v$version.tar.gz"
tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT
curl -fsSL "$url" -o "$tmp" || { echo "cannot download $url (is the tag pushed?)" >&2; exit 1; }
sha=$(shasum -a 256 "$tmp" | cut -d' ' -f1)
python3 - "$repo" "$url" "$sha" <<'PY'
import re, sys, pathlib
repo, url, sha = sys.argv[1:]
p = pathlib.Path("Formula/ferrocast.rb"); s = p.read_text()
s = re.sub(r'homepage "[^"]*"', f'homepage "https://github.com/{repo}"', s)
s = re.sub(r'url "[^"]*"', f'url "{url}"', s, count=1)
s = re.sub(r'sha256 "[0-9a-f]{64}"', f'sha256 "{sha}"', s)
s = re.sub(r'head "[^"]*"', f'head "https://github.com/{repo}.git"', s)
p.write_text(s)
PY
echo "Formula updated: $url"
echo "sha256 $sha"
