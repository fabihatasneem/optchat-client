# PROGRESS — OptChat for Mac (GPUI)

## Layout
- `crates/engine` — native Rust engine (no UI): types, store (rusqlite on the web app's schema), view, compactor,
  converse (canonical contexts/requests), lifecycle (tasks, turns, reports, budgets), task_tools, agents (claude CLI
  stream-json + codex app-server), local (loopback MCP + Anthropic/OpenAI proxies), tools, mcp_client, computer (cua),
  metrics, rates, models, prompts, trim, keyterms, stt (dictation), mic (cpal), hub (app-facing API).
- `crates/theme` — Zed theme-format loader + semantic palette, 17 bundled themes (THEMES.md has licenses).
- `crates/app` — GPUI app (`OptChat` binary): main, state (mirror + commands), workspace, feed, markdown, composer,
  tabs, tasks_view, statusbar, dialogs (new task, palette, metrics, approval, lightbox, toast), panels (memory,
  context, log, models, settings), theme, ui (primitives + probes).
- `scripts/shot.sh <png>` — screenshot of the OptChat window only (never the whole screen).
- DB: `data/optchat.db` (own copy). App data: `data/optchat-gpui/` (claude-home, codex catalog, prefs.json).
- Run: `OPTCHAT_REFRAME_MCP=0 ./target/debug/OptChat` (reframe MCP off while testing; never touch it).

## Done
- Engine compiles; tests: unit (view, store, metrics, rates, adapters, local, trim, tools, keyterms, stt offline),
  `tests/tasks_offline.rs` (fork/zoom/pending/lifecycle/batching), `tests/model_turns.rs` (Haiku tool turn + child
  report; Codex luna turn) PASS, `tests/hub_realdb.rs` (real DB copy loads in ~0.5 s, views valid, no refold).
- Dictation live tests passed (agent report): Claude, Codex, fallback, drop.
- App launches on the real DB, renders rail/sidebar tasks/tabs/feed/composer/status bar (shots/dev-01.png).

## Next
- Verify every panel visually; fix issues; e2e via Computer Use (tests/e2e.ts) with probes (OPTCHAT_PROBES=file).
- .app bundle + codesign; perf measurements (release); theme screenshots.
- Tick PARITY.md items as verified.

## Known issues / notes
- The real DB copy has unbuilt nodes: launching starts Haiku compactions (small spend), as the web app would.
- Root task model is Opus in the copy: tests must use a Haiku child or switch the model first.
