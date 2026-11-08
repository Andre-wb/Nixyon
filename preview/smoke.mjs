// Функциональный прогон превью: сценарии проходят до конца, без исключений в консоли.
//
//   node preview/smoke.mjs
//
// Каждый шаг — пара «действие, проверка». Сценарий считается пройденным, когда
// проверка вернула истину и за шаг не было ошибок в консоли. Выход 1 при первом провале:
// так прогон годится и для ручной проверки, и для будущего CI.
const port = process.env.CDP_PORT ?? "9223";
const base = process.env.BASE ?? "http://127.0.0.1:8123/preview/index.html";

const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
const page = targets.find((t) => t.type === "page");
const ws = new WebSocket(page.webSocketDebuggerUrl);
let nextId = 1;
const pending = new Map();
let errors = [];
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
  if (message.method === "Runtime.exceptionThrown") {
    errors.push(message.params.exceptionDetails.exception?.description ?? message.params.exceptionDetails.text);
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
const evaluate = async (expression) => {
  const result = await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true, userGesture: true });
  return result.result?.value;
};
const click = (selector) => evaluate(`(() => { const n = document.querySelector(${JSON.stringify(selector)}); if (!n) return false; n.click(); return true; })()`);
const text = (selector) => evaluate(`document.querySelector(${JSON.stringify(selector)})?.textContent ?? ""`);
const visible = (selector) => evaluate(`(() => {
  const n = document.querySelector(${JSON.stringify(selector)});
  if (!n) return false;
  if (n.tagName === "DIALOG") return n.open === true;
  if (n.hidden) return false;
  return n.getBoundingClientRect().height > 0;
})()`);
const wait = async (selector, timeout = 4000) => {
  const started = Date.now();
  while (Date.now() - started < timeout) {
    if (await visible(selector)) return true;
    await sleep(60);
  }
  return false;
};

await send("Page.enable");
await send("Runtime.enable");
await send("Network.enable");
await send("Network.setCacheDisabled", { cacheDisabled: true });
await send("Emulation.setDeviceMetricsOverride", { width: 390, height: 844, deviceScaleFactor: 1, mobile: true, screenWidth: 390, screenHeight: 844 });

const results = [];
async function step(name, action, check) {
  errors = [];
  let ok = false;
  let detail = "";
  try {
    await action();
    ok = Boolean(await check());
  } catch (error) {
    detail = String(error).slice(0, 200);
  }
  if (errors.length) {
    ok = false;
    detail = errors[0].slice(0, 240);
  }
  results.push({ name, ok, detail });
  console.log(ok ? "PASS" : "FAIL", name, detail ? `— ${detail}` : "");
  return ok;
}

async function open(query) {
  await send("Page.navigate", { url: `${base}${query}` });
  await sleep(900);
}

await open("?screen=onboarding");
await step("онбординг: кнопка «Продолжить» ведёт на главный экран",
  async () => click("#onboard-continue"),
  async () => (await wait("#screen-home")) && !(await visible("#screen-onboarding")),
);

await step("главный экран: архив встреч загружен",
  async () => {},
  async () => (await evaluate("document.querySelectorAll('#meetings-list .meeting-row').length")) >= 4,
);

await step("поиск по встречам: находятся реплики",
  async () => {
    await evaluate(`(() => { const f = document.querySelector("#meetings-search"); f.value = "интерфейс"; f.dispatchEvent(new Event("input", { bubbles: true })); return true; })()`);
    await sleep(700);
  },
  async () => (await evaluate("document.querySelectorAll('#meeting-hits .meeting-row').length")) > 0,
);

await step("фильтр по метке: список сужается",
  async () => {
    await evaluate(`(() => { const f = document.querySelector("#meetings-search"); f.value = ""; f.dispatchEvent(new Event("input", { bubbles: true })); return true; })()`);
    await sleep(500);
    await click('#label-filter .chip');
    await sleep(300);
  },
  async () => (await evaluate("document.querySelectorAll('#meetings-list .meeting-row').length")) < 4,
);

await step("встреча открывается из списка",
  async () => {
    await click('#label-filter .chip');
    await sleep(300);
    await click("#meetings-list .meeting-row");
    await sleep(1200);
  },
  async () => (await wait("#screen-transcript")) && (await evaluate("document.querySelectorAll('#turns .turn').length")) > 3,
);

