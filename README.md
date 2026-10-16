# OptChat client (GPUI)

OptChat's native client, written in Rust on [GPUI](https://github.com/zed-industries/zed) (Zed's UI framework). It
runs on **Linux**, **macOS**, **Windows**, **iPhone** and **iPad**. Android support is in development.

See the [Windows installation guide](shells/windows/README.md) for installation, updates and engine setup.

**License status:** The project license and dependency redistribution review are deferred. Public release
approval remains pending; the Windows installer checked into this branch is a development-signed test build.

**It's a client only.** The OptChat *engine* is a separate TypeScript service in another repo: tasks, agents,
memory, dictation (speech-to-text) and storage all live there. The client talks to an engine over one websocket
(`/engine/ws`), plus HTTPS for blobs and `/stt`.

## Connecting to an engine

1. **Choose an engine.** Obtain access from your OptChat engine administrator. If you run an engine yourself,
   its administration page is `/engine` on that engine. Use a dedicated test workspace for development.
2. **Get a client token.** On the engine's `/engine` admin page, open **Client tokens** and create one scoped to your
   test workspace. It looks like `oc1.<header>.<secret>`; the header carries the engine's addresses.
3. **Add the engine.** In the app, open **Workspaces → Add engine** and paste the token (phones can scan it as a QR
   code). Every workspace in the token's scope connects by itself.
   - Tokens go to the OS secure store (Keychain on Apple), never to prefs or logs.
   - Never paste a token into an issue, a log or a commit.

## Layout

```
crates/proto      wire types and parsers (no UI, no OS)
crates/theme      themes
crates/platform   the Platform trait: the shared crates' only door to OS features (token store, memory, QR scan, …)
crates/client     connections, state/reducer (a GPUI entity), rpc, reconnect, dictation pipeline
crates/ui         every view, the phone layout, kit components (input, menus, buttons), bundled fonts/icons (assets/)
shells/mac        the macOS app: entry point, platform.rs, packaging, release/bundle.sh, tools/
shells/ios        the iOS app crate (app/), the Xcode wrapper (OptChat.xcodeproj), release/ (TestFlight)
shells/linux      the Linux app: Wayland/X11 entry point, Secret Service tokens, local demo
shells/windows    Windows shell, EXE installer, portable ZIP and MSIX packaging
shells/android    Android app using the shared client/UI, GPUI backend adaptation and APK packaging
vendor/zed        git submodule: zed-industries/zed (GPUI), pinned; our patches come from patches/zed
patches/zed       our GPUI patches, applied to vendor/zed by tools/zed-vendor.sh
tests             deno UI suites + a fake engine (tests/lib/fake-engine.ts)
tools             zed-vendor.sh
docs              platforms.md, ios-plan.md (the iOS design notes), archive/
```

### The boundary rule

- The shared crates (`crates/*`) never call an OS API. That means no `objc`, `security-framework`, `libc::proc_*`,
  `mach2`, `windows` or `jni`.
- Anything OS-specific goes through `optchat_platform::Platform`. Each shell implements it in
  `shells/<os>/src/platform.rs` and installs it once at launch.
- GPUI's own cross-platform APIs are allowed in the shared crates: clipboard, `open_url`, windows and text input.
- A new platform is a new shell. If you find you have to change a shared crate for your OS, the trait is missing
  something: tell us and we'll extend it.

### Dependencies

- Anything published on crates.io is a normal Cargo dependency (`[workspace.dependencies]` in the root
  `Cargo.toml`).
