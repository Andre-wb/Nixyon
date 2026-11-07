#!/usr/bin/env bash
# Демо-сборки APK: release, подпись отладочным ключом. Две версии:
#   полная: все модели внутри APK, работает сразу и без сети;
#   лёгкая: только код и каталог моделей, при первом запуске телефон качает модели с
#   зеркала.
#
#   scripts/build-demo-apk.sh          обе версии
#   scripts/build-demo-apk.sh --full   только полная
#   scripts/build-demo-apk.sh --lite   только лёгкая
#
# Полная лежит там же, где Gradle кладёт release-APK (app-universal-release.apk), лёгкая
# рядом с папкой outputs/apk под именем nixyon-lite.apk: вторая сборка не затирает первую.
# Модели распознавания берутся из models/ строго по models.json, языковая модель из
# models/llm/ по llm.json; каждый файл сверяется по размеру и SHA-256.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODELS="${ROOT}/models"
OUTPUTS="${ROOT}/nixyon-app/src-tauri/gen/android/app/build/outputs"
STAGE="${ROOT}/nixyon-app/src-tauri/gen/android/app/build/nixyon-demo-assets"
GRADLE_APK="${OUTPUTS}/apk/universal/release/app-universal-release.apk"
LITE_APK="${OUTPUTS}/nixyon-lite.apk"

build_full=1
build_lite=1
case "${1:-}" in
  "") ;;
  --full) build_lite=0 ;;
  --lite) build_full=0 ;;
  *)
    echo "использование: scripts/build-demo-apk.sh [--full | --lite]" >&2
    exit 2
    ;;
esac

# Исходники llama.cpp нужны сборке плагина, языковая модель только полной версии. Скрипты
# сверяют SHA-256 и повторно не качают.
"${ROOT}/scripts/fetch-llama-cpp.sh"
if [[ "${build_full}" -eq 1 ]]; then
  "${ROOT}/scripts/fetch-llm.sh"
  [[ -f "${MODELS}/models.json" ]] || { echo "ошибка: нет ${MODELS}/models.json, сначала scripts/fetch-models.sh" >&2; exit 1; }
  [[ -f "${MODELS}/llm/llm.json" ]] || { echo "ошибка: нет ${MODELS}/llm/llm.json, сначала scripts/fetch-llm.sh" >&2; exit 1; }
fi

[[ -n "${ANDROID_HOME:-}" ]] || { echo "ошибка: не задан ANDROID_HOME" >&2; exit 1; }
build_tools="$(ls -d "${ANDROID_HOME}"/build-tools/* | sort -V | tail -1)"

sha256() {
  if command -v sha256sum > /dev/null; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi
}

# Демо подписывается отладочным ключом из ~/.android. Если его нет (не стояла Android Studio),
# создаётся такой же, какой создаёт Android Studio: тот же псевдоним и пароли.
KEYSTORE_DIR="${HOME}/.android"
KEYSTORE="${KEYSTORE_DIR}/debug.keystore"
if [[ -d "${KEYSTORE}" ]]; then
  echo "ошибка: ${KEYSTORE} папка, а не ключ (её создаёт Docker, когда файла нет); удалите её и запустите снова" >&2
  exit 1
fi
if [[ ! -f "${KEYSTORE}" ]]; then
  mkdir -p "${KEYSTORE_DIR}"
  [[ -w "${KEYSTORE_DIR}" ]] || { echo "ошибка: нет записи в ${KEYSTORE_DIR}; на Linux создайте папку на хосте: mkdir -p ~/.android" >&2; exit 1; }
  command -v keytool > /dev/null || { echo "ошибка: нет ${KEYSTORE} и нет keytool, чтобы его создать" >&2; exit 1; }
  keytool -genkeypair -keystore "${KEYSTORE}" -storepass android -alias androiddebugkey -keypass android \
    -keyalg RSA -keysize 2048 -validity 10000 -dname "CN=Android Debug,O=Android,C=US" > /dev/null
  echo "создан отладочный ключ: ${KEYSTORE}"
fi

gradle_build() {
  rm -f "${GRADLE_APK}"
  (cd "${ROOT}/nixyon-app" && cargo tauri android build --apk --target aarch64 --ci)
  [[ -f "${GRADLE_APK}" ]] || { echo "ошибка: APK не найден: ${GRADLE_APK}" >&2; exit 1; }
}

report() {
  "${build_tools}/apksigner" verify "$2" || { echo "ошибка: подпись APK не сошлась" >&2; exit 1; }
  echo "$1: $2"
  echo "размер: $(( $(wc -c < "$2") / 1024 / 1024 )) МБ"
}

build_lite_apk() {
  rm -f "${LITE_APK}"
  ORG_GRADLE_PROJECT_nixyonDebugSigned=true gradle_build
  mv "${GRADLE_APK}" "${LITE_APK}"
  report "лёгкий APK" "${LITE_APK}"
}

