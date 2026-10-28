#!/bin/bash
# Build dist/Ferrocast.app. No install or publishing side effects.
#   FERROCAST_SIGN_IDENTITY  codesign identity; default "-" (ad-hoc, local use only).
#                            With a "Developer ID Application: …" identity the hardened runtime
#                            and a secure timestamp are enabled, as notarization requires.
#   FERROCAST_DIST           output directory; default dist/ in the repository.
set -euo pipefail
cd "$(dirname "$0")/.."
version=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
identity=${FERROCAST_SIGN_IDENTITY:--}
cargo build --release --locked -p ferrocast
dist=${FERROCAST_DIST:-$PWD/dist}
app="$dist/Ferrocast.app"
bin_src="${CARGO_TARGET_DIR:-$PWD/target}/release/ferrocast"
helper_built=()
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$bin_src" "$app/Contents/MacOS/ferrocast"
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

# build_swift_helper <name> <src_dir> [weak framework…]
# Compiles every .swift file in <src_dir> to Contents/MacOS/<name> for arm64, macOS 13 minimum,
# linking the listed frameworks weakly so the helper still launches on older systems. Skips with a
# NOTE (never a failure) when swiftc is missing. Signing happens later, with the app's flags.
# Records built helpers in helper_built so one signing loop covers every helper.
build_swift_helper() {
  local name=$1 src=$2; shift 2
  if ! command -v swiftc >/dev/null 2>&1 || ! xcrun --find swiftc >/dev/null 2>&1; then
    echo "NOTE: swiftc not found; $name is not built and its feature is disabled"
    return 0
  fi
  local files=("$src"/*.swift)
  [ -e "${files[0]}" ] || { echo "NOTE: no Swift sources in $src; $name is not built"; return 0; }
  local weak=() fw
  for fw in "$@"; do weak+=(-Xlinker -weak_framework -Xlinker "$fw"); done
  swiftc -O -target arm64-apple-macos13.0 "${weak[@]}" "${files[@]}" -o "$app/Contents/MacOS/$name"
  chmod 755 "$app/Contents/MacOS/$name"
  helper_built+=("$name")
}

build_swift_helper ferrocast-capture crates/macos/swift/capture ScreenCaptureKit

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
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleLocalizations</key><array><string>en</string><string>es</string><string>de</string><string>fr</string><string>pt-BR</string><string>ja</string><string>zh-Hans</string></array>
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
  <key>NSMicrophoneUsageDescription</key><string>Ferrocast listens to your microphone only while you dictate into a note or when you turn it on in a recording.</string>
  <key>NSCameraUsageDescription</key><string>Ferrocast records your camera only when you turn it on in a recording.</string>
  <key>NSScreenCaptureUsageDescription</key><string>Ferrocast captures your screen only when you run a capture or recording command.</string>
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
# Helpers first, with the app's flags but their own identifier, then the bundle.
for h in ${helper_built[@]+"${helper_built[@]}"}; do
  codesign "${flags[@]/dev.ferrocast.launcher/dev.ferrocast.launcher.${h#ferrocast-}}" "$app/Contents/MacOS/$h"
done
codesign "${flags[@]}" "$app"
codesign --verify --deep --strict --verbose=1 "$app"
if [ "$identity" = "-" ]; then
  echo "NOTE: ad-hoc signature. Fine for local use; macOS may re-ask for permissions after each rebuild."
fi
printf '\nBundle ready: %s (v%s)\nRun: open "%s"\n' "$app" "$version" "$app"
