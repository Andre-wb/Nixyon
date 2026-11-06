"""Проверки scripts/fetch-child-datasets.py. Запуск: python3 -I -B -m unittest discover -s scripts"""

import contextlib
import hashlib
import importlib.util
import io
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path

_SPEC = importlib.util.spec_from_file_location(
    "fetch_child_datasets", Path(__file__).with_name("fetch-child-datasets.py")
)
fetch = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(fetch)


def tar_bytes(members):
    buf = io.BytesIO()
    with tarfile.open(fileobj=buf, mode="w:gz") as tar:
        for name, data, kind in members:
            info = tarfile.TarInfo(name)
            info.type = kind
            if kind == tarfile.SYMTYPE:
                info.linkname = "/etc/passwd"
            else:
                info.size = len(data)
            tar.addfile(info, io.BytesIO(data) if kind == tarfile.REGTYPE else None)
    return buf.getvalue()


def zip_bytes(members):
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w") as z:
        for name, data, mode in members:
            info = zipfile.ZipInfo(name)
            info.external_attr = mode << 16
            z.writestr(info, data)
    return buf.getvalue()


class FetchChildDatasetsTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name) / "root"
        self.mirror = Path(self.tmp.name) / "mirror"
        self.saved = dict(fetch.DATASETS)
        self.saved_min_free = fetch.MIN_FREE_BYTES
        fetch.MIN_FREE_BYTES = 0

    def tearDown(self):
        fetch.DATASETS.clear()
        fetch.DATASETS.update(self.saved)
        fetch.MIN_FREE_BYTES = self.saved_min_free
        self.tmp.cleanup()

    def serve(self, name, rel, data, pinned):
        path = self.mirror / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        fetch.DATASETS[name] = (rel, pinned, len(data))
        return [self.mirror.as_uri()]

    def run_fetch(self, name, mirrors):
        with contextlib.redirect_stdout(io.StringIO()):
            return fetch.fetch(self.root, name, mirrors)

    def test_a_pinned_archive_is_unpacked_marked_complete_and_removed(self):
        data = tar_bytes([("set/a.txt", b"hello", tarfile.REGTYPE)])
        mirrors = self.serve("demo", "1/set.tar.gz", data, hashlib.sha256(data).hexdigest())
        dest = self.run_fetch("demo", mirrors)
        self.assertEqual((dest / "set/a.txt").read_bytes(), b"hello")
        self.assertTrue((dest / fetch.COMPLETE).exists())
        self.assertFalse((self.root / fetch.DOWNLOAD_DIR / "set.tar.gz").exists())
        (self.mirror / "1/set.tar.gz").unlink()
        self.assertEqual(self.run_fetch("demo", mirrors), dest)

    def test_a_wrong_pinned_sum_fails_and_leaves_no_archive(self):
        data = tar_bytes([("a.txt", b"x", tarfile.REGTYPE)])
        mirrors = self.serve("demo", "1/set.tar.gz", data, "0" * 64)
        with self.assertRaises(fetch.FetchError):
            self.run_fetch("demo", mirrors)
        self.assertFalse((self.root / fetch.DOWNLOAD_DIR / "set.tar.gz").exists())
        self.assertFalse((self.root / "demo" / fetch.COMPLETE).exists())

    def test_the_first_sum_of_an_unpinned_archive_is_recorded_and_checked_later(self):
        first = zip_bytes([("k/a.txt", b"one", 0o100644)])
        mirrors = self.serve("kids", "2/kids.zip", first, None)
        self.run_fetch("kids", mirrors)
        record = self.root / fetch.DOWNLOAD_DIR / "kids.zip.sha256"
        self.assertEqual(record.read_text().strip(), hashlib.sha256(first).hexdigest())
        (self.root / "kids" / fetch.COMPLETE).unlink()
        self.serve("kids", "2/kids.zip", zip_bytes([("k/a.txt", b"two", 0o100644)]), None)
        with self.assertRaises(fetch.FetchError):
            self.run_fetch("kids", mirrors)

    def test_paths_leaving_the_folder_and_links_are_refused(self):
        cases = [
            ("t1", "1/t1.tgz", tar_bytes([("../evil", b"x", tarfile.REGTYPE)])),
            ("t2", "1/t2.tgz", tar_bytes([("link", b"", tarfile.SYMTYPE)])),
            ("z1", "1/z1.zip", zip_bytes([("/abs", b"x", 0o100644)])),
            ("z2", "1/z2.zip", zip_bytes([("link", b"/etc/passwd", 0o120777)])),
        ]
        for name, rel, data in cases:
            mirrors = self.serve(name, rel, data, hashlib.sha256(data).hexdigest())
            with self.subTest(name=name), self.assertRaises(fetch.FetchError):
                self.run_fetch(name, mirrors)
            self.assertFalse((Path(self.tmp.name) / "evil").exists())

    def test_the_next_mirror_is_tried_when_one_fails(self):
        data = tar_bytes([("a.txt", b"x", tarfile.REGTYPE)])
        good = self.serve("demo", "1/set.tar.gz", data, hashlib.sha256(data).hexdigest())
        dead = (Path(self.tmp.name) / "nowhere").as_uri()
        dest = self.run_fetch("demo", [dead, *good])
        self.assertTrue((dest / "a.txt").exists())

    def test_nothing_is_downloaded_when_the_disk_lacks_room_for_archive_set_and_reserve(self):
        data = tar_bytes([("a.txt", b"x", tarfile.REGTYPE)])
        mirrors = self.serve("demo", "1/set.tar.gz", data, hashlib.sha256(data).hexdigest())
        fetch.MIN_FREE_BYTES = 10**18
        with self.assertRaises(fetch.FetchError) as caught:
            self.run_fetch("demo", mirrors)
        self.assertIn("мало места", str(caught.exception))
        self.assertFalse((self.root / fetch.DOWNLOAD_DIR / "set.tar.gz").exists())

    def test_unknown_names_print_usage(self):
        with contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(fetch.main([str(self.root), "nope"]), 2)
            self.assertEqual(fetch.main([]), 2)


if __name__ == "__main__":
    unittest.main()
