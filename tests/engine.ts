// Engines by token, end to end against the REAL engine (spec-workspaces §6, protocol-tokens-v1), through the app's
// own handlers (test channel, headless): Add engine with only an oc1 token (data/oc1-token: a test token whose scope
// is test-ios; never printed) and no name → the first answering address (on this Mac the local one) and its host as
// the name; the token's workspaces connect by themselves, nothing outside its scope; one Haiku child turn there (a
// spawned task with its own model; no engine-wide or workspace-wide settings change); relaunch restores it from the
// token store; Remove forgets engine, workspaces and token. Then the engine going away and back, through a local
// relay this test starts and kills (the real engine never goes down).
import { App, check, failed } from "./lib/app.ts";
import { ROOT, sleep } from "./lib/cua.ts";

const TOKEN = Deno.readTextFileSync(`${ROOT}/data/oc1-token`).trim();
const WS = "test-ios";
const env = { OPTCHAT_TEST_HOOKS: "1" };
let a = await App.launch({ bare: true, headless: true, env });
const st = () => a.state();
const conn = (w: string) => (st()?.connected ?? []).find((c: any) => c.ws === w);
const dir = a.dir;
try {
  check(st().ready === false && (st().connected ?? []).length === 0, "a fresh app: no engine, no workspace");
  await a.cmd({ op: "addEngine", token: TOKEN, name: "" });
  await a.waitFor(() => conn(WS)?.conn === "open" && st().workspace === WS && st().loaded, 30000, "the token's workspace connected");
  const e = st().engines?.[0];
  check(e?.name === "chat.reframe.dev", `Add engine with only the token: on this Mac the local address wins, named after its host ("${e?.name}")`);
  check((st().connected ?? []).length === 1, `only the token's scope is connected: ${(st().connected as any[]).map((c) => c.ws)}`);
  // one Haiku turn in a child task (its own model; nothing workspace-wide changes)
  const t0 = st().current;
  await a.cmd({ op: "spawn", title: "oc1 smoke", instruction: "Reply with exactly: pong-oc1", model: "claude-haiku-5-5" });
  await a.waitFor(() => st().current !== t0, 15000, "child opened");
  const child = st().current;
  await a.waitFor(() => !st().running && /pong-oc1/.test(st().lastTalk ?? ""), 180000, "Haiku reply");
  check(true, `a Haiku turn in #${child} on ${WS} replied pong-oc1`);
  // relaunch: from the token store
  await a.quit({ keep: true });
  a = await App.launch({ bare: true, headless: true, dir, env });
  await a.waitFor(() => conn(WS)?.conn === "open" && st().loaded, 30000, "relaunch");
  const prefs = Deno.readTextFileSync(`${dir}/prefs.json`), store = Deno.readTextFileSync(`${dir}/tokens.json`);
  check(!prefs.includes(TOKEN) && store.includes("engine|") && !a.stderr().includes(TOKEN), "relaunch: connected again from the token store; the token is not in the prefs or the log");
  await a.cmd({ op: "menu" });
  await sleep(500);
  await a.click("wsp.remove.0");
  await a.waitFor(() => (st().engines ?? []).length === 0 && (st().connected ?? []).length === 0, 5000, "removed");
  check(!Deno.readTextFileSync(`${dir}/tokens.json`).includes("engine|"), "Remove: engine, workspace and token gone");
  await a.quit();
} catch (err) {
  check(false, String(err).slice(0, 400));
  await a.quit().catch(() => {});
} finally {
  try { Deno.removeSync(dir, { recursive: true }); } catch { /* gone */ }
}

// the engine goes away and comes back: a local relay (its address given as the engine's; the token per connect)
const PORT = 18787;
const relay = () => new Deno.Command("deno", { args: ["run", "-A", `${ROOT}/tests/lib/relay.ts`, String(PORT)], env: { DENO_TLS_CA_STORE: "system" }, stdout: "null", stderr: "null" }).spawn();
let r = relay();
await sleep(1500);
const tdir = await Deno.makeTempDir({ prefix: "optchat-e2e-" });
Deno.writeTextFileSync(`${tdir}/tokens.json`, JSON.stringify({ [WS]: TOKEN }), { mode: 0o600 });
try {
  a = await App.launch({ bare: true, headless: true, env: { ...env, OPTCHAT_TEST_TOKENS: `${tdir}/tokens.json` } });
  await a.cmd({ op: "addEndpoint", endpoint: `http://127.0.0.1:${PORT}`, name: "local relay" }, { op: "connect", ws: WS, endpoint: `http://127.0.0.1:${PORT}` });
  await a.waitFor(() => conn(WS)?.conn === "open" && st().loaded, 30000, "through the relay");
  check(true, "through the local relay: connected");
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
    await a.waitFor(() => st().conn === "open" && st().loaded, 20000, "resnapshotted");
    check(on < 6000, `round ${round}: relay back, online after ${on} ms, resnapshotted after ${Date.now() - t1} ms`);
  }
  await a.quit();
} catch (err) {
  check(false, String(err).slice(0, 400));
  await a.quit().catch(() => {});
} finally {
  try { r.kill("SIGKILL"); } catch { /* gone */ }
  try { Deno.removeSync(tdir, { recursive: true }); } catch { /* gone */ }
}
Deno.exit(failed() ? 1 : 0);
