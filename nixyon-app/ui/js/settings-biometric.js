import { errorCode, errorText, invoke } from "./api.js";
import { $ } from "./dom.js";
import { K, t } from "./i18n.js";

// Раздел «Защита» в настройках: переключатель входа по отпечатку, по умолчанию выключен.
// Палец система спрашивает и при включении, и при выключении. Если на телефоне нет
// отпечатка, включить нельзя: переключатель возвращается назад, рядом подсказка и кнопка
// в настройки Android. На ноутбуке отпечатка нет, раздел скрыт.

function showStatus(text, { error = false, offerSettings = false } = {}) {
  const status = $("settings-biometric-status");
  status.classList.toggle("error-text", error);
  status.textContent = text;
  $("settings-biometric-open").hidden = !offerSettings;
}

function failureText(error) {
  const code = errorCode(error);
  if (code === "biometric_missing") return t(K.settings_biometric_missing);
  if (code === "locked") return t(K.settings_biometric_cancelled);
  return errorText(error);
}

async function toggle(box) {
  const enabled = box.checked;
  showStatus("");
  box.disabled = true;
  try {
    await invoke("set_biometric", { enabled });
  } catch (error) {
    box.checked = !enabled;
    showStatus(failureText(error), {
      error: true,
      offerSettings: errorCode(error) === "biometric_missing",
    });
  } finally {
    box.disabled = false;
  }
}

export async function renderSettingsBiometric() {
  showStatus("");
  let status = null;
  try {
    status = await invoke("biometric_status");
  } catch (error) {
    $("settings-biometric-block").hidden = false;
    $("settings-biometric").disabled = true;
    showStatus(errorText(error), { error: true });
    return;
  }
  $("settings-biometric-block").hidden = status === null;
  if (status === null) return;
  $("settings-biometric").disabled = false;
  $("settings-biometric").checked = status.enabled;
}

export function initSettingsBiometric() {
  const box = $("settings-biometric");
  box.addEventListener("change", () => toggle(box));
  $("settings-biometric-open").addEventListener("click", () => invoke("open_biometric_settings").catch(() => null));
}
