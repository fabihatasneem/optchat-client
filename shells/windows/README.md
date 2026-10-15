# OptChat for Windows

Build files are prepared on Linux. Native Windows compilation, MakeAppx validation, installation and desktop
qualification are still pending. Nothing in this guide claims those checks have passed.

## Package formats and architectures

Use **MSIX** as the installer format, with a native package for each CPU architecture:

- **x64**, `x86_64-pc-windows-msvc`: Intel/AMD 64-bit PCs. Intended minimum: Windows 10 22H2 (build 19045).
  Also qualify current Windows 11 x64 systems. These are qualification targets, not verified support.
- **ARM64**, `aarch64-pc-windows-msvc`: Windows on ARM, including Snapdragon PCs. Intended minimum:
  Windows 11 (build 22000). Prefer the native package over x64 emulation.
- **32-bit x86/ARM32**: excluded from the initial release. Supporting them needs separate GPUI, dependency,
  compiler and desktop qualification; changing an installer label does not make a 32-bit application.
- **Windows 7/8, Server, S mode and a Microsoft Store listing**: outside the initial scope.

Home, Pro and Enterprise do not need separate binaries. Choose by CPU and OS version, then qualify the editions
and policies you intend to support. WSL is a Linux environment; use native Windows tools for these builds.
A GPU/driver compatible with the pinned GPUI DirectX backend is required.

Official distribution comparison, checked **2026-10-15**:

