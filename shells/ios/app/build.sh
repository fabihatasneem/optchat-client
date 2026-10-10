#!/bin/bash
# Build the OptChat iOS binary with cargo.
#   app/build.sh            simulator check build (no bundle)
#   app/build.sh xcode      from OptChat.xcodeproj's script phase: builds for the platform Xcode is building and places
#                           the binary + fonts (+ the dev CA in Debug) into Xcode's bundle; Xcode signs and provisions it.
# GPUI: zed-industries/zed at ZED_COMMIT (gpui-ios-platform, zed#63068) in vendor/zed (a worktree of vendor-probe's
# clone, or a fresh clone), with patches/*.patch applied (lazy SafariServices; release drawables when hidden).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"; ROOT="$(cd "$HERE/.." && pwd)"
ZED="$ROOT/vendor/zed"; ZED_COMMIT=97dc71d8b8f79012c1441aa01121fba95667292a
MODE="${1:-check}"
if [ ! -e "$ZED/.git" ]; then
  if [ -d "$ROOT/vendor-probe/zed-gpui-ios/.git" ]; then git -C "$ROOT/vendor-probe/zed-gpui-ios" worktree add -q --detach "$ZED" "$ZED_COMMIT"
  else git clone -q --filter=blob:none https://github.com/zed-industries/zed.git "$ZED"; git -C "$ZED" checkout -q "$ZED_COMMIT"; fi
fi
[ "$(git -C "$ZED" rev-parse HEAD)" = "$ZED_COMMIT" ] || { echo "vendor/zed is not at $ZED_COMMIT" >&2; exit 1; }
git -C "$ZED" checkout -q -- crates/gpui_ios crates/gpui_apple
for p in "$HERE"/patches/*.patch; do git -C "$ZED" apply "$p"; done

TARGET=aarch64-apple-ios-sim; PROFILE=release
if [ "$MODE" = xcode ]; then
  [ "${PLATFORM_NAME:-}" = iphoneos ] && TARGET=aarch64-apple-ios
  [ "${CONFIGURATION:-Release}" = Debug ] && PROFILE=dev
fi
OUTJSON="$(mktemp)"
( cd "$HERE" && env -i HOME="$HOME" PATH="$HOME/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin" ${DEVELOPER_DIR:+DEVELOPER_DIR="$DEVELOPER_DIR"} \
    IPHONEOS_DEPLOYMENT_TARGET=18.0 RUSTUP_TOOLCHAIN="$(cat "$ZED/rust-toolchain.toml" | sed -n 's/^channel = "\(.*\)"/\1/p')" \
    cargo build --profile $PROFILE --target $TARGET --message-format=json-render-diagnostics > "$OUTJSON" )
EXE="$(python3 -c "import json,sys;print([e['executable'] for e in map(json.loads,open(sys.argv[1])) if e.get('reason')=='compiler-artifact' and e.get('target',{}).get('name')=='optchat-ios' and e.get('executable')][0])" "$OUTJSON")"
rm -f "$OUTJSON"
if [ "$MODE" = xcode ]; then
  R="$TARGET_BUILD_DIR/$UNLOCALIZED_RESOURCES_FOLDER_PATH"; mkdir -p "$R"
  cp "$EXE" "$TARGET_BUILD_DIR/$EXECUTABLE_PATH"
  # archives: a dSYM for crash symbolication (the release profile keeps line tables)
  if [ "${ACTION:-}" = install ] && [ -n "${DWARF_DSYM_FOLDER_PATH:-}" ]; then
    mkdir -p "$DWARF_DSYM_FOLDER_PATH"; dsymutil "$EXE" -o "$DWARF_DSYM_FOLDER_PATH/$DWARF_DSYM_FILE_NAME"
  fi
  cp "$ZED/assets/fonts/lilex/Lilex-Regular.ttf" "$R/" # OFL
  if [ "${CONFIGURATION:-Release}" = Debug ]; then cp "$ROOT/legacy-swiftui/OptChat/Resources/dev-root-ca.der" "$R/"; else rm -f "$R/dev-root-ca.der"; fi
  echo "placed $TARGET ($PROFILE) at $TARGET_BUILD_DIR/$EXECUTABLE_PATH"
else
  echo "built $EXE"
fi
