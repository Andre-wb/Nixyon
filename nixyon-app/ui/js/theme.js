import { invoke } from "./api.js";

// Тема одна на всех устройствах: тёмная палитра с фоном. Выбор «Авто / Светлая /
// Тёмная» потребовал бы вторую полную палитру и вторую проверку контраста.
export function applyTheme() {
  document.documentElement.dataset.theme = "dark";
  invoke("set_system_bars", { dark: true }).catch(() => {});
}
