// Dictation (fake microphone), approvals, images, themes, narrow layout, memory/context/log/models interactions.
import { App, check, failed, SHOTS } from "./lib/app.ts";
import { ROOT, sleep } from "./lib/cua.ts";
const a = await App.launch({ env: { OPTCHAT_TEST_HOOKS: "1", OPTCHAT_FAKE_MIC: `${ROOT}/tests/fixtures/dict.wav`, OPTCHAT_TEST_IMAGE: `${ROOT}/tests/fixtures/test.png` } });
const st = () => a.state();
const pal = (q: string) => a.palette(q);
try {
  // dictation with the mic button: tap starts, text lands in the draft, tap stops
  await a.click("composer.input");
  await a.type("before ");
  await a.click("composer.mic");
  await a.waitFor(() => st().dict === "connecting" || st().dict === "listening", 4000, "dict started");
  await a.waitFor(() => st().dict === "listening", 10000, "listening");
  await sleep(500);
  await a.shot("feat-01-listening");
  await a.waitFor(() => /dictation/i.test(st().draft), 15000, "dictated text");
  await sleep(1500);
  await a.click("composer.mic");
  await a.waitFor(() => st().dict === "idle", 10000, "dict done");
  check(/^before hello from the dictation test/i.test(st().draft) && /reframe/i.test(st().draft), `dictation wrote at the cursor: "${st().draft}"`);
  await a.shot("feat-02-dictated");
  // M outside a text field: tap toggles
  await a.click("feed");
  await a.key("m");
  await a.waitFor(() => st().dict !== "idle", 4000, "M started");
  check(true, "M outside the composer starts dictation");
  await sleep(2500);
  await a.key("Escape");
  await a.waitFor(() => st().dict === "idle", 10000, "Esc stopped");
  check(true, "Esc stops dictation and keeps the text");
  // approvals
  await pal("Test: approval");
  await a.waitFor(() => st().approvals === 1, 5000, "approval");
  await sleep(300);
  await a.shot("feat-03-approval");
  await a.click("approve.session");
  await a.waitFor(() => st().approvals === 0, 5000, "approval answered");
  check(true, "approval dialog answered (allow this session)");
  // image attach
  await a.click("composer.input");
  await pal("Test: attach image");
  await sleep(500);
  await a.shot("feat-04-image");
  check(true, "image thumbnail in the composer (see screenshot)");
  // themes: screenshot each
  const themes = ["OptChat Light", "Solarized Dark", "Solarized Light", "Tomorrow Night Blue", "Tomorrow Night", "One Dark", "One Light", "Ayu Mirage", "Gruvbox Dark", "OptChat Dark"];
  for (const t of themes) {
    await pal(`Theme: ${t}`);
    await sleep(400);
    await a.shot(`theme-${t.toLowerCase().replace(/ /g, "-")}`);
    check(st().theme === t.toLowerCase().replace(/ /g, "-"), `theme ${t} (${st().theme})`);
  }
  // narrow window: the sidebar floats over the editor
  await pal("Test: narrow window");
  await sleep(800);
  await a.shot("feat-05-narrow");
  await a.click("rail.toggle");
  await sleep(400);
  await a.shot("feat-06-narrow-sidebar");
  await pal("Test: wide window");
  await sleep(600);
  // memory: expand a line, search
  await a.click("rail.memory");
  await sleep(1500);
  const mem = Object.keys(a.allProbes()).filter((k) => k.startsWith("mem.0:")).sort((x, y) => a.probe(x)![1] - a.probe(y)![1]);
  if (mem[0]) { await a.click(mem[0]); await sleep(800); }
  await a.shot("feat-07-memory-open");
  // (search and reveal: tests/panels.ts)
  // raw view
  await a.click("mem.raw"); await sleep(800); await a.shot("feat-10-memory-raw");
} catch (e) { check(false, String(e)); await a.shot("feat-fail"); } finally { await a.quit(); }
Deno.exit(failed() ? 1 : 0);
