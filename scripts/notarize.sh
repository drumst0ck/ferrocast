#!/bin/bash
# Notarize and staple the DMG so Gatekeeper accepts it on other Macs.
# Requires an Apple Developer account. Never commit credentials; pass them via the environment:
#   FERROCAST_SIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)"   (used by bundle.sh)
#   NOTARY_PROFILE=<name stored with: xcrun notarytool store-credentials NAME …>
set -euo pipefail
cd "$(dirname "$0")/.."
: "${FERROCAST_SIGN_IDENTITY:?set FERROCAST_SIGN_IDENTITY to a Developer ID Application identity}"
: "${NOTARY_PROFILE:?set NOTARY_PROFILE to a notarytool keychain profile}"
[ "$FERROCAST_SIGN_IDENTITY" != "-" ] || { echo "ad-hoc signatures cannot be notarized" >&2; exit 1; }
scripts/bundle.sh
scripts/dmg.sh
version=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
dmg="dist/Ferrocast-${version}.dmg"
codesign --force --sign "$FERROCAST_SIGN_IDENTITY" --timestamp "$dmg"
xcrun notarytool submit "$dmg" --keychain-profile "$NOTARY_PROFILE" --wait
xcrun stapler staple "$dmg"
spctl --assess --type open --context context:primary-signature -v "$dmg"
echo "Notarized: $PWD/$dmg"
