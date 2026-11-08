import { channel, errorCode, errorText, invoke } from "./api.js";
import { $, el } from "./dom.js";
import { bytes } from "./format.js";
import { K, t } from "./i18n.js";
import { onLanguageChange, openModal, show } from "./screens.js";

// Название и описание каждой модели по её ключу из каталога.
const TEXTS = {
  speech: [K.models_speech_name, K.models_speech_desc],
  live: [K.models_live_name, K.models_live_desc],
  denoiser: [K.models_denoiser_name, K.models_denoiser_desc],
  age: [K.models_age_name, K.models_age_desc],
  emotion: [K.models_emotion_name, K.models_emotion_desc],
  llm: [K.models_llm_name, K.models_llm_desc],
};

const listeners = [];
// Ответ плагина: модели из каталога и что уже скачано. null: управления моделями нет
// (ноутбук), тогда приложение работает как раньше.
let status = null;
// Отмеченные необязательные модели, которые ещё не скачаны.
const chosen = new Set();
let downloading = false;
let progress = null;

export function onModelsChange(listener) {
  listeners.push(listener);
}

function changed(ready) {
  for (const listener of listeners) listener(ready);
}

function installed(key) {
  return Boolean(status?.packs.some((p) => p.key === key && p.installed));
}

// Без ответа плагина считаем, что модели на месте: о причине скажет сам движок.
export function speechReady() {
  return status === null || installed("speech");
}

// Что уйдёт в загрузку: обязательные и отмеченные модели, которых ещё нет.
function toDownload() {
  return (status?.packs ?? []).filter((p) => !p.installed && (p.required || chosen.has(p.key)));
}

// Строка модели: название, описание и размер слева, справа переключатель. У обязательной
// и уже скачанной модели переключателя нет, вместо него надпись.
// Название, описание и размер модели; общие для экрана загрузки и настроек.
export function modelText(pack) {
  const [name, desc] = TEXTS[pack.key] ?? [null, null];
  const text = el("span", "model-text");
  text.append(el("span", "model-name", name ? t(name) : pack.key));
  if (desc) text.append(el("span", "meta", t(desc)));
  text.append(el("span", "model-size", bytes(pack.size)));
  return text;
}

// Ответ плагина о моделях; null, если управления моделями нет.
export function currentStatus() {
  return status;
}

// Перечитывает состояние моделей и сообщает об этом подписчикам: так кнопки протокола и
// имён узнают, что языковую модель включили или выключили.
export async function refreshModels() {
  await refresh();
  changed(false);
}

function row(pack) {
  const item = el("li");
  const fixed = pack.required || pack.installed;
  const label = el(fixed ? "div" : "label", "model-row");
  label.append(modelText(pack));
  if (fixed) {
    label.append(el("span", "model-state", t(pack.installed ? K.models_installed : K.models_required)));
  } else {
    const box = el("input", "switch");
    box.type = "checkbox";
    box.setAttribute("role", "switch");
    box.checked = chosen.has(pack.key);
    box.disabled = downloading;
    box.addEventListener("change", () => {
      if (box.checked) chosen.add(pack.key);
      else chosen.delete(pack.key);
      renderTotal();
    });
    label.append(box);
  }
  item.append(label);
  return item;
}

function renderTotal() {
  const total = toDownload().reduce((sum, p) => sum + p.size, 0);
  $("models-total").textContent = total > 0 ? t(K.models_total, { size: bytes(total) }) : "";
  $("models-download").disabled = total === 0;
}

function renderProgress() {
  $("models-progress").hidden = !downloading;
  if (!downloading) return;
  const done = progress?.done ?? 0;
  const total = progress?.total ?? 0;
  const share = total > 0 ? Math.min(1, done / total) : 0;
  let phase = t(K.models_downloading, { done: bytes(done), total: bytes(total) });
  if (progress?.phase === "waiting") phase = t(K.models_waiting);
  if (progress?.phase === "checking") phase = t(K.models_checking);
  $("models-phase").textContent = phase;
  $("models-fill").style.transform = `scaleX(${share})`;
  $("models-bar").setAttribute("aria-valuenow", String(Math.round(share * 100)));
}

function render() {
  const list = $("models-list");
  list.replaceChildren(...(status?.packs ?? []).map(row));
  renderTotal();
  renderProgress();
  $("models-download").hidden = downloading;
  $("models-cancel").hidden = !downloading;
  $("models-later").hidden = downloading;
}

async function refresh() {
  try {
    status = await invoke("models_status");
  } catch {
    status = null;
  }
}

// Экран попадает в историю, чтобы системная «Назад» вела на главный экран. Он считается
// показанным сразу: при следующем запуске сам не откроется.
export function openModels() {
  if (location.hash !== "#models") history.pushState(null, "", "#models");
  $("models-error").textContent = "";
  render();
  show("models");
  invoke("set_models_prompt_seen").catch(() => null);
}

function leave() {
  if (location.hash === "#models") history.back();
  else show("home");
}

// Вне Wi-Fi перед загрузкой спрашиваем: моделей на несколько гигабайт.
function confirmMetered(size) {
  return new Promise((resolve) => {
    const dialog = $("models-metered");
    $("models-metered-text").textContent = t(K.models_metered, { size: bytes(size) });
    let answer = false;
    const yes = () => {
      answer = true;
      dialog.close();
    };
    const no = () => dialog.close();
    $("models-metered-yes").addEventListener("click", yes, { once: true });
    $("models-metered-no").addEventListener("click", no, { once: true });
    dialog.addEventListener(
      "close",
      () => {
        $("models-metered-yes").removeEventListener("click", yes);
        $("models-metered-no").removeEventListener("click", no);
        resolve(answer);
      },
      { once: true },
    );
    openModal(dialog);
  });
}

async function download() {
  const packs = toDownload();
  if (packs.length === 0) return;
  let allowMetered = false;
  await refresh();
  if (status?.metered) {
    allowMetered = await confirmMetered(packs.reduce((sum, p) => sum + p.size, 0));
    if (!allowMetered) return;
  }
  await run(
    packs.map((p) => p.key),
    allowMetered,
  );
}

async function run(packs, allowMetered) {
  downloading = true;
  progress = null;
  $("models-error").textContent = "";
  render();
  try {
    await invoke("download_models", {
      packs,
      allowMetered,
      notificationTitle: t(K.models_notification),
      onProgress: channel((p) => {
        progress = p;
        renderProgress();
      }),
    });
    downloading = false;
    chosen.clear();
    await refresh();
    render();
    changed(true);
    leave();
  } catch (error) {
    downloading = false;
    await refresh();
    render();
    if (errorCode(error) !== "cancelled") $("models-error").textContent = errorText(error);
  }
}

export async function initModels(promptSeen) {
  $("models-download").addEventListener("click", download);
  $("models-cancel").addEventListener("click", () => invoke("cancel_model_download").catch(() => null));
  $("models-later").addEventListener("click", leave);
  $("models-back").addEventListener("click", leave);
  onLanguageChange(render);
  await refresh();
  // Сразу отмечены все необязательные модели: так после загрузки работает всё.
  for (const pack of status?.packs ?? []) if (!pack.required && !pack.installed) chosen.add(pack.key);
  changed(false);
  if (status?.downloading) {
    chosen.clear();
    for (const key of status.pending) chosen.add(key);
    openModels();
    run(status.pending, status.pendingAllowMetered);
  } else if (status && !installed("speech") && !promptSeen) {
    openModels();
  }
}
