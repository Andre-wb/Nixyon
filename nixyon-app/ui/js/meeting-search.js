import { errorText, invoke } from "./api.js";
import { $, el } from "./dom.js";
import { K, t } from "./i18n.js";
import { selectedLabels } from "./label-filter.js";
import { meetingTitle } from "./meeting-title.js";
import { onLanguageChange } from "./screens.js";

// Пауза после последнего нажатия: поиск идёт по всем встречам и не запускается на каждую
// букву.
const DEBOUNCE_MS = 300;
// Сколько знаков текста реплики видно по обе стороны от совпадения.
const CONTEXT = 40;

let query = "";
let results = null;
let error = "";
let timer = null;
let generation = 0;
let onOpen = () => {};
let onModeChange = () => {};

// Индексы JS в единицах UTF-16: край фрагмента сдвигается к пробелу, чтобы не резать
// слово, и не встаёт между половинками суррогатной пары.
function fragmentBounds(text, start, end) {
  let from = Math.max(0, start - CONTEXT);
  let to = Math.min(text.length, end + CONTEXT);
  if (from > 0) {
    const space = text.indexOf(" ", from);
    if (space !== -1 && space < start) from = space + 1;
  }
  if (to < text.length) {
    const space = text.lastIndexOf(" ", to);
    if (space >= end) to = space;
  }
  const low = (i) => i > 0 && i < text.length && /[\uDC00-\uDFFF]/.test(text[i]);
  if (low(from)) from += 1;
  if (low(to)) to -= 1;
  return [from, to];
}

function fragment(hit) {
  const { text, start, end } = hit;
  const [from, to] = fragmentBounds(text, start, end);
  const node = el("span", "meta hit-text");
  node.append(
    (from > 0 ? "…" : "") + text.slice(from, start),
    el("mark", "hit", text.slice(start, end)),
    text.slice(end, to) + (to < text.length ? "…" : ""),
  );
  return node;
}

function render() {
  const list = $("meeting-hits");
  const note = $("meeting-hits-note");
  list.replaceChildren();
  if (!query) {
    list.hidden = true;
    note.textContent = "";
    return;
  }
  const hits = results?.hits ?? [];
  for (const hit of hits) {
    const row = el("button", "meeting-row");
    row.type = "button";
    row.append(el("span", "speaker-name", meetingTitle(hit.title, hit.meeting)), fragment(hit));
    row.addEventListener("click", () =>
      onOpen(hit.meeting, { query, turnId: hit.turnId, start: hit.start }),
    );
    const item = el("li");
    item.append(row);
    list.append(item);
  }
  list.hidden = hits.length === 0;
  if (error) note.textContent = error;
  else if (!results) note.textContent = "";
  else if (hits.length === 0) note.textContent = t(K.search_none);
  else note.textContent = results.truncated ? t(K.home_search_truncated, { n: hits.length }) : "";
}

async function run() {
  const mine = ++generation;
  if (!query) {
    results = null;
    error = "";
    render();
    return;
  }
  try {
    const found = await invoke("search_meetings", { query, labels: selectedLabels() });
    // Ответ на устаревший запрос не затирает более свежий.
    if (mine !== generation) return;
    results = found;
    error = "";
  } catch (e) {
    if (mine !== generation) return;
    results = null;
    error = errorText(e);
  }
  render();
}

export function hasMeetingQuery() {
  return query !== "";
}

// Запрос остаётся, а набор встреч поменялся (фильтр, возврат на главный экран).
export function rerunMeetingSearch() {
  if (!query) return;
  clearTimeout(timer);
  run();
}

function placeholder() {
  $("meetings-search").placeholder = t(K.home_search_label);
}

// opened(meetingId, target): открыть встречу на совпадении. modeChanged: запрос появился
// или исчез, и вместо результатов снова нужен список.
export function initMeetingSearch(opened, modeChanged) {
  onOpen = opened;
  onModeChange = modeChanged;
  const input = $("meetings-search");
  input.addEventListener("input", () => {
    const had = query !== "";
    query = input.value.trim();
    if (had !== (query !== "")) onModeChange();
    clearTimeout(timer);
    if (!query) {
      run();
      return;
    }
    timer = setTimeout(run, DEBOUNCE_MS);
  });
  placeholder();
  onLanguageChange(() => {
    placeholder();
    render();
  });
}
