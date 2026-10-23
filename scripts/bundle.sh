#!/bin/bash
# Build dist/Ferrocast.app. No install or publishing side effects.
#   FERROCAST_SIGN_IDENTITY  codesign identity; default "-" (ad-hoc, local use only).
#                            With a "Developer ID Application: …" identity the hardened runtime
#                            and a secure timestamp are enabled, as notarization requires.
set -euo pipefail
cd "$(dirname "$0")/.."
version=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
identity=${FERROCAST_SIGN_IDENTITY:--}
cargo build --release --locked -p ferrocast
app="$PWD/dist/Ferrocast.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/ferrocast "$app/Contents/MacOS/ferrocast"
chmod 755 "$app/Contents/MacOS/ferrocast"
# Apple Translation helper (Swift; the Translation framework needs the macOS 26 SDK). With an older
# SDK it is skipped and the app hides the Apple engine, so every build still works.
helper="$app/Contents/MacOS/ferrocast-translate"
sdk_major=$(xcrun --show-sdk-version 2>/dev/null | cut -d. -f1 || true)
if [ "${sdk_major:-0}" -ge 26 ] 2>/dev/null; then
  xcrun swiftc -O -target "$(uname -m)-apple-macos13.0" -parse-as-library \
    -Xlinker -weak_framework -Xlinker Translation \
    crates/macos/swift/translate/main.swift -o "$helper"
  chmod 755 "$helper"
else
  echo "NOTE: macOS SDK ${sdk_major:-unknown} has no Translation framework; the Apple translation engine will be hidden."
fi
[ -f resources/Ferrocast.icns ] || python3 scripts/make_icon.py
cp resources/Ferrocast.icns "$app/Contents/Resources/Ferrocast.icns"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>Ferrocast</string>
  <key>CFBundleDisplayName</key><string>Ferrocast</string>
  <key>CFBundleIdentifier</key><string>dev.ferrocast.launcher</string>
  <key>CFBundleExecutable</key><string>ferrocast</string>
  <key>CFBundleIconFile</key><string>Ferrocast</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${version}</string>
  <key>CFBundleVersion</key><string>${version}</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>LSUIElement</key><true/>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSHumanReadableCopyright</key><string>MIT licensed. Not affiliated with Raycast.</string>
  <key>CFBundleURLTypes</key><array><dict>
    <key>CFBundleURLName</key><string>dev.ferrocast.url</string>
    <key>CFBundleURLSchemes</key><array><string>ferrocast</string></array>
  </dict></array>
  <key>NSAppleEventsUsageDescription</key><string>Ferrocast uses Automation for system commands you explicitly run.</string>
  <key>NSMicrophoneUsageDescription</key><string>Ferrocast listens to your microphone only while you dictate into a note.</string>
  <key>NSSpeechRecognitionUsageDescription</key><string>Ferrocast turns your speech into text only while you dictate into a note.</string>
</dict></plist>
PLIST
plutil -lint "$app/Contents/Info.plist"
flags=(--force --sign "$identity" -i dev.ferrocast.launcher --entitlements resources/Ferrocast.entitlements)
if [ "$identity" != "-" ]; then
  flags+=(--options runtime --timestamp)
else
  # An ad-hoc signature's default requirement is the binary hash, which changes on every build and
  # makes macOS forget Accessibility permission after each upgrade. Pin it to the bundle identifier
  # instead so builds of this app share one requirement.
  flags+=(--requirements '=designated => identifier "dev.ferrocast.launcher"')
fi
# the helper is signed first: the app's seal covers it, and notarization wants every binary signed
if [ -x "$helper" ]; then
  helper_flags=(--force --sign "$identity" -i dev.ferrocast.launcher.translate)
  [ "$identity" != "-" ] && helper_flags+=(--options runtime --timestamp)
  codesign "${helper_flags[@]}" "$helper"
fi
codesign "${flags[@]}" "$app"
codesign --verify --strict --verbose=1 "$app"
if [ "$identity" = "-" ]; then
  echo "NOTE: ad-hoc signature. Fine for local use; macOS may re-ask for permissions after each rebuild."
fi
printf '\nBundle ready: %s (v%s)\nRun: open "%s"\n' "$app" "$version" "$app"
