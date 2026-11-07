#!/usr/bin/env bash
# Скачивает языковую модель для протокола встречи, сверяет SHA-256 и раскладывает её в
# models/llm/ вместе с манифестом llm.json. Модель не входит в APK и в models.json:
# на телефон её кладёт scripts/push-llm.sh. Скачанное хранится в models/_download/llm/.
#
# Qwen3.5-4B Q3_K_M в сборке unsloth (Apache-2.0), ревизия закреплена. Квант выбран по
# размеру: Q4_K_M весил 2,7 ГБ и почти целиком заполнял APK, Q3_K_M даёт 2,3 ГБ при
# заметно меньшей разнице в качестве протокола встречи, чем переход на более мелкую
# модель. Сумма совпала с той, что Hugging Face указывает для файла в этой ревизии.
# Сначала файл берётся с зеркала на сервере Nixyon Hub (scripts/model-mirror.sh),
# затем с Hugging Face.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=scripts/model-mirror.sh
source "${ROOT}/scripts/model-mirror.sh"
CACHE="${ROOT}/models/_download/llm"
DEST="${ROOT}/models/llm"
REVISION="e87f176479d0855a907a41277aca2f8ee7a09523"
NAME="Qwen3.5-4B-Q3_K_M.gguf"
URL="https://huggingface.co/unsloth/Qwen3.5-4B-GGUF/resolve/${REVISION}/${NAME}"
SHA256="d6981ab4d77ba712b48ef69d69042d75b5e39b9dce5fb5a5b054fd08e06afb95"
SIZE=2293388448
# Меняется при смене модели: по нему приложение видит, что файл нужно проверить заново.
VERSION="qwen3.5-4b-q3_k_m-unsloth-${REVISION:0:8}"
FILE="qwen3.5-4b-q3_k_m.gguf"

mkdir -p "${CACHE}" "${DEST}"
cached="${CACHE}/${NAME}"
if [[ -f "${cached}" ]] && [[ "$(sha256_of "${cached}")" == "${SHA256}" ]]; then
  echo "есть: ${NAME}"
else
  echo "скачиваю: ${NAME} (2,3 ГБ)"
  model_download "${cached}.part" "${SHA256}" curl -fSL --retry 3 -o "${cached}.part" "${URL}" || exit 1
  mv "${cached}.part" "${cached}"
fi

# Жёсткая ссылка вместо копии: второй раз 2,3 ГБ на диске не нужны.
rm -f "${DEST}/${FILE}"
ln "${cached}" "${DEST}/${FILE}" 2>/dev/null || cp "${cached}" "${DEST}/${FILE}"
printf '{\n  "version": "%s",\n  "model": { "path": "%s", "size": %s, "sha256": "%s" }\n}\n' \
  "${VERSION}" "${FILE}" "${SIZE}" "${SHA256}" >"${DEST}/llm.json"
echo "готово: ${DEST}/llm.json"