- [Claude Desktop](https://support.claude.com/en/articles/12622703-deploy-claude-desktop-for-windows) provides
  **x64 and ARM64 MSIX** for managed deployment, plus a user-facing installer on its
  [download page](https://claude.com/download). Full Cowork installation has additional admin requirements;
  OptChat does not install a Cowork service.
- [OpenAI's Windows deployment guide](https://learn.chatgpt.com/docs/enterprise/windows-deployment) currently
  presents the Codex Windows app as the ChatGPT desktop app. It offers a **Microsoft Store web installer** and
  **offline x64/ARM64 MSIX** packages. It explicitly says standalone MSI and non-Store EXE packages are unavailable.
  The MSIX download URLs retain `codex-app-prod` and the package identity is `OpenAI.Codex`.

OptChat follows their shared MSIX approach. A **portable ZIP** is added for early testing. An optional
**MSIX bundle** contains both architectures and lets Windows select one. There is no custom EXE bootstrapper,
MSI, service, Store listing or automatic updater here. Updates currently use a newer signed MSIX with the same
identity/publisher; an HTTPS App Installer feed or Store submission can follow qualification.

## Build files and output

`src/` provides native startup, Windows shortcuts, last-window exit, `%APPDATA%\OptChat\prefs.json`, Credential
Manager tokens and process memory measurement. `OPTCHAT_PREFS_DIR` isolates test settings. `build.rs` and
`packaging/` provide executable resources, the MSIX manifest and icons. `../../tools/zed-vendor.ps1` applies the same
pinned GPUI patches as the Bash helper without requiring Bash or discarding local submodule edits.

`release/build.ps1` compiles and packages; `bundle.ps1` combines matching x64/ARM64 MSIX files;
`new-test-certificate.ps1` creates a development signing identity; `check-package.ps1` inspects packages;
`test-packaging.ps1` runs cross-platform parser and package fixture checks; `test-vendor.ps1` checks CRLF
preparation, idempotence and edit preservation. The Windows GitHub Actions workflow builds
both architectures on native runners and retains unsigned development artifacts for 14 days.

For workspace version `0.1.0` and revision `0`, the x64 output is:

```text
dist-dev/windows-x64/
  OptChat-0.1.0.0-windows-x64-portable.zip
  OptChat-0.1.0.0-windows-x64-portable.zip.sha256
  OptChat-0.1.0.0-windows-x64-unsigned.msix
  OptChat-0.1.0.0-windows-x64-unsigned.msix.sha256
  OptChat-0.1.0.0-windows-x64-build-info.json
```

Signed packages omit `-unsigned`; ARM64 filenames use `arm64`. ZIPs contain `OptChat/optchat.exe`, notices,
build metadata and a DLL import record. MSIX adds the manifest and visual assets. Icons/fonts are embedded in
the EXE. Ship each package with its checksum. Development `.cer` files belong only on test machines.

The repository links the MSVC runtime statically. The build checks the PE architecture and rejects unexpected
direct dynamic VC++ runtime imports. Windows system DLLs, Schannel TLS and WASAPI audio remain OS services.
No Electron, Node, WebView2 or OpenSSL runtime is bundled. Validate `imports.txt` and clean-machine startup for
each final artifact; static configuration alone is not evidence of runtime completeness.

## Windows prerequisites

1. Clone the desired source branch into a short native path such as `C:\src\optchat-client`. Existing clones can
   fetch and switch once the Windows packaging branch's push is approved. The build initializes the pinned submodule.
2. Install **PowerShell 7.4+**, **Git for Windows**, **Rust via rustup**, **CMake** and **Visual Studio 2022 or newer**
   with **Desktop development with C++**. The repository selects Rust 1.98.1 in `rust-toolchain.toml`.
3. Install matching MSVC tools and Spectre-mitigated libraries: x64/x86 for x64; ARM64 for ARM64. Install a
   **Windows SDK >= 10.0.20348**, including `rc.exe`, `fxc.exe`, `makeappx.exe` and `signtool.exe`.
   SDK 10.0.26100 is suitable. Import `shells/windows/windows.vsconfig` through Visual Studio Installer for a
   VS 2022 component checklist covering both targets. No PostgreSQL or full Zed application setup is needed.
4. Use the matching Visual Studio developer shell. Start `pwsh -NoProfile` inside it to inherit the C++ environment
   while running PowerShell 7. Alternatively initialize VS directly from PowerShell 7:

```powershell
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
$vs = & $vswhere -latest -products '*' -property installationPath
& "$vs\Common7\Tools\Launch-VsDevShell.ps1" -Arch amd64 -HostArch amd64 -SkipAutomaticLocation
# On native ARM64, use -Arch arm64 -HostArch arm64 instead.
```

Check `cl.exe`, `link.exe`, `dumpbin.exe`, `cmake`, `cargo` and `git` are on PATH. Scripts locate the SDK tools.
Unset custom `RUSTFLAGS`, encoded flags and target-specific flag environment variables, which override the static
CRT configuration. Prefer native builds for each architecture. Cross-compiling ARM64 needs matching VS cross tools
and still requires ARM64 hardware qualification.

## Build and inspect

From the repository root in developer PowerShell 7:

```powershell
./shells/windows/release/test-packaging.ps1
./shells/windows/release/test-vendor.ps1
./shells/windows/release/build.ps1 -Architecture x64
./shells/windows/release/check-package.ps1 `
  -Package ./dist-dev/windows-x64/OptChat-0.1.0.0-windows-x64-portable.zip -Architecture x64
./shells/windows/release/check-package.ps1 `
  -Package ./dist-dev/windows-x64/OptChat-0.1.0.0-windows-x64-unsigned.msix -Architecture x64
```

On ARM64 substitute `arm64` in arguments/paths. `-Formats zip` builds just the portable package. Release EXEs use
the Windows GUI subsystem so launch does not open a console. Compilation uses `--locked --release` with a separate
Windows target directory. The script never installs packages or changes certificate trust.
Existing artifacts are not overwritten: increment `-Revision` or move old artifacts aside. Versions are
`workspace-major.minor.patch.Revision`; Store-specific versioning/identity would need a submission review.

Run native shell tests after building (use the corresponding target on ARM64):

```powershell
cargo test --locked --release -p optchat-windows --target x86_64-pc-windows-msvc --target-dir target/windows
```

The Credential Manager test creates and deletes its own unique entry. Tests do not replace installed-app checks.

## Development signing and installation

Unsigned MSIX files cannot be installed normally. Create a development certificate **once**, then reuse it for upgrades:

```powershell
$thumb = ./shells/windows/release/new-test-certificate.ps1
./shells/windows/release/build.ps1 -Architecture x64 -Revision 1 -CertificateThumbprint $thumb
```

The private key stays non-exportable in your current user's Personal store. Only the public
`build/windows/test-signing/OptChat-Development.cer` is exported. The script does not automatically trust it.
Review and import that public certificate on the test machine from an **elevated** PowerShell window:

```powershell
Import-Certificate -FilePath ./build/windows/test-signing/OptChat-Development.cer `
  -CertStoreLocation Cert:\LocalMachine\TrustedPeople
```

Then run the Appx deployment cmdlets in **Windows PowerShell 5.1**, as the normal signed-in user:

```powershell
Add-AppxPackage -Path ./dist-dev/windows-x64/OptChat-0.1.0.1-windows-x64.msix
Get-AppxPackage -Name Reframe.OptChat | Select-Object Name, Version, Architecture, InstallLocation
```

Launch from Start. For ZIP testing, extract the entire folder and run `OptChat\optchat.exe`. Add `-RequireSignature`
to `check-package.ps1` after trusting the test certificate. Rebuild with `-Revision 2` and the same `$thumb` for an
upgrade, then install the newer MSIX. Remove the current-user package with:

```powershell
Get-AppxPackage -Name Reframe.OptChat | Remove-AppxPackage
```

Settings intentionally remain in `%APPDATA%\OptChat` across ZIP/MSIX and uninstall; the manifest disables AppData
write virtualization. Tokens remain in Credential Manager unless removed in the app. Verify this lifecycle on
the intended Windows versions. Never add broad AppData cleanup. Managed machine-wide installation can provision
a signed package using elevated `Add-AppxProvisionedPackage -Online -PackagePath <package.msix> -SkipLicense`;
qualify that separately. OptChat has no Store offline-license file.

Remove the development certificate's Trusted People entry when no longer needed. Copying the public `.cer` to a
second test machine allows package trust but does not let it sign upgrades with the original private key.

## Optional architecture bundle

Build both architectures at the same version and Publisher, then:

```powershell
./shells/windows/release/bundle.ps1 `
  -X64Package ./dist-dev/windows-x64/OptChat-0.1.0.0-windows-x64-unsigned.msix `
  -Arm64Package ./dist-dev/windows-arm64/OptChat-0.1.0.0-windows-arm64-unsigned.msix
```

Output is `dist-dev/windows-bundle/OptChat-0.1.0.0-windows-unsigned.msixbundle` plus a checksum. Pass a matching
certificate thumbprint to sign the bundle. The final bundle must be signed and trusted; individual development
input packages can be unsigned. Identity, publisher, version and architecture must match.

## CI and production

The workflow supports manual dispatch and relevant pull requests, using `windows-2022` x64 and `windows-11-arm`
native ARM64 runners. It creates unsigned development artifacts and runs package/shell tests. It does not publish,
install certificates, install the app or automate a GUI. A Windows Server build runner does not prove Windows 10
compatibility. CI was not dispatched during Linux preparation. Collect both jobs' MSIX files before bundling.

For production, import the organization's code-signing certificate into the current user's Personal store or
adapt signing to a signing service. Its subject must exactly match `-Publisher`. `-Release` requires the production
Publisher, `-CertificateThumbprint`, an HTTPS RFC 3161 `-TimestampUrl` and an approved `-LicenseNotice` file. It
writes to `dist/windows-<architecture>/` and signs the EXE and MSIX. This guard does not establish release approval
or notice completeness. The approved maintainer remains **Fabiha Tasneem <fabiha@reframe.so>**; project/dependency
copyright and license review is deferred, as on Linux. Keep private keys/PFX files outside version control.

Complete [release/QUALIFICATION.md](release/QUALIFICATION.md) before public distribution, including clean-machine,
real-engine, TLS, GPU/input/audio, install/upgrade/removal, production signing, notices and HTTPS hosting.

References: [MSIX packaging](https://learn.microsoft.com/en-us/windows/msix/desktop/desktop-to-uwp-manual-conversion),
[signing](https://learn.microsoft.com/en-us/windows/msix/package/sign-app-package-using-signtool),
[bundling](https://learn.microsoft.com/en-us/windows/msix/packaging-tool/bundle-msix-packages),
[AppData virtualization](https://learn.microsoft.com/en-us/windows/msix/desktop/flexible-virtualization).
