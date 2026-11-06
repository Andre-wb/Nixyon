import { channel, errorCode, errorText, invoke } from "./api.js";
import { $ } from "./dom.js";
import { clock } from "./format.js";
import { K, t } from "./i18n.js";
import { renderLive } from "./live.js";
import { onLanguageChange, show } from "./screens.js";
import { open } from "./transcript.js";

const PHASES = {
  importing: K.processing_importing,
  prepare: K.processing_prepare,
  recognize: K.processing_recognize,
  voices: K.processing_voices,
};

let phase = "prepare";
let done = 0;
let total = 0;
let mode = null;
let onFailure = () => {};

function render() {
  $("processing-phase").textContent = t(PHASES[phase] ?? K.processing_prepare);
  const share = total > 0 ? Math.min(1, done / total) : 0;
  $("progress-fill").style.transform = `scaleX(${share})`;
  $("progress").setAttribute("aria-valuenow", String(Math.round(share * 100)));
  $("processing-time").textContent = total > 0 ? t(K.processing_progress, { done: clock(done), total: clock(total) }) : "";
}

function update(next) {
  phase = next.phase ?? phase;
  done = next.doneSec ?? done;
  total = next.totalSec ?? total;
  render();
}

// Импорт: выбор файла закрылся, файл раскодируется.
export function showImport(progress) {
  mode = "import";
  update({ phase: "importing", ...progress });
  show("processing");
}

// live: строки живого текста записи, видны до готовой расшифровки.
export async function transcribe(path, name, speakers, live = []) {
  mode = "transcribe";
  renderLive("processing-live", live);
  update({ phase: "prepare", doneSec: 0, totalSec: 0 });
  show("processing");
  try {
    const result = await invoke("transcribe", {
      path,
      name,
      speakers,
      onProgress: channel((p) => update(p)),
    });
    open(result.transcript, result.warnings);
  } catch (error) {
    onFailure(errorCode(error) === "cancelled" ? null : errorText(error), errorCode(error));
  } finally {
    mode = null;
    renderLive("processing-live", []);
  }
}

export function initProcessing(failure) {
  onFailure = failure;
  $("cancel").addEventListener("click", () => {
    invoke(mode === "import" ? "cancel_import" : "cancel_transcription").catch(() => null);
  });
  onLanguageChange(render);
}
