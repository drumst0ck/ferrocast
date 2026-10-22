---
title: "Releasing"
description: "How to cut a Ferrocast release: one script, one tag, and a GitHub Actions workflow that publishes the DMG, the update ZIP and the Homebrew formula bump."
sidebar:
  order: 4
---
A release is a tag. `scripts/release.sh X.Y.Z` bumps the version, moves the `## [Unreleased]` section of `CHANGELOG.md` under a dated heading, commits `release: vX.Y.Z`, tags it and pushes. The tag starts `.github/workflows/release.yml`.

```bash
scripts/release.sh 0.2.0 --dry-run     # shows what would happen
scripts/release.sh 0.2.0               # master only, clean tree, tag must not exist
scripts/release.sh 0.2.0-rc.1          # a prerelease: published, but the app ignores it
```

## What the workflow does

1. Checks that the tag equals the version in `Cargo.toml`, then runs fmt, clippy and the tests.
2. Builds `Ferrocast.app` with `scripts/bundle.sh`.
3. Packages `Ferrocast-X.Y.Z.dmg` (for people), `Ferrocast-X.Y.Z.zip` (what the in-app updater downloads) and `SHA256SUMS.txt` (the updater checks the ZIP against it).
4. Publishes a GitHub Release with the changelog section as notes. A tag with a suffix is a prerelease.
5. For stable releases, commits the update of `Formula/ferrocast.rb` (new source tarball and sha256) straight to `master`, so `brew upgrade ferrocast` sees the version right away.

## Signing

With no secrets the app is signed ad-hoc. To sign with a Developer ID and notarize, add these repository secrets; nothing else changes:

- `APPLE_CERT_P12` (base64 of the Developer ID Application certificate), `APPLE_CERT_PASSWORD`, `APPLE_SIGN_IDENTITY`.
- `APPLE_NOTARY_KEY` (base64 of the App Store Connect `.p8` key), `APPLE_NOTARY_KEY_ID`, `APPLE_NOTARY_ISSUER`.

Once releases are notarized, switch the default of “Install updates” to automatic: a self-update then keeps the Accessibility permission.

:::caution[Limits]
The updater verifies a SHA-256 published next to the ZIP, over HTTPS from GitHub. There is no separate signing key for updates yet.
:::

## Related

- [Install](/docs/getting-started/install/)
- [Build from source](/docs/getting-started/build-from-source/)
