# PROGRESS — OptChat for Mac (GPUI)

## Engines and workspaces (current, phase 4)
The app connects to OptChat engines: one websocket per engine (and client token) to `/engine/ws`, carrying every
connected workspace (`connect` / `disconnect`, `ws` on every message). Engines and connections are app settings
(`~/Library/Application Support/com.reframe.optchat.gpui/prefs.json`), tokens in the Keychain. Background workspaces
stay subscribed; a switch swaps their state in (0.003 ms + one frame). Workspace menu under the top-left icon,
Notifications rail view, notice dots in the status bar, no toasts, Settings Workspace / App, root layout by window
size. Code: client.rs (EngineConn + per-workspace Client), state.rs (WsData swap, route, notices), workspaces.rs
(menu, dialogs), notices_view.rs, tokens.rs, testcmd.rs (test channel).
- Tests: only `test-gpui-a` / `test-gpui-b` (tokens in data/test-tokens.json, git-ignored, never printed). App env
  for tests: OPTCHAT_ENGINE (+ _NAME), OPTCHAT_CONNECT, OPTCHAT_TEST_TOKENS, OPTCHAT_TOKEN_DIR (file token store
  instead of the Keychain: a rebuilt ad hoc binary would make the Keychain ask, and tests answer no system dialog).
- tests/engine.ts: phase 4 end to end through the app's own handlers (22/22): add engine, connect a + b, switch,
  a Haiku child with a tiny budget in b → warning → dot on the icon, notifications read (seen read by another client),
  relaunch restores, disconnect, local relay killed twice → offline in 154 / 458 ms, back in 3.8 / 3.5 s.
- tests/workspaces.ts: the same through the UI (clicks, keys, dialogs) with screenshots (menu, notifications, status
  bar, + ~1000 px JPEG copies): 21/21 headless. With Computer Use it needs an uncovered window (the screen was locked,
  then in use, during this session; one CUA attempt hit "window not drawing").
- Headless captures: a 1-pt resize (test op `nudge`) makes AppKit draw and present the covered window; then
  scripts/shot.sh captures our window only.
- Headless mode (OPTCHAT_E2E_HEADLESS=1): every suite without Computer Use — input through the test channel
  (keystrokes via Window::dispatch_keystroke, mouse/scroll as NSEvents handed to our own view: GPUI's real input
  path), frames laid out by the app itself every 100 ms (Window::draw, nothing presented), no screenshots; results
  in shots/e2e-results-headless.txt. AX menu-bar checks need Computer Use.
- One socket per engine since engine build 2026-10-10.21-connect-tokens: each `connect` carries its workspace's token;
  refused / revoked tokens drop only that workspace ("token refused" in the panel).
- Workspaces panel (rail view, user decision; replaced the popup menu and dialogs): workspaces.rs. tests/panel.ts runs
  it against tests/lib/fake-engine.ts (127.0.0.1, names from argv; no real workspace), 16/16 headless.
