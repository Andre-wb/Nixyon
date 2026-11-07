import { channel, errorCode, errorText, invoke, startMicrophone } from "./api.js";
import { $ } from "./dom.js";
import { clock, dateTime } from "./format.js";
import { K, t } from "./i18n.js";
import { renderLive } from "./live.js";
import { onModelsChange, openModels, speechReady } from "./models.js";
import { showImport, transcribe } from "./processing.js";
import { onLanguageChange, show } from "./screens.js";
import { open, onTranscriptChange } from "./transcript.js";

const MIN_SPEAKERS = 1;
const MAX_SPEAKERS = 10;

const STOP_REASONS = {
  background: K.recording_stopped_background,
  interrupted: K.recording_stopped_interrupted,
  limit: K.recording_stopped_limit,
  storage: K.recording_stopped_storage,
};

let unknownCount = true;
let count = 2;
let recording = false;
let paused = false;
// Команда паузы ещё не ответила: второе нажатие в это время ничего не делает.
let pausing = false;
let liveLines = [];
let startedAt = 0;
// Пауза: когда её поставили и сколько всего длилась до текущей.
let pausedAt = 0;
let pausedMs = 0;
let timer = null;
let last = null;
// Запись, восстановленная после падения приложения, пока её не расшифровали.
let recovered = null;

function renderCount() {
  $("count-unknown").setAttribute("aria-pressed", String(unknownCount));
  $("count-value").textContent = String(count);
  document.querySelector(".stepper").classList.toggle("unknown", unknownCount);
  $("count-less").disabled = !unknownCount && count <= MIN_SPEAKERS;
  $("count-more").disabled = !unknownCount && count >= MAX_SPEAKERS;
}

function setCount(next) {
  unknownCount = false;
  count = Math.min(MAX_SPEAKERS, Math.max(MIN_SPEAKERS, next));
  renderCount();
}

function speakers() {
  return unknownCount ? null : count;
}

// Сообщение под кнопками: ошибка (красная полоса) или сведение (синяя). При отказе в
// микрофоне рядом кнопка, открывающая настройки Android; у восстановленной записи кнопка
// её расшифровки.
function notice(text, { info = false, code = null, offerRecovered = false } = {}) {
  const box = $("home-notice");
  box.hidden = !text;
  box.classList.toggle("info", info);
  $("home-notice-text").textContent = text ?? "";
  $("open-app-settings").hidden = code !== "mic_denied";
  $("recovered-transcribe").hidden = !offerRecovered;
}

// Плагин восстанавливает при запуске запись, оборванную падением приложения; её
// предлагается расшифровать один раз. Ошибка тут не важна: восстанавливать просто нечего.
async function offerRecovered() {
  let saved = null;
  try {
    saved = await invoke("recovered_recording");
  } catch {
    return;
  }
  if (!saved?.path) return;
  recovered = saved;
  notice(t(K.recording_recovered, { time: clock(saved.durationSec) }), { info: true, offerRecovered: true });
}

function transcribeRecovered() {
  if (!recovered) return;
  const { path } = recovered;
  recovered = null;
  notice(null);
  transcribe(path, recordingName(startOf(path)), speakers());
}

// Время начала записи стоит в имени её файла: rec-<миллисекунды>.wav.
function startOf(path) {
  const match = /rec-(\d+)\.wav$/.exec(path);
  return match ? new Date(Number(match[1])) : new Date();
}

// Сколько записано без пауз, в миллисекундах.
function elapsedMs(now = Date.now()) {
  return now - startedAt - pausedMs - (paused ? now - pausedAt : 0);
}

function renderRecorder() {
  $("screen-home").classList.toggle("recording", recording);
  $("screen-home").classList.toggle("paused", paused);
  $("record-label").textContent = t(recording ? K.home_stop : K.home_record);
  $("record").setAttribute("aria-label", t(recording ? K.home_stop : K.home_record));
  $("timer").hidden = !recording;
  if (recording) $("timer").textContent = `${t(paused ? K.home_paused : K.home_recording)}, ${clock(elapsedMs() / 1000)}`;
  $("record-pause").hidden = !recording;
  $("record-pause-label").textContent = t(paused ? K.home_resume : K.home_pause);
  $("import").disabled = recording;
}

function setLive(lines) {
  liveLines = lines;
  renderLive("home-live", lines);
}

function setLevel(dbfs) {
  const level = Math.min(1, Math.max(0, (dbfs + 60) / 60));
  $("level-ring").style.transform = `scale(${1 + level * 0.35})`;
}

