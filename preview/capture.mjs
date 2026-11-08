// РЎРєСЂРёРЅС€РѕС‚С‹ СЌРєСЂР°РЅРѕРІ РґР»СЏ РґРёР·Р°Р№РЅ-Р±РѕСЂРґР° Рё РґРѕРєСѓРјРµРЅС‚Р°С†РёРё.
//
//   node preview/capture.mjs
//
// РџРёС€РµС‚ `artifacts/*.png` РІ РјР°СЃС€С‚Р°Р±Рµ 1: С„Р°Р№Р»С‹ РёРґСѓС‚ РІ СЂРµРїРѕР·РёС‚РѕСЂРёР№, РїРѕСЌС‚РѕРјСѓ 2Г— РґР»СЏ РЅРёС…
// СЃР»РёС€РєРѕРј С‚СЏР¶С‘Р»С‹Р№. РЁРёСЂРёРЅС‹ Рё РјР°СЂС€СЂСѓС‚С‹ вЂ” РЅРёР¶Рµ; СЃРѕСЃС‚РѕСЏРЅРёРµ РґРµРјРѕ-РґР°РЅРЅС‹С… РїСЂРёС…РѕРґРёС‚ РёР· mock.js.
import fs from "node:fs";

const port = process.env.CDP_PORT ?? "9223";
const base = process.env.BASE ?? "http://127.0.0.1:8123/preview/index.html";
const out = "artifacts";

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
const evaluate = async (expression) =>
  (await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true, userGesture: true })).result?.value;

await send("Page.enable");
await send("Runtime.enable");
await send("Network.enable");
await send("Network.setCacheDisabled", { cacheDisabled: true });
fs.mkdirSync(out, { recursive: true });

async function shot(name, query, { width = 390, height = 844, full = false, wait = 2400, action = null } = {}) {
  await send("Emulation.setDeviceMetricsOverride", {
    width,
    height,
    deviceScaleFactor: 1,
    mobile: true,
    screenWidth: width,
    screenHeight: height,
  });
  await send("Page.navigate", { url: `${base}${query}` });
  await sleep(wait);
  if (action) {
    await evaluate(action);
    await sleep(700);
  }
  await evaluate("document.fonts.ready.then(() => true)");
  const png = await send("Page.captureScreenshot", { format: "png", fromSurface: true, captureBeyondViewport: full });
  fs.writeFileSync(`${out}/${name}.png`, Buffer.from(png.data, "base64"));
  console.log(name, fs.statSync(`${out}/${name}.png`).size);
}

// РџСЂРѕРєСЂСѓС‚РєР° Рє СЌР»РµРјРµРЅС‚Сѓ РїРµСЂРµРґ СЃРЅРёРјРєРѕРј: РЅР° СЌРєСЂР°РЅРµ РІСЃС‚СЂРµС‡ РёРЅС‚РµСЂРµСЃРµРЅ СЃРїРёСЃРѕРє, Р° РЅРµ hero.
const scrollTo = (selector, offset = 220) =>
  `(() => { const n = document.querySelector('${selector}'); if (n) window.scrollTo(0, n.getBoundingClientRect().top + window.scrollY - ${offset}); return true; })()`;

await shot("onboarding", "?screen=onboarding");
await shot("home", "?screen=home");
await shot("home-full", "?screen=home", { full: true });
await shot("home-320", "?screen=home", { width: 320, height: 720 });
await shot("home-en", "?lang=en&screen=home");
await shot("search", "?screen=search", { action: scrollTo("#meeting-hits", 260) });
await shot("transcript", "?screen=transcript");
await shot("transcript-full", "?screen=transcript", { full: true });
await shot("player", "?screen=player");
await shot("emotion", "?screen=emotion");
await shot("fillers", "?screen=fillers");
await shot("labels", "?screen=labels");
await shot("speaker", "?screen=speaker");
await shot("voices", "?screen=voices");
await shot("settings", "?screen=settings");
await shot("settings-full", "?screen=settings", { full: true });
await shot("models", "?screen=models");
await shot("export", "?screen=export");
await shot("stats", "?screen=stats");
await shot("summary", "?screen=summary");
await shot("processing", "?screen=processing", { wait: 1200 });
await shot("recording", "?screen=home", { wait: 1200, action: "document.querySelector('#record').click()" });

// Р”РёР·Р°Р№РЅ-Р±РѕСЂРґ: СЃС‚Р°С‚РёС‡РµСЃРєР°СЏ СЃС‚СЂР°РЅРёС†Р° СЃ РІРѕСЃРµРјСЊСЋ СЌРєСЂР°РЅР°РјРё Рё РїР°Р»РёС‚СЂРѕР№.
await send("Emulation.setDeviceMetricsOverride", {
  width: 1520,
  height: 1800,
  deviceScaleFactor: 1,
  mobile: false,
  screenWidth: 1520,
  screenheight: 1800,
});
await send("Page.navigate", { url: base.replace("index.html", "board.html") });
await sleep(2000);
await evaluate("document.fonts.ready.then(() => true)");
const board = await send("Page.captureScreenshot", { format: "png", fromSurface: true, captureBeyondViewport: false });
fs.writeFileSync(`${out}/design-board.png`, Buffer.from(board.data, "base64"));
console.log("design-board", fs.statSync(`${out}/design-board.png`).size);

ws.close();

