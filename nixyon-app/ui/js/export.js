import { errorText, invoke } from "./api.js";
import { $ } from "./dom.js";
import { dateTime } from "./format.js";
import { K, language, t } from "./i18n.js";
import { openModal } from "./screens.js";

const STORE_KEY = "nixyon.export.format";
const FORMATS = ["txt", "md", "srt", "json", "docx", "pdf"];
let transcript = null;

// Последний выбранный формат помнит само устройство; без хранилища берётся TXT.
function rememberedFormat() {
  try {
    const value = localStorage.getItem(STORE_KEY);
    return FORMATS.includes(value) ? value : "txt";
  } catch {
    return "txt";
  }
}

function chosenFormat() {
  return document.querySelector('input[name="format"]:checked')?.value ?? "txt";
}

function status(text, isError = false) {
  const box = $("export-status");
  box.textContent = text;
  box.classList.toggle("error-text", isError);
}

async function prepare() {
  return invoke("export_transcript", {
    meeting: transcript.created_at,
    format: chosenFormat(),
    language: language(),
    recordedAt: dateTime(transcript.created_at) || null,
  });
}

async function save() {
  status("");
  try {
    const file = await prepare();
    if (await invoke("save_export", file)) status(t(K.export_sheet_saved));
  } catch (error) {
    status(errorText(error), true);
  }
}

async function share() {
  status("");
  try {
    await invoke("share_export", await prepare());
  } catch (error) {
    status(errorText(error), true);
  }
}

export function openExport(current) {
  transcript = current;
  const format = rememberedFormat();
  for (const radio of document.querySelectorAll('input[name="format"]')) radio.checked = radio.value === format;
  status("");
  openModal($("export-sheet"));
}

export function initExport() {
  for (const radio of document.querySelectorAll('input[name="format"]')) {
    radio.addEventListener("change", () => {
      try {
        localStorage.setItem(STORE_KEY, radio.value);
      } catch {
        // Без хранилища формат просто не запомнится до следующего раза.
      }
    });
  }
  $("export-save").addEventListener("click", save);
  $("export-share").addEventListener("click", share);
}
