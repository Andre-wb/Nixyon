#!/usr/bin/env bash
# Демо-сборка APK: release, подпись отладочным ключом. Моделей в APK нет, только код и
# каталог моделей: при первом запуске телефон качает выбранные модели с зеркала.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Исходники llama.cpp нужны сборке плагина; скрипт сверяет SHA-256 и повторно не качает.
"${ROOT}/scripts/fetch-llama-cpp.sh"

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

cd "${ROOT}/nixyon-app"
ORG_GRADLE_PROJECT_nixyonDebugSigned=true \
  cargo tauri android build --apk --target aarch64 --ci

apk="${ROOT}/nixyon-app/src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk"
[[ -f "${apk}" ]] || { echo "ошибка: APK не найден: ${apk}" >&2; exit 1; }
[[ -n "${ANDROID_HOME:-}" ]] || { echo "ошибка: не задан ANDROID_HOME" >&2; exit 1; }
build_tools="$(ls -d "${ANDROID_HOME}"/build-tools/* | sort -V | tail -1)"
"${build_tools}/apksigner" verify "${apk}" || { echo "ошибка: подпись APK не сошлась" >&2; exit 1; }
echo "демо APK: ${apk}"
echo "размер: $(( $(wc -c < "${apk}") / 1024 / 1024 )) МБ"
