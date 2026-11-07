import { errorText, invoke } from "./api.js";
import { $ } from "./dom.js";
import { clock, dateTime, longDateTime } from "./format.js";
import { openModal } from "./screens.js";

// Своё название встречи или, если его нет, дата и время на языке интерфейса.
export function meetingTitle(title, createdAt) {
  return title || longDateTime(createdAt);
}

// Строка под названием: дата нужна, только когда название своё, иначе она повторилась бы.
export function meetingMeta(title, createdAt, durationSec) {
  return title ? `${dateTime(createdAt)}, ${clock(durationSec)}` : clock(durationSec);
}

let meeting = null;
let onRenamed = null;

// Лист переименования встречи. renamed получает новое название или null.
export function openRename(id, title, renamed) {
  meeting = id;
  onRenamed = renamed;
  $("meeting-title-input").value = title ?? "";
  $("meeting-error").textContent = "";
  openModal($("meeting-sheet"));
}

async function save() {
  if (!meeting) return;
  try {
    const title = await invoke("rename_meeting", { meeting, title: $("meeting-title-input").value });
    onRenamed?.(title ?? null);
    $("meeting-sheet").close();
  } catch (error) {
    $("meeting-error").textContent = errorText(error);
  }
}

export function initMeetingTitle() {
  $("meeting-save").addEventListener("click", save);
  $("meeting-title-input").addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      save();
    }
  });
}
