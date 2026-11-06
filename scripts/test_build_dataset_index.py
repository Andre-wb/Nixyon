"""Проверки scripts/build-dataset-index.py. Запуск: python3 -I -B -m unittest discover -s scripts"""

import contextlib
import importlib.util
import io
import json
import struct
import tempfile
import unittest
from pathlib import Path

_SPEC = importlib.util.spec_from_file_location(
    "build_dataset_index", Path(__file__).with_name("build-dataset-index.py")
)
index = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(index)


def ogg_page(granule, payload, seq):
    assert len(payload) < 255
    return (
        b"OggS"
        + bytes([0, 0])
        + struct.pack("<qIII", granule, 1, seq, 0)
        + bytes([1, len(payload)])
        + payload
    )


def vorbis_head(rate):
    return b"\x01vorbis" + struct.pack("<IBIiii", 0, 1, rate, 0, 0, 0) + b"\xb8\x01"


class OggDurationTest(unittest.TestCase):
    def duration(self, data):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "clip.opus"
            path.write_bytes(data)
            return index.ogg_duration(path)

    def test_ogg_s_bytes_inside_audio_are_not_a_page(self):
        # Сжатый звук может случайно содержать байты OggS: за ними не отметка времени.
        fake = b"OggS" + b"\x00\x00" + struct.pack("<q", 10**15) + b"\x00" * 20
        data = ogg_page(0, vorbis_head(16_000), 0) + ogg_page(32_000, fake, 1)
        self.assertEqual(self.duration(data), 2.0)

    def test_cut_last_page_is_skipped(self):
        data = ogg_page(0, vorbis_head(16_000), 0) + ogg_page(16_000, b"\x00" * 40, 1)
        cut = ogg_page(10**15, b"\x00" * 40, 2)[:30]
        self.assertEqual(self.duration(data + cut), 1.0)


def wav_bytes(samples):
    data = b"\x00\x00" * samples
    fmt = struct.pack("<HHIIHH", 1, 1, 16_000, 32_000, 2, 16)
    return (
        b"RIFF"
        + struct.pack("<I", 4 + 8 + len(fmt) + 8 + len(data))
        + b"WAVE"
        + b"fmt "
        + struct.pack("<I", len(fmt))
        + fmt
        + b"data"
        + struct.pack("<I", len(data))
        + data
    )


class ReferenceTest(unittest.TestCase):
    def build(self, *flags):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root / "src").mkdir()
            (root / "src" / "a.wav").write_bytes(wav_bytes(16_000))
            code = index.main(
                ["x", *flags, "wav", str(root), "voice", "Голос", str(root / "src")]
            )
            if code != 0:
                return code, None
            return code, json.loads((root / "voice" / "dataset.json").read_text("utf-8"))

    def test_who_wrote_the_texts_goes_into_the_description(self):
        self.assertEqual(self.build("--reference", "human")[1]["reference"], "human")
        self.assertEqual(self.build()[1]["reference"], "unknown")

    def test_unknown_reference_value_is_refused(self):
        self.assertEqual(self.build("--reference", "robot")[0], 2)
        self.assertEqual(self.build("--reference")[0], 2)


class DialogsTest(unittest.TestCase):
    def test_a_dialog_carries_its_truth_text_and_speakers(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            src = root / "dialogs"
            src.mkdir()
            (src / "ru2.wav").write_bytes(wav_bytes(32_000))
            turns = [
                {"speaker": 2, "start": 1.0, "end": 2.0, "text": "два"},
                {"speaker": 1, "start": 0.0, "end": 1.0, "text": "раз"},
            ]
            (src / "ru2.truth.json").write_text(json.dumps({"speakers": 2, "turns": turns}))
            (src / "real3.wav").write_bytes(wav_bytes(16_000))
            (src / "real3.truth.json").write_text(
                json.dumps({"speakers": 3, "turns": [{"speaker": 1, "start": 0, "end": 1}]})
            )
            code = index.main(["x", "--reference", "human", "dialogs", str(root), "talk", "Разговоры", str(src)])
            self.assertEqual(code, 0)
            clips = [
                json.loads(line)
                for line in (root / "talk" / "clips.jsonl").read_text("utf-8").splitlines()
            ]
            ru2 = next(c for c in clips if c["id"] == "ru2")
            self.assertEqual(ru2["text"], "раз два")
            self.assertEqual(ru2["speakers"], 2)
            self.assertEqual(ru2["truth"], "dialogs/ru2.truth.json")
            self.assertEqual(ru2["duration"], 2.0)
            real3 = next(c for c in clips if c["id"] == "real3")
            self.assertIsNone(real3["text"], "текст не у всех реплик")


class OutsideRootTest(unittest.TestCase):
    def test_records_outside_the_root_are_refused_with_a_message_not_a_traceback(self):
        with tempfile.TemporaryDirectory() as root, tempfile.TemporaryDirectory() as src:
            (Path(src) / "a.wav").write_bytes(wav_bytes(16_000))
            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                code = index.main(["x", "wav", root, "voice", "Голос", src])
            self.assertEqual(code, 2)
            self.assertIn("вне корня наборов", stderr.getvalue())
            self.assertFalse((Path(root) / "voice" / "clips.jsonl").exists())


if __name__ == "__main__":
    unittest.main()
