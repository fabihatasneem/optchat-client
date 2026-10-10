// The local engine on this Mac connects by itself (spec-workspaces §6): with the engine's local-client-token file
// readable, the app adds "This Mac" and connects every workspace that token reaches, with no step at all; the token
// is the engine's file (never copied into the token store); a relaunch does the same. Against a fake local engine
// (OPTCHAT_LOCAL_ENGINE=http://127.0.0.1:<port>) and a TEST token file (OPTCHAT_LOCAL_TOKEN_FILE): tests never read
// the real file.
import { App, check, failed } from "./lib/app.ts";
import { ROOT, sleep } from "./lib/cua.ts";

const PORT = 18795;
const eng = new Deno.Command("deno", { args: ["run", "-A", `${ROOT}/tests/lib/fake-engine.ts`, String(PORT), "alpha", "beta", "gamma:off"], stdout: "null", stderr: "null" }).spawn();
await sleep(1200);
const dir = await Deno.makeTempDir({ prefix: "optchat-e2e-" });
const local = `${dir}/local-token`;
Deno.writeTextFileSync(local, "oct_local_test_token\n", { mode: 0o600 });
const env = { OPTCHAT_TEST_HOOKS: "1", OPTCHAT_LOCAL_TOKEN_FILE: local, OPTCHAT_LOCAL_ENGINE: `http://127.0.0.1:${PORT}` };
let a = await App.launch({ bare: true, dir, env });
const st = () => a.state();
try {
  await a.waitFor(() => (st().connected ?? []).length === 2 && (st().connected as any[]).every((c) => c.conn === "open"), 15000, "auto-connected");
  const e = st().engines?.[0];
  check(e?.name === "This Mac" && (st().connected as any[]).map((c) => c.ws).sort().join(",") === "alpha,beta", `the local engine connected by itself: "${e?.name}" with ${(st().connected as any[]).map((c) => c.ws)} (gamma is disabled)`);
  check(st().workspace === "alpha" && st().loaded, "its first workspace is on screen");
  let store = "";
  try { store = Deno.readTextFileSync(`${dir}/tokens.json`); } catch { /* none */ }
  check(!store.includes("oct_local"), "the local token stays the engine's file (not copied into the token store)");
  await a.quit({ keep: true });
  a = await App.launch({ bare: true, dir, env });
  await a.waitFor(() => (st().connected ?? []).length === 2 && (st().connected as any[]).every((c) => c.conn === "open"), 15000, "relaunch");
  check((st().engines ?? []).length === 1, "relaunch: the same engine (not added twice), both workspaces connected again");
} catch (err) {
  check(false, String(err).slice(0, 400));
} finally {
  await a.quit();
  eng.kill("SIGKILL");
  try { Deno.removeSync(dir, { recursive: true }); } catch { /* gone */ }
}
Deno.exit(failed() ? 1 : 0);
