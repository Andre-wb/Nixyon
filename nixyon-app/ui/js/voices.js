import { channel, errorText, invoke, startMicrophone } from "./api.js";
import { $, el } from "./dom.js";
import { clock, dateTime } from "./format.js";
import { K, t } from "./i18n.js";
import { meetingTitle } from "./meeting-title.js";
import { showMeeting } from "./meetings.js";
import { onLanguageChange, onScreenChange, openModal, show } from "./screens.js";

let voices = [];
// Индекс встреч не прочитан: у голосов нет списка встреч.
let meetingsFailed = false;
// Имя голоса, чей лист открыт; null, пока лист закрыт.
let cardName = null;
// Предел длины образца приходит из ядра вместе со списком голосов.
let maxSampleSec = null;
// Запись образца: путь сохранённого файла, пока голос не сохранён или не отменён.
let sample = null;
let recording = false;
let startedAt = 0;
let timer = null;
let pendingDelete = null;

function renderSummary() {
  const names = voices.map((v) => v.name).join(", ");
  $("voices-summary").textContent = names || t(K.home_voices_none);
}

// Счётчик встреч голоса; пусто, если встреч нет.
function meetingsText(voice) {
  if (meetingsFailed) return t(K.voices_meetings_failed);
  return voice.meetings.length > 0 ? t(K.voices_meetings, { n: voice.meetings.length }) : "";
}

function renderList() {
  const list = $("voices-list");
  list.replaceChildren();
  for (const voice of voices) {
    const row = el("div", "voice-row");
    const text = el("button", "voice-text voice-open");
    text.type = "button";
    text.append(el("span", "speaker-name", voice.name));
    // У голоса, запомненного только по встречам, нет даты образца.
    if (voice.hasSample) text.append(el("span", "meta", t(K.voices_added, { date: dateTime(voice.createdAt) })));
    const counter = meetingsText(voice);
    if (counter) text.append(el("span", "meta", counter));
    text.addEventListener("click", () => openCard(voice.name));
    const remove = el("button", "icon-button");
    remove.type = "button";
    const label = t(K.voices_delete, { name: voice.name });
    remove.setAttribute("aria-label", label);
    remove.title = label;
    remove.append(trashIcon());
    remove.addEventListener("click", () => askDelete(voice.name));
    row.append(text, remove);
    const item = el("li");
    item.append(row);
    list.append(item);
  }
  list.hidden = voices.length === 0;
  $("voices-empty").hidden = voices.length > 0;
}

function trashIcon() {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("class", "icon");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("aria-hidden", "true");
  const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
  path.setAttribute("d", "M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3");
  svg.append(path);
  return svg;
}

function apply(view) {
  voices = view.voices;
  maxSampleSec = view.maxSampleSec;
  meetingsFailed = view.meetingsFailed;
  renderSummary();
  renderList();
  renderCard();
}

// Лист голоса: встречи архива, на которых он узнан, новые сверху.
function renderCard() {
  const voice = voices.find((v) => v.name === cardName);
  if (!voice) return;
  $("voice-card-title").textContent = voice.name;
  $("voice-card-meta").textContent = meetingsText(voice);
  const list = $("voice-card-meetings");
  list.replaceChildren();
  for (const meeting of voice.meetings) {
    const row = el("button", "meeting-row");
    row.type = "button";
    row.append(el("span", "speaker-name", meetingTitle(meeting.title, meeting.id)));
    // Дата нужна, только когда название своё: иначе название и есть дата.
    if (meeting.title) row.append(el("span", "meta", dateTime(meeting.id)));
    row.addEventListener("click", () => openFromCard(meeting.id));
    const item = el("li");
    item.append(row);
    list.append(item);
  }
  list.hidden = voice.meetings.length === 0;
  $("voice-card-empty").hidden = meetingsFailed || voice.meetings.length > 0;
}

function openCard(name) {
  cardName = name;
  renderCard();
  openModal($("voice-card"));
}

// Встреча открывается, когда закрытие листа уже снято из истории: иначе запись экрана
// встречи легла бы под запись листа, и «Назад» ушёл бы не туда.
function openFromCard(id) {
  window.addEventListener(
    "popstate",
    async () => {
      try {
        await showMeeting(id);
        $("voices-error").textContent = "";
      } catch (error) {
        $("voices-error").textContent = errorText(error);
      }
    },
    { once: true },
  );
  $("voice-card").close();
}

export async function loadVoices() {
  try {
    apply(await invoke("list_voices"));
    $("voices-error").textContent = "";
  } catch (error) {
    $("voices-error").textContent = errorText(error);
  }
}

