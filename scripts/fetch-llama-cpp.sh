#!/usr/bin/env bash
# Скачивает исходники llama.cpp в third_party/llama.cpp/ и сверяет SHA-256 архива.
# Из них Gradle собирает библиотеку языковой модели для плагина Android.
# Если архив уже лежит и сумма совпадает, повторно не скачивает.
#
# Сумма снята при скачивании архива тега b12005 с GitHub: для архива
# исходников релиз своей суммы не публикует.
#
#   scripts/fetch-llama-cpp.sh            только llama.cpp
#   scripts/fetch-llama-cpp.sh --vulkan   ещё заголовки Vulkan (с vulkan.hpp) и SPIRV для
#                                         сборки с видеокартой (свойство Gradle nixyonLlmVulkan)
#
# Заголовки клонируются git по тегу и сверяются по коммиту тега.
set -euo pipefail

with_vulkan=0
case "${1:-}" in
  "") ;;
  --vulkan) with_vulkan=1 ;;
  *)
    echo "использование: scripts/fetch-llama-cpp.sh [--vulkan]" >&2
    exit 2
    ;;
esac

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

[[ "${with_vulkan}" -eq 1 ]] || exit 0

# Тег один для обоих, как в build.gradle.kts плагина.
VULKAN_TAG="vulkan-sdk-1.4.363.0"
VULKAN_DIR="${DEST}/${VULKAN_TAG}"

# Признак .ready пишется после сверки коммита: прерванное клонирование повторится.
clone_checked() {
  local name="$1" url="$2" commit="$3"
  local dir="${VULKAN_DIR}/${name}"
  if [[ -f "${dir}/.ready" ]]; then
    echo "есть: ${name} ${VULKAN_TAG}"
    return
  fi
  echo "клонирую: ${name} ${VULKAN_TAG}"
  rm -rf "${dir}"
  git -c advice.detachedHead=false clone --quiet --depth 1 --branch "${VULKAN_TAG}" "${url}" "${dir}"
  local actual
  actual="$(git -C "${dir}" rev-parse HEAD)"
  if [[ "${actual}" != "${commit}" ]]; then
    rm -rf "${dir}"
    echo "ошибка: ${name} ${VULKAN_TAG} на коммите ${actual}, ожидался ${commit}" >&2
    exit 1
  fi
  touch "${dir}/.ready"
}

mkdir -p "${VULKAN_DIR}"
clone_checked Vulkan-Headers https://github.com/KhronosGroup/Vulkan-Headers.git 6802bb4733b63ed5efd3adb308a6c885ef180ea1
clone_checked SPIRV-Headers https://github.com/KhronosGroup/SPIRV-Headers.git 496543121ce6419f23d6fa5d7194ba66c36212d2
echo "готово: ${VULKAN_DIR}"
