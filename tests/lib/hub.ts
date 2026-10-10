// A second client of the OptChat hub, like a browser tab: one websocket to /ws on a TEST instance (?db=t<id>), the
// snapshot, the events, commands and rpc. Used to check that the GPUI app and "the browser" stay in sync.
// Deno must trust the local edge's certificate: run with DENO_TLS_CA_STORE=system (tests/e2e.ts sets it).
import { sleep } from "./cua.ts";

export const HUB = Deno.env.get("OPTCHAT_HUB") ?? "https://chat.reframe.dev";

export function checkTestDb(db: string) {
  if (!/^t[a-z0-9]{1,24}$/.test(db)) throw new Error(`not a test instance: ${db} (tests never use the live chat)`);
}

export class Peer {
  ws!: WebSocket;
  events: any[] = [];
  snapshot: any = null;
  open = false;
  #next = 1;
  #pending = new Map<number, (m: any) => void>();
  constructor(public db: string, public task = 0) { checkTestDb(db); }

  static async connect(db: string, task = 0): Promise<Peer> {
    const p = new Peer(db, task);
    await p.#connect();
    return p;
  }
  #connect(): Promise<void> {
    const host = HUB.replace(/^https:\/\//, "");
    this.snapshot = null;
    this.ws = new WebSocket(`wss://${host}/ws?task=${this.task}&db=${this.db}`, { headers: { origin: `https://${host}` } } as any);
    this.ws.onopen = () => { this.open = true; };
    this.ws.onclose = () => { this.open = false; };
    this.ws.onmessage = (ev) => {
      const m = JSON.parse(String(ev.data));
      if ((m.type === "rpc" || m.type === "controlDone") && m.id !== undefined && this.#pending.has(m.id)) {
        this.#pending.get(m.id)!(m);
        this.#pending.delete(m.id);
        return;
      }
      if (m.type === "snapshot") this.snapshot = m;
      this.events.push(m);
    };
    return this.waitFor(() => this.snapshot, 60_000, "peer snapshot");
  }
  send(m: Record<string, unknown>) { this.ws.send(JSON.stringify(m)); }
  rpc(method: string, params: Record<string, unknown> = {}): Promise<any> {
    const id = this.#next++;
    return new Promise((resolve, reject) => {
      this.#pending.set(id, (m) => m.error ? reject(new Error(m.error)) : resolve(m.result));
      this.send({ type: "rpc", id, method, params: { task: this.task, ...params } });
      setTimeout(() => { if (this.#pending.delete(id)) reject(new Error(`${method} timed out`)); }, 30_000);
    });
  }
  /** a control / spawn: resolves with the controlDone */
  command(m: Record<string, unknown>): Promise<any> {
    const id = this.#next++;
    return new Promise((resolve, reject) => {
      this.#pending.set(id, resolve);
      this.send({ ...m, id });
      setTimeout(() => { if (this.#pending.delete(id)) reject(new Error(`${m.type} timed out`)); }, 60_000);
    });
  }
  openTask(task: number) { this.task = task; this.snapshot = null; this.send({ type: "open", task }); return this.waitFor(() => this.snapshot?.task === task, 30_000, `peer open #${task}`); }
  /** events since index k matching f */
  since(k: number, f: (m: any) => boolean) { return this.events.slice(k).filter(f); }
  async waitFor<T>(f: () => T, ms = 30_000, what = "condition"): Promise<T> {
    const end = Date.now() + ms;
    while (Date.now() < end) {
      const v = f();
      if (v) return v;
      await sleep(100);
    }
    throw new Error(`peer: timed out waiting for ${what}`);
  }
  /** the test instance on Haiku (the copy and fresh instances default to Opus): root settings + compactor */
  async useHaiku(task = 0) {
    this.send({ type: "settings", task, patch: { model: "claude-haiku-5-5", effort: "low", compactorModel: "claude-haiku-5-5", compactorEffort: "low" } });
    await this.waitFor(() => this.events.some((m) => m.type === "task" && m.task?.id === task && m.task.settings?.model === "claude-haiku-5-5") || (this.snapshot?.task === task && this.events.some((m) => m.type === "settings" && m.settings?.model === "claude-haiku-5-5")), 20_000, "Haiku settings");
  }
  close() { try { this.ws.close(); } catch { /* */ } }
}
