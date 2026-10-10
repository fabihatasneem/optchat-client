// Quitting the client: ⌘Q exits promptly even while a dictation socket is open, leaves no child process, and the
// hub keeps serving (the engine, its turns and its logs live in the hub now). No model turn.
import { App, check, failed } from "./lib/app.ts";
import { ROOT, sleep } from "./lib/cua.ts";
import { Peer } from "./lib/hub.ts";
const a = await App.launch({ env: { OPTCHAT_TEST_HOOKS: "1", OPTCHAT_FAKE_MIC: `${ROOT}/tests/fixtures/dict.wav` } });
const st = () => a.state();
try {
  await a.palette("Test: mic tap"); // a dictation: a second socket to /stt, the fake microphone streaming
  await a.waitFor(() => st().dict === "listening", 15_000, "dictating");
  check(true, "a dictation is running (its own socket to /stt)");
} catch (e) { check(false, String(e).slice(0, 300)); }
const kids = a.children();
const t0 = performance.now();
await a.quit();
const ms = Math.round(performance.now() - t0);
check(kids.length === 0 && a.orphans.length === 0, `no child process before or after ⌘Q (${kids.length} / ${a.orphans.length})`);
check(ms < 9000, `⌘Q exits promptly with the sockets open (${ms} ms including Computer Use)`);
const peer = await Peer.connect(a.db, 0);
check(!!peer.snapshot && peer.snapshot.T > 0, `the hub keeps serving after the client quit (#0 has ${peer.snapshot?.T} entries)`);
peer.close();
await sleep(100);
Deno.exit(failed() ? 1 : 0);
