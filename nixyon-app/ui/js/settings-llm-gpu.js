import { errorCode, errorText, invoke } from "./api.js";
import { $ } from "./dom.js";
import { K, t } from "./i18n.js";
import { onLanguageChange } from "./screens.js";

// Раздел «Ускорение» в настройках: видеокарта для протокола и подсказки имён, по умолчанию
// выключена. Есть только в APK, собранном с видеокартой; в другой сборке переключатель
// неактивен. Действует со следующего запуска модели. Под переключателем видно, на чём
// модель считала в последний раз. На ноутбуке раздел скрыт.

let lastStatus = null;

function showStatus(text, error = false) {
  const status = $("settings-llm-gpu-status");
  status.classList.toggle("error-text", error);
  status.textContent = text;
}

function deviceText(status) {
  if (!status.built) return t(K.settings_llm_gpu_not_built);
  const device = status.device;
  if (!device) return "";
  if (device.startsWith("Vulkan")) {
    return t(K.settings_llm_gpu_last_gpu, { device: device.replace(/^Vulkan:\s*/, "") });
  }
  return t(K.settings_llm_gpu_last_cpu);
}

function apply(status) {
  lastStatus = status;
  $("settings-llm-gpu-block").hidden = status === null;
  if (status === null) return;
  const box = $("settings-llm-gpu");
  box.disabled = !status.built;
  box.checked = status.enabled;
  showStatus(deviceText(status));
}

async function toggle(box) {
  const enabled = box.checked;
  box.disabled = true;
  try {
    await invoke("set_llm_gpu", { enabled });
    apply({ ...lastStatus, enabled });
  } catch (error) {
    if (errorCode(error) === "llm_gpu_not_built") {
      apply({ ...lastStatus, built: false, enabled: false });
      return;
    }
    box.checked = !enabled;
    box.disabled = false;
    showStatus(errorText(error), true);
  }
}

export async function renderSettingsLlmGpu() {
  showStatus("");
  try {
    apply(await invoke("llm_gpu_status"));
  } catch (error) {
    $("settings-llm-gpu-block").hidden = false;
    $("settings-llm-gpu").disabled = true;
    showStatus(errorText(error), true);
  }
}

export function initSettingsLlmGpu() {
  const box = $("settings-llm-gpu");
  box.addEventListener("change", () => toggle(box));
  onLanguageChange(() => {
    if (lastStatus !== null) showStatus(deviceText(lastStatus));
  });
}
