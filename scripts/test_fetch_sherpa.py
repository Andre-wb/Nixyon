"""Проверки scripts/fetch-sherpa.sh. Запуск: python3 -I -B -m unittest discover -s scripts"""

import re
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("fetch-sherpa.sh").read_text(encoding="utf-8")


def pinned_files():
    # Строки массива FILES: имя файла и его SHA-256.
    body = re.search(r"^FILES=\(\n(.*?)^\)", SCRIPT, re.S | re.M).group(1)
    return dict(re.findall(r'^\s*"(\S+) ([0-9a-f]{64})"', body, re.M))


class FetchSherpaTest(unittest.TestCase):
    def test_every_build_host_has_its_static_library_pinned(self):
        # Крейт sherpa-onnx-sys ищет в папке архив под платформу сборки и падает без него.
        names = pinned_files()
        for platform in ("osx-arm64", "osx-x64", "linux-aarch64", "linux-x64"):
            self.assertIn(f"sherpa-onnx-v${{VERSION}}-{platform}-static-lib.tar.bz2", names)

    def test_every_pinned_static_library_is_picked_for_its_host(self):
        # Из архива платформы хоста достаётся libonnxruntime.a для крейта ort.
        picked = set(re.findall(r'\) platform="(\S+)" ;;', SCRIPT))
        pinned = {
            m.group(1)
            for name in pinned_files()
            if (m := re.fullmatch(r"sherpa-onnx-v\$\{VERSION\}-(\S+)-static-lib\.tar\.bz2", name))
        }
        self.assertEqual(picked, pinned)

    def test_android_library_is_pinned(self):
        self.assertIn("sherpa-onnx-${VERSION}.aar", pinned_files())

    def test_android_onnx_runtime_comes_from_the_pinned_aar_into_the_folder_cargo_uses(self):
        # Приложение линкуется с той же libonnxruntime.so, что лежит в aar для arm64.
        self.assertIn('aar="${DEST}/sherpa-onnx-${VERSION}.aar"', SCRIPT)
        self.assertIn('"jni/arm64-v8a/libonnxruntime.so"', SCRIPT)
        config = (Path(__file__).parent.parent / "nixyon-app/.cargo/config.toml").read_text(encoding="utf-8")
        folder = re.search(r'android_lib="\$\{DEST\}/(\S+)"', SCRIPT).group(1)
        self.assertIn(f'"../third_party/sherpa-onnx/{folder}"', config)


if __name__ == "__main__":
    unittest.main()
