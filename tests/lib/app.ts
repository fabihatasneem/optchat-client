// The app under test: launched from dist/OptChat.app on a temp copy of the project's database, driven through
// Computer Use with coordinates from the app's probe file, observed through its state file and screenshots.
import { BUNDLE, Cua, ROOT, sleep } from "./cua.ts";

export const SHOTS = `${ROOT}/shots`;
const TITLEBAR = 32; // points above the content area
const SCALE = 2; // screenshot pixels per point (Retina)

export class App {
  cua!: Cua;
  probes = `${SHOTS}/probes.json`;
  dir = "";
  db = "";
  log: string[] = [];
  proc?: Deno.ChildProcess;
  static async launch(o: { db?: string; fresh?: boolean; env?: Record<string, string> } = {}): Promise<App> {
    const me = new App();
    me.dir = await Deno.makeTempDir({ prefix: "optchat-e2e-" });
    me.db = `${me.dir}/optchat.db`;
    if (o.fresh) {
      // an empty database
    } else {
      const src = o.db ?? `${ROOT}/data/optchat.db`;
      const r = await new Deno.Command("sqlite3", { args: [src, `.backup '${me.db}'`] }).output();
      if (!r.success) throw new Error("db copy failed");
    }
    try { Deno.removeSync(me.probes); } catch { /* none */ }
    try { Deno.removeSync(`${me.probes}.state.json`); } catch { /* none */ }
    const env = { OPTCHAT_REFRAME_MCP: "0", OPTCHAT_PROBES: me.probes, OPTCHAT_DB: me.db, ...(o.env ?? {}) };
    const args = ["-n", "-W"];
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
    while (Date.now() < end) {
      try { if (f()) return; } catch { /* retry */ }
      await sleep(150);
    }
    throw new Error(`timed out waiting for ${what}; state=${JSON.stringify(this.state())?.slice(0, 600)}`);
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
  async type(text: string) { await this.act(`await app.typeText(${JSON.stringify(text)}); 1`); await sleep(150); }
  async key(k: string) { await this.act(`await app.pressKey(${JSON.stringify(k)}); 1`); await sleep(200); }
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
  /** run a command palette entry: open, type, wait until the query is in, Return, wait until it closed */
  async palette(q: string) {
    await this.key("cmd+k");
    await this.waitFor(() => this.state().overlay === "palette", 4000, "palette");
    await this.type(q);
    await this.waitFor(() => this.state().palette?.query === q, 4000, `palette query "${q}"`);
    await this.key("Return");
    await this.waitFor(() => this.state().overlay !== "palette", 4000, "palette closed");
    await sleep(300);
  }
  async quit() {
    try { await this.cua.js(`await app.pressKey("cmd+q"); 1`); } catch { /* gone */ }
    // `open -W` returns once the app has exited; if it has not within 8 s, kill our own instance
    const exited = await Promise.race([this.proc!.status.then(() => true), sleep(8000).then(() => false)]);
    if (!exited) { console.log("app did not quit on cmd+q: killing it"); await killLeftovers(); await this.proc!.status; }
    this.cua.close();
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
