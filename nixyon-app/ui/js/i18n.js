import { DICTIONARIES, K } from "../generated/i18n.js";

export { K };

let lang = "ru";

// «Авто» берёт язык устройства; неизвестный язык даёт русский.
export function resolveLanguage(setting) {
  if (setting === "ru" || setting === "en") return setting;
  const device = (navigator.language || "").toLowerCase();
  return device.startsWith("en") ? "en" : "ru";
}

export function setLanguage(code) {
  lang = code;
  document.documentElement.lang = code;
  for (const node of document.querySelectorAll("[data-i18n]")) {
    node.textContent = t(node.dataset.i18n);
  }
  for (const node of document.querySelectorAll("[data-i18n-label]")) {
    const text = t(node.dataset.i18nLabel);
    node.setAttribute("aria-label", text);
    node.title = text;
  }
  for (const node of document.querySelectorAll("[data-i18n-placeholder]")) {
    node.placeholder = t(node.dataset.i18nPlaceholder);
  }
}

export function language() {
  return lang;
}

export function locale() {
  return lang === "en" ? "en-US" : "ru-RU";
}

export function t(key, params = {}) {
  const text = DICTIONARIES[lang][key] ?? DICTIONARIES.ru[key] ?? key;
  return text.replace(/\{(\w+)\}/g, (all, name) => (name in params ? String(params[name]) : all));
}
