import { errorText, invoke } from "./api.js";
import { $ } from "./dom.js";
import { K, resolveLanguage, setLanguage, t } from "./i18n.js";
import { languageChanged, show } from "./screens.js";
import { initSettingsBiometric, renderSettingsBiometric } from "./settings-biometric.js";
import { initSettingsModels, renderSettingsModels } from "./settings-models.js";
import { initSettingsStorage, renderSettingsStorage } from "./settings-storage.js";
import { applyTheme, themeSetting } from "./theme.js";

let setting = "auto";
let hotwords = [];

// Список слов приходит вместе с настройками при старте.
export function applyHotwords(list) {
  hotwords = list ?? [];
}

async function saveHotwords() {
  const status = $("hotwords-status");
  status.classList.remove("error-text");
  try {
    const saved = await invoke("set_hotwords", { text: $("hotwords").value });
    applyHotwords(saved.hotwords);
    $("hotwords").value = hotwords.join("\n");
    status.textContent = t(K.settings_hotwords_saved);
  } catch (error) {
    status.classList.add("error-text");
    status.textContent = errorText(error);
  }
}

export function applySetting(value) {
  setting = value;
  setLanguage(resolveLanguage(value));
  languageChanged();
}

export function initSettings() {
  $("open-settings").addEventListener("click", () => {
    for (const radio of document.querySelectorAll('input[name="language"]')) {
      radio.checked = radio.value === setting;
    }
    for (const radio of document.querySelectorAll('input[name="theme"]')) {
      radio.checked = radio.value === themeSetting();
    }
    $("hotwords").value = hotwords.join("\n");
    $("hotwords-status").textContent = "";
    $("settings-models-status").textContent = "";
    renderSettingsModels();
    renderSettingsBiometric();
    renderSettingsStorage();
    // Экран попадает в историю, чтобы системная «Назад» вела на главный экран.
    if (location.hash !== "#settings") history.pushState(null, "", "#settings");
    show("settings");
  });
  $("settings-back").addEventListener("click", () => history.back());
  initSettingsModels();
  initSettingsBiometric();
  initSettingsStorage();
  $("hotwords-save").addEventListener("click", saveHotwords);
  for (const radio of document.querySelectorAll('input[name="theme"]')) {
    radio.addEventListener("change", async () => {
      applyTheme(radio.value);
      try {
        await invoke("set_theme", { theme: radio.value });
      } catch {
        // Тема уже применена; не сохранился только выбор в настройках ядра.
      }
    });
  }
  for (const radio of document.querySelectorAll('input[name="language"]')) {
    radio.addEventListener("change", async () => {
      applySetting(radio.value);
      try {
        await invoke("set_language", { language: radio.value });
      } catch {
        // Язык уже применён к экрану; не сохранился только выбор на следующий запуск.
      }
    });
  }
}
