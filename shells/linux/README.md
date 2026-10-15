# OptChat for Linux

**Installing the app?** Follow the [Linux installation guide](../../docs/linux-installation.md) for package
downloads, installation, first launch, updates and troubleshooting. Prebuilt packages do not need Rust or Podman.

This README covers source development and packaging. For release maintainers, the
[release guide](release/README.md) covers building, qualification, signing and publication.

The native Rust/GPUI shell runs the shared OptChat UI on Wayland and X11. Linux hooks stay in `src/platform.rs`:
Secret Service token storage (oo7), resident memory from `/proc/self/statm`, and the optional test QR hook.

## Build and run from source

Start from a source checkout containing `shells/linux/run.sh`; use the source commit shown on a successful Linux
workflow run if the default checkout does not include it. Install [rustup](https://rustup.rs/) and the packages
listed below. `rust-toolchain.toml` selects Rust 1.98.1.

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
Camera QR scanning is not implemented. Flatpak is outside the chosen release scope.

## Native packages and production distribution

The agreed minimum is **Ubuntu 24.04**, with qualification on Ubuntu 24.04, Ubuntu 26.04 and Debian 13.
Fedora RPMs are built on Fedora 43 and qualified on Fedora 43 and 44. Both x86_64 and ARM64 are in scope.
Flatpak is outside this release's scope. Native signed APT/DNF repositories provide managed updates, following
[OpenAI's documented Linux distribution model](https://learn.chatgpt.com/docs/linux/linux-app).

Follow [the step-by-step distribution guide](release/README.md), starting with rootless Podman setup. Then:

```bash
shells/linux/release/container-build.sh deb
shells/linux/release/container-build.sh rpm
```

Run these on each native architecture. They build from source in separate baseline containers and target directories;
the current Ubuntu 26.04 binary is never reused. Outputs go under `dist-dev/ubuntu24.04-<CPU>/` and
`dist-dev/fedora43-<CPU>/`, with checksums and provenance. The first x86_64 baseline DEB/RPM builds require
GLIBC 2.39 and their extracted applications pass 23 smoke checks on each of the host's Wayland and XWayland
backends. The earlier Ubuntu 26.04 development DEB/AppImage requires GLIBC 2.43.
Producing package files does not establish production readiness.

`release/check-install.sh` checks dependencies/install/removal/reinstall across the agreed OS versions and tests
upgrades when a previous package is supplied. `test-native-package.sh` tests an extracted DEB/RPM with the fake
engine on a real desktop. Clean desktop qualification, secure token persistence, real-engine behavior and final
signed hosted repository checks remain required on both architectures. Use
[the qualification checklist](release/QUALIFICATION.md) to record each OS/CPU result.

`release/package_deb.py` and `release/package_rpm.py` can also run directly on their native baseline systems.
The approved default maintainer is `Fabiha Tasneem <fabiha@reframe.so>`; `--maintainer` or
`OPTCHAT_PACKAGE_MAINTAINER` can override it. Copyright/license work is deferred, with a
[notice template](packaging/COPYRIGHT.template) to complete later. Development packages can be built now;
production revisions require the approved notice and, for RPM, the approved SPDX expression.
`release/apt_repository.py` prepares signed APT metadata;
`release/rpm_repository.py` signs RPM copies and prepares signed DNF metadata. Both need an owner-supplied GPG key
and qualified packages; they do not upload repositories or change this host's package-manager configuration.
Customer installation and update commands are in the distribution guide, using the final HTTPS repository URLs.

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

The current package is verified on **Ubuntu 26.04 x86_64**. Its bundled binaries reference **GLIBC 2.43**. The package checks pass with normal launch and extract-and-run on Wayland, and with the X11 backend through
XWayland. Other Linux distributions, older Ubuntu versions, aarch64, standalone Xorg sessions and real microphone
recording have not been verified.
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

## Verify the package and check usability

Test an existing AppImage without rebuilding it:

```bash
shells/linux/test-package.sh
# Or point the helper at another version:
shells/linux/test-package.sh /absolute/path/to/OptChat.AppImage
```

The helper verifies the matching checksum and runs the 23-check fake-engine suite with normal AppImage launch,
extract-and-run, and the X11 backend when DISPLAY is available. In a Wayland session the X11 test uses XWayland;
it is not a test of a standalone Xorg desktop. The app runs from a temporary directory to catch source-directory
assumptions. The helper needs this checkout and Deno; the customer's AppImage does not need development tools.
All test instances use temporary prefs/tokens and are closed automatically.

For manual checks, open the packaged application with the large fake fixture:

```bash
OPTCHAT_DEMO_BINARY="$PWD/dist-dev/OptChat-0.1.0-x86_64.AppImage" \
OPTCHAT_DEMO_BIG=1 OPTCHAT_DEMO_PORT=18792 \
shells/linux/demo.sh
```

This bypasses Cargo and uses the AppImage. The fixture contains 1,770 entries in the main chat and five child tasks.
Keep the terminal open during the test. Closing the app cleans up that demo's fake engine and temporary data.
Change the port if it is already in use. Use `APPIMAGE_EXTRACT_AND_RUN=1` only to test the FUSE-free fallback;
normal launch should also be checked for customer usability.

Manually inspect fonts/icons, resize the window, toggle full screen, scroll the long chat, switch among the provided
tasks/workspaces, use the real keyboard and clipboard (including multiline and non-ASCII text), open/dismiss dialogs,
and leave it idle for several minutes. Watch for freezes, clipped text, lost drafts and continuing memory growth
when repeating the same actions. Automated probe checks do not establish that scrolling and mouse interaction feel smooth.

Then copy only the AppImage/checksum into a clean desktop VM or another computer on each intended target OS.
Verify launch outside the checkout and without development packages. Use a scoped test-engine token to verify
real connections, secure-store persistence after reopening, attachments and dictation where supported. The fake
engine returns fixed data; it does not verify real AI execution or speech transcription. Record the tested OS,
desktop session, graphics hardware, package checksum and any failures. A local smoke pass alone is not a broad
Linux compatibility claim.

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
