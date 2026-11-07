import { errorText, invoke } from "./api.js";
import { $, el } from "./dom.js";
import { bytes } from "./format.js";
import { K, t } from "./i18n.js";
import { onLanguageChange, openModal } from "./screens.js";

// Раздел «Место на телефоне» и срок автоочистки звука встреч в настройках.

// Части в том порядке, в каком они видны в разделе.
const PARTS = [
  ["recordings", K.storage_recordings],
  ["transcripts", K.storage_transcripts],
  ["models", K.storage_models],
  ["voices", K.storage_voices],
  ["temporary", K.storage_temporary],
];

// Срок очистки в днях; null: звук не удаляется. Приходит с настройками при старте.
let cleanupDays = null;
let usage = null;
// Номер последнего подсчёта: ответ более раннего не затирает новый.
let measuring = 0;

export function applyAudioCleanup(days) {
  cleanupDays = days ?? null;
}

// Пояс устройства: на сколько минут местное время впереди UTC.
function offsetMinutes() {
  return -new Date().getTimezoneOffset();
}

// Удаляет звук встреч старше выбранного срока. Ошибка не мешает работе: звук удалится
// при следующем запуске.
export function cleanOldAudio() {
  return invoke("clean_old_audio", { offsetMinutes: offsetMinutes() }).catch(() => 0);
}

function status(id, text, failed = false) {
  const box = $(id);
  box.classList.toggle("error-text", failed);
  box.textContent = text;
}

function renderUsage() {
  const list = $("storage-list");
  if (!usage) {
    list.replaceChildren();
    list.hidden = true;
    return;
  }
  list.hidden = false;
  list.replaceChildren(
    ...PARTS.map(([part, key]) => {
      const row = el("div", "model-row");
      row.append(el("span", "", t(key)), el("span", "model-size", bytes(usage[part])));
      const item = el("li");
      item.append(row);
      return item;
    }),
  );
}

async function measure() {
  const mine = ++measuring;
  try {
    const found = await invoke("storage_usage");
    if (mine !== measuring) return;
    usage = found;
  } catch (error) {
    if (mine !== measuring) return;
    usage = null;
    status("storage-status", errorText(error), true);
  }
  renderUsage();
}

function renderCleanup() {
  const value = cleanupDays === null ? "off" : String(cleanupDays);
  for (const radio of document.querySelectorAll('input[name="audio-cleanup"]')) {
    radio.checked = radio.value === value;
  }
}

// Показ раздела при открытии настроек: размеры считаются заново.
export function renderSettingsStorage() {
  status("storage-status", "");
  status("audio-cleanup-status", "");
  renderCleanup();
  measure();
}

async function saveCleanup(radio) {
  const days = radio.value === "off" ? null : Number(radio.value);
  status("audio-cleanup-status", "");
  try {
    const saved = await invoke("set_audio_cleanup", { days });
    applyAudioCleanup(saved.audio_cleanup_days);
    await cleanOldAudio();
    measure();
  } catch (error) {
    renderCleanup();
    status("audio-cleanup-status", errorText(error), true);
  }
}

// Кнопка очистки: подтверждение, затем команда ядра и новый подсчёт места.
function confirmed(button, dialog, yes, no, command) {
  $(button).addEventListener("click", () => {
    status("storage-status", "");
    openModal($(dialog));
  });
  $(no).addEventListener("click", () => $(dialog).close());
  $(yes).addEventListener("click", async () => {
    $(dialog).close();
    $(button).disabled = true;
    try {
      await invoke(command);
      status("storage-status", t(K.storage_done));
    } catch (error) {
      status("storage-status", errorText(error), true);
    } finally {
      $(button).disabled = false;
    }
    measure();
  });
}

export function initSettingsStorage() {
  for (const radio of document.querySelectorAll('input[name="audio-cleanup"]')) {
    radio.addEventListener("change", () => saveCleanup(radio));
  }
  confirmed(
    "storage-remove-audio",
    "storage-remove-audio-confirm",
    "storage-remove-audio-yes",
    "storage-remove-audio-no",
    "remove_all_audio",
  );
  confirmed(
    "storage-clear-temporary",
    "storage-clear-temporary-confirm",
    "storage-clear-temporary-yes",
    "storage-clear-temporary-no",
    "clear_temporary_files",
  );
  onLanguageChange(renderUsage);
}
