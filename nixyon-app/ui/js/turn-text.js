import { el } from "./dom.js";

// Места слов с временем в тексте хода (индексы строки JS). Слово ищется по порядку от
// конца предыдущего; не найденное слово остаётся без места, тап по нему не работает.
export function wordRanges(text, words) {
  const ranges = [];
  let cursor = 0;
  (words ?? []).forEach((word, i) => {
    const at = word.text ? text.indexOf(word.text, cursor) : -1;
    if (at < 0) return;
    ranges.push({ i, start: at, end: at + word.text.length });
    cursor = at + word.text.length;
  });
  return ranges;
}

// Текст хода из кусков: слова с временем в <span class="word">, найденное поиском в
// <mark>. Совпадение поиска может пересекать границы слов, тогда оно режется по ним.
export function renderTurnText(node, text, ranges, hits) {
  const cuts = new Set([0, text.length]);
  for (const r of [...ranges, ...hits]) {
    cuts.add(Math.max(0, Math.min(text.length, r.start)));
    cuts.add(Math.max(0, Math.min(text.length, r.end)));
  }
  const points = [...cuts].sort((a, b) => a - b);
  const out = [];
  let container = null;
  let containerWord = -1;
  for (let k = 0; k + 1 < points.length; k += 1) {
    const [a, b] = [points[k], points[k + 1]];
    const piece = text.slice(a, b);
    const hit = hits.find((h) => h.start <= a && b <= h.end);
    let part = piece;
    if (hit) {
      part = el("mark", hit.current ? "hit current" : "hit", piece);
      part.dataset.hit = String(hit.i);
    }
    const word = ranges.find((r) => r.start <= a && b <= r.end);
    if (!word) {
      container = null;
      out.push(part);
      continue;
    }
    if (!container || containerWord !== word.i) {
      container = el("span", "word");
      container.dataset.word = String(word.i);
      containerWord = word.i;
      out.push(container);
    }
    container.append(part);
  }
  node.replaceChildren(...out);
}
