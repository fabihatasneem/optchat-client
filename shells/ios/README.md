# shells/ios: OptChat for iPhone and iPad

The shared client and UI run inside an iOS shell on GPUI's iOS backend (zed PR #63068, in the `vendor/zed`
submodule plus `patches/zed`).

- `app/`: the `optchat-ios` crate, a member of the root workspace.
  - `src/main.rs`: the shell. It provides safe-area insets, keyboard avoidance, the status bar style, and lifecycle
    footprint logging to `Documents/diag.log`.
  - Its `Platform`: Keychain tokens, the memory footprint via `task_info`, and the camera QR scanner
    (`native/qr_scanner.m`, compiled by `build.rs`, which accepts whole `oc1.` tokens only).
  - Also here: `Info.plist` (camera and mic usage strings), `OptChat.entitlements`, `Assets.xcassets`, and
    `UITests/` (XCUITest).
  - `build.sh`: `build.sh` alone makes a simulator check build. `build.sh xcode` is called by the Xcode script phase:
    it builds with cargo for the platform being built and places the binary and the Lilex font in the bundle.
- `OptChat.xcodeproj`: the Xcode wrapper. A script phase runs the cargo build; Xcode compiles the icon catalog and
  signs.
- `Config/Signing.xcconfig`: committed defaults (bundle id `com.reframe.optchat`, empty team).
  `Config/Signing.local.xcconfig` is git-ignored and holds your team (`DEVELOPMENT_TEAM = …`); copy it from the
  `.example` file.
- `release/`:
  - `testflight.sh`: archive, export and upload.
  - `asc.py`: read-only App Store Connect queries.
  - Both read the API key config from `OPTCHAT_ASC_JSON` (default `~/.config/optchat/asc.json`, outside the repo;
    shape in `asc.example.json`). Never commit `.p8` keys or that JSON.

## Build

```bash
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
shells/ios/app/build.sh                                     # simulator binary (target/aarch64-apple-ios-sim/release-ios/)
xcodebuild -project shells/ios/OptChat.xcodeproj -scheme OptChat -configuration Debug \
  -destination 'generic/platform=iOS Simulator' -derivedDataPath target/xcode build
```

## Test hooks (Debug)

These go in the scheme's or simctl's launch environment:
- `OPTCHAT_SEED_OC1` (base64 of an oc1 token): adds the engine the way the Add engine panel would.
- `OPTCHAT_SEED`: base64 JSON `{endpoint, workspace, token}`.
- `OPTCHAT_RESET=1`: starts from empty settings.
- `OPTCHAT_AUTOTEST_OPEN=<task>`.

Use test workspaces and test tokens only.

## Notes

`docs/ios-plan.md` has the background:
- §12: the UIKit vs GPUI bake-off;
- §13: the shell and the TestFlight pipeline;
- §14: the state as of 2026-10-11.

The old repo's `vendor/optchat-shared` pin is gone: the app builds the workspace crates directly.
