#!/usr/bin/env bash
# Закидывает языковую модель из models/llm/ в папку приложения на устройстве. Модели
# распознавания при этом не трогаются: они остаются в APK или в своей папке.
# Переустанавливать приложение только через `adb install -r`: удаление стирает эту папку.
set -euo pipefail

APP_ID="dev.demo.nixyon"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOURCE="${ROOT}/models/llm"
REMOTE="/sdcard/Android/data/${APP_ID}/files/llm"

if [[ ! -f "${SOURCE}/llm.json" ]]; then
  echo "ошибка: нет ${SOURCE}/llm.json, сначала запустите scripts/fetch-llm.sh" >&2
  exit 1
fi
file="$(grep -o '"path": "[^"]*"' "${SOURCE}/llm.json" | cut -d'"' -f4)"

adb shell mkdir -p "${REMOTE}"
# Пока манифеста нет, приложение не видит модель и не возьмёт недокачанный файл.
adb shell rm -f "${REMOTE}/llm.json"
echo "закидываю: ${file} (2,3 ГБ)"
adb push "${SOURCE}/${file}" "${REMOTE}/${file}"
adb push "${SOURCE}/llm.json" "${REMOTE}/llm.json"
echo "готово: ${REMOTE}"
