#!/usr/bin/env bash
# Качество распознавания на наборах записей: собирает quality_eval, прогоняет через весь
# конвейер приложения выбранные наборы и кладёт отчёт (report.md, summary.json, clips/, audio/)
# в папку целиком, когда он готов: пока идёт прогон, прежний отчёт остаётся доступен.
# Копия итогов прогона с коммитом ложится в <папка отчёта>-history (последние 100).
#
#   scripts/quality.sh [--exclude <файл>] <корень наборов> <папка отчёта> <набор>=<минуты|all> [...]
#
# --exclude: JSON {"<набор>": ["<id клипа>", ...]} с клипами, у которых плохой эталон;
# они не берутся в проверку.
#
# Набор: <корень>/<набор>/dataset.json и clips.jsonl (scripts/build-dataset-index.py).
# Модели берутся из models/ (scripts/fetch-models.sh).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

usage="использование: scripts/quality.sh [--exclude <файл>] <корень наборов> <папка отчёта> <набор>=<минуты|all> [...]"
exclude=""
if [[ "${1:-}" == "--exclude" ]]; then
  if [[ $# -lt 2 ]]; then
    echo "${usage}" >&2
    exit 2
  fi
  exclude="$2"
  shift 2
fi
if [[ $# -lt 3 ]]; then
  echo "${usage}" >&2
  exit 2
fi
datasets="$1"
out="$2"
shift 2
if [[ ! -d "${datasets}" ]]; then
  echo "нет папки наборов ${datasets}" >&2
  exit 1
fi
if [[ ! -f "${ROOT}/models/models.json" ]]; then
  echo "нет моделей в ${ROOT}/models: сначала scripts/fetch-models.sh" >&2
  exit 1
fi

takes=()
for item in "$@"; do
  takes+=(--take "${item}")
done

part="${out}.part"
old="${out}.old"
rm -rf "${part}" "${old}"

cd "${ROOT}"
extra=(--history "${out}-history")
if [[ -n "${exclude}" ]]; then
  extra+=(--exclude "${exclude}")
fi
# Без git (архив исходников) прогон идёт без коммита.
if commit="$(git -C "${ROOT}" rev-parse HEAD 2>/dev/null)"; then
  extra+=(--commit "${commit}")
fi
cargo run --release --locked -p nixyon-eval --bin quality_eval -- \
  "${ROOT}/models" "${datasets}" --out "${part}" "${takes[@]}" "${extra[@]}"

if [[ -d "${out}" ]]; then
  mv "${out}" "${old}"
fi
mv "${part}" "${out}"
rm -rf "${old}"
echo "отчёт: ${out}" >&2
