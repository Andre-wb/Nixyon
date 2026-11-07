import { errorCode, errorText, invoke } from "./api.js";
import { $ } from "./dom.js";

// Ключ данных открывается отпечатком, если вход по отпечатку включён в настройках. Система
// сама показывает запрос; при отмене экран остаётся, повтор по кнопке. Приложение не
// закрывается. Если вход выключен, запроса нет и экран не появляется. Прочие сбои хранилища экран не держат:
// приложение открывается, и ошибку покажет команда, которой нужны файлы.
const LOCK_CODES = new Set(["locked", "biometric_unavailable", "storage_key_lost"]);

function click(id) {
  return new Promise((resolve) => $(id).addEventListener("click", () => resolve(id), { once: true }));
}

// Ключ стёрт системой: начать заново можно только после явного подтверждения, старые
// записи при этом удаляются.
async function confirmReset() {
  const dialog = $("lock-reset-confirm");
  dialog.showModal();
  const answer = await Promise.race([click("lock-reset-yes"), click("lock-reset-no")]);
  dialog.close();
  if (answer !== "lock-reset-yes") return;
  try {
    await invoke("reset_storage");
  } catch (error) {
    $("lock-error").textContent = errorText(error);
  }
}

export async function unlockApp() {
  const lock = $("lock");
  for (;;) {
    try {
      await invoke("unlock_storage");
      lock.hidden = true;
      return;
    } catch (error) {
      const code = errorCode(error);
      if (!LOCK_CODES.has(code)) {
        lock.hidden = true;
        return;
      }
      const lost = code === "storage_key_lost";
      lock.hidden = false;
      $("lock-error").textContent = errorText(error);
      $("lock-retry").hidden = lost;
      $("lock-reset").hidden = !lost;
      if (lost) {
        await click("lock-reset");
        $("lock-error").textContent = "";
        await confirmReset();
      } else {
        await click("lock-retry");
        $("lock-error").textContent = "";
      }
    }
  }
}
