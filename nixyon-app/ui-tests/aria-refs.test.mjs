import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

// Экранный чтец берёт подпись по id: ссылка на несуществующий элемент оставляет экран
// или окно без названия.
test("aria-labelledby и aria-describedby ссылаются на существующие id", () => {
  const html = readFileSync(new URL("../ui/index.html", import.meta.url), "utf8");
  const ids = new Set([...html.matchAll(/\sid="([^"]+)"/g)].map((m) => m[1]));
  const missing = [];
  for (const [, attr, value] of html.matchAll(/\s(aria-labelledby|aria-describedby)="([^"]+)"/g)) {
    for (const ref of value.split(/\s+/)) if (!ids.has(ref)) missing.push(`${attr}: ${ref}`);
  }
  assert.deepEqual(missing, []);
});

test("id в разметке не повторяются", () => {
  const html = readFileSync(new URL("../ui/index.html", import.meta.url), "utf8");
  const seen = new Set();
  const repeated = [];
  for (const [, id] of html.matchAll(/\sid="([^"]+)"/g)) {
    if (seen.has(id)) repeated.push(id);
    seen.add(id);
  }
  assert.deepEqual(repeated, []);
});
