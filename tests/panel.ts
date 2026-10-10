// The Workspaces panel with engine tokens (spec-workspaces §6), against a FAKE engine on 127.0.0.1
// (tests/lib/fake-engine.ts; no real workspace): Add engine asks only for a token (scanned here by the test
// "scanner", OPTCHAT_TEST_QR, or pasted) and an optional name; the oc1 token carries the engine's addresses (the
// first that answers wins: here the second, the first is closed) and every workspace in its scope connects by itself;
// live scope changes (removed → disconnected and gone, added → appears connected), the same after a relaunch; a
// damaged token is refused with a message; the icon and ⇧⌘O behave like a rail item; the engine going away and back.
import { App, check, failed } from "./lib/app.ts";
import { ROOT, sleep } from "./lib/cua.ts";

const PORT = 18790;
const NAMES = ["alpha", "beta", "gamma", "delta:off"];
const fake = () => new Deno.Command("deno", { args: ["run", "-A", `${ROOT}/tests/lib/fake-engine.ts`, String(PORT), ...NAMES, "--scope=alpha,beta"], stdout: "null", stderr: "null" }).spawn();
const b64 = (s: string) => btoa(s).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
const TOKEN = `oc1.${b64(JSON.stringify({ v: 1, urls: ["http://127.0.0.1:1", `http://127.0.0.1:${PORT}`], id: "t1" }))}.s3cret`;
let eng = fake();
await sleep(1200);
const dir = await Deno.makeTempDir({ prefix: "optchat-e2e-" });
const env = { OPTCHAT_TEST_HOOKS: "1", OPTCHAT_TEST_QR: TOKEN };
let a = await App.launch({ bare: true, dir, env });
const st = () => a.state();
const panel = () => st()?.wsPanel ?? {};
const rows = () => (panel().rows ?? []) as any[];
const row = (ws: string) => rows().find((r) => r.ws === ws);
const shown = () => st().view === "workspaces" && st().sideOpen;
const scope = (set: string) => fetch(`http://127.0.0.1:${PORT}/test/scope?set=${set}`).then((r) => r.text());
try {
  await a.waitFor(() => !!a.probe("empty.add"), 8000, "empty state");
  await a.click("empty.add");
  await a.waitFor(() => shown() && panel().adding, 3000, "panel with the add form");
  check(st().overlay === null && panel().canScan, "Add engine opens in the Workspaces panel: a token field (and a camera button where the platform can scan)");
  // a damaged token: a message, nothing added
  await a.click("wsp.token");
  await a.type("oc1.not-base64!.x");
  await a.click("wsp.add.go");
  await a.waitFor(() => !!panel().err, 3000, "error");
  check((panel().engines ?? []).length === 0, `a damaged token is refused: "${panel().err}"`);
  // the scanner fills the field; no name: the engine is named after the host it connected through
  await a.click("wsp.scan");
  await a.waitFor(() => panel().hasToken, 3000, "scanned");
  await a.click("wsp.add.go");
  await a.waitFor(() => row("alpha")?.kind === "connected" && row("beta")?.kind === "connected", 15000, "the token's workspaces connected");
  const e0 = panel().engines[0];
  check(e0.endpoint === `http://127.0.0.1:${PORT}` && e0.name === `127.0.0.1:${PORT}` && e0.auto, `the first address that answers wins (${e0.endpoint}); the name is its host (${e0.name})`);
  check(!row("gamma") && !panel().adding, "every workspace in the token's scope connected by itself (alpha, beta); gamma (out of scope) is not shown; no per-workspace step");
  check(st().engineSockets === 1, `one socket for the engine (${st().engineSockets})`);
  check(!a.probe("wsp.disabled.0") || !rows().some((r) => r.kind === "disabled"), "a token's engine shows only the token's scope: no disabled row (delta is disabled on the engine)");
  // live scope changes
  await scope("alpha,gamma");
  await a.waitFor(() => !row("beta") && row("gamma")?.kind === "connected", 8000, "scope changed");
  check(true, "removed from the token: beta disconnected and gone; added: gamma appears connected");
  // the rail: the icon and ⇧⌘O
  await a.click("logo");
  await a.waitFor(() => !st().sideOpen, 3000, "hidden");
  await a.key("cmd+1");
  await a.key("cmd+shift+o");
  await a.waitFor(shown, 3000, "⇧⌘O");
  check(true, "the icon toggles the panel like a rail item; ⇧⌘O brings it back");
  await a.click("wsp.ws.alpha");
  await a.waitFor(() => st().workspace === "alpha", 3000, "switched");
  await a.waitFor(() => row("gamma")?.alert && st().bgAlert, 5000, "background dot");
  check(true, "a workspace in the background with an unread warning: a dot on its row and on the icon");
  // relaunch: the engine (token in the store) and its scope's workspaces again
  await a.quit({ keep: true });
  a = await App.launch({ bare: true, dir, env });
  await a.waitFor(() => (st().connected ?? []).length === 2 && (st().connected as any[]).every((c) => c.conn === "open"), 15000, "relaunch");
  check((st().connected as any[]).map((c) => c.ws).sort().join(",") === "alpha,gamma" && st().workspace === "alpha", `relaunch: the token's workspaces again (${(st().connected as any[]).map((c) => c.ws)}), alpha on screen`);
  const store = Deno.readTextFileSync(`${dir}/tokens.json`), prefs = Deno.readTextFileSync(`${dir}/prefs.json`);
  check(store.includes("engine|") && !prefs.includes("s3cret"), "the engine token is in the token store, not in the prefs");
  // the engine goes away and comes back
  eng.kill("SIGKILL");
  await eng.status;
  const t0 = Date.now();
  await a.waitFor(() => st().engines?.[0]?.online === false, 5000, "offline");
  check(true, `engine gone: offline after ${Date.now() - t0} ms`);
  eng = fake();
  const t1 = Date.now();
  await a.waitFor(() => st().engines?.[0]?.online === true && (st().connected as any[]).every((c) => c.conn === "open"), 15000, "back");
  check(true, `engine back: online and its workspaces connected again after ${Date.now() - t1} ms`);
  // remove the engine
  await a.cmd({ op: "menu" });
  await a.click("wsp.remove.0");
  await a.waitFor(() => (st().engines ?? []).length === 0 && (st().connected ?? []).length === 0, 4000, "removed");
  check(!Deno.readTextFileSync(`${dir}/tokens.json`).includes("engine|"), "Remove: the engine, its workspaces and its token are gone");
} catch (e) {
  check(false, String(e).slice(0, 400));
  await a.shot("wsp-fail").catch(() => {});
} finally {
  await a.quit();
  try { eng.kill("SIGKILL"); } catch { /* gone */ }
  if (!Deno.env.get("OPTCHAT_KEEP_E2E")) { try { Deno.removeSync(dir, { recursive: true }); } catch { /* gone */ } } else console.log("kept", dir);
}
Deno.exit(failed() ? 1 : 0);
