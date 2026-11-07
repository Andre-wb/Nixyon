import { errorText, invoke } from "./api.js";
import { $, el } from "./dom.js";
import { K, t } from "./i18n.js";
import { meetingMeta, meetingTitle } from "./meeting-title.js";
import { onLanguageChange, onScreenChange } from "./screens.js";
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

function render() {
  const list = $("meetings-list");
  list.replaceChildren();
  for (const meeting of meetings) {
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
  list.hidden = meetings.length === 0;
  $("meetings-empty").hidden = meetings.length > 0;
}

async function load() {
  const mine = ++loading;
  try {
    const found = await invoke("list_meetings");
    if (mine !== loading) return;
    meetings = found;
    $("meetings-error").textContent = "";
  } catch (error) {
    if (mine !== loading) return;
    meetings = [];
    $("meetings-error").textContent = errorText(error);
  }
  render();
}

async function openMeeting(id) {
  try {
    const opened = await invoke("open_meeting", { meeting: id });
    $("meetings-error").textContent = "";
    open(opened.transcript, [], opened.title ?? null);
  } catch (error) {
    $("meetings-error").textContent = errorText(error);
  }
}

// Список обновляется при каждом показе главного экрана: так в нём видны и новая встреча,
// и правки имён, и переименование.
export function initMeetings() {
  onScreenChange((name) => {
    if (name === "home") load();
  });
  onLanguageChange(render);
}
