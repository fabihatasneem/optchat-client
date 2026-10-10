// The whole UI suite against dist/OptChat.app, a client of the OptChat hub on TEST instances only (OPTCHAT_TEST_DB,
// default test-gpui-c, a copy of the main chat; sync.ts and the background checks use test-gpui-d) (build it first: scripts/bundle.sh release).
//   deno run -A tests/e2e.ts            all suites (model turns use Claude Haiku and Codex GPT-6-Luna only)
//   deno run -A tests/e2e.ts --offline  skip the suites that call models
//   deno run -A tests/e2e.ts panels     only the named suites
// Each suite launches the app on its own temp copy of data/optchat.db and quits it. Results: shots/e2e-results.txt
//
// PARITY areas covered:
//   smoke     launch on the real chat copy, palette model switch, streamed turns (Claude + Codex)
//   flow      new-task dialog, spawn + tab, tool card + foot chips, report card in the parent, pending Add / ⌘Enter
//             append / Run, pause / resume (ask bar), close as done + end bar, cancel
//   features  dictation (fake mic: button tap, M key, Esc), approval dialog, image attach, 10 themes live
//   panels    ⌘1–⌘6 + ⌘B, zoom, status bar model / effort menus, budgets popover, models rate edit (saved to the DB),
//             request log + subtree, context debugger + wire, settings tabs, narrow window sheet + Esc, context menu,
//             tab switcher / ctrl-tab / ⌘W, memory search + reveal
//   chat      tool card expand, step table, context ↗ → Context view, image lightbox, Continue as a new task, new-task
//             mode cards, task row / tab context menus (existing history, no model calls)
//   inputs    composer auto-grow cap, title ≤ 80, instruction box grows, close ≤ 280 / ask ≤ 2000, relaunch restores
//             tabs, current task, placeholder, window size
//   quit      ⌘Q leaves no child process behind (claude compactions, codex, bash)
//   perf      launch, scrolling frame times on 1749 messages, task switch, memory (release build)
//   parity    no model calls: skeletons, rail, memory summary / failed jobs / cview / raw, context image rows + follow
//             latest, log breakdown / filters / row → context, models providers / refresh / hidden / history, text size,
//             appearance follow (hook), toasts 2.6 / 6 s, error line (dictation 6 s, engine stays), mic tap / hold /
//             states / audio.ms metric, composer paste / drop (1568 px) / 20 cap / ×, dialog images, continue select
//             + continues link, task rows (open, no subtasks, chevron animation, dot tooltip), ⌘⇧[ ⌘⇧] ⌘L ⌘, middle-click,
//             focus on switch, disabled while loading, sidebar drag 220–760, budget bar, palette lifecycle commands,
//             menu bar (AX), load earlier + follow mode, report expand, v2 work rows, markdown blocks, cut text,
//             turn error, echo images, dictation restart, child / root empty states (fresh database)
//   live      Haiku children: ⌘Enter in the dialog, Context "still running", rail badge + memory dot, status texts
//             (running, settling, waiting for a slot, #id mode, N working, N queued), live text + thinking, follow
//             mode while streaming, ⌘. stop, Enter / ⌘Enter while dictating, "waiting for summaries", compaction jobs
//             + memory rows refreshing
import { ROOT } from "./lib/cua.ts";
const ALL = [
  { name: "panels", models: false },
  { name: "features", models: false },
  { name: "chat", models: false },
  { name: "inputs", models: false },
  { name: "quit", models: false },
  { name: "smoke", models: true },
  { name: "flow", models: true },
  { name: "perf", models: false },
  { name: "parity", models: false },
  { name: "live", models: true },
  { name: "sync", models: true },
  { name: "engine", models: true },
  { name: "panel", models: false },
  { name: "workspaces", models: true },
  { name: "localtoken", models: false },
];
const args = Deno.args.filter((x) => !x.startsWith("--"));
const offline = Deno.args.includes("--offline");
const suites = ALL.filter((s) => (args.length ? args.includes(s.name) : true) && !(offline && s.models));
const out: string[] = [`e2e ${new Date().toISOString()}`];
let bad = 0;
const run = async (name: string) => {
  // the hub's edge certificate comes from the macOS trust store; history suites use the live-sized test copy
  const env = { DENO_TLS_CA_STORE: "system", OPTCHAT_TEST_DB: Deno.env.get("OPTCHAT_TEST_DB") ?? "test-gpui-c" };
  const p = new Deno.Command("deno", { args: ["run", "-A", `${ROOT}/tests/${name}.ts`], env, stdout: "piped", stderr: "piped" }).spawn();
  const r = await p.output();
  const text = new TextDecoder().decode(r.stdout) + new TextDecoder().decode(r.stderr);
  const lines = text.split("\n").filter((l) => /^(ok|FAIL) /.test(l));
  const fails = lines.filter((l) => l.startsWith("FAIL")).length + (r.success || lines.some((l) => l.startsWith("FAIL")) ? 0 : 1);
  return { text, lines, fails };
};
for (const s of suites) {
  const t0 = performance.now();
  console.log(`\n== ${s.name}`);
  let r = await run(s.name);
  // macOS stops drawing a window that other windows cover, and keys then go by the last drawn frame: when the
  // machine is in use a suite can fail for that reason alone. Offline suites get one retry; both runs are reported.
  let note = "";
  if (r.fails && !s.models) {
    console.log(`  (${r.fails} failed: retrying once)`);
    const first = r.fails;
    r = await run(s.name);
    note = ` (retry; first run had ${first} failure${first > 1 ? "s" : ""})`;
  }
  for (const l of r.lines) console.log(l.slice(0, 220));
  if (s.name === "perf") console.log(r.text.slice(r.text.indexOf("{")).slice(0, 1200));
  // failures that all happened while the window was not drawing say nothing about the app: reported as skipped
  const fl = r.lines.filter((l) => l.startsWith("FAIL"));
  const stalled = fl.length > 0 && fl.every((l) => l.includes("window not drawing"));
  if (stalled) note += " — SKIPPED: the window was covered (not drawing) when these failed";
  else bad += r.fails;
  out.push(`\n== ${s.name}: ${r.lines.length - fl.length} ok, ${r.fails} failed, ${Math.round((performance.now() - t0) / 1000)} s${note}`, ...r.lines);
}
out.push(`\nTOTAL: ${bad ? `${bad} failed` : "all passed"}`);
// OPTCHAT_E2E_HEADLESS: input through the app's test channel, no Computer Use, no screenshots (a locked screen)
const file = Deno.env.get("OPTCHAT_E2E_HEADLESS") ? "e2e-results-headless.txt" : "e2e-results.txt";
if (Deno.env.get("OPTCHAT_E2E_HEADLESS")) out.splice(1, 0, "headless: input through the test channel (native events to the app's own view), frames laid out by the app, no screenshots");
Deno.writeTextFileSync(`${ROOT}/shots/${file}`, out.join("\n") + "\n");
console.log(`\n${bad ? `${bad} failed` : "all passed"} (shots/${file})`);
Deno.exit(bad ? 1 : 0);
