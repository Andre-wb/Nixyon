import { $, dot, el } from "./dom.js";
import { K, t } from "./i18n.js";
import { openModal } from "./screens.js";

// Выбор участника для «слушать только одного». Кнопка в плеере показывает текущий выбор,
// по касанию снизу открывается лист со списком: «Все участники» и участники встречи.
// Выбор применяется сразу и закрывает лист.

let chosen = null;
let listeners = [];

function choose(value) {
  const sheet = $("player-speaker-sheet");
  if (sheet.open) sheet.close();
  for (const listener of listeners) listener(value);
}

function option(value, name, id) {
  const label = el("label", "choice");
  const input = el("input");
  input.type = "radio";
  input.name = "player-speaker";
  input.value = value;
  input.checked = value === (chosen === null ? "" : String(chosen));
  input.addEventListener("change", () => choose(value));
  label.append(input);
  if (id !== undefined) label.append(dot(id));
  label.append(el("span", "player-speaker-option", name));
  return label;
}

// speakers: участники по порядку ({ id, name }); speaker: выбранный id или null.
export function renderPlayerSpeaker(speakers, speaker) {
  chosen = speaker;
  const current = speakers.find((s) => s.id === speaker);
  $("player-speaker-name").textContent = current ? current.name : t(K.player_speaker_all);
  $("player-speaker").classList.toggle("is-chosen", Boolean(current));
  $("player-speaker-list").replaceChildren(
    option("", t(K.player_speaker_all)),
    ...speakers.map((s) => option(String(s.id), s.name, s.id)),
  );
}

// listener получает "" для всех участников или id выбранного строкой.
export function initPlayerSpeaker(listener) {
  listeners = [listener];
  $("player-speaker").addEventListener("click", () => openModal($("player-speaker-sheet")));
}
