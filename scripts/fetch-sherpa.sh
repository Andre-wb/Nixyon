#!/usr/bin/env bash
# Скачивает бинарники sherpa-onnx в third_party/sherpa-onnx/ и сверяет SHA-256.
# Если файл уже лежит и сумма совпадает, повторно не скачивает, поэтому после
# первого запуска сборка идёт без сети.
#
# Суммы сняты с релиза v2.0.1 на GitHub: релиз не публикует свои контрольные суммы,
# поэтому это закрепление того, что было скачано. Сумма архива для Intel Mac совпала с
# digest, который GitHub показывает для файла.
set -euo pipefail

VERSION="2.0.1"
BASE_URL="https://github.com/k2-fsa/sherpa-onnx/releases/download/v${VERSION}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="${ROOT}/third_party/sherpa-onnx"

# имя файла и его SHA-256
FILES=(
  "sherpa-onnx-${VERSION}.aar a8d09d1d32f3eb8520482c6d3d43fae674681c5b050385967c70a3c2c9bf0ee7"
  "sherpa-onnx-v${VERSION}-osx-arm64-static-lib.tar.bz2 d7a9b14f337958e7373c8c650f046df33d210bbaff2f33cb8322dda663ee5e1c"
  # Для сборки на Intel Mac.
  "sherpa-onnx-v${VERSION}-osx-x64-static-lib.tar.bz2 6a3cb3c14104d3a02e7b2a6cd5621f590b14892cce7d0bc0935f7655ef518938"
  # Для сборки и тестов в Docker (linux/arm64 на Mac, linux/amd64 на Intel и Linux).
  "sherpa-onnx-v${VERSION}-linux-aarch64-static-lib.tar.bz2 b6c6090e40fd18d6812f04fe21260602cd1223d7b5e83ad39dab87ad7c1c031d"
  "sherpa-onnx-v${VERSION}-linux-x64-static-lib.tar.bz2 55cfadf65ba50d0afc1a802b7a337942819e51c94a5b5a384def1978f7c70871"
)

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d' ' -f1
  else
    shasum -a 256 "$1" | cut -d' ' -f1
  fi
}

mkdir -p "${DEST}"

for entry in "${FILES[@]}"; do
  name="${entry%% *}"
  expected="${entry##* }"
  target="${DEST}/${name}"

  if [[ -f "${target}" ]] && [[ "$(sha256_of "${target}")" == "${expected}" ]]; then
    echo "есть: ${name}"
    continue
  fi

  echo "скачиваю: ${name}"
  tmp="${target}.part"
  rm -f "${tmp}"
  curl -fSL --retry 3 -o "${tmp}" "${BASE_URL}/${name}"

  actual="$(sha256_of "${tmp}")"
  if [[ "${actual}" != "${expected}" ]]; then
    rm -f "${tmp}"
    echo "ошибка: сумма ${name} не совпала (ожидалась ${expected}, получена ${actual})" >&2
    exit 1
  fi
  mv "${tmp}" "${target}"
  echo "готово: ${name}"
done

# Крейт ort работает на том же ONNX Runtime, что и sherpa-onnx: из архива своей платформы
# достаётся libonnxruntime.a, на эту папку указывает ORT_LIB_LOCATION. Второй копии рантайма
# в программе нет. В контейнере ORT_LIB_LOCATION задаёт compose: своя папка, не общая с хостом.
case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) platform="osx-arm64" ;;
  Darwin-x86_64) platform="osx-x64" ;;
  Linux-x86_64) platform="linux-x64" ;;
  Linux-aarch64 | Linux-arm64) platform="linux-aarch64" ;;
  *)
    echo "ошибка: для $(uname -s) $(uname -m) нет статических библиотек sherpa-onnx" >&2
    exit 1
    ;;
esac
stem="sherpa-onnx-v${VERSION}-${platform}-static-lib"
archive="${DEST}/${stem}.tar.bz2"
ort_lib="${ORT_LIB_LOCATION:-${DEST}/ort-lib}"
# По сумме архива видно, из какого архива уже распакована библиотека.
marker="${ort_lib}/source.sha256"
archive_sum="$(sha256_of "${archive}")"
if [[ -f "${ort_lib}/libonnxruntime.a" && -f "${marker}" && "$(cat "${marker}")" == "${archive_sum}" ]]; then
  echo "есть: ${ort_lib}/libonnxruntime.a"
else
  mkdir -p "${ort_lib}"
  unpack="$(mktemp -d)"
  tar -xjf "${archive}" -C "${unpack}" "${stem}/lib/libonnxruntime.a"
  mv "${unpack}/${stem}/lib/libonnxruntime.a" "${ort_lib}/libonnxruntime.a"
  rm -rf "${unpack}"
  echo "${archive_sum}" >"${marker}"
  echo "готово: ${ort_lib}/libonnxruntime.a"
fi

# Приложение на Android считает модель голоса крейтом ort и линкуется с libonnxruntime.so из
# aar: та же библиотека попадает в APK вместе с sherpa-onnx. Папку указывает
# nixyon-app/.cargo/config.toml. Библиотека одна для любого хоста сборки.
aar="${DEST}/sherpa-onnx-${VERSION}.aar"
android_lib="${DEST}/ort-lib-android"
android_marker="${android_lib}/source.sha256"
aar_sum="$(sha256_of "${aar}")"
if [[ -f "${android_lib}/libonnxruntime.so" && -f "${android_marker}" && "$(cat "${android_marker}")" == "${aar_sum}" ]]; then
  echo "есть: ${android_lib}/libonnxruntime.so"
else
  mkdir -p "${android_lib}"
  python3 -I -c '
import shutil, sys, zipfile
with zipfile.ZipFile(sys.argv[1]) as aar, aar.open("jni/arm64-v8a/libonnxruntime.so") as src, open(sys.argv[2], "wb") as dst:
    shutil.copyfileobj(src, dst)
' "${aar}" "${android_lib}/libonnxruntime.so.part"
  mv "${android_lib}/libonnxruntime.so.part" "${android_lib}/libonnxruntime.so"
  echo "${aar_sum}" >"${android_marker}"
  echo "готово: ${android_lib}/libonnxruntime.so"
fi
