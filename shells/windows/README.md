# shells/windows: to be built

There's no code here yet. See the root `README.md` (Windows) and `docs/platforms.md`.

To start:
1. Install MSVC, the Windows SDK (≥ 10.0.20348) and CMake.
2. Copy `shells/mac` here as `optchat-windows`.
3. Implement `Platform`: Credential Manager tokens, `GetProcessMemoryInfo`, prefs in `%APPDATA%\OptChat`.
4. Signing (Authenticode) and an installer (MSIX or WiX) come later, in `release/`.
