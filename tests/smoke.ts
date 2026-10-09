import { App, check } from "./lib/app.ts";
import { sleep } from "./lib/cua.ts";
const a = await App.launch();
try {
  console.log("state", JSON.stringify(a.state()).slice(0, 300));
  await a.shot("smoke-01-launch");
  await a.key("cmd+k");
  await a.waitFor(() => a.state().overlay === "palette", 5000, "palette");
  await a.type("Model: Claude Haiku");
  await a.shot("smoke-02-palette");
  await a.key("Return");
  await a.waitFor(() => a.state().model === "claude-haiku-5-5", 8000, "model haiku");
  check(true, "palette switched the model to Haiku");
  await a.click("composer.input");
  await a.type("Reply with exactly: pong-gpui-1");
  await a.key("Return");
  await a.waitFor(() => a.state().running, 15000, "turn running");
  await a.shot("smoke-03-running");
  await a.waitFor(() => !a.state().running && (a.state().lastTalk ?? "").includes("pong-gpui-1"), 120000, "reply");
  check(true, "Haiku replied pong-gpui-1");
  await sleep(500);
  await a.shot("smoke-04-replied");
} finally {
  await a.quit();
}
