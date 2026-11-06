import { channel, errorCode, errorText, invoke } from "./api.js";
import { $ } from "./dom.js";
import { clock } from "./format.js";
import { K, t } from "./i18n.js";
import { onLanguageChange, onScreenChange } from "./screens.js";

let opened = false;
let playing = false;
let position = 0;
let duration = 0;
let generation = 0;
// Все слова с временем по порядку и ходы: по ним ищется, что сейчас звучит.
let words = [];
let turns = [];
let currentKey = null;
let currentTurn = null;
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
  turns = transcript.turns.map((turn) => ({ id: turn.id, start: turn.start, end: turn.end }));
  for (const turn of transcript.turns) {
    (turn.words ?? []).forEach((w, i) => words.push({ turnId: turn.id, i, start: w.start, end: w.end }));
  }
  words.sort((a, b) => a.start - b.start);
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
  currentKey = null;
  reapplyPlaying();
}

export async function openPlayer(transcript) {
  await closePlayer();
  const mine = ++generation;
  buildIndex(transcript);
  note("");
  try {
    const result = await invoke("open_playback", { onEvent: channel(onEvent) });
    if (mine !== generation) return;
    opened = true;
    duration = result.durationSec;
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

export function playFrom(sec) {
  if (!opened) return;
  currentTurn = null;
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

export function initPlayer() {
  $("player-toggle").addEventListener("click", toggle);
  onScreenChange((name) => {
    if (name !== "transcript") closePlayer();
  });
  onLanguageChange(renderBar);
  renderBar();
}
