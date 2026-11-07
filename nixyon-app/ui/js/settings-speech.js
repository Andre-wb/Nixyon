import { invoke } from "./api.js";

// Язык речи записей: "auto" (определяется по каждой записи), "ru" или "en".
let choice = "auto";

// Выбор приходит вместе с настройками при старте.
export function applySpeechLanguage(value) {
  choice = value ?? "auto";
}

// Живой текст слушает русская модель: при выбранном английском он показал бы кашу.
export function liveTextShown() {
  return choice !== "en";
}

export function renderSettingsSpeech() {
  for (const radio of document.querySelectorAll('input[name="speech-language"]')) {
    radio.checked = radio.value === choice;
  }
}

export function initSettingsSpeech() {
  for (const radio of document.querySelectorAll('input[name="speech-language"]')) {
    radio.addEventListener("change", async () => {
      const previous = choice;
      choice = radio.value;
      try {
        await invoke("set_speech_language", { choice: radio.value });
      } catch {
        // Выбор не сохранился: следующая запись распознавалась бы не так, как видно в
        // настройках, поэтому показываем прежний.
        choice = previous;
        renderSettingsSpeech();
      }
    });
  }
}
