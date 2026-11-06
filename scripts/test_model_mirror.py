"""Тесты зеркала моделей: model-mirror.sh (загрузка с зеркала кусками и запасной путь)
и publish-models.sh (раскладка скачанных моделей под их SHA-256)."""

import hashlib
import http.server
import os
import shutil
import subprocess
import tempfile
import threading
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent
LIBRARY = SCRIPTS / "model-mirror.sh"
PUBLISH = SCRIPTS / "publish-models.sh"
# Больше порога, с которого файл делится на куски.
BIG = 17 * 1024 * 1024


def sha(data):
    return hashlib.sha256(data).hexdigest()


class Mirror(http.server.ThreadingHTTPServer):
    """Зеркало в памяти: /models/<сумма>, HEAD и один кусок Range."""

    # Все куски подключаются разом: очереди по умолчанию (5) не хватает.
    request_queue_size = 64

    def __init__(self):
        super().__init__(("127.0.0.1", 0), MirrorHandler)
        self.files = {}
        self.ranges = []
        self.requests = 0
        self.ignore_range = False
        self.lock = threading.Lock()

    @property
    def url(self):
        return f"http://127.0.0.1:{self.server_address[1]}/models"


class MirrorHandler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_HEAD(self):
        self.answer(with_body=False)

    def do_GET(self):
        self.answer(with_body=True)

    def answer(self, with_body):
        server = self.server
        with server.lock:
            server.requests += 1
        data = server.files.get(self.path.removeprefix("/models/"))
        if data is None:
            self.send_error(404)
            return
        status, body = 200, data
        wanted = self.headers.get("Range")
        if wanted and not server.ignore_range:
            start, end = (int(x) for x in wanted.removeprefix("bytes=").split("-"))
            with server.lock:
                server.ranges.append((start, end))
            status, body = 206, data[start : end + 1]
        self.send_response(status)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        if with_body:
            self.wfile.write(body)


class DownloadTest(unittest.TestCase):
    def setUp(self):
        self.dir = Path(tempfile.mkdtemp())
        self.mirror = Mirror()
        threading.Thread(target=self.mirror.serve_forever, daemon=True).start()

    def tearDown(self):
        self.mirror.shutdown()
        self.mirror.server_close()
        shutil.rmtree(self.dir)

    def download(self, expected, source, mirror=None, jobs="16"):
        """model_download с запасным путём «скопировать source»; код выхода и вывод."""
        out = self.dir / "model.onnx.part"
        env = {**os.environ, "NIXYON_FETCH_JOBS": jobs}
        env["NIXYON_MODELS_MIRROR"] = self.mirror.url if mirror is None else mirror
        run = subprocess.run(
            [
                "bash",
                "-c",
                'set -euo pipefail; source "$1"; model_download "$2" "$3" cp "$4" "$2"',
                "_",
                str(LIBRARY),
                str(out),
                expected,
                str(source),
            ],
            capture_output=True,
            text=True,
            env=env,
            check=False,
        )
        return run, out

    def source(self, data):
        path = self.dir / "source.bin"
        path.write_bytes(data)
        return path

    def test_a_big_file_comes_from_the_mirror_in_sixteen_ranges(self):
        data = os.urandom(BIG)
        self.mirror.files[sha(data)] = data
        run, out = self.download(sha(data), self.dir / "нет")
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertEqual(out.read_bytes(), data)
        self.assertIn("взят с зеркала", run.stdout)
        self.assertEqual(len(self.mirror.ranges), 16)
        self.assertEqual(sorted(self.mirror.ranges)[-1][1], BIG - 1)
        self.assertEqual(list(self.dir.glob("model.onnx.part.*")), [])

    def test_a_small_file_comes_in_one_request(self):
        data = b"small model"
        self.mirror.files[sha(data)] = data
        run, out = self.download(sha(data), self.dir / "нет")
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertEqual(out.read_bytes(), data)
        self.assertEqual(self.mirror.ranges, [])

    def test_without_the_file_on_the_mirror_the_source_is_used(self):
        data = b"model from the source"
        run, out = self.download(sha(data), self.source(data))
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertEqual(out.read_bytes(), data)
        self.assertNotIn("взят с зеркала", run.stdout)

    def test_wrong_content_on_the_mirror_falls_back_to_the_source(self):
        data = b"real model"
        self.mirror.files[sha(data)] = b"forged model"
        run, out = self.download(sha(data), self.source(data))
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertEqual(out.read_bytes(), data)

    def test_a_mirror_that_ignores_ranges_falls_back_to_the_source(self):
        data = os.urandom(BIG)
        self.mirror.files[sha(data)] = data
        self.mirror.ignore_range = True
        source = self.source(data)
        run, out = self.download(sha(data), source)
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertEqual(out.read_bytes(), data)
        self.assertNotIn("взят с зеркала", run.stdout)

    def test_an_empty_mirror_address_turns_the_mirror_off(self):
        data = b"model"
        self.mirror.files[sha(data)] = data
        run, out = self.download(sha(data), self.source(data), mirror="")
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertEqual(self.mirror.requests, 0)

    def test_a_bad_jobs_value_skips_the_mirror(self):
        data = b"model"
        self.mirror.files[sha(data)] = data
        run, _ = self.download(sha(data), self.source(data), jobs="0")
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertEqual(self.mirror.requests, 0)
        self.assertIn("NIXYON_FETCH_JOBS", run.stderr)

    def test_a_wrong_sum_from_the_source_is_an_error_and_leaves_no_file(self):
        run, out = self.download("0" * 64, self.source(b"other"))
        self.assertNotEqual(run.returncode, 0)
        self.assertIn("сумма model.onnx не совпала", run.stderr)
        self.assertFalse(out.exists())


