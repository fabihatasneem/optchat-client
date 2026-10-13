# OptChat for Linux

The native Rust/GPUI shell runs the shared OptChat UI on Wayland and X11. Linux hooks stay in `src/platform.rs`:
Secret Service token storage (oo7), resident memory from `/proc/self/statm`, and the optional test QR hook.

## Setup and run

Install rustup and the packages listed below. `rust-toolchain.toml` selects Rust 1.98.1.

```bash
sudo apt install build-essential clang cmake pkg-config libasound2-dev libfontconfig-dev libssl-dev \
  libwayland-dev libx11-xcb-dev libxkbcommon-x11-dev libvulkan1 mesa-vulkan-drivers
tools/zed-vendor.sh
cargo run -p optchat-linux
```

`shells/linux/run.sh` prepares the pinned vendor patches and launches the app from any working directory.
The binary is `target/debug/optchat`. A build needs network access the first time and several GB of disk space.
Wayland and X11 are enabled. Vulkan is preferred, with GLES available through GPUI's renderer.

Preferences use `$XDG_CONFIG_HOME/optchat/prefs.json`, or `~/.config/optchat/prefs.json` if XDG_CONFIG_HOME is unset,
empty or relative. `OPTCHAT_PREFS_DIR` overrides that directory for tests.

A running, unlocked Secret Service keyring is required to save engine tokens. Tokens are separate from prefs.
For isolated tests, `OPTCHAT_TOKEN_DIR` uses the existing 0600 file store; this is never enabled by default.
Use Workspaces → Add engine to connect with a scoped test token as described in the root README.

Linux uses Ctrl shortcuts and standard Ctrl+Left/Right, Ctrl+Backspace and Ctrl+Home/End text navigation.
F11 toggles full screen; closing the last window or Ctrl+Q exits the process.
Dictation uses cpal/ALSA; microphone availability depends on your desktop audio configuration.
Camera QR scanning and Flatpak packaging are not implemented.

## Package an AppImage

Install the packaging prerequisites (in addition to the build prerequisites above):

```bash
sudo apt install curl desktop-file-utils binutils
```

Python 3.11 or newer is required for tool download verification. From the repository root, run:

```bash
shells/linux/release/bundle.sh --check
```

The script also works from another directory when invoked with its absolute path. It:

1. Verifies the pinned linuxdeploy, AppImage output plugin and AppImage runtime in `build/appimage-tools/`.
   Missing, truncated or changed files are downloaded to temporary files and installed only after size and SHA256
   match `release/appimage-tools.json`. The pinned GitHub asset URLs and cache-busting downloads prevent reuse of
   stale partial downloads. If an upstream asset is removed, verify a replacement and update the pins.
2. Builds `optchat-linux` with the stripped `release-lite` Cargo profile and the lockfile.
3. Validates the desktop entry and stages the executable, icon, fonts' license text and shared libraries in a temporary
   AppDir. Fonts and UI icons are already embedded in the executable.
4. Creates an x86_64 AppImage using the verified runtime, with extract-and-run to avoid requiring FUSE during packaging.
5. With `--check`, runs `tests/linux.ts` against the staged AppImage using an isolated fake engine and temporary test
   tokens/preferences. A failed check prevents replacing the previous package in `dist-dev/`.
6. Writes a checksum and build information, then moves all three files into `dist-dev/` and removes the staging directory.

For version 0.1.0, the outputs are:

```text
dist-dev/OptChat-0.1.0-x86_64.AppImage
dist-dev/OptChat-0.1.0-x86_64.AppImage.sha256
dist-dev/OptChat-0.1.0-x86_64.AppImage.build.json
```

The version comes from `[workspace.package]` in the root Cargo.toml. Change that version before the next versioned
release. `CARGO_BUILD_JOBS` controls build parallelism (default 4); `OPTCHAT_PACKAGE_PROFILE=release` selects the
regular release profile. Omitting `--check` builds a package without running the graphical smoke suite. That suite
requires a running desktop and Deno 2.x (`DENO_BIN` may point to its absolute path).

The JSON records the build OS, Rust version, source revision/local changes, maximum GLIBC requirement among the
bundled ELF files, pinned tool hashes and whether the packaged smoke suite passed. An AppImage still uses the host's
graphics drivers, desktop audio services and secure keyring.

### Compatibility and sharing

The current package is verified on **Ubuntu 26.04 x86_64**. Its bundled binaries reference **GLIBC 2.43**. Other
Linux distributions, older Ubuntu versions, aarch64, X11 sessions and real microphone recording have not been verified.
For broader support, build the package inside the oldest intended target distribution's VM/container and test it
there and on the other target desktops. A successful build on Ubuntu 26.04 does not establish support for older systems.

On each target desktop, check window rendering and fonts/icons, fake-engine connections, workspace switching,
keyboard shortcuts and reconnect. Separately test adding a scoped test token through Secret Service and reopening
with the same temporary prefs directory; the fake-engine smoke suite uses its isolated file token store. Verify
microphone recording if dictation is part of the release requirements.

To verify an artifact after copying or downloading it:

```bash
cd dist-dev
sha256sum -c OptChat-0.1.0-x86_64.AppImage.sha256
```

Share the AppImage together with its checksum. The build JSON is useful for testers; state the tested build platform
when sharing. Commit `shells/linux/release/`, `shells/linux/packaging/`, the Linux implementation and the updated
README/platform docs. `build/`, `target/` and `dist-dev/` are generated outputs and are already git-ignored. Preference
files and client tokens stay outside the package.

## Local demo and checks

Install Deno 2.x. These commands use only the fake engine on loopback and isolated temporary files:

```bash
shells/linux/demo.sh
cargo build -p optchat-linux
deno run -A tests/linux.ts
cargo test -p optchat-proto -p optchat-theme -p optchat-client -p optchat-linux
```

The demo uses workspaces `alpha` and `beta` on port 18790 (override with `OPTCHAT_DEMO_PORT`). Its fake engine and
temporary preferences/tokens are removed when the app exits. It never reads the real local engine token.
`DENO_BIN` can point to your Deno binary; the demo also finds `~/.deno/bin/deno`.

The smoke suite launches the native binary, verifies rendered frames, the chat snapshot, a shared engine socket,
text entry, Ctrl+A/Ctrl+K/Ctrl+Q, workspace switching, reconnect, and test-token file permissions. It requires a
running graphical desktop and deliberately does not automate other applications.

To run the same checks against a packaged executable, set `OPTCHAT_TEST_BINARY` to its absolute path. For an
AppImage, extract-and-run avoids requiring FUSE:

```bash
APPIMAGE_EXTRACT_AND_RUN=1 \
OPTCHAT_TEST_BINARY="$PWD/dist-dev/OptChat-0.1.0-x86_64.AppImage" \
deno run -A tests/linux.ts
```

If linuxdeploy segfaults even on `--help`, check that its download is complete. Compare its file size and SHA256
with the matching GitHub release asset before executing it. A truncated ELF file can crash before packaging starts.

To verify the desktop keyring with a temporary test entry (created and removed by the test):

```bash
cargo test -p optchat-linux secret_service_round_trip -- --ignored
```

The older macOS UI suites still rely on AppKit mouse injection and macOS bundle launching. `tests/linux.ts` uses
GPUI's existing test command channel for keyboard and application actions.
