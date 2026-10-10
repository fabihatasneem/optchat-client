// Phase 4 (spec-workspaces §4) against the real engine: needs test workspaces and their tokens (OPTCHAT_TEST_DB /
// data/test-tokens.json; the names below are the last ones given). The panel UI alone: tests/panel.ts (fake engine).
// Engines and workspaces in the app. Through the UI only (Computer Use on probes):
// add the engine, connect test-gpui-a and test-gpui-b with their tokens (prefilled from data/test-tokens.json by the
// test hook, never printed), switch between them, run a Haiku turn in the background one (a child with a tiny budget:
// it pauses with a warning) and see the dot on the workspace icon, read the notice in the Notifications view,
// disconnect. Then the engine going away (a local relay this test starts and kills; the real engine stays up): offline
// within 5 s, back when it returns. Never touches any other workspace (main, test-ios, v2 are listed, never clicked).
import { App, check, failed, SHOTS } from "./lib/app.ts";
import { ROOT, sleep } from "./lib/cua.ts";

const env = { OPTCHAT_TEST_HOOKS: "1" };
let a = await App.launch({ bare: true, env });
const st = () => a.state();
const extra = (k: string) => st()?.[k];
const wsActive = () => st()?.workspace;
const conn = (ws: string) => (st()?.connected ?? []).find((c: any) => c.ws === ws);
const jpg = async (name: string) => {
  // a phone-legible ~1000 px copy next to the full screenshot
  const src = await a.shot(name);
  await new Deno.Command("sips", { args: ["-Z", "1000", src, "--out", `${SHOTS}/${name}-1000.jpg`], stdout: "null", stderr: "null" }).output();
};

