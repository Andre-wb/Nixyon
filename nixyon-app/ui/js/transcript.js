import { errorText, invoke } from "./api.js";
import { $, dot, el, speakerColor } from "./dom.js";
import { clock, dateTime } from "./format.js";
import { K, t } from "./i18n.js";
import { openExport } from "./export.js";
import { closePlayer, openPlayer, playFrom, refreshPlayerIndex } from "./player.js";
import { applySearch, resetSearch } from "./search.js";
import { openStats } from "./stats.js";
import { onLanguageChange, openModal, pushTranscript } from "./screens.js";
import { renderTimeline } from "./timeline.js";
import { wordRanges } from "./turn-text.js";
import { prepareNames } from "./names.js";
import { loadVoices } from "./voices.js";

const OVERLAP_FLAG = "overlap";

let current = null;
let warnings = [];
let editing = null;
let pendingMerge = null;
let editingTurn = null;
const changeListeners = [];
// Места слов с временем в тексте каждого хода: id хода превращается в список отрезков.
let ranges = new Map();

export function turnWordRanges(id) {
  return ranges.get(id) ?? [];
}

export function onTranscriptChange(listener) {
  changeListeners.push(listener);
}

// Метка голоса: возраст и пол вместе, что из них известно; пусто, если модель не ответила.
const VOICE_LABELS = {
  "child/female": K.speaker_voice_girl,
  "child/male": K.speaker_voice_boy,
  "adult/female": K.speaker_voice_woman,
  "adult/male": K.speaker_voice_man,
  "/female": K.speaker_voice_female,
  "/male": K.speaker_voice_male,
  "child/": K.speaker_age_child,
  "adult/": K.speaker_age_adult,
};

function voiceLabel(speaker) {
  const key = VOICE_LABELS[`${speaker?.age ?? ""}/${speaker?.gender ?? ""}`];
  return key ? t(key) : "";
}

export function speakerName(transcript, id) {
  const speaker = transcript.speakers.find((s) => s.id === id);
  return speaker?.name || t(K.export_speaker_default, { n: id });
}

export function open(transcript, newWarnings = []) {
  current = transcript;
  warnings = newWarnings;
  resetSearch();
  closePlayer();
  render();
  prepareNames(transcript);
  pushTranscript();
  openPlayer(transcript);
}

export function setTranscript(transcript) {
  current = transcript;
  render();
  refreshPlayerIndex(transcript);
  for (const listener of changeListeners) listener(transcript);
}

function render() {
  if (!current) return;
  $("transcript-file").textContent = current.audio.file;
  $("transcript-meta").textContent = `${dateTime(current.created_at)}, ${clock(current.audio.duration_sec)}`;
  $("transcript-warning").hidden = !warnings.includes("clipping");

  const speakers = $("speakers");
  speakers.replaceChildren();
  const ordered = [...current.speakers].sort((a, b) => b.talk_time_sec - a.talk_time_sec || a.id - b.id);
  for (const s of ordered) {
    const row = el("button", "speaker-row");
    row.type = "button";
    const talk = t(K.transcript_talk_time, { time: clock(s.talk_time_sec) });
    const age = voiceLabel(s);
    row.append(
      dot(s.id),
      el("span", "speaker-name", speakerName(current, s.id)),
      el("span", "meta", age ? `${age}, ${talk}` : talk),
    );
    row.addEventListener("click", () => openSpeaker(s.id));
    const item = el("li");
    item.append(row);
    speakers.append(item);
  }

  renderTimeline(current);

  const turns = $("turns");
  const fragment = document.createDocumentFragment();
  ranges = new Map(current.turns.map((turn) => [turn.id, wordRanges(turn.text, turn.words)]));
  for (const turn of current.turns) {
    const item = el("li", "turn");
    item.style.setProperty("--speaker-color", speakerColor(turn.speaker));
    const head = el("div", "turn-head");
    // Время хода: кнопка «слушать с этого места», доступная и без тапа по словам.
    const time = el("button", "turn-time", clock(turn.start));
    time.type = "button";
    time.setAttribute("aria-label", t(K.transcript_play_from, { time: clock(turn.start) }));
    time.addEventListener("click", () => playFrom(turn.start));
    const editButton = el("button", "icon-button turn-edit");
    editButton.type = "button";
    const editLabel = t(K.transcript_edit_text);
    editButton.setAttribute("aria-label", editLabel);
    editButton.title = editLabel;
    editButton.append($("edit-icon").content.cloneNode(true));
    editButton.addEventListener("click", () => openTurnEdit(turn.id));
    head.append(el("span", "turn-name", speakerName(current, turn.speaker)), time, editButton);
    if (turn.flags?.includes(OVERLAP_FLAG)) head.append(el("span", "turn-flag", t(K.transcript_overlap)));
    const text = el("p", "turn-text", turn.text);
    text.dataset.turnId = String(turn.id);
    // Тап по слову играет с него; у хода без времени слов тап играет ход с начала.
    text.addEventListener("click", (event) => {
      const word = event.target.closest?.(".word");
      if (word && turn.words) playFrom(turn.words[Number(word.dataset.word)].start);
      else if (!turn.words?.length) playFrom(turn.start);
    });
    item.append(head, text);
    fragment.append(item);
  }
  turns.replaceChildren(fragment);
  applySearch();
}

