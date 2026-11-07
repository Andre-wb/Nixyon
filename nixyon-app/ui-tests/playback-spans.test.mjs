import assert from "node:assert/strict";
import { test } from "node:test";

import { intersectSpans, mergeSpans, playbackRanges, speechSpans } from "../ui/js/playback-spans.js";

const turn = (speaker, start, end, words = []) => ({ speaker, start, end, words });
const word = (start, end) => ({ start, end });

test("перекрытые и соседние отрезки сливаются, порядок по началу", () => {
  assert.deepEqual(
    mergeSpans([
      [5, 6],
      [0, 2],
      [1, 3],
      [3, 4],
    ]),
    [
      [0, 4],
      [5, 6],
    ],
  );
  assert.deepEqual(mergeSpans([]), []);
});

test("пересечение оставляет только общие части", () => {
  assert.deepEqual(
    intersectSpans(
      [
        [0, 5],
        [8, 12],
      ],
      [
        [1, 2],
        [4, 9],
        [11, 20],
      ],
    ),
    [
      [1, 2],
      [4, 5],
      [8, 9],
      [11, 12],
    ],
  );
  assert.deepEqual(intersectSpans([[0, 1]], [[1, 2]]), []);
});

test("пауза между словами длиннее секунды разрывает речь, края с запасом", () => {
  const spans = speechSpans([turn(1, 0, 10, [word(1, 2), word(2.5, 3), word(6, 7)])]);
  assert.equal(spans.length, 2);
  assert.ok(Math.abs(spans[0][0] - 0.8) < 1e-9 && Math.abs(spans[0][1] - 3.2) < 1e-9);
  assert.ok(Math.abs(spans[1][0] - 5.8) < 1e-9 && Math.abs(spans[1][1] - 7.2) < 1e-9);
});

test("реплика без времени слов целиком считается речью, начало не уходит ниже нуля", () => {
  assert.deepEqual(speechSpans([turn(1, 0, 4)]), [[0, 4.2]]);
});

test("без участника и без пропуска тишины играется вся запись", () => {
  assert.equal(playbackRanges([turn(1, 0, 4)], { speaker: null, skipSilence: false }), null);
});

test("выбранный участник: только его реплики", () => {
  const turns = [turn(1, 0, 4), turn(2, 4, 6), turn(1, 6, 9)];
  assert.deepEqual(playbackRanges(turns, { speaker: 1, skipSilence: false }), [
    [0, 4],
    [6, 9],
  ]);
  assert.deepEqual(playbackRanges(turns, { speaker: 3, skipSilence: false }), []);
});

test("пропуск тишины без участника даёт конечные отрезки речи", () => {
  const ranges = playbackRanges([turn(1, 0, 10, [word(1, 2), word(6, 7)])], { speaker: null, skipSilence: true });
  assert.equal(ranges.length, 2);
  assert.ok(ranges.every(([start, end]) => Number.isFinite(start) && Number.isFinite(end) && start < end));
});

test("участник и пропуск тишины вместе: речь только внутри его реплик", () => {
  const turns = [turn(1, 0, 3, [word(0.5, 2.5)]), turn(2, 3, 8, [word(3.5, 7.5)])];
  const ranges = playbackRanges(turns, { speaker: 1, skipSilence: true });
  assert.equal(ranges.length, 1);
  assert.ok(Math.abs(ranges[0][0] - 0.3) < 1e-9 && Math.abs(ranges[0][1] - 2.7) < 1e-9);
});
