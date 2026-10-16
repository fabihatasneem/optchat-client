# Android maintainer guide

The Android app in `app/` contains a Rust `cdylib` (`optchat-android`) and a Gradle NativeActivity wrapper.
It uses the existing shared client/UI and GPUI pin; `vendor/gpui-mobile` is adapted by
`patches/gpui-mobile/android-app.patch`. `prepare-backend.ps1` applies or verifies exactly that patch.

## Build on Windows

Install JDK 21, Android SDK platform 36, build tools 36.0.0, NDK 29.0.14206865, Platform Tools and the
`aarch64-linux-android` Rust target. The Gradle 8.13 wrapper pins its download checksum; AGP is 8.13.0.
Rust uses the root toolchain file. Tool paths can be provided through `JAVA_HOME`, `ANDROID_HOME`,
`-JavaHome` and `-SdkRoot`. The scripts also recognize `%LOCALAPPDATA%\OptChat\AndroidTools`.

```powershell
./shells/android/doctor.ps1
./shells/android/build-app.ps1
```

The build prepares the submodules, runs Cargo with the lockfile, packages a stripped ARM64 library, checks
every ELF LOAD segment for at least 16 KB alignment, and verifies APK signing and ZIP page alignment.
The APK, SHA256 sidecar and build manifest go to `dist-dev/android-app/`. `-SkipNative` is for packaging
Java/resource changes against an existing current native library; rebuild normally after Rust changes.
Production signing is not configured. Debug builds use the machine's Android debug keystore outside Git.

## Device checks

Enable USB debugging and accept this computer on the device. Install the development APK with `adb install -r`.
The separate instrumentation APK uses native touch injection and Android's EditText InputConnection to check
engine token entry, Unicode/composition/UTF-16 selection, encrypted token storage and a chat round trip.
It uses the local fake engine only. Start with an empty development installation; the suite refuses connected
workspaces and must never run against real user engines.

```powershell
./shells/android/test-app.ps1
```

The suite starts a loopback fake engine, forwards its port over ADB and enables probes through an app-private
`files/test-mode` file in a debug build. Normal launches do not write probe/input data. Logs/screenshots go to
ignored `build/android/client-check/`. After checks, the suite removes its fake engine, test mode and test APK.
Do not enable test mode while entering real credentials. The installed OptChat app stays available afterwards.

## Remaining qualification

See [QUALIFICATION.md](QUALIFICATION.md). Real-engine use, microphone capture/STT, file selection, camera/QR,
accessibility, long background/process-death behavior, multiple Android versions/devices, and 16 KB runtime
compatibility need separate checks. API 26 is the declared minimum, not a tested support baseline.
The backend uses available system Bengali fonts and bitmap emoji; fallback coverage varies by device.
The project's deferred license/notice review and production distribution requirements still apply.
