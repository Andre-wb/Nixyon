import fs from "node:fs";

const port = process.env.CDP_PORT ?? "9223";
const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
const page = targets.find((t) => t.type === "page");
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
const send = (method, params = {}) => {
  const id = nextId++;
  ws.send(JSON.stringify({ id, method, params }));
  return new Promise((resolve, reject) => pending.set(id, { resolve, reject }));
};
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const evaluate = async (expression) => {
  const result = await send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
  return result.result?.value;
};

await send("Page.enable");
await send("Runtime.enable");
await send("Network.enable");
await send("Network.setCacheDisabled", { cacheDisabled: true });
await send("Emulation.setDeviceMetricsOverride", {
  width: 1024,
  height: 1024,
  deviceScaleFactor: 1,
  mobile: false,
  screenWidth: 1024,
  screenHeight: 1024,
});

async function capture(query, out, omitBackground = false) {
  await send("Page.navigate", { url: `http://127.0.0.1:8123/preview/icon.html${query}` });
  await sleep(900);
  await evaluate("document.fonts.ready.then(() => { const f=document.querySelector('.feather'); if (f) f.style.animation='none'; return true; })");
  const shot = await send("Page.captureScreenshot", {
    format: "png",
    fromSurface: true,
    captureBeyondViewport: false,
    omitBackground,
  });
  fs.writeFileSync(out, Buffer.from(shot.data, "base64"));
  console.log(out, fs.statSync(out).size);
}

await capture("?layer=full", "artifacts/app-icon-1024.png");
await capture("?layer=background", ".android-build/icon-background.png");
ws.close();
