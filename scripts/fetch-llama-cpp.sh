#!/usr/bin/env bash
# Скачивает исходники llama.cpp в third_party/llama.cpp/ и сверяет SHA-256 архива.
# Из них Gradle собирает библиотеку языковой модели для плагина Android.
# Если архив уже лежит и сумма совпадает, повторно не скачивает.
#
# Сумма снята 2026-10-30 при скачивании архива тега b12005 с GitHub: для архива
# исходников релиз своей суммы не публикует.
set -euo pipefail

TAG="b12005"
SHA256="b3afe02970f0b0055654eb6c10152674203c62b9218179b540c4cd065d8c9e99"
URL="https://github.com/ggml-org/llama.cpp/archive/refs/tags/${TAG}.tar.gz"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="${ROOT}/third_party/llama.cpp"
ARCHIVE="${DEST}/llama.cpp-${TAG}.tar.gz"
SOURCE="${DEST}/llama.cpp-${TAG}"

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d' ' -f1
  else
    shasum -a 256 "$1" | cut -d' ' -f1
  fi
}

mkdir -p "${DEST}"

if [[ -f "${ARCHIVE}" ]] && [[ "$(sha256_of "${ARCHIVE}")" == "${SHA256}" ]]; then
  echo "есть: ${ARCHIVE##*/}"
else
  echo "скачиваю: ${ARCHIVE##*/}"
  rm -f "${ARCHIVE}.part"
  curl -fSL --retry 3 -o "${ARCHIVE}.part" "${URL}"
  actual="$(sha256_of "${ARCHIVE}.part")"
  if [[ "${actual}" != "${SHA256}" ]]; then
    rm -f "${ARCHIVE}.part"
    echo "ошибка: сумма ${ARCHIVE##*/} не совпала (ожидалась ${SHA256}, получена ${actual})" >&2
    exit 1
  fi
  mv "${ARCHIVE}.part" "${ARCHIVE}"
  # Архив новый: распакованное от прежнего не годится.
  rm -rf "${SOURCE}"
fi

# Признак полной распаковки пишется последним: прерванная распаковка повторится.
if [[ ! -f "${SOURCE}/.unpacked" ]]; then
  rm -rf "${SOURCE}"
  tar -xzf "${ARCHIVE}" -C "${DEST}"
  touch "${SOURCE}/.unpacked"
fi
echo "готово: ${SOURCE}"
