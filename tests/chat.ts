// Feed details and dialogs on existing history (no model calls): tool card expand, step table, context chip →
// Context view, image lightbox, new-task dialog modes, Continue as a new task, task row / tab context menus.
import { App, check, failed } from "./lib/app.ts";
import { sleep } from "./lib/cua.ts";
const a = await App.launch();
const st = () => a.state();
const keys = (pre: string) => Object.keys(a.allProbes()).filter((k) => k.startsWith(pre));
const feedBox = () => a.probe("feed");
const visible = (name: string) => {
  const p = a.probe(name), f = feedBox();
  return !!p && !!f && p[1] >= f[1] && p[1] + p[3] <= f[1] + f[3];
};
/** the lowest probe with this prefix (the newest in the feed), scrolled into view */
const lastVisible = (pre: string) => keys(pre).filter(visible).sort((x, y) => a.probe(y)![1] - a.probe(x)![1])[0];
/** the newest fully visible element with this prefix, scrolling up a page at a time until one shows */
const newest = async (pre: string) => {
  for (let i = 0; i < 8; i++) {
    const k = lastVisible(pre);
    if (k) return k;
    await a.scroll("feed", "up", 1);
    await sleep(500);
  }
  return undefined;
};
const open = async (id: number) => {
  await a.switchTo(id);
  await sleep(1200);
};
const step = async (name: string, f: () => Promise<void>) => {
  try { await f(); } catch (e) { check(false, `${name}: ${String(e).slice(0, 300)}`); await a.shot(`chat-fail-${name}`); await a.key("Escape"); await sleep(300); }
};
try {
  await step("tool-card", async () => {
    await open(92);
    const t = await newest("tool.");
    check(!!t, `a tool card is shown in #92 (${t})`);
    if (!t) return;
    const before = a.probe(t)!;
    const key = `t${t.split(".")[1]}`;
    await a.click(t);
    await a.waitFor(() => st().feed?.open?.includes(key), 3000, "tool card open").catch(() => {});
    await sleep(500);
    await a.shot("chat-01-tool-open");
    check(st().feed?.open?.includes(key) && a.probe(t)![3] >= before[3], `the tool card expands to its input / output (${key})`);
    await a.click(t, { dx: before[2] / 2, dy: before[3] / 2 }); // collapse again
    await sleep(400);
  });
  await step("steps-and-context", async () => {
    const s = await newest("foot.steps.");
    check(!!s, `turn foot shows the steps chip (${s})`);
    if (s) {
      const fk = `foot${s.slice("foot.steps.".length)}`;
      await a.click(s);
      await a.waitFor(() => st().feed?.open?.includes(fk), 3000, "step table open").catch(() => {});
      await sleep(400);
      await a.shot("chat-02-steps");
      check(st().feed?.open?.includes(fk), "the steps chip opens the turn's step table");
      await a.click(s);
      await sleep(400);
    }
    const c = await newest("foot.ctx.");
    check(!!c, "turn foot shows context ↗");
    if (c) {
      await a.click(c);
      await a.waitFor(() => st().view === "context" && st().sideOpen && !!st().selectedRequest, 4000, "context view on that call");
      await sleep(1200);
      await a.shot("chat-03-context-from-chip");
      check(true, `context ↗ pinned call ${st().selectedRequest} in the Context view`);
    }
  });
  await step("lightbox", async () => {
    await open(8);
    const im = await newest("img.");
    check(!!im, `an image is shown in #8 (${im})`);
    if (!im) return;
    await a.click(im);
    await a.waitFor(() => st().overlay === "lightbox", 4000, "lightbox");
    await sleep(500);
    await a.shot("chat-04-lightbox");
    await a.key("Escape");
    await a.waitFor(() => st().overlay === null, 4000, "lightbox closed");
    check(true, "image opens in the lightbox; Esc closes it");
  });
  await step("memory-image", async () => {
    // #8's first message is the picture: open it in the Memory view's tree
    await a.key("cmd+2");
    await a.waitFor(() => st().view === "memory", 3000, "memory");
    await sleep(1200);
    const leaf = a.probe("mem.8:0:1") ? "mem.8:0:1" : keys("mem.8:").sort((x, y) => a.probe(x)![1] - a.probe(y)![1])[0];
    check(!!leaf, `the memory tree of #8 lists its lines (${leaf})`);
    if (leaf) {
      await a.click(leaf);
      await sleep(1500);
      await a.shot("chat-04b-memory-image");
      check((st().memory?.open ?? []).includes(leaf.slice(4)), `the memory line opens down to its message, with the picture (${leaf}; chat-04b-memory-image)`);
    }
    await a.key("cmd+1");
  });
  await step("continue", async () => {
    // #8 is done: the end bar offers Continue as a new task
    await a.click("endbar.continue");
    await a.waitFor(() => st().overlay === "newtask", 4000, "dialog");
    await sleep(500);
    await a.shot("chat-05-continue-dialog");
    check(st().newtask?.mode === "continue", `Continue as a new task opens the dialog preset to continue (mode ${st().newtask?.mode})`);
    await a.key("Escape");
    await a.waitFor(() => st().overlay === null, 4000, "closed");
  });
  await step("newtask-modes", async () => {
    await a.key("cmd+0");
    await a.waitFor(() => st().current === 0, 4000, "main");
    await a.key("cmd+n");
    await a.waitFor(() => st().overlay === "newtask", 4000, "dialog");
    for (const m of ["fork", "blind", "continue", "handoff"]) {
      if (!a.probe(`newtask.mode.${m}`)) { check(false, `mode card ${m}`); continue; }
      await a.click(`newtask.mode.${m}`);
      await a.waitFor(() => st().newtask?.mode === m, 2000, `mode ${m}`).catch(() => {});
      await a.shot(`chat-06-mode-${m}`);
      check(st().newtask?.mode === m, `mode card ${m} selects ${m}`);
    }
    await a.key("Escape");
    await a.waitFor(() => st().overlay === null, 4000, "closed");
  });
  await step("row-menu", async () => {
    if (!(st().view === "tasks" && st().sideOpen)) await a.key("cmd+1"); // ⌘1 on the shown view hides it
    await a.waitFor(() => st().view === "tasks" && st().sideOpen, 3000, "tasks");
    await sleep(600);
    const row = keys("trow.").filter((k) => k !== "trow.0").sort((x, y) => a.probe(x)![1] - a.probe(y)![1])[0];
    await a.click(row, { right: true });
    await a.waitFor(() => st().taskMenu === true, 3000, "row menu open");
    await sleep(300);
    await a.shot("chat-07-row-menu");
    await a.key("Escape");
    await a.waitFor(() => st().taskMenu === false, 3000, "row menu closed");
    check(true, `right-click on a task row opens its menu; Esc closes it (${row})`);
    await a.click("tab.0", { right: true });
    await a.waitFor(() => st().taskMenu === true, 3000, "tab menu open");
    await sleep(300);
    await a.shot("chat-08-tab-menu");
    await a.key("Escape");
    await a.waitFor(() => st().taskMenu === false, 3000, "tab menu closed");
    check(true, "right-click on a tab opens the same task menu (main chat: Pause…, New subtask…, Copy #0)");
  });
} catch (e) { check(false, String(e)); await a.shot("chat-fail"); } finally { await a.quit(); }
Deno.exit(failed() ? 1 : 0);
