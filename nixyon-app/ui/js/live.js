import { $, el } from "./dom.js";

// Живой текст: последние строки потокового распознавания без спикеров. Это черновик,
// точный текст и спикеры приходят с расшифровкой после остановки.
export function renderLive(id, lines) {
  const box = $(id);
  box.querySelector(".live-lines").replaceChildren(...lines.map((line) => el("p", null, line)));
  box.hidden = lines.length === 0;
}
