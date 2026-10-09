// Launch, palette model switch, and one streamed turn each on Claude Haiku and Codex GPT-6-Luna through the UI.
import { App, check, failed } from "./lib/app.ts";
import { sleep } from "./lib/cua.ts";
const a = await App.launch();
const st = () => a.state();
const turn = async (model: string, id: string, word: string) => {
  await a.palette(`Model: ${model}`);
  await a.waitFor(() => st().model === id, 8000, `model ${id}`);
  check(true, `palette switched the model to ${model}`);
  await a.click("composer.input");
  await a.type(`Reply with exactly: ${word}`);
  await a.key("Return");
  await a.waitFor(() => st().running, 20000, "turn running");
  await a.shot(`smoke-${id}-running`);
  await a.waitFor(() => !st().running && (st().lastTalk ?? "").includes(word), 180000, `reply ${word}`);
  check(true, `${model} replied ${word}`);
  await sleep(600);
  await a.shot(`smoke-${id}-replied`);
};
try {
  check(st().t > 1000, `launched on the copied chat (${st().t} messages, first frame ${Math.round(st().firstFrameMs)} ms)`);
  await a.shot("smoke-01-launch");
  await turn("Claude Haiku", "claude-haiku-5-5", "pong-gpui-1");
  await turn("GPT-6-Luna", "gpt-6-luna", "pong-gpui-2");
} catch (e) { check(false, String(e).slice(0, 400)); await a.shot("smoke-fail"); } finally { await a.quit(); }
Deno.exit(failed() ? 1 : 0);
