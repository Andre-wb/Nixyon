import { invoke } from "./api.js";
import { $, el } from "./dom.js";

// Выбранные метки фильтра списка встреч. Только в памяти: после перезапуска фильтр
// сброшен.
const selected = new Set();
let labels = [];
let onChange = () => {};

function render() {
  const box = $("label-filter");
  box.replaceChildren();
  for (const label of labels) {
    const chip = el("button", "chip", label.name);
    chip.type = "button";
    chip.setAttribute("aria-pressed", String(selected.has(label.id)));
    chip.addEventListener("click", () => {
      if (selected.has(label.id)) selected.delete(label.id);
      else selected.add(label.id);
      render();
      onChange();
    });
    box.append(chip);
  }
  box.hidden = labels.length === 0;
}

// Список меток читается заново при каждом показе главного экрана. Метки, которых больше
// нет, из фильтра уходят. Ошибку чтения показывает список встреч, фильтр просто пуст.
export async function loadLabelFilter() {
  try {
    labels = await invoke("list_labels");
  } catch {
    labels = [];
  }
  const known = new Set(labels.map((l) => l.id));
  for (const id of [...selected]) if (!known.has(id)) selected.delete(id);
  render();
}

export function selectedLabels() {
  return [...selected];
}

// Встреча проходит фильтр, если у неё есть каждая выбранная метка.
export function matchesFilter(meeting) {
  const own = meeting.labels ?? [];
  return [...selected].every((id) => own.includes(id));
}

export function initLabelFilter(changed) {
  onChange = changed;
}
