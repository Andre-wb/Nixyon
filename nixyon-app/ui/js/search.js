import { errorText, invoke } from "./api.js";
import { $ } from "./dom.js";
import { K, t } from "./i18n.js";
import { reapplyPlaying } from "./player.js";
import { onLanguageChange } from "./screens.js";
import { turnWordRanges } from "./transcript.js";
import { renderTurnText } from "./turn-text.js";

// Пауза после последнего нажатия: поиск идёт в Rust и не должен запускаться на каждую букву.
const DEBOUNCE_MS = 200;

let hits = [];
let index = 0;
let query = "";
let timer = null;
let generation = 0;

// Найденные места ходов: id хода и отрезки текста в единицах UTF-16.
function hitsByTurn() {
  const byTurn = new Map();
  hits.forEach((hit, i) => {
    if (!byTurn.has(hit.turnId)) byTurn.set(hit.turnId, []);
    byTurn.get(hit.turnId).push({ ...hit, i });
  });
  for (const list of byTurn.values()) list.sort((a, b) => a.start - b.start);
  return byTurn;
}

// Текст хода заново собирается из исходной строки: слова с временем и подсвеченные
// отрезки. Потом возвращается подсветка звучащего слова.
export function applySearch() {
  const byTurn = hitsByTurn();
  for (const node of document.querySelectorAll("#turns .turn-text")) {
    const text = node.dataset.text ?? node.textContent;
    node.dataset.text = text;
    const id = Number(node.dataset.turnId);
    const list = (byTurn.get(id) ?? []).map((hit) => ({ ...hit, current: hit.i === index }));
    renderTurnText(node, text, turnWordRanges(id), list);
  }
  renderCount();
  reapplyPlaying();
}

function renderCount() {
  const count = $("search-count");
  if (hits.length > 0) count.textContent = t(K.search_count, { current: index + 1, total: hits.length });
  else count.textContent = query ? t(K.search_none) : "";
  $("search-prev").disabled = hits.length < 2;
  $("search-next").disabled = hits.length < 2;
}

function scrollToCurrent() {
  const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  document.querySelector("#turns mark.current")?.scrollIntoView({
    block: "center",
    behavior: reduce ? "auto" : "smooth",
  });
}

function step(delta) {
  if (hits.length === 0) return;
  index = (index + delta + hits.length) % hits.length;
  applySearch();
  scrollToCurrent();
}

async function run(text) {
  const mine = ++generation;
  query = text.trim();
  if (!query) {
    hits = [];
    index = 0;
    applySearch();
    return;
  }
  try {
    const found = await invoke("search_transcript", { query });
    // Ответ на устаревший запрос не затирает более свежий.
    if (mine !== generation) return;
    hits = found;
    index = 0;
    applySearch();
    scrollToCurrent();
  } catch (error) {
    if (mine !== generation) return;
    hits = [];
    applySearch();
    $("search-count").textContent = errorText(error);
  }
}

// Новая расшифровка: прежний запрос сбрасывается.
export function resetSearch() {
  clearTimeout(timer);
  generation += 1;
  hits = [];
  index = 0;
  query = "";
  $("search").value = "";
  renderCount();
}

function placeholder() {
  $("search").placeholder = t(K.search_label);
}

export function initSearch() {
  const input = $("search");
  input.addEventListener("input", () => {
    clearTimeout(timer);
    timer = setTimeout(() => run(input.value), DEBOUNCE_MS);
  });
  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      step(e.shiftKey ? -1 : 1);
    }
  });
  $("search-prev").addEventListener("click", () => step(-1));
  $("search-next").addEventListener("click", () => step(1));
  placeholder();
  onLanguageChange(() => {
    placeholder();
    renderCount();
  });
}