try {
  // ---- nothing yet: the empty state; Add engine
  await a.waitFor(() => !!a.probe("empty.add"), 8000, "empty state");
  check(st().ready === false && (st().connected ?? []).length === 0, "a fresh app: no workspace, the editor says what to do");
  await a.shot("ws-01-empty");
  await a.click("empty.add");
  await a.waitFor(() => st().wsPanel?.adding, 4000, "add engine dialog");
  await a.click("wsp.endpoint");
  await a.type("chat.reframe.dev");
  await a.click("wsp.add.go");
  await a.waitFor(() => (extra("wsPanel")?.rows ?? []).some((r: any) => r.ws === "test-gpui-a" && r.kind === "available"), 12000, "engine's workspaces listed");
  const rows = extra("wsPanel").rows;
  check(rows.some((r: any) => r.ws === "test-gpui-b" && r.kind === "available"), "the menu lists the engine's running workspaces as available");
  check(!rows.some((r: any) => r.kind === "disabled"), "disabled workspaces are collapsed");
  if (a.probe("wsp.disabled.0")) {
    await a.click("wsp.disabled.0");
    await a.waitFor(() => (extra("wsPanel")?.rows ?? []).some((r: any) => r.kind === "disabled"), 3000, "disabled expanded");
    check(true, "a click shows the disabled ones");
  }
  await jpg("ws-02-menu-available");

  // ---- connect a, then b
  await a.click("wsp.connect.test-gpui-a");
  await a.waitFor(() => extra("wsPanel")?.connecting && extra("wsPanel")?.hasToken, 4000, "connect dialog with the test token");
  await a.shot("ws-03-connect");
  await a.click("wsp.go");
  await a.waitFor(() => wsActive() === "test-gpui-a" && st().loaded && st().ready, 20000, "test-gpui-a on screen");
  check(st().tasks > 50, `test-gpui-a connected: ${st().tasks} tasks`);
  if (!(st().view === "workspaces" && st().sideOpen)) await a.click("logo");
  await a.waitFor(() => (st().view === "workspaces" && st().sideOpen), 3000, "menu");
  await a.click("wsp.connect.test-gpui-b");
  await a.waitFor(() => extra("wsPanel")?.connecting && extra("wsPanel")?.hasToken, 4000, "connect b");
  await a.click("wsp.go");
  await a.waitFor(() => wsActive() === "test-gpui-b" && st().loaded, 20000, "test-gpui-b on screen");
  check(conn("test-gpui-a")?.conn === "open", "test-gpui-a stays connected in the background");

  // ---- switching is a swap: instant, with the task on screen and its tabs
  if (!(st().view === "workspaces" && st().sideOpen)) await a.click("logo");
  await a.waitFor(() => (st().view === "workspaces" && st().sideOpen), 3000, "menu");
  await jpg("ws-04-menu-connected");
  const before = (st().wsSwitches ?? []).length;
  await a.click("wsp.ws.test-gpui-a");
  await a.waitFor(() => wsActive() === "test-gpui-a", 3000, "switched to a");
  check(st().loaded, "switched to test-gpui-a: its chat is there at once (no loading)");
  const ms = (st().wsSwitches ?? []).slice(before);
  check(ms.length > 0 && ms.every((m: number) => m < 16), `the switch took ${ms.map((m: number) => m.toFixed(2)).join(", ")} ms (state swap, < 1 frame)`);
  await a.key("cmd+shift+o");
  await a.waitFor(() => (st().view === "workspaces" && st().sideOpen), 3000, "menu by key");
  await a.click("wsp.ws.test-gpui-b");
  await a.waitFor(() => wsActive() === "test-gpui-b" && st().loaded, 3000, "back to b");
  check(true, "⇧⌘O opens the menu");

  // ---- a Haiku turn in the background workspace: a child with a tiny budget in b (it pauses: a warning), seen from a
  await a.key("cmd+n");
  await a.waitFor(() => st().overlay === "newtask", 5000, "new task dialog");
  await a.type("say hi (budget test)");
  await a.click("newtask.instruction");
  await a.type("Reply with exactly one word: hi");
  await a.click("newtask.usd");
  await a.type("0.0001");
  await a.click("newtask.start");
  await a.waitFor(() => st().overlay !== "newtask" && st().current !== 0, 10000, "child spawned in b");
  const child = st().current;
  if (!(st().view === "workspaces" && st().sideOpen)) await a.click("logo");
  await a.waitFor(() => (st().view === "workspaces" && st().sideOpen), 3000, "menu");
  await a.click("wsp.ws.test-gpui-a");
  await a.waitFor(() => wsActive() === "test-gpui-a", 3000, "a on screen while b works");
  await a.waitFor(() => st().bgAlert && (conn("test-gpui-b")?.unread?.warn ?? 0) + (conn("test-gpui-b")?.unread?.error ?? 0) > 0, 180000, "background warning");
  check(!!a.probe("logo.dot"), `b's #${child} paused on its budget while a is on screen: a dot on the workspace icon (b unread ${JSON.stringify(conn("test-gpui-b").unread)})`);
  await jpg("ws-05-background-dot");
  if (!(st().view === "workspaces" && st().sideOpen)) await a.click("logo");
  await a.waitFor(() => (st().view === "workspaces" && st().sideOpen), 3000, "menu");
  const brow = (extra("wsPanel")?.rows ?? []).find((r: any) => r.ws === "test-gpui-b");
  check(brow?.alert, "the menu marks test-gpui-b with the dot");
  await a.click("wsp.ws.test-gpui-b");
  await a.waitFor(() => wsActive() === "test-gpui-b", 3000, "b on screen");
  check(!st().bgAlert, "on screen, b's notices move to the status bar (no dot on the icon)");

  // ---- the status bar dots → the Notifications view; a click opens the task and marks it read
  await a.waitFor(() => !!a.probe("status.notice.warn"), 4000, "status bar warn dot");
  await jpg("ws-06-statusbar");
  await a.click("status.notices");
  await a.waitFor(() => st().view === "notifications" && extra("notices")?.loaded, 8000, "notifications view");
  await a.click("notif.filter.warn");
  await a.waitFor(() => extra("notices")?.level === "warn" && extra("notices")?.first, 4000, "warn filter");
  const first = extra("notices").first;
  check(/budget|paused/i.test(first.text) && first.task === child && !first.read, `the newest warning: "${first.text}"`);
  await jpg("ws-07-notifications");
  const warn0 = st().unread.warn;
  await a.click("notif.row.0");
  await a.waitFor(() => st().current === child && st().unread.warn < warn0, 8000, "opened + read");
  check(true, `a click opened #${child} and marked it read (warn ${warn0} → ${st().unread.warn}, on the engine)`);
  await a.click("notif.filter.all");
  await a.click("notif.readAll");
  await a.waitFor(() => st().unread.ok + st().unread.info + st().unread.warn + st().unread.error === 0, 8000, "all read");
  check(!a.probe("status.notice.warn"), "Mark all read clears the status bar dots");
  await a.shot("ws-08-all-read");

  // ---- settings: Workspace / App
  await a.key("cmd+,");
  await a.waitFor(() => st().view === "settings", 3000, "settings");
  await a.click("settings.app");
  await a.waitFor(() => st().settingsGlobal === true && !!a.probe("settings.engines"), 3000, "app tab");
  await a.shot("ws-09-settings-app");
  await a.click("settings.workspace");
  await a.waitFor(() => st().settingsGlobal === false, 3000, "workspace tab");
  await a.shot("ws-10-settings-workspace");
  check(true, "Settings: Workspace (the workspace's chat settings) / App (engines, theme, zoom)");

  // ---- relaunch: the connections come back (tokens from the store), on the same workspace
  await a.quit({ keep: true });
  const dir = a.dir;
  a = await App.launch({ bare: true, dir, env });
  await a.waitFor(() => (st().connected ?? []).length === 2 && wsActive() === "test-gpui-b" && st().loaded, 20000, "relaunch restores");
  check(true, "relaunch: both connections back, test-gpui-b on screen");

  // ---- disconnect a from the menu
  if (!(st().view === "workspaces" && st().sideOpen)) await a.click("logo");
  await a.waitFor(() => (st().view === "workspaces" && st().sideOpen), 3000, "menu");
  await a.click("wsp.disc.test-gpui-a");
  await a.waitFor(() => !conn("test-gpui-a"), 4000, "a disconnected");
  check(wsActive() === "test-gpui-b" && (extra("wsPanel")?.rows ?? []).some((r: any) => r.ws === "test-gpui-a" && r.kind === "available"), "Disconnect: a is available again, b stays on screen");
  await a.shot("ws-11-disconnected");
  await a.quit();
} catch (e) {
  check(false, String(e).slice(0, 400));
  await a.shot("ws-fail").catch(() => {});
  await a.quit().catch(() => {});
}

