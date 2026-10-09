# PARITY.md — OptChat for Mac (GPUI) vs. the OptChat web app

Definition of done. Derived from `optchat-src` (readme.md, spec-tasks.md, spec-metrics.md, spec-dictation.md,
~/optchat-tasks-spec.md v2.6, lib/*, app/ui/*). An item is ticked only when it is implemented AND verified
(unit/integration test, e2e Computer Use run, or a screenshot I looked at). The verification is noted after `—`.

Legend: `[ ]` open · `[x]` done and verified · `[~]` done, verification partial (reason given)

## A. Storage and schema (lib/db.ts, lib/store.ts)
- [ ] A1 Open the project's own copy `data/optchat.db` only (path override via env for tests); never `~/.reframe/data/optchat.db`
- [ ] A2 All 17 tables kept compatible: message, context, request(+turn,+task), turn(+task), log, node, wire, kv, blob(data,mime,filename,size,meta), task, tlog(task,i), tnode(task), metric, metric_event, metric_total, rate(model), model — same columns, same JSON in `data`
- [ ] A3 Schema sync on open: create missing tables/indexes (fresh DB works), never drop
- [ ] A4 PRAGMA synchronous=FULL; log appends durable before returning
- [ ] A5 Writer lock (pid file `writer.lock` in the app dir); a second live process refuses to open the same chat
- [ ] A6 Serialized writes (no read-then-write race); upsert by id for message/context/request/turn/task/wire/kv/rate/model
- [ ] A7 tlog id `t:i`, tnode id `t:l:i`; first node build wins (insert-if-absent)
- [ ] A8 Blobs content-addressed (sha256 hex), stored once; image bytes externalized from message bodies (data "" + blob)
- [ ] A9 kv: `t<id>:view` {view, batching}, `t<id>:cview`, `t<id>:inbox`, `global:settings`, `root:<sha>` → root context id
- [ ] A10 metrics emission is one transaction (INSERT OR IGNORE event + ON CONFLICT totals for task and every ancestor, own + `*`)
- [ ] A11 `putMetricTotals` (rebuild) replaces the cache in one transaction
- [ ] A12 v3→v4 migration (idempotent, keyed on task 0): log/node → tlog/tnode of task 0, view/cview kv, v2 agents a1–a7 → children #1–#7, request/turn task column
- [ ] A13 metrics migration (idempotent, keyed on `metric` non-empty): seed rates, backfill ledger from requests/turns, fold totals; later starts upsert new metric definitions
- [ ] A14 `spent` field dropped from task rows on load
- [ ] A15 Loading 1.5 GB DB: logs + nodes + totals + rates load off the UI thread; first paint before load completes (skeleton)
- [ ] A16 In-memory store for tests (same trait)

## B. Memory machinery (lib/view.ts, optchat.ts Task)
- [ ] B1 Parts {t,l,i}; start/end/span; offsets over segments
- [ ] B2 `mostDuePair` due = (T − last)/2^l over global positions, ties → oldest, pairs within one tree, parent must be buildable/built
- [ ] B3 `mergeDown` to target; `tiles` validity check of a saved view
- [ ] B4 renderBlocks: blocks of 4 lines, `<chat>\n` prefix, `</chat>` closing block, mark on last whole block
- [ ] B5 Line format `start+2^l|text` and `start+2^l@t|text` for foreign trees; newlines flattened
- [ ] B6 View saved and loaded (never rebuilt), folded once from the log when missing/invalid ("view folded" log line)
- [ ] B7 Append: each message appends its line; past HIGH (viewBudget) one batch merges down to LOW (half); `batching` persisted; pending batch runs at load
- [ ] B8 Compactions' view (cview) 16–32 KB sawtooth, re-merged when the view merges or past 32 KB; derived when missing
- [ ] B9 `#want`: a pending batch stuck on an inherited pair builds that parent (any reader may build)
- [ ] B10 Long texts split at CAP (30,000 chars) over several messages (echo excepted); images are their own leaf messages with a marker text `[image name w×h]`
- [ ] B11 appendMany rollback on a failed store write (no phantom entries)
- [ ] B12 render(skip) leaves out own lines made only of messages delivered as new; settled(skip)
- [ ] B13 Unbuilt queue / mergeReady / wanted queues; rebuildQueues at load
- [ ] B14 Fork/continue copies the source's view and cview; segments = source.segments ++ {source, cut=T}
- [ ] B15 Property tests: tiling, no inherited merge past cut, Taelin push order equals reference `due` rule (view-tasks equivalents)

## C. Compactor (optchat.ts #build / #compact, prompts.ts)
- [ ] C1 Shared pool of 8 jobs across all tasks; settle-waiting tasks first; first 8 unbuilt per task; ready merges; wanted
- [ ] C2 Level 0: message ≤ 512 B (report lines 280 B) and not an image → its own text, no call
- [ ] C3 Merge: halves joined if ≤ 512 B, else a model call
- [ ] C4 Compaction input: builder's cview lines up to the node (stops at first unbuilt), blocks + cache mark, step text (compactMessageTask / compactMergeTask with 512-dash ruler), image attached for image leaves
- [ ] C5 Same SYSTEM prompt and same tool list as turns (tools never called: tool call → error result)
- [ ] C6 Too-long retry in the same conversation (`tooLong`), up to 5 tries; best (shortest) kept; cut to limit as last resort
- [ ] C7 Output cleanup (strip tags/id prefix); empty reply → failure
- [ ] C8 Output cap 16,000 tokens (CLAUDE_CODE_MAX_OUTPUT_TOKENS + proxy max_tokens clamp + thinking budget kept below)
- [ ] C9 150 s hard timeout; runaway abort (>4096 chars or long blank runs)
- [ ] C10 Failed node → `failed` map, retried at the next message, or after 5 s when a turn waits
- [ ] C11 Prefix-writing lock: a call whose marked prefix another call is writing waits for its first response (≤ 90 s)
- [ ] C12 Node record {l,i,text,size,requestId,tries,by}; `by` when a child builds a parent's node; node event to every task whose timeline holds the tree
- [ ] C13 Compactor model/effort from the builder's settings (default Haiku 5.5, xhigh)
- [ ] C14 Imported v2 tasks named once (titleTask JSON) with TITLE_VERSION

## D. Tasks engine (spec-tasks, optchat.ts Engine)
- [ ] D1 Task record fields (id, parent, title, mode, prev, status, reason, result, settings, budget, segments, createdAt, startedBy, endedAt, interrupted, unrun, imported)
- [ ] D2 Statuses pending/idle/active/paused/canceled/done; terminal = canceled|done
- [ ] D3 Root created on an empty DB; restart: active root → idle, active child → paused "interrupted by a restart" + interrupted
- [ ] D4 spawn(mode fork/continue/handoff/blind, title lowercased ≤80, instruction/images, prev, budget, settings, start)
- [ ] D5 spawn checks: parent not terminal, depth > 0, mode valid, title required, instruction or image required, prev a terminal child of the parent (continue only), unknown model refused, effort normalized
- [ ] D6 Child settings = parent's + partial override; depth = min(parent−1, given)
- [ ] D7 Budget defaults to what the parent has left (usd/minutes/tokens), refuses more; deadline ≤ parent's; pending keeps deadlineMinutes until start
- [ ] D8 readable(X) = up(X) ∪ subtree(X) (up stops at root/blind, follows prev for continue)
- [ ] D9 Per-turn header `<task>` text exactly as the TS (identity, memory readable, children (live + last 8 ended), depth, budgets left, deadline, instructions)
- [ ] D10 Turn input `[view pieces][header][new messages][images]`; cache text = last whole block
- [ ] D11 Inbound kinds user/parent/task/go; active vs passive delivery; mid-turn inject at the next tool boundary (logged with turnId)
- [ ] D12 Inbox persisted (kv), queue summary emitted ("N queued")
- [ ] D13 Scheduler: at most maxTurns turns at once (global, default 4); waiting tasks are `active`
- [ ] D14 Settle: a turn waits until the view's parts are built (status "settling"); a stop while settling cancels
- [ ] D15 endTurn: heartbeats flushed, turn row saved, computer released, report to parent (talk/error/status), status → idle/active, kick
- [ ] D16 Reports child→parent: `#C talk|user|error|status: …` logged whole, eager 280-byte line (3 tries then a cut line), delivered active/passive per §6; image leaves
- [ ] D17 userSend in a child → passive `#C user` report
- [ ] D18 pause (cascade, skips pending/terminal/paused; interrupts running turns; interrupted flag; `#C status` when by user)
- [ ] D19 resume (cascade; message; budget raise with parent limits; over-limit refused; RESUMED or `go` when unrun)
- [ ] D20 cancel / close (cascade with reasons "an ancestor was canceled/closed"; inbox flushed into the log; result ≤ 280 lowercased; root can't end)
- [ ] D21 Pending: spawn start:false logs instruction with unrun; append (↑) logs at once and sets unrun; start (→ empty) flips pending → idle with `go`; kick ignores pending; pause/resume refuse pending
- [ ] D22 tell(run, settings): run:false → append; paused + text → resume with it; empty + run → start; settings-only tell
- [ ] D23 Continue after a stopped/failed turn (empty →) sends the CONTINUE text
- [ ] D24 stop(x): cancels the running turn ("stopped (by the user)"), `#C status: turn stopped by you`
- [ ] D25 stopTool(callId): stops one tool, the turn goes on with "(stopped by the user)"
- [ ] D26 setSettings: merge, model check, effort normalized, compactor check, depth clamp, unchanged no-op; returns now / next turn / saved
- [ ] D27 Mid-turn model switch: live settings reach the next model call on the same backend (Claude via proxy body rewrite, Codex via thread-id map); cross-backend waits for next turn; request row records the model that ran
- [ ] D28 Budgets: usd/minutes/tokens on subtree totals, deadline; exceeded → pause subtree (skipping pending) + active `#C error` naming the metric (root: error event); overLimits at turn start; deadline checker every 10 s
- [ ] D29 Computer lock: one driver at a time, from the first computer call until the turn ends; waiters
- [ ] D30 Hub-swap fence equivalent: a disposed engine never writes (fenced store); drain catch fails the turn visibly and idles the task
- [ ] D31 `describe` / task_list text identical to the TS
- [ ] D32 Turn record (startedBy, backend, model, effort, status waiting/running/done/failed/cancelled, userIndexes, requestIds, usage, cost, error, timestamps)
- [ ] D33 Request record per model call (purpose, backend, provider, model/effort live, contextId, outputId, status, stop, usage, cost, wireId, timing, task)
- [ ] D34 Canonical contexts: root context per system prompt (kv `root:<sha>`), parent chain, marks; tool results and injected messages appended
- [ ] D35 contextMessages(contextId) for the debugger (parent chain, envelopes with children)
- [ ] D36 Everything the agent says/does logged as it happens (talk, tool `name {json}` with meta callId/name, echo with isError, echo images as leaves unless already in the log); thinking never logged

## E. Agents (lib/agents.ts)
- [ ] E1 Claude Code per turn: `claude` with stream-json in/out, `--include-partial-messages`, `--tools ""`, `--setting-sources=`, `--strict-mcp-config`, `--no-session-persistence`, `--mcp-config` (loopback http), `--allowedTools mcp__o__*`, `--model`, `--effort`, `--thinking-display summarized`
- [ ] E2 Claude env: clearEnv; HOME, PATH (incl ~/.local/bin, /usr/local/bin, /opt/homebrew/bin), CLAUDE_CONFIG_DIR app claude-home, CLAUDE_SECURESTORAGE_CONFIG_DIR="", ANTHROPIC_BASE_URL loopback, the disable flags, MCP timeouts, CLAUDE_CODE_MAX_OUTPUT_TOKENS for compactions
- [ ] E3 System prompt via `initialize` control request; user message with view pieces; inject with `priority: next`; interrupt control request + kill after 3 s; followUp for compactions
- [ ] E4 Steps from stream events: message_start usage (incl. 1h cache writes), deltas (text/thinking), message_delta stop/usage, assistant blocks, result → cost report (modelUsage), errors
- [ ] E5 Tool call waits for its step to finish before running (ordering of onStepEnd before runTool)
- [ ] E6 Codex: one shared `codex app-server` (JSON-RPC stdio), args (model_catalog_json, notify=[], web_search disabled, optchat provider at loopback proxy, `--disable` list, user MCP servers disabled), initialize with experimentalApi
- [ ] E7 Codex catalog copy with code mode off (tool_mode null, apply_patch null, …)
- [ ] E8 Codex turn: thread/start (ephemeral, baseInstructions, dynamicTools, approval never, danger-full-access, experimentalRawEvents), turn/start with input text/images, turn/steer, turn/interrupt, rawResponseItem/completed → steps, rawResponse/completed usage → step end + wire, turn/completed
- [ ] E9 Codex server requests routed by thread id; item/tool/call → runTool; other requests declined
- [ ] E10 Adapters: Anthropic blocks ↔ canonical (text, thinking+signature, redacted, tool_use with mcp__o__ prefix, tool_result, image, document); Responses items ↔ canonical (message, reasoning encrypted, function_call, custom_tool_call, outputs)
- [ ] E11 Subscription logins only (no API keys anywhere); PATH set by the app

## F. Loopback server (lib/local.ts)
- [ ] F1 127.0.0.1 random port, secret path; 404 otherwise
- [ ] F2 MCP endpoint per run: initialize, tools/list, tools/call (callId from `_meta.claudecode/toolUseId`), ping, notifications 202, GET 405, DELETE 200
- [ ] F3 Anthropic proxy: records wire body, cache layout (drop all cache_control; mark view block(s) ≤2, system, last block; 1h ttl; ≤4), live settings rewrite (model + output_config.effort), max_tokens clamp + thinking budget for compactions; streams the response back
- [ ] F4 OpenAI proxy for Codex: live settings by thread-id/session-id header (model + reasoning.effort), prompt_cache_key + session-id/thread-id pinned to `optchat-<hash(lead)>`, x-codex-window-id dropped, upstream chatgpt.com/backend-api/codex for ChatGPT logins
- [ ] F5 Unit tests for applyCacheLayout / applyLiveSettings / openai rewrite with a stub upstream

## G. Tools (lib/tools.ts, task-tools.ts, computer.ts, mcp-client.ts)
- [ ] G1 bash: bash -lc in its own process group, timeout (default 120 s, max 600 s), kill group on timeout/stop (TERM then KILL), partial output, head/tail keep, exit code text
- [ ] G2 read_file (numbered lines, offset/limit, "[N more lines]"), write_file (mkdir -p), edit_file (unique / replace_all), glob (globToRegExp, ** recursion, 500 cap, skip .git/node_modules), grep (grep -rnE -I), web_fetch (30 s, HTML → text)
- [ ] G3 cap(): 30,000 chars head 60% / tail with "[... N characters cut ...]"
- [ ] G4 zoom(id, n, task?) and date(id, task?) with readable checks and exact error texts; zoom n=1 returns image parts
- [ ] G5 task_spawn / task_tell / task_pause / task_resume / task_cancel / task_close / task_list with the exact schemas and descriptions (byte-identical tool list across tasks)
- [ ] G6 images arg: {id, task?} from readable memory or {path} files (png/jpeg/gif/webp ≤ 5 MB)
- [ ] G7 Tool groups per settings: core / computer (only models marked capable, cua available) / mcp; depth 0 removes task tools
- [ ] G8 Computer use: spawn ChatGPT.app's cua_repl MCP over stdio with the env; `computer` (js) and `computer_reset` tools; turn_ended hook; elicitation → approvals
- [ ] G9 reframe MCP over streamable HTTP (JSON or SSE), tools re-exported as `reframe_*` (implemented; disabled in my test runs by env so tests never touch it)
- [ ] G10 Tool call metrics (time.tool, calls.tool) and running-tool registry for reconnect/stop

## H. Metrics, rates, models (spec-metrics)
- [ ] H1 METRICS definitions (12 incl. audio.ms); kinds chat/compact/dictation
- [ ] H2 emit(task, source, items, meta): idempotent `<source>:<metric>` ids, own + subtree for chain, per kind and `*`, change callback → task event + budget check
- [ ] H3 charge per model call: usd, tokens.in/cache_read/cache_write/out, time.model, calls.model, calls.unpriced; meta model/effort/provider/backend/request/turn/rate/ratePer1M/cacheWrite1h (+node, tries for compactions)
- [ ] H4 time.active heartbeats every 10 s + flush at end; source `turn:<id>#n`
- [ ] H5 rebuild folds the ledger
- [ ] H6 Rates: inForce(model, at), history, add, price (long context, 1h writes), SEED_TABLE, syncPublished at start (skip manual), calibrate from Claude cost reports (2% threshold, 3 agreeing reports, never over manual), reprice unpriced calls once
- [ ] H7 Model catalog: Claude list + Codex models_cache.json (visibility, efforts minus ultra, default effort, computer use not luna); `model` table refresh at start, hidden flag
- [ ] H8 spendByModel from the ledger (cached 30 s)
- [ ] H9 metrics.breakdown by kind / model / source (own vs subtree)

## I. Dictation (spec-dictation, lib/stt.ts, keyterms.ts)
- [ ] I1 Native microphone capture (CoreAudio via cpal), mono, resampled to pcm16 16 kHz, 100 ms chunks with level; recording starts at the press; buffered until the service is ready
- [ ] I2 Claude voice_stream client (keychain token, headers, keyterms header, KeepAlive every 8 s, CloseStream, Transcript* messages)
- [ ] I3 ChatGPT dictation client (codex auth.json, subprotocols, session.start config, 24 kHz resample, audio.append base64, 700 ms silence before session.close, transcript.segment/final)
- [ ] I4 Session: service order from global setting with the other as fallback; failure before ready → next service with all audio; drop after text → keep text, one reconnect re-sending last 1.5 s, overlap merge (≤ 12 words)
- [ ] I5 A new dictation finishes the previous; done event with text/service/audioMs
- [ ] I6 keyterms(): sources (title ×3, related titles ×2, last 40 log entries weighted, tool files only, echoes never), candidates by shape, ranking, hints ≤ 20 (base terms first), words ≤ 60, spoken(), rewriter()
- [ ] I7 `audio.ms` metric (kind dictation) with meta service/keyterms/chars
- [ ] I8 UI: mic button states idle / connecting (spinning ring) / listening (red, 4 level bars, pulse, red composer outline) / finishing (bars settle); no labels
- [ ] I9 Gesture rule: press while idle starts; release > 300 ms later stops (push-to-talk), earlier = tap (keeps running); press while dictating stops
- [ ] I10 M key outside text fields (tap/hold); inside the composer M types "m"; Esc stops and keeps text
- [ ] I11 Text lands at the cursor/selection (before + text + after with spacing), composer read-only while dictating, caret after the text at the end
- [ ] I12 Enter while dictating: stop, wait for final text, then → ; ⌘Enter: stop then ↑
- [ ] I13 Errors in the composer error line for 6 s ("microphone access is blocked", "dictation is unavailable: log in…")
- [ ] I14 Engine test with a WAV file through the session (both services), plus fallback test

## J. App shell and layout (app.tsx)
- [ ] J1 Native macOS window, title "<task name> · #id · OptChat"; restores size; min size
- [ ] J2 Activity rail: logo (opens main chat), Tasks (badge with active count), Memory (dot while compacting), Context, Log, Models, spacer, Settings, sidebar toggle
- [ ] J3 Rail click on the active view hides the sidebar; ⌘B toggles it
- [ ] J4 Resizable sidebar (220–760 px), width remembered; sidebar head with view title and per-view actions; hide button
- [ ] J5 Editor tabs of open tasks: status dot (root: chat icon), title, spinner while running, close button, middle-click closes; closing the last tab leaves the main chat; active tab scrolled into view
- [ ] J6 Tab actions at the right: pause (not paused/pending/terminal), resume (paused), close as done (non-root, non-terminal), cancel (non-root), new task (non-terminal)
- [ ] J7 Task bar: breadcrumb (ancestors clickable, "main chat"), continues #prev link, result (→ …) or status: reason
- [ ] J8 Task bar ask mode: label, input (autofocus, max 280 for close), "and its subtree" hint, confirm button (danger for cancel), Esc; Enter confirms, Esc cancels; required result for close
- [ ] J9 Remembered per-user prefs: view, sidebar open/width, task scope, show ended, settings tab, theme (local prefs file)
- [ ] J10 Open tasks persisted across launches (better than web: reopen last tabs)
- [ ] J11 Toast notices (controlDone ok 2.6 s / error 6 s)
- [ ] J12 Approval dialog (computer use): message, task, tool params, Deny / Allow once / Allow this session / Always allow; queue of approvals
- [ ] J13 Lightbox for images (click to enlarge, click/Esc to close)
- [ ] J14 Skeletons until a task's data is loaded; never the empty state for the root
- [ ] J15 Phone layout N/A on Mac → narrow-window layout: the sidebar floats over the editor below ~760 px

## K. Tasks view (sidebar) (tasks.tsx TasksView)
- [ ] K1 Scope "here": ancestors path rows above, the current task, its subtree; "all": the whole tree from #0
- [ ] K2 Row: chevron (collapse), status dot (idle grey, active pulsing accent, paused amber, canceled red, done green, pending hollow ring), title, line 2 `#id · $cost · active time · mode · status word (working/waiting/status)`, spinner when running; current highlighted; ended dimmed; tooltip with title/mode/status/reason/result
- [ ] K3 Toggle all/here, show/hide finished, new task (+) buttons in the sidebar head
- [ ] K4 "no subtasks yet" hint
- [ ] K5 Click opens the task in a tab (instant, same engine)

## L. Feed (chat.tsx)
- [ ] L1 Virtualized variable-height list (gpui `list`) of turn groups; smooth scroll of 2000+ messages
- [ ] L2 Groups by turnId; "Load earlier messages (N more)" (or infinite load) for older history
- [ ] L3 User bubble (right, pre-wrap) with #i and time; note kind same style
- [ ] L4 Parent bubble "↳ from #P / from you, as #P / from the main chat" (clickable), markdown body, image leaves
- [ ] L5 Report card `#C replied | you wrote to it | error | status`, child title, #i · time, open ↗; long bodies clipped at 320 chars, click to expand; colored by kind; image reports
- [ ] L6 v2 work messages `[a1] …` rendered as report blocks linking to #1..#7
- [ ] L7 Talk: markdown (headings, lists nested/loose, task lists, tables, code blocks with language bar + copy, inline code, links open in browser, blockquotes, emphasis, strikethrough)
- [ ] L8 Live streaming text with a cursor; thinking shown live (collapsible "thinking…" / "thought" with last line preview), never logged
- [ ] L9 "waiting for the model…" and "waiting for summaries: N left (k in progress) · retrying · elapsed" states
- [ ] L10 Tool card: chevron, name, arg preview per tool (bash command, computer title, zoom id+n@t, task_* previews…), state running (elapsed) / done / error, per-tool stop button, #i–#j, task id chips for task_* tools, expandable input/output (+ images), "shortened · show all N chars"
- [ ] L11 Echo images under their tool card; lone echoes as cards
- [ ] L12 Image messages (blob from DB) with caption, click to lightbox
- [ ] L13 Turn foot chips: model · effort (with a → b when switched), N steps ▾ (step table: #, model if switched, stop, in, cached, write, out, cost, time; click → context), in (cached %), out, cost (amber unpriced), duration, live, stopped, context ↗
- [ ] L14 Turn error line "⚠ …"
- [ ] L15 "not run yet" divider at rec.unrun
- [ ] L16 Empty states: child "This task has no messages yet. Its first turn is starting."; root intro (only when the root really has nothing)
- [ ] L17 Follow mode: autoscroll while at the bottom; stays put when scrolled up; jump-to-bottom affordance
- [ ] L18 Cut entries (slim copy) with on-demand full text — in-process the full text is available; long texts collapsed with "show all"
- [ ] L19 Endbar for terminal tasks: dot + "This task is done: result / canceled (reason). Its memory stays readable." + "Continue as a new task" (opens dialog preset continue/prev)

## M. Composer (app.tsx Composer)
- [ ] M1 Multi-line input (IME, selection, undo), auto-grow to 260 px, draft per task
- [ ] M2 Placeholder `Message #N…` / `Message OptChat…`
- [ ] M3 Buttons: attach image · spacer · Stop (running) · mic · ↑ (append, outlined; hidden while running) · → (run, primary accent)
- [ ] M4 → enabled with text/image, or runnable (unrun, pending, paused, or last turn stopped/failed); ↑ enabled only with content
- [ ] M5 Keys: Enter = →, Shift+Enter newline, ⌘Enter = ↑ (→ while a turn runs); IME composition respected
- [ ] M6 Images: attach picker (NSOpenPanel), paste from clipboard, drag & drop files; downscale to 1568 px longest side; re-encode jpeg/png; thumbnails with remove ×; max 20
- [ ] M7 Error line above the composer (last engine error / dictation error)
- [ ] M8 Focus the composer on task switch
- [ ] M9 Disabled until the task is loaded

## N. Status bar (tasks.tsx StatusBar)
- [ ] N1 Left: connection-equivalent status dot + status text (running / settling / waiting for a slot / task status), `#id mode` (child), `summarizing N` with spinner, `N working`, `N queued`
- [ ] N2 Right: cost (subtree) with budget bar (accent/amber/red) and `/ $budget`, active time `/ budget`, deadline minutes (red when passed), tokens, msgs (+inherited tooltip), depth, model select (grouped Claude Code / Codex), effort select
- [ ] N3 Running tint (indigo) and paused tint
- [ ] N4 Metrics popover: one row per metric own · subtree, chat · memory split rows, budget fields ($, agent-min, tokens, deadline min), Set budget / Clear, Enter/Esc
- [ ] N5 Model/effort selects hidden on terminal tasks; work on pending tasks

## O. New-task dialog (tasks.tsx)
- [ ] O1 Title "New task under the main chat / #N title", explainer line
- [ ] O2 Mode cards handoff / fork / continue (disabled without terminal siblings) / blind with descriptions
- [ ] O3 Continue: which task select (terminal siblings)
- [ ] O4 Title (≤ 80), instruction (multi-line, image paste), Attach images, thumbnails
- [ ] O5 Model select (inherited marker), $ budget, minutes
- [ ] O6 Footer Cancel · Add (pending) · Start (primary, ⌘Enter); disabled until valid
- [ ] O7 After spawn the new task opens in a tab

## P. Memory view (panels.tsx MemoryPanel)
- [ ] P1 Summary card: messages (own + inherited), tree nodes built, view lines (inherited count) and bytes, batches at HIGH → LOW, compactions' view lines/bytes, fill bar (amber while batching) with half marker, "batch pending" note, level legend (count × 2^l msg, level colors)
- [ ] P2 Compaction jobs: running nodes in this timeline (spinner, click reveals), failed (retrying), count in other tasks
- [ ] P3 Search box (Enter) over readable trees' messages; hits (latest first, kind tag, text) → reveal the path
- [ ] P4 Toggle the view / compactions' view; "raw" shows the rendered view text, blocks, mark index, marked chars and the header
- [ ] P5 Tree rows: chevron, level color bar, `start+n`, `@t` badge for foreign trees, text; spinner when being built; retry badge when failed; unbuilt style; open → two halves (zoom) down to the message (full text, kind, date, image)
- [ ] P6 Rows refresh as nodes get built

## Q. Context debugger (panels.tsx ContextPanel)
- [ ] Q1 Shows the selected request (follows latest chat request unless pinned; "follow latest" button)
- [ ] Q2 Header chips: model · effort, backend, status · stop, cost
- [ ] Q3 Usage bar (cache read green, write amber, uncached accent) and legend with reasoning tokens
- [ ] Q4 KV: input tokens + cached %, cost split, first token, total time, started, canonical context size
- [ ] Q5 Context sent: canonical messages oldest first with context separators, roles, kinds, previews, sizes, raw marker, expandable full text, images, envelopes with children, cache breakpoint markers
- [ ] Q6 Output section; wire request (show/hide, pretty JSON)
- [ ] Q7 Running call message ("still running")

## R. Log view (panels.tsx RequestsPanel + Breakdown)
- [ ] R1 Scope this task / with its subtree
- [ ] R2 Breakdown tables by kind (memory/chat labels), by model, by source (turns, compactions, tools, children) with cost bars, tokens, time; events count
- [ ] R3 Totals card: cost (ledger), turns/compactions loaded, input tokens + cached %, output tokens, note
- [ ] R4 Filter all / chat / compact; table when · task (subtree) · model · in · cache % · out · cost; click selects → Context view
- [ ] R5 Server log (in Settings > Global) — engine log lines, last 300

## S. Models view (models.tsx)
- [ ] S1 Providers grouped (Anthropic · Claude Code, OpenAI · Codex); count of runnable models; refresh catalog button; show/hide hidden
- [ ] S2 Model card: label, id, spend (ledger, tooltip calls/tokens), tags (not offered now, hidden, computer use, efforts), rate in force (in / cache read / cache write / write 1h / out), long-context line, source chip + since date + note, or "No price yet…"
- [ ] S3 Edit rate (fields, Anthropic ratios auto-filled from base input, note) → new manual row; history table; hide/show from pickers (pickers update)

## T. Settings (panels.tsx SettingsPanel)
- [ ] T1 Two tabs: Global and the task's ("Main chat" / "Task #N"); last tab remembered
- [ ] T2 Global: turns at once (1,2,3,4,6,8,12,16), dictation service Claude/Codex, server log, build id
- [ ] T3 Global (GPUI addition): theme picker (+ follow system appearance), font size
- [ ] T4 Task: instructions (saved on blur), depth select (0..parent−1, "(no task tools)"), compactor model + effort, tools toggles (core, computer with availability text, reframe MCP), computer approvals ask/always, tool list by group, working directory, view budget (≥ 2000)

## U. Keyboard, commands, polish (GPUI additions beyond the web app)
- [ ] U1 Command palette (⌘K / ⌘⇧P): every command (open task by title, new task, pause/resume/close/cancel, switch view, theme, toggle sidebar, focus composer, stop, models, settings)
- [ ] U2 App menus (OptChat, File, Edit, View, Task, Window, Help) with shortcuts
- [ ] U3 Shortcuts: ⌘B sidebar, ⌘1..⌘6 rail views, ⌘N new task, ⌘W close tab, ⌘⇧[ / ⌘⇧] prev/next tab, ⌘. stop, ⌘L focus composer, ⌘, settings, ⌘T task switcher, Esc closes dialogs/popovers
- [ ] U4 Context menus: task rows / tabs (open, pause, resume, close, cancel, new subtask, copy id), messages (copy text, copy markdown), tool cards (copy input/output)
- [ ] U5 Smooth animations: spinners, pulsing dots, mic pulse, popover/dialog fade, chevron rotate
- [ ] U6 Focus handling: dialogs trap focus, Esc returns to composer, Tab order in dialogs
- [ ] U7 Tooltips on icon buttons

## V. Themes
- [ ] V1 Theme system with live switching (no restart) applied to every view
- [ ] V2 Zed theme JSON format loader (theme family files with `themes[].appearance/style`), license noted (Zed themes: GPL/Apache per repo; noted in THEMES.md)
- [ ] V3 Themes: OptChat Dark (current web palette), OptChat Light, Solarized Light, Solarized Dark, Tomorrow Night, Tomorrow Night Blue, One Dark, One Light, Ayu Dark/Light/Mirage, Gruvbox Dark/Light (Zed built-ins)
- [ ] V4 Follow system appearance by default (light/dark pair), live on appearance change
- [ ] V5 Screenshot of every theme

## W. Packaging, performance, process
- [ ] W1 .app bundle com.reframe.optchat.gpui, Info.plist (NSMicrophoneUsageDescription), icon, ad hoc codesign; launchable from Finder
- [ ] W2 PATH set by the app (~/.local/bin, /usr/local/bin, /opt/homebrew/bin + system)
- [ ] W3 No UI-thread stalls: DB/IO/network on the engine runtime; UI receives coalesced events
- [ ] W4 Measured: launch time to first frame and to loaded chat, frame times while scrolling a 2000-message chat, memory; release build
- [ ] W5 Graceful shutdown: running turns cancelled, codex app-server and claude processes killed, writer lock released
- [ ] W6 Engine log file under the app data dir

## X. Tests
- [ ] X1 cargo test: view algorithms + property tests (tiling, cut, due order vs reference over 20k steps)
- [ ] X2 cargo test: trim, keyterms, cap/globToRegExp, cache layout, live settings rewrite, rates pricing/calibration, metrics invariants + rebuild
- [ ] X3 cargo test: store on a temp copy (load counts match sqlite), migration idempotent, schema compatible
- [ ] X4 cargo test (models): Haiku turn end to end via claude CLI; zoom across fork; lifecycle (spawn/report/pause/resume/close/cancel/budget); Codex gpt-6-luna turn; mid-turn switch
- [ ] X5 Dictation engine test with a WAV file (Claude and Codex services, fallback)
- [ ] X6 tests/e2e.ts (Deno, Computer Use) walking every area, screenshots in shots/
