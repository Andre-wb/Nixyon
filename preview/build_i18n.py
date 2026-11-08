#!/usr/bin/env python3
"""ui/generated/i18n.js из ui/locales — то же, что делает src-tauri/build.rs.

При сборке приложения модуль ключей создаёт `nixyon-locales` из `build.rs`, а в
репозитории он лежит готовым, чтобы локальный preview и design-preview APK собирались
без Rust. Скрипт повторяет генератор из `nixyon-locales/src/js_module.rs`, поэтому
`python preview/build_i18n.py` и `cargo build` дают один и тот же файл.

    python preview/build_i18n.py            # перезаписать ui/generated/i18n.js
    python preview/build_i18n.py --check    # только проверить и сравнить, код возврата 1 при расхождении

Проверки повторяют `nixyon-locales/src/check.rs` и `used_keys` из `js_module.rs`:
ключи по алфавиту, без повторов и пустых значений, одинаковый набор ключей и
одинаковые подстановки `{name}` в ru и en, а все ключи из кода интерфейса существуют.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
UI = ROOT / "nixyon-app" / "ui"
LOCALES = UI / "locales"
GENERATED = UI / "generated" / "i18n.js"

HEADER = "// Создан генератором nixyon-locales из ui/locales при сборке. Не править руками.\n"
PLACEHOLDER = re.compile(r"\{(\w+)\}")


def read_entries(path: Path) -> list[tuple[str, str]]:
    """Строки словаря в порядке файла: ключ и значение."""
    text = path.read_text(encoding="utf-8")
    data = json.loads(text)
    if not isinstance(data, dict):
        raise SystemExit(f"{path}: ожидался JSON-объект")
    entries: list[tuple[str, str]] = []
    for key, value in data.items():
        if not isinstance(value, str):
            raise SystemExit(f"{path}: значение ключа {key} не строка")
        entries.append((key, value))
    return entries


def check(lang: str, entries: list[tuple[str, str]], errors: list[str]) -> None:
    seen: set[str] = set()
    previous: str | None = None
    for key, value in entries:
        if key in seen:
            errors.append(f"{lang}: ключ {key} повторяется")
        elif previous is not None and previous > key:
            errors.append(f"{lang}: ключ {key} стоит не по алфавиту")
        if not value.strip():
            errors.append(f"{lang}: пустое значение у ключа {key}")
        seen.add(key)
        previous = key


def check_pair(ru: list[tuple[str, str]], en: list[tuple[str, str]], errors: list[str]) -> None:
    ru_map, en_map = dict(ru), dict(en)
    for key, value in ru:
        if key not in en_map:
            errors.append(f"en: нет ключа {key}")
        elif PLACEHOLDER.findall(value) != PLACEHOLDER.findall(en_map[key]):
            errors.append(f"en: подстановки у ключа {key} не совпадают с ru.json")
    for key, _ in en:
        if key not in ru_map:
            errors.append(f"en: лишний ключ {key}, его нет в ru.json")


def ui_sources() -> list[Path]:
    """Код интерфейса: все .js и .html, кроме generated и locales."""
    files: list[Path] = []
    for path in sorted(UI.rglob("*")):
        if path.suffix in {".js", ".html"} and path.is_file():
            parts = path.relative_to(UI).parts
            if parts[0] in {"generated", "locales"}:
                continue
            files.append(path)
    return files


def used_keys(sources: list[str], keys: list[str]) -> list[str]:
    """`K.имя` в JS и `data-i18n*="ключ"` в HTML, как в nixyon-locales."""
    by_id = {js_identifier(key): key for key in keys}
    found: set[str] = set()
    for text in sources:
        for match in re.finditer(r"K\.([A-Za-z0-9_]+)", text):
            before = text[match.start() - 1] if match.start() > 0 else ""
            if before and (before.isalnum() or before in "_.$"):
                continue
            name = match.group(1)
            found.add(by_id.get(name, name))
        for attr in ('data-i18n="', 'data-i18n-label="', 'data-i18n-placeholder="'):
            for match in re.finditer(re.escape(attr) + r"([^\"]+)\"", text):
                found.add(match.group(1))
    return sorted(found)


def js_identifier(key: str) -> str:
    return "".join(c if c.isascii() and c.isalnum() else "_" for c in key)


def json_text(value: object) -> str:
    """Компактный JSON как у serde_json: без пробелов, не-ASCII как есть."""
    return json.dumps(value, ensure_ascii=False, separators=(",", ":"), sort_keys=True)


def module(ru: list[tuple[str, str]], en: list[tuple[str, str]]) -> str:
    ids = sorted((js_identifier(key), key) for key, _ in ru)
    ru_map = {key: value for key, value in ru}
    en_map = {key: value for key, value in en}
    out = [HEADER, "export const K = Object.freeze({\n"]
    for identifier, key in ids:
        out.append(f"  {identifier}: {json.dumps(key, ensure_ascii=False)},\n")
    out.append("});\n\nexport const DICTIONARIES = Object.freeze({\n")
    out.append(f"  ru: Object.freeze({json_text(ru_map)}),\n")
    out.append(f"  en: Object.freeze({json_text(en_map)}),\n")
    out.append("});\n")
    return "".join(out)


def main() -> int:
    ru = read_entries(LOCALES / "ru.json")
    en = read_entries(LOCALES / "en.json")
    errors: list[str] = []
    check("ru", ru, errors)
    check("en", en, errors)
    check_pair(ru, en, errors)

    dictionaries = [key for key, _ in ru]
    sources = [path.read_text(encoding="utf-8") for path in ui_sources()]
    missing = [key for key in used_keys(sources, dictionaries) if key not in set(dictionaries)]
    if missing:
        errors.append("в коде интерфейса есть ключи, которых нет в ui/locales/ru.json: " + ", ".join(missing))

    if errors:
        print("\n".join(errors))
        return 1

    text = module(ru, en)
    current = GENERATED.read_text(encoding="utf-8") if GENERATED.exists() else None
    check_only = "--check" in sys.argv[1:]
    if check_only:
        if current != text:
            print("ui/generated/i18n.js устарел: запустите python preview/build_i18n.py")
            return 1
        print(f"словари в порядке: {len(ru)} ключей, generated/i18n.js совпадает")
        return 0

    if current != text:
        GENERATED.parent.mkdir(parents=True, exist_ok=True)
        GENERATED.write_text(text, encoding="utf-8", newline="\n")
        print(f"ui/generated/i18n.js обновлён: {len(ru)} ключей")
    else:
        print(f"ui/generated/i18n.js уже актуален: {len(ru)} ключей")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
