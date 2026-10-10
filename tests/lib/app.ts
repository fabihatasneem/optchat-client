// The app under test: dist-dev/OptChat Dev.app (never the user's dist/OptChat.app) as a client of the OptChat engine, connected to TEST workspaces only
// (test-gpui-e / test-gpui-f, scoped tokens from data/test-tokens.json; never the live chat), with its prefs and its
// token store in a temp folder; driven through Computer Use with coordinates from the app's probe
// file, observed through its state file and screenshots.
import { BUNDLE, Cua, ROOT, sleep } from "./cua.ts";
import { checkTestDb, HUB, TOKENS } from "./hub.ts";

export const SHOTS = `${ROOT}/shots`;
const TITLEBAR = 32; // points above the content area
const SCALE = 2; // screenshot pixels per point (Retina)

/** the test workspace a suite uses: OPTCHAT_TEST_DB, else `fallback` (test-gpui-e / test-gpui-f only) */
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
  /** `db`: the test workspace on screen; `also`: more test workspaces connected in the background; `dir`: relaunch
   *  with a previous run's prefs (see quit({ keep: true })); `bare`: no engine or connection (the UI adds them) */
  static async launch(o: { db?: string; also?: string[]; env?: Record<string, string>; dir?: string; bare?: boolean; headless?: boolean } = {}): Promise<App> {
    const me = new App();
    me.dir = o.dir ?? await Deno.makeTempDir({ prefix: "optchat-e2e-" });
    me.db = o.db ?? testDb("test-gpui-e"); // a copy of the main chat on the engine (never the live chat)
    checkTestDb(me.db);
    for (const w of o.also ?? []) checkTestDb(w);
    try { Deno.removeSync(me.probes); } catch { /* none */ }
    try { Deno.removeSync(`${me.probes}.state.json`); } catch { /* none */ }
    const conn = o.bare ? {} : { OPTCHAT_ENGINE: HUB, OPTCHAT_ENGINE_NAME: "reframe.dev", OPTCHAT_CONNECT: [me.db, ...(o.also ?? [])].join(",") };
    const hl = o.headless || Deno.env.get("OPTCHAT_E2E_HEADLESS") ? { OPTCHAT_HEADLESS: "1", OPTCHAT_TEST_HOOKS: "1" } : {};
    // never the engine's real local client token: a test names its own file (or none)
    // never engine-wide settings (turns at once, dictation service) on the real engine: the app refuses them in tests
    const env = { ...hl, OPTCHAT_TEST_NO_GLOBAL: "1", OPTCHAT_LOCAL_TOKEN_FILE: `${me.dir}/no-local-token`, OPTCHAT_PROBES: me.probes, OPTCHAT_PREFS_DIR: me.dir, OPTCHAT_TOKEN_DIR: me.dir, OPTCHAT_TEST_TOKENS: TOKENS, ...conn, ...(o.env ?? {}) };
    const args = ["-n", "-W", "--stderr", `${me.dir}/stderr.txt`];
    for (const [k, v] of Object.entries(env)) args.push("--env", `${k}=${v}`);
    args.push(`${ROOT}/dist-dev/OptChat Dev.app`);
    await killLeftovers();
    me.proc = new Deno.Command("open", { args, stdout: "null", stderr: "null" }).spawn(); // -W: returns when the app quits
    // OPTCHAT_E2E_HEADLESS=1: every suite without Computer Use (a locked screen): input through the app's test
    // channel (native events to its own view), frames laid out by the app itself, no screenshots
    me.headless = !!o.headless || !!Deno.env.get("OPTCHAT_E2E_HEADLESS");
    if (!me.headless) me.cua = await Cua.start();
    if (o.bare) await me.waitFor(() => me.state()?.pid, 30_000, "app up");
    else await me.waitFor(() => me.state()?.ready && me.state()?.loaded, 30_000, "app ready");
    if (!me.headless) await me.cua.js(`globalThis.app = await cua.getApp("${BUNDLE}"); 1`);
    return me;
  }
  headless = false;
  #seq = 0;
  /** the test channel (OPTCHAT_TEST_HOOKS): commands run through the app's own handlers (crate::testcmd) */
  async cmd(...cmds: Record<string, unknown>[]) {
    const p = `${this.probes}.cmd`;
    for (let i = 0; i < 40; i++) { try { Deno.statSync(p); await sleep(100); } catch { break; } }
    const seq = this.#seq += cmds.length;
    Deno.writeTextFileSync(`${p}.tmp`, JSON.stringify(cmds.map((c, i) => ({ ...c, seq: seq - cmds.length + i + 1 }))));
    Deno.renameSync(`${p}.tmp`, p);
    // done when the app has run the last one (and published its state after it)
    for (let i = 0; i < 40; i++) { try { Deno.statSync(p); await sleep(50); } catch { break; } }
    for (let i = 0; i < 150; i++) {
      const lc = this.state()?.lastCmd;
      if ((lc?.seq ?? 0) >= seq || (i >= 4 && lc && lc.seq === undefined)) break; // (an older build: no seq)
      await sleep(50);
    }
    await sleep(120);
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
    if (this.headless) return await this.#actHeadless(code);
    for (let attempt = 0; ; attempt++) {
      const r = await this.cua.js(code);
      if (!r.isError) return r;
      if (attempt >= 1) throw new Error(`Computer Use: ${r.text.slice(0, 300)}`);
      console.log(`  (re-attaching Computer Use: ${r.text.slice(0, 120).replace(/\n/g, " ")})`);
      await this.cua.js(`globalThis.app = await cua.getApp("${BUNDLE}"); await app.getAXState(); 1`);
      await sleep(300);
    }
  }
  /** the few raw Computer Use calls the suites make, as test-channel input (screen pixels → points) */
  async #actHeadless(code: string): Promise<{ text: string; images: string[] }> {
    const pt = (a: string, b: string) => [Number(a) / SCALE, Number(b) / SCALE - TITLEBAR];
    let m = code.match(/app\.click\(\[(\d+), (\d+)\](?:, \{ mouseButton: "(\w+)" \})?\)/);
    if (m) { const [x, y] = pt(m[1], m[2]); await this.cmd({ op: "click", x, y, button: m[3] ?? "left" }); return { text: "", images: [] }; }
    m = code.match(/app\.drag\(\[(\d+), (\d+)\], \[(\d+), (\d+)\]\)/);
    if (m) { const [x0, y0] = pt(m[1], m[2]); const [x1, y1] = pt(m[3], m[4]); await this.cmd({ op: "drag", x0, y0, x1, y1 }); return { text: "", images: [] }; }
    if (/pressKey\("Escape"\)/.test(code)) { await this.cmd({ op: "key", keys: "Escape" }); return { text: "", images: [] }; }
    throw new Error(`headless: not available without Computer Use: ${code.slice(0, 80)}`);
  }
  px(x: number, y: number): [number, number] { return [Math.round(x * SCALE), Math.round((y + TITLEBAR) * SCALE)]; }
  async clickAt(x: number, y: number, opts = "") {
    if (this.headless) {
      const button = /right/.test(opts) ? "right" : /middle/.test(opts) ? "middle" : "left";
      await this.cmd({ op: "click", x, y, button });
      await sleep(150);
      return;
    }
    const [a, b] = this.px(x, y);
    await this.act(`await app.click([${a}, ${b}]${opts ? `, ${opts}` : ""}); 1`);
    await sleep(250);
  }
  /** click the center (or an offset) of a probed element */
  async click(name: string, o: { dx?: number; dy?: number; right?: boolean; wait?: number } = {}) {
    const p = await this.waitProbe(name, o.wait ?? 5000);
    await this.clickAt(p[0] + (o.dx ?? p[2] / 2), p[1] + (o.dy ?? p[3] / 2), o.right ? `{ mouseButton: "right" }` : "");
  }
  async type(text: string) {
    if (this.headless) { await this.cmd({ op: "type", text }); return; }
    await this.ensureActive(); await this.act(`await app.typeText(${JSON.stringify(text)}); 1`); await sleep(150); }
  /** keys reach only the key window: if another app took the focus, click a neutral spot (the status text) first */
  async ensureActive() {
    if (this.headless || this.state()?.active !== false) return;
    console.log("  (window inactive: clicking the status bar to activate it)");
    const p = this.probe("status.text") ?? this.probe("statusbar");
    if (p) await this.clickAt(p[0] + Math.min(p[2] / 2, 30), p[1] + p[3] / 2);
    await this.waitFor(() => this.state()?.active !== false, 3000, "window active").catch(() => {});
  }
  async key(k: string) {
    if (this.headless) { await this.cmd({ op: "key", keys: k }); await sleep(100); return; }
    await this.ensureActive(); await this.act(`await app.pressKey(${JSON.stringify(k)}); 1`); await sleep(200); }
  async scroll(name: string, dir: "up" | "down", pages = 3) {
    const p = await this.waitProbe(name);
    if (this.headless) {
      await this.cmd({ op: "scroll", x: p[0] + p[2] / 2, y: p[1] + p[3] / 2, dy: (dir === "up" ? 1 : -1) * 800 * pages });
      await sleep(300);
      return;
    }
    const [a, b] = this.px(p[0] + p[2] / 2, p[1] + p[3] / 2);
    await this.act(`await app.scroll([${a}, ${b}], "${dir}", ${pages}); 1`);
    await sleep(300);
  }
  async shot(name: string): Promise<string> {
    if (this.headless) {
      // a 1-pt resize makes AppKit draw and present the window even when covered; then a capture of our window only
      await this.cmd({ op: "nudge" });
      await sleep(400);
      const png = `${SHOTS}/${name}.png`, jpg = `${SHOTS}/${name}.jpg`;
      const r = await new Deno.Command(`${ROOT}/scripts/shot.sh`, { args: [png], stdout: "null", stderr: "null" }).output();
      if (!r.success) return `${jpg} (headless: no capture)`;
      await new Deno.Command("sips", { args: ["-s", "format", "jpeg", png, "--out", jpg], stdout: "null", stderr: "null" }).output();
      try { Deno.removeSync(png); } catch { /* */ }
      return jpg;
    }
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
      // not drawn yet: in the feed (opened at its bottom) it is above; elsewhere below
      const dir = p && box ? (p[1] < box[1] ? "up" : "down") : container === "feed" ? "up" : "down";
      if (this.headless && p && box) {
        // headless: scroll by what it takes to center it (Computer Use scrolls by pages)
        const dy = Math.max(-box[3] * 0.9, Math.min(box[3] * 0.9, box[1] + box[3] / 2 - (p[1] + Math.min(p[3], box[3]) / 2)));
        await this.cmd({ op: "scroll", x: box[0] + box[2] / 2, y: box[1] + box[3] / 2, dy });
      } else await this.scroll(container, dir, 1);
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
    if (this.headless) await this.cmd({ op: "quit" }).catch(() => {});
    else try { await this.cua.js(`await app.pressKey("cmd+q"); 1`); } catch { /* gone */ }
    // `open -W` returns once the app has exited; if it has not within 8 s, kill our own instance
    const exited = await Promise.race([this.proc!.status.then(() => true), sleep(8000).then(() => false)]);
    if (!exited) { console.log("app did not quit on cmd+q: killing it"); await killLeftovers(); await this.proc!.status; }
    this.cua?.close();
    await sleep(300);
    this.orphans = kids.filter((p) => new Deno.Command("kill", { args: ["-0", p], stderr: "null" }).outputSync().success);
    // the prefs folder (OPTCHAT_KEEP_E2E=1 keeps it for debugging)
    if (!o.keep && !Deno.env.get("OPTCHAT_KEEP_E2E")) { try { Deno.removeSync(this.dir, { recursive: true }); } catch { /* gone */ } }
  }
}

/** only instances of this project's bundle (never any other app) */
async function killLeftovers() {
  const pat = `${ROOT}/dist-dev/OptChat Dev.app/Contents/MacOS/`; // never the user's app in dist/
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
