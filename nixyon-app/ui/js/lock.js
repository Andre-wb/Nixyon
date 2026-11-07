import { errorText, invoke } from "./api.js";
import { $ } from "./dom.js";

// Ключ данных открывается отпечатком, если он есть на телефоне. Система сама показывает
// запрос; при отмене экран остаётся, и повтор — по кнопке. Приложение не закрывается.
// Без отпечатка запроса нет, экран не появляется.
export async function unlockApp() {
  const lock = $("lock");
  for (;;) {
    try {
      await invoke("unlock_storage");
      lock.hidden = true;
      return;
    } catch (error) {
      lock.hidden = false;
      $("lock-error").textContent = errorText(error);
      await new Promise((resolve) => $("lock-retry").addEventListener("click", resolve, { once: true }));
      $("lock-error").textContent = "";
    }
  }
}
