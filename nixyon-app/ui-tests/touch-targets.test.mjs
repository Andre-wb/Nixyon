import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

// В styles.css min-height в пикселях задают только у того, на что нажимают: кнопки, строки
// списков, поля. Цель касания не меньше 44 px.
const MIN_TARGET_PX = 44;

test("цели касания в стилях не ниже 44 px", () => {
  const css = readFileSync(new URL("../ui/styles.css", import.meta.url), "utf8");
  const small = [];
  for (const rule of css.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    for (const [, px] of rule[2].matchAll(/min-height:\s*(\d+(?:\.\d+)?)px\s*;/g)) {
      if (Number(px) < MIN_TARGET_PX) small.push(`${rule[1].trim().split("\n").pop()}: ${px}px`);
    }
  }
  assert.deepEqual(small, []);
});
