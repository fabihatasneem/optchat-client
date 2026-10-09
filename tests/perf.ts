// Release-build measurements: launch to first frame / loaded chat, scrolling the 1749-message main chat (frame
// intervals while scrolling), task switch latency, memory.
import { App, SHOTS } from "./lib/app.ts";
import { sleep } from "./lib/cua.ts";
const t0 = performance.now();
const a = await App.launch({ env: { OPTCHAT_TEST_HOOKS: "1" } });
const wall = performance.now() - t0;
const st = () => a.state();
const out: Record<string, unknown> = {};
try {
  out.launch = { firstFrameMs: st().firstFrameMs, loadedMs: st().loadedMs, wallToReadyMs: Math.round(wall), messages: st().t };
  await sleep(1500);
  out.rssIdleMb = st().rss;
  await a.palette("Test: reset frame stats");
  for (let k = 0; k < 12; k++) await a.scroll("feed", "up", 5);
  for (let k = 0; k < 12; k++) await a.scroll("feed", "down", 5);
  await sleep(500);
  out.scroll = st().frames;
  out.rssAfterScrollMb = st().rss;
  await a.shot("perf-scrolled");
  // task switching: open #92 (128 msgs) and back to the main chat (1749)
  const sw: number[] = [];
  for (const id of [92, 0, 92, 0]) {
    const t1 = performance.now();
    await a.key("cmd+t"); await a.type(`#${id} `); await a.key("Return");
    await a.waitFor(() => st().current === id && st().loaded, 10000, `open ${id}`);
    sw.push(Math.round(performance.now() - t1));
  }
  out.switchMs = sw;
} catch (e) { await a.shot("perf-fail"); throw e; } finally { await a.quit(); }
console.log(JSON.stringify(out, null, 2));
Deno.writeTextFileSync(`${SHOTS}/perf.json`, JSON.stringify(out, null, 2));
