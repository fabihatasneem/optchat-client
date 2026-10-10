// The narrow / phone layout (crate::phone), on the desktop in a phone-sized window (400×860 pt), against the fake
// engine's perf fixture (1,770-entry main chat with turns and tool calls + 5 tasks; fake dictation). Headless input
// through the test channel (clicks, holds, drags, trackpad swipes); screenshots next to the design's (optchat-design
// shots: storyboard-bar-A, iphone-*): shots/phone-*.jpg and phone-*-1000.jpg.
import { App, check, failed, SHOTS } from "./lib/app.ts";
import { ROOT, sleep } from "./lib/cua.ts";

const PORT = 18792;
const eng = new Deno.Command("deno", { args: ["run", "-A", `${ROOT}/tests/lib/fake-engine.ts`, String(PORT), "phone", "--big"], stdout: "null", stderr: "null" }).spawn();
await sleep(1200);
const dir = await Deno.makeTempDir({ prefix: "optchat-e2e-" });
Deno.writeTextFileSync(`${dir}/fake-tokens.json`, JSON.stringify({ phone: "fake-token" }));
const a = await App.launch({ bare: true, dir, env: { OPTCHAT_TEST_HOOKS: "1", OPTCHAT_FAKE_MIC: `${ROOT}/tests/fixtures/dict.wav`, OPTCHAT_ENGINE: `http://127.0.0.1:${PORT}`, OPTCHAT_ENGINE_NAME: "fake", OPTCHAT_CONNECT: "phone", OPTCHAT_TEST_TOKENS: `${dir}/fake-tokens.json` } });
const st = () => a.state();
const ph = () => st()?.phone ?? {};
const center = (p: number[]) => [p[0] + p[2] / 2, p[1] + p[3] / 2];
const shot = async (name: string) => {
  const src = await a.shot(name);
  await new Deno.Command("sips", { args: ["-Z", "1000", src, "--out", `${SHOTS}/${name}-1000.jpg`], stdout: "null", stderr: "null" }).output();
};
/** press the chat button for `ms` (a tap, a long-press, a hold) */
const press = async (ms: number) => {
  const [x, y] = center(await a.waitProbe("phone.chat"));
  if (ms < 200) {
    // a tap: press and release in one go (separate commands could take longer than the long-press delay headless)
    await a.cmd({ op: "down", x, y }, { op: "up", x, y });
  } else {
    await a.cmd({ op: "down", x, y });
    await sleep(ms);
    await a.cmd({ op: "up", x, y });
  }
  await sleep(250);
};
try {
  await a.waitFor(() => st().ready && st().loaded, 20000, "loaded");
  await a.cmd({ op: "resize", w: 400, h: 860 });
  await a.waitFor(() => st().narrow && !!a.probe("phone.chat"), 5000, "phone layout");
  await sleep(800);
  check(!!a.probe("phone.tab.tasks") && !!a.probe("phone.tab.more") && !!a.probe("phone.cost"), "a 400 pt window: the phone layout (bottom bar Tasks · Memory · chat · Alerts · More, header with cost)");
  check(/Idle|Working/.test(st().phoneHeader?.subtitle ?? ""), `header: "${st().phoneHeader?.title}", subtitle "${st().phoneHeader?.subtitle}", cost ${st().phoneHeader?.cost}`);
  await shot("phone-01-idle");

  // the compact chat: a quiet line per turn, expands on tap
  const foot = Object.keys(a.allProbes()).filter((k) => k.startsWith("foot.line.")).sort((x, y) => a.probe(y)![1] - a.probe(x)![1])[0];
  check(!!foot && !Object.keys(a.allProbes()).some((k) => k.startsWith("foot.model.")), `compact feed: one quiet line per turn (${foot}), no chips`);
  if (foot) {
    await a.click(foot);
    await sleep(400);
    await shot("phone-02-turn-expanded");
  }

  // tap: the composer opens; type; tap outside: the draft floats; tap ^: sent
  await press(80);
  await a.waitFor(() => ph().expanded, 3000, "composer open");
  check(st().focus === "composer", "a tap on the chat button opens the composer, focused");
  await a.type("let it finish, then ask the child whether the arc renderer is stuck");
  await shot("phone-03-editing");
  await a.click("feed", { dy: 40 });
  await a.waitFor(() => !ph().expanded && ph().floating && ph().button === "send", 3000, "draft floats");
  check(true, "a tap outside closes it: the draft floats above the chat, the button becomes ^");
  await shot("phone-04-draft-floats");
  await a.click("phone.card");
  await a.waitFor(() => ph().expanded, 3000, "edit again");
  check(true, "a tap on the card edits it");
  await a.click("feed", { dy: 40 });
  await a.waitFor(() => ph().floating, 3000, "floats again");
  await sleep(400);
  await press(80);
  await a.waitFor(() => !ph().floating && ph().draft === "", 5000, "sent");
  check(true, "^ sends it (and runs)");

  // long-press: dictation, ✕ stops → ^
  await press(700);
  await a.waitFor(() => ph().dictating && ph().button === "stop", 5000, "dictating");
  await sleep(2500);
  await shot("phone-05-dictating");
  check(ph().floating, "a long-press dictates: ✕, the words stream into the floating card");
  await press(80);
  await a.waitFor(() => !ph().dictating, 8000, "stopped");
  await a.waitFor(() => ph().button === "send", 4000, "^ after dictation").catch(() => {});
  check(ph().button === "send", `✕ stops; what was said is the draft (^): "${ph().draft}"`);
  await shot("phone-06-dictated");
  // double-tap sends
  {
    const [x, y] = center(await a.waitProbe("phone.chat"));
    await a.cmd({ op: "click", x, y }, { op: "click", x, y });
    await a.waitFor(() => ph().draft === "" && !ph().floating, 5000, "double-tap sent");
    check(true, "a double-tap sends");
  }
  // hold-and-release: push-to-talk
  await press(2200);
  await a.waitFor(() => !ph().dictating, 8000, "push-to-talk ended");
  check(ph().draft.length > 0, `hold-and-release: push-to-talk, ended by the release ("${ph().draft}")`);
  // clear the draft for what follows
  await a.waitFor(() => st().dict === "idle", 8000, "dictation finished");
  await a.click("phone.card");
  await a.waitFor(() => ph().expanded, 3000, "edit");
  await a.key("cmd+a");
  await a.key("BackSpace");
  await a.waitFor(() => ph().draft === "", 3000, "draft cleared");
  await a.click("feed", { dy: 40 });

  // with the keyboard up (composer open, focused): double-tap sends, long-press dictates, hold-to-talk stops on release
  await press(80);
  await a.waitFor(() => ph().expanded, 3000, "composer open (keyboard up)").catch((e) => { console.log("phone state:", JSON.stringify(ph()), JSON.stringify(st().lastCmd)); throw e; });
  await a.waitFor(() => st().focus === "composer", 1500, "focused").catch(async () => await a.click("composer.input"));
  await a.type("sent by a double tap");
  {
    const [x, y] = center(await a.waitProbe("phone.chat"));
    await a.cmd({ op: "click", x, y }, { op: "click", x, y });
    await a.waitFor(() => ph().draft === "" && !ph().expanded, 5000, "double-tap with the keyboard up");
    check(true, "keyboard up: a double-tap sends");
  }
  await press(80);
  await a.waitFor(() => ph().expanded, 3000, "open again");
  await press(700);
  await a.waitFor(() => ph().dictating && !ph().expanded, 5000, "long-press with the keyboard up");
  check(true, "keyboard up: a long-press dictates (the keyboard goes down, words float)");
  await press(80);
  await a.waitFor(() => !ph().dictating, 8000, "stopped");
  await press(80); // ^ sends what was said
  await a.waitFor(() => ph().draft === "", 5000, "sent");
  await press(80);
  await a.waitFor(() => ph().expanded, 3000, "open again");
  await press(2200);
  await a.waitFor(() => !ph().dictating, 8000, "hold-to-talk ended by the release");
  check(ph().draft.length > 0, `keyboard up: hold-to-talk stops on release ("${ph().draft}")`);
  await a.click("phone.card");
  await a.waitFor(() => ph().expanded, 3000, "edit");
  const run = a.probe("composer.run");
  check(run && run[2] >= 44 && run[3] >= 44, `the composer's run button is a 44 pt target (${run?.[2]}×${run?.[3]})`);
  await a.key("cmd+a");
  await a.key("BackSpace");
  await a.click("feed", { dy: 40 });

  // screens: Tasks (full screen, large title), the active tab again → chat; More; Status
  await a.click("phone.tab.tasks");
  await a.waitFor(() => ph().screen === "tasks" && !!a.probe("phone.screenTitle"), 3000, "tasks screen");
  await sleep(500);
  await shot("phone-07-tasks");
  await a.click("phone.tab.tasks");
  await a.waitFor(() => ph().screen === null, 3000, "back by the active tab");
  check(true, "Tasks opens as a full-screen panel; tapping its tab again returns to the chat");
  // left edge swipe: the last panel
  await a.cmd({ op: "drag", x0: 6, y0: 400, x1: 200, y1: 400 });
  await a.waitFor(() => ph().screen === "tasks", 3000, "edge swipe opened the last panel");
  check(true, "an edge swipe from the left opens the last panel (Tasks)");
  await a.cmd({ op: "drag", x0: 396, y0: 400, x1: 150, y1: 400 });
  await a.waitFor(() => ph().screen === null, 3000, "right edge back");
  check(true, "an edge swipe from the right returns to the chat");
  await a.click("phone.tab.more");
  await a.waitFor(() => ph().screen === "more", 3000, "more");
  await shot("phone-08-more");
  await a.click("phone.more.workspaces");
  await a.waitFor(() => ph().screen === "workspaces", 3000, "workspaces");
  await sleep(300);
  await shot("phone-09-workspaces");
  await a.click("phone.back");
  await a.click("phone.cost");
  await a.waitFor(() => ph().screen === "status", 3000, "status");
  await sleep(300);
  check(st().phoneStatus?.messages === "1770" || Number(st().phoneStatus?.messages) > 0, `the cost opens Status (everything from the status bar): ${JSON.stringify(st().phoneStatus)}`);
  await shot("phone-10-status");
  await a.click("phone.back");
  await a.click("phone.tab.notifications");
  await a.waitFor(() => ph().screen === "notifications", 3000, "alerts");
  await sleep(400);
  await shot("phone-11-alerts");
  await a.click("phone.back");

  // the title: tap → switcher; swipe → next open task
  await a.click("phone.title");
  await a.waitFor(() => st().overlay === "palette", 3000, "switcher");
  check(true, "a tap on the title opens the task switcher");
  await a.key("Escape");
  await a.switchTo(1);
  await a.switchTo(0);
  const before = st().current;
  const t = await a.waitProbe("phone.title");
  await a.cmd({ op: "drag", x0: t[0] + t[2] - 10, y0: t[1] + 10, x1: t[0] + 10, y1: t[1] + 10 });
  await a.waitFor(() => st().current !== before, 4000, "title swipe");
  check(true, `a swipe on the title goes to the next open task (#${before} → #${st().current})`);
  await shot("phone-12-next-task");
  // trackpad swipe right on the chat: the last panel
  await a.cmd({ op: "scroll", x: 200, y: 400, dx: 140, dy: 0 });
  await a.waitFor(() => ph().screen !== null, 3000, "trackpad swipe").catch(() => {});
  check(ph().screen !== null, `a two-finger swipe right opens the last panel (${ph().screen})`);
  // nothing scrolls sideways: the widest probe in the feed fits the window
  await a.click("phone.back").catch(() => {});
} catch (e) {
  check(false, String(e).slice(0, 400));
  await a.shot("phone-fail").catch(() => {});
} finally {
  await a.quit();
  eng.kill("SIGKILL");
  try { Deno.removeSync(dir, { recursive: true }); } catch { /* gone */ }
}
Deno.exit(failed() ? 1 : 0);
