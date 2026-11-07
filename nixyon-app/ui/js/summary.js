import { channel, errorCode, errorText, invoke } from "./api.js";
import { $ } from "./dom.js";
import { K, t } from "./i18n.js";
import { onModelsChange } from "./models.js";
import { onLanguageChange, openModal } from "./screens.js";

// Протокол встречи языковой моделью на телефоне. Модель работает минуты, поэтому лист
// можно закрыть: составление продолжается, результат сохраняется и виден при новом открытии.
let running = false;
let progress = null;
let text = null;

function render() {
  $("summary-running").hidden = !running;
  $("summary-start").hidden = running;
  $("summary-start").textContent = t(text ? K.summary_again : K.summary_start);
  // Ответ модели недоверенный: только как текст, никогда как разметка.
  $("summary-text").textContent = text ?? "";
  $("summary-text").hidden = running || !text;
  const share = progress && progress.total > 0 ? Math.min(1, progress.done / progress.total) : 0;
  $("summary-phase").textContent = t(progress?.phase === "writing" ? K.summary_writing : K.summary_reading);
  $("summary-progress-fill").style.transform = `scaleX(${share})`;
  $("summary-progress").setAttribute("aria-valuenow", String(Math.round(share * 100)));
}

async function start() {
  $("summary-error").textContent = "";
  running = true;
  progress = null;
  render();
  try {
    text = await invoke("summarize_meeting", {
      onProgress: channel((p) => {
        progress = p;
        render();
      }),
    });
  } catch (error) {
    if (errorCode(error) !== "cancelled") $("summary-error").textContent = errorText(error);
  } finally {
    running = false;
    render();
  }
}

async function openSummary() {
  $("summary-error").textContent = "";
  if (!running) text = await invoke("last_summary").catch(() => null);
  render();
  openModal($("summary-sheet"));
}

async function checkAvailable() {
  $("open-summary").hidden = !(await invoke("summary_available").catch(() => false));
}

// Кнопка видна, только если на телефоне есть языковая модель; после загрузки моделей
// это проверяется заново.
export async function initSummary() {
  $("open-summary").addEventListener("click", openSummary);
  $("summary-start").addEventListener("click", start);
  $("summary-cancel").addEventListener("click", () => invoke("cancel_summary").catch(() => null));
  onLanguageChange(render);
  onModelsChange(checkAvailable);
  await checkAvailable();
}
