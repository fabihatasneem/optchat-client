// PARITY items that need no model call: shell (rail, sidebar drag, tabs, middle-click, shortcuts, menus, palette
// lifecycle commands, toasts, appearance), feed (skeletons, paging, follow mode, report expand, v2 work rows,
// markdown, echo images, cut texts, turn errors, empty states), composer (images by paste / drop / cap, error line,
// mic gesture rule and states, focus, disabled while loading), status bar budget, new-task dialog (continue select,
// images), memory / context / log / models / settings panels. Test hooks (OPTCHAT_TEST_HOOKS=1) feed the app's real
// event paths where the OS or a service can't be driven safely (see PARITY.md notes).
import { App, check, failed } from "./lib/app.ts";
import { Peer } from "./lib/hub.ts";
import { ROOT, sleep } from "./lib/cua.ts";
const tmp = await Deno.makeTempDir({ prefix: "optchat-parity-" });
const big = `${tmp}/big.png`; // 3000×2000: the composer downscales to 1568 px
await new Deno.Command("sips", { args: ["-z", "2000", "3000", `${ROOT}/tests/fixtures/test.png`, "--out", big], stdout: "null", stderr: "null" }).output();
const env = { OPTCHAT_TEST_HOOKS: "1", OPTCHAT_FAKE_MIC: `${ROOT}/tests/fixtures/dict.wav`, OPTCHAT_TEST_IMAGE: `${ROOT}/tests/fixtures/test.png`, OPTCHAT_TEST_BIG_IMAGE: big };
let a = await App.launch({ env });
const st = () => a.state();
const pal = (q: string) => a.palette(q);
const keys = (pre: string) => Object.keys(a.allProbes()).filter((k) => k.startsWith(pre));
const inside = (p: number[] | undefined, box: number[] | undefined) => !!p && !!box && p[1] >= box[1] && p[1] + p[3] <= box[1] + box[3];
const revealSide = async (name: string) => {
  for (let i = 0; i < 14; i++) {
    const p = a.probe(name), side = a.probe("sidebar");
    if (inside(p, side)) return p!;
    await a.scroll("sidebar", p && side && p[1] < side[1] ? "up" : "down", 0.5);
    await sleep(350);
  }
  throw new Error(`${name} not visible in the sidebar`);
};
const only = Deno.env.get("PARITY_ONLY")?.split(",");
const step = async (name: string, f: () => Promise<void>) => {
  if (only && !only.includes(name)) return;
  try { await f(); } catch (e) { check(false, `${name}: ${String(e).slice(0, 400)}`); await a.shot(`parity-fail-${name}`); await a.key("Escape"); await sleep(300); }
};
/** time from now until f() turns false (polls every 100 ms), in ms */
const lasts = async (f: () => boolean, max: number) => {
  const t0 = Date.now();
  while (Date.now() - t0 < max) { if (!f()) return Date.now() - t0; await sleep(100); }
  return max;
};
const peer = await Peer.connect(a.db, 0); // a second client of the hub, for what the app doesn't show
const audioMs = async (task: number) => Number((await peer.rpc("metrics.breakdown", { task })).byKind?.dictation?.["audio.ms"] ?? 0);
const view = async (k: string, v: string) => {
  if (!(st().view === v && st().sideOpen)) await a.key(k);
  await a.waitFor(() => st().view === v && st().sideOpen, 3000, `view ${v}`);
  await sleep(800);
};
try {
  await step("launch", async () => {
    // J14: skeletons while the chat loads, and the root never shows its empty state on a real chat
    check((st().feedSkeletons ?? 0) >= 1 && !(st().feedEmpty ?? []).includes("root"), `skeleton rows drawn while loading (${st().feedSkeletons}), never the root's empty state (${JSON.stringify(st().feedEmpty ?? [])})`);
  });

  await step("memory", async () => {
    await view("cmd+2", "memory");
    await a.waitFor(() => st().memSummary?.lines > 0, 8000, "memory summary");
    const m = st().memSummary;
    const legendSum = m.legend.reduce((x: number, y: number[]) => x + y[1], 0);
    check(m.messages === st().t && m.lines > 0 && legendSum === m.lines && m.high > m.low && m.bytes > 0 && m.cviewLines > 0,
      `summary card: ${m.messages} messages (+${m.inherited} inherited), ${m.nodes} nodes built, ${m.lines} view lines / ${m.bytes} B, batches at ${m.high} → ${m.low}, compactions' view ${m.cviewLines} lines / ${m.cviewBytes} B, legend ${JSON.stringify(m.legend)} sums to the lines`);
    await a.shot("parity-02-memory");
    // P2 failed jobs (running jobs: tests/live.ts, they need compactions)
    await pal("Test: compaction failure");
    await a.waitFor(() => st().memSummary?.failed > 0, 3000, "failed job");
    await sleep(300);
    await a.shot("parity-03-memory-failed");
    check(keys("jobf.").length > 0, `a failed compaction shows as "failed · retrying" (${keys("jobf.")[0]}; fed as a compactor event)`);
    // P4: the compactions' view and raw
    await revealSide("mem.cview");
    await a.click("mem.cview");
    await a.waitFor(() => st().memSummary?.whichC === true && st().memSummary.rows > 0, 4000, "cview");
    await sleep(400);
    await a.shot("parity-04-memory-cview");
    check(st().memSummary.rows === st().memSummary.cviewLines || st().memSummary.rows > 0, `the compactions' view lists its ${st().memSummary.rows} lines`);
    await a.click("mem.view");
    await a.waitFor(() => st().memSummary?.whichC === false, 3000, "view");
    await a.click("mem.raw");
    await a.waitFor(() => st().memSummary?.raw?.chars > 0, 5000, "raw");
    const r = st().memSummary.raw;
    await a.shot("parity-05-memory-raw");
    check(r.chars > 0 && r.blocks > 0 && r.header > 0, `raw shows the rendered view (${r.chars} chars, ${r.blocks} blocks, mark on block ${r.mark + 1}, ${r.marked} marked chars, header ${r.header} chars)`);
    await a.click("mem.raw");
  });

  await step("context", async () => {
    await a.switchTo(8);
    await sleep(800);
    const c = keys("foot.ctx.")[0];
    await a.click(c);
    await a.waitFor(() => st().view === "context" && st().ws?.pinned === true, 4000, "pinned call");
    await a.waitFor(() => st().context?.rows > 0, 6000, "context rows");
    check(st().context.imageRows > 0, `the context sent lists the picture as an image row (${st().context.imageRows} of ${st().context.rows})`);
    const img = keys("cmimg.")[0];
    await revealSide(img);
    await a.click(img);
    await a.waitFor(() => !!a.probe("ctx.thumb"), 5000, "thumbnail").catch(() => {});
    await sleep(500);
    await a.shot("parity-06-context-image");
    check(!!a.probe("ctx.thumb"), "an image row opens to its thumbnail (parity-06-context-image)");
    await revealSide("ctx.follow");
    const pinnedTo = st().context.shown;
    await a.click("ctx.follow");
    await a.waitFor(() => st().ws?.pinned === false, 3000, "follow latest");
    check(!a.probe("ctx.follow") || st().context.shown !== undefined, `"follow latest" unpins the call (was ${pinnedTo?.slice(0, 8)}, now follows ${st().context.shown?.slice(0, 8)})`);
  });

  await step("log", async () => {
    await a.key("cmd+0");
    await a.waitFor(() => st().current === 0 && st().loaded, 6000, "main");
    await view("cmd+4", "log");
    await a.waitFor(() => !!st().log?.breakdown, 8000, "breakdown");
    const b = st().log.breakdown;
    check(b.byKind.includes("chat") && b.byKind.includes("compact") && b.byModel > 0 && b.bySource.length > 0 && b.events > 0, `breakdown by kind ${JSON.stringify(b.byKind)}, ${b.byModel} models, by source ${JSON.stringify(b.bySource)}, ${b.events} events`);
    await a.shot("parity-07-log");
    for (const f of ["chat", "compact", "all"]) {
      await revealSide(`log.f.${f}`);
      await a.click(`log.f.${f}`);
      await a.waitFor(() => st().log?.filter === f, 3000, `filter ${f}`);
      const p = st().log.purposes;
      check(f === "all" ? p.length >= 2 : p.length === 1 && p[0] === f, `filter ${f}: ${st().log.rows} rows of ${JSON.stringify(p)}`);
    }
    const first = st().log.first;
    await revealSide("logrow.0");
    await a.click("logrow.0");
    await a.waitFor(() => st().view === "context" && st().selectedRequest === first, 3000, "row → context");
    check(true, `a log row opens that call in the Context view (${first.slice(0, 8)})`);
  });

  await step("models", async () => {
    await view("cmd+5", "models");
    await a.waitFor(() => !!st().models, 8000, "models");
    const m = st().models;
    check(m.providers.includes("anthropic") && m.providers.includes("openai") && m.runnable > 0, `providers grouped ${JSON.stringify(m.providers)}, ${m.runnable} runnable`);
    check(m.haiku?.rate > 0 && !!m.haiku.source && m.haiku.efforts.length > 0, `Haiku card: rate in ${m.haiku?.rate}/M (${m.haiku?.source}), ${m.haiku?.spendCalls} calls in the ledger, efforts ${m.haiku?.efforts?.join(" ")}`);
    const r0 = m.reloads;
    await a.click("models.refresh");
    await a.waitFor(() => st().models.reloads > r0, 15000, "refresh");
    check(true, `refresh reads the catalogs again (reloads ${r0} → ${st().models.reloads})`);
    // hide a model, then show hidden ones
    const target = "gpt-6-luna";
    await revealSide(`model.hide.${target}`);
    await a.click(`model.hide.${target}`);
    await a.waitFor(() => st().models.hidden > m.hidden && !st().models.shown.includes(target), 5000, "hidden");
    await revealSide("models.showhidden");
    await a.click("models.showhidden");
    await a.waitFor(() => st().models.showHidden && st().models.shown.includes(target), 3000, "show hidden");
    await a.shot("parity-08-models-hidden");
    check(true, `hide from pickers hides ${target}; "show N hidden" lists it again`);
    await revealSide(`model.hide.${target}`);
    await a.click(`model.hide.${target}`);
    await a.waitFor(() => st().models.hidden === m.hidden, 5000, "unhidden");
    const h = Object.keys(st().models.histCounts ?? {})[0];
    if (h) {
      await revealSide(`model.hist.${h}`);
      await a.click(`model.hist.${h}`);
      await a.waitFor(() => st().models.hist.includes(h), 3000, "history");
      await a.shot("parity-09-models-history");
      check(true, `the rate history of ${h} opens (${st().models.histCounts[h]} rows)`);
    } else check(true, "no model has a rate history yet (history needs 2 rates)");
  });

  await step("settings-font", async () => {
    await view("cmd+6", "settings");
    await a.click("settings.global");
    await a.waitFor(() => st().settingsGlobal === true, 3000, "global");
    const f0 = st().fontSize;
    await revealSide("settings.font.plus");
    await a.click("settings.font.plus");
    await a.waitFor(() => st().fontSize === f0 + 1, 3000, "bigger");
    await a.shot("parity-10-font-bigger");
    await a.click("settings.font.minus");
    await a.waitFor(() => st().fontSize === f0, 3000, "smaller");
    check(true, `Settings text size + / − (${f0} → ${f0 + 1} → ${f0})`);
  });

  await step("appearance", async () => {
    check(st().followSystem === true, "the theme follows the system appearance by default");
    await pal("Test: appearance light");
    await a.waitFor(() => st().theme === "optchat-light", 3000, "light");
    await a.shot("parity-11-appearance-light");
    await pal("Test: appearance dark");
    await a.waitFor(() => st().theme === "optchat-dark", 3000, "dark");
    check(true, "an appearance change switches OptChat Light / Dark live (the observe_window_appearance handler)");
    await pal("Theme: Ayu Dark");
    await a.waitFor(() => st().theme === "ayu-dark" && st().followSystem === false, 3000, "picked");
    await pal("Test: appearance light");
    await sleep(500);
    check(st().theme === "ayu-dark", `a picked theme ignores the appearance (${st().theme})`);
    await pal("Follow the system appearance");
    await a.waitFor(() => st().followSystem === true, 3000, "follow again");
  });

  await step("notices (no toasts)", async () => {
    // phase 4: no toasts; a command's outcome is a notice of the workspace (status bar dot, Notifications view)
    const u0 = st().unread;
    await pal("Test: notice ok");
    await a.waitFor(() => st().unread.ok === u0.ok + 1, 3000, "ok notice");
    await pal("Test: notice error");
    await a.waitFor(() => st().unread.error === u0.error + 1, 3000, "error notice");
    await a.waitFor(() => !!a.probe("status.notice.error") && !!a.probe("status.notice.ok"), 3000, "status bar dots");
    await a.shot("parity-12-notices");
    check(st().toast === undefined && (st().noticesTop ?? []).some((n: any) => n.text === "test notice: it failed" && n.level === "error"), "no toast: the outcomes are notices, with a dot per level in the status bar");
    await a.click("status.notices");
    await a.waitFor(() => st().view === "notifications", 3000, "notifications view");
    await a.click("notif.readAll");
    await a.waitFor(() => st().unread.ok === 0 && st().unread.error === 0, 8000, "read");
    check(!a.probe("status.notice.error"), "Mark all read clears them");
  });

  await step("errors", async () => {
    await pal("Test: dictation error");
    await a.waitFor(() => st().composer?.error === "microphone access is blocked", 3000, "dictation error");
    await a.shot("parity-13-dictation-error");
    await a.waitFor(() => st().dictErrorGone?.message === "microphone access is blocked", 9000, "dictation error gone");
    const ms = st().dictErrorGone.lifeMs;
    check(ms >= 6000 && ms < 6400, `a dictation error shows in the composer error line for ${ms} ms (6 s)`);
    await pal("Test: engine error");
    await a.waitFor(() => (st().composer?.error ?? "").includes("test engine error"), 3000, "engine error");
    await sleep(7000);
    await a.shot("parity-14-engine-error");
    check((st().composer?.error ?? "").includes("test engine error"), `an engine error stays in the error line (${st().composer?.error})`);
  });

  await step("mic", async () => {
    const audio0 = await audioMs(st().current);
    await a.click("composer.input");
    await pal("Test: mic tap");
    await a.waitFor(() => st().dict === "connecting" || st().dict === "listening", 4000, "tap started");
    await a.waitFor(() => !!st().micRelease, 3000, "released");
    await a.waitFor(() => st().dict === "listening", 10000, "listening");
    await sleep(1200);
    await a.shot("parity-15-mic-listening");
    check(st().micRelease.stops === false && st().dict === "listening" && st().composer?.dictating === true, `a tap (released after ${st().micRelease.ms} ms) keeps dictating; red outline on (dictating ${st().composer?.dictating})`);
    // a press while dictating stops
    await pal("Test: mic tap");
    await a.waitFor(() => st().dict === "idle", 15000, "stopped by a press");
    check(true, "a press while dictating stops it");
    await sleep(800);
    // hold: released after 650 ms → stops
    await pal("Test: mic hold");
    await a.waitFor(() => st().dict === "connecting" || st().dict === "listening", 4000, "hold started");
    await a.waitFor(() => st().micRelease?.ms >= 600, 4000, "hold released");
    await a.waitFor(() => st().dict === "finishing" || st().dict === "idle", 4000, "hold stopped");
    check(st().micRelease.stops === true, `push-to-talk: released after ${st().micRelease.ms} ms (> 300) stops`);
    await a.waitFor(() => st().dict === "idle", 15000, "idle");
    const seen = st().micSeen ?? [];
    check(["idle", "connecting", "listening", "finishing"].every((x) => seen.includes(x)), `mic button drew every state: ${JSON.stringify(seen)}`);
    // I7: the dictations went into the hub's ledger as audio.ms (kind dictation) of this task
    let audio1 = audio0;
    for (let k = 0; k < 20 && audio1 <= audio0; k++) { await sleep(500); audio1 = await audioMs(st().current); }
    check(audio1 > audio0, `audio.ms (kind dictation) recorded by the hub for #${st().current}: ${audio0} → ${audio1} ms`);
    await a.key("cmd+a"); await a.key("BackSpace");
  });

  await step("composer-images", async () => {
    await a.click("composer.input");
    await pal("Test: paste image");
    await a.waitFor(() => st().composer?.images === 1, 3000, "pasted");
    await pal("Test: drop big image");
    await a.waitFor(() => st().composer?.images === 2, 5000, "dropped");
    const [w, h] = st().composer.dims[1];
    check(Math.max(w, h) === 1568 && Math.abs(w / h - 1.5) < 0.01, `a dropped 3000×2000 image is downscaled to ${w}×${h} (1568 px longest side, ${st().composer.types[1]})`);
    await pal("Test: attach 21 images");
    await a.waitFor(() => st().composer?.images === 20, 5000, "cap");
    check(true, "at most 20 images (21 more offered, 20 kept)");
    await sleep(400);
    await a.shot("parity-16-composer-images");
    await a.click("thumbx.0");
    await a.waitFor(() => st().composer?.images === 19, 3000, "removed");
    check(true, "the thumbnail × removes an image (20 → 19)");
  });

  await step("newtask-images", async () => {
    await pal("Test: new task with 21 images");
    await a.waitFor(() => st().overlay === "newtask" && st().newtask?.images === 20, 5000, "dialog images");
    await sleep(400);
    await a.shot("parity-17-newtask-images");
    await a.click("ntthumb.0");
    await a.waitFor(() => st().newtask?.images === 19, 3000, "removed");
    check(true, "new-task dialog: images capped at 20, × removes one");
    await a.key("Escape");
    await a.waitFor(() => st().overlay === null, 3000, "closed");
    check(st().focus === "composer", `Esc closes the dialog and gives the focus back to the composer (${st().focus})`);
    await pal("Test: new task paste image");
    await a.waitFor(() => st().overlay === "newtask" && st().newtask?.images === 1, 5000, "pasted into the dialog");
    check(true, "⌘V of an image in the dialog attaches it");
    await a.key("Escape");
    await a.waitFor(() => st().overlay === null, 3000, "closed");
  });

  await step("continue-select", async () => {
    await a.key("cmd+0");
    await a.key("cmd+n");
    await a.waitFor(() => st().overlay === "newtask", 4000, "dialog");
    await a.click("newtask.mode.continue");
    await a.waitFor(() => st().newtask?.mode === "continue", 3000, "continue");
    check(st().newtask.prev === null && !st().newtask.ok, "continue without a chosen task can't start");
    const p = await a.waitProbe("newtask.prevrow");
    await a.clickAt(p[0] + 60, p[1] + p[3] - 14);
    await sleep(500);
    await a.shot("parity-18-continue-menu");
    await a.key("Down");
    await a.key("Return");
    await a.waitFor(() => st().newtask?.prev !== null, 3000, "prev chosen");
    await a.shot("parity-19-continue-chosen");
    check(true, `the which-task select picks a finished sibling (#${st().newtask.prev})`);
    await a.key("Escape");
    await a.waitFor(() => st().overlay === null, 3000, "closed");
  });

  await step("continues-link", async () => {
    await a.switchTo(23);
    await a.click("taskbar.prev");
    await a.waitFor(() => st().current === 22 && st().loaded, 5000, "prev opened");
    check(true, "the task bar's continues #22 link opens #22");
  });

  await step("tasks-panel", async () => {
    await a.switchTo(92);
    await view("cmd+1", "tasks");
    // K5: a row click opens the task
    await revealSide("trow.93");
    await a.click("trow.93");
    await a.waitFor(() => st().current === 93 && st().tabs.includes(93), 4000, "row opened");
    check(true, "a task row click opens it in a tab");
    // K4
    await a.waitFor(() => !!a.probe("tasks.nokids") && st().tasksPanel?.noKids === true, 3000, "no subtasks yet");
    await a.shot("parity-20-no-subtasks");
    check(true, "a task without children shows \"no subtasks yet\"");
    // K2 chevron collapse + U5 animation
    await a.switchTo(92);
    await sleep(600);
    await revealSide("trow.chev.92");
    const rows0 = st().tasksPanel.rows, f0 = st().chevronFrames ?? 0;
    await a.click("trow.chev.92");
    await a.waitFor(() => st().tasksPanel.rows < rows0, 3000, "collapsed");
    await sleep(300);
    check((st().chevronFrames ?? 0) - f0 >= 2, `the chevron collapses the subtree (${rows0} → ${st().tasksPanel.rows} rows) and turns over ${(st().chevronFrames ?? 0) - f0} animated frames`);
    await a.click("trow.chev.92");
    await a.waitFor(() => st().tasksPanel.rows === rows0, 3000, "expanded");
    // K2: the dot's tooltip
    // every task (rows don't move when the click opens the task)
    await a.click("side.scope");
    await sleep(600);
    await revealSide("trow.dot.94");
    await a.click("trow.dot.94");
    await a.waitFor(() => /empty trash/.test(st().tooltip ?? "") && /\ndone$/.test(st().tooltip ?? ""), 4000, "dot tooltip");
    await a.shot("parity-21-dot-tooltip");
    check(true, `the status dot has a tooltip: ${JSON.stringify(st().tooltip)}`);
    await a.click("side.scope");
    await sleep(300);
  });

  await step("tabs-keys", async () => {
    await a.switchTo(92);
    await a.switchTo(93);
    const tabs = st().tabs as number[];
    const k = tabs.indexOf(93);
    await a.key("cmd+shift+bracketright");
    await a.waitFor(() => st().current === tabs[(k + 1) % tabs.length], 3000, "⌘⇧]");
    await a.key("cmd+shift+bracketleft");
    await a.waitFor(() => st().current === 93, 3000, "⌘⇧[");
    check(true, `⌘⇧] / ⌘⇧[ move to the next / previous tab (${JSON.stringify(tabs)})`);
    const p = a.probe("tab.93")!;
    const [x, y] = a.px(p[0] + p[2] / 2, p[1] + p[3] / 2);
    await a.act(`await app.click([${x}, ${y}], { mouseButton: "middle" }); 1`);
    await a.waitFor(() => !st().tabs.includes(93), 3000, "middle-click closed");
    check(true, "a middle click closes the tab");
    await a.switchTo(92); // a running-capable task: finished tasks have no composer
    await a.click("feed");
    await a.waitFor(() => st().focus !== "composer", 2000, "focus left the composer");
    await a.key("cmd+l");
    await a.waitFor(() => st().focus === "composer", 2000, "⌘L");
    check(true, "⌘L focuses the composer");
    await a.key("cmd+comma");
    await a.waitFor(() => st().view === "settings" && st().sideOpen, 3000, "⌘,");
    check(true, "⌘, shows Settings");
    // M8 / M9: a switch focuses the composer; the composer is disabled until the task is loaded
    const d0 = st().composerDisabledWhileLoading ?? 0;
    await a.click("feed");
    await a.switchTo(41);
    await sleep(300);
    check(st().focus === "composer", `a task switch focuses the composer (${st().focus})`);
    check((st().composerDisabledWhileLoading ?? 0) > d0, `the composer drew disabled while the task loaded (${d0} → ${st().composerDisabledWhileLoading} renders)`);
  });

  await step("sidebar-drag", async () => {
    await view("cmd+1", "tasks");
    const w0 = st().ws.sideW;
    const r = await a.waitProbe("side.resizer");
    const [x0, y0] = a.px(r[0] + r[2] / 2, r[1] + 300);
    await a.act(`await app.drag([${x0}, ${y0}], [${x0 + 240}, ${y0}]); 1`);
    await a.waitFor(() => Math.abs(st().ws.sideW - (w0 + 120)) < 12, 3000, "dragged wider").catch(() => {});
    await sleep(500);
    await a.shot("parity-22-sidebar-wide");
    const w1 = st().ws.sideW;
    const saved = JSON.parse(Deno.readTextFileSync(`${a.dir}/prefs.json`)).side_w;
    check(Math.abs(w1 - (w0 + 120)) < 12 && Math.abs(saved - w1) < 0.5, `dragging the resizer 120 pt widens the sidebar ${w0} → ${Math.round(w1)} pt, saved in prefs (${Math.round(saved)})`);
    const r2 = a.probe("side.resizer")!;
    const [x1, y1] = a.px(r2[0] + r2[2] / 2, r2[1] + 300);
    await a.act(`await app.drag([${x1}, ${y1}], [${a.px(60, 0)[0]}, ${y1}]); 1`);
    await a.waitFor(() => st().ws.sideW === 220, 3000, "min width").catch(() => {});
    check(st().ws.sideW === 220, `and stops at 220 pt (${st().ws.sideW})`);
    await a.act(`await app.drag([${a.px(a.probe("side.resizer")![0] + 3, 0)[0]}, ${y1}], [${a.px(a.probe("side.resizer")![0] + 3, 0)[0] + 200}, ${y1}]); 1`);
    await sleep(500);
  });

  await step("budget-bar", async () => {
    await a.switchTo(92);
    await a.click("status.cost");
    await a.waitFor(() => st().overlay === "metrics", 3000, "popover");
    await a.type("0.01");
    await a.key("Return");
    await a.waitFor(() => st().statusbar?.budget === 0.01, 10000, "budget set");
    await sleep(400);
    await a.shot("parity-23-budget-bar");
    const sb = st().statusbar;
    check(!!a.probe("status.budgetbar") && sb.budgetPct === 100 && sb.budgetColor === "red", `a $0.01 budget on #92 shows the budget bar: ${sb.budgetPct}% ${sb.budgetColor} (cost $${sb.cost.toFixed(2)})`);
    await a.click("status.cost");
    await a.waitFor(() => st().overlay === "metrics", 3000, "popover");
    await a.key("cmd+a");
    await a.type(String(Math.ceil(sb.cost * 1.1 + 1)));
    await a.key("Return");
    await a.waitFor(() => st().statusbar?.budget === Math.ceil(sb.cost * 1.1 + 1), 10000, "budget 2");
    check(["amber", "accent"].includes(st().statusbar.budgetColor), `a larger budget: ${Math.round(st().statusbar.budgetPct)}% ${st().statusbar.budgetColor}`);
    await peer.command({ type: "control", task: 92, op: "budget", budget: {} }); // the shared test instance: no budget left behind
  });

  await step("palette-lifecycle", async () => {
    await a.switchTo(92);
    for (const [q, op] of [["Pause this task", "pause"], ["Close this task as done", "close"], ["Cancel this task", "cancel"]]) {
      await pal(q);
      await a.waitFor(() => st().ask === op, 3000, `ask ${op}`);
      await a.key("Escape");
      await a.waitFor(() => st().ask === null, 3000, "ask closed");
    }
    check(true, "palette: Pause / Close as done / Cancel open the task bar's ask field");
    await a.switchTo(106);
    await pal("Resume this task");
    await a.waitFor(() => st().ask === "resume", 3000, "ask resume");
    await a.key("Escape");
    await pal("New task");
    await a.waitFor(() => st().overlay === "newtask", 3000, "dialog");
    await a.key("Escape");
    await a.waitFor(() => st().overlay === null, 3000, "closed");
    await pal("Show models");
    await a.waitFor(() => st().view === "models", 3000, "models");
    await a.click("feed");
    await pal("Focus the composer");
    await a.waitFor(() => st().focus === "composer", 3000, "focus");
    check(true, "palette: Resume (paused #106), New task…, Show models, Focus the composer");
  });

  await step("menus", async () => {
    await view("cmd+6", "settings");
    const ax = async () => (await a.act(`nodeRepl.write(await app.getAXState({disableDiffing:true, emit:false}))`)).text;
    const bar = await ax();
    const idx = (name: string) => Number(bar.match(new RegExp(`\\n\\s*(\\d+) ${name}\\n`))?.[1]);
    const titles = ["OptChat", "File", "Edit", "View", "Task", "Window", "Help"];
    check(titles.every((t) => idx(t) > 0), `menu bar: ${titles.filter((t) => idx(t) > 0).join(", ")}`);
    const want: Record<string, string[]> = {
      OptChat: ["About OptChat", "Settings…", "Services", "Hide OptChat", "Hide Others", "Show All", "Quit OptChat"],
      File: ["New Task…", "Open Main Chat", "Go to Task…", "Close Tab"],
      Edit: ["Undo", "Redo", "Cut", "Copy", "Paste", "Select All"],
      View: ["Command Palette…", "Toggle Sidebar", "Tasks", "Memory", "Context", "Log", "Models", "Next Theme", "Bigger Text", "Smaller Text", "Actual Size", "Next Tab", "Previous Tab"],
      Task: ["Focus Composer", "Stop the Running Turn", "Pause…", "Resume…", "Close as Done…", "Cancel…"],
      Window: ["Minimize", "Zoom", "Show Next Tab", "Show Previous Tab", "Main Chat"],
      Help: ["Keyboard Shortcuts", "Commands…", "Show Data Folder"],
    };
    const missing: string[] = [];
    for (const t of titles) {
      await ax();
      await a.act(`await app.click(${idx(t)}); 1`);
      await sleep(500);
      const m = await ax();
      for (const item of want[t]) if (!m.includes(` ${item}`)) missing.push(`${t} › ${item}`);
      if (t === "Task") {
        // pick Pause… from our own menu: the task bar asks
        const pi = Number(m.match(/\n\s*(\d+) Pause…/)?.[1]);
        await a.act(`await app.click(${pi}); 1`);
        await a.waitFor(() => st().ask === "pause", 3000, "menu pause");
        await a.key("Escape");
        continue;
      }
      if (t === "View") {
        const mi = Number(m.match(/\n\s*(\d+) Models,/)?.[1]);
        await a.act(`await app.click(${mi}); 1`);
        await sleep(400);
        continue;
      }
      await a.act(`await app.performSecondaryAction(1, "Cancel"); 1`);
      await sleep(300);
    }
    check(missing.length === 0, `every menu has its items (${missing.length ? "missing " + missing.join("; ") : "OptChat, File, Edit, View, Task, Window, Help"}); Task › Pause… and View › Models picked`);
    check(st().view === "models" && st().sideOpen, `View › Models shows the Models view (${st().view})`);
  });

  await step("feed-paging-follow", async () => {
    await a.key("cmd+0");
    await a.waitFor(() => st().current === 0 && st().loaded, 6000, "main");
    await sleep(1000);
    const from0 = st().feed.from;
    check(from0 > 0, `the main chat opens on its last entries (from ${from0} of ${st().t})`);
    for (let i = 0; i < 30 && st().feed.from === from0; i++) { await a.scroll("feed", "up", 10); await sleep(250); }
    check(st().feed.from < from0, `scrolling to the top loads earlier messages (from ${from0} → ${st().feed.from})`);
    await a.shot("parity-24-load-earlier");
    // follow mode
    check(st().feed.scrolledUp === true && !!a.probe("feed.jump"), "scrolled up: follow mode off, jump-to-bottom button shown");
    const top = JSON.stringify(st().feed.top);
    await sleep(1500);
    check(JSON.stringify(st().feed.top) === top, "the view stays put while scrolled up");
    await a.click("feed.jump");
    await a.waitFor(() => st().feed.scrolledUp === false && !a.probe("feed.jump"), 3000, "pinned again");
    check(true, "the jump button returns to the bottom and follows again");
    await sleep(800);
  });

  await step("report-expand", async () => {
    const r = keys("rep.").sort((x, y) => a.probe(y)![1] - a.probe(x)![1])[0];
    check(!!r, `a long report card is shown clipped (${r})`);
    if (!r) return;
    const key = `r${r.split(".")[1]}`;
    const h0 = a.probe(r)![3];
    await a.reveal(r);
    await a.click(r);
    await a.waitFor(() => st().feed.open.includes(key), 3000, "expanded");
    await sleep(500);
    await a.shot("parity-25-report-expanded");
    check((a.probe(r)?.[3] ?? 0) > h0, `a click expands the report to its full text (${Math.round(h0)} → ${Math.round(a.probe(r)?.[3] ?? 0)} px)`);
  });

  await step("work-rows", async () => {
    Deno.writeTextFileSync(`${a.probes}.arg`, "722");
    await pal("Test: reveal entry");
    await a.waitFor(() => st().feed.from === 0 && !!a.probe("work.722.0"), 5000, "work row");
    await sleep(500);
    await a.reveal("work.722.0");
    await a.shot("parity-26-work-row");
    await a.click("work.722.0");
    await a.waitFor(() => st().current === 1 && st().loaded, 5000, "#1 opened");
    check(true, "a v2 work message [a1] … renders as a report block whose link opens #1");
  });

  await step("cut-text", async () => {
    await a.switchTo(106);
    await sleep(800);
    await a.reveal("tool.7");
    await a.click("tool.7");
    await a.waitFor(() => !!a.probe("cut.8"), 4000, "cut button");
    await a.reveal("cut.8");
    await a.shot("parity-29-cut");
    await a.click("cut.8");
    await a.waitFor(() => (st().feed.full ?? []).includes(8), 3000, "full text");
    await sleep(300);
    check(!a.probe("cut.8"), "a cut tool output shows \"shortened · show all N chars\"; a click shows the full text");
  });

  await step("turn-error", async () => {
    await a.switchTo(9);
    await sleep(800);
    const e = keys("foot.err.")[0];
    await a.shot("parity-30-turn-error");
    check(!!e, `a failed / stopped turn shows its error line "⚠ …" (${e}; parity-30)`);
  });

  await step("echo-images", async () => {
    await a.switchTo(32);
    await sleep(800);
    Deno.writeTextFileSync(`${a.probes}.arg`, "31");
    await pal("Test: reveal entry");
    await a.waitFor(() => !!a.probe("tool.31"), 4000, "tool card");
    await a.click("tool.31");
    await a.waitFor(() => (a.probe("toolimg.31")?.[3] ?? 0) > 100, 5000, "echo image");
    const box = a.probe("toolimg.31")!;
    await a.scroll("feed", "down", 0.3);
    await sleep(600);
    await a.shot("parity-31-echo-image");
    check(st().feed.open.includes("t31"), `a tool's echo image is drawn under its expanded tool card (#32 computer call: ${Math.round(box[2])}×${Math.round(box[3])} pt image strip; parity-31)`);
  });

  await step("markdown", async () => {
    await a.switchTo(70);
    await sleep(1200);
    await a.shot("parity-27-markdown");
    await a.key("cmd+0");
    await a.waitFor(() => st().current === 0 && st().loaded, 6000, "main");
    Deno.writeTextFileSync(`${a.probes}.arg`, "1596");
    await pal("Test: reveal entry");
    await sleep(1500);
    await a.shot("parity-28-markdown-quote");
    // a pending task (Add: no model call) whose instruction has a task list, a link and strikethrough: the parent
    // bubble renders it as markdown (the link is not clicked: it would open the browser)
    await a.key("cmd+n");
    await a.waitFor(() => st().overlay === "newtask", 4000, "dialog");
    await a.type("markdown sample");
    await a.click("newtask.instruction");
    for (const line of ["Checklist:", "", "- [x] a finished item", "- [ ] an open item with ~~old~~ new text", "", "See [the docs](https://example.com/docs) and *notes*."]) {
      if (line) await a.type(line);
      await a.key("Return");
    }
    await a.click("newtask.add");
    await a.waitFor(() => st().overlay === null && st().status === "pending", 8000, "pending child");
    await sleep(1200);
    await a.shot("parity-28b-markdown-tasklist");
    const b = st().mdBlocks ?? [], il = st().mdInline ?? [];
    check(["para", "heading", "list", "tasklist", "code", "table", "quote"].every((x) => b.includes(x)) && ["code", "bold", "italic", "strike", "link"].every((x) => il.includes(x)), `markdown drawn in the feed: blocks ${JSON.stringify(b)}, inline ${JSON.stringify(il)} (parity-27/28/28b)`);
  });

  await step("dictate-again", async () => {
    // last: the engine's previous session's done event also clears the new session (same task), which then can't be
    // stopped (hub.rs dictate_start; reported) — so nothing runs after this in this launch but the child empty state
    await a.click("composer.input");
    await pal("Test: mic tap");
    await a.waitFor(() => st().dict === "listening", 10000, "listening");
    // I5: a new dictation finishes the previous one
    await a.waitFor(() => /dictation/i.test(st().ws?.dictText ?? ""), 15000, "text").catch(() => {});
    const d0 = st().ws.dictDone;
    await pal("Test: dictate again");
    await a.waitFor(() => st().ws.dictDone > d0, 15000, "previous finished");
    check(true, `a new dictation finishes the previous one (done events ${d0} → ${st().ws.dictDone})`);
    await a.waitFor(() => st().dict === "listening", 10000, "second listening").catch(() => {});
  });

  await step("child-empty", async () => {
    await a.switchTo(93);
    await pal("Test: empty feed");
    await a.waitFor(() => (st().feedEmpty ?? []).includes("child") && !!a.probe("feed.empty"), 3000, "child empty");
    await a.shot("parity-32-child-empty");
    check(/^This task has no messages yet\./.test(st().feedEmptyText ?? ""), `a child with no messages: "${st().feedEmptyText}"`);
  });
} catch (e) { check(false, String(e)); await a.shot("parity-fail"); } finally { await a.quit(); }

// L16 (engine clients): no fresh workspace exists to show the root's intro; with nothing connected the app shows
// what to do next (Add engine / Workspaces)
try {
  a = await App.launch({ bare: true, env });
  await a.waitFor(() => !!a.probe("empty") && !!a.probe("empty.add"), 8000, "no-workspace state");
  await a.shot("parity-33-no-workspace");
  check(true, "nothing connected: the editor shows Add engine / Workspaces");
} catch (e) { check(false, `no workspace: ${String(e).slice(0, 300)}`); } finally { await a.quit(); }
try { Deno.removeSync(`${a.probes}.arg`); } catch { /* none */ }
try { Deno.removeSync(tmp, { recursive: true }); } catch { /* gone */ }
peer.close();
Deno.exit(failed() ? 1 : 0);
