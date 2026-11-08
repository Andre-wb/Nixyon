// Any page, any size: one screenshot through the DevTools protocol.
//
//   node preview/snap.mjs <url> <out.png> [width] [height] [full]
//
// The browser has to be started with `--remote-debugging-port` (CDP_PORT, 9223 by
// default). No npm packages: only the built-in fetch and WebSocket.
import fs from "node:fs";
import path from "node:path";

const [url, out, widthArg = "390", heightArg = "844", mode = ""] = process.argv.slice(2);
if (!url || !out) {
  console.error("usage: node preview/snap.mjs <url> <out.png> [width] [height] [full]");
  process.exit(2);
}

const width = Number(widthArg);
const height = Number(heightArg);
const port = process.env.CDP_PORT ?? "9223";

const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
const page = targets.find((t) => t.type === "page");
if (!page) throw new Error("no page target");

const ws = new WebSocket(page.webSocketDebuggerUrl);
let nextId = 1;
const pending = new Map();
ws.addEventListener("message", (event) => {
  const message = JSON.parse(event.data);
  if (!message.id) return;
  const item = pending.get(message.id);
  if (!item) return;
  pending.delete(message.id);
  if (message.error) item.reject(new Error(JSON.stringify(message.error)));
  else item.resolve(message.result);
});
await new Promise((resolve, reject) => {
  ws.addEventListener("open", resolve, { once: true });
  ws.addEventListener("error", reject, { once: true });
});

function send(method, params = {}) {
  const id = nextId++;
  ws.send(JSON.stringify({ id, method, params }));
  return new Promise((resolve, reject) => pending.set(id, { resolve, reject }));
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const evaluate = async (expression, awaitPromise = false) =>
  (await send("Runtime.evaluate", { expression, awaitPromise, returnByValue: true, userGesture: true })).result?.value;

await send("Page.enable");
await send("Runtime.enable");
await send("Emulation.setDeviceMetricsOverride", {
  width,
  height,
  deviceScaleFactor: 2,
  mobile: width < 600,
  screenWidth: width,
  screenHeight: height,
});
await send("Page.navigate", { url });
for (let i = 0; i < 80; i += 1) {
  if (await evaluate(`document.readyState === "complete"`)) break;
  await sleep(100);
}
await sleep(Number(process.env.SHOT_WAIT ?? 900));
await evaluate("document.fonts?.ready ? document.fonts.ready.then(() => true) : true", true);
const info = await evaluate(
  `({ screen: document.querySelector(".screen:not([hidden])")?.id ?? null,
     dialog: document.querySelector("dialog[open]")?.id ?? null,
     scrollWidth: document.documentElement.scrollWidth,
     innerWidth, height: document.documentElement.scrollHeight })`,
);
console.log(JSON.stringify(info));

const shot = await send("Page.captureScreenshot", {
  format: "png",
  fromSurface: true,
  captureBeyondViewport: mode === "full",
});
fs.mkdirSync(path.dirname(out), { recursive: true });
fs.writeFileSync(out, Buffer.from(shot.data, "base64"));
ws.close();
