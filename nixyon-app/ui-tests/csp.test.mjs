import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

// Стили задаются только файлами приложения. Переменные и свойства, которые код ставит
// через element.style, CSP не ограничивает, поэтому 'unsafe-inline' не нужен.
test("CSP не разрешает встроенные стили и скрипты", () => {
  const conf = JSON.parse(readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8"));
  const csp = conf.app.security.csp;
  assert.ok(!csp.includes("'unsafe-inline'"), csp);
  assert.ok(!csp.includes("'unsafe-eval'"), csp);
});
