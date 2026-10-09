// Screenshots of every view, for review.
import { App } from "./lib/app.ts";
import { sleep } from "./lib/cua.ts";
const a = await App.launch();
const step = async (name: string, f: () => Promise<void>) => { try { await f(); await sleep(600); await a.shot(`tour-${name}`); console.log("shot", name); } catch (e) { console.log("FAIL", name, String(e).slice(0, 300)); } };
try {
  await step("memory", async () => { await a.click("rail.memory"); });
  await step("context", async () => { await a.click("rail.context"); });
  await step("log", async () => { await a.click("rail.log"); });
  await step("models", async () => { await a.click("rail.models"); });
  await step("settings-task", async () => { await a.click("rail.settings"); await sleep(300); await a.click("settings.task"); });
  await step("settings-global", async () => { await a.click("settings.global"); });
  await step("tasks-all", async () => { await a.click("rail.tasks"); await a.click("side.scope"); });
  await step("newtask", async () => { await a.key("cmd+n"); });
  await a.key("Escape");
  await step("palette", async () => { await a.key("cmd+k"); await a.type("the"); });
  await a.key("Escape");
  await step("metrics", async () => { await a.click("status.cost"); });
  await a.key("Escape");
  await step("child", async () => { await a.key("cmd+t"); await a.type("#92"); await a.key("Return"); });
  await step("ask-pause", async () => { await a.click("act.pause"); });
  await a.key("Escape");
} finally { await a.quit(); }
