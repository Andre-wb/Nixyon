import { $ } from "./dom.js";

const SCREENS = ["home", "processing", "transcript", "voices"];
const languageListeners = [];
const screenListeners = [];

export function onScreenChange(listener) {
  screenListeners.push(listener);
}

export function show(name) {
  for (const s of SCREENS) $(`screen-${s}`).hidden = s !== name;
  for (const listener of screenListeners) listener(name);
  window.scrollTo({ top: 0 });
  const heading = document.querySelector(`#screen-${name} h1`);
  heading?.setAttribute("tabindex", "-1");
  heading?.focus({ preventScroll: true });
}

// Расшифровка попадает в историю, чтобы системная кнопка «Назад» вела на главный экран.
export function pushTranscript() {
  if (location.hash !== "#transcript") history.pushState(null, "", "#transcript");
  show("transcript");
}

export function onLanguageChange(listener) {
  languageListeners.push(listener);
}

export function languageChanged() {
  for (const listener of languageListeners) listener();
}

// Открытые листы и окна подтверждения: системная «Назад» закрывает верхнее из них, а не
// уводит страницу под ним. Каждое открытое окно добавляет запись в историю; если окно
// закрыто кнопкой, его запись снимается, и этот шаг назад обрабатывать не нужно.
const modals = [];
let ownBacks = 0;

export function openModal(dialog) {
  dialog.showModal();
  history.pushState({ modal: modals.length + 1 }, "", location.hash);
  modals.push(dialog);
  dialog.addEventListener(
    "close",
    () => {
      const index = modals.indexOf(dialog);
      if (index === -1) return;
      modals.splice(index, 1);
      ownBacks += 1;
      history.back();
    },
    { once: true },
  );
}

// true, если шаг назад относился к окну и уже обработан.
export function handleModalBack() {
  if (ownBacks > 0) {
    ownBacks -= 1;
    return true;
  }
  const top = modals.pop();
  if (!top) return false;
  top.close();
  return true;
}
