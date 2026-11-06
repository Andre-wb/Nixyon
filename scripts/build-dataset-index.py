"""Индекс набора записей для проверки качества: <корень>/<набор>/dataset.json и clips.jsonl.

Запуск: python3 -I scripts/build-dataset-index.py [--not-default] [--reference <кто>] <источник> <корень наборов> <набор> <название> <папка> [...]

--not-default: набор на странице проверки снят и включается только вручную (например,
полные части open_stt без текста: прогон по ним идёт сутками).

--reference: кто писал тексты набора: human (человек), auto (машина, например подгонка
текста к звуку) или unknown (неизвестно, так же без флага). Отчёт показывает WER по
наборам с текстом от человека отдельно.

Источники:
  cv          папка с парами <id>.mp3 и <id>.json (Common Voice);
  open_stt    папка с парами <имя>.wav или .opus и <имя>.txt (open_stt), по всем подпапкам;
  wav         папка с .wav без текста (например, запись одного человека);
  opus        папка с .opus (внутри Ogg Vorbis или Opus) без текста: полные части open_stt;
  podlodka    папка с parquet-файлами bond005/podlodka_speech: звук пишется в <папка>/audio;
  axondata    папка с парами <имя>.mp3 и <имя>.docx (AxonData, русская расшифровка в docx);
  dialogs     папка с парами <имя>.wav и <имя>.truth.json (разговоры с разметкой «кто и когда
              говорит» из scripts/fetch-test-audio.sh): по ним считается DER.

В clips.jsonl на строку: id, audio (путь от корня наборов), text (или null), duration
в секундах, speakers (если известно), truth (путь к разметке спикеров, если она есть). Звук не копируется, кроме podlodka, где он лежит
внутри parquet. Одинаковые записи (по содержимому) берутся один раз.
"""

import hashlib
import json
import re
import struct
import sys
import zipfile
from pathlib import Path

# Как размечен набор: один голос в клипе, разговор нескольких людей, или вперемешку.
KINDS = {"single", "dialog", "mixed"}
REFERENCES = {"human", "auto", "unknown"}

DESCRIPTIONS = {
    "cv": ("single", "CC0"),
    "open_stt": ("single", "CC BY-NC 4.0"),
    "wav": ("single", "см. источник"),
    "opus": ("single", "CC BY-NC 4.0"),
    "podlodka": ("mixed", "см. источник"),
    "axondata": ("dialog", "CC BY-NC 4.0"),
    "dialogs": ("dialog", "см. scripts/fetch-test-audio.sh"),
}


