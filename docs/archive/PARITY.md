# PARITY.md — OptChat for Mac (GPUI) vs. the OptChat web app

Definition of done. Derived from `optchat-src` (readme.md, spec-tasks.md, spec-metrics.md, spec-dictation.md,
~/optchat-tasks-spec.md v2.6, lib/*, app/ui/*). An item is ticked only when it is implemented AND verified
(unit/integration test, e2e Computer Use run, or a screenshot I looked at). The verification is noted after `—`.

Legend: `[ ]` open · `[x]` done and verified · `[~]` done, verification partial (reason given)

## A. Storage and schema (lib/db.ts, lib/store.ts)
- [x] A1 Open the project's own copy `data/optchat.db` only (path override via env for tests); never `~/.reframe/data/optchat.db` — own copy data/optchat.db, OPTCHAT_DB override in tests; nothing under ~/.reframe/data
- [x] A2 All 17 tables kept compatible: message, context, request(+turn,+task), turn(+task), log, node, wire, kv, blob(data,mime,filename,size,meta), task, tlog(task,i), tnode(task), metric, metric_event, metric_total, rate(model), model — store_file::fresh_file_schema_matches_the_web_apps_database (column-for-column vs the web DB); hub_realdb loads it
- [x] A3 Schema sync on open: create missing tables/indexes (fresh DB works), never drop — store_file::schema_sync_adds_missing_columns_never_drops (create tables, ADD COLUMN, indexes; rows + unknown columns kept)
- [x] A4 PRAGMA synchronous=FULL; log appends durable before returning — store_file::synchronous_full_and_blobs_stored_once (PRAGMA synchronous = 2)
- [x] A5 Writer lock (pid file `writer.lock` in the app dir); a second live process refuses to open the same chat — <dir>/<instance>/writer.lock like takeLock; store_file::writer_lock_refuses_a_second_live_process
- [x] A6 Serialized writes (no read-then-write race); upsert by id for message/context/request/turn/task/wire/kv/rate/model — single connection mutex, upserts; store::roundtrip_memory
- [x] A7 tlog id `t:i`, tnode id `t:l:i`; first node build wins (insert-if-absent) — t:i / t:l:i, insert-if-absent; store::roundtrip_memory
- [x] A8 Blobs content-addressed (sha256 hex), stored once; image bytes externalized from message bodies (data "" + blob) — store_migrate::a8_put_message_externalizes_image_bytes_into_blobs
- [x] A9 kv: `t<id>:view` {view, batching}, `t<id>:cview`, `t<id>:inbox`, `global:settings`, `root:<sha>` → root context id — store_migrate::a9_kv_keys_view_cview_inbox, metrics_rates::a9_global_settings_kv (root:<sha> in memory_compactor c4_c5_c13)
- [x] A10 metrics emission is one transaction (INSERT OR IGNORE event + ON CONFLICT totals for task and every ancestor, own + `*`) — store_migrate::a10_emission_is_one_transaction_over_the_chain, metrics_rates::a10_h5_emission_chain_and_runtime_rebuild
- [x] A11 `putMetricTotals` (rebuild) replaces the cache in one transaction — store_migrate::a11_put_metric_totals_replaces_the_cache_in_one_transaction
- [x] A12 v3→v4 migration (idempotent, keyed on task 0): log/node → tlog/tnode of task 0, view/cview kv, v2 agents a1–a7 → children #1–#7, request/turn task column — store_migrate::a12_v3_to_v4_migration_on_a_fixture (synthetic v3 database)
- [x] A13 metrics migration (idempotent, keyed on `metric` non-empty): seed rates, backfill ledger from requests/turns, fold totals; later starts upsert new metric definitions — store_migrate::a13_metrics_migration_backfills_the_ledger_once
- [x] A14 `spent` field dropped from task rows on load — store_migrate::x3_a14_b6_engine_loads_the_migrated_fixture (spent stripped)
- [x] A15 Loading 1.5 GB DB: logs + nodes + totals + rates load off the UI thread; first paint before load completes (skeleton) — engine loads on tokio; first frame ~200 ms before chat loaded ~850 ms (perf.json)
- [x] A16 In-memory store for tests (same trait) — Store::memory() used by the harness and roundtrip_memory

## B. Memory machinery (lib/view.ts, optchat.ts Task)
- [x] B1 Parts {t,l,i}; start/end/span; offsets over segments — view::tests::tiling, view_props
- [x] B2 `mostDuePair` due = (T − last)/2^l over global positions, ties → oldest, pairs within one tree, parent must be buildable/built — view_props::d2_fork_of_fork_merge_down_matches_reference (20000 steps), d_ties_and_age_oldest_pair_most_due
- [x] B3 `mergeDown` to target; `tiles` validity check of a saved view — view_props (tiling after every step), view::tests::tiling
- [x] B4 renderBlocks: blocks of 4 lines, `<chat>\n` prefix, `</chat>` closing block, mark on last whole block — view::tests::blocks
- [x] B5 Line format `start+2^l|text` and `start+2^l@t|text` for foreign trees; newlines flattened — memory_compactor::b5_line_format_own_foreign_flattened
- [x] B6 View saved and loaded (never rebuilt), folded once from the log when missing/invalid ("view folded" log line) — store_migrate::x3_a14_b6_engine_loads_the_migrated_fixture (refold + "view folded"), hub_realdb (saved view reused)
- [x] B7 Append: each message appends its line; past HIGH (viewBudget) one batch merges down to LOW (half); `batching` persisted; pending batch runs at load — memory_compactor::b7_high_low_exact_numbers, b7_pending_batch_runs_at_load
- [x] B8 Compactions' view (cview) 16–32 KB sawtooth, re-merged when the view merges or past 32 KB; derived when missing — memory_compactor::b8_cview_sawtooth_16_32_kb, b8_cview_remerged_when_the_view_merges
- [x] B9 `#want`: a pending batch stuck on an inherited pair builds that parent (any reader may build) — memory_compactor::b9_c12_want_builds_the_inherited_parent
- [x] B10 Long texts split at CAP (30,000 chars) over several messages (echo excepted); images are their own leaf messages with a marker text `[image name w×h]` — engine_offline::long_texts_split_at_cap_and_images_are_leaves (CAP 30000 split, echo excepted, `[image name w×h]` leaves)
- [x] B11 appendMany rollback on a failed store write (no phantom entries) — engine_offline::append_rolls_back_when_the_store_fails
- [x] B12 render(skip) leaves out own lines made only of messages delivered as new; settled(skip) — memory_compactor::b12_render_and_settled_with_skip
- [x] B13 Unbuilt queue / mergeReady / wanted queues; rebuildQueues at load — memory_compactor::b13_queues_rebuilt_at_load
- [x] B14 Fork/continue copies the source's view and cview; segments = source.segments ++ {source, cut=T} — view_props::a_b_fork_copies_view_own_lines_append (view, cview, batching, segments)
- [x] B15 Property tests: tiling, no inherited merge past cut, Taelin push order equals reference `due` rule (view-tasks equivalents) — crates/engine/tests/view_props.rs: c (2000 random pushes over fork chains, no inherited part past its cut), d1 (Taelin push = due rule, 20001 steps), d2 (fork-of-fork reference, 20000)

## C. Compactor (optchat.ts #build / #compact, prompts.ts)
- [x] C1 Shared pool of 8 jobs across all tasks; settle-waiting tasks first; first 8 unbuilt per task; ready merges; wanted — memory_compactor::c1_pool_of_8_settle_waiters_first, c1_ready_merges_before_wanted; queues keep insertion order like the TS Sets (engine OrderedSet)
- [x] C2 Level 0: message ≤ 512 B (report lines 280 B) and not an image → its own text, no call — memory_compactor::c2_level0_limits_512_and_280
- [x] C3 Merge: halves joined if ≤ 512 B, else a model call — tasks_offline asserts 0:1:0 built from halves
- [x] C4 Compaction input: builder's cview lines up to the node (stops at first unbuilt), blocks + cache mark, step text (compactMessageTask / compactMergeTask with 512-dash ruler), image attached for image leaves — memory_compactor::c4_c5_c13_the_compaction_request, c4_merge_request_cache_mark_and_image_leaf
- [x] C5 Same SYSTEM prompt and same tool list as turns (tools never called: tool call → error result) — memory_compactor::c4_c5_c13_the_compaction_request (SYSTEM + tools, tool call → error result)
- [x] C6 Too-long retry in the same conversation (`tooLong`), up to 5 tries; best (shortest) kept; cut to limit as last resort — memory_compactor::c6_too_long_retries
- [x] C7 Output cleanup (strip tags/id prefix); empty reply → failure — compactor::tests::cleanup
- [x] C8 Output cap 16,000 tokens (CLAUDE_CODE_MAX_OUTPUT_TOKENS + proxy max_tokens clamp + thinking budget kept below) — local::tests::max_tokens_clamp + engine_offline::proxies_pin_openai_routing_and_clamp_anthropic_tokens (stub upstream)
- [x] C9 150 s hard timeout; runaway abort (>4096 chars or long blank runs) — memory_compactor::c9_timeout_and_runaway_abort (paused tokio clock)
- [x] C10 Failed node → `failed` map, retried at the next message, or after 5 s when a turn waits — memory_compactor::c10_failed_map_retries
- [x] C11 Prefix-writing lock: a call whose marked prefix another call is writing waits for its first response (≤ 90 s) — memory_compactor::c11_prefix_writing_lock
- [x] C12 Node record {l,i,text,size,requestId,tries,by}; `by` when a child builds a parent's node; node event to every task whose timeline holds the tree — memory_compactor::b9_c12_want_builds_the_inherited_parent (`by`, node events to holders)
- [x] C13 Compactor model/effort from the builder's settings (default Haiku 5.5, xhigh) — memory_compactor::c4_c5_c13_the_compaction_request (builder's settings), c13_default_compactor_settings (Haiku 5.5, xhigh)
- [x] C14 Imported v2 tasks named once (titleTask JSON) with TITLE_VERSION — memory_compactor::c14_imported_tasks_titled_once

## D. Tasks engine (spec-tasks, optchat.ts Engine)
- [x] D1 Task record fields (id, parent, title, mode, prev, status, reason, result, settings, budget, segments, createdAt, startedBy, endedAt, interrupted, unrun, imported) — TaskRecord fields; hub_realdb loads 109 tasks
- [x] D2 Statuses pending/idle/active/paused/canceled/done; terminal = canceled|done — six statuses, terminal(); tasks_offline
- [x] D3 Root created on an empty DB; restart: active root → idle, active child → paused "interrupted by a restart" + interrupted — engine_offline::restart_demotes_active_tasks
- [x] D4 spawn(mode fork/continue/handoff/blind, title lowercased ≤80, instruction/images, prev, budget, settings, start) — lifecycle_more::d4_d5_d6_spawn_modes_checks_settings (modes, lowercase, ≤80, images, prev, budget, start)
- [x] D5 spawn checks: parent not terminal, depth > 0, mode valid, title required, instruction or image required, prev a terminal child of the parent (continue only), unknown model refused, effort normalized — lifecycle_more::d4_d5_d6_spawn_modes_checks_settings (every check with the TS texts)
- [x] D6 Child settings = parent's + partial override; depth = min(parent−1, given) — lifecycle_more::d4_d5_d6_spawn_modes_checks_settings (merge + depth clamp)
- [x] D7 Budget defaults to what the parent has left (usd/minutes/tokens), refuses more; deadline ≤ parent's; pending keeps deadlineMinutes until start — engine_offline::child_budget_defaults_to_what_the_parent_has_left
- [x] D8 readable(X) = up(X) ∪ subtree(X) (up stops at root/blind, follows prev for continue) — readable(); tasks_offline zoom across fork, blind not readable
- [x] D9 Per-turn header `<task>` text exactly as the TS (identity, memory readable, children (live + last 8 ended), depth, budgets left, deadline, instructions) — lifecycle_more::d9_d31_header_describe_task_list_identical_to_the_ts (full header text vs TS-generated fixtures)
- [x] D10 Turn input `[view pieces][header][new messages][images]`; cache text = last whole block — lifecycle_more::d10_turn_input_layout_and_cache_mark
- [x] D11 Inbound kinds user/parent/task/go; active vs passive delivery; mid-turn inject at the next tool boundary (logged with turnId) — lifecycle_more::d11_inbound_kinds_inject_active_passive (user/parent/task/go, inject at the tool boundary with turnId)
- [x] D12 Inbox persisted (kv), queue summary emitted ("N queued") — lifecycle_more::d12_inbox_persisted_with_queue_summary
- [x] D13 Scheduler: at most maxTurns turns at once (global, default 4); waiting tasks are `active` — lifecycle_more::d13_scheduler_max_turns
- [x] D14 Settle: a turn waits until the view's parts are built (status "settling"); a stop while settling cancels — lifecycle_more::d14_settle_then_stop_or_pause (settling status, no agent while settling, stop cancels, pause keeps the message); settle → run in every model test
- [x] D15 endTurn: heartbeats flushed, turn row saved, computer released, report to parent (talk/error/status), status → idle/active, kick — lifecycle_more::d15_end_turn (beats, turn row, computer released, report, status, kick)
- [x] D16 Reports child→parent: `#C talk|user|error|status: …` logged whole, eager 280-byte line (3 tries then a cut line), delivered active/passive per §6; image leaves — lifecycle_more::d16_reports_and_eager_lines (all four kinds, 280-byte eager line, 3 tries then a cut line)
- [x] D17 userSend in a child → passive `#C user` report — engine_offline::user_send_in_a_child_reports_passively; tasks_offline asserts the `#C user` report text
- [x] D18 pause (cascade, skips pending/terminal/paused; interrupts running turns; interrupted flag; `#C status` when by user) — tasks_offline + lifecycle::pause_mid_turn_then_resume
- [x] D19 resume (cascade; message; budget raise with parent limits; over-limit refused; RESUMED or `go` when unrun) — engine_offline::resume_budget_over_the_parents_is_refused, resume_says_resumed_or_runs_what_waits
- [x] D20 cancel / close (cascade with reasons "an ancestor was canceled/closed"; inbox flushed into the log; result ≤ 280 lowercased; root can't end) — engine_offline::cancel_and_close_cascade (ancestor reasons, inbox flushed, 280 cut, lowercase, root refused)
- [x] D21 Pending: spawn start:false logs instruction with unrun; append (↑) logs at once and sets unrun; start (→ empty) flips pending → idle with `go`; kick ignores pending; pause/resume refuse pending — tasks_offline (pending, append, refusals) + flow.ts (Add / ⌘Enter / Run)
- [x] D22 tell(run, settings): run:false → append; paused + text → resume with it; empty + run → start; settings-only tell — engine_offline::tell_paths (all four)
- [x] D23 Continue after a stopped/failed turn (empty →) sends the CONTINUE text — engine_offline::continue_after_a_failed_or_stopped_turn
- [x] D24 stop(x): cancels the running turn ("stopped (by the user)"), `#C status: turn stopped by you` — engine_offline::stop_pause_cancel_texts_of_a_waiting_turn (TS texts; mid-run stop exercised in lifecycle model tests)
- [x] D25 stopTool(callId): stops one tool, the turn goes on with "(stopped by the user)" — converse::tests::stop_tool_marks_the_result
- [x] D26 setSettings: merge, model check, effort normalized, compactor check, depth clamp, unchanged no-op; returns now / next turn / saved — lifecycle_more::d26_set_settings (merge, model check, effort, compactor, depth, unchanged; now / next turn / saved)
- [x] D27 Mid-turn model switch: live settings reach the next model call on the same backend (Claude via proxy body rewrite, Codex via thread-id map); cross-backend waits for next turn; request row records the model that ran — lifecycle_more::d27_mid_turn_switch_offline (Codex thread map, Claude rewrite, cross-backend); live: lifecycle::codex_mid_turn_effort_switch (gpt-6-luna), cross_backend_switch_waits_for_next_turn, mid_turn_model_switch
- [x] D28 Budgets: usd/minutes/tokens on subtree totals, deadline; exceeded → pause subtree (skipping pending) + active `#C error` naming the metric (root: error event); overLimits at turn start; deadline checker every 10 s — engine_offline::deadline_passed_pauses_the_subtree_and_reports, limits_over_at_turn_start_refuse_the_turn; lifecycle::budget_exceeded_pauses_and_reports
- [x] D29 Computer lock: one driver at a time, from the first computer call until the turn ends; waiters — engine_offline::computer_lock_one_driver_fifo
- [x] D30 Hub-swap fence equivalent: a disposed engine never writes (fenced store); drain catch fails the turn visibly and idles the task — lifecycle::disposed_engine_never_writes; store fence in roundtrip_memory
- [x] D31 `describe` / task_list text identical to the TS — lifecycle_more::d9_d31_header_describe_task_list_identical_to_the_ts (describe / task_list full text)
- [x] D32 Turn record (startedBy, backend, model, effort, status waiting/running/done/failed/cancelled, userIndexes, requestIds, usage, cost, error, timestamps) — lifecycle_more::d32_d33_d34_d36_records_contexts_log (turn record fields/statuses)
- [x] D33 Request record per model call (purpose, backend, provider, model/effort live, contextId, outputId, status, stop, usage, cost, wireId, timing, task) — lifecycle_more::d32_d33_d34_d36_records_contexts_log (request record fields)
- [x] D34 Canonical contexts: root context per system prompt (kv `root:<sha>`), parent chain, marks; tool results and injected messages appended — lifecycle_more::d32_d33_d34_d36_records_contexts_log (root context per prompt, chain, marks, results, injects)
- [x] D35 contextMessages(contextId) for the debugger (parent chain, envelopes with children) — context debugger lists the canonical context with envelopes (panels.ts ctx, panels-10/11)
- [x] D36 Everything the agent says/does logged as it happens (talk, tool `name {json}` with meta callId/name, echo with isError, echo images as leaves unless already in the log); thinking never logged — lifecycle_more::d32_d33_d34_d36_records_contexts_log (talk, tool meta, echo isError, echo images, no thinking)

## E. Agents (lib/agents.ts)
- [x] E1 Claude Code per turn: `claude` with stream-json in/out, `--include-partial-messages`, `--tools ""`, `--setting-sources=`, `--strict-mcp-config`, `--no-session-persistence`, `--mcp-config` (loopback http), `--allowedTools mcp__o__*`, `--model`, `--effort`, `--thinking-display summarized` — model_turns::haiku_turn_tool_and_child_report
- [x] E2 Claude env: clearEnv; HOME, PATH (incl ~/.local/bin, /usr/local/bin, /opt/homebrew/bin), CLAUDE_CONFIG_DIR app claude-home, CLAUDE_SECURESTORAGE_CONFIG_DIR="", ANTHROPIC_BASE_URL loopback, the disable flags, MCP timeouts, CLAUDE_CODE_MAX_OUTPUT_TOKENS for compactions — model_turns (env_clear + PATH + config dirs + base URL)
- [x] E3 System prompt via `initialize` control request; user message with view pieces; inject with `priority: next`; interrupt control request + kill after 3 s; followUp for compactions — lifecycle_more::e3_claude_protocol (fake claude: initialize, view pieces, inject priority next, interrupt + kill, follow-up)
- [x] E4 Steps from stream events: message_start usage (incl. 1h cache writes), deltas (text/thinking), message_delta stop/usage, assistant blocks, result → cost report (modelUsage), errors — model_turns: usage incl. 1 h writes, cost from modelUsage
- [x] E5 Tool call waits for its step to finish before running (ordering of onStepEnd before runTool) — haiku tool turn
- [x] E6 Codex: one shared `codex app-server` (JSON-RPC stdio), args (model_catalog_json, notify=[], web_search disabled, optchat provider at loopback proxy, `--disable` list, user MCP servers disabled), initialize with experimentalApi — model_turns::codex_luna_turn; smoke.ts GPT-6-Luna turn through the UI
- [x] E7 Codex catalog copy with code mode off (tool_mode null, apply_patch null, …) — catalog copy used by the luna tests
- [x] E8 Codex turn: thread/start (ephemeral, baseInstructions, dynamicTools, approval never, danger-full-access, experimentalRawEvents), turn/start with input text/images, turn/steer, turn/interrupt, rawResponseItem/completed → steps, rawResponse/completed usage → step end + wire, turn/completed — lifecycle_more::e8_codex_protocol (fake app-server: thread/start params, turn/start, steer, interrupt, raw events, usage)
- [x] E9 Codex server requests routed by thread id; item/tool/call → runTool; other requests declined — codex tool call routed to run_tool (luna)
- [x] E10 Adapters: Anthropic blocks ↔ canonical (text, thinking+signature, redacted, tool_use with mcp__o__ prefix, tool_result, image, document); Responses items ↔ canonical (message, reasoning encrypted, function_call, custom_tool_call, outputs) — adapters unit tests equal to adapters.ts output (fixtures/ts_adapters.json, both directions)
- [x] E11 Subscription logins only (no API keys anywhere); PATH set by the app — subscription logins only; PATH via child_path

## F. Loopback server (lib/local.ts)
- [x] F1 127.0.0.1 random port, secret path; 404 otherwise — local::tests::mcp_endpoint
- [x] F2 MCP endpoint per run: initialize, tools/list, tools/call (callId from `_meta.claudecode/toolUseId`), ping, notifications 202, GET 405, DELETE 200 — local::tests (initialize, list, call with _meta callId, ping, notifications 202, GET 405, DELETE 200)
- [x] F3 Anthropic proxy: records wire body, cache layout (drop all cache_control; mark view block(s) ≤2, system, last block; 1h ttl; ≤4), live settings rewrite (model + output_config.effort), max_tokens clamp + thinking budget for compactions; streams the response back — local::tests cache_layout / live_settings / max_tokens_clamp + stub-upstream proxy test; streaming via the model tests
- [x] F4 OpenAI proxy for Codex: live settings by thread-id/session-id header (model + reasoning.effort), prompt_cache_key + session-id/thread-id pinned to `optchat-<hash(lead)>`, x-codex-window-id dropped, upstream chatgpt.com/backend-api/codex for ChatGPT logins — local::tests::openai_pinning + stub-upstream test (prompt_cache_key, session/thread ids, window id dropped)
- [x] F5 Unit tests for applyCacheLayout / applyLiveSettings / openai rewrite with a stub upstream — engine_offline::proxies_pin_openai_routing_and_clamp_anthropic_tokens (axum stub upstream on 127.0.0.1)

## G. Tools (lib/tools.ts, task-tools.ts, computer.ts, mcp-client.ts)
- [x] G1 bash: bash -lc in its own process group, timeout (default 120 s, max 600 s), kill group on timeout/stop (TERM then KILL), partial output, head/tail keep, exit code text — tools::tests bash_basics, bash_timeout_kills_group, bash_cancel, background child
- [x] G2 read_file (numbered lines, offset/limit, "[N more lines]"), write_file (mkdir -p), edit_file (unique / replace_all), glob (globToRegExp, ** recursion, 500 cap, skip .git/node_modules), grep (grep -rnE -I), web_fetch (30 s, HTML → text) — tools::tests incl. web_fetch against a local stub server (timeout, HTML → text, invalid URL text)
- [x] G3 cap(): 30,000 chars head 60% / tail with "[... N characters cut ...]" — tools::tests::cap_head_tail
- [x] G4 zoom(id, n, task?) and date(id, task?) with readable checks and exact error texts; zoom n=1 returns image parts — task_tools::tests::date_matches_js_to_string + engine_offline::date_tool_matches_js (zone name like V8); zoom inputs as JS
- [x] G5 task_spawn / task_tell / task_pause / task_resume / task_cancel / task_close / task_list with the exact schemas and descriptions (byte-identical tool list across tasks) — engine_offline::tool_specs_byte_identical_to_the_ts (all 18 tools: order, group, description, schema bytes; fixture generated from optchat-src)
- [x] G6 images arg: {id, task?} from readable memory or {path} files (png/jpeg/gif/webp ≤ 5 MB) — lifecycle_more::g6_images_argument ({id, task?}, {path}, formats, 5 MB, TS error texts)
- [x] G7 Tool groups per settings: core / computer (only models marked capable, cua available) / mcp; depth 0 removes task tools — lifecycle_more::g7_g9_tool_groups_and_reframe_mcp (core / computer capable+available / mcp; depth 0 drops task tools)
- [x] G8 Computer use: spawn ChatGPT.app's cua_repl MCP over stdio with the env; `computer` (js) and `computer_reset` tools; turn_ended hook; elicitation → approvals — lifecycle_more::g8_computer_use_fake_cua_and_approvals (spawn args/env, computer/computer_reset, elicitation → approvals)
- [x] G9 reframe MCP over streamable HTTP (JSON or SSE), tools re-exported as `reframe_*` (implemented; disabled in my test runs by env so tests never touch it) — lifecycle_more::g7_g9_tool_groups_and_reframe_mcp (streamable HTTP stub, JSON and SSE, reframe_* names)
- [x] G10 Tool call metrics (time.tool, calls.tool) and running-tool registry for reconnect/stop — calls.tool metric asserted in the haiku test

## H. Metrics, rates, models (spec-metrics)
- [x] H1 METRICS definitions (12 incl. audio.ms); kinds chat/compact/dictation — 12 metric definitions
- [x] H2 emit(task, source, items, meta): idempotent `<source>:<metric>` ids, own + subtree for chain, per kind and `*`, change callback → task event + budget check — metrics::tests::fold_invariants + budget test
- [x] H3 charge per model call: usd, tokens.in/cache_read/cache_write/out, time.model, calls.model, calls.unpriced; meta model/effort/provider/backend/request/turn/rate/ratePer1M/cacheWrite1h (+node, tries for compactions) — charge meta; mid_turn_model_switch checks the ledger model
- [x] H4 time.active heartbeats every 10 s + flush at end; source `turn:<id>#n` — time.active > 0 in the haiku test
- [x] H5 rebuild folds the ledger — metrics_rates::a10_h5_emission_chain_and_runtime_rebuild (Engine::metrics_rebuild)
- [x] H6 Rates: inForce(model, at), history, add, price (long context, 1h writes), SEED_TABLE, syncPublished at start (skip manual), calibrate from Claude cost reports (2% threshold, 3 agreeing reports, never over manual), reprice unpriced calls once — metrics_rates::h6_in_force_history_add_price, h6_seed_table_is_the_published_table, h6_sync_published_skips_manual, h6_calibrate_rules, h6_reprice_unpriced_once
- [x] H7 Model catalog: Claude list + Codex models_cache.json (visibility, efforts minus ultra, default effort, computer use not luna); `model` table refresh at start, hidden flag — metrics_rates::h7_claude_list_and_codex_models_cache, h7_catalog_table_refresh_and_hidden (fixture models_cache)
- [x] H8 spendByModel from the ledger (cached 30 s) — metrics_rates::h8_spend_by_model_cached_30_s
- [x] H9 metrics.breakdown by kind / model / source (own vs subtree) — metrics_rates::h9_breakdown_by_kind_model_source

## I. Dictation (spec-dictation, lib/stt.ts, keyterms.ts)
- [~] I1 Native microphone capture (CoreAudio via cpal), mono, resampled to pcm16 16 kHz, 100 ms chunks with level; recording starts at the press; buffered until the service is ready — cpal capture (device test ignored, needs mic permission); e2e uses the fake mic
- [x] I2 Claude voice_stream client (keychain token, headers, keyterms header, KeepAlive every 8 s, CloseStream, Transcript* messages) — stt::tests::live_claude
- [x] I3 ChatGPT dictation client (codex auth.json, subprotocols, session.start config, 24 kHz resample, audio.append base64, 700 ms silence before session.close, transcript.segment/final) — stt::tests::live_codex
- [x] I4 Session: service order from global setting with the other as fallback; failure before ready → next service with all audio; drop after text → keep text, one reconnect re-sending last 1.5 s, overlap merge (≤ 12 words) — stt::tests::live_fallback, live_drop, merge_overlap
- [~] I5 A new dictation finishes the previous; done event with text/service/audioMs — done once; "a new dictation finishes the previous" untested
- [x] I6 keyterms(): sources (title ×3, related titles ×2, last 40 log entries weighted, tool files only, echoes never), candidates by shape, ranking, hints ≤ 20 (base terms first), words ≤ 60, spoken(), rewriter() — keyterms::tests::same_as_typescript
- [~] I7 `audio.ms` metric (kind dictation) with meta service/keyterms/chars — audio.ms emitted; not asserted
- [~] I8 UI: mic button states idle / connecting (spinning ring) / listening (red, 4 level bars, pulse, red composer outline) / finishing (bars settle); no labels — mic states drawn; listening shot (feat-01)
- [~] I9 Gesture rule: press while idle starts; release > 300 ms later stops (push-to-talk), earlier = tap (keeps running); press while dictating stops — 300 ms gesture rule; taps exercised only
- [x] I10 M key outside text fields (tap/hold); inside the composer M types "m"; Esc stops and keeps text — features.ts: M starts dictation outside a text field, Esc stops and keeps the text
- [x] I11 Text lands at the cursor/selection (before + text + after with spacing), composer read-only while dictating, caret after the text at the end — features.ts: dictation wrote at the cursor
- [~] I12 Enter while dictating: stop, wait for final text, then → ; ⌘Enter: stop then ↑ — Enter while dictating waits for the final text; untested (would start a model turn)
- [~] I13 Errors in the composer error line for 6 s ("microphone access is blocked", "dictation is unavailable: log in…") — dictation errors expire after 6 s (state.dict.error + redraw timer), engine errors stay like the web; not exercised (needs a failing service)
- [x] I14 Engine test with a WAV file through the session (both services), plus fallback test — stt live tests on a WAV (Claude, Codex, fallback, drop)

## J. App shell and layout (app.tsx)
- [x] J1 Native macOS window, title "<task name> · #id · OptChat"; restores size; min size — inputs.ts: relaunch restores the window size; title follows the task (and renames)
- [~] J2 Activity rail: logo (opens main chat), Tasks (badge with active count), Memory (dot while compacting), Context, Log, Models, spacer, Settings, sidebar toggle — rail items, badge, memory dot; badge not seen non-zero
- [x] J3 Rail click on the active view hides the sidebar; ⌘B toggles it — panels.ts ⌘1–⌘6 and ⌘B
- [~] J4 Resizable sidebar (220–760 px), width remembered; sidebar head with view title and per-view actions; hide button — resizer 220–760, width saved; drag untested
- [~] J5 Editor tabs of open tasks: status dot (root: chat icon), title, spinner while running, close button, middle-click closes; closing the last tab leaves the main chat; active tab scrolled into view — tab dot/title/spinner/close/middle-click, active tab scrolled into view; ⌘W tested
- [x] J6 Tab actions at the right: pause (not paused/pending/terminal), resume (paused), close as done (non-root, non-terminal), cancel (non-root), new task (non-terminal) — flow.ts pause/resume/close/cancel from the tab actions
- [~] J7 Task bar: breadcrumb (ancestors clickable, "main chat"), continues #prev link, result (→ …) or status: reason — breadcrumb, continues link, result/status; continues link untested
- [x] J8 Task bar ask mode: label, input (autofocus, max 280 for close), "and its subtree" hint, confirm button (danger for cancel), Esc; Enter confirms, Esc cancels; required result for close — inputs.ts: close result stops at 280, other ask inputs up to 2000
- [x] J9 Remembered per-user prefs: view, sidebar open/width, task scope, show ended, settings tab, theme (local prefs file) — inputs.ts relaunch (prefs file restored)
- [x] J10 Open tasks persisted across launches (better than web: reopen last tabs) — inputs.ts: relaunch reopens the tabs and the current task
- [~] J11 Toast notices (controlDone ok 2.6 s / error 6 s) — toasts 2.6 s / 6 s; not asserted
- [x] J12 Approval dialog (computer use): message, task, tool params, Deny / Allow once / Allow this session / Always allow; queue of approvals — features.ts approval dialog (allow this session)
- [x] J13 Lightbox for images (click to enlarge, click/Esc to close) — chat.ts lightbox opens and Esc closes
- [~] J14 Skeletons until a task's data is loaded; never the empty state for the root — skeletons until loaded; not asserted
- [x] J15 Phone layout N/A on Mac → narrow-window layout: the sidebar floats over the editor below ~760 px — panels.ts narrow window sheet + Esc

## K. Tasks view (sidebar) (tasks.tsx TasksView)
- [x] K1 Scope "here": ancestors path rows above, the current task, its subtree; "all": the whole tree from #0 — tasks here/all; tour shots
- [~] K2 Row: chevron (collapse), status dot (idle grey, active pulsing accent, paused amber, canceled red, done green, pending hollow ring), title, line 2 `#id · $cost · active time · mode · status word (working/waiting/status)`, spinner when running; current highlighted; ended dimmed; tooltip with title/mode/status/reason/result — rows complete; dot tooltip missing (row tooltip has the details)
- [x] K3 Toggle all/here, show/hide finished, new task (+) buttons in the sidebar head — scope / show-ended / + in the head
- [~] K4 "no subtasks yet" hint — "no subtasks yet"; not seen in a test
- [~] K5 Click opens the task in a tab (instant, same engine) — row click opens; switcher-open tested

## L. Feed (chat.tsx)
- [x] L1 Virtualized variable-height list (gpui `list`) of turn groups; smooth scroll of 2000+ messages — gpui list; perf.ts in-app 120 Hz gesture on the 1749-message chat: work/frame p50 3.5 ms, p95 4.9 ms, max 6.6 ms; frame interval at the 60 Hz display's vsync
- [~] L2 Groups by turnId; "Load earlier messages (N more)" (or infinite load) for older history — paging: last 120 entries + "Load earlier messages (N more)", infinite load at the top; not asserted
- [x] L3 User bubble (right, pre-wrap) with #i and time; note kind same style — user bubble (flow-04)
- [x] L4 Parent bubble "↳ from #P / from you, as #P / from the main chat" (clickable), markdown body, image leaves — parent bubble (flow-02/04)
- [~] L5 Report card `#C replied | you wrote to it | error | status`, child title, #i · time, open ↗; long bodies clipped at 320 chars, click to expand; colored by kind; image reports — report cards; expand untested
- [~] L6 v2 work messages `[a1] …` rendered as report blocks linking to #1..#7 — v2 work rows; no test
- [~] L7 Talk: markdown (headings, lists nested/loose, task lists, tables, code blocks with language bar + copy, inline code, links open in browser, blockquotes, emphasis, strikethrough) — markdown parse test (headings, lists, tables, code, quotes, links); render not asserted
- [~] L8 Live streaming text with a cursor; thinking shown live (collapsible "thinking…" / "thought" with last line preview), never logged — live text + thinking; seen during smoke runs only
- [~] L9 "waiting for the model…" and "waiting for summaries: N left (k in progress) · retrying · elapsed" states — "waiting for the model…" seen (smoke)
- [x] L10 Tool card: chevron, name, arg preview per tool (bash command, computer title, zoom id+n@t, task_* previews…), state running (elapsed) / done / error, per-tool stop button, #i–#j, task id chips for task_* tools, expandable input/output (+ images), "shortened · show all N chars" — chat.ts: a tool card expands to its input / output; task_spawn previews from cut inputs (lenient_tests)
- [~] L11 Echo images under their tool card; lone echoes as cards — echo images; unverified
- [x] L12 Image messages (blob from DB) with caption, click to lightbox — chat.ts: image message shown and opened in the lightbox
- [x] L13 Turn foot chips: model · effort (with a → b when switched), N steps ▾ (step table: #, model if switched, stop, in, cached, write, out, cost, time; click → context), in (cached %), out, cost (amber unpriced), duration, live, stopped, context ↗ — chat.ts: steps chip opens the step table, context ↗ opens the Context view on that call
- [~] L14 Turn error line "⚠ …" — turn error line; unverified
- [x] L15 "not run yet" divider at rec.unrun — NOT RUN YET divider (flow-04)
- [~] L16 Empty states: child "This task has no messages yet. Its first turn is starting."; root intro (only when the root really has nothing) — empty states; not seen
- [~] L17 Follow mode: autoscroll while at the bottom; stays put when scrolled up; jump-to-bottom affordance — follow mode (bottom-pinned list), jump-to-bottom button, sending re-pins; not asserted
- [~] L18 Cut entries (slim copy) with on-demand full text — cut texts + "show all"; untested
- [x] L19 Endbar for terminal tasks: dot + "This task is done: result / canceled (reason). Its memory stays readable." + "Continue as a new task" (opens dialog preset continue/prev) — flow.ts end bar + Continue; chat.ts Continue presets the dialog to continue

## M. Composer (app.tsx Composer)
- [x] M1 Multi-line input (IME, selection, undo), auto-grow to 260 px, draft per task — inputs.ts: composer grows 39 → 131 px at 5 lines, stops at ~250 px, shrinks when cleared
- [x] M2 Placeholder `Message #N…` / `Message OptChat…` — inputs.ts relaunch on #92: placeholder "Message #92…"
- [x] M3 Buttons: attach image · spacer · Stop (running) · mic · ↑ (append, outlined; hidden while running) · → (run, primary accent) — composer buttons (smoke/flow shots)
- [x] M4 → enabled with text/image, or runnable (unrun, pending, paused, or last turn stopped/failed); ↑ enabled only with content — run/append enable rules (flow-04)
- [~] M5 Keys: Enter = →, Shift+Enter newline, ⌘Enter = ↑ (→ while a turn runs); IME composition respected — Enter / Shift+Enter / ⌘Enter (flow, inputs); IME not checked
- [~] M6 Images: attach picker (NSOpenPanel), paste from clipboard, drag & drop files; downscale to 1568 px longest side; re-encode jpeg/png; thumbnails with remove ×; max 20 — attach (test hook), paste, drop, 1568 px, ×, max 20; only the hook path tested (features.ts asserts the attachment)
- [~] M7 Error line above the composer (last engine error / dictation error) — error line (engine errors stay, dictation errors 6 s); not asserted
- [~] M8 Focus the composer on task switch — focus on switch; inputs relaunch shows focus "composer"
- [~] M9 Disabled until the task is loaded — disabled until loaded; untested

## N. Status bar (tasks.tsx StatusBar)
- [~] N1 Left: connection-equivalent status dot + status text (running / settling / waiting for a slot / task status), `#id mode` (child), `summarizing N` with spinner, `N working`, `N queued` — status texts; queued/slot/settling not seen
- [~] N2 Right: cost (subtree) with budget bar (accent/amber/red) and `/ $budget`, active time `/ budget`, deadline minutes (red when passed), tokens, msgs (+inherited tooltip), depth, model select (grouped Claude Code / Codex), effort select — cost/time/tokens/msgs/depth + grouped model menu (panels-02); budget bar not seen
- [x] N3 Running tint (indigo) and paused tint — running/paused tints (smoke, flow-05)
- [x] N4 Metrics popover: one row per metric own · subtree, chat · memory split rows, budget fields ($, agent-min, tokens, deadline min), Set budget / Clear, Enter/Esc — panels.ts budgets popover
- [x] N5 Model/effort selects hidden on terminal tasks; work on pending tasks — selects hidden on done, shown on pending (flow)

## O. New-task dialog (tasks.tsx)
- [x] O1 Title "New task under the main chat / #N title", explainer line — title + explainer (flow-01)
- [x] O2 Mode cards handoff / fork / continue (disabled without terminal siblings) / blind with descriptions — chat.ts: each mode card selects its mode
- [~] O3 Continue: which task select (terminal siblings) — continue's which-task select; untested
- [~] O4 Title (≤ 80), instruction (multi-line, image paste), Attach images, thumbnails — title ≤ 80 and the growing instruction box (inputs.ts); image paste, × and the 20 cap implemented, not tested
- [x] O5 Model select (inherited marker), $ budget, minutes — model (inherited), $ budget, minutes (flow-01)
- [~] O6 Footer Cancel · Add (pending) · Start (primary, ⌘Enter); disabled until valid — Cancel · Add · Start, ⌘Enter; ⌘Enter untested
- [x] O7 After spawn the new task opens in a tab — flow.ts spawned task opens in a tab

## P. Memory view (panels.tsx MemoryPanel)
- [~] P1 Summary card: messages (own + inherited), tree nodes built, view lines (inherited count) and bytes, batches at HIGH → LOW, compactions' view lines/bytes, fill bar (amber while batching) with half marker, "batch pending" note, level legend (count × 2^l msg, level colors) — summary card (tour-memory)
- [~] P2 Compaction jobs: running nodes in this timeline (spinner, click reveals), failed (retrying), count in other tasks — jobs chips; running/failed not seen
- [x] P3 Search box (Enter) over readable trees' messages; hits (latest first, kind tag, text) → reveal the path — panels.ts memory search → hit reveals message in the tree (asserted)
- [~] P4 Toggle the view / compactions' view; "raw" shows the rendered view text, blocks, mark index, marked chars and the header — view/cview toggle, raw (feat-10 shot)
- [x] P5 Tree rows: chevron, level color bar, `start+n`, `@t` badge for foreign trees, text; spinner when being built; retry badge when failed; unbuilt style; open → two halves (zoom) down to the message (full text, kind, date, image) — chat.ts memory tree opened down to the message, picture shown (chat-04b-memory-image)
- [~] P6 Rows refresh as nodes get built — rows refresh as nodes are built; no test

## Q. Context debugger (panels.tsx ContextPanel)
- [~] Q1 Shows the selected request (follows latest chat request unless pinned; "follow latest" button) — follows latest / pinned via context ↗ (chat.ts); "follow latest" button untested
- [x] Q2 Header chips: model · effort, backend, status · stop, cost — header chips (panels-10)
- [x] Q3 Usage bar (cache read green, write amber, uncached accent) and legend with reasoning tokens — usage bar + legend (panels-10)
- [x] Q4 KV: input tokens + cached %, cost split, first token, total time, started, canonical context size — KV rows (panels-10)
- [~] Q5 Context sent: canonical messages oldest first with context separators, roles, kinds, previews, sizes, raw marker, expandable full text, images, envelopes with children, cache breakpoint markers — canonical messages incl. image thumbnails (shared blob cache); image rows not exercised
- [x] Q6 Output section; wire request (show/hide, pretty JSON) — panels.ts wire request shown (asserted)
- [~] Q7 Running call message ("still running") — "still running" message; not exercised

## R. Log view (panels.tsx RequestsPanel + Breakdown)
- [x] R1 Scope this task / with its subtree — panels.ts subtree toggle (asserted)
- [~] R2 Breakdown tables by kind (memory/chat labels), by model, by source (turns, compactions, tools, children) with cost bars, tokens, time; events count — breakdown tables (panels-08)
- [x] R3 Totals card: cost (ledger), turns/compactions loaded, input tokens + cached %, output tokens, note — totals card (panels-08)
- [~] R4 Filter all / chat / compact; table when · task (subtree) · model · in · cache % · out · cost; click selects → Context view — filter + click → Context; untested
- [x] R5 Server log (in Settings > Global) — server log in Settings > Global (panels-12)

## S. Models view (models.tsx)
- [~] S1 Providers grouped (Anthropic · Claude Code, OpenAI · Codex); count of runnable models; refresh catalog button; show/hide hidden — models panel (panels-05)
- [~] S2 Model card: label, id, spend (ledger, tooltip calls/tokens), tags (not offered now, hidden, computer use, efforts), rate in force (in / cache read / cache write / write 1h / out), long-context line, source chip + since date + note, or "No price yet…" — model card (panels-05)
- [x] S3 Edit rate (fields, Anthropic ratios auto-filled from base input, note) → new manual row; history table; hide/show from pickers (pickers update) — panels.ts rate edit saved to the database (asserted in the DB)

## T. Settings (panels.tsx SettingsPanel)
- [x] T1 Two tabs: Global and the task's ("Main chat" / "Task #N"); last tab remembered — panels.ts settings Global / Task tabs
- [x] T2 Global: turns at once (1,2,3,4,6,8,12,16), dictation service Claude/Codex, server log, build id — turns, dictation service, server log, build (panels-12)
- [~] T3 Global (GPUI addition): theme picker (+ follow system appearance), font size — theme picker, follow system, chat text size control (−/+/reset); size control not exercised in a test
- [x] T4 Task: instructions (saved on blur), depth select (0..parent−1, "(no task tools)"), compactor model + effort, tools toggles (core, computer with availability text, reframe MCP), computer approvals ask/always, tool list by group, working directory, view budget (≥ 2000) — task settings (panels-13)

## U. Keyboard, commands, polish (GPUI additions beyond the web app)
- [~] U1 Command palette (⌘K / ⌘⇧P): every command (open task by title, new task, pause/resume/close/cancel, switch view, theme, toggle sidebar, focus composer, stop, models, settings) — palette: tasks, commands, themes, models, test hooks; lifecycle commands untested
- [~] U2 App menus (OptChat, File, Edit, View, Task, Window, Help) with shortcuts — menus: OptChat (About, Settings, Services, Hide, Hide Others, Show All, Quit), File, Edit, View, Task, Window (Minimize, Zoom, Full Screen, tabs), Help (shortcuts, commands, data folder); not driven by a test (the menu bar is OS UI)
- [~] U3 Shortcuts: ⌘B sidebar, ⌘1..⌘6 rail views, ⌘N new task, ⌘W close tab, ⌘⇧[ / ⌘⇧] prev/next tab, ⌘. stop, ⌘L focus composer, ⌘, settings, ⌘T task switcher, Esc closes dialogs/popovers — all bindings; ⌘1–6 ⌘B ⌘T ⌘W ⌘N ⌘= ⌘⇧0 ctrl-tab Esc tested; ⌘⇧[ ⌘⇧] ⌘. ⌘L ⌘, untested
- [x] U4 Context menus: task rows / tabs (open, pause, resume, close, cancel, new subtask, copy id), messages (copy text, copy markdown), tool cards (copy input/output) — chat.ts: task row and tab menus with the web's actions (Open, Pause…, Resume…, Close…, Cancel…, New subtask…, Copy #id); messages / tool cards have Copy
- [~] U5 Smooth animations: spinners, pulsing dots, mic pulse, popover/dialog fade, chevron rotate — spinners / pulses (12.5 Hz clock), mic pulse, toast, dialog/popover fade-in (120 ms), chevrons rotate without animation
- [~] U6 Focus handling: dialogs trap focus, Esc returns to composer, Tab order in dialogs — Esc returns focus to the composer; focus restored to open dialogs; no Tab-order trap
- [x] U7 Tooltips on icon buttons — tooltips on icon buttons (tour shots)

## V. Themes
- [x] V1 Theme system with live switching (no restart) applied to every view — features.ts: every bundled theme applied live
- [x] V2 Zed theme JSON format loader (theme family files with `themes[].appearance/style`), license noted (Zed themes: GPL/Apache per repo; noted in THEMES.md) — crates/theme Zed loader + unit tests; THEMES.md licenses
- [x] V3 Themes: OptChat Dark (current web palette), OptChat Light, Solarized Light, Solarized Dark, Tomorrow Night, Tomorrow Night Blue, One Dark, One Light, Ayu Dark/Light/Mirage, Gruvbox Dark/Light (Zed built-ins) — 17 themes; theme::tests::all_bundled_parse_and_order
- [~] V4 Follow system appearance by default (light/dark pair), live on appearance change — follow system by default; a live appearance flip not tested
- [x] V5 Screenshot of every theme — features.ts: all 17 bundled themes applied and screenshotted (shots/theme-*.jpg)

## W. Packaging, performance, process
- [x] W1 .app bundle com.reframe.optchat.gpui, Info.plist (NSMicrophoneUsageDescription), icon, ad hoc codesign; launchable from Finder — bundle.sh: com.reframe.optchat.gpui, mic usage string, icon, ad hoc signature; every e2e launches the .app
- [x] W2 PATH set by the app (~/.local/bin, /usr/local/bin, /opt/homebrew/bin + system) — PATH set by the app
- [x] W3 No UI-thread stalls: DB/IO/network on the engine runtime; UI receives coalesced events — perf.json: per-frame work ≤ 6.6 ms while scrolling, task switch 14–34 ms (in-app), idle with a spinner ~10% CPU (12.5 Hz ticker)
- [x] W4 Measured: launch time to first frame and to loaded chat, frame times while scrolling a 2000-message chat, memory; release build — perf.ts (release): first frame ~200 ms, chat loaded ~850 ms, RSS ~185 MB, scroll work and intervals, switches
- [x] W5 Graceful shutdown: running turns cancelled, codex app-server and claude processes killed, writer lock released — quit.ts: ⌘Q leaves no child process (claude, bash tool groups)
- [x] W6 Engine log file under the app data dir — quit.ts: optchat-gpui/engine.log written (rotated at 4 MB)

## X. Tests
- [x] X1 cargo test: view algorithms + property tests (tiling, cut, due order vs reference over 20k steps) — view::tests + view_props (fork cuts over 2000 pushes, 20001/20000-step references)
- [x] X2 cargo test: trim, keyterms, cap/globToRegExp, cache layout, live settings rewrite, rates pricing/calibration, metrics invariants + rebuild — trim, keyterms, tools, local, rates, metrics unit tests
- [x] X3 cargo test: store on a temp copy (load counts match sqlite), migration idempotent, schema compatible — store_migrate::a12 / a13 / x3_a14_b6 on a v3 fixture (load counts match sqlite) + store_file
- [x] X4 cargo test (models): Haiku turn end to end via claude CLI; zoom across fork; lifecycle (spawn/report/pause/resume/close/cancel/budget); Codex gpt-6-luna turn; mid-turn switch — model_turns + lifecycle (Haiku, GPT-6-Luna) passed
- [x] X5 Dictation engine test with a WAV file (Claude and Codex services, fallback) — stt live WAV tests passed
- [x] X6 tests/e2e.ts (Deno, Computer Use) walking every area, screenshots in shots/ — tests/e2e.ts: panels, features, chat, inputs, quit, smoke, flow, perf
