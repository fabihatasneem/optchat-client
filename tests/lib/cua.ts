// Drives the built OptChat.app with ChatGPT's Computer Use (cua_repl over MCP stdio). Only our own app is ever
// approved; every other approval is declined. Element positions come from the app's probe file (OPTCHAT_PROBES).
import { type McpClient, stdioClient } from "./mcp-client.ts";

const R = "/Applications/ChatGPT.app/Contents/Resources";
const NM = `${R}/cua_node/lib/node_modules`;
const HOME = Deno.env.get("HOME") ?? "";
export const BUNDLE = "com.reframe.optchat.gpui.dev"; // the dev build (dist-dev), never the user's app
export const ROOT = new URL("../..", import.meta.url).pathname.replace(/\/$/, "");

export type Rect = [number, number, number, number];

export class Cua {
  c!: McpClient;
  session = crypto.randomUUID();
  n = 0;
  elicits: string[] = [];
  static async start(): Promise<Cua> {
    const me = new Cua();
    me.c = await stdioClient({
      command: `${R}/cua_node/bin/node`,
      args: [`${NM}/@oai/cua-repl/bin/cua-repl.mjs`],
      env: {
        HOME, PATH: Deno.env.get("PATH") ?? "/usr/bin:/bin", USER: Deno.env.get("USER") ?? "",
        NODE_REPL_NATIVE_PIPE_CONNECT_TIMEOUT_MS: "1000", NODE_REPL_NODE_MODULE_DIRS: NM, NODE_REPL_NODE_PATH: `${R}/cua_node/bin/node`,
        NODE_REPL_TRUSTED_CODE_PATHS: `${HOME}/.codex:${NM}`, CODEX_HOME: `${HOME}/.codex`, BROWSER_USE_AVAILABLE_BACKENDS: "chrome,iab,mcpapps",
        BROWSER_USE_TINYSKY_ENABLED: "1", NODE_REPL_TRUSTED_SERVICES: JSON.stringify({ browser: "@oai/browser-desktop/service", sky: "@oai/sky/service" }),
        SKY_CUA_SERVICE_PATH: `${HOME}/.codex/computer-use/Codex Computer Use.app`, CUA_REPL_NODE_REPL_PATH: `${R}/cua_node/bin/node_repl`,
        CUA_REPL_ENABLED_SURFACES: "browser,computer",
      },
      onElicit: async (p) => {
        const app = String(p?._meta?.tool_params?.app ?? "");
        const ok = app === BUNDLE || /^OptChat$/i.test(app);
        me.elicits.push(`${ok ? "accept" : "decline"} ${app}`);
        return ok ? { action: "accept", content: {}, _meta: { persist: "session" } } : { action: "decline" };
      },
    });
    return me;
  }
  async js(code: string, timeout = 30_000): Promise<{ text: string; images: string[]; isError: boolean }> {
    const r = await this.c.callTool("js", { code, timeout_ms: timeout }, { "x-codex-turn-metadata": { session_id: this.session, turn_id: `t${++this.n}` } }, timeout + 30_000);
    const text = (r.content ?? []).filter((p) => p.type === "text").map((p) => p.text ?? "").join("\n");
    const images = (r.content ?? []).filter((p) => p.type === "image").map((p) => p.data ?? "");
    return { text, images, isError: !!r.isError };
  }
  close() { this.c.close(); }
}

export async function sh(cmd: string[], env: Record<string, string> = {}): Promise<string> {
  const o = await new Deno.Command(cmd[0]!, { args: cmd.slice(1), env, stdout: "piped", stderr: "piped" }).output();
  return new TextDecoder().decode(o.stdout) + new TextDecoder().decode(o.stderr);
}

export const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