function openTurnEdit(id) {
  const turn = current.turns.find((x) => x.id === id);
  if (!turn) return;
  editingTurn = id;
  $("turn-sheet-meta").textContent = `${speakerName(current, turn.speaker)}, ${clock(turn.start)}`;
  $("turn-text-input").value = turn.text;
  $("turn-error").textContent = "";
  openModal($("turn-sheet"));
}

// Перенос строки в реплике не нужен: вставленный текст склеивается через пробел.
async function saveTurnText() {
  const text = $("turn-text-input").value.replace(/[\r\n\t]+/g, " ");
  try {
    setTranscript(await invoke("edit_turn_text", { id: editingTurn, text }));
    $("turn-sheet").close();
  } catch (error) {
    $("turn-error").textContent = errorText(error);
  }
}

function openSpeaker(id) {
  editing = id;
  const speaker = current.speakers.find((s) => s.id === id);
  $("speaker-sheet-title").textContent = speakerName(current, id);
  const age = voiceLabel(speaker);
  $("speaker-age").textContent = age;
  $("speaker-age").hidden = !age;
  const input = $("speaker-name");
  input.value = speaker?.name ?? "";
  input.placeholder = t(K.export_speaker_default, { n: id });
  $("speaker-error").textContent = "";
  // Запомнить можно только голос участника с именем: под этим именем он и сохранится.
  $("remember-block").hidden = !speaker?.name;
  $("speaker-remember-status").textContent = "";
  const targets = $("merge-targets");
  targets.replaceChildren();
  const others = current.speakers.filter((s) => s.id !== id);
  $("merge-block").hidden = others.length === 0;
  for (const other of others) {
    const row = el("button", "speaker-row");
    row.type = "button";
    row.append(dot(other.id), el("span", "speaker-name", speakerName(current, other.id)), el("span"));
    row.addEventListener("click", () => askMerge(id, other.id));
    const item = el("li");
    item.append(row);
    targets.append(item);
  }
  openModal($("speaker-sheet"));
}

async function saveName() {
  try {
    setTranscript(await invoke("rename_speaker", { id: editing, name: $("speaker-name").value }));
    $("speaker-sheet").close();
  } catch (error) {
    $("speaker-error").textContent = errorText(error);
  }
}

async function rememberVoice() {
  try {
    await invoke("remember_voice", { speaker: editing });
    $("speaker-remember-status").textContent = t(K.speaker_remembered);
    $("speaker-error").textContent = "";
    loadVoices();
  } catch (error) {
    $("speaker-error").textContent = errorText(error);
  }
}

// Слияние необратимо: сначала подтверждение с именами обоих участников.
function askMerge(absorb, keep) {
  pendingMerge = { absorb, keep };
  $("merge-confirm-text").textContent = t(K.speaker_merge_confirm, {
    from: speakerName(current, absorb),
    into: speakerName(current, keep),
  });
  openModal($("merge-confirm"));
}

async function merge() {
  $("merge-confirm").close();
  if (!pendingMerge) return;
  try {
    setTranscript(await invoke("merge_speakers", pendingMerge));
    $("speaker-sheet").close();
  } catch (error) {
    $("speaker-error").textContent = errorText(error);
  }
  pendingMerge = null;
}

export function initTranscript() {
  $("back").addEventListener("click", () => history.back());
  $("open-export").addEventListener("click", () => current && openExport(current));
  $("open-stats").addEventListener("click", () => current && openStats(current));
  $("speaker-save").addEventListener("click", saveName);
  $("speaker-remember").addEventListener("click", rememberVoice);
  $("speaker-name").addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      saveName();
    }
  });
  $("merge-yes").addEventListener("click", merge);
  $("turn-save").addEventListener("click", saveTurnText);
  $("turn-text-input").addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      saveTurnText();
    }
  });
  $("merge-no").addEventListener("click", () => {
    pendingMerge = null;
    $("merge-confirm").close();
  });
  onLanguageChange(render);
}
