"""Скачивает свободные наборы для обучения классификатора «ребёнок или взрослый».

Запуск: python3 -I scripts/fetch-child-datasets.py <корень наборов> [<набор> ...]

Наборы (без имён скачиваются все):
  speechocean762     OpenSLR 101, дети и взрослые, читают по-английски, Apache-2.0;
  samromur_children  OpenSLR 117, Samrómur Children 21.09, дети 4-17 лет, CC BY 4.0;
  samromur           OpenSLR 112, Samrómur 21.05, взрослые той же платформы, CC BY 4.0.

Каждый набор распаковывается в <корень>/<набор>/, архив после распаковки удаляется,
готовый набор повторно не качается. Сумма SHA-256 архива speechocean762 закреплена
(снята с архива, скачанного для замера). Для архивов Samrómur OpenSLR суммы
не публикует: сумма первой загрузки записывается в <корень>/_download/<архив>.sha256, и
следующие загрузки сверяются с ней. Архив чужой, поэтому распаковка отказывает на
абсолютных путях, «..», ссылках и особых файлах.

На сервере диск общий с другими службами, и переполнение уже роняло их. Поэтому до
загрузки проверяется место: архив и распакованный набор вместе, плюс запас
MIN_FREE_BYTES, иначе загрузка не начинается.
"""

import hashlib
import shutil
import sys
import tarfile
import urllib.request
import zipfile
from pathlib import Path, PurePosixPath

MIRRORS = (
    "https://openslr.elda.org/resources",
    "https://openslr.trmal.net/resources",
    "https://openslr.magicdatatech.com/resources",
)
# Путь на зеркале, SHA-256 (если закреплена) и размер архива в байтах: у speechocean762
# по скачанному архиву, у Samrómur по заголовку Content-Length зеркала elda.
DATASETS = {
    "speechocean762": (
        "101/speechocean762.tar.gz",
        "876c34b828c2ebcc35ba2ff1ba5947d50e6b0162fc4a4af91db0942422b3bdd5",
        520_810_923,
    ),
    "samromur_children": ("117/samromur_children_21.09.zip", None, 6_834_935_694),
    "samromur": ("112/samromur_21.05.tgz", None, 7_035_026_731),
}
MIN_FREE_BYTES = 20 * 1000**3
DOWNLOAD_DIR = "_download"
COMPLETE = ".complete"
CHUNK = 1 << 20
TIMEOUT_SEC = 60


class FetchError(Exception):
    pass


def sha256_of(path):
    digest = hashlib.sha256()
    with open(path, "rb") as f:
        while chunk := f.read(CHUNK):
            digest.update(chunk)
    return digest.hexdigest()


def safe_member(name):
    path = PurePosixPath(name)
    return bool(name) and not path.is_absolute() and ".." not in path.parts and "\\" not in name


def extract_tar(archive, dest):
    with tarfile.open(archive, "r:*") as tar:
        for member in tar:
            if not safe_member(member.name) or not (member.isfile() or member.isdir()):
                raise FetchError(f"{archive.name}: недопустимый элемент {member.name!r}")
            if member.isdir():
                (dest / member.name).mkdir(parents=True, exist_ok=True)
                continue
            target = dest / member.name
            target.parent.mkdir(parents=True, exist_ok=True)
            source = tar.extractfile(member)
            with open(target, "wb") as out:
                shutil.copyfileobj(source, out, CHUNK)


def extract_zip(archive, dest):
    with zipfile.ZipFile(archive) as z:
        for info in z.infolist():
            # Старшие биты внешних атрибутов: тип файла Unix; ссылки и особые файлы не берём.
            kind = (info.external_attr >> 16) & 0o170000
            if not safe_member(info.filename) or kind not in (0, 0o100000, 0o040000):
                raise FetchError(f"{archive.name}: недопустимый элемент {info.filename!r}")
            target = dest / info.filename
            if info.is_dir():
                target.mkdir(parents=True, exist_ok=True)
                continue
            target.parent.mkdir(parents=True, exist_ok=True)
            with z.open(info) as source, open(target, "wb") as out:
                shutil.copyfileobj(source, out, CHUNK)


def extract(archive, dest):
    if archive.name.endswith(".zip"):
        extract_zip(archive, dest)
    else:
        extract_tar(archive, dest)


def download(urls, target):
    part = target.with_name(target.name + ".part")
    errors = []
    for url in urls:
        try:
            with urllib.request.urlopen(url, timeout=TIMEOUT_SEC) as response, open(part, "wb") as out:
                shutil.copyfileobj(response, out, CHUNK)
            part.replace(target)
            return
        except OSError as e:
            errors.append(f"{url}: {e}")
    part.unlink(missing_ok=True)
    raise FetchError("не скачалось ни с одного зеркала: " + "; ".join(errors))


def check_sum(archive, pinned, record):
    actual = sha256_of(archive)
    expected = pinned or (record.read_text(encoding="utf-8").strip() if record.exists() else None)
    if expected is None:
        record.write_text(actual + "\n", encoding="utf-8")
        print(f"сумма первой загрузки {archive.name}: {actual}")
    elif actual != expected:
        archive.unlink()
        raise FetchError(f"{archive.name}: сумма {actual}, ждали {expected}")
    return actual


def check_space(root, archive_bytes):
    # Пока архив не удалён, на диске лежат и он, и распакованный набор.
    need = 2 * archive_bytes + MIN_FREE_BYTES
    free = shutil.disk_usage(root).free
    if free < need:
        raise FetchError(f"мало места: свободно {free / 1000**3:.1f} ГБ, нужно {need / 1000**3:.1f} ГБ")


def fetch(root, name, mirrors=MIRRORS):
    rel, pinned, archive_bytes = DATASETS[name]
    dest = root / name
    if (dest / COMPLETE).exists():
        print(f"есть: {name}")
        return dest
    cache = root / DOWNLOAD_DIR
    cache.mkdir(parents=True, exist_ok=True)
    archive = cache / PurePosixPath(rel).name
    check_space(root, archive_bytes)
    if not archive.exists():
        print(f"скачиваю: {name}")
        download([f"{m}/{rel}" for m in mirrors], archive)
    actual = check_sum(archive, pinned, cache / (archive.name + ".sha256"))
    # Недораспакованная папка от прошлого сбоя не должна смешаться с новой.
    shutil.rmtree(dest, ignore_errors=True)
    dest.mkdir(parents=True)
    print(f"распаковываю: {name}")
    extract(archive, dest)
    (dest / COMPLETE).write_text(actual + "\n", encoding="utf-8")
    archive.unlink()
    return dest


def main(argv):
    if not argv or any(name not in DATASETS for name in argv[1:]):
        print(__doc__, file=sys.stderr)
        return 2
    root = Path(argv[0])
    try:
        for name in argv[1:] or list(DATASETS):
            fetch(root, name)
    except (FetchError, OSError, tarfile.TarError, zipfile.BadZipFile) as e:
        print(f"ошибка: {e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
