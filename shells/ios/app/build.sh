#!/bin/bash
# Build the OptChat iOS binary with cargo (the optchat-ios crate, a member of the root workspace).
#   shells/ios/app/build.sh            simulator check build (no bundle)
#   shells/ios/app/build.sh xcode      from OptChat.xcodeproj's script phase: builds for the platform Xcode is building and
#                                      places the binary + fonts (+ a dev CA in Debug, if configured) into Xcode's bundle;
#                                      Xcode signs and provisions it.
# GPUI: the workspace's one pin, vendor/zed (tools/zed-vendor.sh: zed gpui-ios-platform at 97dc71d + patches/zed).
# Signing identity: DEVELOPMENT_TEAM / PRODUCT_BUNDLE_IDENTIFIER come from Xcode (Config/Signing.xcconfig and the
# git-ignored Config/Signing.local.xcconfig); nothing team-specific is hard-coded here.
# OPTCHAT_DEV_CA=<a .der file>: Debug builds bundle it as dev-root-ca.der (to trust a local dev edge's certificate).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"; ROOT="$(cd "$HERE/../../.." && pwd)"
MODE="${1:-check}"
"$ROOT/tools/zed-vendor.sh" # the submodule + our GPUI patches (no-op when in place)

TARGET=aarch64-apple-ios-sim; PROFILE=release-ios
if [ "$MODE" = xcode ]; then
  [ "${PLATFORM_NAME:-}" = iphoneos ] && TARGET=aarch64-apple-ios
  [ "${CONFIGURATION:-Release}" = Debug ] && PROFILE=dev
fi
# Simulator: entitlements live in the binary's __TEXT,__entitlements section (Xcode's linker adds it for Swift apps;
# our binary is linked by cargo), without which the Keychain fails with errSecMissingEntitlement (-34018).
LINKARGS=""
if [ "$TARGET" = aarch64-apple-ios-sim ]; then
  ENT="$ROOT/target/optchat-ios-simulated.xcent"; mkdir -p "$ROOT/target"
  APPID="${DEVELOPMENT_TEAM:-SIMULATOR}.${PRODUCT_BUNDLE_IDENTIFIER:-com.reframe.optchat}"
  cat > "$ENT" <<XC
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>application-identifier</key><string>$APPID</string>
<key>keychain-access-groups</key><array><string>$APPID</string></array>
</dict></plist>
XC
  LINKARGS="-C link-arg=-Wl,-sectcreate,__TEXT,__entitlements,$ENT"
fi
OUTJSON="$(mktemp)"
( cd "$ROOT" && env -i HOME="$HOME" PATH="$HOME/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin" ${DEVELOPER_DIR:+DEVELOPER_DIR="$DEVELOPER_DIR"} \
    IPHONEOS_DEPLOYMENT_TARGET=15.0 ${LINKARGS:+RUSTFLAGS="$LINKARGS"} \
    cargo build -p optchat-ios --profile $PROFILE --target $TARGET --message-format=json-render-diagnostics > "$OUTJSON" )
EXE="$(python3 -c "import json,sys;print([e['executable'] for e in map(json.loads,open(sys.argv[1])) if e.get('reason')=='compiler-artifact' and e.get('target',{}).get('name')=='optchat-ios' and e.get('executable')][0])" "$OUTJSON")"
rm -f "$OUTJSON"
if [ "$MODE" = xcode ]; then
  R="$TARGET_BUILD_DIR/$UNLOCALIZED_RESOURCES_FOLDER_PATH"; mkdir -p "$R"
  cp "$EXE" "$TARGET_BUILD_DIR/$EXECUTABLE_PATH"
  # archives: a dSYM for crash symbolication (the release-ios profile keeps line tables)
  if [ "${ACTION:-}" = install ] && [ -n "${DWARF_DSYM_FOLDER_PATH:-}" ]; then
    mkdir -p "$DWARF_DSYM_FOLDER_PATH"; dsymutil "$EXE" -o "$DWARF_DSYM_FOLDER_PATH/$DWARF_DSYM_FILE_NAME"
  fi
  cp "$ROOT/assets/fonts/Lilex-Regular.ttf" "$R/" # OFL
  if [ "${CONFIGURATION:-Release}" = Debug ] && [ -n "${OPTCHAT_DEV_CA:-}" ] && [ -f "$OPTCHAT_DEV_CA" ]; then cp "$OPTCHAT_DEV_CA" "$R/dev-root-ca.der"; else rm -f "$R/dev-root-ca.der"; fi
  echo "placed $TARGET ($PROFILE) at $TARGET_BUILD_DIR/$EXECUTABLE_PATH"
else
  echo "built $EXE"
fi
