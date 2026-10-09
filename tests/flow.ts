// Task lifecycle through the UI with Claude Haiku: spawn, tool card, pending add/append/run, pause/resume, close, cancel.
import { App, check, failed } from "./lib/app.ts";
import { sleep } from "./lib/cua.ts";
const a = await App.launch();
const st = () => a.state();
try {
  await a.palette("Model: Claude Haiku");
  await a.waitFor(() => st().model === "claude-haiku-5-5", 8000, "haiku");
  // spawn a handoff child from the dialog
  await a.key("cmd+n");
  await a.waitFor(() => st().overlay === "newtask", 5000, "dialog");
  await a.type("say pong");
  await a.click("newtask.instruction");
  await a.type("Run `echo tool-ok-7` with the bash tool, then reply with exactly: pong-child plus that output.");
  await a.shot("flow-01-dialog");
  await a.click("newtask.start");
  await a.waitFor(() => st().current !== 0, 10000, "child opened");
  const child = st().current;
  check(st().tabs.includes(child), `spawned #${child} opened in a tab`);
  await a.waitFor(() => !st().running && (st().lastTalk ?? "").includes("pong-child"), 180000, "child reply");
  check((st().lastTalk ?? "").includes("tool-ok-7"), `child replied with the tool output: ${st().lastTalk}`);
  await sleep(600);
  await a.shot("flow-02-child");
  // expand the tool card
  const tool = Object.keys(a.allProbes()).find((k) => k.startsWith("foot.model"));
  check(!!tool, "turn foot shows the model chip");
  // back to the main chat: the report card
  await a.key("cmd+0");
  await a.waitFor(() => st().current === 0 && st().loaded, 8000, "main");
  await sleep(800);
  check(st().lastKinds.includes("task"), `main chat got the child's report (${st().lastKinds.join(",")})`);
  await a.shot("flow-03-report");
  // pending: Add from the dialog
  await a.key("cmd+n");
  await a.waitFor(() => st().overlay === "newtask", 5000, "dialog 2");
  await a.type("count to two");
  await a.click("newtask.instruction");
  await a.type("Reply with exactly: one two");
  await a.click("newtask.add");
  await a.waitFor(() => st().current !== 0 && st().status === "pending", 10000, "pending child");
  const pend = st().current;
  check(st().unrun === 0, "pending: instruction logged as not run yet (unrun 0)");
  await a.click("composer.input");
  await a.type("and then say three");
  await a.key("cmd+Return");
  await a.waitFor(() => st().t === 2 && st().status === "pending", 8000, "appended");
  check(true, "↑ (⌘Enter) appended to the pending task without running it");
  await sleep(400);
  await a.shot("flow-04-pending");
  await a.click("composer.run");
  await a.waitFor(() => st().status !== "pending", 8000, "started");
  await a.waitFor(() => !st().running && st().status === "idle" && (st().lastTalk ?? "").length > 0, 180000, "pending ran");
  check(/three/i.test(st().lastTalk ?? ""), `→ ran both messages: ${st().lastTalk}`);
  // pause / resume
  await a.click("act.pause");
  await a.waitFor(() => st().ask === "pause", 4000, "ask pause");
  await a.type("holding");
  await a.key("Return");
  await a.waitFor(() => st().status === "paused", 8000, "paused");
  check(true, "paused from the tab action");
  await a.shot("flow-05-paused");
  await a.click("act.resume");
  await a.key("Return");
  await a.waitFor(() => st().status !== "paused", 8000, "resumed");
  check(true, "resumed");
  // close as done
  await a.click("act.close");
  await a.type("said one two three");
  await a.key("Return");
  await a.waitFor(() => st().status === "done", 8000, "done");
  await sleep(500);
  await a.shot("flow-06-done");
  check(!!a.probe("endbar.continue"), "a done task offers Continue as a new task");
  // cancel the first child
  await a.key("cmd+t"); await sleep(300); await a.shot("flow-06b-switcher"); await a.type(`#${child} `); await sleep(300); await a.shot("flow-06c-typed"); await a.key("Return");
  await a.waitFor(() => st().current === child, 8000, "child again");
  await a.click("act.cancel");
  await a.key("Return");
  await a.waitFor(() => st().status === "canceled", 8000, "canceled");
  check(true, `canceled #${child}`);
  await a.key("cmd+0");
  await sleep(1000);
  await a.shot("flow-07-main");
} catch (e) { check(false, String(e)); } finally { await a.quit(); }
Deno.exit(failed() ? 1 : 0);
