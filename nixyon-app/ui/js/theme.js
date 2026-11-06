import { invoke } from "./api.js";

const STORAGE_KEY = "nixyon.theme";
const systemDark = window.matchMedia("(prefers-color-scheme: dark)");
let setting = "auto";

// Значки строки состояния Android под тему: при «Авто» по системе.
function syncSystemBars() {
  const dark = setting === "dark" || (setting === "auto" && systemDark.matches);
  invoke("set_system_bars", { dark }).catch(() => {});
}

export function themeSetting() {
  return setting;
}

// «Авто»: без атрибута, цвета по системе; «Светлая» и «Тёмная»: data-theme на <html>.
export function applyTheme(value) {
  setting = value === "light" || value === "dark" ? value : "auto";
  if (setting === "auto") delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = setting;
  try {
    localStorage.setItem(STORAGE_KEY, setting);
  } catch {
    // Без копии в localStorage тема применится после загрузки настроек.
  }
  syncSystemBars();
}

systemDark.addEventListener("change", () => {
  if (setting === "auto") syncSystemBars();
});
