#!/usr/bin/env python3
"""Каталог моделей для APK: какие файлы телефон качает с зеркала, их размеры и SHA-256.

Каталог собирается из models/models.json (пишет fetch-models.sh) и models/llm/llm.json
(пишет fetch-llm.sh) и лежит в репозитории, в assets Kotlin-плагина. Самих моделей в APK
нет: по каталогу телефон проверяет скачанное, поэтому каталог нельзя брать с зеркала.

    scripts/build-model-catalog.py          записать каталог
    scripts/build-model-catalog.py --check  только сверить, код 1 при расхождении
"""

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MODELS = ROOT / "models"
CATALOG = ROOT / "nixyon-app/plugins/android/android/src/main/assets/catalog"

# Фильтр музыки на телефоне не используется (ModelRole.kt), в каталог он не попадает.
SKIP_ROLES = {"tagging_model", "tagging_labels"}


def speech_catalog(manifest):
    files = {role: entry for role, entry in manifest["files"].items() if role not in SKIP_ROLES}
    return {"version": manifest["version"], "files": files}


def llm_catalog(manifest):
    return {"version": manifest["version"], "model": manifest["model"]}


def render(data):
    return json.dumps(data, ensure_ascii=False, indent=2) + "\n"


def build(models_dir):
    """Тексты обоих файлов каталога: имя файла и содержимое."""
    speech = json.loads((models_dir / "models.json").read_text(encoding="utf-8"))
    llm = json.loads((models_dir / "llm" / "llm.json").read_text(encoding="utf-8"))
    return {
        "models.json": render(speech_catalog(speech)),
        "llm.json": render(llm_catalog(llm)),
    }


def differences(expected, catalog_dir):
    """Имена файлов каталога, которые не совпадают с собранными или которых нет."""
    out = []
    for name, text in expected.items():
        path = catalog_dir / name
        if not path.is_file() or path.read_text(encoding="utf-8") != text:
            out.append(name)
    return out


def main(argv):
    check = argv == ["--check"]
    if argv and not check:
        print("использование: scripts/build-model-catalog.py [--check]", file=sys.stderr)
        return 2
    try:
        expected = build(MODELS)
    except FileNotFoundError as e:
        print(f"нет {e.filename}: сначала scripts/fetch-models.sh и scripts/fetch-llm.sh", file=sys.stderr)
        return 1
    if check:
        stale = differences(expected, CATALOG)
        if stale:
            print(
                "каталог моделей устарел: " + ", ".join(stale) + "; запустите scripts/build-model-catalog.py",
                file=sys.stderr,
            )
            return 1
        print("каталог моделей совпадает с models.json и llm.json")
        return 0
    CATALOG.mkdir(parents=True, exist_ok=True)
    for name, text in expected.items():
        (CATALOG / name).write_text(text, encoding="utf-8")
    print(f"каталог моделей: {CATALOG}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
