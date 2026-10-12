# shells/mac: OptChat for macOS

This is a thin shell around `optchat-ui`. The crate is `optchat-mac` and the binary is `OptChat`.

- `src/main.rs`: the entry point. It installs the platform, loads prefs from
  `~/Library/Application Support/com.reframe.optchat.gpui/prefs.json` (`OPTCHAT_PREFS_DIR` overrides it), sets up the
  app menu and keys, and opens the window.
- `src/platform.rs`: the macOS `Platform`. It covers:
  - Keychain tokens (service `com.reframe.optchat.gpui`, the format the app has always used);
  - resident memory via `proc_pidinfo`;
  - the test QR "scanner", used when `OPTCHAT_TEST_QR` is set.
- `packaging/OptChat.icns`: the app icon.
- `release/bundle.sh [release|debug]`: builds and ad hoc signs `dist-dev/OptChat Dev.app`
  (`com.reframe.optchat.gpui.dev`), which the tests use. `SHIP=1` replaces `dist/OptChat.app`
  (`com.reframe.optchat.gpui`); only use it for a finished, verified build. `--profile release-lite` is for dmg
  builds.
- `tools/`:
  - `shot.sh`: screenshots this repo's OptChat window only. It builds `winid` from `winid.swift` on first use.
  - `icon.swift`: generates the icon.

**Not done yet:** Developer ID signing, notarization and an installer (pkg or dmg). The plan is in the team notes
(`mac-installer-plan.md`). The scripts must never contain certificate names, team ids or passwords; read them from
env vars.
