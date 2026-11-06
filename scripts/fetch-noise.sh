#!/usr/bin/env bash
# Скачивает шум помещений из набора DEMAND (Zenodo 1227121, CC BY 4.0) и собирает
# зашумлённые копии ru2, ru4, ru6 (ОСШ 20, 10, 5 дБ) в models/dialogs/ для замера
# шумоподавления. Нужны диалоги из scripts/fetch-test-audio.sh.
# Берутся офис, столовая и ресторан, 16 кГц; из архива нужен только первый канал.
# Скачанное хранится в models/_download/demand/ и повторно не качается.
#
# Суммы MD5 архивов взяты из записи Zenodo (2026-10-30).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CACHE="${ROOT}/models/_download/demand"
URL="https://zenodo.org/records/1227121/files"

# место и MD5 архива
PLACES=(
  "OOFFICE 7b61cc2d182d5a654cb9c3101ddd4041"
  "PCAFETER 99927d148128254141a9417d051510bb"
  "PRESTO b98d2e6854eeebb397f29a8ad7457092"
)

md5_of() {
  if command -v md5sum >/dev/null 2>&1; then
    md5sum "$1" | cut -d' ' -f1
  else
    md5 -q "$1"
  fi
}

mkdir -p "${CACHE}"

for entry in "${PLACES[@]}"; do
  place="${entry%% *}"
  expected="${entry##* }"
  archive="${CACHE}/${place}_16k.zip"

  if [[ -f "${archive}" ]] && [[ "$(md5_of "${archive}")" == "${expected}" ]]; then
    echo "есть: ${place}_16k.zip"
  else
    echo "скачиваю: ${place}_16k.zip"
    rm -f "${archive}.part"
    curl -fSL --retry 3 -o "${archive}.part" "${URL}/${place}_16k.zip?download=1"
    actual="$(md5_of "${archive}.part")"
    if [[ "${actual}" != "${expected}" ]]; then
      rm -f "${archive}.part"
      echo "ошибка: сумма ${place}_16k.zip не совпала (ожидалась ${expected}, получена ${actual})" >&2
      exit 1
    fi
    mv "${archive}.part" "${archive}"
  fi
  if [[ ! -f "${CACHE}/${place}/ch01.wav" ]]; then
    unzip -q -o "${archive}" "${place}/ch01.wav" -d "${CACHE}"
  fi
done

cargo run --quiet --release -p nixyon-sherpa --bin make_noisy_dialogs -- \
  "${ROOT}/models/dialogs" "${CACHE}"
