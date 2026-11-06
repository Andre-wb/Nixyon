# Инструменты для проверок ядра на Linux: fmt, clippy, тесты, утилиты transcribe и
# make_test_dialogs, скрипты скачивания. Исходники монтируются при запуске (compose.yaml),
# в образе только инструменты, поэтому правки кода не требуют пересборки образа.
FROM rust:1.95.0-slim-trixie@sha256:e14e87345b4d5964ddcc3491d27ee046a0f23820f340c3c1e24da6880141f7c0

# g++: статическая sherpa-onnx линкуется с libstdc++.
# curl, bzip2: скрипты скачивания моделей и записей.
# python3-minimal: словарь BPE для горячих слов в fetch-models.sh.
RUN apt-get update \
    && apt-get install -y --no-install-recommends g++ curl ca-certificates bzip2 python3-minimal \
    && rm -rf /var/lib/apt/lists/*

RUN rustup component add rustfmt clippy

# Пользователь с тем же uid, что у владельца репозитория на хосте: файлы,
# созданные в смонтированной папке, остаются его.
ARG UID=1000
ARG GID=1000
# На Mac GID обычно 20 (staff), в Debian он уже занят: тогда берётся существующая группа.
RUN (getent group "${GID}" > /dev/null || groupadd --gid "${GID}" builder) \
    && useradd --uid "${UID}" --gid "${GID}" --create-home builder \
    && mkdir -p /cache/cargo /cache/target \
    && chown -R "${UID}:${GID}" /cache

# Реестр cargo и каталог сборки живут в томах: Linux-сборка не смешивается с
# сборкой на Mac в target/ репозитория.
ENV CARGO_HOME=/cache/cargo \
    CARGO_TARGET_DIR=/cache/target

USER builder
WORKDIR /work
