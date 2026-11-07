#!/usr/bin/env bash
# Скачивает языковую модель для протокола встречи, сверяет SHA-256 и раскладывает её в
# models/llm/ вместе с манифестом llm.json. Модель не входит в APK и в models.json:
# телефон качает её с зеркала по каталогу (scripts/build-model-catalog.py), при разработке
# её кладёт scripts/push-llm.sh. Скачанное хранится в models/_download/llm/.
#
#   scripts/fetch-llm.sh                  скачать модель и записать llm.json
#   scripts/fetch-llm.sh --manifest-only  только llm.json, без модели: его сверяет с
#                                         каталогом тест скриптов в CI
#
# Qwen3.5-4B Q3_K_M в сборке unsloth (Apache-2.0), ревизия закреплена. Квант выбран по
# размеру: Q4_K_M весит 2,7 ГБ, Q3_K_M 2,3 ГБ. На тестовых встречах
# (scripts/llm-meeting-eval.sh) Q3_K_M находит те же задачи и не попадает в ловушки, как и
# Q4_K_M, процессу нужно на 1,1-1,5 ГБ меньше памяти, ответ дольше на 2-5 с. Решения
# он чаще повторяет задачами. Сумма совпала с той, что Hugging Face указывает для файла в
# этой ревизии.
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

write_manifest() {
  printf '{\n  "version": "%s",\n  "model": { "path": "%s", "size": %s, "sha256": "%s" }\n}\n' \
    "${VERSION}" "${FILE}" "${SIZE}" "${SHA256}" >"${DEST}/llm.json"
  echo "готово: ${DEST}/llm.json"
}

case "${1:-}" in
  "") ;;
  --manifest-only)
    mkdir -p "${DEST}"
    write_manifest
    exit 0
    ;;
  *)
    echo "использование: scripts/fetch-llm.sh [--manifest-only]" >&2
    exit 2
    ;;
esac

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
write_manifest
