#!/usr/bin/env bash
# Кладёт скачанные модели в папку зеркала, которую раздаёт Nixyon Hub: каждый файл под
# своей SHA-256. Берутся файлы, которые скрипты загрузки уже сверили по сумме: архивы и
# модели в models/_download, языковая модель в models/_download/llm, набор ort в
# models/ort. Ещё кладутся готовые файлы из models/models.json и models/llm/llm.json:
# их по каталогу качает телефон, архивы он не распаковывает. Сумма такого файла должна
# совпасть с манифестом, иначе скрипт падает. Файлы, которых среди скачанных больше нет,
# из зеркала удаляются.
#
#   scripts/publish-models.sh <папка зеркала>
#
# Суммы запоминаются в <папка зеркала>/.sums по пути, размеру и времени изменения файла:
# без этого каждый запуск заново считал бы SHA-256 у нескольких гигабайт. Ошибка в .sums
# клиентам не вредит: скрипты загрузки сверяют сумму каждого файла с зеркала.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODELS="${ROOT}/models"
# shellcheck source=scripts/model-mirror.sh
source "${ROOT}/scripts/model-mirror.sh"

if [[ $# -ne 1 ]]; then
  echo "использование: scripts/publish-models.sh <папка зеркала>" >&2
  exit 2
fi
mirror="$1"
if [[ ! -d "${mirror}" ]]; then
  echo "нет папки зеркала ${mirror}" >&2
  exit 1
fi

mtime_of() {
  stat -c %Y "$1" 2>/dev/null || stat -f %m "$1"
}

sums="${mirror}/.sums"
old_sums=""
[[ -f "${sums}" ]] && old_sums="$(cat "${sums}")"
new_sums=""
keep=" "

# Сумма файла: из .sums, если путь, размер и время изменения те же, иначе считается.
sum_of() {
  local file="$1" size="$2" mtime="$3" line
  while IFS=' ' read -r -a line; do
    if [[ "${line[1]:-}" == "${size}" && "${line[2]:-}" == "${mtime}" && "${line[3]:-}" == "${file}" ]]; then
      printf '%s' "${line[0]}"
      return
    fi
  done <<<"${old_sums}"
  sha256_of "${file}"
}

# Сумма последнего выложенного файла: по ней готовые файлы сверяются с манифестом.
published_sha=""

publish() {
  local file="$1" size mtime sha
  size="$(size_of "${file}")"
  mtime="$(mtime_of "${file}")"
  sha="$(sum_of "${file}" "${size}" "${mtime}")"
  new_sums+="${sha} ${size} ${mtime} ${file}"$'\n'
  keep+="${sha} "
  published_sha="${sha}"
  if [[ -f "${mirror}/${sha}" && "$(size_of "${mirror}/${sha}")" == "${size}" ]]; then
    return
  fi
  # Через временное имя: хаб не отдаёт файл, пока он не скопирован целиком.
  cp "${file}" "${mirror}/.${sha}.tmp"
  mv "${mirror}/.${sha}.tmp" "${mirror}/${sha}"
  echo "в зеркале: ${file#"${MODELS}"/}"
}

shopt -s nullglob
for file in "${MODELS}"/_download/* "${MODELS}"/_download/llm/* "${MODELS}"/ort/*; do
  [[ -f "${file}" && ! -L "${file}" ]] || continue
  case "${file}" in
    *.part | */models.json) continue ;;
  esac
  publish "${file}"
done

# Готовые файлы для телефона: путь, размер и сумма из манифеста.
ready_files() {
  python3 -I -c '
import json, sys
from pathlib import Path
models = Path(sys.argv[1])
speech = models / "models.json"
if speech.is_file():
    for f in json.loads(speech.read_text())["files"].values():
        print(models / f["path"], f["sha256"])
llm = models / "llm" / "llm.json"
if llm.is_file():
    f = json.loads(llm.read_text())["model"]
    print(models / "llm" / f["path"], f["sha256"])
' "${MODELS}"
}

ready="$(ready_files)"
while read -r file expected; do
  [[ -n "${file}" ]] || continue
  if [[ ! -f "${file}" ]]; then
    echo "нет ${file#"${MODELS}"/} из манифеста" >&2
    exit 1
  fi
  publish "${file}"
  if [[ "${published_sha}" != "${expected}" ]]; then
    echo "сумма ${file#"${MODELS}"/} не совпала с манифестом" >&2
    exit 1
  fi
done <<<"${ready}"

for file in "${mirror}"/* "${mirror}"/.*.tmp; do
  [[ -f "${file}" ]] || continue
  name="${file##*/}"
  case "${keep}" in
    *" ${name} "*) ;;
    *) rm -f "${file}" ;;
  esac
done
printf '%s' "${new_sums}" >"${sums}.tmp"
mv "${sums}.tmp" "${sums}"
echo "зеркало: ${mirror}"
