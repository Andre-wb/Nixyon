#!/usr/bin/env bash
# Замер модели audEERING на устройстве Android или в эмуляторе (arm64): собирает
# age_gender_bench под aarch64-linux-android с ONNX Runtime из aar sherpa-onnx (той же
# библиотекой, что в APK), кладёт её, модель и 30 с речи в /data/local/tmp и запускает.
# scripts/bench-age-gender-android.sh [запись.wav] [секунд] [потоков] [повторов] [bytes|file]
# Нужны: models/ort/age-gender.onnx (scripts/fetch-models.sh),
# third_party/sherpa-onnx/ort-lib-android (scripts/fetch-sherpa.sh), Android NDK, ffmpeg, adb.
# Устройство выбирается переменной ANDROID_SERIAL, если подключено несколько.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
AUDIO="${1:-${ROOT}/models/dialogs/ru2.wav}"
SECONDS_OF_SPEECH="${2:-30}"
THREADS="${3:-4}"
REPEATS="${4:-3}"
LOAD="${5:-bytes}"
MODEL="${ROOT}/models/ort/age-gender.onnx"
ORT_LIB="${ROOT}/third_party/sherpa-onnx/ort-lib-android"
TARGET="aarch64-linux-android"
# Как minSdk приложения.
API=24
REMOTE="/data/local/tmp/nixyon-bench"

die() {
  echo "ошибка: $*" >&2
  exit 1
}

[[ -f "${MODEL}" ]] || die "нет ${MODEL}: сначала scripts/fetch-models.sh"
[[ -f "${ORT_LIB}/libonnxruntime.so" ]] || die "нет ${ORT_LIB}/libonnxruntime.so: сначала scripts/fetch-sherpa.sh"
[[ -f "${AUDIO}" ]] || die "нет записи ${AUDIO}"
command -v ffmpeg >/dev/null || die "нужен ffmpeg"
command -v adb >/dev/null || die "нужен adb"

ndk="${ANDROID_NDK_HOME:-}"
if [[ -z "${ndk}" ]]; then
  sdk="${ANDROID_HOME:-${HOME}/Library/Android/sdk}"
  ndk="$(find "${sdk}/ndk" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sort | tail -n 1)"
fi
[[ -n "${ndk}" && -d "${ndk}" ]] || die "не найден Android NDK: задайте ANDROID_NDK_HOME"
linker="$(find "${ndk}/toolchains/llvm/prebuilt" -name "${TARGET}${API}-clang" -type f | head -n 1)"
[[ -n "${linker}" ]] || die "в ${ndk} нет ${TARGET}${API}-clang"

abi="$(adb shell getprop ro.product.cpu.abi | tr -d '\r')"
[[ "${abi}" == "arm64-v8a" ]] || die "устройство ${abi}, утилита собирается под arm64-v8a"

# Своя папка сборки: под Android ort линкуется с другой библиотекой, чем на хосте.
export CARGO_TARGET_DIR="${ROOT}/target/android-bench"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="${linker}"
export ORT_LIB_LOCATION="${ORT_LIB}"
export ORT_PREFER_DYNAMIC_LINK=1
cargo build --release --locked -p nixyon-ort --bin age_gender_bench --target "${TARGET}" \
  --manifest-path "${ROOT}/Cargo.toml"
bin="${CARGO_TARGET_DIR}/${TARGET}/release/age_gender_bench"

work="$(mktemp -d)"
trap 'rm -rf "${work}"' EXIT
ffmpeg -v error -i "${AUDIO}" -t "${SECONDS_OF_SPEECH}" -ac 1 -ar 16000 -f f32le "${work}/speech.f32"

adb shell mkdir -p "${REMOTE}"
adb push "${bin}" "${ORT_LIB}/libonnxruntime.so" "${work}/speech.f32" "${REMOTE}/" >/dev/null
# Модель большая: кладётся заново, только если размер на устройстве другой.
local_size="$(wc -c <"${MODEL}" | tr -d ' ')"
remote_size="$(adb shell "stat -c %s ${REMOTE}/age-gender.onnx 2>/dev/null || echo 0" | tr -d '\r')"
if [[ "${local_size}" != "${remote_size}" ]]; then
  adb push "${MODEL}" "${REMOTE}/age-gender.onnx" >/dev/null
fi

echo "устройство: $(adb shell getprop ro.product.model | tr -d '\r'), Android $(adb shell getprop ro.build.version.release | tr -d '\r'), ядер $(adb shell nproc | tr -d '\r')"
adb shell "cd ${REMOTE} && chmod 755 age_gender_bench && LD_LIBRARY_PATH=${REMOTE} ./age_gender_bench age-gender.onnx speech.f32 ${SECONDS_OF_SPEECH} ${THREADS} ${REPEATS} ${LOAD}"
