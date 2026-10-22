#!/bin/bash
# Print the CHANGELOG.md section of one version (without its heading), for the GitHub release body.
#   scripts/release_notes.sh X.Y.Z [CHANGELOG.md]
# Falls back to a pointer to the changelog when the version has no section.
set -euo pipefail
version=${1:?usage: release_notes.sh X.Y.Z [CHANGELOG.md]}
file=${2:-$(dirname "$0")/../CHANGELOG.md}
body=$(awk -v v="$version" '
  /^## \[/ { if (on) exit; if (index($0, "## [" v "]") == 1) { on = 1; next } }
  on { print }
' "$file" | sed -e :a -e '/^\n*$/{$d;N;ba' -e '}')
if [ -n "$(printf '%s' "$body" | tr -d '[:space:]')" ]; then
  printf '%s\n' "$body"
else
  echo "See CHANGELOG.md for the changes in $version."
fi
