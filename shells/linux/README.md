# shells/linux: to be built

There's no code here yet. The steps, system packages and known blockers are in the root `README.md` (Linux) and in
`docs/platforms.md`.

To start:
1. Copy `shells/mac` here, rename the crate to `optchat-linux` and add it to the workspace members.
2. Implement `optchat_platform::Platform` in `src/platform.rs`: Secret Service tokens, `/proc/self/statm` memory.
3. Use the prefs dir `$XDG_CONFIG_HOME/optchat`.
4. Run against the fake engine.
