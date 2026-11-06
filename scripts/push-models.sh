#!/usr/bin/env bash
# Закидывает модели из models/ в папку приложения на устройстве (режим отладки).
# Переустанавливать приложение только через `adb install -r`: удаление стирает
# эту папку вместе с моделями.
set -euo pipefail

APP_ID="dev.demo.nixyon"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODELS="${ROOT}/models"
REMOTE="/sdcard/Android/data/${APP_ID}/files/models"

if [[ ! -f "${MODELS}/models.json" ]]; then
  echo "ошибка: нет ${MODELS}/models.json, сначала запустите scripts/fetch-models.sh" >&2
  exit 1
fi

# Папки может не быть, пока приложение ни разу не запускалось.
adb shell mkdir -p "${REMOTE}"

# Файлы берутся из манифеста, чтобы не забыть ни одного и не закинуть лишнего.
files=()
while IFS= read -r line; do
  files+=("${line}")
done < <(grep -o '"path": "[^"]*"' "${MODELS}/models.json" | cut -d'"' -f4)

for file in "${files[@]}" example.wav; do
  echo "закидываю: ${file}"
  adb push "${MODELS}/${file}" "${REMOTE}/${file}"
done

# Манифест закидывается последним: пока он старый, приложение не примет недокачанный набор.
adb push "${MODELS}/models.json" "${REMOTE}/models.json"
echo "готово: ${REMOTE}"
