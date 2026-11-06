import { $, el, speakerColor } from "./dom.js";
import { clock } from "./format.js";
import { K, t } from "./i18n.js";
import { onPlayerChange, playFrom } from "./player.js";
import { speakerName } from "./transcript.js";

// Шаг перемотки стрелками с клавиатуры.
const KEY_STEP_SEC = 5;

let duration = 0;
let opened = false;
let position = 0;

function percent(sec) {
  return duration > 0 ? `${((Math.min(Math.max(sec, 0), duration) / duration) * 100).toFixed(3)}%` : "0%";
}

// Полосы говорящих: по строке на участника в порядке списка участников, ходы на общей
// шкале времени записи. Поверх полос лежит область перемотки с линией позиции.
export function renderTimeline(current) {
  duration = current.audio.duration_sec;
  const lanes = $("timeline-lanes");
  lanes.replaceChildren();
  const ordered = [...current.speakers].sort((a, b) => b.talk_time_sec - a.talk_time_sec || a.id - b.id);
  // Ряды заданы явно: область перемотки лежит поверх рядов, и без явных номеров сетка
  // расставила бы имена и полосы вокруг неё.
  ordered.forEach((s, row) => {
    const name = el("span", "timeline-name", speakerName(current, s.id));
    name.style.gridRow = String(row + 1);
    lanes.append(name);
    const track = el("div", "timeline-track");
    track.style.gridRow = String(row + 1);
    track.setAttribute("aria-hidden", "true");
    for (const turn of current.turns.filter((x) => x.speaker === s.id)) {
      const bar = el("span", "timeline-turn");
      bar.style.left = percent(turn.start);
      bar.style.width = percent(turn.end - turn.start);
      bar.style.setProperty("--speaker-color", speakerColor(s.id));
      track.append(bar);
    }
    lanes.append(track);
  });
  // Область перемотки накрывает полосы всех участников, но не ось времени под ними.
  const seek = el("div", "timeline-seek");
  seek.id = "timeline-seek";
  seek.style.gridRow = `1 / span ${Math.max(ordered.length, 1)}`;
  seek.append(el("span", "timeline-playhead"));
  seek.addEventListener("click", onTap);
  seek.addEventListener("keydown", onKey);
  const axis = el("div", "timeline-axis");
  axis.setAttribute("aria-hidden", "true");
  axis.style.gridRow = String(ordered.length + 1);
  axis.append(el("span", null, clock(0)), el("span", null, clock(duration)));
  lanes.append(seek, axis);
  renderPosition();
}

// Пока запись не открыта, таймлайн только показывает, кто когда говорил.
function renderPosition() {
  const seek = $("timeline-seek");
  if (!seek) return;
  seek.querySelector(".timeline-playhead").style.left = percent(position);
  seek.querySelector(".timeline-playhead").hidden = !opened;
  if (opened) {
    seek.setAttribute("role", "slider");
    seek.tabIndex = 0;
    seek.setAttribute("aria-label", t(K.timeline_seek));
    seek.setAttribute("aria-valuemin", "0");
    seek.setAttribute("aria-valuemax", String(Math.round(duration)));
    seek.setAttribute("aria-valuenow", String(Math.round(position)));
    seek.setAttribute("aria-valuetext", clock(position));
  } else {
    for (const name of ["role", "aria-label", "aria-valuemin", "aria-valuemax", "aria-valuenow", "aria-valuetext"]) {
      seek.removeAttribute(name);
    }
    seek.removeAttribute("tabindex");
  }
  seek.classList.toggle("active", opened);
}

function onTap(event) {
  if (!opened || duration <= 0) return;
  const box = event.currentTarget.getBoundingClientRect();
  if (box.width <= 0) return;
  const share = Math.min(Math.max((event.clientX - box.left) / box.width, 0), 1);
  playFrom(share * duration);
}

function onKey(event) {
  if (!opened) return;
  const moves = {
    ArrowLeft: position - KEY_STEP_SEC,
    ArrowDown: position - KEY_STEP_SEC,
    ArrowRight: position + KEY_STEP_SEC,
    ArrowUp: position + KEY_STEP_SEC,
    Home: 0,
    End: Math.max(duration - 0.5, 0),
  };
  if (!(event.key in moves)) return;
  event.preventDefault();
  playFrom(Math.min(Math.max(moves[event.key], 0), duration));
}

export function initTimeline() {
  onPlayerChange((state) => {
    opened = state.opened;
    position = state.position;
    renderPosition();
  });
}
