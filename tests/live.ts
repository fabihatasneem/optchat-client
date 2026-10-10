// PARITY items that need model turns, all on Claude Haiku children of the copied chat (root switched to Haiku first):
// ⌘Enter in the new-task dialog, live text + cursor, rail badge + memory dot, status texts (running, settling,
// waiting for a slot, #id mode, N working, N queued), "waiting for summaries", Context "still running", compaction
// jobs + memory rows refreshing, follow mode while streaming, ⌘. stop, Enter / ⌘Enter while dictating (fake mic).
import { Peer } from "./lib/hub.ts";
import { App, check, failed } from "./lib/app.ts";
import { ROOT, sleep } from "./lib/cua.ts";
const a = await App.launch({ env: { OPTCHAT_TEST_HOOKS: "1", OPTCHAT_FAKE_MIC: `${ROOT}/tests/fixtures/dict.wav` } });
const st = () => a.state();
const pal = (q: string) => a.palette(q);
const keys = (pre: string) => Object.keys(a.allProbes()).filter((k) => k.startsWith(pre));
const peer = await Peer.connect(a.db, 0); // a second client of the hub, for what the app doesn't show
const step = async (name: string, f: () => Promise<void>) => {
  try { await f(); } catch (e) { check(false, `${name}: ${String(e).slice(0, 400)}`); await a.shot(`live-fail-${name}`); await a.key("Escape"); await sleep(300); }
};
const idle = (ms = 180000) => a.waitFor(() => st().loaded && !st().running && !(st().ws?.waiting ?? []).includes(st().current), ms, "turn done");
const send = async (text: string, k = "Return") => {
  await a.click("composer.input");
  await a.type(text);
  await a.key(k);
};
const spawn = async (title: string, instruction: string) => {
  await a.key("cmd+n");
  await a.waitFor(() => st().overlay === "newtask", 5000, "dialog");
  await a.type(title);
  await a.click("newtask.instruction");
  await a.type(instruction);
  await a.click("newtask.start");
  await a.waitFor(() => st().overlay === null && st().current !== 0, 10000, "spawned");
  return st().current as number;
};
let child = 0;
try {
  await pal("Model: Claude Haiku");
  await a.waitFor(() => st().model === "claude-haiku-5-5", 8000, "haiku");

  await step("newtask-cmd-enter", async () => {
    await a.key("cmd+n");
    await a.waitFor(() => st().overlay === "newtask", 5000, "dialog");
    await a.key("cmd+Return");
    await sleep(600);
    check(st().overlay === "newtask" && !st().newtask.ok, "⌘Enter does nothing while the dialog is not valid");
    await a.type("say live one");
    await a.click("newtask.instruction");
    await a.type("Reply with exactly: live-1");
    await a.key("cmd+Return");
    await a.waitFor(() => st().overlay === null && st().current !== 0, 10000, "spawned");
    child = st().current;
    if (!child) throw new Error("no child: stopping (never run model turns on the root)");
    check(st().tabs.includes(child), `⌘Enter starts the task (#${child} opened)`);
    // while the first turn runs: rail badge, status texts, live text, Context "still running"
    await a.key("cmd+3");
    await a.waitFor(() => st().view === "context", 3000, "context");
    await a.waitFor(() => (st().contextSeen ?? []).includes("stillRunning"), 60000, "still running").catch(() => {});
    await a.shot("live-01-context-running");
    check((st().contextSeen ?? []).includes("stillRunning"), "the Context view shows \"still running\" for the call in flight");
    await idle();
    check(/live-1/.test(st().lastTalk ?? ""), `#${child} replied: ${st().lastTalk}`);
    check((st().railSeen ?? []).includes("badge"), "the Tasks rail button showed a badge with the active count");
    check(st().statusbar?.mode === `#${child} handoff`, `status bar: ${st().statusbar?.mode}`);
    const seen = st().statusSeen ?? [];
    check(seen.includes("running"), `status texts seen: ${JSON.stringify(seen)}`);
  });

  if (!child) throw new Error("no child task: the rest needs one");
  await step("follow-stop", async () => {
    await send("Write 200 numbered sentences, one per line, each about a different country or city. Nothing else.");
    await a.waitFor(() => st().running && (st().feed?.liveNow ?? 0) > 80, 60000, "streaming");
    await a.shot("live-03-streaming");
    check((st().liveSeen ?? []).includes("text") && (st().liveSeen ?? []).includes("cursor"), `live text streamed with a cursor (${st().feed.liveText} chars; thinking ${st().feed.liveThinking} chars; seen ${JSON.stringify(st().liveSeen)})`);

    // follow mode while streaming: scrolled up stays put
    await a.scroll("feed", "up", 2);
    await a.waitFor(() => st().feed.scrolledUp, 3000, "scrolled up");
    const top = JSON.stringify(st().feed.top), len = st().feed.liveNow;
    await sleep(1500);
    check(JSON.stringify(st().feed.top) === top && st().feed.liveNow > len, `scrolled up while text streams in (${len} → ${st().feed.liveNow} chars): the view stays put at ${top}`);
    await a.click("feed.jump");
    await a.waitFor(() => !st().feed.scrolledUp, 3000, "following");
    check(st().running, "still streaming when ⌘. is pressed");
    await a.key("cmd+period");
    await a.waitFor(() => !st().running, 20000, "stopped");
    await sleep(1000);
    await a.shot("live-04-stopped");
    const n = (await peer.rpc("turns.recent", { task: child, limit: 20 })).filter((t: any) => t.status === "cancelled").length;
    check(Number(n) >= 1, `⌘. stops the running turn (${n} cancelled turn in #${child})`);
  });

  await step("dictation-enter", async () => {
    await idle();
    const t0 = st().t;
    await a.click("composer.input");
    await a.click("composer.mic");
    await a.waitFor(() => st().dict === "listening", 15000, "listening");
    await a.waitFor(() => /dictation/i.test(st().draft), 20000, "dictated text");
    await a.key("Return");
    await a.waitFor(() => st().t > t0 && st().draft === "", 20000, "sent after the final text");
    const tail = await peer.rpc("log.range", { task: child, from: Math.max(0, st().t - 20), to: st().t, full: true });
    const last: string = [...tail].reverse().find((e: any) => e.kind === "user")?.text ?? "";
    const final = (st().ws?.dictText ?? "").trim();
    check(/hello from the dictation test/i.test(last) && last.trim() === final && st().dict === "idle", `Enter while dictating: stopped, waited for the final text, then sent and ran it ("${last.slice(0, 80)}")`);
    await idle();
    // ⌘Enter while dictating: stop, then ↑ (append, no turn)
    const t1 = st().t;
    await a.click("composer.input");
    await a.click("composer.mic");
    await a.waitFor(() => st().dict === "listening", 15000, "listening 2");
    await a.waitFor(() => /dictation/i.test(st().draft), 20000, "dictated text 2");
    await a.key("cmd+Return");
    await a.waitFor(() => st().t > t1 && st().draft === "", 20000, "appended");
    await sleep(2500);
    check(!st().running && st().dict === "idle", `⌘Enter while dictating: stopped, then appended without a turn (running ${st().running})`);
  });

  await step("settling", async () => {
    await idle();
    // a long message is a leaf that needs a summary: the next turn waits for it ("settling")
    for (let i = 0; i < 2; i++) {
      await a.click("composer.input");
      await a.type(`Context note ${i}, no reply needed: ${"the quick brown fox jumps over the lazy dog and keeps running across the field. ".repeat(24)}`);
      await a.key("cmd+Return");
    }
    // run at once (→ runs what was appended): its summary is still being written
    await a.waitFor(() => !!a.probe("composer.run") && st().unrun !== null, 5000, "appended");
    await a.click("composer.run");
    await a.waitFor(() => (st().statusSeen ?? []).includes("settling") && (st().waitingSeen ?? []).includes("summaries"), 30000, "settling").catch(() => {});
    await a.shot("live-05-settling");
    check((st().statusSeen ?? []).includes("settling") && (st().waitingSeen ?? []).includes("summaries"), `the turn waits for the view to be built: status "settling", feed "${st().waitingSummaries}"`);
    // the memory: jobs and the dot while it summarizes
    await a.key("cmd+2");
    await a.waitFor(() => st().view === "memory", 3000, "memory");
    await idle();
    check((st().railSeen ?? []).includes("memDot"), "the Memory rail button showed its dot while compacting");
  });

  await step("memory-jobs", async () => {
    // more long notes: compaction jobs run in this timeline; rows fill in as nodes get built
    await a.waitFor(() => st().view === "memory" && st().sideOpen, 3000, "memory");
    const built0 = st().memSummary?.builtRows ?? 0, nodes0 = st().memSummary?.nodes ?? 0;
    for (let i = 0; i < 3; i++) {
      await a.click("composer.input");
      await a.type(`Another note ${i}, no reply needed: ${"summaries are built in the background as the view grows and merges. ".repeat(12)}`);
      await a.key("cmd+Return");
      await sleep(400);
    }
    await a.waitFor(() => st().memSummary?.running > 0 && keys("job.").length > 0, 20000, "running job").catch(() => {});
    await a.shot("live-06-memory-jobs");
    const running = st().memSummary?.running ?? 0;
    const job = keys("job.")[0];
    check(running > 0 && !!job, `compaction jobs running in this timeline (${running}; ${job})`);
    if (job) {
      await a.click(job);
      await a.waitFor(() => !!st().memory?.hit, 3000, "revealed").catch(() => {});
      check(!!st().memory?.hit, `a click on a running job reveals its node (${st().memory?.hit})`);
    }
    await a.waitFor(() => (st().memSummary?.nodes ?? 0) > nodes0 && (st().memSummary?.builtRows ?? 0) > built0, 120000, "rows refreshed").catch(() => {});
    await a.shot("live-07-memory-built");
    check((st().memSummary?.builtRows ?? 0) > built0, `memory rows refresh as nodes get built (built rows ${built0} → ${st().memSummary?.builtRows}, nodes ${nodes0} → ${st().memSummary?.nodes})`);
  });

  await step("slot", async () => {
    await a.key("cmd+6");
    await a.waitFor(() => st().view === "settings", 3000, "settings");
    await a.click("settings.global");
    await a.waitFor(() => st().settingsGlobal === true, 3000, "global");
    const p = await a.waitProbe("picker.maxturns");
    await a.clickAt(p[0] + 20, p[1] + p[3] / 2);
    await sleep(500);
    await a.key("Down");
    await a.key("Return");
    await a.waitFor(() => st().ws?.maxTurns === 1, 5000, "one turn at once");
    check(true, "Settings: turns at once = 1");
    await a.key("cmd+0");
    await a.waitFor(() => st().current === 0, 5000, "main");
    const b = await spawn("write sentences", "Write 40 numbered sentences, one per line, each about a different city. Nothing else.");
    await a.key("cmd+0");
    await a.waitFor(() => st().current === 0, 5000, "main");
    const c = await spawn("slot", "Reply with exactly: slot-ok");
    await a.waitFor(() => (st().statusSeen ?? []).includes("waiting for a slot"), 30000, "waiting for a slot").catch(() => {});
    const working = st().statusbar?.working ?? 0;
    // a message to the waiting task waits in its inbox: "1 queued"
    await send("Also add: queued-ok");
    await a.waitFor(() => (st().queuedSeen ?? []).length > 0, 8000, "queued").catch(() => {});
    await a.shot("live-02-queued");
    check((st().queuedSeen ?? []).length > 0, `a message to a task that waits for its turn shows ${JSON.stringify(st().queuedSeen)}`);
    await a.shot("live-08-slot");
    check((st().statusSeen ?? []).includes("waiting for a slot") && (st().ws?.waiting ?? []).includes(c), `#${c} waits while #${b} runs: status "waiting for a slot"`);
    check(working >= 1, `status bar: ${working} working`);
    await a.waitFor(() => /slot-ok/.test(st().lastTalk ?? ""), 240000, "slot reply");
    check(true, `#${c} ran once the slot was free`);
    // back to 4
    await a.key("cmd+6");
    await a.waitFor(() => st().view === "settings", 3000, "settings");
    const p2 = await a.waitProbe("picker.maxturns");
    await a.clickAt(p2[0] + 20, p2[1] + p2[3] / 2);
    await sleep(500);
    for (let i = 0; i < 4; i++) await a.key("Down");
    await a.key("Return");
    await a.waitFor(() => st().ws?.maxTurns === 4, 5000, "four").catch(() => {});
  });
} catch (e) { check(false, String(e).slice(0, 400)); await a.shot("live-fail"); } finally { peer.close(); await a.quit(); }
Deno.exit(failed() ? 1 : 0);