// ---- the engine goes away (a local relay we start and kill) and comes back
const PORT = 18787;
const relay = () => new Deno.Command("deno", { args: ["run", "-A", `${ROOT}/tests/lib/relay.ts`, String(PORT)], env: { DENO_TLS_CA_STORE: "system" }, stdout: "null", stderr: "null" }).spawn();
let r = relay();
await sleep(1500);
try {
  a = await App.launch({ db: "test-gpui-a", env: { ...env, OPTCHAT_ENGINE: `http://127.0.0.1:${PORT}`, OPTCHAT_ENGINE_NAME: "local relay" } });
  check(st().engines?.[0]?.online === true, "through the relay: connected");
  r.kill("SIGKILL");
  await r.status;
  const t0 = Date.now();
  await a.waitFor(() => st().engines?.[0]?.online === false && st().conn !== "open", 5000, "offline within 5 s");
  check(true, `the relay killed: offline after ${Date.now() - t0} ms`);
  await a.waitFor(() => /disconnected|connecting/.test(extra("statusbar")?.status ?? ""), 2000, "status").catch(() => {});
  check(/disconnected|connecting/.test(extra("statusbar")?.status ?? ""), `status bar: "${extra("statusbar")?.status}"`);
  if (!(st().view === "workspaces" && st().sideOpen)) await a.click("logo");
  await a.waitFor(() => (st().view === "workspaces" && st().sideOpen), 3000, "menu");
  await jpg("ws-12-offline-menu");
  r = relay();
  const t1 = Date.now();
  await a.waitFor(() => st().engines?.[0]?.online === true && st().conn === "open" && st().loaded, 15000, "back online");
  check(Date.now() - t1 < 8000, `the relay back: online again after ${Date.now() - t1} ms`);
  await a.quit();
} catch (e) {
  check(false, String(e).slice(0, 400));
  await a.shot("ws-fail-relay").catch(() => {});
  await a.quit().catch(() => {});
} finally {
  try { r.kill("SIGKILL"); } catch { /* gone */ }
}
Deno.exit(failed() ? 1 : 0);
