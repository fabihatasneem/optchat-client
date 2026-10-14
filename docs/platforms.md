# Platforms

| Platform | Shell | Status | GPUI backend at our pin |
|---|---|---|---|
| macOS | `shells/mac` | ships (dev bundle, ad hoc signed; notarization to do) | `gpui_macos` (Metal) |
| iOS / iPadOS | `shells/ios` | TestFlight builds | `gpui_ios` (zed PR #63068) + `patches/zed` |
| Linux | `shells/linux` | native shell, AppImage and development Debian packaging | `gpui_linux` (Wayland + X11) on `gpui_wgpu` (Vulkan/GLES) |
| Windows | `shells/windows` (README only) | to be built | `gpui_windows` (DirectX, DirectWrite) |
| Android | `shells/android` (README only) | research | **none** (no `gpui_android`; `gpui_platform` has no Android branch) |

## What a shell provides (`optchat_platform::Platform`)

| Hook | macOS | iOS | Linux | Windows (plan) | Android (plan) |
|---|---|---|---|---|---|
| `secure_store` (client tokens) | Keychain | Keychain | Secret Service (oo7) | Credential Manager | Keystore (JNI) |
| `resident_mb` (perf probes) | `proc_pidinfo` | `task_info` footprint | `/proc/self/statm` | `GetProcessMemoryInfo` | `/proc/self/statm` |
| `can_scan_qr` / `scan_qr` | test hook only | AVCaptureSession | test hook only | no | CameraX / ML Kit |

Covered by GPUI itself (no hook needed): clipboard, opening URLs, windows, IME, credentials API (an alternative
token store).

**Not behind the trait yet** (candidates for the next hooks):
- **Dictation capture:** cpal in `crates/client/src/mic.rs`. It works on macOS, iOS, Linux (ALSA) and Windows
  (WASAPI); macOS has the mic-permission check.
- **Prefs directory:** chosen by each shell's `main.rs`.
- **Local engine token path:** `$HOME/.reframe/data/workspaces/local-client-token`, desktop only.
- **Headless test input:** AppKit-only, in `crates/ui/src/testcmd.rs`, with no-op stubs elsewhere.
- **Haptics and system notifications:** later.

## Linux system packages

These come from zed's `script/linux` at the pin. Debian/Ubuntu: `libasound2-dev libfontconfig-dev libssl-dev
libwayland-dev libx11-xcb-dev libxkbcommon-x11-dev`, plus clang, cmake, pkg-config and a Vulkan driver. Fedora
equivalents: `alsa-lib-devel fontconfig-devel wayland-devel libxcb-devel libxkbcommon-x11-devel openssl-devel`.

## Windows toolchain

You need MSVC build tools (with the Spectre-mitigated libs), the Windows SDK ≥ 10.0.20348 and CMake. See
`vendor/zed/docs/src/development/windows.md`.

## Linux checks

`cargo run -p optchat-linux` launches the native desktop app. `shells/linux/demo.sh` launches a temporary fake-engine
demo. `cargo build -p optchat-linux && deno run -A tests/linux.ts` verifies native window rendering, connections,
keyboard input, workspace switching and reconnect using isolated test tokens and preferences.
Shared bindings use GPUI's `secondary-` modifier (Command on macOS, Ctrl elsewhere); Linux navigation overrides
live in the shell. `shells/linux/release/bundle.sh --check` builds an AppImage and verifies the packaged executable
before moving it to `dist-dev/` alongside its checksum and build information. Packaging tools and the AppImage
runtime are pinned and verified by size/SHA256. The current package is tested on Ubuntu 26.04 x86_64 and references
GLIBC 2.43. `shells/linux/test-package.sh` passes on Wayland and the X11 backend through XWayland; compatibility
with other distributions/older releases and standalone Xorg desktops is not yet verified. Camera scanning and the
full mouse-driven UI suite remain future work. Flatpak is outside the chosen release scope.

## Managed Linux distribution

`shells/linux/release/container-build.sh` selects the agreed native baseline: Ubuntu 24.04 for DEBs, Fedora 43 for RPMs.
Both x86_64 and ARM64 are in scope. Qualify DEBs on Ubuntu 24.04/26.04 and Debian 13; qualify RPMs on Fedora 43/44.
Separate native Cargo targets prevent reuse of newer-host binaries. `package_deb.py` infers ABI dependencies using
dpkg-shlibdeps, while RPM uses its native ELF dependency generator. `check-install.sh` covers container lifecycle checks;
`test-native-package.sh` covers extracted-app graphical smoke tests. Desktop qualification is still separate.
`apt_repository.py` and `rpm_repository.py` prepare signed APT/DNF repositories with an owner-supplied GPG key.
Fresh Ubuntu 24.04/x86_64 and Fedora 43/x86_64 development packages require GLIBC 2.39 and pass extracted-app
smoke checks on Ubuntu 26.04 with Wayland and XWayland. The earlier host development `.deb` and AppImage
require GLIBC 2.43. Rootless container lifecycle qualification is recorded separately under `build/qualification/`.
Maintainer/license metadata, baseline builds on both CPUs, clean-desktop lifecycle and real-engine tests, production
keys and HTTPS hosting must be complete before a stable release. Flatpak is excluded from the chosen scope.
See `shells/linux/release/README.md` and `release/QUALIFICATION.md` for the ordered workflow and sign-off record.