class PublishTest(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp())
        (self.root / "scripts").mkdir()
        for script in (LIBRARY, PUBLISH):
            shutil.copy(script, self.root / "scripts" / script.name)
        self.models = self.root / "models"
        for folder in ("_download/llm", "_download/archive-dir", "ort"):
            (self.models / folder).mkdir(parents=True)
        self.mirror = self.root / "mirror"
        self.mirror.mkdir()

    def tearDown(self):
        shutil.rmtree(self.root)

    def publish(self):
        return subprocess.run(
            ["bash", str(self.root / "scripts" / PUBLISH.name), str(self.mirror)],
            capture_output=True,
            text=True,
            check=False,
        )

    def test_checked_files_are_laid_out_by_sum_and_the_rest_is_skipped_or_removed(self):
        files = {
            "_download/model.tar.bz2": b"archive",
            "_download/llm/model.gguf": b"language model",
            "ort/redimnet2-b6.onnx": b"voice model",
        }
        for name, data in files.items():
            (self.models / name).write_bytes(data)
        (self.models / "_download/half.tar.bz2.part").write_bytes(b"half")
        (self.models / "ort/models.json").write_text("{}")
        (self.models / "_download/link.onnx").symlink_to(self.models / "ort/models.json")
        stale = self.mirror / ("f" * 64)
        stale.write_bytes(b"old model")
        (self.mirror / ".tmp-leftover.tmp").write_bytes(b"x")

        run = self.publish()
        self.assertEqual(run.returncode, 0, run.stderr)
        published = sorted(p.name for p in self.mirror.iterdir() if not p.name.startswith("."))
        self.assertEqual(published, sorted(sha(d) for d in files.values()))
        for data in files.values():
            self.assertEqual((self.mirror / sha(data)).read_bytes(), data)
        self.assertFalse(stale.exists())
        self.assertFalse((self.mirror / ".tmp-leftover.tmp").exists())

        sums = (self.mirror / ".sums").read_text().splitlines()
        self.assertEqual(sorted(line.split()[0] for line in sums), published)

        again = self.publish()
        self.assertEqual(again.returncode, 0, again.stderr)
        self.assertNotIn("в зеркале:", again.stdout)

    def test_a_missing_mirror_folder_is_an_error(self):
        shutil.rmtree(self.mirror)
        run = self.publish()
        self.assertNotEqual(run.returncode, 0)
        self.assertIn("нет папки зеркала", run.stderr)


if __name__ == "__main__":
    unittest.main()
