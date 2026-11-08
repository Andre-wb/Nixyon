// РРЅС‚РµСЂС„РµР№СЃ РїРѕРґ РЅР°РіСЂСѓР·РєРѕР№: РІСЃРµ СЌРєСЂР°РЅС‹, РІСЃРµ С€РёСЂРёРЅС‹, РѕС€РёР±РєРё РІ РєРѕРЅСЃРѕР»Рё.
//
//   node preview/audit.mjs [С€РёСЂРёРЅР°]
//
// РџСЂРѕРІРµСЂСЏРµС‚, С‡С‚Рѕ РєР°Р¶РґР°СЏ СЃС‚СЂР°РЅРёС†Р° РѕС‚РєСЂС‹РІР°РµС‚СЃСЏ Р±РµР· РёСЃРєР»СЋС‡РµРЅРёР№, РЅРёС‡РµРіРѕ РЅРµ РІС‹Р»РµР·Р°РµС‚ Р·Р°
// СЌРєСЂР°РЅ РїРѕ РіРѕСЂРёР·РѕРЅС‚Р°Р»Рё Рё РІСЃРµ С†РµР»Рё РЅР°Р¶Р°С‚РёСЏ РЅРµ РјРµРЅСЊС€Рµ 40 px (РєСЂРѕРјРµ СЂР°РґРёРѕРєРЅРѕРїРѕРє РІРЅСѓС‚СЂРё
// РєР°СЂС‚РѕС‡РєРё РІС‹Р±РѕСЂР°, РіРґРµ РЅР°Р¶РёРјР°РµС‚СЃСЏ РІСЃСЏ РєР°СЂС‚РѕС‡РєР°).
//
//   CDP_PORT  РїРѕСЂС‚ Р±СЂР°СѓР·РµСЂР° СЃ --remote-debugging-port (РїРѕ СѓРјРѕР»С‡Р°РЅРёСЋ 9223)
//   BASE      Р°РґСЂРµСЃ РїСЂРµРІСЊСЋ (РїРѕ СѓРјРѕР»С‡Р°РЅРёСЋ http://127.0.0.1:8123/preview/index.html)
const port = process.env.CDP_PORT ?? "9223";
const base = process.env.BASE ?? "http://127.0.0.1:8123/preview/index.html";

const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
const page = targets.find((t) => t.type === "page");
const ws = new WebSocket(page.webSocketDebuggerUrl);
let nextId = 1;
const pending = new Map();
let events = [];
ws.addEventListener("message", (event) => {
  const message = JSON.parse(event.data);
  if (message.id) {
    const item = pending.get(message.id);
    if (!item) return;
    pending.delete(message.id);
    if (message.error) item.reject(new Error(JSON.stringify(message.error)));
    else item.resolve(message.result);
    return;
  }
  if (["Runtime.exceptionThrown", "Log.entryAdded", "Runtime.consoleAPICalled"].includes(message.method)) {
    events.push(message);
  }
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
const evaluate = async (expression, awaitPromise = false) => {
  const result = await send("Runtime.evaluate", { expression, awaitPromise, returnByValue: true, userGesture: true });
  return result.result?.value;
};

await send("Page.enable");
await send("Runtime.enable");
await send("Log.enable");

const widths = process.argv[2] ? [Number(process.argv[2])] : [320, 390, 480, 800];
const screens = [
  "?screen=onboarding",
  "?screen=home",
  "?screen=search",
  "?screen=transcript",
  "?screen=player",
  "?screen=voices",
  "?screen=settings",
  "?screen=models",
  "?screen=export",
  "?screen=stats",
  "?screen=summary",
  "?screen=speaker",
  "?screen=labels",
  "?screen=fillers",
  "?screen=emotion",
  "?screen=turn",
  "?screen=processing",
];

let failures = 0;
for (const width of widths) {
  const height = width <= 320 ? 720 : 844;
  await send("Emulation.setDeviceMetricsOverride", {
    width,
    height,
    deviceScaleFactor: 2,
    mobile: true,
    screenWidth: width,
    screenHeight: height,
  });
  for (const query of screens) {
    events = [];
    await send("Page.navigate", { url: `${base}${query}` });
    await sleep(2600);
    const report = await evaluate(`(() => {
      const q = (s) => document.querySelector(s);
      const overflow = document.documentElement.scrollWidth - innerWidth;
      const small = [...document.querySelectorAll("button:not([hidden]), a, input, textarea, select, .choice")]
        .filter(n => {
          if (n.matches('input[type="radio"], input[type="checkbox"]') && (n.closest(".choice") || n.closest(".label-check") || n.closest(".filler-item"))) return false;
          if (n.closest("[hidden]")) return false;
          const r = n.getBoundingClientRect();
          const style = getComputedStyle(n);
          return r.width > 0 && r.height > 0 && style.visibility !== "hidden" && (r.width < 40 || r.height < 40);
        })
        .map(n => ({ tag: n.tagName, id: n.id, cls: String(n.className ?? "").slice(0, 50), w: Math.round(n.getBoundingClientRect().width), h: Math.round(n.getBoundingClientRect().height) }))
        .slice(0, 6);
      return {
        overflow,
        dialog: q("dialog[open]")?.id ?? null,
        screen: [...document.querySelectorAll(".screen")].find(s => !s.hidden)?.id,
        emptyText: [...document.querySelectorAll("#screen-home *, #screen-transcript *, #screen-settings *")]
          .filter(n => n.children.length === 0 && n.textContent.trim() === "" && getComputedStyle(n).display !== "none" && n.closest("[hidden]") === null && /^(H1|H2|P|BUTTON|LABEL)$/.test(n.tagName) && n.getBoundingClientRect().height > 8)
          .filter(n => !n.closest("[aria-hidden='true']") && !n.matches(".status, .error, .meta, .search-count, .meetings-empty"))
          .map(n => n.tagName + "#" + n.id + "." + String(n.className ?? "").slice(0, 30)).slice(0, 6),
        manrope: document.fonts.check('16px Manrope'),
      };
    })()`);
    const errors = events
      .filter((event) => event.method === "Runtime.exceptionThrown" || event.params?.entry?.level === "error")
      .map((event) =>
        event.method === "Runtime.exceptionThrown"
          ? `${event.params.exceptionDetails.text} ${event.params.exceptionDetails.exception?.description ?? ""}`.slice(0, 300)
          : `${event.params.entry.text} ${event.params.entry.url ?? ""}`.slice(0, 300),
      );
    const bad = report.overflow > 0 || errors.length > 0 || report.emptyText.length > 0;
    if (bad) failures += 1;
    console.log(`${width}${query ? " " + query : " home"}`, JSON.stringify({ ...report, errors }));
  }
}
console.log("FAILURES", failures);
ws.close();

