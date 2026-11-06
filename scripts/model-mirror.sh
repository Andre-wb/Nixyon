# Скачивание моделей: сначала с зеркала на сервере Nixyon Hub, где каждый файл лежит под
# своей SHA-256, затем, если зеркало недоступно или отдало не то, из исходного места.
# Подключается через source из скриптов загрузки.
#
# NIXYON_MODELS_MIRROR: адрес зеркала; пустая строка выключает зеркало.
# NIXYON_FETCH_JOBS: на сколько кусков делится большой файл с зеркала, от 1 до 64 (16).

MIRROR_DEFAULT="https://nixyon.onyxar.org/models"
# Файлы меньше этого качаются одним запросом: делить их на куски незачем.
MIRROR_SPLIT_MIN=$((16 * 1024 * 1024))

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d' ' -f1
  else
    shasum -a 256 "$1" | cut -d' ' -f1
  fi
}

size_of() {
  wc -c <"$1" | tr -d ' '
}

# Скачивает с зеркала файл с суммой $2 в $1. Успех: файл на месте и сумма сошлась.
mirror_get() {
  local out="$1" sha="$2"
  local base="${NIXYON_MODELS_MIRROR-${MIRROR_DEFAULT}}"
  [[ -n "${base}" ]] || return 1
  local jobs="${NIXYON_FETCH_JOBS:-16}"
  if ! [[ "${jobs}" =~ ^[1-9][0-9]?$ ]] || [[ "${jobs}" -gt 64 ]]; then
    echo "NIXYON_FETCH_JOBS: целое от 1 до 64, зеркало пропущено" >&2
    return 1
  fi
  local url="${base%/}/${sha}" size
  size="$(curl -fsSI --connect-timeout 5 --max-time 30 "${url}" 2>/dev/null |
    tr -d '\r' | awk 'tolower($1) == "content-length:" { n = $2 } END { print n }')" || return 1
  [[ "${size}" =~ ^[0-9]+$ ]] || return 1
  rm -f "${out}"
  if [[ "${jobs}" -eq 1 || "${size}" -lt "${MIRROR_SPLIT_MIN}" ]]; then
    curl -fsS --connect-timeout 5 --retry 3 -o "${out}" "${url}" || return 1
  else
    mirror_get_parts "${out}" "${url}" "${size}" "${jobs}" || return 1
  fi
  [[ "$(sha256_of "${out}")" == "${sha}" ]]
}

# Качает файл длиной $3 кусками по Range в $4 потоков и склеивает в $1. Кусок не той
# длины (сервер не понял Range) считается ошибкой.
mirror_get_parts() {
  local out="$1" url="$2" size="$3" jobs="$4"
  local chunk=$(((size + jobs - 1) / jobs))
  local i start end failed=0
  local pids=() lens=()
  for ((i = 0; i < jobs; i++)); do
    start=$((i * chunk))
    [[ "${start}" -lt "${size}" ]] || break
    end=$((start + chunk - 1))
    [[ "${end}" -lt "${size}" ]] || end=$((size - 1))
    lens+=($((end - start + 1)))
    curl -fsS --connect-timeout 5 --retry 3 --retry-connrefused -r "${start}-${end}" -o "${out}.${i}" "${url}" &
    pids+=($!)
  done
  for i in "${!pids[@]}"; do
    wait "${pids[$i]}" || failed=1
  done
  for i in "${!lens[@]}"; do
    if [[ "${failed}" -eq 0 ]] && [[ ! -f "${out}.${i}" || "$(size_of "${out}.${i}")" -ne "${lens[$i]}" ]]; then
      failed=1
    fi
  done
  if [[ "${failed}" -eq 0 ]]; then
    : >"${out}"
    for i in "${!lens[@]}"; do
      cat "${out}.${i}" >>"${out}"
    done
  fi
  for i in "${!lens[@]}"; do
    rm -f "${out}.${i}"
  done
  return "${failed}"
}

# Кладёт в $1 файл с суммой $2: с зеркала, иначе командой из остальных аргументов, которая
# пишет его в $1. Сумма сверяется в обоих случаях; не сошлась: файла нет, ошибка.
model_download() {
  local out="$1" sha="$2"
  shift 2
  local name="${out##*/}"
  name="${name%.part}"
  if mirror_get "${out}" "${sha}"; then
    echo "  взят с зеркала: ${name}"
    return 0
  fi
  rm -f "${out}"
  if ! "$@"; then
    rm -f "${out}"
    echo "ошибка: не удалось скачать ${name}" >&2
    return 1
  fi
  local actual
  actual="$(sha256_of "${out}")"
  if [[ "${actual}" != "${sha}" ]]; then
    rm -f "${out}"
    echo "ошибка: сумма ${name} не совпала (ожидалась ${sha}, получена ${actual})" >&2
    return 1
  fi
}
