// Two clients of one hub, in sync: the GPUI app and a second websocket client (as the browser would be) on the same
// TEST instance. Messages sent from either side stream into both; spawn / pause / resume / settings / rate edits from
// one side show in the other; the app reconnects (and resnapshots) after its connection drops. Claude Haiku only.
import { App, check, failed, testDb } from "./lib/app.ts";
import { sleep } from "./lib/cua.ts";
import { Peer } from "./lib/hub.ts";

const db = testDb("test-gpui-f");
const peer = await Peer.connect(db, 0);
await peer.useHaiku(0);
const a = await App.launch({ db, env: { OPTCHAT_TEST_HOOKS: "1" } });
const st = () => a.state();
const step = async (name: string, f: () => Promise<void>) => {
  try { await f(); } catch (e) { check(false, `${name}: ${String(e).slice(0, 300)}`); await a.shot(`sync-fail-${name}`); }
};
const talks = (pred: (t: string) => boolean) => peer.events.some((m) => m.type === "log" && m.task === 0 && m.entry?.kind === "talk" && pred(m.entry.text));
try {
  await step("settings-in", async () => {
    await a.waitFor(() => st().model === "claude-haiku-5-5", 10_000, "app shows the Haiku the peer set");
    check(true, `the app shows the model the peer set (${st().model}) — instance ${st().instance}`);
  });
  await step("send-from-app", async () => {
    const k = peer.events.length;
    await a.click("composer.input");
    await a.type("Reply with exactly: pong-gpui-1");
    await a.key("Return");
    await peer.waitFor(() => peer.since(k, (m) => m.type === "delta" && m.task === 0).length > 0, 120_000, "the peer sees the reply stream");
    let sawLive = false;
    await a.waitFor(() => { if (st().running) sawLive = true; return !st().running && (st().lastTalk ?? "").includes("pong-gpui-1"); }, 120_000, "the app shows the reply");
    await peer.waitFor(() => talks((t) => t.includes("pong-gpui-1")), 30_000, "the peer gets the talk entry");
    const userInPeer = peer.since(k, (m) => m.type === "log" && m.entry?.kind === "user" && m.entry.text.includes("pong-gpui-1")).length;
    check(userInPeer === 1 && sawLive, `sent from the app: the peer saw the message, the streamed deltas and the reply; the app saw the turn run`);
  });
  await step("send-from-peer", async () => {
    let sawRunning = false;
    peer.send({ type: "send", task: 0, text: "Reply with exactly: pong-peer-2" });
    await a.waitFor(() => { if (st().running) sawRunning = true; return !st().running && (st().lastTalk ?? "").includes("pong-peer-2"); }, 120_000, "the app shows the peer's turn");
    check(sawRunning && st().lastKinds.includes("user"), `sent from the peer: the app showed the turn running and the reply (${st().lastTalk})`);
    await a.shot("sync-01-both-turns");
  });
  let child = -1;
  await step("spawn-from-peer", async () => {
    const r = await peer.command({ type: "spawn", task: 0, mode: "handoff", title: "sync child", instruction: "Reply with exactly: ok", start: false });
    child = r.spawned;
    await a.waitFor(() => st().tasks > 1 && a.state().tasks >= 2, 10_000, "the app's tree has the child");
    check(r.ok && typeof child === "number", `spawned from the peer: #${child} (${r.message}); the app's tree has ${st().tasks} tasks`);
  });
  await step("pause-resume-from-app", async () => {
    // pause the main chat from the app's tab action; the peer sees the task's status
    const k = peer.events.length;
    await a.click("act.pause");
    await a.waitFor(() => st().ask === "pause", 4000, "ask bar");
    await a.type("sync check");
    await a.key("Return");
    await peer.waitFor(() => peer.since(k, (m) => m.type === "task" && m.task?.id === 0 && m.task.status === "paused").length > 0, 15_000, "the peer sees the pause");
    await a.waitFor(() => st().status === "paused", 10_000, "app paused");
    const k2 = peer.events.length;
    await a.click("act.resume");
    await a.key("Return");
    await peer.waitFor(() => peer.since(k2, (m) => m.type === "task" && m.task?.id === 0 && m.task.status !== "paused").length > 0, 15_000, "the peer sees the resume");
    check(true, "pause and resume from the app reach the peer (task events)");
    await a.waitFor(() => !st().running, 120_000, "the resumed turn ends");
  });
  await step("settings-from-app", async () => {
    const k = peer.events.length;
    await a.palette("Effort: medium");
    await peer.waitFor(() => peer.since(k, (m) => (m.type === "task" && m.task?.id === 0 && m.task.settings?.effort === "medium") || (m.type === "settings" && m.settings?.effort === "medium")).length > 0, 15_000, "the peer sees the effort");
    check(true, "an effort change from the app reaches the peer");
    peer.send({ type: "settings", task: 0, patch: { effort: "low" } });
    await a.waitFor(() => st().effort === "low", 10_000, "the app sees the peer's change back");
    check(true, "and the peer's change back shows in the app");
  });
  await step("rate-from-app", async () => {
    await a.key("cmd+5");
    await a.waitFor(() => st().view === "models", 4000, "models");
    await sleep(1500);
    await a.click("model.edit.claude-haiku-5-5", { wait: 10_000 });
    const r = await a.waitProbe("rate.0");
    await a.clickAt(r[0] + r[2] / 2, r[1] + r[3] - 10);
    await a.key("cmd+a");
    await a.type("0.37");
    await a.reveal("rate.save", "sidebar");
    await a.click("rate.save");
    let rows: any = null;
    for (let k = 0; k < 40 && !rows; k++) {
      const list = await peer.rpc("models.list");
      const h = list.find((m: any) => m.id === "claude-haiku-5-5");
      if (h?.rate?.per1m?.in === 0.37) rows = h; else await sleep(500);
    }
    check(!!rows, `a rate set in the app's Models view is the hub's rate for the peer (in $0.37/1M)`);
    await a.key("cmd+1");
  });
  await step("reconnect", async () => {
    const r0 = st().reconnects;
    await a.palette("Test: drop the hub connection");
    await a.waitFor(() => st().reconnects > r0 && st().conn === "open" && st().loaded, 30_000, "reconnected");
    peer.send({ type: "send", task: 0, text: "Reply with exactly: pong-after-reconnect" });
    await a.waitFor(() => (st().lastTalk ?? "").includes("pong-after-reconnect"), 120_000, "in sync after reconnecting");
    check(true, `the app reconnects after its connection drops (${st().reconnects} reconnect) and stays in sync`);
    await a.shot("sync-02-after-reconnect");
  });
} catch (e) { check(false, String(e)); await a.shot("sync-fail"); } finally { peer.close(); await a.quit(); }
Deno.exit(failed() ? 1 : 0);
