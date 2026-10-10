// Release-build measurements, the app as a client of the hub on the live-sized test instance (main chat ~1,770 log
// entries; the snapshot carries the last 120, older ones load on demand):
// - launch: first frame, chat loaded (in-app clocks)
// - scrolling: a continuous in-app gesture (test hook: the feed's list scrolled every 8.3 ms, direction flipping every
//   second; posting OS scroll events would need an Accessibility grant) -> presented frame intervals and
//   main-thread work per frame
// - task switch: in-app time from opening a task to the first render with its data (excludes Computer Use latency)
// - memory (RSS)
import { App, SHOTS, testDb } from "./lib/app.ts";
import { sleep } from "./lib/cua.ts";
const t0 = performance.now();
const a = await App.launch({ db: testDb("tmv23x72y"), env: { OPTCHAT_TEST_HOOKS: "1", OPTCHAT_TIMING: "1" } });
const wall = performance.now() - t0;
const st = () => a.state();
const out: Record<string, unknown> = {};
try {
  out.launch = { firstFrameMs: st().firstFrameMs, loadedMs: st().loadedMs, wallToReadyMs: Math.round(wall), messages: st().t };
  await sleep(1500);
  out.rssIdleMb = st().rss;
  await a.palette("Test: reset frame stats");
  await a.palette("Test: scroll gesture");
  out.gesture = "in-app gesture: 6 s at 120 Hz, 14 px per step (840 px/s), direction flips every second";
  await sleep(6600);
  out.scroll = st().frames;
  out.rssAfterScrollMb = st().rss;
  await a.shot("perf-scrolled");
  await a.palette("Test: reset frame stats");
  await a.palette("Test: fast scroll gesture");
  out.gestureFast = "in-app gesture: 4 s at 120 Hz, 60 px per step (7200 px/s)";
  await sleep(4600);
  out.scrollFast = st().frames;
  // task switching, measured in the app: from the open to the first render of the hub's snapshot ("shown"), and to
  // its turns + requests (two rpcs after the snapshot: the foot chips) ("full")
  const order = [92, 0, 8, 0, 3, 0, 105, 0, 1, 0, 92, 0];
  const before = (st().switches ?? []).length, beforeFull = (st().switchesFull ?? []).length;
  for (const id of order) {
    const n = (st().switches ?? []).length, nf = (st().switchesFull ?? []).length;
    await a.switchTo(id);
    await a.waitFor(() => (st().switches ?? []).length > n && (st().switchesFull ?? []).length > nf, 15000, `switch ${id} recorded`);
    await sleep(300);
  }
  const shown: number[] = (st().switches ?? []).slice(before), full: number[] = (st().switchesFull ?? []).slice(beforeFull);
  const pct = (v: number[], q: number) => { const x = [...v].sort((p, r) => p - r); return Math.round(x[Math.min(x.length - 1, Math.floor((x.length - 1) * q))] * 10) / 10; };
  const sel = (v: number[], main: boolean) => v.filter((_, k) => (order[k] === 0) === main);
  out.switch = {
    order,
    shownMs: shown.map((x) => Math.round(x * 10) / 10),
    fullMs: full.map((x) => Math.round(x * 10) / 10),
    all: { shownP50: pct(shown, 0.5), shownP90: pct(shown, 0.9), fullP50: pct(full, 0.5), fullP90: pct(full, 0.9) },
    mainChat: { shownP50: pct(sel(shown, true), 0.5), shownP90: pct(sel(shown, true), 0.9), fullP50: pct(sel(full, true), 0.5), fullP90: pct(sel(full, true), 0.9) },
    otherTasks: { shownP50: pct(sel(shown, false), 0.5), shownP90: pct(sel(shown, false), 0.9), fullP50: pct(sel(full, false), 0.5), fullP90: pct(sel(full, false), 0.9) },
    rpcShareP50: pct(full.map((f, k) => f - shown[k]), 0.5),
  };
  // the in-process engine's numbers (same machine, 1,749-message chat, commit 86e0447), for comparison
  out.inProcess = { firstFrameMs: 200, loadedMs: 850, switchMs: "14–34 (main chat 9–34)", scrollWorkP50: 3.2, scrollWorkP95: 4.7, scrollWorkMax: 5.6, rssMb: 185 };
  out.timing = a.stderr().split("\n").filter((l) => /^open #/.test(l)).slice(-30);
} catch (e) { await a.shot("perf-fail"); throw e; } finally { await a.quit(); }
console.log(JSON.stringify(out, null, 2));
Deno.writeTextFileSync(`${SHOTS}/perf.json`, JSON.stringify(out, null, 2));
