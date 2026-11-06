"""Страница для проверки разметки спикеров на слух: плеер, дорожки сочетаний и текст по ходу записи.

Запуск: python3 -I scripts/listen-page.py <папка дампов> <папка звука> <папка страницы>

Папка дампов: то, что пишет eval с --dump и --combo: <сочетание>/<запись>.json и, если есть,
combos.json с названиями сочетаний. Без combos.json сочетания берутся по именам подпапок.
Папка звука: исходные записи <запись>.mp3 (или .wav, .m4a, .ogg, .opus, .flac).

В папку страницы пишется index.html и audio/<запись>.wav (16 кГц, моно, через ffmpeg).
Страница играет WAV, а не mp3: браузер перематывает mp3 с переменным битрейтом неточно,
и текст расходится со звуком. Готовые WAV повторно не пересчитываются.

Страница содержит расшифровки и звук разговоров: это личные данные, папку страницы
держать вне репозитория.
"""

import bisect
import json
import shutil
import subprocess
import sys
from pathlib import Path

TEMPLATE = Path(__file__).with_name("listen-page.html")
AUDIO_EXTENSIONS = (".mp3", ".wav", ".m4a", ".ogg", ".opus", ".flac")
# Сочетание, по которому по умолчанию раскрашивается текст: ReDimNet2 и pyannote.
PREFERRED_LIVE_COMBO = "B"
OVERLAP_LOOKBACK = 8


def load_combos(dumps):
    """Сочетания по порядку имён: имя и название для подписи."""
    names = sorted(p.name for p in dumps.iterdir() if p.is_dir())
    if not names:
        raise ValueError(f"в {dumps} нет папок сочетаний")
    titles = {}
    index = dumps / "combos.json"
    if index.is_file():
        titles = {k: v.get("title", k) for k, v in json.loads(index.read_text("utf-8")).items()}
    return {name: titles.get(name, name) for name in names}


def record_names(dumps, combos):
    """Записи, которые есть у всех сочетаний."""
    sets = [{p.stem for p in (dumps / c).glob("*.json")} for c in combos]
    return sorted(set.intersection(*sets))


def speaker_at(lane, starts, x):
    """Номер спикера реплики, в которую попадает момент x; 0, если такой реплики нет."""
    # Реплики разных спикеров могут перекрываться, поэтому смотрим несколько ближайших
    # реплик, начавшихся до x, а не только последнюю.
    last = bisect.bisect_right(starts, x) - 1
    for i in range(last, max(-1, last - OVERLAP_LOOKBACK), -1):
        start, end, speaker, _ = lane[i]
        if start <= x <= end:
            return speaker
    return 0


def build_record(dumps, combos, name):
    lanes = {}
    duration = 0.0
    words = []
    for c in combos:
        dump = json.loads((dumps / c / f"{name}.json").read_text("utf-8"))
        duration = dump["audio"]["duration_sec"]
        turns = sorted(dump["turns"], key=lambda t: t["start"])
        lanes[c] = [[round(t["start"], 2), round(t["end"], 2), t["speaker"], t["text"]] for t in turns]
        # Распознавание у сочетаний общее, слова берутся у первого.
        if not words:
            words = sorted(
                ([round(w["start"], 2), round(w["end"], 2), w["text"]] for t in turns for w in t["words"]),
                key=lambda w: w[0],
            )
    starts = {c: [t[0] for t in lane] for c, lane in lanes.items()}
    for w in words:
        middle = (w[0] + w[1]) / 2
        w.append([speaker_at(lanes[c], starts[c], middle) for c in combos])
    return {"audio": f"audio/{name}.wav", "duration": duration, "lanes": lanes, "words": words}


def build_data(dumps):
    combos = load_combos(dumps)
    names = record_names(dumps, combos)
    if not names:
        raise ValueError(f"в {dumps} нет записей, общих для всех сочетаний")
    keys = list(combos)
    live = keys.index(PREFERRED_LIVE_COMBO) if PREFERRED_LIVE_COMBO in keys else 0
    return {
        "combos": combos,
        "liveCombo": live,
        "records": {n: build_record(dumps, combos, n) for n in names},
    }


def render_page(data):
    # "</" внутри строк JSON закрыл бы тег script раньше времени.
    payload = json.dumps(data, ensure_ascii=False).replace("</", "<\\/")
    return TEMPLATE.read_text("utf-8").replace("__DATA__", payload, 1)


def find_audio(audio_dir, name):
    for ext in AUDIO_EXTENSIONS:
        path = audio_dir / f"{name}{ext}"
        if path.is_file():
            return path
    return None


def convert_audio(source, target):
    target.parent.mkdir(parents=True, exist_ok=True)
    partial = target.with_suffix(".part.wav")
    subprocess.run(
        ["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", "-i", str(source),
         "-ac", "1", "-ar", "16000", "-c:a", "pcm_s16le", str(partial)],
        check=True,
    )
    partial.replace(target)


def main(argv):
    if len(argv) != 3:
        print(__doc__.strip().splitlines()[2], file=sys.stderr)
        return 2
    dumps, audio_dir, out = (Path(a) for a in argv)
    try:
        data = build_data(dumps)
    except (OSError, ValueError, KeyError, json.JSONDecodeError) as e:
        print(f"ошибка: дампы не читаются: {e}", file=sys.stderr)
        return 1
    out.mkdir(parents=True, exist_ok=True)
    need = [n for n in data["records"] if not (out / "audio" / f"{n}.wav").is_file()]
    if need and shutil.which("ffmpeg") is None:
        print("ошибка: для перевода звука в WAV нужен ffmpeg", file=sys.stderr)
        return 1
    for name in need:
        source = find_audio(audio_dir, name)
        if source is None:
            print(f"{name}: звука нет в {audio_dir}, файл можно будет выбрать на странице", file=sys.stderr)
            continue
        try:
            convert_audio(source, out / "audio" / f"{name}.wav")
        except subprocess.CalledProcessError as e:
            print(f"ошибка: {name}: ffmpeg не перевёл звук ({e.returncode})", file=sys.stderr)
            return 1
        print(f"{name}: звук переведён в WAV")
    page = out / "index.html"
    page.write_text(render_page(data), "utf-8")
    print(f"страница: {page}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
