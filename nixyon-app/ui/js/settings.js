import { errorText, invoke } from "./api.js";
import { $ } from "./dom.js";
import { K, resolveLanguage, setLanguage, t } from "./i18n.js";
import { languageChanged, show } from "./screens.js";
import { initSettingsBiometric, renderSettingsBiometric } from "./settings-biometric.js";
import { initSettingsLlmGpu, renderSettingsLlmGpu } from "./settings-llm-gpu.js";
import { initSettingsModels, renderSettingsModels } from "./settings-models.js";
import { initSettingsSpeech, renderSettingsSpeech } from "./settings-speech.js";
import { initSettingsStorage, renderSettingsStorage } from "./settings-storage.js";

// Настройки: отдельный экран с языком интерфейса и речи, моделями, защитой, местом на
// телефоне, очисткой звука, видеокартой и словарём подсказок. Тема одна, тёмная.
let setting = "auto";
let hotwords = [];

export function applySetting(value) {
  setting = value === "ru" || value === "en" ? value : "auto";
  setLanguage(resolveLanguage(setting));
  for (const radio of document.querySelectorAll('input[name="language"]')) {
    radio.checked = radio.value === setting;
  }
  languageChanged();
}

export function applyHotwords(list) {
  hotwords = Array.isArray(list) ? list : [];
  const box = $("hotwords");
  if (box) box.value = hotwords.join("\n");
}

async function saveHotwords() {
  const status = $("hotwords-status");
  status.classList.remove("error-text");
  status.textContent = "";
  try {
    const saved = await invoke("set_hotwords", { text: $("hotwords").value });
    applyHotwords(saved.hotwords ?? []);
    status.textContent = t(K.settings_hotwords_saved);
  } catch (error) {
    status.classList.add("error-text");
    status.textContent = errorText(error);
  }
}

function openSettings() {
  for (const radio of document.querySelectorAll('input[name="language"]')) {
    radio.checked = radio.value === setting;
  }
  renderSettingsSpeech();
  $("hotwords").value = hotwords.join("\n");
  $("hotwords-status").textContent = "";
  $("settings-models-status").textContent = "";
  renderSettingsModels();
  renderSettingsBiometric();
  renderSettingsStorage();
  renderSettingsLlmGpu();
  // Экран попадает в историю, чтобы системная «Назад» вела на главный экран.
  if (location.hash !== "#settings") history.pushState(null, "", "#settings");
  show("settings");
}

export function initSettings() {
  $("open-settings").addEventListener("click", openSettings);
  $("settings-back").addEventListener("click", () => history.back());
  initSettingsModels();
  initSettingsBiometric();
  initSettingsStorage();
  initSettingsLlmGpu();
  initSettingsSpeech();
  $("hotwords-save").addEventListener("click", saveHotwords);
  for (const radio of document.querySelectorAll('input[name="language"]')) {
    radio.addEventListener("change", async () => {
      applySetting(radio.value);
      try {
        await invoke("set_language", { language: radio.value });
      } catch {
        // Язык уже применён; не сохранился только выбор в настройках ядра.
      }
    });
  }
}
