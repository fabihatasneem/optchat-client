// The whole UI suite against dist/OptChat.app (build it first: scripts/bundle.sh release).
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
//   perf      launch, scrolling frame times on 1749 messages, task switch, memory (release build)
import { ROOT } from "./lib/cua.ts";
const ALL = [
  { name: "panels", models: false },
  { name: "features", models: false },
  { name: "smoke", models: true },
  { name: "flow", models: true },
  { name: "perf", models: false },
];
const args = Deno.args.filter((x) => !x.startsWith("--"));
const offline = Deno.args.includes("--offline");
const suites = ALL.filter((s) => (args.length ? args.includes(s.name) : true) && !(offline && s.models));
const out: string[] = [`e2e ${new Date().toISOString()}`];
let bad = 0;
for (const s of suites) {
  const t0 = performance.now();
  console.log(`\n== ${s.name}`);
  const p = new Deno.Command("deno", { args: ["run", "-A", `${ROOT}/tests/${s.name}.ts`], stdout: "piped", stderr: "piped" }).spawn();
  const r = await p.output();
  const text = new TextDecoder().decode(r.stdout) + new TextDecoder().decode(r.stderr);
  const lines = text.split("\n").filter((l) => /^(ok|FAIL) /.test(l));
  for (const l of lines) console.log(l.slice(0, 220));
  if (s.name === "perf") console.log(text.slice(text.indexOf("{")).slice(0, 900));
  const fails = lines.filter((l) => l.startsWith("FAIL")).length + (r.success ? 0 : lines.some((l) => l.startsWith("FAIL")) ? 0 : 1);
  bad += fails;
  out.push(`\n== ${s.name}: ${lines.length - lines.filter((l) => l.startsWith("FAIL")).length} ok, ${fails} failed, ${Math.round((performance.now() - t0) / 1000)} s`, ...lines);
}
out.push(`\nTOTAL: ${bad ? `${bad} failed` : "all passed"}`);
Deno.writeTextFileSync(`${ROOT}/shots/e2e-results.txt`, out.join("\n") + "\n");
console.log(`\n${bad ? `${bad} failed` : "all passed"} (shots/e2e-results.txt)`);
Deno.exit(bad ? 1 : 0);
