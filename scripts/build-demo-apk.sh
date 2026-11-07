#!/usr/bin/env bash
# Демо-сборка APK с моделями внутри: release, подпись отладочным ключом. Модели распознавания
# берутся из models/ строго по models.json, языковая модель из models/llm/ по llm.json; каждый
# файл сверяется по размеру и SHA-256.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODELS="${ROOT}/models"
STAGE="${ROOT}/nixyon-app/src-tauri/gen/android/app/build/nixyon-demo-assets"

# Исходники llama.cpp и языковая модель нужны только этой сборке, поэтому скрипт докачивает их
# сам: так сборка работает и там, где перед ней запускают только fetch-sherpa.sh и
# fetch-models.sh (хаб сборок). Скрипты сверяют SHA-256 и повторно не качают.
"${ROOT}/scripts/fetch-llama-cpp.sh"
"${ROOT}/scripts/fetch-llm.sh"

[[ -f "${MODELS}/models.json" ]] || { echo "ошибка: нет ${MODELS}/models.json, сначала scripts/fetch-models.sh" >&2; exit 1; }
[[ -f "${MODELS}/llm/llm.json" ]] || { echo "ошибка: нет ${MODELS}/llm/llm.json, сначала scripts/fetch-llm.sh" >&2; exit 1; }

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

rm -rf "${STAGE}"
mkdir -p "${STAGE}/models"
# Фильтр музыки (tagging_*) на телефоне не используется, поэтому в APK он не кладётся и
# из манифеста внутри APK исключается вместе с файлами. Обе роли необязательны (ModelRole.kt).
SKIP_ROLES="tagging_model tagging_labels"
python3 -I -c '
import json, sys
skip = set(sys.argv[3].split())
manifest = json.load(open(sys.argv[1]))
manifest["files"] = {role: f for role, f in manifest["files"].items() if role not in skip}
with open(sys.argv[2], "w") as out:
    json.dump(manifest, out, ensure_ascii=False, indent=2)
    out.write("\n")
for f in manifest["files"].values():
    print(f["path"], f["size"], f["sha256"])
' "${MODELS}/models.json" "${STAGE}/models/models.json" "${SKIP_ROLES}" >"${STAGE}/models/.entries"
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
mkdir -p "${STAGE}/llm"
cp "${MODELS}/llm/llm.json" "${STAGE}/llm/llm.json"
read -r llm_path llm_size llm_sum < <(python3 -I -c '
import json, sys
f = json.load(open(sys.argv[1]))["model"]
print(f["path"], f["size"], f["sha256"])
' "${MODELS}/llm/llm.json")
llm_file="${MODELS}/llm/${llm_path}"
[[ -f "${llm_file}" ]] || { echo "ошибка: нет ${llm_file}" >&2; exit 1; }
[[ "$(wc -c < "${llm_file}" | tr -d ' ')" == "${llm_size}" ]] || { echo "ошибка: размер ${llm_path} не совпал с llm.json" >&2; exit 1; }
[[ "$(sha256 "${llm_file}")" == "${llm_sum}" ]] || { echo "ошибка: SHA-256 ${llm_path} не совпал с llm.json" >&2; exit 1; }
ln "${llm_file}" "${STAGE}/llm/${llm_path}" 2>/dev/null || cp "${llm_file}" "${STAGE}/llm/${llm_path}"
echo "языковая модель: ${llm_path}"

cd "${ROOT}/nixyon-app"
ORG_GRADLE_PROJECT_nixyonModelsAssets="${STAGE}" \
  cargo tauri android build --apk --target aarch64 --ci

apk="${ROOT}/nixyon-app/src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk"
[[ -f "${apk}" ]] || { echo "ошибка: APK не найден: ${apk}" >&2; exit 1; }

# Gradle держит две свои копии ресурсов (по 2,8 ГБ с языковой моделью). После сборки они не
# нужны, а место нужно под выравнивание; при следующей сборке Gradle создаст их заново.
rm -rf "${ROOT}/nixyon-app/src-tauri/gen/android/app/build/intermediates/assets/universalRelease" \
  "${ROOT}/nixyon-app/src-tauri/gen/android/app/build/intermediates/compressed_assets/universalRelease"

# llama.cpp отображает модель прямо из файла APK, для этого её веса должны лежать на смещении,
# кратном 32 байтам. Gradle выравнивает несжатые файлы только по 4 байтам, поэтому APK
# выравнивается заново (64 байта для файлов, страницы 16 КБ для библиотек) и подписывается
# тем же отладочным ключом: выравнивание меняет байты, старая подпись перестаёт сходиться.
# Выравнивание проверяется и после подписи.
[[ -n "${ANDROID_HOME:-}" ]] || { echo "ошибка: не задан ANDROID_HOME" >&2; exit 1; }
build_tools="$(ls -d "${ANDROID_HOME}"/build-tools/* | sort -V | tail -1)"
"${build_tools}/zipalign" -f -P 16 64 "${apk}" "${apk}.aligned"
mv "${apk}.aligned" "${apk}"
"${build_tools}/zipalign" -c -P 16 64 "${apk}" || { echo "ошибка: APK не выровнен" >&2; exit 1; }
# --alignment-preserved: без него apksigner сам выравнивает файлы обратно по 4 байтам.
"${build_tools}/apksigner" sign --alignment-preserved true --ks "${KEYSTORE}" --ks-pass pass:android \
  --key-pass pass:android --ks-key-alias androiddebugkey "${apk}"
"${build_tools}/zipalign" -c -P 16 64 "${apk}" || { echo "ошибка: подпись сбила выравнивание" >&2; exit 1; }
"${build_tools}/apksigner" verify "${apk}" || { echo "ошибка: подпись APK не сошлась" >&2; exit 1; }
echo "демо APK: ${apk}"
echo "размер: $(( $(wc -c < "${apk}") / 1024 / 1024 )) МБ"
