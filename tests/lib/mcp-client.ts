// Minimal MCP client: stdio (newline-delimited JSON-RPC) and streamable HTTP (JSON or SSE replies).
// Supports server->client elicitation requests (used by cua_repl for per-app approval).

export type McpTool = { name: string; description?: string; inputSchema: Record<string, unknown> };
export type McpContent = { type: string; text?: string; data?: string; mimeType?: string };
export type McpCallResult = { content?: McpContent[]; isError?: boolean; structuredContent?: unknown };
export type ElicitHandler = (params: Record<string, any>) => Promise<{ action: "accept" | "decline" | "cancel"; content?: unknown; _meta?: unknown }>;

const PROTOCOL = "2025-06-18";

export interface McpClient {
  listTools(): Promise<McpTool[]>;
  callTool(name: string, args: unknown, meta?: Record<string, unknown>, timeoutMs?: number): Promise<McpCallResult>;
  close(): void;
  readonly alive: boolean;
}

export async function stdioClient(opts: { command: string; args: string[]; env: Record<string, string>; onElicit?: ElicitHandler; onStderr?: (s: string) => void }): Promise<McpClient> {
  const p = new Deno.Command(opts.command, { args: opts.args, env: opts.env, clearEnv: true, stdin: "piped", stdout: "piped", stderr: "piped" }).spawn();
  let alive = true;
  p.status.then(() => { alive = false; for (const [, w] of pending) w.reject(new Error("MCP server exited")); pending.clear(); });
  const writer = p.stdin.getWriter();
  const enc = new TextEncoder();
  const send = (m: unknown) => writer.write(enc.encode(JSON.stringify(m) + "\n"));
  const pending = new Map<number, { resolve: (v: any) => void; reject: (e: Error) => void }>();
  let next = 1;
  (async () => {
    let buf = "";
    for await (const chunk of p.stdout.pipeThrough(new TextDecoderStream() as unknown as TransformStream<Uint8Array, string>)) {
      buf += chunk;
      let i;
      while ((i = buf.indexOf("\n")) >= 0) {
        const line = buf.slice(0, i).trim(); buf = buf.slice(i + 1);
        if (!line) continue;
        let m: any; try { m = JSON.parse(line); } catch { continue; }
        if (m.method && m.id !== undefined) {
          // server -> client request
          let result: unknown = {};
          try {
            if (m.method === "elicitation/create") result = opts.onElicit ? await opts.onElicit(m.params ?? {}) : { action: "decline" };
            else if (m.method === "ping") result = {};
            else { await send({ jsonrpc: "2.0", id: m.id, error: { code: -32601, message: "unsupported" } }); continue; }
          } catch (e) { result = { action: "decline", _meta: { error: String(e) } }; }
          await send({ jsonrpc: "2.0", id: m.id, result });
        } else if (m.id !== undefined && pending.has(m.id)) {
          const w = pending.get(m.id)!; pending.delete(m.id);
          m.error ? w.reject(new Error(m.error.message ?? "MCP error")) : w.resolve(m.result);
        }
      }
    }
  })();
  (async () => { for await (const c of p.stderr.pipeThrough(new TextDecoderStream() as unknown as TransformStream<Uint8Array, string>)) opts.onStderr?.(c); })();
  const request = (method: string, params: unknown, timeoutMs = 120_000) => new Promise<any>((resolve, reject) => {
    const id = next++;
    const t = setTimeout(() => { pending.delete(id); reject(new Error(`MCP ${method} timed out`)); }, timeoutMs);
    pending.set(id, { resolve: (v) => { clearTimeout(t); resolve(v); }, reject: (e) => { clearTimeout(t); reject(e); } });
    send({ jsonrpc: "2.0", id, method, params });
  });
  await request("initialize", { protocolVersion: PROTOCOL, capabilities: { elicitation: { form: {} } }, clientInfo: { name: "optchat", version: "0.1.0" } }, 30_000);
  await send({ jsonrpc: "2.0", method: "notifications/initialized" });
  return {
    get alive() { return alive; },
    async listTools() { return (await request("tools/list", {})).tools ?? []; },
    async callTool(name, args, meta, timeoutMs) { return await request("tools/call", { name, arguments: args, ...(meta ? { _meta: meta } : {}) }, timeoutMs ?? 300_000); },
    close() { try { p.kill(); } catch { /* gone */ } },
  };
}

export async function httpClient(opts: { url: string; token?: () => Promise<string | null> }): Promise<McpClient> {
  let session: string | null = null;
  let next = 1;
  const post = async (body: Record<string, unknown>, timeoutMs = 120_000) => {
    const headers: Record<string, string> = { "content-type": "application/json", accept: "application/json, text/event-stream", "mcp-protocol-version": PROTOCOL };
    const tok = await opts.token?.();
    if (tok) headers.authorization = `Bearer ${tok}`;
    if (session) headers["mcp-session-id"] = session;
    const res = await fetch(opts.url, { method: "POST", headers, body: JSON.stringify(body), signal: AbortSignal.timeout(timeoutMs) });
    session = res.headers.get("mcp-session-id") ?? session;
    if (res.status === 401) throw Object.assign(new Error("unauthorized"), { status: 401 });
    if (res.status === 202 || body.id === undefined) { await res.body?.cancel(); return null; }
    if (!res.ok) throw new Error(`MCP HTTP ${res.status}: ${(await res.text()).slice(0, 300)}`);
    const type = res.headers.get("content-type") ?? "";
    if (type.includes("text/event-stream")) {
      const text = await res.text();
      for (const block of text.split(/\n\n/)) {
        const data = block.split("\n").filter((l) => l.startsWith("data:")).map((l) => l.slice(5).trim()).join("\n");
        if (!data) continue;
        const m = JSON.parse(data);
        if (m.id === body.id) { if (m.error) throw new Error(m.error.message); return m.result; }
      }
      throw new Error("MCP: no response in event stream");
    }
    const m = await res.json();
    if (m.error) throw new Error(m.error.message);
    return m.result;
  };
  await post({ jsonrpc: "2.0", id: next++, method: "initialize", params: { protocolVersion: PROTOCOL, capabilities: {}, clientInfo: { name: "optchat", version: "0.1.0" } } });
  await post({ jsonrpc: "2.0", method: "notifications/initialized" });
  return {
    alive: true,
    async listTools() { return (await post({ jsonrpc: "2.0", id: next++, method: "tools/list", params: {} }))?.tools ?? []; },
    async callTool(name, args, meta, timeoutMs) {
      return await post({ jsonrpc: "2.0", id: next++, method: "tools/call", params: { name, arguments: args, ...(meta ? { _meta: meta } : {}) } }, timeoutMs ?? 300_000);
    },
    close() {},
  };
}
