// The Workspaces panel (rail view; spec-workspaces §4 as decided: not a popup), through the UI against a FAKE engine
// on 127.0.0.1 (tests/lib/fake-engine.ts; no real workspace): Add engine inline, available / disabled workspaces,
// Connect with the token typed inline, the current one checked, switching, the background dot on the icon, the icon
// and ⇧⌘O behaving like a rail item, Disconnect, the engine going offline (greyed, retried) and coming back.
import { App, check, failed, SHOTS } from "./lib/app.ts";
import { ROOT, sleep } from "./lib/cua.ts";

const PORT = 18790;
const NAMES = ["alpha", "beta", "gamma:off"];
const fake = () => new Deno.Command("deno", { args: ["run", "-A", `${ROOT}/tests/lib/fake-engine.ts`, String(PORT), ...NAMES], stdout: "null", stderr: "null" }).spawn();
let eng = fake();
await sleep(1200);
const a = await App.launch({ bare: true, env: { OPTCHAT_TEST_HOOKS: "1" } });
const st = () => a.state();
const panel = () => st()?.wsPanel ?? {};
const rows = () => (panel().rows ?? []) as any[];
const row = (ws: string) => rows().find((r) => r.ws === ws);
const shown = () => st().view === "workspaces" && st().sideOpen;
/** a phone-legible copy: the rail + sidebar (to the sidebar's right edge), its top part, 1000 px wide */
const phone = async (name: string) => {
  const src = await a.shot(name);
  const side = a.probe("sidebar") ?? [48, 0, 320, 800];
  const wPt = side[0] + side[2], hPt = 520;
  await new Deno.Command("python3", { args: ["-c", `from PIL import Image\nim=Image.open('${src}')\nW,H=im.size\ns=W/${a.state()?.window?.[2] ?? 1360}\nt=int(28*s)\nc=im.crop((0,0,int(${wPt}*s),t+int(${hPt}*s)))\nc=c.resize((1000,int(c.height*1000/c.width)))\nc.save('${SHOTS}/${name}-phone.jpg',quality=88)`] }).output();
};

