// A fake OptChat engine on 127.0.0.1 for UI tests that need no real workspace (the Workspaces panel): the public list
// (GET /engine/workspaces, the first one marked default), and /engine/ws speaking just enough of the protocol (one
// socket, `connect {ws, task, token}` → hello + an empty main chat snapshot with unread notices, rpc answers,
// disconnect). Token "bad" is refused like the engine does (error "no token that reaches workspace …"); any other is
// let in. GET /test/revoke?ws=<name> drops that workspace from every socket ("disconnected", reason "token revoked").
// Nothing here is the real engine. Workspace names come from argv (never hard-coded in the app).
//   deno run -A tests/lib/fake-engine.ts <port> <name[:off]>...
const port = Number(Deno.args[0] ?? 18790);
const list = Deno.args.slice(1).map((a, i) => ({ name: a.split(":")[0], running: !a.endsWith(":off"), ...(i === 0 ? { default: true } : {}) }));
const sockets = new Set<WebSocket>();
const settings = { model: "claude-haiku-5-5", effort: "low", compactorModel: "claude-haiku-5-5", compactorEffort: "low", cwd: "/tmp", tools: { core: true, computer: false, mcp: false }, computerApproval: "ask", viewBudget: 128000, instructions: "", depth: 2 };
const now = new Date().toISOString();
const notices = (ws: string) => [
  { id: `${ws}-n1`, at: now, level: "warn", task: 0, key: "k1", text: `${ws}: a test warning from the fake engine`, count: 1, readAt: null },
  { id: `${ws}-n2`, at: now, level: "info", task: 0, key: "k2", text: `${ws}: settings: saved`, count: 1, readAt: null },
];
const read = new Set<string>();
const unread = (ws: string) => {
  const u = { ok: 0, info: 0, warn: 0, error: 0 } as Record<string, number>;
  for (const n of notices(ws)) if (!read.has(n.id)) u[n.level]++;
  return u;
};
const snapshot = (ws: string, task: number) => ({
  type: "snapshot", ws, version: "fake", build: "fake", task: 0, instance: ws,
  tasks: [{ task: { id: 0, parent: null, title: "", mode: "root", status: "idle", settings, createdAt: now, startedBy: "user" }, running: false, waiting: false, queue: 0, metrics: {} }],
  global: { maxTurns: 4, dictation: "claude" }, settings, models: [], tools: [], computerAvailable: false,
  log: [{ i: 0, kind: "talk", size: 40, date: now, text: `Hello from the fake engine (${ws}, task ${task}).` }], T: 1, G: 1,
  view: { parts: [], bytes: 0, T: 1, G: 1, batching: false, low: 0, high: 0, compaction: { lines: 0, bytes: 0 } },
  nodesCount: 0, compactor: { busy: [], failed: {} }, running: null, queue: [], approvals: [], serverLog: [], turns: [], requests: [], notices: unread(ws),
});

Deno.serve({ hostname: "127.0.0.1", port, onListen: () => console.log(`fake engine on ${port}`) }, (req) => {
  const url = new URL(req.url);
  if (url.pathname === "/engine/workspaces") return Response.json(list);
  if (url.pathname === "/test/revoke") {
    for (const s of sockets) s.send(JSON.stringify({ type: "disconnected", ws: url.searchParams.get("ws"), reason: "token revoked" }));
    return new Response("ok");
  }
  if (url.pathname !== "/engine/ws") return new Response("not found", { status: 404 });
  const { socket, response } = Deno.upgradeWebSocket(req);
  sockets.add(socket);
  socket.onclose = () => sockets.delete(socket);
  const send = (m: unknown) => { try { socket.send(JSON.stringify(m)); } catch { /* closed */ } };
  socket.onopen = () => send({ type: "workspaces", workspaces: list });
  socket.onmessage = (e) => {
    const m = JSON.parse(String(e.data));
    const ws = m.ws;
    if (m.type === "connect" && (!m.token || m.token === "bad")) {
      send({ type: "error", ws, message: `no token that reaches workspace ${ws}` });
    } else if (m.type === "connect" || m.type === "open") {
      if (m.type === "connect") send({ type: "hello", ws, version: "fake", build: "fake", running: true });
      send(snapshot(ws, m.task ?? 0));
    } else if (m.type === "disconnect") send({ type: "disconnected", ws });
    else if (m.type === "rpc") send({ type: "rpc", ws, id: m.id, result: m.method === "notices" ? notices(ws).map((n) => ({ ...n, readAt: read.has(n.id) ? now : null })) : [] });
    else if (m.type === "notices.read") {
      for (const n of notices(ws)) if (m.all || (m.ids ?? []).includes(n.id) || n.level === m.level) read.add(n.id);
      send({ type: "notices", ws, notices: notices(ws).map((n) => ({ ...n, readAt: read.has(n.id) ? now : null })), unread: unread(ws) });
    } else if (m.id !== undefined) send({ type: "controlDone", ws, id: m.id, ok: true, message: "saved" });
  };
  return response;
});
