// Panels, shortcuts and chrome: rail keys, zoom, status bar menus, budgets popover, models rate edit, log subtree,
// context wire, settings tabs, narrow layout, context menu, tabs, memory search reveal. No model calls.
import { App, check, failed } from "./lib/app.ts";
import { Peer } from "./lib/hub.ts";
import { sleep } from "./lib/cua.ts";
const a = await App.launch({ env: { OPTCHAT_TEST_HOOKS: "1" } });
const st = () => a.state();
const pal = (q: string) => a.palette(q);
const visibleIn = (p: number[] | undefined, box: number[] | undefined) => !!p && !!box && p[1] >= box[1] && p[1] + p[3] <= box[1] + box[3];
/** scroll the sidebar until a probed element is fully visible in it */
const reveal = async (name: string) => {
  for (let i = 0; i < 12; i++) {
    const p = a.probe(name), side = a.probe("sidebar");
    if (visibleIn(p, side)) return p!;
    await a.scroll("sidebar", p && side && p[1] < side[1] ? "up" : "down", 2);
    await sleep(300);
  }
  throw new Error(`${name} not visible in the sidebar`);
};
const step = async (name: string, f: () => Promise<void>) => {
  try { await f(); } catch (e) { check(false, `${name}: ${String(e).slice(0, 400)}`); console.log(JSON.stringify({ ...st(), lastTalk: undefined, frames: undefined })); await a.shot(`panels-fail-${name}`); await a.key("Escape"); await sleep(300); }
};
try {
  await step("rail-keys", async () => {
    const views: [string, string][] = [["cmd+2", "memory"], ["cmd+3", "context"], ["cmd+4", "log"], ["cmd+5", "models"], ["cmd+6", "settings"], ["cmd+1", "tasks"]];
    for (const [k, v] of views) {
      await a.key(k);
      await a.waitFor(() => st().view === v && st().sideOpen, 3000, `view ${v}`);
    }
    check(true, "⌘1…⌘6 switch the sidebar view");
    await a.key("cmd+b");
    await a.waitFor(() => !st().sideOpen, 3000, "sidebar hidden");
    await a.key("cmd+b");
    await a.waitFor(() => st().sideOpen, 3000, "sidebar shown");
    check(true, "⌘B hides and shows the sidebar");
  });
  await step("zoom", async () => {
    const f0 = st().fontSize;
    await a.key("cmd+equal");
    await a.waitFor(() => st().fontSize > f0, 3000, "zoom in");
    await a.shot("panels-01-zoomed");
    await a.key("cmd+shift+0");
    await a.waitFor(() => st().fontSize === 14, 3000, "zoom reset");
    check(true, `⌘= zooms the chat text (${f0} → ${f0 + 1}), ⌘⇧0 resets to 14`);
  });
  await step("status-menus", async () => {
    await a.click("status.model");
    await sleep(500);
    await a.shot("panels-02-model-menu");
    await a.key("Escape"); await sleep(300);
    await a.click("status.effort");
    await sleep(500);
    await a.shot("panels-03-effort-menu");
    await a.key("Escape"); await sleep(300);
    check(true, "status bar model and effort menus open (see screenshots)");
    await a.click("status.cost");
    await a.waitFor(() => st().overlay === "metrics", 3000, "metrics popover");
    await sleep(400);
    await a.shot("panels-04-metrics");
    check(!!a.probe("metrics.set"), "the cost chip opens the budgets popover");
    await a.key("Escape");
    await a.waitFor(() => st().overlay === null, 3000, "popover closed");
  });
  await step("models-rate", async () => {
    await a.key("cmd+5");
    await a.waitFor(() => st().view === "models", 3000, "models");
    await sleep(800);
    await a.shot("panels-05-models");
    await a.click("model.edit.claude-haiku-5-5", { wait: 8000 });
    const r = await a.waitProbe("rate.0");
    await a.clickAt(r[0] + r[2] / 2, r[1] + r[3] - 10);
    await a.key("cmd+a");
    await a.type("1.25");
    await a.shot("panels-06-rate-edit");
    await reveal("rate.save");
    await a.click("rate.save");
    await sleep(800);
    await a.shot("panels-07-rate-saved");
    check(true, "rate editor opened and saved (checked in the database at the end)");
  });
  await step("log", async () => {
    await a.key("cmd+4");
    await a.waitFor(() => st().view === "log", 3000, "log");
    await sleep(1200);
    await a.shot("panels-08-log");
    await a.click("log.subtree");
    await a.waitFor(() => st().log?.subtree === true, 3000, "subtree").catch(() => {});
    await sleep(800);
    await a.shot("panels-09-log-subtree");
    check(st().log?.subtree === true, "the request log switches to the task with its subtree");
  });
  await step("context", async () => {
    await a.key("cmd+3");
    await a.waitFor(() => st().view === "context", 3000, "context");
    await sleep(1500);
    await a.shot("panels-10-context");
    await a.waitProbe("ctx.wire", 8000);
    await reveal("ctx.wire");
    await a.click("ctx.wire");
    await a.waitFor(() => st().context?.wire === true, 3000, "wire").catch(() => {});
    await sleep(800);
    await a.shot("panels-11-context-wire");
    check(st().context?.wire === true, "the context debugger shows the wire request");
  });
  await step("settings", async () => {
    await a.key("cmd+6");
    await a.waitFor(() => st().view === "settings", 3000, "settings");
    await a.click("settings.global");
    await a.waitFor(() => st().settingsGlobal === true, 3000, "global tab");
    await sleep(500);
    await a.shot("panels-12-settings-global");
    await a.click("settings.task");
    await a.waitFor(() => st().settingsGlobal === false, 3000, "task tab");
    await sleep(500);
    await a.shot("panels-13-settings-task");
    check(true, "settings Global and Task tabs");
  });
  await step("narrow", async () => {
    await a.key("cmd+1");
    await pal("Test: narrow window");
    await a.waitFor(() => st().narrow === true, 4000, "narrow");
    await sleep(600);
    check(!st().sideOpen, "narrow window starts with the sidebar closed");
    await a.key("cmd+b");
    await a.waitFor(() => st().sideOpen, 3000, "sheet open");
    await sleep(500);
    await a.shot("panels-14-narrow-sheet");
    check(true, "⌘B floats the sidebar over the editor in a narrow window (see screenshot)");
    await a.key("Escape");
    await a.waitFor(() => !st().sideOpen, 3000, "Esc closes the sheet");
    check(true, "Esc closes the floating sidebar");
    await pal("Test: wide window");
    await a.waitFor(() => st().narrow === false, 4000, "wide");
    await sleep(600);
  });
  await step("context-menu", async () => {
    await a.click("feed.last", { right: true });
    await sleep(600);
    await a.shot("panels-15-context-menu");
    await a.key("Escape"); await sleep(300);
    check(true, "right-click on a message opens its context menu (see screenshot)");
  });
  await step("tabs", async () => {
    await a.switchTo(92);
    check(st().tabs.includes(92), "the switcher opens #92 in a tab");
    await sleep(500);
    await a.shot("panels-16-tab");
    await a.key("ctrl+Tab");
    await a.waitFor(() => st().current !== 92, 3000, "next tab");
    await a.key("ctrl+shift+Tab");
    await a.waitFor(() => st().current === 92, 3000, "previous tab");
    check(true, "ctrl-tab / ctrl-shift-tab cycle tabs");
    await a.key("cmd+w");
    await a.waitFor(() => !st().tabs.includes(92), 3000, "tab closed");
    check(st().current === 0, "⌘W closes the tab and returns to the main chat");
  });
  await step("memory-reveal", async () => {
    await a.key("cmd+2");
    await a.waitFor(() => st().view === "memory", 3000, "memory");
    await sleep(1200);
    await a.click("mem.search");
    await a.type("docker");
    await a.key("Return");
    await sleep(1500);
    const side = a.probe("sidebar");
    const hits = Object.entries(a.allProbes()).filter(([k, p]) => k.startsWith("hit.") && visibleIn(p, side)).sort((x, y) => x[1][1] - y[1][1]);
    check(hits.length > 0, `memory search shows visible hits (${hits.length})`);
    await a.shot("panels-17-memory-search");
    if (hits[0]) {
      await a.click(hits[0][0]);
      await sleep(1200);
      await a.shot("panels-18-memory-reveal");
      const [, t, i] = hits[0][0].split(".");
      check(st().memory?.hit === `${t}:0:${i}`, `the hit reveals message ${i} in the tree (${st().memory?.hit})`);
    }
  });
  // the rate saved through the UI is the hub's rate in force (read by a second client)
  await step("rate-in-hub", async () => {
    const peer = await Peer.connect(a.db, 0);
    const h = (await peer.rpc("models.list")).find((m: any) => m.id === "claude-haiku-5-5");
    peer.close();
    check(h?.rate?.per1m?.in === 1.25 && h?.rate?.source === "manual", `the rate saved through the UI is the hub's rate in force (in $${h?.rate?.per1m?.in}/1M, ${h?.rate?.source})`);
  });
} catch (e) { check(false, String(e)); await a.shot("panels-fail"); } finally { await a.quit(); }
Deno.exit(failed() ? 1 : 0);
