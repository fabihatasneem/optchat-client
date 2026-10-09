// Release-build measurements on the 1749-message main chat:
// - launch: first frame, chat loaded (in-app clocks)
// - scrolling: a continuous in-app gesture (test hook: the feed's list scrolled every 8.3 ms, direction flipping every
//   second; posting OS scroll events would need an Accessibility grant) -> presented frame intervals and
//   main-thread work per frame
// - task switch: in-app time from opening a task to the first render with its data (excludes Computer Use latency)
// - memory (RSS)
import { App, SHOTS } from "./lib/app.ts";
import { sleep } from "./lib/cua.ts";
const t0 = performance.now();
const a = await App.launch({ env: { OPTCHAT_TEST_HOOKS: "1", OPTCHAT_TIMING: "1" } });
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
  // task switching: #92 (128 msgs) and back to the main chat (1749), twice
  const before = (st().switches ?? []).length;
  for (const id of [92, 0, 92, 0]) {
    const n = (st().switches ?? []).length;
    await a.switchTo(id);
    await a.waitFor(() => (st().switches ?? []).length > n, 10000, `switch ${id} recorded`);
    await sleep(300);
  }
  out.switchMs = (st().switches ?? []).slice(before).map((x: number) => Math.round(x * 10) / 10);
  out.timing = a.stderr().split("\n").filter((l) => /^(snapshot|open) #/.test(l));
} catch (e) { await a.shot("perf-fail"); throw e; } finally { await a.quit(); }
console.log(JSON.stringify(out, null, 2));
Deno.writeTextFileSync(`${SHOTS}/perf.json`, JSON.stringify(out, null, 2));
