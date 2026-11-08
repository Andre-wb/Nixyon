import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

// Длинное тире, средняя точка и стрелки не допускаются ни в текстах интерфейса, ни в
// комментариях кода интерфейса.
const FORBIDDEN = /[—·←-⇿⟰-⟿⤀-⥿⬀-⬯]/u;
const TEXT_FILES = /\.(css|html|js|json|mjs)$/;
const ROOT = new URL("../ui/", import.meta.url).pathname;

function files(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return files(path);
    return TEXT_FILES.test(entry.name) ? [path] : [];
  });
}

test("в файлах интерфейса нет длинного тире, средней точки и стрелок", () => {
  const found = [];
  for (const path of files(ROOT)) {
    readFileSync(path, "utf8")
      .split("\n")
      .forEach((line, index) => {
        const match = line.match(FORBIDDEN);
        if (match) found.push(`${path.slice(ROOT.length)}:${index + 1}: ${match[0]}`);
      });
  }
  assert.deepEqual(found, []);
});
