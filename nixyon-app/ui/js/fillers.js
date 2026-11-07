import { channel, errorCode, errorText, invoke } from "./api.js";
import { $, el } from "./dom.js";
import { clock } from "./format.js";
import { K, t } from "./i18n.js";
import { onModelsChange } from "./models.js";
import { onLanguageChange, openModal } from "./screens.js";
import { currentTranscript, setTranscript, speakerName } from "./transcript.js";

// Слова-паразиты языковой моделью. Модель работает минуты, поэтому лист можно закрыть:
// проверка идёт дальше, результат виден при новом открытии. Предложенные правки не
// сохраняются, пока не нажата «Заменить».
let available = false;
let picked = new Set();
let running = false;
let progress = null;
let result = null;
// Расшифровка, для которой получен `result`: после новой записи старые правки не нужны.
let resultFor = null;

export function fillersAvailable() {
  return available;
}

function turns() {
  return currentTranscript()?.turns ?? [];
}

function renderList() {
  const transcript = currentTranscript();
  const list = $("filler-list");
  list.replaceChildren();
  for (const turn of turns()) {
    const box = el("input");
    box.type = "checkbox";
    box.className = "select-circle";
    box.value = String(turn.id);
    box.checked = picked.has(turn.id);
    box.addEventListener("change", () => {
      if (box.checked) picked.add(turn.id);
      else picked.delete(turn.id);
      renderPick();
    });
    const body = el("span", "filler-body");
    body.append(
      el("span", "filler-who", `${speakerName(transcript, turn.speaker)}, ${clock(turn.start)}`),
      el("span", "filler-text", turn.text),
    );
    const label = el("label", "filler-item");
    label.append(box, body);
    const item = el("li");
    item.append(label);
    list.append(item);
  }
}

function renderPick() {
  const all = turns();
  $("filler-all").checked = all.length > 0 && picked.size === all.length;
  $("filler-count").textContent = t(K.fillers_selected, { n: picked.size });
  $("filler-run-selected").disabled = running || picked.size === 0;
  $("filler-run-all").disabled = running || all.length === 0;
}

function render() {
  $("filler-running").hidden = !running;
  $("filler-pick").hidden = running;
  const share = progress && progress.total > 0 ? Math.min(1, progress.done / progress.total) : 0;
  $("filler-progress-fill").style.transform = `scaleX(${share})`;
  $("filler-progress").setAttribute("aria-valuenow", String(Math.round(share * 100)));

  const done = !running && result !== null;
  const changes = done ? result.changes : [];
  $("filler-result").hidden = !done;
  $("filler-found").textContent = done ? t(K.fillers_found, { n: changes.length }) : "";
  $("filler-skipped").hidden = !(done && result.skipped > 0);
  $("filler-skipped").textContent = done ? t(K.fillers_skipped, { n: result?.skipped ?? 0 }) : "";
  $("filler-unanswered").hidden = !(done && result.unanswered > 0);
  $("filler-unanswered").textContent = done ? t(K.fillers_unanswered, { n: result?.unanswered ?? 0 }) : "";
  $("filler-empty").hidden = !(done && changes.length === 0);
  $("filler-apply").hidden = changes.length === 0;
  $("filler-apply").textContent = t(K.fillers_apply, { n: changes.length });
  // Ответ модели недоверенный: только как текст, никогда как разметка.
  const rows = changes.map((change) => {
    const item = el("li", "filler-change");
    item.append(
      el("p", "meta", t(K.turn_check_before)),
      el("p", "turn-check-text", change.before),
      ...(change.delete
        ? [el("p", "filler-delete", t(K.fillers_will_delete))]
        : [el("p", "meta", t(K.turn_check_after)), el("p", "turn-check-text", change.after)]),
    );
    return item;
  });
  $("filler-changes").replaceChildren(...rows);
  renderPick();
}

// Пустой список фраз означает весь диалог; иначе только выбранные, в порядке диалога.
async function run(ids) {
  $("filler-error").textContent = "";
  $("filler-note").textContent = "";
  running = true;
  progress = null;
  result = null;
  const transcript = currentTranscript();
  render();
  try {
    resultFor = transcript;
    result = await invoke("remove_filler_words", {
      ids,
      onProgress: channel((p) => {
        progress = p;
        render();
      }),
    });
  } catch (error) {
    if (errorCode(error) !== "cancelled") $("filler-error").textContent = errorText(error);
  } finally {
    running = false;
    progress = null;
    render();
  }
}

async function apply() {
  if (!result?.changes.length) return;
  const changes = result.changes.map(({ id, before, after, delete: remove }) => ({ id, before, after, delete: remove }));
  try {
    const applied = await invoke("apply_turn_texts", { changes });
    setTranscript(applied.transcript);
    result = null;
    // Не вставшие правки не теряются молча: лист остаётся открытым и говорит, сколько их.
    if (applied.skipped > 0) {
      renderList();
      render();
      $("filler-note").textContent = t(K.fillers_apply_skipped, { n: applied.skipped });
    } else {
      $("filler-sheet").close();
    }
  } catch (error) {
    $("filler-error").textContent = errorText(error);
  }
}

// Лист открывается с уже выбранными фразами, если его вызвали из реплики. Готовый
// результат проверки остаётся, пока открыта та же расшифровка.
export function openFillers(preselected = []) {
  if (!running) {
    if (!sameRecording(resultFor, currentTranscript())) result = null;
    if (preselected.length > 0 || result === null) picked = new Set(preselected);
    $("filler-error").textContent = "";
    $("filler-note").textContent = "";
    renderList();
  }
  render();
  openModal($("filler-sheet"));
}

// Правка реплики заменяет объект расшифровки, но запись та же.
function sameRecording(a, b) {
  return a !== null && b !== null && a.created_at === b.created_at;
}

async function checkAvailable() {
  available = await invoke("summary_available").catch(() => false);
  $("open-fillers").hidden = !available;
}

// Кнопки видны, только если на телефоне есть языковая модель; после загрузки моделей
// это проверяется заново.
export async function initFillers() {
  $("open-fillers").addEventListener("click", () => openFillers());
  $("filler-run-selected").addEventListener("click", () => run([...picked].sort((a, b) => a - b)));
  $("filler-run-all").addEventListener("click", () => run(null));
  $("filler-cancel").addEventListener("click", () => invoke("cancel_summary").catch(() => null));
  $("filler-apply").addEventListener("click", apply);
  $("filler-all").addEventListener("change", (event) => {
    const on = event.target.checked;
    for (const box of $("filler-list").querySelectorAll("input")) box.checked = on;
    picked = new Set(on ? turns().map((turn) => turn.id) : []);
    renderPick();
  });
  onLanguageChange(render);
  onModelsChange(checkAvailable);
  await checkAvailable();
}
