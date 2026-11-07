import { channel, errorCode, errorText, invoke } from "./api.js";
import { $, el } from "./dom.js";
import { K, t } from "./i18n.js";
import { onModelsChange } from "./models.js";
import { onLanguageChange, openModal } from "./screens.js";
import { onTranscriptChange, setTranscript, speakerName } from "./transcript.js";

// Подсказка имён участников той же языковой моделью, что и протокол: кто представился
// или к кому обратились по имени. Имя только предлагается, подписывает пользователь.
// Лист можно закрыть: модель доработает, подсказки будут видны при новом открытии.
let available = false;
let transcript = null;
let running = false;
let progress = null;
// null: для этой расшифровки ещё не искали.
let suggestions = null;

function unnamed(id) {
  return !transcript?.speakers.find((s) => s.id === id)?.name?.trim();
}

function renderButton() {
  $("open-names").hidden = !(available && transcript?.speakers.some((s) => unnamed(s.id)));
}

// Реплика, из которой видно имя: показывается рядом с подсказкой.
function turnText(id) {
  return transcript?.turns.find((turn) => turn.id === id)?.text ?? "";
}

function render() {
  renderButton();
  $("names-running").hidden = !running;
  $("names-start").hidden = running;
  const share = progress && progress.total > 0 ? Math.min(1, progress.done / progress.total) : 0;
  $("names-progress-fill").style.transform = `scaleX(${share})`;
  $("names-progress").setAttribute("aria-valuenow", String(Math.round(share * 100)));

  // Подписанные участники из списка уходят.
  const open = (suggestions ?? []).filter((s) => unnamed(s.speaker));
  const list = $("names-list");
  list.replaceChildren();
  for (const suggestion of open) list.append(card(suggestion));
  $("names-none").hidden = running || suggestions === null || open.length > 0;
}

// Имя и реплика пришли от модели: только как текст, никогда как разметка.
function card(suggestion) {
  const item = el("li", "card name-suggestion");
  const text = t(K.names_suggestion, { speaker: speakerName(transcript, suggestion.speaker), name: suggestion.name });
  const actions = el("div", "name-actions");
  const accept = el("button", "button button-primary", t(K.names_accept));
  accept.type = "button";
  accept.addEventListener("click", () => acceptName(suggestion));
  const skip = el("button", "button button-secondary", t(K.names_skip));
  skip.type = "button";
  skip.addEventListener("click", () => {
    suggestions = suggestions.filter((s) => s !== suggestion);
    render();
  });
  actions.append(accept, skip);
  item.append(el("p", "speaker-name", text), el("p", "meta", turnText(suggestion.turn)), actions);
  return item;
}

async function acceptName(suggestion) {
  $("names-error").textContent = "";
  try {
    setTranscript(await invoke("rename_speaker", { id: suggestion.speaker, name: suggestion.name }));
  } catch (error) {
    $("names-error").textContent = errorText(error);
  }
}

async function start() {
  $("names-error").textContent = "";
  running = true;
  progress = null;
  render();
  try {
    suggestions = await invoke("suggest_speaker_names", {
      onProgress: channel((p) => {
        progress = p;
        render();
      }),
    });
  } catch (error) {
    const code = errorCode(error);
    if (code === "llm_failed") $("names-error").textContent = t(K.names_failed);
    else if (code !== "cancelled") $("names-error").textContent = errorText(error);
  } finally {
    running = false;
    render();
  }
}

function openNames() {
  $("names-error").textContent = "";
  render();
  openModal($("names-sheet"));
}

// Новая расшифровка: прежние подсказки ей не годятся.
export function prepareNames(next) {
  if (next?.created_at !== transcript?.created_at) suggestions = null;
  transcript = next;
  render();
}

async function checkAvailable() {
  available = await invoke("summary_available").catch(() => false);
  renderButton();
}

// Кнопка видна, только если на телефоне есть языковая модель и кто-то подписан «Спикер N»;
// после загрузки моделей наличие модели проверяется заново.
export async function initNames() {
  $("open-names").addEventListener("click", openNames);
  $("names-start").addEventListener("click", start);
  $("names-cancel").addEventListener("click", () => invoke("cancel_summary").catch(() => null));
  onTranscriptChange(prepareNames);
  onLanguageChange(render);
  onModelsChange(checkAvailable);
  await checkAvailable();
}
