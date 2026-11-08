import { invoke } from "./api.js";

// Nixyon uses one visual environment on every device. The ambient dark palette is the
// product, not a theme: the Auto/Light/Dark preference of the previous version would
// mean a second full palette and a second contrast pass, so it is intentionally gone.
export function applyTheme() {
  document.documentElement.dataset.theme = "dark";
  invoke("set_system_bars", { dark: true }).catch(() => {});
}
