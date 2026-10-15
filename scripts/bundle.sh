#!/bin/bash
# Build a self-contained macOS bundle. No install or publishing side effects.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release --locked -p ferrocast
app="$PWD/dist/Ferrocast.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/ferrocast "$app/Contents/MacOS/ferrocast"
chmod 755 "$app/Contents/MacOS/ferrocast"
cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>Ferrocast</string>
  <key>CFBundleDisplayName</key><string>Ferrocast</string>
  <key>CFBundleIdentifier</key><string>dev.ferrocast.launcher</string>
  <key>CFBundleExecutable</key><string>ferrocast</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>LSUIElement</key><true/>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSAppleEventsUsageDescription</key><string>Ferrocast uses Automation for system commands you explicitly run.</string>
</dict></plist>
PLIST
plutil -lint "$app/Contents/Info.plist"
# Ad-hoc signing is only for local development; distribution needs Developer ID
# signing and notarization. Rebuilds can require regranting TCC permissions.
codesign --force --sign - "$app"
codesign --verify --strict "$app"
printf '\nBundle ready: %s\nRun: open "%s"\n' "$app" "$app"
