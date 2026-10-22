#!/bin/bash
# Checks scripts/release.sh and scripts/release_notes.sh in a scratch repository. No network, no push.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
fail() { echo "FAIL: $*" >&2; exit 1; }

cd "$tmp"
git init -q -b master .
git config user.email t@t; git config user.name t
mkdir scripts
cp "$root/scripts/release.sh" "$root/scripts/release_notes.sh" scripts/
cat > Cargo.toml <<'T'
[workspace.package]
version = "0.1.0"
T
cat > CHANGELOG.md <<'T'
# Changelog

## [Unreleased]

### Added
- A new thing.

## [0.1.0] - 2026-10-15

### Added
- First release.
T
git add -A && git commit -qm init

# refusals
scripts/release.sh 1.2 --no-push >/dev/null 2>&1 && fail "accepted a bad version"
echo dirty > dirty.txt
scripts/release.sh 0.2.0 --no-push >/dev/null 2>&1 && fail "accepted a dirty tree"
rm dirty.txt
git checkout -q -b topic
scripts/release.sh 0.2.0 --no-push >/dev/null 2>&1 && fail "released from a non-master branch"
git checkout -q master

# dry run changes nothing
scripts/release.sh 0.2.0 --dry-run | grep -q "dry run" || fail "dry run did not say so"
[ -z "$(git status --porcelain)" ] || fail "dry run touched the tree"
git rev-parse -q --verify refs/tags/v0.2.0 >/dev/null && fail "dry run created a tag"

# real run, no push
scripts/release.sh 0.2.0 --no-push >/dev/null
grep -q '^version = "0.2.0"' Cargo.toml || fail "version not bumped"
grep -q '^## \[Unreleased\]$' CHANGELOG.md || fail "Unreleased heading lost"
grep -q '^## \[0.2.0\] - [0-9-]*$' CHANGELOG.md || fail "no dated 0.2.0 heading"
[ "$(git log -1 --format=%s)" = "release: v0.2.0" ] || fail "wrong commit message"
git rev-parse -q --verify refs/tags/v0.2.0 >/dev/null || fail "tag missing"
scripts/release.sh 0.2.0 --no-push >/dev/null 2>&1 && fail "released the same tag twice"

# release notes: the 0.2.0 section holds the entry that was Unreleased; unknown version falls back
scripts/release_notes.sh 0.2.0 | grep -q "A new thing." || fail "notes missing the entry"
scripts/release_notes.sh 0.2.0 | grep -q "First release" && fail "notes leaked the previous version"
scripts/release_notes.sh 9.9.9 | grep -q "See CHANGELOG.md" || fail "no fallback for an unknown version"
echo "release scripts: ok"
