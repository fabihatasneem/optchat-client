// Engines by token through the UI against the REAL engine (spec-workspaces §6), headless clicks: Add engine from the
// empty state, the token "scanned" (OPTCHAT_TEST_QR: the test scanner hands the app data/oc1-token, never printed; on
// iOS it is the camera) with no name, Add → the token's workspace (test-ios) appears connected with no further step;
// the panel, the status bar's workspace chip and notice dots, the Notifications view, Settings Workspace / App; a
// relaunch restores it; Remove. Screenshots: shots/ws-*.jpg (+ -1000.jpg).
import { App, check, failed, SHOTS } from "./lib/app.ts";
import { ROOT, sleep } from "./lib/cua.ts";

const TOKEN = Deno.readTextFileSync(`${ROOT}/data/oc1-token`).trim();
const WS = "test-ios";
const env = { OPTCHAT_TEST_HOOKS: "1", OPTCHAT_TEST_QR: TOKEN };
let a = await App.launch({ bare: true, env });
const dir = a.dir;
const st = () => a.state();
const panel = () => st()?.wsPanel ?? {};
const row = (w: string) => ((panel().rows ?? []) as any[]).find((r) => r.ws === w);
const jpg = async (name: string) => {
  const src = await a.shot(name);
  await new Deno.Command("sips", { args: ["-Z", "1000", src, "--out", `${SHOTS}/${name}-1000.jpg`], stdout: "null", stderr: "null" }).output();
};
try {
  await a.waitFor(() => !!a.probe("empty.add"), 8000, "empty state");
  await jpg("ws-01-empty");
  await a.click("empty.add");
  await a.waitFor(() => st().view === "workspaces" && st().sideOpen && panel().adding, 3000, "add form");
  await a.click("wsp.scan");
  await a.waitFor(() => panel().hasToken, 3000, "token scanned");
  await jpg("ws-02-add-engine");
  await a.click("wsp.add.go");
  await a.waitFor(() => row(WS)?.kind === "connected" && st().workspace === WS && st().loaded, 30000, "connected by itself");
  check(!panel().adding && (panel().rows as any[]).filter((r) => r.kind === "connected").length === 1, `Add engine with the token only: ${WS} connected by itself, nothing outside the token's scope`);
  check(panel().engines?.[0]?.name === "chat.reframe.dev", `named after the host it connected through ("${panel().engines?.[0]?.name}")`);
  await jpg("ws-03-panel-connected");
  check(!!a.probe("status.ws"), "the status bar shows the workspace");
  await a.click("status.notices");
  await a.waitFor(() => st().view === "notifications" && st().notices?.loaded, 10000, "notifications view");
  await jpg("ws-04-notifications");
  check(true, "the status bar's notices open the Notifications view (the engine's list)");
  await a.key("cmd+,");
  await a.click("settings.app");
  await a.waitFor(() => st().settingsGlobal === true && !!a.probe("settings.engines"), 3000, "app tab");
  await jpg("ws-05-settings-app");
  await a.click("settings.workspace");
  await a.waitFor(() => st().settingsGlobal === false, 3000, "workspace tab");
  check(true, "Settings: Workspace / App (engines)");
  // relaunch
  await a.quit({ keep: true });
  a = await App.launch({ bare: true, dir, env });
  await a.waitFor(() => (st().connected ?? []).some((c: any) => c.ws === WS && c.conn === "open") && st().loaded, 30000, "relaunch");
  check(true, "relaunch: the engine and its workspace again");
  // remove
  await a.key("cmd+shift+o");
  await a.waitFor(() => st().view === "workspaces" && st().sideOpen, 3000, "panel");
  await sleep(400);
  await a.click("wsp.remove.0");
  await a.waitFor(() => (st().engines ?? []).length === 0 && (st().connected ?? []).length === 0, 5000, "removed");
  check(true, "Remove: the engine and its workspace are gone");
} catch (e) {
  check(false, String(e).slice(0, 400));
  await a.shot("ws-fail").catch(() => {});
} finally {
  await a.quit();
  try { Deno.removeSync(dir, { recursive: true }); } catch { /* gone */ }
}
Deno.exit(failed() ? 1 : 0);
