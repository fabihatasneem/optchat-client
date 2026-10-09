// The engine log file; quitting stops every child process: claude turns/compactions (summaries start at launch on the copied chat),
// the codex app-server, bash tools. No model turn is started here; launch-time compactions use Haiku.
import { App, check, failed } from "./lib/app.ts";
import { sleep } from "./lib/cua.ts";
const a = await App.launch();
let kids: string[] = [];
for (let i = 0; i < 60 && kids.length === 0; i++) { await sleep(500); kids = a.children(); }
const names = kids.map((p) => new TextDecoder().decode(new Deno.Command("ps", { args: ["-o", "comm=", "-p", p] }).outputSync().stdout).trim().split("/").pop());
check(kids.length > 0, `the app runs child processes before quitting (${kids.length}: ${names.join(", ")})`);
let log = "";
try { log = Deno.readTextFileSync(`${a.dir}/optchat-gpui/engine.log`); } catch { /* none */ }
check(/database .* build /.test(log), `the engine log file is written (${log.split("\n").filter(Boolean).length} lines in optchat-gpui/engine.log)`);
await a.quit();
check(a.orphans.length === 0, `none is left after ⌘Q (${a.orphans.length} left)`);
Deno.exit(failed() ? 1 : 0);
