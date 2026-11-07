import { channel, errorCode, errorText, invoke } from "./api.js";
import { $ } from "./dom.js";
import { clock } from "./format.js";
import { K, locale, t } from "./i18n.js";
import { playbackRanges } from "./playback-spans.js";
import { onLanguageChange, onScreenChange } from "./screens.js";

const SPEEDS = [0.75, 1, 1.5, 2];
const SKIP_SEC = 10;

let opened = false;
let playing = false;
let position = 0;
let duration = 0;
let generation = 0;
// Все слова с временем по порядку и ходы: по ним ищется, что сейчас звучит.
let words = [];
let turns = [];
// Участники для выбора «слушать только одного»: id и имя по порядку.
let speakers = [];
let currentKey = null;
let currentTurn = null;
// Скорость и пропуск тишины переживают открытие другой записи; участник выбирается заново.
let speed = 1;
let skipSilence = false;
let speaker = null;
const changeListeners = [];

// Подписчик узнаёт позицию и открыт ли плеер после каждого события и закрытия.
export function onPlayerChange(listener) {
  changeListeners.push(listener);
}

function notify() {
  for (const listener of changeListeners) listener({ opened, position });
}

function buildIndex(transcript) {
  words = [];
  turns = transcript.turns.map((turn) => ({
    id: turn.id,
    speaker: turn.speaker,
    start: turn.start,
    end: turn.end,
    words: turn.words ?? [],
  }));
  for (const turn of transcript.turns) {
    (turn.words ?? []).forEach((w, i) => words.push({ turnId: turn.id, i, start: w.start, end: w.end }));
  }
  words.sort((a, b) => a.start - b.start);
  speakers = [...transcript.speakers]
    .sort((a, b) => a.id - b.id)
    .map((s) => ({ id: s.id, name: s.name || t(K.export_speaker_default, { n: s.id }) }));
}

// Последнее слово, начавшееся не позже позиции; подходит, только если ещё не кончилось.
function wordAt(sec) {
  let lo = 0;
  let hi = words.length - 1;
  let found = -1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (words[mid].start <= sec) {
      found = mid;
      lo = mid + 1;
    } else {
      hi = mid - 1;
    }
  }
  return found >= 0 && sec < words[found].end ? words[found] : null;
}

function turnAt(sec) {
  return turns.find((turn) => turn.start <= sec && sec < turn.end) ?? null;
}

// Подсветка звучащего слова, а если у хода нет времени слов, всего хода. Вызывается и
// после каждой перерисовки текста.
export function reapplyPlaying() {
  for (const node of document.querySelectorAll("#turns .playing")) node.classList.remove("playing");
  if (!opened || (!playing && position === 0)) return;
  const word = wordAt(position);
  if (word) {
    document
      .querySelector(`#turns .turn-text[data-turn-id="${word.turnId}"] .word[data-word="${word.i}"]`)
      ?.classList.add("playing");
    return;
  }
  const turn = turnAt(position);
  if (turn && !words.some((w) => w.turnId === turn.id)) {
    document.querySelector(`#turns .turn-text[data-turn-id="${turn.id}"]`)?.closest(".turn")?.classList.add("playing");
  }
}

// При переходе к новому ходу он прокручивается в видимую область, если ушёл за край.
function follow() {
  const turn = turnAt(position);
  const key = turn?.id ?? null;
  if (!playing || key === currentTurn) return;
  currentTurn = key;
  const node = document.querySelector("#turns .playing");
  if (!node) return;
  const box = node.getBoundingClientRect();
  if (box.top < 0 || box.bottom > window.innerHeight - $("player").offsetHeight) {
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    node.scrollIntoView({ block: "center", behavior: reduce ? "auto" : "smooth" });
  }
}

function renderBar() {
  notify();
  $("player").hidden = !opened;
  document.body.classList.toggle("has-player", opened);
  $("player-time").textContent = `${clock(position)} / ${clock(duration)}`;
  const toggle = $("player-toggle");
  const label = t(playing ? K.player_pause : K.player_play);
  toggle.setAttribute("aria-label", label);
  toggle.title = label;
  // У SVG нет свойства hidden, поэтому видимость значков задаётся стилем.
  $("player-icon-play").style.display = playing ? "none" : "";
  $("player-icon-pause").style.display = playing ? "" : "none";
}

// Выбор скорости, пропуска тишины и участника: состояние показывается всегда, даже до открытия.
function renderControls() {
  for (const button of document.querySelectorAll("#player-speeds .chip")) {
    button.setAttribute("aria-pressed", String(Number(button.dataset.speed) === speed));
  }
  $("player-skip-silence").setAttribute("aria-pressed", String(skipSilence));
  const select = $("player-speaker");
  select.replaceChildren(
    new Option(t(K.player_speaker_all), ""),
    ...speakers.map((s) => new Option(s.name, String(s.id))),
  );
  select.value = speaker === null ? "" : String(speaker);
}

function formatSpeed(value) {
  return new Intl.NumberFormat(locale(), { maximumFractionDigits: 2 }).format(value);
}

