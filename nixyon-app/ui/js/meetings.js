import { errorText, invoke } from "./api.js";
import { $, el } from "./dom.js";
import { setLast } from "./home.js";
import { K, t } from "./i18n.js";
import { initLabelFilter, loadLabelFilter, matchesFilter } from "./label-filter.js";
import { hasMeetingQuery, initMeetingSearch, rerunMeetingSearch } from "./meeting-search.js";
import { meetingMeta, meetingTitle } from "./meeting-title.js";
import { onLanguageChange, onScreenChange } from "./screens.js";
import { searchFor } from "./search.js";
import { open } from "./transcript.js";

// Сколько участников видно в строке встречи; остальные показаны числом.
const SHOWN_PARTICIPANTS = 3;

let meetings = [];
// Номер последней загрузки списка: ответ более ранней загрузки не затирает новый.
let loading = 0;

function participantsText(participants) {
  if (!participants?.length) return "";
  const names = participants
    .slice(0, SHOWN_PARTICIPANTS)
    .map((p) => p.name || t(K.export_speaker_default, { n: p.id }))
    .join(", ");
  const rest = participants.length - SHOWN_PARTICIPANTS;
  return rest > 0 ? `${names} ${t(K.home_meetings_more, { n: rest })}` : names;
}

// Пока в поле поиска есть запрос, вместо списка видны найденные места.
function render() {
  const list = $("meetings-list");
  list.replaceChildren();
  const searching = hasMeetingQuery();
  const shown = meetings.filter(matchesFilter);
  for (const meeting of shown) {
    const row = el("button", "meeting-row");
    row.type = "button";
    row.append(
      el("span", "speaker-name", meetingTitle(meeting.title, meeting.id)),
      el("span", "meta", meetingMeta(meeting.title, meeting.id, meeting.duration_sec)),
    );
    const who = participantsText(meeting.participants);
    if (who) row.append(el("span", "meta", who));
    row.addEventListener("click", () => openMeeting(meeting.id));
    const item = el("li");
    item.append(row);
    list.append(item);
  }
  list.hidden = searching || shown.length === 0;
  $("meetings-empty").hidden = searching || meetings.length > 0;
  $("meetings-filter-empty").hidden = searching || meetings.length === 0 || shown.length > 0;
}

async function load() {
  const mine = ++loading;
  try {
    const [found] = await Promise.all([invoke("list_meetings"), loadLabelFilter()]);
    if (mine !== loading) return;
    meetings = found;
    $("meetings-error").textContent = "";
  } catch (error) {
    if (mine !== loading) return;
    meetings = [];
    $("meetings-error").textContent = errorText(error);
  }
  // Карточка «Продолжить» на главном экране берёт из архива последнюю открытую встречу.
  setLast(meetings);
  render();
  rerunMeetingSearch();
}

// Открывает встречу архива на экране расшифровки; ошибку отдаёт вызывающему.
export async function showMeeting(id) {
  const opened = await invoke("open_meeting", { meeting: id });
  open(opened.transcript, [], opened.title ?? null, opened.has_audio);
}

// target: совпадение из поиска по встречам ({ query, turnId, start }); встреча
// открывается с тем же запросом на этой реплике.
async function openMeeting(id, target = null) {
  try {
    await showMeeting(id);
    $("meetings-error").textContent = "";
    if (target) searchFor(target.query, target);
  } catch (error) {
    $("meetings-error").textContent = errorText(error);
  }
}

// Список обновляется при каждом показе главного экрана: так в нём видны и новая встреча,
// и правки имён, и переименование.
export function initMeetings() {
  initMeetingSearch(openMeeting, render);
  initLabelFilter(() => {
    render();
    rerunMeetingSearch();
  });
  onScreenChange((name) => {
    if (name === "home") load();
  });
  onLanguageChange(render);
}
