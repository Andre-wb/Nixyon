import { errorText, invoke } from "./api.js";
import { $, el } from "./dom.js";
import { K, t } from "./i18n.js";
import { openModal } from "./screens.js";
import { currentMeeting } from "./transcript.js";

// Подпись каждой эмоции по её ключу из расшифровки.
const LABELS = {
  neutral: K.emotion_neutral,
  joy: K.emotion_joy,
  sadness: K.emotion_sadness,
  anger: K.emotion_anger,
  fear: K.emotion_fear,
  surprise: K.emotion_surprise,
  disgust: K.emotion_disgust,
};

let turnId = null;
let saved = null;

// Подпись эмоции реплики; без эмоции «Эмоция?».
export function emotionLabel(emotion) {
  const key = LABELS[emotion];
  return t(key ?? K.emotion_unknown);
}

// Метка эмоции в шапке реплики: нажатие открывает список выбора. Цветная точка только
// дублирует подпись, смысл несёт текст.
export function emotionChip(turn, onOpen) {
  const known = Boolean(LABELS[turn.emotion]);
  const label = emotionLabel(turn.emotion);
  const chip = el("button", "turn-emotion");
  chip.type = "button";
  if (known) chip.dataset.emotion = turn.emotion;
  chip.setAttribute("aria-label", t(K.emotion_change, { emotion: label }));
  if (known && turn.emotion !== "neutral") {
    const dot = el("span", "emotion-dot");
    dot.dataset.emotion = turn.emotion;
    dot.setAttribute("aria-hidden", "true");
    chip.append(dot);
  }
  chip.append(el("span", "", label));
  chip.addEventListener("click", onOpen);
  return chip;
}

// Список эмоций для реплики `turn`; `meta`: кто и когда говорит, `onSaved` получает
// расшифровку после правки.
export function openEmotion(turn, meta, onSaved) {
  turnId = turn.id;
  saved = onSaved;
  $("emotion-sheet-meta").textContent = meta;
  $("emotion-error").textContent = "";
  for (const radio of document.querySelectorAll('#emotion-sheet input[name="emotion"]')) {
    radio.checked = radio.value === turn.emotion;
  }
  openModal($("emotion-sheet"));
}

async function choose(emotion) {
  if (turnId === null) return;
  try {
    const transcript = await invoke("set_turn_emotion", { meeting: currentMeeting(), id: turnId, emotion });
    $("emotion-sheet").close();
    saved?.(transcript);
  } catch (error) {
    $("emotion-error").textContent = errorText(error);
  }
}

export function initEmotion() {
  for (const radio of document.querySelectorAll('#emotion-sheet input[name="emotion"]')) {
    radio.addEventListener("change", () => {
      if (radio.checked) choose(radio.value);
    });
  }
}