try {
  await a.waitFor(() => !!a.probe("empty.add"), 8000, "empty state");
  await a.click("empty.add");
  await a.waitFor(() => shown() && panel().adding, 3000, "panel with the add form");
  check(st().overlay === null, "Add engine opens the Workspaces panel in the sidebar (no popup), its form inline");
  await a.click("wsp.endpoint");
  await a.type(`http://127.0.0.1:${PORT}`);
  await a.click("wsp.add.go");
  await a.waitFor(() => row("alpha")?.kind === "available" && row("beta")?.kind === "available", 8000, "available listed");
  check(!row("gamma"), "the engine's workspaces: alpha, beta available; the disabled one collapsed");
  await a.click("wsp.disabled.0");
  await a.waitFor(() => row("gamma")?.kind === "disabled", 3000, "disabled expanded");
  check(true, "a click shows the disabled one (gamma)");

  // connect alpha: the token typed inline under its row
  await a.click("wsp.connect.alpha");
  await a.waitFor(() => panel().connecting === "alpha", 3000, "token row");
  check(st().focus !== "composer", "Connect… opens the token field inline, focused");
  await a.type("fake-token-alpha");
  await a.waitFor(() => panel().hasToken, 2000, "token typed");
  await phone("wsp-00-connect");
  await a.click("wsp.go");
  await a.waitFor(() => st().workspace === "alpha" && st().loaded, 8000, "alpha on screen");
  check(row("alpha")?.on && row("alpha")?.kind === "connected", "alpha connected and checked as the current one");
  check(st().engineSockets === 1, `one socket for the engine (${st().engineSockets})`);
  await a.click("wsp.connect.beta");
  await a.waitFor(() => panel().connecting === "beta", 3000, "token row beta");
  await a.type("fake-token-beta");
  await a.key("Return");
  await a.waitFor(() => st().workspace === "beta" && st().loaded, 8000, "beta on screen (Enter connects)");
  check(row("alpha")?.alert && !!a.probe("logo.dot"), "alpha in the background has an unread warning: dot on its row and on the icon");
  check(st().engineSockets === 1, `alpha and beta (different tokens) share the engine's one socket (${st().engineSockets})`);
  await phone("wsp-01-panel");

  // the icon and ⇧⌘O behave like a rail item
  await a.click("logo");
  await a.waitFor(() => !st().sideOpen, 3000, "hidden");
  await a.click("logo");
  await a.waitFor(shown, 3000, "shown");
  check(true, "the icon toggles the panel like a rail item (click on the shown view hides the sidebar)");
  await a.key("cmd+1");
  await a.waitFor(() => st().view === "tasks", 3000, "tasks");
  await a.key("cmd+shift+o");
  await a.waitFor(shown, 3000, "⇧⌘O");
  check(true, "another rail item replaces it; ⇧⌘O brings it back");

  // switching from the panel
  await a.click("wsp.ws.alpha");
  await a.waitFor(() => st().workspace === "alpha" && row("alpha")?.on, 3000, "switched");
  check(!row("alpha")?.alert && row("beta")?.alert && st().bgAlert, "switched to alpha from its row: alpha's notices move to the status bar, beta (now in the background) gets the dot");

  // disconnect beta
  await a.click("wsp.disc.beta");
  await a.waitFor(() => row("beta")?.kind === "available", 4000, "beta available again");
  check(st().workspace === "alpha", "Disconnect beta: available again, alpha stays");

  // a refused token: the connection shows it
  await a.click("wsp.connect.beta");
  await a.waitFor(() => panel().connecting === "beta", 3000, "token row beta");
  await a.type("bad");
  await a.click("wsp.go");
  await a.waitFor(() => row("beta")?.state === "token refused", 6000, "refused shown");
  check(st().engines?.[0]?.online === true && (st().connected ?? []).find((c: any) => c.ws === "alpha")?.conn === "open", `a refused token: beta's row says "${row("beta")?.state}", the engine's one socket and alpha stay up`);
  await a.click("wsp.ws.alpha");
  await a.waitFor(() => st().workspace === "alpha", 3000, "alpha");
  await a.click("wsp.disc.beta");
  await a.waitFor(() => row("beta")?.kind === "available", 4000, "beta removed");

  // a token revoked on the engine drops only its workspace
  await a.click("wsp.connect.beta");
  await a.waitFor(() => panel().connecting === "beta", 3000, "token row beta");
  await a.type("fake-token-beta");
  await a.click("wsp.go");
  await a.waitFor(() => st().workspace === "beta" && st().loaded, 8000, "beta back");
  await fetch(`http://127.0.0.1:${PORT}/test/revoke?ws=beta`).then((r) => r.text());
  await a.waitFor(() => row("beta")?.state === "token refused" && (st().connected ?? []).find((c: any) => c.ws === "alpha")?.conn === "open", 6000, "revoked");
  check(true, "a revoked token drops only its workspace (beta: token refused); alpha stays connected on the same socket");
  await a.click("wsp.ws.alpha");
  await a.waitFor(() => st().workspace === "alpha", 3000, "alpha");
  await a.click("wsp.disc.beta");
  await a.waitFor(() => row("beta")?.kind === "available", 4000, "beta removed again");

  // the engine goes away: greyed "offline · retrying", then back
  eng.kill("SIGKILL");
  await eng.status;
  const t0 = Date.now();
  await a.waitFor(() => st().engines?.[0]?.online === false, 5000, "offline");
  check(true, `engine gone: offline after ${Date.now() - t0} ms`);
  await phone("wsp-02-offline");
  eng = fake();
  const t1 = Date.now();
  await a.waitFor(() => st().engines?.[0]?.online === true && st().conn === "open", 10000, "back");
  check(true, `engine back: online after ${Date.now() - t1} ms`);
} catch (e) {
  check(false, String(e).slice(0, 400));
  await a.shot("wsp-fail").catch(() => {});
} finally {
  await a.quit();
  try { eng.kill("SIGKILL"); } catch { /* gone */ }
}
Deno.exit(failed() ? 1 : 0);