- GPUI isn't published, so it comes in as a **git submodule** (`vendor/zed`), never as copied source:
  - The submodule is upstream zed, branch `gpui-ios-platform` (zed PR #63068), pinned at `97dc71d`.
  - `tools/zed-vendor.sh` applies `patches/zed/*.patch` to it.
  - The root `Cargo.toml` `[patch]`es every zed crate to `vendor/zed/crates/*`, so all shells build against the same
    GPUI.

## Getting the code

```bash
git clone --recursive https://github.com/fabihatasneem/optchat-client.git
cd optchat-client
tools/zed-vendor.sh          # applies our GPUI patches (the build scripts also run it; it's a no-op when done)
```

- If you cloned without `--recursive`, run `git submodule update --init --depth 1 vendor/zed`, or just
  `tools/zed-vendor.sh`, which initializes the submodule too.
- Submodule tips:
  - `git status` ignores the patched files inside `vendor/zed` (`ignore = dirty`).
  - After a pull that moves the pin, run `git submodule update vendor/zed && tools/zed-vendor.sh`.

## Source-build prerequisites (developers, all platforms)

- **Rust:** rustup. The toolchain (1.98.1) comes from `rust-toolchain.toml` automatically.
- **Deno 2.x:** for the tests and the fake engine.
- **Network:** the first build fetches a lot.

## Build and run

### macOS (works today)

You need Xcode (command line tools and the Metal toolchain).

```bash
cargo run -p optchat-mac                  # debug run
shells/mac/release/bundle.sh              # dist-dev/OptChat Dev.app (com.reframe.optchat.gpui.dev, ad hoc signed)
```

- `SHIP=1 shells/mac/release/bundle.sh` replaces `dist/OptChat.app`. Only run it for a finished, verified build.
- Notarization isn't set up yet. More in `shells/mac/README.md`.

### iOS / iPadOS (works today)

You need Xcode and `rustup target add aarch64-apple-ios aarch64-apple-ios-sim`.

```bash
shells/ios/app/build.sh                   # simulator build of the Rust binary (no bundle)
open shells/ios/OptChat.xcodeproj         # Run: a script phase builds the crate, Xcode bundles and signs
```

For a device, copy `shells/ios/Config/Signing.local.xcconfig.example` to `Signing.local.xcconfig` (it's git-ignored)
and set your team. More in `shells/ios/README.md`.

### Linux (Wayland and X11)

Install Rust through rustup (the pinned toolchain is selected automatically), and these Debian/Ubuntu packages:

```bash
sudo apt install build-essential clang cmake pkg-config libasound2-dev libfontconfig-dev libssl-dev \
  libwayland-dev libx11-xcb-dev libxkbcommon-x11-dev libvulkan1 mesa-vulkan-drivers
tools/zed-vendor.sh
cargo run -p optchat-linux
# Or, from any directory:
shells/linux/run.sh
```

Fedora package equivalents: `alsa-lib-devel fontconfig-devel wayland-devel libxcb-devel
libxkbcommon-x11-devel openssl-devel`, plus a compiler, clang, cmake, pkg-config and Vulkan drivers.

- GPUI uses Vulkan/GLES and chooses Wayland or X11 from the desktop environment.
- Preferences are in `$XDG_CONFIG_HOME/optchat/prefs.json`, falling back to `~/.config/optchat/prefs.json`.
- Tokens use Secret Service (GNOME Keyring, KWallet or another compatible unlocked desktop keyring).
- Desktop shortcuts use Ctrl (for example Ctrl+K, Ctrl+N, Ctrl+V and Ctrl+Q).
- `cargo build` at the root builds the shared crates; select the Linux or macOS shell explicitly with `-p`.

For a local demo with no real engine or credentials, install Deno 2.x and run:

```bash
shells/linux/demo.sh
```

The demo starts the fake engine on loopback with `alpha` and `beta`, uses temporary preferences and test tokens,
and cleans up its engine and temporary files when the app exits. `OPTCHAT_DEMO_PORT` changes its default port
(18790). For automated checks: `cargo build -p optchat-linux && deno run -A tests/linux.ts`.

Build and test a distributable x86_64 AppImage:

```bash
shells/linux/release/bundle.sh --check
```

This produces the AppImage, SHA256 checksum and build information in `dist-dev/`. Packaging downloads are verified
against pinned sizes and SHA256 hashes. The current local build is tested on Ubuntu 26.04 x86_64; build on the oldest
Linux version you intend to support and test the package there before claiming broader compatibility.
Check an existing package with `shells/linux/test-package.sh` (normal launch, extract-and-run and the available
X11 backend). Manual usability and clean-machine testing are described in `shells/linux/README.md`.
For native DEB/RPM distribution and signed APT/DNF updates, follow
[the step-by-step Linux release guide](shells/linux/release/README.md).

Linux package metadata defaults to the approved maintainer `Fabiha Tasneem <fabiha@reframe.so>`.
Copyright/license work is deferred; complete [the notice template](shells/linux/packaging/COPYRIGHT.template)
later. Development packaging can continue while the production notice and RPM license expression are pending.

Build DEBs on Ubuntu 24.04 and qualify them on Ubuntu 24.04, Ubuntu 26.04 and Debian 13; build RPMs on Fedora 43
and qualify them on Fedora 43 and 44. Both x86_64 and ARM64 are in scope, using native builders. Flatpak is excluded.
The earlier Ubuntu 26.04 development DEB/AppImage requires GLIBC 2.43. Fresh x86_64 DEB/RPM baseline builds
now require GLIBC 2.39; their extracted applications pass the fake-engine smoke suite on Ubuntu 26.04's
Wayland and XWayland backends. See the release guide for the remaining qualification and metadata requirements.
Clean-desktop qualification, real-engine checks and production metadata/signing/hosting remain release requirements.

### Windows

To install a prebuilt app, follow the [Windows installation guide](shells/windows/README.md).
The native shell uses DirectX/DirectWrite, Credential Manager for tokens and `%APPDATA%\OptChat` for settings.
The following commands are for developers with PowerShell 7.4+, Rust, MSVC, the Windows SDK and CMake.
Both x64 and ARM64 are targeted; 32-bit Windows is outside the release scope.

```powershell
# From the matching Visual Studio developer PowerShell, with PowerShell 7.4+:
./shells/windows/release/build.ps1 -Architecture x64
# On an ARM64 Windows machine with its native C++ tools:
./shells/windows/release/build.ps1 -Architecture arm64
```

Each build produces an EXE installer under `dist-dev/windows-<architecture>/`, plus a SHA256 sidecar and a
generated release manifest. Inno Setup 6.7.3 is required for packaging. Use `-Formats exe,zip,msix` to also build
the optional portable ZIP and MSIX. Installable MSIX packages require signing and certificate trust. An optional MSIX bundle
combines both architectures. See [the maintainer guide](shells/windows/BUILDING.md) for packaging and signing,
and [the qualification record](shells/windows/release/QUALIFICATION.md) for release validation.

### Android (development build)

The Android shell runs the shared OptChat client and phone UI using a patched `gpui-mobile` backend over
our existing GPUI pin. Download and connection instructions are in the [Android guide](shells/android/README.md).
The initial hello-window experiment has been replaced by the client.

```powershell
./shells/android/build-app.ps1
```

The build produces an ARM64 development APK, checksum and source/tool manifest under `dist-dev/android-app/`.
Android tokens use AES-GCM encryption with an Android Keystore key. HTTPS/WSS use rustls with bundled public
trust roots. See the [maintainer guide](shells/android/BUILDING.md), [research](shells/android/RESEARCH.md) and
[device qualification](shells/android/QUALIFICATION.md). Play publication and production signing remain future work.

## Adding a platform shell (checklist)

1. Copy `shells/mac` to `shells/<os>`, rename the crate, and add it to `members`.
2. Implement `optchat_platform::Platform` in `shells/<os>/src/platform.rs`. Every method has a default (no store, no
   QR, no memory figure), so you can start with `impl Platform for MyOs {}` and add `secure_store()` first.
3. In `main.rs`:
   - call `optchat_platform::install(...)` first thing;
   - build the tokio runtime and `gpui_platform::application()`;
   - register fonts and open the root view (copy these steps from the mac shell).
4. Keep signing, installers and store scripts in `shells/<os>/release/`. Keys and certificates never go in the repo:
   read them from env vars that point outside it.
5. Update `shells/<os>/README.md` and `docs/platforms.md`: what works, what doesn't, and how to build.

## Testing

**Fake-engine suites are the default.** They need no real workspace. Build the dev bundle first
(`shells/mac/release/bundle.sh`); the harness is macOS-only today (see below).

```bash
OPTCHAT_E2E_HEADLESS=1 deno run -A tests/panel.ts       # Workspaces panel, oc1 tokens, scope changes
OPTCHAT_E2E_HEADLESS=1 deno run -A tests/localtoken.ts  # the local engine's own token
OPTCHAT_E2E_HEADLESS=1 deno run -A tests/phone.ts       # phone layout, gestures, fake dictation
OPTCHAT_E2E_HEADLESS=1 deno run -A tests/perf-fake.ts   # a 1,770-entry chat: frame times, memory
```

You can also run the fake engine by itself: `deno run -A tests/lib/fake-engine.ts 18790 alpha beta`.

- **Other suites need a real test engine:** `live`, `panels`, `parity`, `sync`, `engine`, `e2e`, and the others that
  connect to a workspace. They need its test workspaces plus a 0600 token file at `data/test-tokens.json` that is
  git-ignored and never committed. Ask for your own. The suites refuse workspace names that aren't test workspaces.
- **Never test against real user workspaces or the live chat.** Never read the engine's real `local-client-token`.
  The harness points `OPTCHAT_LOCAL_TOKEN_FILE`, `OPTCHAT_TOKEN_DIR` and `OPTCHAT_PREFS_DIR` at temp paths.
- **Unit tests:** `cargo test -p optchat-proto -p optchat-theme -p optchat-client`.
- **Caveat:** the UI harness launches the macOS bundle. In headless mode it injects mouse input as NSEvents, and
  otherwise it uses ChatGPT.app's Computer Use and `screencapture`. On Linux or Windows:
  - Linux has `deno run -A tests/linux.ts` for native window, connection and keyboard smoke checks;
  - start with `cargo test` and a manual run against the fake engine;
  - porting the harness (launch the binary directly, inject input through GPUI from the test channel, capture
    screenshots with GPUI's headless renderer, which supports Linux) is a good early task.

## Known blockers

**Shared crates (fixed):** `optchat-client` no longer depends on `security-framework`, the memory probe went behind
the trait, and the token test store's unix file mode is `cfg(unix)`. Still open:
- The local engine token path uses `$HOME`.
- Mic permission UX exists only on macOS.

**Linux:**
- Secret Service needs a running keyring, so headless CI should use the file store (`OPTCHAT_TOKEN_DIR`).
- You need ALSA headers for cpal and a Vulkan driver.
- The UI test harness is macOS-only.
- The desktop shell uses our iOS PR branch pin, not zed main; keep running the Linux smoke suite when changing it.

**Windows:**
- Native x64 build, MSIX installation and fake-engine smoke checks passed on one Windows machine.
- EXE installation, upgrade and removal are tested on x64. Clean-machine, real-engine and ARM64 checks remain.
- Bundled fonts through DirectWrite, GPU drivers, clipboard, IME, file dialogs and microphone need native checks.
- MSIX signing and certificate trust are required for installation. Public distribution needs production signing
  and the deferred project/dependency notice review.
- Local engine token discovery still needs `OPTCHAT_LOCAL_TOKEN_FILE` when `HOME` is unset.

**Android:**
- The adapted community backend is carried separately from pinned Zed; it still needs broader device qualification.
- ARM64 on the tested OnePlus 9 / Android 14 is the current development target.
- Tokens can be pasted; QR scanning is not implemented.
- Microphone permission is requested on dictation use; real capture/STT and a real engine remain unqualified.
- Other Android versions, 16 KB runtime compatibility, accessibility, long background use and performance need checks.

**macOS/iOS:** both depend on the GPUI pin above. Don't bump it without re-running the iOS simulator build and the
fake-engine suites.
