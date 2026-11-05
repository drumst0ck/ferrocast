#!/bin/bash
# Cut a release: bump the version, close the CHANGELOG's Unreleased section, commit and tag.
#   scripts/release.sh X.Y.Z [--dry-run] [--no-push]
# The tag push triggers .github/workflows/release.yml. A version with a suffix (X.Y.Z-rc.1) is
# published as a prerelease, which the in-app updater ignores.
# Refuses: a dirty tree, a branch other than master (RELEASE_ALLOW_BRANCH=1 overrides), a tag that
# exists, or a version that is not X.Y.Z[-suffix].
set -euo pipefail
cd "$(dirname "$0")/.."
version=${1:?usage: release.sh X.Y.Z [--dry-run] [--no-push]}
shift || true
dry=0; push=1
for a in "$@"; do
  case "$a" in
    --dry-run) dry=1 ;;
    --no-push) push=0 ;;
    *) echo "unknown option: $a" >&2; exit 2 ;;
  esac
done
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$ ]] || { echo "version must look like 1.2.3 or 1.2.3-rc.1" >&2; exit 2; }
tag="v$version"
branch=$(git rev-parse --abbrev-ref HEAD)
[ "$branch" = master ] || [ "${RELEASE_ALLOW_BRANCH:-}" = 1 ] || { echo "release from master (on $branch); RELEASE_ALLOW_BRANCH=1 overrides" >&2; exit 1; }
[ -z "$(git status --porcelain)" ] || { echo "working tree is not clean" >&2; exit 1; }
git rev-parse -q --verify "refs/tags/$tag" >/dev/null && { echo "tag $tag already exists" >&2; exit 1; }
grep -q '^## \[Unreleased\]' CHANGELOG.md || { echo "CHANGELOG.md has no ## [Unreleased] section" >&2; exit 1; }
current=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
today=$(date +%Y-%m-%d)
echo "release $current -> $version ($tag, $today) on $branch; push=$push"
if [ "$dry" = 1 ]; then
  echo "dry run: would edit Cargo.toml, Cargo.lock, CHANGELOG.md, commit 'release: $tag' and tag $tag"
  exit 0
fi
# Any failure before the release commit (cargo update, a git step) puts the files back exactly as
# they were, so a rerun is not refused for a dirty tree.
backup=$(mktemp -d)
files=(Cargo.toml CHANGELOG.md)
[ -f Cargo.lock ] && files+=(Cargo.lock)
[ -d web ] && files+=(web/CHANGELOG.md)
for f in "${files[@]}"; do mkdir -p "$backup/$(dirname "$f")"; cp -p "$f" "$backup/$f"; done
committed=0
cleanup() {
  if [ "$committed" = 0 ]; then
    git read-tree HEAD 2>/dev/null || true
    for f in "${files[@]}"; do cp -p "$backup/$f" "$f"; done
    echo "release failed: Cargo.toml, Cargo.lock and the CHANGELOGs were restored" >&2
  fi
  [ -z "$backup" ] || command rm -r "$backup"
}
trap cleanup EXIT
# awk, not sed: the macOS and GNU sed dialects differ for first-match and newline handling
awk -v v="$version" '!done && /^version = "/ { print "version = \"" v "\""; done = 1; next } { print }' Cargo.toml > "$backup/Cargo.toml.new" && mv "$backup/Cargo.toml.new" Cargo.toml
# keep a fresh empty Unreleased section above the new version heading
awk -v v="$version" -v d="$today" '/^## \[Unreleased\]/ && !done { print; print ""; print "## [" v "] - " d; done = 1; next } { print }' CHANGELOG.md > "$backup/CHANGELOG.md.new" && mv "$backup/CHANGELOG.md.new" CHANGELOG.md
if [ -f Cargo.lock ]; then cargo update --workspace --offline >/dev/null; fi
# the website reads web/CHANGELOG.md (changelog pages, sitemap, changelog:push): keep it identical
if [ -d web ]; then cp CHANGELOG.md web/CHANGELOG.md; git add web/CHANGELOG.md; fi
git add Cargo.toml CHANGELOG.md
[ -f Cargo.lock ] && git add Cargo.lock
git commit -qm "release: $tag"
committed=1
git tag -a "$tag" -m "Ferrocast $version"
if [ "$push" = 1 ]; then
  if ! { git push origin "$branch" && git push origin "$tag"; }; then
    echo "push failed: the commit and tag $tag exist locally. Retry: git push origin $branch && git push origin $tag" >&2
    exit 1
  fi
  echo "pushed $tag: the Release workflow is running"
else
  echo "created $tag locally (not pushed)"
fi