async function startRecording() {
  notice(null);
  try {
    await startMicrophone(
      channel((event) => {
        if (event.type === "level" && !paused) setLevel(event.dbfs);
        if (event.type === "live") setLive(event.lines);
        if (event.type === "stopped") finishRecording(event, STOP_REASONS[event.reason]);
      }),
    );
    recording = true;
    startedAt = Date.now();
    timer = setInterval(renderRecorder, 250);
    renderRecorder();
  } catch (error) {
    notice(errorText(error), { code: errorCode(error) });
  }
}

async function stopRecording() {
  try {
    finishRecording(await invoke("stop_recording"), null);
  } catch (error) {
    resetRecorder();
    notice(errorText(error), { code: errorCode(error) });
  }
}

// Кнопка паузы: первое нажатие ставит запись на паузу, второе продолжает её в том же файле.
async function togglePause() {
  if (!recording || pausing) return;
  const pause = !paused;
  pausing = true;
  $("record-pause").disabled = true;
  try {
    await invoke(pause ? "pause_recording" : "resume_recording", {
      notificationTitle: t(pause ? K.recording_notification_paused : K.recording_notification_title),
    });
  } catch (error) {
    notice(errorText(error), { code: errorCode(error) });
    return;
  } finally {
    pausing = false;
    $("record-pause").disabled = false;
  }
  const now = Date.now();
  if (pause) pausedAt = now;
  else pausedMs += now - pausedAt;
  paused = pause;
  renderRecorder();
}

function resetRecorder() {
  recording = false;
  paused = false;
  pausedMs = 0;
  clearInterval(timer);
  setLevel(-90);
  setLive([]);
  renderRecorder();
}

// Записанное сразу уходит в обработку, живой текст остаётся на экране обработки; если
// запись остановилась сама, причина остаётся на главном экране.
function finishRecording(saved, reasonKey) {
  if (!recording) return;
  const live = liveLines;
  resetRecorder();
  if (reasonKey) notice(t(reasonKey), { info: true });
  if (!saved.path) {
    notice(t(K.recording_empty), { info: true });
    return;
  }
  transcribe(saved.path, recordingName(new Date(startedAt)), speakers(), live);
}

// Имя записи с микрофона вместо внутреннего имени файла: по нему называются и файлы
// экспорта, поэтому в нём нет двоеточий.
function recordingName(date) {
  const pad = (n) => String(n).padStart(2, "0");
  const day = `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
  return `${t(K.recording_name)} ${day} ${pad(date.getHours())}-${pad(date.getMinutes())}`;
}

async function importFile() {
  notice(null);
  try {
    const result = await invoke("import_audio", { onProgress: channel((p) => showImport(p)) });
    if (result.cancelled) {
      show("home");
      return;
    }
    transcribe(result.path, result.name, speakers());
  } catch (error) {
    show("home");
    if (errorCode(error) !== "cancelled") notice(errorText(error), { code: errorCode(error) });
  }
}

export function setLast(transcript) {
  last = transcript;
  $("last").hidden = !last;
  if (!last) return;
  $("last-file").textContent = last.audio.file;
  $("last-meta").textContent = `${dateTime(last.created_at)}, ${clock(last.audio.duration_sec)}`;
}

export function failed(text, code) {
  show("home");
  if (text) notice(text, { code });
}

// Пока модели речи не скачаны, записывать и расшифровывать нечем: вместо записи и
// выбора файла плашка с кнопкой загрузки. ready: модели только что скачались.
function renderModels(ready) {
  const missing = !speechReady();
  $("models-missing").hidden = !missing;
  $("count-group").hidden = missing;
  $("recorder").hidden = missing;
  $("import").hidden = missing;
  if (ready && !missing) notice(t(K.models_ready), { info: true });
}

export function initHome() {
  $("count-unknown").addEventListener("click", () => {
    unknownCount = !unknownCount;
    renderCount();
  });
  $("count-less").addEventListener("click", () => setCount(unknownCount ? count : count - 1));
  $("count-more").addEventListener("click", () => setCount(unknownCount ? count : count + 1));
  $("record").addEventListener("click", () => (recording ? stopRecording() : startRecording()));
  $("record-pause").addEventListener("click", togglePause);
  $("import").addEventListener("click", importFile);
  $("open-app-settings").addEventListener("click", () => invoke("open_app_settings").catch(() => null));
  $("recovered-transcribe").addEventListener("click", transcribeRecovered);
  $("last").addEventListener("click", () => last && open(last));
  $("models-open").addEventListener("click", openModels);
  onModelsChange(renderModels);
  onTranscriptChange(setLast);
  onLanguageChange(() => {
    renderRecorder();
    setLast(last);
  });
  renderCount();
  renderRecorder();
  offerRecovered();
}
