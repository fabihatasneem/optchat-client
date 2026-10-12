# shells/android: research first

There's no GPUI Android backend in our pinned zed (`vendor/zed` has no `gpui_android`, and `gpui_platform` has no
Android branch). The options are:
- **(a)** a `gpui_android` backend (android-activity + wgpu/`gpui_wgpu`, text, touch, IME) carried as patches;
- **(b)** a non-GPUI shell reusing `optchat-proto`.

The first deliverable is a written comparison plus a "hello window" spike for (a). See the root `README.md`
(Android) and `docs/platforms.md`.
