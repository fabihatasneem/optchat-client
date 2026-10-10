// Perf probes on a fixed, offline fixture (tests/lib/fake-engine.ts --big: a 1,770-entry main chat of markdown answers
// + five child tasks), so runs compare across builds with no real workspace: launch (first frame, chat loaded), an
// in-app scroll gesture (main-thread work per frame), task switches (open → first render with the task's data), RSS.
// Headless (OPTCHAT_E2E_HEADLESS=1) the app lays frames out itself every 33 ms: frame *intervals* are then the test's
// clock, but the work per frame is the app's own. Writes shots/perf-fake.json (or $PERF_OUT).
import { App, SHOTS } from "./lib/app.ts";
import { ROOT, sleep } from "./lib/cua.ts";

const PORT = 18791;
const eng = new Deno.Command("deno", { args: ["run", "-A", `${ROOT}/tests/lib/fake-engine.ts`, String(PORT), "perf", "--big"], stdout: "null", stderr: "null" }).spawn();
await sleep(1200);
const dir = await Deno.makeTempDir({ prefix: "optchat-e2e-" });
Deno.writeTextFileSync(`${dir}/fake-tokens.json`, JSON.stringify({ perf: "fake-token" }));
const t0 = performance.now();
const a = await App.launch({ bare: true, dir, env: { OPTCHAT_TEST_HOOKS: "1", OPTCHAT_ENGINE: `http://127.0.0.1:${PORT}`, OPTCHAT_ENGINE_NAME: "fake", OPTCHAT_CONNECT: "perf", OPTCHAT_TEST_TOKENS: `${dir}/fake-tokens.json` } });
const st = () => a.state();
const out: Record<string, unknown> = {};
const pct = (v: number[], q: number) => { const x = [...v].sort((p, r) => p - r); return Math.round(x[Math.min(x.length - 1, Math.floor((x.length - 1) * q))] * 100) / 100; };
try {
  await a.waitFor(() => st().ready && st().loaded, 20000, "loaded");
  out.launch = { firstFrameMs: Math.round(st().firstFrameMs), loadedMs: Math.round(st().loadedMs), wallToReadyMs: Math.round(performance.now() - t0), messages: st().t };
  await sleep(1500);
  out.rssIdleMb = st().rss;
  const scroll = async (hook: string, ms: number) => {
    await a.palette("Test: reset frame stats");
    await a.palette(hook);
    await sleep(ms);
    return st().frames;
  };
  out.scroll = await scroll("Test: scroll gesture", 6600);
  out.scrollFast = await scroll("Test: fast scroll gesture", 4600);
  out.rssAfterScrollMb = st().rss;
  const order = [1, 0, 2, 0, 3, 0, 4, 0, 5, 0, 1, 0];
  const before = (st().switches ?? []).length;
  for (const id of order) {
    const n = (st().switches ?? []).length;
    await a.switchTo(id);
    await a.waitFor(() => (st().switches ?? []).length > n, 10000, `switch ${id}`);
    await sleep(250);
  }
  const shown: number[] = (st().switches ?? []).slice(before);
  out.switch = { order, shownMs: shown.map((x) => Math.round(x * 10) / 10), p50: pct(shown, 0.5), p90: pct(shown, 0.9), max: Math.max(...shown) };
  out.rssEndMb = st().rss;
} finally {
  await a.quit();
  eng.kill("SIGKILL");
  try { Deno.removeSync(dir, { recursive: true }); } catch { /* gone */ }
}
console.log(JSON.stringify(out, null, 2));
Deno.writeTextFileSync(Deno.env.get("PERF_OUT") ?? `${SHOTS}/perf-fake.json`, JSON.stringify(out, null, 2));