def wav_duration(path):
    with open(path, "rb") as f:
        head = f.read(12)
        if head[:4] != b"RIFF" or head[8:12] != b"WAVE":
            raise ValueError(f"{path}: не WAV")
        rate = channels = bits = None
        while True:
            chunk = f.read(8)
            if len(chunk) < 8:
                raise ValueError(f"{path}: нет данных")
            tag, size = chunk[:4], struct.unpack("<I", chunk[4:])[0]
            if tag == b"fmt ":
                fmt = f.read(size)
                channels, rate = struct.unpack("<HI", fmt[2:8])
                bits = struct.unpack("<H", fmt[14:16])[0]
                if size % 2:
                    f.read(1)
            elif tag == b"data":
                if not rate:
                    raise ValueError(f"{path}: данные раньше формата")
                return size / (rate * channels * (bits // 8))
            else:
                f.seek(size + size % 2, 1)


def ogg_duration(path):
    """По конечной отметке времени Ogg: у Vorbis в отсчётах своей частоты, у Opus в 48 кГц
    минус pre-skip (RFC 7845). Файлы .opus в open_stt внутри Ogg Vorbis."""
    data = Path(path).read_bytes()
    if data[:4] != b"OggS":
        raise ValueError(f"{path}: не Ogg")
    # Страницы проходятся с начала по их длине: байты OggS встречаются и внутри сжатого
    # звука, и поиск последнего вхождения находил бы ложную страницу. Обрезанная последняя
    # страница не считается.
    granule = 0
    i = 0
    while i + 27 <= len(data) and data[i : i + 4] == b"OggS":
        segments = data[i + 26]
        if i + 27 + segments > len(data):
            break
        end = i + 27 + segments + sum(data[i + 27 : i + 27 + segments])
        if end > len(data):
            break
        page_granule = struct.unpack("<q", data[i + 6 : i + 14])[0]
        # -1: на странице не кончается ни один пакет, отметки нет.
        if page_granule != -1:
            granule = page_granule
        i = end
    head = data[28:64]
    if head.startswith(b"\x01vorbis"):
        rate = struct.unpack("<I", head[12:16])[0]
        return max(granule, 0) / rate
    if head.startswith(b"OpusHead"):
        pre_skip = struct.unpack("<H", head[10:12])[0]
        return max(granule - pre_skip, 0) / 48_000
    raise ValueError(f"{path}: в Ogg не Vorbis и не Opus")


# Битрейт Layer III в кбит/с по индексу из заголовка кадра: у MPEG-1 своя таблица, у
# MPEG-2 и MPEG-2.5 (частоты 8-24 кГц, например телефонные звонки) своя.
MP3_BITRATES = {
    1: [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320],
    2: [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160],
}
# Частота по версии MPEG (биты заголовка 11, 10, 00) и индексу частоты.
MP3_RATES = {0b11: [44100, 48000, 32000], 0b10: [22050, 24000, 16000], 0b00: [11025, 12000, 8000]}


def mp3_duration(path):
    """Длительность mp3 по всем кадрам: у файлов с переменным битрейтом первый кадр врёт."""
    data = Path(path).read_bytes()
    i = 0
    if data[:3] == b"ID3":
        size = data[6:10]
        i = 10 + ((size[0] << 21) | (size[1] << 14) | (size[2] << 7) | size[3])
    seconds = 0.0
    while i + 4 <= len(data):
        version = (data[i + 1] >> 3) & 0b11
        layer = (data[i + 1] >> 1) & 0b11
        index = data[i + 2] >> 4
        rate_index = (data[i + 2] >> 2) & 0b11
        if not (
            data[i] == 0xFF
            and data[i + 1] & 0xE0 == 0xE0
            and version != 0b01
            and layer == 0b01
            and 0 < index < 15
            and rate_index < 3
        ):
            i += 1
            continue
        mpeg1 = version == 0b11
        rate = MP3_RATES[version][rate_index]
        bitrate = MP3_BITRATES[1 if mpeg1 else 2][index] * 1000
        padding = (data[i + 2] >> 1) & 1
        length = (144 if mpeg1 else 72) * bitrate // rate + padding
        seconds += (1152 if mpeg1 else 576) / rate
        i += max(length, 1)
    if seconds == 0:
        raise ValueError(f"{path}: нет кадра mp3")
    return seconds


def content_key(path):
    return hashlib.sha1(Path(path).read_bytes()).hexdigest()


class OutsideRoot(Exception):
    """Запись лежит вне корня наборов: путь от корня к ней не записать."""


def relative(path, root):
    resolved, base = Path(path).resolve(), Path(root).resolve()
    if not resolved.is_relative_to(base):
        raise OutsideRoot(f"{path}: запись вне корня наборов {root}, папки с записями должны лежать внутри него")
    return resolved.relative_to(base).as_posix()


def from_cv(folder, root):
    for meta in sorted(Path(folder).glob("*.json")):
        clip = json.loads(meta.read_text(encoding="utf-8"))
        audio = meta.with_suffix(".mp3")
        if audio.exists():
            yield {
                "id": clip["id"],
                "audio": relative(audio, root),
                "text": clip["text"],
                "duration": float(clip["duration"]),
                "speakers": 1,
            }


def from_open_stt(folder, root):
    for text_file in sorted(Path(folder).rglob("*.txt")):
        for ext, duration in ((".opus", ogg_duration), (".wav", wav_duration)):
            audio = text_file.with_suffix(ext)
            if audio.exists():
                yield {
                    "id": text_file.stem,
                    "audio": relative(audio, root),
                    "text": text_file.read_text(encoding="utf-8").strip(),
                    "duration": duration(audio),
                    "speakers": 1,
                }
                break


def from_wav(folder, root):
    for audio in sorted(Path(folder).rglob("*.wav")):
        yield {
            "id": audio.stem,
            "audio": relative(audio, root),
            "text": None,
            "duration": wav_duration(audio),
            "speakers": 1,
        }


def from_opus(folder, root):
    for audio in sorted(Path(folder).rglob("*.opus")):
        yield {
            "id": audio.stem,
            "audio": relative(audio, root),
            "text": None,
            "duration": ogg_duration(audio),
            "speakers": 1,
        }


def from_podlodka(folder, root, out_dir):
    import pyarrow.parquet as pq

    audio_dir = out_dir / "audio"
    audio_dir.mkdir(parents=True, exist_ok=True)
    for part in sorted(Path(folder).glob("*.parquet")):
        for n, row in enumerate(pq.read_table(part).to_pylist()):
            data = row["audio"]["bytes"]
            key = hashlib.sha1(data).hexdigest()[:16]
            audio = audio_dir / f"{key}.wav"
            if not audio.exists():
                audio.write_bytes(data)
            yield {
                "id": f"{part.stem}_{n:04d}",
                "audio": relative(audio, root),
                "text": row["transcription"].strip(),
                "duration": wav_duration(audio),
                "speakers": None,
            }


def docx_paragraphs(path):
    xml = zipfile.ZipFile(path).read("word/document.xml").decode("utf-8")
    paragraphs = (re.sub(r"<[^>]+>", "", p) for p in re.findall(r"<w:p[ >].*?</w:p>", xml))
    return [p.strip() for p in paragraphs if p.strip()]


def from_axondata(folder, root):
    timestamp = re.compile(r"^\d{2}:\d{2}:\d{2}$")
    for doc in sorted(Path(folder).glob("*.docx")):
        paragraphs = docx_paragraphs(doc)
        # Русская расшифровка идёт после английской и кончается вместе с файлом.
        start = next(i for i, p in enumerate(paragraphs) if p.startswith("RU transcription"))
        text = " ".join(p for p in paragraphs[start + 1 :] if not timestamp.match(p))
        audio = doc.with_suffix(".mp3")
        yield {
            "id": doc.stem,
            "audio": relative(audio, root),
            "text": text,
            "duration": mp3_duration(audio),
            "speakers": 2,
        }


def take_option(argv, name):
    """Значение флага и argv без него; флага нет: None."""
    if name not in argv:
        return None, argv
    i = argv.index(name)
    if i + 1 >= len(argv):
        raise ValueError(f"{name}: нет значения")
    return argv[i + 1], argv[:i] + argv[i + 2 :]


def from_dialogs(folder, root):
    for truth_file in sorted(Path(folder).glob("*.truth.json")):
        audio = truth_file.with_name(truth_file.name.removesuffix(".truth.json") + ".wav")
        if not audio.exists():
            continue
        truth = json.loads(truth_file.read_text(encoding="utf-8"))
        texts = [turn.get("text") for turn in sorted(truth["turns"], key=lambda t: t["start"])]
        yield {
            "id": audio.stem,
            "audio": relative(audio, root),
            # Текст есть, только если он есть у каждой реплики.
            "text": " ".join(texts) if all(texts) else None,
            "duration": wav_duration(audio),
            "speakers": truth["speakers"],
            "truth": relative(truth_file, root),
        }


def main(argv):
    default_selected = "--not-default" not in argv
    argv = [a for a in argv if a != "--not-default"]
    try:
        reference, argv = take_option(argv, "--reference")
    except ValueError as e:
        print(e, file=sys.stderr)
        return 2
    reference = reference or "unknown"
    if reference not in REFERENCES:
        print(f"--reference: одно из {', '.join(sorted(REFERENCES))}", file=sys.stderr)
        return 2
    if len(argv) < 5:
        print(__doc__, file=sys.stderr)
        return 2
    source, root, name, title = argv[1], Path(argv[2]), argv[3], argv[4]
    folders = argv[5:]
    if source not in DESCRIPTIONS or not folders:
        print(__doc__, file=sys.stderr)
        return 2
    if not re.fullmatch(r"[a-z0-9_]+", name):
        print("имя набора: строчные латинские буквы, цифры и _", file=sys.stderr)
        return 2
    out_dir = root / name
    out_dir.mkdir(parents=True, exist_ok=True)
    kind, license_name = DESCRIPTIONS[source]
    seen = set()
    clips = []
    for folder in folders:
        if source == "podlodka":
            found = from_podlodka(folder, root, out_dir)
        else:
            found = {
                "cv": from_cv,
                "open_stt": from_open_stt,
                "wav": from_wav,
                "opus": from_opus,
                "axondata": from_axondata,
                "dialogs": from_dialogs,
            }[source](folder, root)
        try:
            for clip in found:
                key = content_key(root / clip["audio"])
                if key in seen:
                    continue
                seen.add(key)
                clips.append(clip)
        except OutsideRoot as e:
            print(e, file=sys.stderr)
            return 2
    if not clips:
        print(f"{name}: клипов не найдено", file=sys.stderr)
        return 1
    with open(out_dir / "clips.jsonl", "w", encoding="utf-8") as f:
        for clip in clips:
            f.write(json.dumps(clip, ensure_ascii=False) + "\n")
    hours = sum(c["duration"] for c in clips) / 3600
    described = {
        "id": name,
        "title": title,
        "kind": kind,
        "has_text": all(c["text"] for c in clips),
        "reference": reference,
        "license": license_name,
        "clips": len(clips),
        "hours": round(hours, 3),
        "default_selected": default_selected,
    }
    (out_dir / "dataset.json").write_text(
        json.dumps(described, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print(f"{name}: клипов {len(clips)}, часов {hours:.2f}, текст {'есть' if described['has_text'] else 'нет'}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
