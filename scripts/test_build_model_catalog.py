"""Проверки scripts/build-model-catalog.py. Запуск: python3 -I -B -m unittest discover -s scripts"""

import importlib.util
import json
import os
import tempfile
import unittest
from pathlib import Path

_SPEC = importlib.util.spec_from_file_location(
    "build_model_catalog", Path(__file__).with_name("build-model-catalog.py")
)
catalog = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(catalog)


def entry(path, size):
    return {"path": path, "size": size, "sha256": "a" * 64}


class BuildTest(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.models = Path(self.dir.name)
        (self.models / "llm").mkdir()
        speech = {
            "version": "set-1",
            "files": {
                "vad": entry("vad.onnx", 10),
                "tagging_model": entry("tagging.int8.onnx", 20),
                "tagging_labels": entry("tagging-labels.csv", 30),
                "age_gender": entry("age-gender.onnx", 40),
            },
        }
        (self.models / "models.json").write_text(json.dumps(speech), encoding="utf-8")
        llm = {"version": "llm-1", "model": entry("model.gguf", 50)}
        (self.models / "llm" / "llm.json").write_text(json.dumps(llm), encoding="utf-8")

    def tearDown(self):
        self.dir.cleanup()

    def test_the_music_filter_is_left_out_and_the_other_roles_stay(self):
        built = json.loads(catalog.build(self.models)["models.json"])
        self.assertEqual(built["version"], "set-1")
        self.assertEqual(sorted(built["files"]), ["age_gender", "vad"])
        self.assertEqual(built["files"]["vad"], entry("vad.onnx", 10))

    def test_the_language_model_keeps_its_version_and_file(self):
        built = json.loads(catalog.build(self.models)["llm.json"])
        self.assertEqual(built, {"version": "llm-1", "model": entry("model.gguf", 50)})

    def test_a_changed_or_missing_catalog_file_is_reported(self):
        expected = catalog.build(self.models)
        out = Path(self.dir.name) / "catalog"
        out.mkdir()
        (out / "models.json").write_text(expected["models.json"], encoding="utf-8")
        self.assertEqual(catalog.differences(expected, out), ["llm.json"])
        (out / "llm.json").write_text(expected["llm.json"].replace("llm-1", "llm-2"), encoding="utf-8")
        self.assertEqual(catalog.differences(expected, out), ["llm.json"])
        (out / "llm.json").write_text(expected["llm.json"], encoding="utf-8")
        self.assertEqual(catalog.differences(expected, out), [])


class CommittedCatalogTest(unittest.TestCase):
    # В CI перед тестами скриптов запускаются fetch-models.sh и fetch-llm.sh --manifest-only,
    # там без манифестов тест падает, а не пропускается.
    def test_the_catalog_in_the_repository_matches_the_fetched_models(self):
        manifests = [catalog.MODELS / "models.json", catalog.MODELS / "llm" / "llm.json"]
        if not all(path.is_file() for path in manifests):
            if os.environ.get("CI"):
                self.fail("нет models.json или llm.json: каталог моделей не с чем сверить")
            self.skipTest("модели не скачаны")
        stale = catalog.differences(catalog.build(catalog.MODELS), catalog.CATALOG)
        self.assertEqual(stale, [], "каталог устарел: запустите scripts/build-model-catalog.py")


if __name__ == "__main__":
    unittest.main()
