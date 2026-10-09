#!/bin/bash
# bundle.sh [release|debug] — builds OptChat.app (com.reframe.optchat.gpui) into dist/, ad hoc signed.
set -e
cd "$(dirname "$0")/.."
PROFILE=${1:-release}
if [ "$PROFILE" = release ]; then cargo build --release -p optchat-app; BIN=target/release/OptChat; else cargo build -p optchat-app; BIN=target/debug/OptChat; fi
APP=dist/OptChat.app
rm -rf "$APP"; mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/OptChat"
cp packaging/OptChat.icns "$APP/Contents/Resources/OptChat.icns"
VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key><string>com.reframe.optchat.gpui</string>
  <key>CFBundleName</key><string>OptChat</string>
  <key>CFBundleDisplayName</key><string>OptChat</string>
  <key>CFBundleExecutable</key><string>OptChat</string>
  <key>CFBundleIconFile</key><string>OptChat</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSSupportsAutomaticGraphicsSwitching</key><true/>
  <key>LSApplicationCategoryType</key><string>public.app-category.productivity</string>
  <key>NSMicrophoneUsageDescription</key><string>OptChat uses the microphone for dictation into the composer.</string>
</dict>
</plist>
PLIST
codesign --force --deep --sign - "$APP"
codesign -dv "$APP" 2>&1 | grep -E "Identifier|Signature"
echo "built $APP"