// Список загружается при каждом показе экрана, см. initVoices.
function openVoices() {
  if (location.hash !== "#voices") history.pushState(null, "", "#voices");
  show("voices");
}

function renderSheet() {
  $("voice-hint").textContent = maxSampleSec === null ? "" : t(K.voice_sheet_hint, { sec: maxSampleSec });
  $("voice-record").textContent = t(recording ? K.voice_sheet_stop : K.voice_sheet_record);
  const elapsed = recording ? (Date.now() - startedAt) / 1000 : sample?.durationSec;
  $("voice-timer").textContent = elapsed === undefined ? "" : t(K.voice_sheet_recorded, { time: clock(elapsed) });
  $("voice-save").disabled = recording || !sample || !$("voice-name").value.trim();
}

function openSheet() {
  sample = null;
  $("voice-name").value = "";
  $("voice-error").textContent = "";
  renderSheet();
  openModal($("voice-sheet"));
}

// Образец, который не сохранили, удаляется: звук голоса не остаётся на устройстве.
function discardSample() {
  if (sample) invoke("discard_voice_recording", { path: sample.path }).catch(() => {});
  sample = null;
}

async function toggleRecording() {
  $("voice-error").textContent = "";
  if (recording) {
    await stopRecording();
    return;
  }
  discardSample();
  try {
    await startMicrophone(
      channel((event) => {
        if (event.type === "stopped") finish(event);
      }),
      true,
    );
    recording = true;
    startedAt = Date.now();
    timer = setInterval(() => {
      if (maxSampleSec !== null && (Date.now() - startedAt) / 1000 >= maxSampleSec) stopRecording();
      else renderSheet();
    }, 250);
    renderSheet();
  } catch (error) {
    $("voice-error").textContent = errorText(error);
  }
}

async function stopRecording() {
  if (!recording) return;
  // Таймер останавливается сразу: иначе пока идёт остановка, он позовёт её повторно.
  clearInterval(timer);
  try {
    finish(await invoke("stop_recording"));
  } catch (error) {
    finish(null);
    $("voice-error").textContent = errorText(error);
  }
}

function finish(saved) {
  if (!recording) return;
  recording = false;
  clearInterval(timer);
  sample = saved?.path ? { path: saved.path, durationSec: saved.durationSec } : null;
  if (saved && !saved.path) $("voice-error").textContent = t(K.recording_empty);
  renderSheet();
}

async function save() {
  if (!sample) return;
  const path = sample.path;
  $("voice-save").disabled = true;
  try {
    apply(await invoke("add_voice", { name: $("voice-name").value, path }));
    sample = null;
    $("voice-sheet").close();
  } catch (error) {
    // Запись образца ядро удаляет при любом исходе: нужна новая.
    sample = null;
    $("voice-error").textContent = errorText(error);
    renderSheet();
  }
}

function closeSheet() {
  if (recording) stopRecording().then(discardSample);
  else discardSample();
}

function askDelete(name) {
  pendingDelete = name;
  $("voice-delete-text").textContent = t(K.voices_delete_confirm, { name });
  openModal($("voice-delete-confirm"));
}

async function confirmDelete() {
  $("voice-delete-confirm").close();
  if (pendingDelete === null) return;
  try {
    apply(await invoke("delete_voice", { name: pendingDelete }));
    $("voices-error").textContent = "";
  } catch (error) {
    $("voices-error").textContent = errorText(error);
  }
  pendingDelete = null;
}

export function initVoices() {
  $("open-voices").addEventListener("click", openVoices);
  $("voices-back").addEventListener("click", () => history.back());
  $("voice-add").addEventListener("click", openSheet);
  $("voice-record").addEventListener("click", toggleRecording);
  $("voice-save").addEventListener("click", save);
  $("voice-name").addEventListener("input", renderSheet);
  $("voice-sheet").addEventListener("close", closeSheet);
  $("voice-card").addEventListener("close", () => {
    cardName = null;
  });
  $("voice-delete-yes").addEventListener("click", confirmDelete);
  $("voice-delete-no").addEventListener("click", () => {
    pendingDelete = null;
    $("voice-delete-confirm").close();
  });
  onLanguageChange(() => {
    renderSummary();
    renderList();
    renderSheet();
    renderCard();
  });
  // Список обновляется при каждом показе: так видны голоса, запомненные на встрече, с
  // которой вернулись.
  onScreenChange((name) => {
    if (name === "voices") loadVoices();
  });
  loadVoices();
}
