// The app under test: dist/OptChat.app as a client of the OptChat hub on a TEST instance (OPTCHAT_HUB_DB=t<id>, never
// the live chat), with its prefs in a temp folder; driven through Computer Use with coordinates from the app's probe
// file, observed through its state file and screenshots.
import { BUNDLE, Cua, ROOT, sleep } from "./cua.ts";
import { checkTestDb } from "./hub.ts";

export const SHOTS = `${ROOT}/shots`;
const TITLEBAR = 32; // points above the content area
const SCALE = 2; // screenshot pixels per point (Retina)

/** the test instance a suite uses: OPTCHAT_TEST_DB (e.g. a live-sized copy seeded for the run), else `fallback` */
export function testDb(fallback: string): string {
  const db = Deno.env.get("OPTCHAT_TEST_DB") ?? fallback;
  checkTestDb(db);
  return db;
}

export class App {
  cua!: Cua;
  probes = `${SHOTS}/probes.json`;
  dir = "";
  db = "";
  log: string[] = [];
  proc?: Deno.ChildProcess;
  /** `db`: the hub test instance (t<id>); `dir`: relaunch with a previous run's prefs (see quit({ keep: true })) */
  static async launch(o: { db?: string; env?: Record<string, string>; dir?: string } = {}): Promise<App> {
    const me = new App();
    me.dir = o.dir ?? await Deno.makeTempDir({ prefix: "optchat-e2e-" });
    me.db = o.db ?? testDb("tgpuie2e");
    checkTestDb(me.db);
    try { Deno.removeSync(me.probes); } catch { /* none */ }
    try { Deno.removeSync(`${me.probes}.state.json`); } catch { /* none */ }
    const env = { OPTCHAT_PROBES: me.probes, OPTCHAT_HUB_DB: me.db, OPTCHAT_PREFS_DIR: me.dir, ...(o.env ?? {}) };
    const args = ["-n", "-W", "--stderr", `${me.dir}/stderr.txt`];
    for (const [k, v] of Object.entries(env)) args.push("--env", `${k}=${v}`);
    args.push(`${ROOT}/dist/OptChat.app`);
    await killLeftovers();
    me.proc = new Deno.Command("open", { args, stdout: "null", stderr: "null" }).spawn(); // -W: returns when the app quits
    me.cua = await Cua.start();
    await me.waitFor(() => me.state()?.ready && me.state()?.loaded, 30_000, "app ready");
    await me.cua.js(`globalThis.app = await cua.getApp("${BUNDLE}"); 1`);
    return me;
  }
  state(): any {
    try { return JSON.parse(Deno.readTextFileSync(`${this.probes}.state.json`)); } catch { return null; }
  }
  allProbes(): Record<string, [number, number, number, number]> {
    try { return JSON.parse(Deno.readTextFileSync(this.probes)); } catch { return {}; }
  }
  probe(name: string) { return this.allProbes()[name]; }
  async waitProbe(name: string, ms = 5000) {
    await this.waitFor(() => !!this.probe(name), ms, `probe ${name}`);
    return this.probe(name)!;
  }
  async waitFor(f: () => unknown, ms = 10_000, what = "condition") {
    const end = Date.now() + ms;
    const frame0 = this.state()?.frame;
    while (Date.now() < end) {
      try { if (f()) return; } catch { /* retry */ }
      await sleep(150);
    }
    const s = this.state() ?? {};
    // macOS stops drawing a fully covered window (GPUI stops its display link): keys then go by the last frame
    const stalled = frame0 !== undefined && s.frame === frame0 && ms >= 2000;
    const brief = { stalled, active: s.active, focus: s.focus, frame: s.frame, current: s.current, loaded: s.loaded, view: s.view, overlay: s.overlay, palette: s.palette, ask: s.ask, sideOpen: s.sideOpen, status: s.status };
    throw new Error(`timed out waiting for ${what}${stalled ? " [window not drawing: covered by other windows?]" : ""}; state=${JSON.stringify(brief)}`);
  }
  /** run a Computer Use action; if the session lost the app, attach again (getApp + fresh state) and retry once */
  async act(code: string): Promise<{ text: string; images: string[] }> {
    for (let attempt = 0; ; attempt++) {
      const r = await this.cua.js(code);
      if (!r.isError) return r;
      if (attempt >= 1) throw new Error(`Computer Use: ${r.text.slice(0, 300)}`);
      console.log(`  (re-attaching Computer Use: ${r.text.slice(0, 120).replace(/\n/g, " ")})`);
      await this.cua.js(`globalThis.app = await cua.getApp("${BUNDLE}"); await app.getAXState(); 1`);
      await sleep(300);
    }
  }
  px(x: number, y: number): [number, number] { return [Math.round(x * SCALE), Math.round((y + TITLEBAR) * SCALE)]; }
  async clickAt(x: number, y: number, opts = "") {
    const [a, b] = this.px(x, y);
    await this.act(`await app.click([${a}, ${b}]${opts ? `, ${opts}` : ""}); 1`);
    await sleep(250);
  }
  /** click the center (or an offset) of a probed element */
  async click(name: string, o: { dx?: number; dy?: number; right?: boolean; wait?: number } = {}) {
    const p = await this.waitProbe(name, o.wait ?? 5000);
    await this.clickAt(p[0] + (o.dx ?? p[2] / 2), p[1] + (o.dy ?? p[3] / 2), o.right ? `{ mouseButton: "right" }` : "");
  }
  async type(text: string) { await this.ensureActive(); await this.act(`await app.typeText(${JSON.stringify(text)}); 1`); await sleep(150); }
  /** keys reach only the key window: if another app took the focus, click a neutral spot (the status text) first */
  async ensureActive() {
    if (this.state()?.active !== false) return;
    console.log("  (window inactive: clicking the status bar to activate it)");
    const p = this.probe("status.text") ?? this.probe("statusbar");
    if (p) await this.clickAt(p[0] + Math.min(p[2] / 2, 30), p[1] + p[3] / 2);
    await this.waitFor(() => this.state()?.active !== false, 3000, "window active").catch(() => {});
  }
  async key(k: string) { await this.ensureActive(); await this.act(`await app.pressKey(${JSON.stringify(k)}); 1`); await sleep(200); }
  async scroll(name: string, dir: "up" | "down", pages = 3) {
    const p = await this.waitProbe(name);
    const [a, b] = this.px(p[0] + p[2] / 2, p[1] + p[3] / 2);
    await this.act(`await app.scroll([${a}, ${b}], "${dir}", ${pages}); 1`);
    await sleep(300);
  }
  async shot(name: string): Promise<string> {
    const r = await this.act(`await app.getScreenshot()`);
    const path = `${SHOTS}/${name}.jpg`;
    if (r.images[0]) Deno.writeFileSync(path, Uint8Array.from(atob(r.images[0]), (ch) => ch.charCodeAt(0)));
    return path;
  }
  /** scroll `container` (a probe name) until probe `name` is fully inside it; returns its bounds */
  async reveal(name: string, container = "feed") {
    for (let i = 0; i < 14; i++) {
      const p = this.probe(name), box = this.probe(container);
      if (p && box && p[1] >= box[1] && p[1] + p[3] <= box[1] + box[3]) return p;
      if (p && box && p[3] > box[3] && p[1] <= box[1] + box[3] / 2) return p; // taller than the box: its top half is in
      if (Deno.env.get("OPTCHAT_E2E_DEBUG")) console.log("reveal", name, p, box);
      await this.scroll(container, p && box && p[1] < box[1] ? "up" : "down", 1);
      await sleep(350);
    }
    throw new Error(`${name} not visible in ${container}`);
  }
  /** run a command palette entry: open, type, wait until the query is in, Return, wait until it closed.
   *  One retry (after Esc and re-activating the window): another app can take the key focus mid-run. */
  async palette(q: string, opener = "cmd+k") {
    for (let attempt = 0; ; attempt++) {
      try {
        await this.key(opener);
        await this.waitFor(() => this.state().overlay === "palette", 3000, "palette");
        await this.type(q);
        await this.waitFor(() => this.state().palette?.query === q, 3000, `palette query "${q}"`);
        await this.key("Return");
        await this.waitFor(() => this.state().overlay !== "palette", 3000, "palette closed");
        await sleep(300);
        return;
      } catch (e) {
        if (attempt >= 1) throw e;
        console.log(`  (palette "${q}" retry: ${String(e).slice(0, 80)})`);
        await this.act(`await app.pressKey("Escape"); 1`);
        const p = this.probe("status.text") ?? this.probe("statusbar");
        if (p) await this.clickAt(p[0] + Math.min(p[2] / 2, 30), p[1] + p[3] / 2);
        await sleep(400);
      }
    }
  }
  /** open a task with the switcher (⌘T) and wait until its data is shown */
  async switchTo(id: number) {
    await this.palette(`#${id} `, "cmd+t");
    await this.waitFor(() => this.state().current === id && this.state().loaded, 10000, `open #${id}`);
  }
  /** the app's stderr so far (engine log lines, OPTCHAT_TIMING output) */
  stderr(): string { try { return Deno.readTextFileSync(`${this.dir}/stderr.txt`); } catch { return ""; } }
  /** child processes still alive after the last quit (claude, codex, bash): should always be empty */
  orphans: string[] = [];
  children(): string[] {
    const pid = this.state()?.pid;
    if (!pid) return [];
    const r = new Deno.Command("pgrep", { args: ["-P", String(pid)] }).outputSync();
    return new TextDecoder().decode(r.stdout).split("\n").filter(Boolean);
  }
  async quit(o: { keep?: boolean } = {}) {
    const kids = this.children();
    try { await this.cua.js(`await app.pressKey("cmd+q"); 1`); } catch { /* gone */ }
    // `open -W` returns once the app has exited; if it has not within 8 s, kill our own instance
    const exited = await Promise.race([this.proc!.status.then(() => true), sleep(8000).then(() => false)]);
    if (!exited) { console.log("app did not quit on cmd+q: killing it"); await killLeftovers(); await this.proc!.status; }
    this.cua.close();
    await sleep(300);
    this.orphans = kids.filter((p) => new Deno.Command("kill", { args: ["-0", p], stderr: "null" }).outputSync().success);
    // the prefs folder (OPTCHAT_KEEP_E2E=1 keeps it for debugging)
    if (!o.keep && !Deno.env.get("OPTCHAT_KEEP_E2E")) { try { Deno.removeSync(this.dir, { recursive: true }); } catch { /* gone */ } }
  }
}

/** only instances of this project's bundle (never any other app) */
async function killLeftovers() {
  const pat = `${ROOT}/dist/OptChat.app/Contents/MacOS/`;
  const r = await new Deno.Command("pgrep", { args: ["-f", pat] }).output();
  const pids = new TextDecoder().decode(r.stdout).split("\n").filter(Boolean);
  for (const pid of pids) await new Deno.Command("kill", { args: [pid] }).output();
  if (pids.length) await sleep(1500);
}

let failures = 0;
export const results: string[] = [];
export function check(ok: unknown, what: string) {
  const line = `${ok ? "ok  " : "FAIL"} ${what}`;
  console.log(line);
  results.push(line);
  if (!ok) failures++;
  return !!ok;
}
export const failed = () => failures;