await step("участник переименовывается",
  async () => {
    await click("#speakers .speaker-row");
    await sleep(400);
    await evaluate(`(() => { const i = document.querySelector("#speaker-name"); i.value = "Анна П."; return true; })()`);
    await click("#speaker-save");
    await sleep(500);
  },
  async () => (await text("#speakers .speaker-name")).includes("Анна П."),
);

await step("эмоция реплики меняется",
  async () => {
    await click("#turns .turn-emotion");
    await sleep(400);
    await evaluate(`(() => { const r = document.querySelector('#emotion-sheet input[value="anger"]'); r.checked = true; r.dispatchEvent(new Event("change", { bubbles: true })); return true; })()`);
    await sleep(600);
  },
  async () => (await text("#turns .turn-emotion")).includes("Гнев"),
);

await step("слова-паразиты: модель предлагает правки и применяет их",
  async () => {
    await click("#open-fillers");
    await sleep(500);
    await click("#filler-run-all");
    await sleep(2200);
    await click("#filler-apply");
    await sleep(700);
  },
  async () => (await evaluate("document.querySelector('#turns').textContent.includes('Ну,')")) === false,
);

await step("правка текста реплики",
  async () => {
    await click("#turns .turn-edit");
    await sleep(400);
    await evaluate(`(() => { const t = document.querySelector("#turn-text-input"); t.value = "Проверка правки текста."; return true; })()`);
    await click("#turn-save");
    await sleep(600);
  },
  async () => (await evaluate("document.querySelectorAll('#turns .turn-text')[0].textContent")) === "Проверка правки текста.",
);

await step("плеер: пуск, перемотка, скорость, пропуск тишины",
  async () => {
    await click("#player-toggle");
    await sleep(700);
    await click("#player-forward");
    await click('#player-speeds .chip[data-speed="1.5"]');
    await click("#player-skip-silence");
    await sleep(400);
  },
  async () => (await text("#player-time")).startsWith("0:") && (await evaluate(`document.querySelector('#player-skip-silence').getAttribute('aria-pressed')`)) === "true",
);

await step("выбор участника ограничивает воспроизведение",
  async () => {
    await evaluate(`(() => { const s = document.querySelector("#player-speaker"); s.value = s.options[1].value; s.dispatchEvent(new Event("change", { bubbles: true })); return true; })()`);
    await sleep(400);
  },
  async () => (await evaluate(`document.querySelector('#player-speaker').value`)) !== "",
);

await step("удаление встречи спрашивает подтверждение",
  async () => {
    await click("#meeting-delete");
    await sleep(400);
  },
  async () => await visible("#meeting-delete-confirm"),
);

await step("отмена удаления возвращает к расшифровке",
  async () => {
    await click("#meeting-delete-no");
    await sleep(500);
  },
  async () => (await wait("#screen-transcript")) && !(await visible("#meeting-delete-confirm")),
);

await step("настройки: экран открывается и показывает модели и место",
  async () => {
    await send("Page.navigate", { url: `${base}?screen=settings` });
    await sleep(1500);
  },
  async () => (await wait("#screen-settings")) && (await evaluate("document.querySelectorAll('#settings-models .model-row').length")) >= 6 && (await evaluate("document.querySelectorAll('#storage-list .model-row').length")) === 5,
);

await step("переключение языка меняет подписи",
  async () => {
    await evaluate(`(() => { const r = document.querySelector('input[name="language"][value="en"]'); r.checked = true; r.dispatchEvent(new Event("change", { bubbles: true })); return true; })()`);
    await sleep(500);
  },
  async () => (await text("#settings-title")) === "Settings",
);

await step("модели: экран загрузки и установка",
  async () => {
    await send("Page.navigate", { url: `${base}?lang=ru&screen=models` });
    await sleep(1200);
    await click("#models-download");
    await sleep(3200);
  },
  async () => (await evaluate(`document.querySelector('#models-list').textContent.includes('Скачана')`)) === true,
);

await step("голоса: лист голоса открывается",
  async () => {
    await send("Page.navigate", { url: `${base}?screen=voices` });
    await sleep(1200);
    await evaluate(`(() => { document.querySelector(".voice-open")?.click(); return true; })()`);
    await sleep(500);
  },
  async () => await visible("#voice-card"),
);

const failed = results.filter((result) => !result.ok);
console.log(`ИТОГ: ${results.length - failed.length} из ${results.length}`);
ws.close();
process.exit(failed.length ? 1 : 0);