# Модели кладутся в папку, которую Gradle добавляет к assets. Фильтр музыки (tagging_*) на
# телефоне не используется, поэтому в APK он не кладётся и из манифеста внутри APK
# исключается вместе с файлами.
stage_models() {
  rm -rf "${STAGE}"
  mkdir -p "${STAGE}/models" "${STAGE}/llm"
  python3 -I -c '
import json, sys
skip = {"tagging_model", "tagging_labels"}
manifest = json.load(open(sys.argv[1]))
manifest["files"] = {role: f for role, f in manifest["files"].items() if role not in skip}
with open(sys.argv[2], "w") as out:
    json.dump(manifest, out, ensure_ascii=False, indent=2)
    out.write("\n")
for f in manifest["files"].values():
    print(f["path"], f["size"], f["sha256"])
' "${MODELS}/models.json" "${STAGE}/models/models.json" >"${STAGE}/models/.entries"
  local path size sum file
  while read -r path size sum; do
    file="${MODELS}/${path}"
    [[ -f "${file}" ]] || { echo "ошибка: нет ${file}" >&2; exit 1; }
    [[ "$(wc -c < "${file}" | tr -d ' ')" == "${size}" ]] || { echo "ошибка: размер ${path} не совпал с models.json" >&2; exit 1; }
    [[ "$(sha256 "${file}")" == "${sum}" ]] || { echo "ошибка: SHA-256 ${path} не совпал с models.json" >&2; exit 1; }
    cp "${file}" "${STAGE}/models/${path}"
    echo "модель: ${path}"
  done <"${STAGE}/models/.entries"
  rm -f "${STAGE}/models/.entries"

  # Языковая модель (2,3 ГБ) ставится в папку жёсткой ссылкой, если можно: вторая копия на
  # диске не нужна. Gradle всё равно копирует её при сборке.
  cp "${MODELS}/llm/llm.json" "${STAGE}/llm/llm.json"
  read -r path size sum < <(python3 -I -c '
import json, sys
f = json.load(open(sys.argv[1]))["model"]
print(f["path"], f["size"], f["sha256"])
' "${MODELS}/llm/llm.json")
  file="${MODELS}/llm/${path}"
  [[ -f "${file}" ]] || { echo "ошибка: нет ${file}" >&2; exit 1; }
  [[ "$(wc -c < "${file}" | tr -d ' ')" == "${size}" ]] || { echo "ошибка: размер ${path} не совпал с llm.json" >&2; exit 1; }
  [[ "$(sha256 "${file}")" == "${sum}" ]] || { echo "ошибка: SHA-256 ${path} не совпал с llm.json" >&2; exit 1; }
  ln "${file}" "${STAGE}/llm/${path}" 2>/dev/null || cp "${file}" "${STAGE}/llm/${path}"
  echo "языковая модель: ${path}"
}

build_full_apk() {
  stage_models
  ORG_GRADLE_PROJECT_nixyonDebugSigned=true ORG_GRADLE_PROJECT_nixyonModelsAssets="${STAGE}" gradle_build

  # Gradle держит две свои копии ресурсов (по 2,8 ГБ с языковой моделью). После сборки они
  # не нужны, а место нужно под выравнивание; при следующей сборке Gradle создаст их заново.
  rm -rf "${ROOT}/nixyon-app/src-tauri/gen/android/app/build/intermediates/assets/universalRelease" \
    "${ROOT}/nixyon-app/src-tauri/gen/android/app/build/intermediates/compressed_assets/universalRelease" \
    "${STAGE}"

  # llama.cpp отображает модель прямо из файла APK, для этого её веса должны лежать на
  # смещении, кратном 32 байтам. Gradle выравнивает несжатые файлы только по 4 байтам,
  # поэтому APK выравнивается заново (64 байта для файлов, страницы 16 КБ для библиотек) и
  # подписывается тем же отладочным ключом: выравнивание меняет байты, старая подпись
  # перестаёт сходиться. Выравнивание проверяется и после подписи.
  "${build_tools}/zipalign" -f -P 16 64 "${GRADLE_APK}" "${GRADLE_APK}.aligned"
  mv "${GRADLE_APK}.aligned" "${GRADLE_APK}"
  "${build_tools}/zipalign" -c -P 16 64 "${GRADLE_APK}" || { echo "ошибка: APK не выровнен" >&2; exit 1; }
  # --alignment-preserved: без него apksigner сам выравнивает файлы обратно по 4 байтам.
  "${build_tools}/apksigner" sign --alignment-preserved true --ks "${KEYSTORE}" --ks-pass pass:android \
    --key-pass pass:android --ks-key-alias androiddebugkey "${GRADLE_APK}"
  "${build_tools}/zipalign" -c -P 16 64 "${GRADLE_APK}" || { echo "ошибка: подпись сбила выравнивание" >&2; exit 1; }
  report "полный APK" "${GRADLE_APK}"
}

# Лёгкая первой: полная последней оставляет свой APK там, где его ищет хаб.
if [[ "${build_lite}" -eq 1 ]]; then
  build_lite_apk
fi
if [[ "${build_full}" -eq 1 ]]; then
  build_full_apk
fi
