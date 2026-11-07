import { channel, errorText, invoke, isQuietError } from "./api.js";
import { $, dot, el, speakerColor } from "./dom.js";
import { clock } from "./format.js";
import { emotionChip, initEmotion, openEmotion } from "./emotion.js";
import { K, t } from "./i18n.js";
import { openExport } from "./export.js";
import { fillersAvailable, openFillers } from "./fillers.js";
import { meetingMeta, meetingTitle, openRename } from "./meeting-title.js";
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
// Своё название открытой встречи; null: встреча называется датой и временем.
let title = null;
let warnings = [];
let editing = null;
let pendingMerge = null;
let editingTurn = null;
// Повторная обработка реплики: идёт ли она, на какой реплике, ход и итог.
let checkRunning = false;
let checkTurn = null;
let checkProgress = null;
let checkResult = null;
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

export function open(transcript, newWarnings = [], newTitle = null) {
  current = transcript;
  title = newTitle;
  warnings = newWarnings;
  resetSearch();
  closePlayer();
  render();
  prepareNames(transcript);
  pushTranscript();
  openPlayer(transcript);
}

export function currentTranscript() {
  return current;
}

// id открытой встречи: команды ядра получают его параметром.
export function currentMeeting() {
  return current?.created_at ?? null;
}

export function setTranscript(transcript) {
  current = transcript;
  render();
  refreshPlayerIndex(transcript);
  for (const listener of changeListeners) listener(transcript);
}

function renderTitle() {
  $("transcript-file").textContent = meetingTitle(title, current.created_at);
  $("transcript-meta").textContent = meetingMeta(title, current.created_at, current.audio.duration_sec);
}

function render() {
  if (!current) return;
  renderTitle();
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
    const actions = el("span", "turn-actions");
    const checkButton = el("button", "icon-button turn-check");
    checkButton.type = "button";
    const checkLabel = t(K.transcript_recheck);
    checkButton.setAttribute("aria-label", checkLabel);
    checkButton.title = checkLabel;
    checkButton.append($("recheck-icon").content.cloneNode(true));
    checkButton.addEventListener("click", () => openTurnCheck(turn.id));
    const editButton = el("button", "icon-button turn-edit");
    editButton.type = "button";
    const editLabel = t(K.transcript_edit_text);
    editButton.setAttribute("aria-label", editLabel);
    editButton.title = editLabel;
    editButton.append($("edit-icon").content.cloneNode(true));
    editButton.addEventListener("click", () => openTurnEdit(turn.id));
    actions.append(checkButton, editButton);
    const meta = `${speakerName(current, turn.speaker)}, ${clock(turn.start)}`;
    const emotion = emotionChip(turn, () => openEmotion(turn, meta, setTranscript));
    head.append(el("span", "turn-name", speakerName(current, turn.speaker)), time, emotion, actions);
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
  $("turn-fillers").hidden = !fillersAvailable();
  openModal($("turn-sheet"));
}

// Удаление фразы необратимо: сначала подтверждение.
async function deleteTurn() {
  $("turn-delete-confirm").close();
  try {
    setTranscript(await invoke("delete_turn", { meeting: currentMeeting(), id: editingTurn }));
    $("turn-sheet").close();
  } catch (error) {
    $("turn-error").textContent = errorText(error);
  }
}

// Повторная обработка: фраза слушается заново и проверяется моделью, итог только
// показывается. Замена текста: обычная правка реплики, поэтому её можно отменить закрытием.
function openTurnCheck(id) {
  if (!checkRunning) {
    checkTurn = id;
    checkProgress = null;
    checkResult = null;
  }
  $("turn-check-error").textContent = "";
  renderTurnCheck();
  openModal($("turn-check-sheet"));
}

function renderTurnCheck() {
  const turn = current?.turns.find((x) => x.id === checkTurn);
  $("turn-check-meta").textContent = turn ? `${speakerName(current, turn.speaker)}, ${clock(turn.start)}` : "";
  $("turn-check-running").hidden = !checkRunning;
  const checking = checkProgress?.stage === "check";
  $("turn-check-phase").textContent = t(checking ? K.turn_check_checking : K.turn_check_listening);
  const share = checkProgress && checkProgress.total > 0 ? Math.min(1, checkProgress.done / checkProgress.total) : 0;
  $("turn-check-progress-fill").style.transform = `scaleX(${share})`;
  $("turn-check-progress").setAttribute("aria-valuenow", String(Math.round(share * 100)));
  const done = !checkRunning && checkResult !== null;
  const candidate = done ? checkCandidate() : "";
  // Сверка с текстом реплики сейчас: её могли поправить, пока шла обработка.
  const changed = done && candidate !== (turn?.text ?? "");
  $("turn-check-result").hidden = !done;
  $("turn-check-before").textContent = turn?.text ?? "";
  // Ответ модели недоверенный: только как текст, никогда как разметка.
  $("turn-check-after").textContent = candidate;
  $("turn-check-note").hidden = !(done && checkResult.correction == null);
  $("turn-check-same").hidden = !(done && !changed);
  $("turn-check-apply").hidden = !changed;
  $("turn-check-start").hidden = checkRunning;
}

