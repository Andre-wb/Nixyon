"""Проверки scripts/listen-page.py. Запуск: python3 -I -B -m unittest discover -s scripts"""

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

_SPEC = importlib.util.spec_from_file_location("listen_page", Path(__file__).with_name("listen-page.py"))
page = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(page)


def turn(start, end, speaker, words):
    return {
        "start": start,
        "end": end,
        "speaker": speaker,
        "text": " ".join(w[2] for w in words),
        "words": [{"start": a, "end": b, "text": t} for a, b, t in words],
    }


def write_dump(root, combo, name, turns, duration=10.0):
    path = root / combo / f"{name}.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps({"audio": {"duration_sec": duration}, "turns": turns}), "utf-8")


class ListenPageTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def test_words_get_speaker_of_each_combo(self):
        words_a = [(0.5, 1.0, "привет"), (3.0, 3.5, "да")]
        write_dump(self.root, "A", "rec", [turn(0.0, 4.0, 1, words_a)])
        write_dump(self.root, "B", "rec", [turn(0.0, 2.0, 1, words_a[:1]), turn(2.5, 4.0, 2, words_a[1:])])
        data = page.build_data(self.root)
        words = data["records"]["rec"]["words"]
        self.assertEqual([w[2] for w in words], ["привет", "да"])
        self.assertEqual([w[3] for w in words], [[1, 1], [1, 2]])

    def test_titles_come_from_combos_json_and_b_is_default(self):
        for c in ("A", "B"):
            write_dump(self.root, c, "rec", [])
        (self.root / "combos.json").write_text(json.dumps({"A": {"title": "первое"}}), "utf-8")
        data = page.build_data(self.root)
        self.assertEqual(data["combos"], {"A": "первое", "B": "B"})
        self.assertEqual(data["liveCombo"], 1)

    def test_only_records_present_in_every_combo(self):
        write_dump(self.root, "A", "both", [])
        write_dump(self.root, "A", "only_a", [])
        write_dump(self.root, "B", "both", [])
        self.assertEqual(list(page.build_data(self.root)["records"]), ["both"])

    def test_word_outside_turns_has_no_speaker(self):
        write_dump(self.root, "A", "rec", [turn(0.0, 1.0, 1, [(5.0, 5.5, "эхо")])])
        self.assertEqual(page.build_data(self.root)["records"]["rec"]["words"][0][3], [0])

    def test_script_tag_in_text_does_not_break_page(self):
        write_dump(self.root, "A", "rec", [turn(0.0, 1.0, 1, [(0.1, 0.5, "</script><b>")])])
        html = page.render_page(page.build_data(self.root))
        self.assertEqual(html.count("</script>"), 1)
        self.assertNotIn("__DATA__", html)

    def test_no_combo_folders_is_an_error(self):
        with self.assertRaises(ValueError):
            page.build_data(self.root)

    def test_audio_found_by_any_known_extension(self):
        (self.root / "rec.m4a").write_bytes(b"")
        self.assertEqual(page.find_audio(self.root, "rec"), self.root / "rec.m4a")
        self.assertIsNone(page.find_audio(self.root, "missing"))


if __name__ == "__main__":
    unittest.main()
