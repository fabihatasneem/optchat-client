# Windows qualification record

Prepared on Linux on 2026-10-15. Windows packages and native behavior have **not** been qualified yet.
Record source commit/dirty state, artifact SHA256, version, signing thumbprint, CPU, Windows edition/build,
GPU/driver and results for each candidate. Synthetic test packages are never distributable applications.

## Intended matrix

- Windows 10 22H2 x64 (build 19045): native x64 ZIP and signed MSIX.
- Current Windows 11 x64: native x64 ZIP, signed MSIX, bundle selection.
- Windows 11 ARM64: native ARM64 ZIP, signed MSIX, bundle selection.
- Windows 11 ARM64 with x64 emulation: optional fallback checks, not a substitute for ARM64.
- Standard user and managed deployment where offered. Server, S mode and 32-bit are excluded initially.

## Ordered native checks

1. Build with the documented native MSVC/SDK tools. Record Rust, VS, SDK and source revisions. Confirm MakeAppx
   schema validation, correct PE machine, DLL imports, icon/version resources and clean-machine runtime completeness.
2. Run `test-packaging.ps1`, native shell tests and `check-package.ps1 -RequireSignature` on the signed candidate.
3. Install for a standard user. Confirm Start entry/icon, window, taskbar, last-window exit and relaunch.
4. Connect to a fake engine for controlled UI checks, then a real engine. Test HTTPS/WSS, certificate/hostname
   rejection, engine additions, workspace switching, reconnect and token restoration.
5. Confirm tokens use Credential Manager and never prefs. Check settings/tokens after relaunch and logout/reboot,
   using isolated entries. Long engine/workspace keys and Unicode secrets must round-trip.
6. Check Ctrl shortcuts, Unicode/multiline input, IME, clipboard text/images, file dialogs/attachments, fonts,
   scrolling/large history, multiple monitors and DPI (100/150/200%).
7. Check actual WASAPI microphone capture/stop, privacy enabled/denied and no input device. No camera/QR scanner
   is implemented in this shell.
8. Upgrade using a higher version and the same identity/publisher. Verify settings/tokens and in-use app behavior.
   Check bundle selection: ARM64 on ARM hardware, x64 on Intel/AMD.
9. Remove MSIX. Confirm registration/shortcuts disappear while `%APPDATA%\OptChat` and test credentials remain.
   Reinstall and verify restoration. Clean up only isolated test settings/credentials.
10. Verify intended managed policies, corporate trust roots, proxy/network, Defender/SmartScreen, production
    signing, redistribution notices and hosting before public distribution.

## Evidence

- Linux checks passed on 2026-10-15: PowerShell 7.6.7 parsed the scripts and exercised both architecture layouts,
  XML escaping/minimum OS, checksum corruption and architecture mismatch rejection. Disposable Git fixtures
  passed preparation from CRLF, idempotence and preservation of extra edits in a patched file. The real pinned
  vendor checkout also passed the PowerShell preparation helper's idempotence check.
- Rust formatting, locked workspace metadata, `optchat-proto` checking, workflow actionlint and PNG/ICO checks
  passed. The actual Windows platform module and its tests type-checked for both Windows MSVC targets in an
  isolated harness using the real shared Platform trait and placeholder GPUI App/Window types. This verifies
  the Windows API bindings, not the full GPUI application or native linking.
- Native MSVC/MakeAppx build: pending.
- Native install/upgrade/removal: pending.
- Clean desktop, real engine and ARM64 hardware: pending.
- Production certificate, notice and hosting: pending.