// Итог обработки: исправление модели, а без него повторное распознавание.
function checkCandidate() {
  return checkResult.correction ?? checkResult.recognized;
}

async function startTurnCheck() {
  if (checkTurn === null) return;
  $("turn-check-error").textContent = "";
  checkRunning = true;
  checkProgress = null;
  checkResult = null;
  renderTurnCheck();
  try {
    checkResult = await invoke("reprocess_turn", {
      meeting: currentMeeting(),
      id: checkTurn,
      onProgress: channel((p) => {
        checkProgress = p;
        renderTurnCheck();
      }),
    });
  } catch (error) {
    if (!isQuietError(error)) $("turn-check-error").textContent = errorText(error);
  } finally {
    checkRunning = false;
    checkProgress = null;
    renderTurnCheck();
  }
}

// Отмена обеих стадий сразу: по ходу работы не видно, началась ли уже проверка моделью,
// пока модель не прислала первый отчёт. Команда для стадии, которая не идёт, ничего не делает.
function cancelTurnCheck() {
  invoke("cancel_transcription").catch(() => null);
  invoke("cancel_summary").catch(() => null);
}

async function applyTurnCheck() {
  if (!checkResult || checkTurn === null) return;
  const text = checkCandidate().replace(/[\r\n\t]+/g, " ");
  try {
    setTranscript(await invoke("edit_turn_text", { meeting: currentMeeting(), id: checkTurn, text }));
    checkResult = null;
    $("turn-check-sheet").close();
  } catch (error) {
    $("turn-check-error").textContent = errorText(error);
  }
}

// Перенос строки в реплике не нужен: вставленный текст склеивается через пробел.
async function saveTurnText() {
  const text = $("turn-text-input").value.replace(/[\r\n\t]+/g, " ");
  try {
    setTranscript(await invoke("edit_turn_text", { meeting: currentMeeting(), id: editingTurn, text }));
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
    setTranscript(await invoke("rename_speaker", { meeting: currentMeeting(), id: editing, name: $("speaker-name").value }));
    $("speaker-sheet").close();
  } catch (error) {
    $("speaker-error").textContent = errorText(error);
  }
}

async function rememberVoice() {
  try {
    await invoke("remember_voice", { meeting: currentMeeting(), speaker: editing });
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

// Удаление встречи необратимо: сначала подтверждение с её названием.
function askDeleteMeeting() {
  if (!current) return;
  $("meeting-delete-error").textContent = "";
  $("meeting-delete-text").textContent = t(K.meeting_delete_confirm, {
    title: meetingTitle(title, current.created_at),
  });
  openModal($("meeting-delete-confirm"));
}

// Плеер закрывается до удаления: стирается и файл, который он играет. После удаления
// экран встречи закрывается, список на главном перечитывается сам.
async function deleteMeeting() {
  $("meeting-delete-confirm").close();
  const meeting = currentMeeting();
  if (!meeting) return;
  await closePlayer();
  try {
    await invoke("delete_meeting", { meeting });
  } catch (error) {
    $("meeting-delete-error").textContent = errorText(error);
    if (currentMeeting() === meeting) openPlayer(current);
    return;
  }
  if (currentMeeting() === meeting) {
    current = null;
    history.back();
  }
}

async function merge() {
  $("merge-confirm").close();
  if (!pendingMerge) return;
  try {
    setTranscript(await invoke("merge_speakers", { meeting: currentMeeting(), ...pendingMerge }));
    $("speaker-sheet").close();
  } catch (error) {
    $("speaker-error").textContent = errorText(error);
  }
  pendingMerge = null;
}

export function initTranscript() {
  $("back").addEventListener("click", () => history.back());
  $("open-export").addEventListener("click", () => current && openExport(current));
  $("open-rename").addEventListener("click", () => {
    if (!current) return;
    const meeting = currentMeeting();
    openRename(meeting, title, (renamed) => {
      if (currentMeeting() !== meeting) return;
      title = renamed;
      renderTitle();
    });
  });
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
  $("meeting-delete").addEventListener("click", askDeleteMeeting);
  $("meeting-delete-no").addEventListener("click", () => $("meeting-delete-confirm").close());
  $("meeting-delete-yes").addEventListener("click", deleteMeeting);
  $("turn-save").addEventListener("click", saveTurnText);
  $("turn-fillers").addEventListener("click", () => {
    $("turn-sheet").close();
    openFillers([editingTurn]);
  });
  $("turn-delete").addEventListener("click", () => openModal($("turn-delete-confirm")));
  $("turn-delete-no").addEventListener("click", () => $("turn-delete-confirm").close());
  $("turn-delete-yes").addEventListener("click", deleteTurn);
  $("turn-check-start").addEventListener("click", startTurnCheck);
  $("turn-check-apply").addEventListener("click", applyTurnCheck);
  $("turn-check-cancel").addEventListener("click", cancelTurnCheck);
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
  initEmotion();
  onLanguageChange(render);
  onLanguageChange(renderTurnCheck);
}
