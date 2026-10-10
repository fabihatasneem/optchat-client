// The local engine's client token (the engine's local-client-token file, scope: all workspaces): on the local engine,
// Connect needs no token field. Simulated with a TEST file holding test-gpui-c's scoped token (OPTCHAT_LOCAL_TOKEN_FILE;
// the real file is never read by tests): c connects at once; d (out of that token's scope) is refused, its row offers
// the token field, and its own token connects it; a relaunch connects c again through the local token, and the token
// store holds only d's.
import { App, check, failed } from "./lib/app.ts";
import { ROOT } from "./lib/cua.ts";

const dir = await Deno.makeTempDir({ prefix: "optchat-e2e-" });
const local = `${dir}/local-token`;
const toks = JSON.parse(Deno.readTextFileSync(`${ROOT}/data/test-tokens.json`));
Deno.writeTextFileSync(local, toks["test-gpui-c"] + "\n", { mode: 0o600 });
const env = { OPTCHAT_TEST_HOOKS: "1", OPTCHAT_LOCAL_TOKEN_FILE: local };
let a = await App.launch({ bare: true, dir, env });
const st = () => a.state();
const panel = () => st()?.wsPanel ?? {};
const row = (ws: string) => ((panel().rows ?? []) as any[]).find((r) => r.ws === ws);
try {
  await a.click("empty.add");
  await a.waitFor(() => panel().adding, 3000, "add form");
  await a.click("wsp.endpoint");
  await a.type("chat.reframe.dev");
  await a.click("wsp.add.go");
  await a.waitFor(() => row("test-gpui-c")?.kind === "available", 10000, "listed");
  await a.click("wsp.connect.test-gpui-c");
  await a.waitFor(() => st().workspace === "test-gpui-c" && st().loaded, 15000, "c connected");
  check(panel().connecting == null, "Connect on the local engine with its local token: connected at once, no token field");
  await a.click("wsp.connect.test-gpui-d");
  await a.waitFor(() => row("test-gpui-d")?.state === "token refused", 10000, "d refused");
  check(true, "a workspace the local token does not reach: its row says token refused");
  await a.click("wsp.ws.test-gpui-d");
  await a.waitFor(() => panel().connecting === "test-gpui-d" && panel().hasToken, 3000, "field");
  check(true, "a click on it opens the token field");
  await a.click("wsp.go");
  await a.waitFor(() => st().workspace === "test-gpui-d" && st().loaded && row("test-gpui-d")?.state == null, 15000, "d connected");
  check(true, "its own token connects it");
  const store = JSON.parse(Deno.readTextFileSync(`${dir}/tokens.json`));
  check(Object.keys(store).length === 1 && Object.keys(store)[0].endsWith("|test-gpui-d"), `the token store holds only the typed token (${Object.keys(store).map((k) => k.split("|")[1])})`);
  // the local token rotated: a stale one is refused; once the file has the new one, a click on the row connects
  await a.click("wsp.disc.test-gpui-c");
  await a.waitFor(() => row("test-gpui-c")?.kind === "available", 4000, "c off");
  Deno.writeTextFileSync(local, "oct_stale_token_for_the_test\n");
  await a.click("wsp.connect.test-gpui-c");
  await a.waitFor(() => row("test-gpui-c")?.state === "token refused", 10000, "stale refused");
  Deno.writeTextFileSync(local, toks["test-gpui-c"] + "\n");
  await a.click("wsp.ws.test-gpui-c");
  await a.waitFor(() => row("test-gpui-c")?.kind === "connected" && row("test-gpui-c")?.state == null && st().workspace === "test-gpui-c", 15000, "rotated token");
  check(panel().connecting == null, "a rotated local token: the stale one is refused, a click on the row connects with the new file, no field");
  await a.quit({ keep: true });
  a = await App.launch({ bare: true, dir, env });
  await a.waitFor(() => (st().connected ?? []).length === 2 && (st().connected as any[]).every((c) => c.conn === "open"), 20000, "relaunch");
  check(true, "relaunch: c through the local token, d through its stored token, both connected");
} catch (e) {
  check(false, String(e).slice(0, 400));
} finally {
  await a.quit();
  try { Deno.removeSync(dir, { recursive: true }); } catch { /* gone */ }
}
Deno.exit(failed() ? 1 : 0);
