# PROGRESS — OptChat for Mac (GPUI)

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
- UI: `deno run -A tests/e2e.ts [--offline] [suite…]` — panels, features, chat, inputs, quit, smoke, flow, perf.
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
