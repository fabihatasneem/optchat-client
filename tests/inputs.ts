// Text inputs and persistence (no model calls): composer auto-grow (capped ~260 px), new-task title ≤ 80 and the
// instruction box growing, close-result ≤ 280 / other ask inputs ≤ 2000, then a relaunch on the same data:
// open tabs, current task, placeholder and window bounds restored.
import { App, check, failed } from "./lib/app.ts";
import { sleep } from "./lib/cua.ts";
let a = await App.launch({ env: { OPTCHAT_TEST_HOOKS: "1" } });
const st = () => a.state();
const h = (name: string) => a.probe(name)?.[3] ?? 0;
const step = async (name: string, f: () => Promise<void>) => {
  try { await f(); } catch (e) { check(false, `${name}: ${String(e).slice(0, 300)}`); await a.shot(`inputs-fail-${name}`); await a.key("Escape"); await sleep(300); }
};
let dir = "";
try {
  await step("composer-grow", async () => {
    await a.click("composer.input");
    const h0 = h("composer.input");
    for (let i = 1; i <= 4; i++) { await a.type(`line ${i}`); await a.key("shift+Return"); }
    await sleep(400);
    const h4 = h("composer.input");
    for (let i = 5; i <= 24; i++) { await a.type(`line ${i}`); await a.key("shift+Return"); }
    await sleep(400);
    const h24 = h("composer.input");
    await a.shot("inputs-01-composer-grown");
    check(h4 > h0 + 50, `the composer grows with its text (${Math.round(h0)} → ${Math.round(h4)} px at 5 lines)`);
    check(h24 <= 262 && h24 >= h4, `and stops growing at ~260 px (${Math.round(h24)} px at 25 lines)`);
    await a.key("cmd+a"); await a.key("BackSpace");
    await sleep(400);
    check(h("composer.input") <= h0 + 4, `and shrinks back when cleared (${Math.round(h("composer.input"))} px)`);
  });
  await step("newtask-limits", async () => {
    await a.key("cmd+n");
    await a.waitFor(() => st().overlay === "newtask", 4000, "dialog");
    await a.type("x".repeat(100));
    await a.waitFor(() => (st().newtask?.title ?? "").length > 0, 3000, "title typed");
    check(st().newtask.title.length === 80, `the title stops at 80 characters (${st().newtask.title.length})`);
    const order: number[] = [];
    for (let i = 0; i < 4; i++) { await a.key("Tab"); await sleep(250); order.push(st().newtask.field); }
    await a.key("shift+Tab"); await sleep(250);
    check(JSON.stringify(order) === "[1,2,3,0]" && st().newtask.field === 3, `Tab moves title → instruction → $ → minutes → title, Shift+Tab back (${JSON.stringify(order)}, then ${st().newtask.field})`);
    await a.click("newtask.instruction");
    const i0 = h("newtask.instruction");
    for (let i = 1; i <= 9; i++) { await a.type(`step ${i}`); await a.key("Return"); } // Return is a newline here
    await sleep(400);
    const i9 = h("newtask.instruction");
    await a.shot("inputs-02-newtask");
    check(i0 >= 90, `the instruction box shows several lines from the start (${Math.round(i0)} px)`);
    check(i9 > i0 + 40, `and grows with the text (${Math.round(i0)} → ${Math.round(i9)} px)`);
    await a.key("Escape");
    await a.waitFor(() => st().overlay === null, 3000, "closed");
  });
  await step("ask-limits", async () => {
    await a.switchTo(92);
    await a.click("act.close");
    await a.waitFor(() => st().ask === "close", 3000, "close ask");
    await a.type("y".repeat(300));
    await sleep(300);
    check(st().askText.length === 280, `the close result stops at 280 characters (${st().askText.length})`);
    await a.key("Escape");
    await a.waitFor(() => st().ask === null, 3000, "ask closed");
    await a.click("act.pause");
    await a.waitFor(() => st().ask === "pause", 3000, "pause ask");
    await a.type("z".repeat(300));
    await sleep(300);
    check(st().askText.length === 300, `a pause reason is not cut at 280 (${st().askText.length}; limit 2000)`);
    await a.key("Escape");
    await a.waitFor(() => st().ask === null, 3000, "ask closed");
  });
  await step("relaunch", async () => {
    await a.palette("Test: narrow window");
    await a.waitFor(() => st().narrow === true, 4000, "narrow");
    await sleep(1500); // bounds are saved 800 ms after the last change
    const w = st().window;
    check(Array.isArray(w) && Math.round(w[2]) === 700, `window bounds remembered (${JSON.stringify(w)})`);
    dir = a.dir;
    await a.quit({ keep: true });
    a = await App.launch({ dir, env: { OPTCHAT_TEST_HOOKS: "1" } });
    await sleep(800);
    check(st().current === 92 && st().tabs.includes(92), `relaunch reopens the tabs and the current task (#${st().current}, tabs ${JSON.stringify(st().tabs)})`);
    check(st().placeholderTask === 92, "the composer starts on that task (placeholder Message #92…)");
    const w2 = st().window;
    check(st().narrow === true && Math.round(w2?.[2]) === 700 && Math.abs(w2?.[3] - w[3]) < 1, `the window comes back at its saved size (${JSON.stringify(w)} → ${JSON.stringify(w2)})`);
    await a.shot("inputs-03-relaunched");
    await a.palette("Test: wide window");
  });
} catch (e) { check(false, String(e)); await a.shot("inputs-fail"); } finally { await a.quit(); }
Deno.exit(failed() ? 1 : 0);