- Test workspaces test-gpui-a/b are purged: engine.ts / workspaces.ts (and the suites on a real workspace) need new
  ones from the orchestrator (OPTCHAT_TEST_DB + data/test-tokens.json; hub.ts's TEST_WORKSPACES list).

## Client mode (hub era)
The app is a pure client of the web app's hub (https://chat.reframe.dev): one websocket like the browser's page
(snapshot on connect / open, events, commands, rpc; reconnect with backoff + resnapshot; offline state in the status
bar), images from /blob, dictation over /stt, the microphone in the app. `crates/proto` holds the wire types; the app
no longer links `crates/engine` (kept in the repo, unlinked, for the user to decide). Per-window state (tabs, sidebar,
window, drafts, theme) in `~/Library/Application Support/com.reframe.optchat.gpui/prefs-<instance>.json`.
- Env: `OPTCHAT_HUB` (default https://chat.reframe.dev), `OPTCHAT_HUB_DB` (a test instance `t<id>`; unset = the live
  chat). Tests: `OPTCHAT_TEST_DB` (default tmv23x72y, a live-sized test copy seeded by the orchestrator), sync.ts uses
  tgpuisync, the empty-chat check tgpuiempty. Deno test clients need `DENO_TLS_CA_STORE=system` (e2e.ts sets it).
- Perf (release, live-sized copy; shots/perf.json): first frame 157 ms, chat loaded 384 ms (in-process: 200 / 850);
  task switch shown p50 33 / p90 51 ms, main chat 50 / 56 (in-process 14–34); with turns + requests (two rpcs after
  the snapshot, ≈ 26 ms p50) p50 49 / p90 109, main chat ≈ 107; scroll work p50 3.7 / p95 4.0 / max 4.3 ms
  (in-process 3.2 / 4.7 / 5.6); RSS 142 MB (185).
- Hub suggestions (not applied; optchat-src is read-only): turns/requests in the snapshot; a (task, at) index on
  request (main chat requests.recent ≈ 55 ms slower than other tasks); an explicit allowance for native clients instead
  of a spoofed Origin; an acknowledgement of settings changes (now / next turn / saved).

## In-process era (until 86e0447)

## Layout
- `crates/engine` — native Rust engine (no UI, no JS): types, store (rusqlite on the web app's schema), view,
  compactor, converse (canonical contexts/requests), lifecycle (tasks, turns, reports, budgets), task_tools, agents
  (claude CLI stream-json + codex app-server), local (loopback MCP + Anthropic/OpenAI proxies), tools, mcp_client,
  computer (cua), metrics, rates, models, prompts, trim, keyterms, stt (dictation), mic (cpal), hub (app-facing API).
- `crates/theme` — Zed theme-format loader + semantic palette, 17 bundled themes (THEMES.md has licenses).
- `crates/app` — GPUI app (`OptChat` binary): main (menus, window), state (mirror + commands), workspace, feed
  (virtual list, paging, follow mode), markdown, composer, tabs, tasks_view, statusbar, dialogs (new task, palette,
  metrics, approval, lightbox, toast), panels (memory, context, log, models, settings), theme, ui (primitives,
  clock-driven animations, probes/test state).
- `scripts/bundle.sh [release|debug]` → `dist/OptChat.app` (com.reframe.optchat.gpui, mic usage string, icon, ad hoc
  signature). `scripts/winid.swift`, `scripts/shot.sh` (window-only screenshots of our own process).
- DB: `data/optchat.db` (own copy; writer lock `data/optchat/writer.lock`). App data: `data/optchat-gpui/`
  (prefs.json, engine.log, claude-home, codex catalog).

## Build / run
- `cargo build --release && scripts/bundle.sh release && open dist/OptChat.app`
- Env: `OPTCHAT_DB=<file>` (another copy), `OPTCHAT_REFRAME_MCP=0` (no reframe MCP), test-only: `OPTCHAT_PROBES`,
  `OPTCHAT_TEST_HOOKS=1`, `OPTCHAT_FAKE_MIC=<wav>`, `OPTCHAT_TEST_IMAGE=<png>`, `OPTCHAT_TIMING=1`.

## Tests
- `cargo test --workspace` (offline, ~100 tests): view + view_props (ports of test/view-tasks.ts incl. 20k-step
  references), store_file (schema identical to the web DB, column sync, lock, blobs), tasks_offline, engine_offline
  (lifecycle texts, budgets, cascades, rollback, CAP split, computer lock, proxies via stub upstream, tool specs
  byte-identical to the TS, JS dates), unit tests (trim, keyterms, tools, local, rates, metrics, adapters, stt).
- Model/network (`-- --ignored`, Haiku + GPT-6-Luna only): model_turns, lifecycle, hub_realdb, stt live.
- UI: `deno run -A tests/e2e.ts [--offline] [suite…]` — panels, features, chat, inputs, quit, smoke, flow, perf, parity (no model calls), live (Haiku).
  Each suite launches dist/OptChat.app on a temp copy of data/optchat.db (removed on quit) and drives it with
  ChatGPT Computer Use (only our bundle id is approved). Results: shots/e2e-results.txt; screenshots in shots/.

## Performance (release, 1749-message main chat; shots/perf.json)
- First frame ~200 ms after launch, chat loaded ~850 ms (1.6 GB DB loads off the UI thread).
- Scrolling (in-app 120 Hz gesture through the list's scroll path): main-thread work per frame p50 3.5 ms,
  p95 4.9 ms, max 6.6 ms (never over 8.3 ms); presented intervals at the built-in 60 Hz display's vsync (p50 16.6 ms).
- Task switch (in-app, open → first render with data): 14–34 ms (main chat included; paging + (task, at) indexes).
- RSS ~170–190 MB. Idle with a "summarizing" spinner ~10% CPU (12.5 Hz clock ticker; 2.5 Hz in background), 0%
  with nothing animating.

## Notes / known limits
- macOS stops drawing a fully covered window (GPUI stops its display link); UI tests run while the machine is in use
  can fail for that alone. e2e detects it ("window not drawing") and retries offline suites once.
- Synthetic OS scroll events need an Accessibility grant (not taken), so scrolling is measured with an in-app gesture.
- The copied chat has unbuilt nodes: launching starts Haiku compactions (small spend), as the web app would.
- The root task's model in the copy is Opus: model tests use Haiku children or switch the model first.
