// A local relay to the engine for the "engine goes away" test: http://127.0.0.1:<port> → https://chat.reframe.dev
// (websockets /engine/ws and /engine/stt piped both ways, GETs forwarded), Origin rewritten for the edge. Started and
// killed by tests/workspaces.ts; the real engine never goes down. Logs nothing about requests (tokens pass through).
//   deno run -A tests/lib/relay.ts <port>
const port = Number(Deno.args[0] ?? 8787);
const UP = Deno.env.get("OPTCHAT_HUB") ?? "https://chat.reframe.dev";
const host = UP.replace(/^https:\/\//, "");

Deno.serve({ hostname: "127.0.0.1", port, onListen: () => console.log(`relay on ${port}`) }, async (req) => {
  const url = new URL(req.url);
  const headers: Record<string, string> = { origin: UP };
  const auth = req.headers.get("authorization");
  if (auth) headers.authorization = auth;
  if (req.headers.get("upgrade")?.toLowerCase() === "websocket") {
    const up = new WebSocket(`wss://${host}${url.pathname}${url.search}`, { headers } as any);
    up.binaryType = "arraybuffer";
    const opened = await new Promise<boolean>((r) => { up.onopen = () => r(true); up.onerror = () => r(false); up.onclose = () => r(false); });
    if (!opened) return new Response("upstream refused", { status: 502 });
    const { socket, response } = Deno.upgradeWebSocket(req);
    socket.binaryType = "arraybuffer";
    const queue: any[] = [];
    up.onmessage = (e) => { if (socket.readyState === 1) socket.send(e.data); else queue.push(e.data); };
    socket.onopen = () => { for (const m of queue.splice(0)) socket.send(m); };
    socket.onmessage = (e) => { if (up.readyState === 1) up.send(e.data); };
    socket.onclose = () => { try { up.close(); } catch { /* */ } };
    up.onclose = () => { try { socket.close(); } catch { /* */ } };
    return response;
  }
  const r = await fetch(`${UP}${url.pathname}${url.search}`, { headers });
  return new Response(r.body, { status: r.status, headers: { "content-type": r.headers.get("content-type") ?? "application/octet-stream" } });
});
