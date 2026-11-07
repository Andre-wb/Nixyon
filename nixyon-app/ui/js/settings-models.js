import { errorText, invoke } from "./api.js";
import { $, el } from "./dom.js";
import { K, t } from "./i18n.js";
import { currentStatus, modelText, onModelsChange, openModels, refreshModels } from "./models.js";
import { onLanguageChange } from "./screens.js";

// Раздел «Модели» в настройках: у скачанных необязательных моделей переключатель
// «включена или выключена». Выключенная модель не грузится, файл остаётся на телефоне.
// Набор распознавания выключить нельзя.

function disabledKeys() {
  return [...document.querySelectorAll("#settings-models input.switch")]
    .filter((box) => !box.checked)
    .map((box) => box.dataset.pack);
}

async function save(box) {
  const status = $("settings-models-status");
  status.classList.remove("error-text");
  status.textContent = "";
  box.disabled = true;
  try {
    await invoke("set_models_disabled", { packs: disabledKeys() });
    await refreshModels();
  } catch (error) {
    box.checked = !box.checked;
    status.classList.add("error-text");
    status.textContent = errorText(error);
  } finally {
    box.disabled = false;
  }
}

function row(pack, disabled) {
  const item = el("li");
  const switchable = !pack.required && pack.installed;
  const label = el(switchable ? "label" : "div", "model-row");
  label.append(modelText(pack));
  if (switchable) {
    const box = el("input", "switch");
    box.type = "checkbox";
    box.setAttribute("role", "switch");
    box.dataset.pack = pack.key;
    box.checked = !disabled.includes(pack.key);
    box.addEventListener("change", () => save(box));
    label.append(box);
  } else {
    const state = pack.required ? K.models_required : pack.installed ? K.models_installed : K.models_not_installed;
    label.append(el("span", "model-state", t(state)));
  }
  item.append(label);
  return item;
}

export function renderSettingsModels() {
  const status = currentStatus();
  $("settings-models-block").hidden = status === null;
  if (status === null) return;
  const disabled = status.disabled ?? [];
  $("settings-models").replaceChildren(...status.packs.map((pack) => row(pack, disabled)));
  $("settings-models-download").hidden = status.packs.every((pack) => pack.installed);
}

export function initSettingsModels() {
  $("settings-models-download").addEventListener("click", openModels);
  onModelsChange(renderSettingsModels);
  onLanguageChange(renderSettingsModels);
}
