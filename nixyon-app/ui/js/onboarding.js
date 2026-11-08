import { $ } from "./dom.js";
import { show } from "./screens.js";

// Мини-обучение вместо входа. Профиля и PIN в приложении нет: всё считается на
// телефоне, поэтому первый запуск знакомит с тремя шагами работы, а не просит вход.
// Флаг «показано» хранится локально: инструкция открывается один раз и снова — из
// настроек.
const SEEN_KEY = "nixyon.onboarding.v1";

function read() {
  try {
    return localStorage.getItem(SEEN_KEY) === "1";
  } catch {
    // Хранилище недоступно: не показываем инструкцию повторно при каждом запуске.
    return true;
  }
}

function markSeen() {
  try {
    localStorage.setItem(SEEN_KEY, "1");
  } catch {
    // Без записи инструкция просто покажется ещё раз в следующий запуск.
  }
}

export function onboardingSeen() {
  return read();
}

// «Продолжить»: начало работы с главного экрана. Запись в истории не нужна, поэтому
// экран заменяется, а не открывается поверх.
function finish() {
  markSeen();
  history.replaceState(null, "", "#home");
  show("home");
}

export function initOnboarding() {
  $("onboard-continue").addEventListener("click", finish);
  // Из настроек инструкцию можно открыть снова: она попадает в историю, и «Назад»
  // возвращает в настройки.
  $("open-onboarding").addEventListener("click", () => {
    if (location.hash !== "#onboarding") history.pushState(null, "", "#onboarding");
    show("onboarding");
  });
}
