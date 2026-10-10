// Phase 4 (spec-workspaces §4) without input: the same steps as tests/workspaces.ts, run through the app's own
// handlers by the test channel (crate::testcmd) and checked on its state file. For runs where nothing can reach the
// window (a locked screen); tests/workspaces.ts does it with Computer Use and screenshots. Test workspaces only
// (test-gpui-g / test-gpui-h; tokens from data/test-tokens.json, never printed). Haiku only (the workspaces' default).
import { App, check, failed } from "./lib/app.ts";
import { ROOT, sleep } from "./lib/cua.ts";
import { HUB, Peer } from "./lib/hub.ts";

const env = { OPTCHAT_TEST_HOOKS: "1" };
let a = await App.launch({ bare: true, headless: true, env });
const st = () => a.state();
const ws = () => st()?.workspace;
const conn = (w: string) => (st()?.connected ?? []).find((c: any) => c.ws === w);
const unreadOf = (w: string) => conn(w)?.unread ?? {};
let dir = a.dir;
let peer: Peer | null = null;

try {
  check(st().ready === false && (st().connected ?? []).length === 0, "a fresh app: no engine, no workspace");
  await a.cmd({ op: "addEndpoint", endpoint: HUB.replace(/^https:\/\//, ""), name: "reframe.dev" });
  await a.waitFor(() => st().engines?.[0]?.online === true && st().engines[0].workspaces.length > 0, 12000, "engine listed");
  const listed = (st().engines[0].workspaces as any[]).map((w) => [w.name, w.running] as [string, boolean]);
  check(listed.some(([n, r]) => n === "test-gpui-g" && r) && listed.some(([n, r]) => n === "test-gpui-h" && r), `Add engine: online, ${listed.length} workspaces listed (${listed.filter(([, r]) => !r).length} disabled)`);
  check(st().view === "workspaces" && st().sideOpen && st().overlay === null, "adding happens in the Workspaces panel (no popup)");

  await a.cmd({ op: "connect", ws: "test-gpui-g" });
  await a.waitFor(() => ws() === "test-gpui-g" && st().loaded && st().ready, 20000, "a on screen");
  check(st().tasks > 50 && conn("test-gpui-g").conn === "open", `Connect test-gpui-g (token from the test file into the token store): ${st().tasks} tasks`);
  await a.cmd({ op: "connect", ws: "test-gpui-h" });
  await a.waitFor(() => ws() === "test-gpui-h" && st().loaded, 20000, "b on screen");
  check(conn("test-gpui-g")?.conn === "open", "Connect test-gpui-h: it comes on screen, test-gpui-g stays subscribed in the background");

  // switching: a swap of state
  const n0 = (st().wsSwitches ?? []).length;
  for (const w of ["test-gpui-g", "test-gpui-h", "test-gpui-g", "test-gpui-h"]) {
    await a.cmd({ op: "switch", ws: w });
    await a.waitFor(() => ws() === w && st().loaded, 2000, `switch to ${w}`);
  }
  const ms = (st().wsSwitches ?? []).slice(n0) as number[];
  check(ms.length === 4 && Math.max(...ms) < 16, `4 switches, loaded at once: ${ms.map((m) => m.toFixed(3)).join(", ")} ms`);
  const tabsB = st().tabs;

  // a Haiku turn in b while a is on screen: a child with a tiny budget pauses with a warning
  const t0 = st().current;
  await a.cmd({ op: "spawn", title: "say hi (budget test)", instruction: "Reply with exactly one word: hi", usd: 0.0001 });
  await a.waitFor(() => st().current !== t0, 10000, "child opened in b");
  const child = st().current;
  check(st().tabs.includes(child), `spawned #${child} in test-gpui-h (Haiku low, budget $0.0001)`);
  await a.cmd({ op: "switch", ws: "test-gpui-g" });
  await a.waitFor(() => ws() === "test-gpui-g", 2000, "a on screen");
  const w0 = (unreadOf("test-gpui-h").warn ?? 0) + (unreadOf("test-gpui-h").error ?? 0);
  // a new warning (earlier runs may have left unread ones)
  await a.waitFor(() => st().bgAlert && (unreadOf("test-gpui-h").warn ?? 0) + (unreadOf("test-gpui-h").error ?? 0) > w0, 180000, "background warning");
  check(true, `b's #${child} warned while a was on screen: the workspace icon's dot is on (b unread ${JSON.stringify(unreadOf("test-gpui-h"))}, was ${w0})`);

  // on screen, the notices: list from the engine, read one (opens its task), read all
  await a.cmd({ op: "switch", ws: "test-gpui-h" });
  // on screen, d's notices are in the status bar; the icon dot is only for the others (c may have its own unread ones)
  await a.waitFor(() => ws() === "test-gpui-h", 2000, "b on screen");
  const cAlert = (unreadOf("test-gpui-g").warn ?? 0) + (unreadOf("test-gpui-g").error ?? 0) > 0;
  check(st().bgAlert === cAlert, `the icon dot follows the background workspace only (c unread ${JSON.stringify(unreadOf("test-gpui-g"))}, dot ${st().bgAlert})`);
  check(JSON.stringify(st().tabs) !== "[]" && st().tabs.includes(child) && tabsB.every((t: number) => st().tabs.includes(t)), "b kept its tabs (per-workspace tabs)");
  await a.cmd({ op: "view", view: "notifications" }, { op: "loadNotices" });
  await a.waitFor(() => st().noticesLoaded && (st().noticesTop ?? []).length > 0, 8000, "notices loaded");
  const warn = (st().noticesTop as any[]).find((n) => (n.level === "warn" || n.level === "error") && n.task === child && !n.read);
  if (!warn) console.log("noticesTop:", JSON.stringify((st().noticesTop as any[]).slice(0, 6).map((n) => [n.level, n.task, n.read, n.text.slice(0, 60)])));
  check(!!warn, `the Notifications view: "${warn?.text}"`);
  const before = st().unread.warn + st().unread.error;
  await a.cmd({ op: "readNotice", id: warn.id });
  await a.waitFor(() => st().unread.warn + st().unread.error < before && st().current === child, 8000, "read + opened");
  check(true, `a click opens #${child} and marks it read on the engine (${before} → ${st().unread.warn + st().unread.error})`);
  // another client sees the read state (it is on the engine)
  peer = await Peer.connect("test-gpui-h");
  const list = await peer.rpc("notices", { limit: 50 });
  check(list.some((n: any) => n.id === warn.id && n.readAt), "another client sees it read (read state is on the engine)");
  await a.cmd({ op: "readAll" });
  await a.waitFor(() => st().unread.ok + st().unread.info + st().unread.warn + st().unread.error === 0, 8000, "all read");
  check(true, "Mark all read: no unread notices (status bar dots off)");
  // the child: leave it paused, budget cleared (the workspace is shared by test runs)
  await peer.command({ type: "control", task: child, op: "budget", budget: {} }).catch(() => {});

  // relaunch: connections and the workspace on screen come back (tokens from the store)
  await a.quit({ keep: true });
  a = await App.launch({ bare: true, headless: true, dir, env });
  await a.waitFor(() => (st().connected ?? []).length === 2 && ws() === "test-gpui-h" && st().loaded, 20000, "relaunch restores");
  check(true, "relaunch: both connections back (tokens from the store), test-gpui-h on screen");
  const prefs = Deno.readTextFileSync(`${dir}/prefs.json`);
  const toks = Object.values(JSON.parse(Deno.readTextFileSync(`${ROOT}/data/test-tokens.json`))) as string[];
  check(!toks.some((t) => prefs.includes(t)) && !toks.some((t) => a.stderr().includes(t)), "no token in the prefs or the log");

  await a.cmd({ op: "disconnect", ws: "test-gpui-g" });
  await a.waitFor(() => !conn("test-gpui-g"), 4000, "a disconnected");
  check(ws() === "test-gpui-h" && st().engines[0].workspaces.some((w: any) => w.name === "test-gpui-g"), "Disconnect test-gpui-g: gone from the app (available again), b stays");
  await a.cmd({ op: "disconnect", ws: "test-gpui-h" });
  await a.waitFor(() => (st().connected ?? []).length === 0 && st().ready === false, 4000, "none");
  check(true, "Disconnect the last one: the empty state");
  await a.quit();
} catch (e) {
  check(false, String(e).slice(0, 400));
  await a.quit().catch(() => {});
} finally {
  peer?.close();
  try { Deno.removeSync(dir, { recursive: true }); } catch { /* gone */ }
}

// the engine goes away and comes back: a local relay this test starts and kills (the real engine stays up)
const PORT = 18787;
const relay = () => new Deno.Command("deno", { args: ["run", "-A", `${ROOT}/tests/lib/relay.ts`, String(PORT)], env: { DENO_TLS_CA_STORE: "system" }, stdout: "null", stderr: "null" }).spawn();
let r = relay();
await sleep(1500);
try {
  a = await App.launch({ db: "test-gpui-g", headless: true, env: { ...env, OPTCHAT_ENGINE: `http://127.0.0.1:${PORT}`, OPTCHAT_ENGINE_NAME: "local relay" } });
  dir = a.dir;
  check(st().engines?.[0]?.online === true && st().conn === "open", "through the local relay: connected");
  for (let round = 1; round <= 2; round++) {
    r.kill("SIGKILL");
    await r.status;
    const t0 = Date.now();
    await a.waitFor(() => st().engines?.[0]?.online === false && st().conn !== "open", 5000, "offline within 5 s");
    check(true, `round ${round}: relay killed, engine offline after ${Date.now() - t0} ms`);
    await sleep(3000);
    r = relay();
    const t1 = Date.now();
    await a.waitFor(() => st().engines?.[0]?.online === true, 10000, "back online");
    const on = Date.now() - t1;
    await a.waitFor(() => st().conn === "open" && st().loaded, 15000, "resnapshotted");
    check(on < 6000, `round ${round}: relay back, engine online after ${on} ms (backoff ≤ 4 s), workspace resnapshotted after ${Date.now() - t1} ms`);
  }
  await a.quit();
} catch (e) {
  check(false, String(e).slice(0, 400));
  await a.quit().catch(() => {});
} finally {
  try { r.kill("SIGKILL"); } catch { /* gone */ }
  try { Deno.removeSync(dir, { recursive: true }); } catch { /* gone */ }
}
Deno.exit(failed() ? 1 : 0);
