// Отрезки записи, которые играются: куски речи и реплики одного участника. Речь уже размечена
// при расшифровке (VAD и слова с временем), поэтому тишину здесь берём из этой разметки, а не
// прогоняем VAD заново. Отрезки в секундах, по возрастанию и не пересекаются.

// Пауза между словами длиннее этой считается тишиной и пропускается.
const SILENCE_SEC = 1;
// Запас до и после речи, чтобы слово не обрезалось.
const PAD_SEC = 0.2;

export function mergeSpans(spans) {
  const sorted = [...spans].sort((a, b) => a[0] - b[0]);
  const out = [];
  for (const [start, end] of sorted) {
    const last = out[out.length - 1];
    if (last && start <= last[1]) last[1] = Math.max(last[1], end);
    else out.push([start, end]);
  }
  return out;
}

export function intersectSpans(a, b) {
  const out = [];
  let i = 0;
  let j = 0;
  while (i < a.length && j < b.length) {
    const start = Math.max(a[i][0], b[j][0]);
    const end = Math.min(a[i][1], b[j][1]);
    if (start < end) out.push([start, end]);
    if (a[i][1] < b[j][1]) i += 1;
    else j += 1;
  }
  return out;
}

// Куски речи: по словам каждой реплики, разрывая там, где тишина длиннее SILENCE_SEC.
// Реплика без времени слов целиком считается речью.
export function speechSpans(turns) {
  const spans = [];
  for (const turn of turns) {
    const words = turn.words ?? [];
    if (!words.length) {
      spans.push([turn.start, turn.end]);
      continue;
    }
    let [start, end] = [words[0].start, words[0].end];
    for (const word of words.slice(1)) {
      if (word.start - end > SILENCE_SEC) {
        spans.push([start, end]);
        start = word.start;
      }
      end = Math.max(end, word.end);
    }
    spans.push([start, end]);
  }
  return mergeSpans(spans.map(([start, end]) => [Math.max(0, start - PAD_SEC), end + PAD_SEC]));
}

// Что играть. speaker: id участника или null, все. skipSilence: пропускать ли тишину.
// null: играется вся запись без ограничений.
export function playbackRanges(turns, { speaker, skipSilence }) {
  if (speaker === null && !skipSilence) return null;
  const windows =
    speaker === null
      ? [[0, Infinity]]
      : mergeSpans(turns.filter((turn) => turn.speaker === speaker).map((turn) => [turn.start, turn.end]));
  if (!skipSilence) return windows;
  return intersectSpans(windows, speechSpans(turns));
}