function note(text) {
  const box = $("playback-note");
  box.textContent = text;
  box.hidden = !text;
}

function onEvent(event) {
  if (!opened) return;
  if (event.type === "error") {
    opened = false;
    playing = false;
    note(t(K.error_playback_failed));
  } else {
    position = event.sec;
    playing = event.type === "playing" || event.type === "position";
  }
  renderBar();
  const key = wordAt(position);
  const next = key ? `${key.turnId}:${key.i}` : `t:${turnAt(position)?.id}`;
  if (next !== currentKey || !playing) {
    currentKey = next;
    reapplyPlaying();
  }
  follow();
}

// Текст хода поправили: время слов у него пропало, указатель слов строится заново.
export function refreshPlayerIndex(transcript) {
  buildIndex(transcript);
  if (speaker !== null && !speakers.some((s) => s.id === speaker)) speaker = null;
  currentKey = null;
  reapplyPlaying();
  renderControls();
  applyRanges();
}

// Отдаёт плееру, что играть: отрезки речи и реплик выбранного участника или вся запись.
// Возвращает запрос, чтобы дождаться его перед следующей командой.
function applyRanges() {
  if (!opened) return Promise.resolve();
  const ranges = playbackRanges(turns, { speaker, skipSilence });
  return invoke("set_playback_ranges", {
    ranges: ranges && ranges.map(([startSec, endSec]) => ({ startSec, endSec })),
  }).catch((error) => note(errorText(error)));
}

function applySpeed() {
  if (!opened) return;
  invoke("set_playback_speed", { speed }).catch((error) => note(errorText(error)));
}

export async function openPlayer(transcript) {
  await closePlayer();
  const mine = ++generation;
  buildIndex(transcript);
  speaker = null;
  note("");
  renderControls();
  try {
    const result = await invoke("open_playback", {
      meeting: transcript.created_at,
      onEvent: channel(onEvent),
      title: transcript.audio.file,
    });
    if (mine !== generation) return;
    opened = true;
    duration = result.durationSec;
    applySpeed();
    applyRanges();
  } catch (error) {
    if (mine !== generation) return;
    note(errorCode(error) === "no_audio" ? t(K.error_no_audio) : errorText(error));
  }
  renderBar();
}

export async function closePlayer() {
  generation += 1;
  const wasOpen = opened;
  opened = false;
  playing = false;
  position = 0;
  duration = 0;
  currentKey = null;
  currentTurn = null;
  renderBar();
  reapplyPlaying();
  if (wasOpen) await invoke("close_playback").catch(() => {});
}

// Играть с места. Если участник выбран, а место — реплика другого, выбор сбрасывается:
// иначе звук сразу ушёл бы к следующей реплике выбранного.
export async function playFrom(sec) {
  if (!opened) return;
  currentTurn = null;
  const turn = turnAt(sec);
  if (speaker !== null && turn && turn.speaker !== speaker) {
    speaker = null;
    renderControls();
    await applyRanges();
  }
  invoke("play_audio", { fromSec: Math.max(0, sec) }).catch((error) => note(errorText(error)));
}

function toggle() {
  if (!opened) return;
  if (playing) {
    invoke("pause_audio").catch((error) => note(errorText(error)));
    return;
  }
  // Дослушанная до конца запись начинается сначала.
  const fromSec = position >= duration - 0.05 ? 0 : null;
  invoke("play_audio", { fromSec }).catch((error) => note(errorText(error)));
}

function skip(deltaSec) {
  if (!opened) return;
  invoke("skip_audio", { deltaSec }).catch((error) => note(errorText(error)));
}

function setSpeed(value) {
  speed = value;
  renderControls();
  applySpeed();
}

function toggleSilence() {
  skipSilence = !skipSilence;
  renderControls();
  applyRanges();
}

function setSpeaker(value) {
  speaker = value === "" ? null : Number(value);
  renderControls();
  applyRanges();
}

function buildSpeedButtons() {
  const group = $("player-speeds");
  for (const value of SPEEDS) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "chip";
    button.dataset.speed = String(value);
    button.textContent = `${formatSpeed(value)}×`;
    button.addEventListener("click", () => setSpeed(value));
    group.append(button);
  }
}

export function initPlayer() {
  buildSpeedButtons();
  // Высота панели меняется, когда строки переносятся на узком экране: отступ под ней
  // должен следовать за ней, иначе последняя реплика уходит под панель.
  const bar = $("player");
  new ResizeObserver(() => document.body.style.setProperty("--player-height", `${bar.offsetHeight}px`)).observe(bar);
  $("player-toggle").addEventListener("click", toggle);
  $("player-back").addEventListener("click", () => skip(-SKIP_SEC));
  $("player-forward").addEventListener("click", () => skip(SKIP_SEC));
  $("player-skip-silence").addEventListener("click", toggleSilence);
  $("player-speaker").addEventListener("change", (event) => setSpeaker(event.target.value));
  onScreenChange((name) => {
    if (name !== "transcript") closePlayer();
  });
  onLanguageChange(() => {
    renderBar();
    renderControls();
  });
  renderBar();
  renderControls();
}
