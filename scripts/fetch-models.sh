#!/usr/bin/env bash
# Скачивает модели распознавания в models/, сверяет SHA-256 и пишет models/models.json:
# манифест, по которому приложение проверяет, что в папке лежит нужный набор.
# Скачанное хранится в models/_download/ и повторно не качается.
#
# Суммы сняты 2026-10-28 с релизов sherpa-onnx на GitHub; у GigaAM с пунктуацией сумма
# совпала с той, что релиз указывает сам (2026-10-29). У модели сегментации pyannote
# своей суммы в релизе нет, она снята при скачивании 2026-10-29. Токенизатор GigaAM v3
# (для горячих слов) берётся у авторов модели на Hugging Face с закреплённой ревизией,
# сумма снята при скачивании 2026-10-29. У потоковой модели для живого текста сумма
# совпала с той, что релиз указывает сам в checksum.txt (2026-10-30). У шумоподавителя
# GTCRN сумма совпала с checksum.txt релиза speech-enhancement-models (2026-11-04).
#
# Каждый файл сначала берётся с зеркала на сервере Nixyon Hub (scripts/model-mirror.sh),
# затем из исходного места.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODELS="${ROOT}/models"
CACHE="${MODELS}/_download"
BASE_URL="https://github.com/k2-fsa/sherpa-onnx/releases/download"
# shellcheck source=scripts/model-mirror.sh
source "${ROOT}/scripts/model-mirror.sh"

# Меняется при любой смене моделей: по нему приложение видит, что набор другой.
VERSION="giga-am-v3-punct-ru-2025-12-16+silero-vad+campplus-zh-en-advanced-16k+pyannote-segmentation-3-0+bpe-vocab+streaming-zipformer-small-ru-vosk-int8-2025-08-16+ced-tiny-audio-tagging-2024-04-19+redimnet2-b6-vb2-vox2-lm+gtcrn-simple"
# GigaAM v3 с пунктуацией: сразу даёт знаки препинания, заглавные буквы и числа цифрами.
GIGAAM="sherpa-onnx-nemo-transducer-punct-giga-am-v3-russian-2025-12-16"
# Нейросегментация: кто и когда говорит, включая наложения голосов.
SEGMENTATION="sherpa-onnx-pyannote-segmentation-3-0"
# Потоковая модель для живого текста во время записи: лучшая по WER из потоковых русских
# моделей sherpa-onnx на ru2, ru4, ru6.
LIVE="sherpa-onnx-streaming-zipformer-small-ru-vosk-int8-2025-08-16"
# Словарь SentencePiece той же модели: из него строится bpe.vocab, по которому sherpa-onnx
# разбивает горячие слова на токены. Части словаря совпадают с tokens.txt, это проверяется.
TOKENIZER_URL="https://huggingface.co/ai-sage/GigaAM-v3/resolve/ec1dc1f01d0d627ab2c0d3acc1e235702300d95e/tokenizer.model"
TOKENIZER_SHA256="828c12c991019eef952a960661f25a92d6ad279591e2ea466b4aeddf1d20a18a"
TOKENIZER="gigaam-v3-tokenizer.model"
# Распознавание звуков по классам AudioSet (речь, музыка, пение): по нему отбрасываются
# куски, где вместо речи звучит музыка. CED tiny, лицензия Apache-2.0.
TAGGING="sherpa-onnx-ced-tiny-audio-tagging-2024-04-19"

# путь в релизе и SHA-256
DOWNLOADS=(
  "asr-models/${GIGAAM}.tar.bz2 f9620a0099019c6afcee26525ef9ed3297fa50dd5691c1902af0c948fc1a470b"
  "asr-models/silero_vad.onnx 9e2449e1087496d8d4caba907f23e0bd3f78d91fa552479bb9c23ac09cbb1fd6"
  "speaker-recongition-models/3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx aa3cfc16963a10586a9393f5035d6d6b57e98d358b347f80c2a30bf4f00ceba2"
  "speaker-segmentation-models/${SEGMENTATION}.tar.bz2 24615ee884c897d9d2ba09bb4d30da6bb1b15e685065962db5b02e76e4996488"
  "asr-models/${LIVE}.tar.bz2 6ba68a01ff3c5445aaf2d61e9b97b026f1149dcc9049d11af3f44f55176341d8"
  "audio-tagging-models/${TAGGING}.tar.bz2 84baf315b57d61aa69480c4fee878dab54cbc7be3e877db334e65d8b087e23c3"
  # Шумоподавление GTCRN для второго VAD: им режутся длинные куски VAD в шуме.
  "speech-enhancement-models/gtcrn_simple.onnx e77603ac0c23dac3227dd2d7135b3a585cbee2679048aecfa886657d3ae1b534"
)

mkdir -p "${CACHE}"

for entry in "${DOWNLOADS[@]}"; do
  rel="${entry%% *}"
  expected="${entry##* }"
  name="${rel##*/}"
  target="${CACHE}/${name}"

  if [[ -f "${target}" ]] && [[ "$(sha256_of "${target}")" == "${expected}" ]]; then
    echo "есть: ${name}"
    continue
  fi

  echo "скачиваю: ${name}"
  tmp="${target}.part"
  model_download "${tmp}" "${expected}" curl -fSL --retry 3 -o "${tmp}" "${BASE_URL}/${rel}" || exit 1
  mv "${tmp}" "${target}"
done

tokenizer="${CACHE}/${TOKENIZER}"
if [[ -f "${tokenizer}" ]] && [[ "$(sha256_of "${tokenizer}")" == "${TOKENIZER_SHA256}" ]]; then
  echo "есть: ${TOKENIZER}"
else
  echo "скачиваю: ${TOKENIZER}"
  model_download "${tokenizer}.part" "${TOKENIZER_SHA256}" \
    curl -fSL --retry 3 -o "${tokenizer}.part" "${TOKENIZER_URL}" || exit 1
  mv "${tokenizer}.part" "${tokenizer}"
fi

# роль и файл; роли читает приложение
ROLES=(
  "vad vad.onnx"
  "asr_encoder asr-encoder.int8.onnx"
  "asr_decoder asr-decoder.onnx"
  "asr_joiner asr-joiner.onnx"
  "asr_tokens asr-tokens.txt"
  "embedding embedding.onnx"
  "segmentation segmentation.onnx"
  "asr_bpe_vocab asr-bpe.vocab"
  "live_encoder live-encoder.int8.onnx"
  "live_decoder live-decoder.onnx"
  "live_joiner live-joiner.int8.onnx"
  "live_tokens live-tokens.txt"
  "tagging_model tagging.int8.onnx"
  "tagging_labels tagging-labels.csv"
  "redimnet2 redimnet2-b6.onnx"
  "denoiser denoiser.onnx"
  "age_gender age-gender.onnx"
)

manifest="${MODELS}/models.json"

# Набор уже разложен, если в манифесте та же версия и каждый файл совпадает с суммой
# из манифеста: тогда большой архив GigaAM заново не распаковывается.
laid_out() {
  [[ -f "${manifest}" && -f "${MODELS}/example.wav" ]] || return 1
  grep -qF "\"version\": \"${VERSION}\"" "${manifest}" || return 1
  local entry file recorded
  for entry in "${ROLES[@]}"; do
    file="${entry##* }"
    [[ -f "${MODELS}/${file}" ]] || return 1
    recorded="$(grep -oE "\"path\": \"${file}\", \"size\": [0-9]+, \"sha256\": \"[0-9a-f]{64}\"" "${manifest}" \
      | grep -oE '[0-9a-f]{64}')" || return 1
    [[ "$(sha256_of "${MODELS}/${file}")" == "${recorded}" ]] || return 1
  done
}

# Модели на крейте ort для замера, отдельным набором models/ort/ со своим models.json.
# В основной набор и в APK из них входит только ReDimNet2-B6 soniqo: модель голоса
# приложения на Android. ReDimNet2-B6 (модель голоса) и Sortformer
# 4spk-v2 (нейросегментация); fp32 Sortformer на 492 МБ качается только при
# NIXYON_SORTFORMER_FP32=1. Ревизии закреплены, суммы совпали с теми, что Hugging Face
# показывает для файлов (2026-11-02). Лёгкие ReDimNet2 (B0-B3, B3 на VoxBlink2) и B6 с
# динамическим входом выгружены из весов PalabraAI/redimnet2 v1.0.0 скриптом
# scripts/export-redimnet2.py и лежат в релизе models-redimnet2-v1 приватного репозитория
# Andre-wb/demo: с зеркала без токена, иначе через API GitHub с токеном, файлы закреплены
# номером, суммы совпали с теми, что GitHub показывает для файлов (2026-11-03).
# Токен GitHub для файлов приватного репозитория: из GITHUB_TOKEN или GH_TOKEN, иначе из
# gh auth token. Сам токен не печатается.
github_token() {
  local token="${GITHUB_TOKEN:-${GH_TOKEN:-}}"
  if [[ -z "${token}" ]] && command -v gh >/dev/null 2>&1; then
    token="$(gh auth token 2>/dev/null || true)"
  fi
  if [[ -z "${token}" ]]; then
    echo "ошибка: для моделей ReDimNet2 из Andre-wb/demo нужен токен GitHub: задайте GITHUB_TOKEN или GH_TOKEN либо войдите в gh (gh auth login)" >&2
    exit 1
  fi
  printf '%s' "${token}"
}

GITHUB_TOKEN_VALUE=""

# Файл релиза приватного репозитория $2 в $1. Токен берётся один раз и только если
# зеркало файл не дало.
github_asset() {
  if [[ -z "${GITHUB_TOKEN_VALUE}" ]]; then
    GITHUB_TOKEN_VALUE="$(github_token)" || return 1
  fi
  # Заголовок с токеном идёт через stdin, а не аргументом: так его не видно в списке
  # процессов. На хранилище файлов после переадресации curl его не передаёт.
  printf 'Authorization: Bearer %s\n' "${GITHUB_TOKEN_VALUE}" |
    curl -fSL --retry 3 -H @- -H "Accept: application/octet-stream" -o "$1" "$2"
}

# Пол и возраст по голосу: audEERING wav2vec2-large-robust, 6 слоёв, официальная выгрузка
# ONNX из записи Zenodo 7761387 (лицензия CC BY-NC-SA 4.0, только некоммерческое
# использование). MD5 архива совпала с записью Zenodo (2026-11-05), тогда же сняты SHA-256
# архива и model.onnx из него. Классы выхода logits_gender по model.yaml архива: female,
# male, child.
AGE_GENDER_URL="https://zenodo.org/records/7761387/files/w2v2-L-robust-6-age-gender.25c844af-1.1.1.zip?download=1"
AGE_GENDER_ZIP="age-gender/w2v2-L-robust-6-age-gender.25c844af-1.1.1.zip"
AGE_GENDER_ZIP_SHA256="ee6b1a69aef08f2f6fdd8e91aa88cc055aab97288b3bbd5dfa4a4997c94fe17f"
AGE_GENDER_FILE="age-gender.onnx"
AGE_GENDER_ROLE="age_gender"
AGE_GENDER_SHA256="75c5cc3debc2013215cee5f331a66b59bd7205da170fb61abab46fc4507df7be"
AGE_GENDER_VERSION="audeering-w2v2-l-robust-6-age-gender-1.1.1"

# Кладёт model.onnx из архива audEERING в набор ort под именем AGE_GENDER_FILE.
fetch_age_gender() {
  local dir="$1"
  local target="${dir}/${AGE_GENDER_FILE}"
  local zip="${CACHE}/${AGE_GENDER_ZIP}"
  if [[ -f "${target}" ]] && [[ "$(sha256_of "${target}")" == "${AGE_GENDER_SHA256}" ]]; then
    echo "есть: ort/${AGE_GENDER_FILE}"
    return 0
  fi
  if [[ -f "${zip}" ]] && [[ "$(sha256_of "${zip}")" == "${AGE_GENDER_ZIP_SHA256}" ]]; then
    echo "есть: ${AGE_GENDER_ZIP}"
  else
    echo "скачиваю: ${AGE_GENDER_ZIP}"
    mkdir -p "$(dirname "${zip}")"
    model_download "${zip}.part" "${AGE_GENDER_ZIP_SHA256}" \
      curl -fSL --retry 3 -o "${zip}.part" "${AGE_GENDER_URL}" || exit 1
    mv "${zip}.part" "${zip}"
  fi
  # Распаковка через python3: он есть на раннере CI, unzip там не гарантирован.
  python3 -I -c 'import shutil, sys, zipfile
with zipfile.ZipFile(sys.argv[1]) as z, z.open("model.onnx") as src, open(sys.argv[2], "wb") as dst:
    shutil.copyfileobj(src, dst)' "${zip}" "${target}.part"
  if [[ "$(sha256_of "${target}.part")" != "${AGE_GENDER_SHA256}" ]]; then
    rm -f "${target}.part"
    echo "ошибка: сумма model.onnx из ${AGE_GENDER_ZIP} не совпала" >&2
    exit 1
  fi
  mv "${target}.part" "${target}"
}

fetch_ort_models() {
  local dir="${MODELS}/ort"
  local hf="https://huggingface.co"
  local gh="https://api.github.com/repos/Andre-wb/demo/releases/assets"
  local version="redimnet2-b6-vb2-vox2-lm+sortformer-4spk-v2-int8-dimq1+redimnet2-palabra-dyn-v1"
  # адрес, имя файла в models/ort/, роль и SHA-256
  local files=(
    "${hf}/soniqo/ReDimNet2-B6-ONNX-FP32/resolve/e911e3f063899805f3d94ee5d1db53fff8e9f3e8/ReDimNet2B6.onnx redimnet2-b6.onnx redimnet2 d960e9f713eaba712f44498e82221b6fc12fcd7eaacd367c9eac45d3a6ce59ec"
    # Веса int8 лежат во внешнем файле, модель ищет его рядом с собой по имени sortformer.data.
    "${hf}/DimQ1/sortformer-4spk-v2-onnx-int8-cpu/resolve/aac523db80e95670a680c7ec3bd7ca8293258e18/sortformer.onnx sortformer.onnx sortformer_int8 722c521c9d5ec9f6c10af9930745877baa3202c3cd64706bb1d27dffb20291ec"
    "${hf}/DimQ1/sortformer-4spk-v2-onnx-int8-cpu/resolve/aac523db80e95670a680c7ec3bd7ca8293258e18/sortformer.data sortformer.data sortformer_int8_data 692c7b9f2eb289683da72ff9e1a291f98b3ad27c9a32391f85496683c8fb238d"
    "${gh}/704084864 redimnet2-b0.onnx redimnet2_b0 92712daf63d41ccab3df0eddb3f6303054e3cb433bf1a1ad7091023c1d6faf85"
    "${gh}/704084817 redimnet2-b1.onnx redimnet2_b1 9f03186d3b83673287d61581c5607996cb3703e7c0d00f348cc9b4663fc0dc06"
    "${gh}/704084818 redimnet2-b2.onnx redimnet2_b2 3dac4b7ff149a6bcd5a078305c525886d512c49faf0e6c7ad7d2dd3d0fd42e2f"
    "${gh}/704084821 redimnet2-b3.onnx redimnet2_b3 b1ea699379433bc0bd19f4dc9cbd2c3b7f9c393608b3d9958bfad7d22a35c568"
    "${gh}/704084829 redimnet2-b3-vb2.onnx redimnet2_b3_vb2 53020c37e541fa10bce230b267f5f4bdbad2e3bb526fe69f257848735a45f2b4"
    "${gh}/704085060 redimnet2-b6-dyn.onnx redimnet2_b6_dyn c50697b6aea9db359a3d6bee606118e82209e64d96dc38614083ba1e7f63c890"
  )
  if [[ "${NIXYON_SORTFORMER_FP32:-0}" == "1" ]]; then
    version="${version}+sortformer-4spk-v2-fp32-dimq1"
    files+=("${hf}/DimQ1/sortformer-4spk-v2-onnx-fp32-cpu/resolve/46bcd3506f581ee8dcbca78149b62c1eaf80ddf5/sortformer.onnx sortformer-fp32.onnx sortformer_fp32 58c2b74072e7ae08ba25ab334260eb675b96f34571574cb0ecac9467e41f27f7")
  fi
  mkdir -p "${dir}"
  fetch_age_gender "${dir}"
  local entry url name expected target
  for entry in "${files[@]}"; do
    read -r url name _ expected <<<"${entry}"
    target="${dir}/${name}"
    if [[ -f "${target}" ]] && [[ "$(sha256_of "${target}")" == "${expected}" ]]; then
      echo "есть: ort/${name}"
      continue
    fi
    echo "скачиваю: ort/${name}"
    if [[ "${url}" == "${gh}/"* ]]; then
      model_download "${target}.part" "${expected}" github_asset "${target}.part" "${url}" || exit 1
    else
      model_download "${target}.part" "${expected}" curl -fSL --retry 3 -o "${target}.part" "${url}" || exit 1
    fi
    mv "${target}.part" "${target}"
  done
  # имя файла, роль и SHA-256 каждой записи манифеста
  local entries=()
  for entry in "${files[@]}"; do
    read -r _ name role expected <<<"${entry}"
    entries+=("${name} ${role} ${expected}")
  done
  entries+=("${AGE_GENDER_FILE} ${AGE_GENDER_ROLE} ${AGE_GENDER_SHA256}")
  version="${version}+${AGE_GENDER_VERSION}"
  {
    printf '{\n  "version": "%s",\n  "files": {\n' "${version}"
    local last=$((${#entries[@]} - 1)) i role sep
    for i in "${!entries[@]}"; do
      read -r name role expected <<<"${entries[$i]}"
      sep=","
      [[ "${i}" -eq "${last}" ]] && sep=""
      printf '    "%s": { "path": "%s", "size": %s, "sha256": "%s" }%s\n' \
        "${role}" "${name}" "$(size_of "${dir}/${name}")" "${expected}" "${sep}"
    done
    printf '  }\n}\n'
  } >"${dir}/models.json"
  echo "готово: ${dir}/models.json"
}

fetch_ort_models

if laid_out; then
  echo "модели уже разложены: ${manifest}"
  exit 0
fi

echo "раскладываю модели в ${MODELS}"
tar -xjf "${CACHE}/${GIGAAM}.tar.bz2" -C "${CACHE}" \
  "${GIGAAM}/encoder.int8.onnx" \
  "${GIGAAM}/decoder.onnx" \
  "${GIGAAM}/joiner.onnx" \
  "${GIGAAM}/tokens.txt" \
  "${GIGAAM}/test_wavs/example.wav"

tar -xjf "${CACHE}/${SEGMENTATION}.tar.bz2" -C "${CACHE}" \
  "${SEGMENTATION}/model.onnx" \
  "${SEGMENTATION}/LICENSE"

tar -xjf "${CACHE}/${LIVE}.tar.bz2" -C "${CACHE}" \
  "${LIVE}/encoder.int8.onnx" \
  "${LIVE}/decoder.onnx" \
  "${LIVE}/joiner.int8.onnx" \
  "${LIVE}/tokens.txt"

tar -xjf "${CACHE}/${TAGGING}.tar.bz2" -C "${CACHE}" \
  "${TAGGING}/model.int8.onnx" \
  "${TAGGING}/class_labels_indices.csv"

cp "${CACHE}/${GIGAAM}/encoder.int8.onnx" "${MODELS}/asr-encoder.int8.onnx"
cp "${CACHE}/${GIGAAM}/decoder.onnx" "${MODELS}/asr-decoder.onnx"
cp "${CACHE}/${GIGAAM}/joiner.onnx" "${MODELS}/asr-joiner.onnx"
cp "${CACHE}/${GIGAAM}/tokens.txt" "${MODELS}/asr-tokens.txt"
cp "${CACHE}/silero_vad.onnx" "${MODELS}/vad.onnx"
cp "${CACHE}/3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx" "${MODELS}/embedding.onnx"
# Модель float32: int8 на замере ставила ложную пометку перекрытия на живой записи.
cp "${CACHE}/${SEGMENTATION}/model.onnx" "${MODELS}/segmentation.onnx"
cp "${CACHE}/${LIVE}/encoder.int8.onnx" "${MODELS}/live-encoder.int8.onnx"
cp "${CACHE}/${LIVE}/decoder.onnx" "${MODELS}/live-decoder.onnx"
cp "${CACHE}/${LIVE}/joiner.int8.onnx" "${MODELS}/live-joiner.int8.onnx"
cp "${CACHE}/${LIVE}/tokens.txt" "${MODELS}/live-tokens.txt"
cp "${CACHE}/${TAGGING}/model.int8.onnx" "${MODELS}/tagging.int8.onnx"
cp "${CACHE}/${TAGGING}/class_labels_indices.csv" "${MODELS}/tagging-labels.csv"
cp "${CACHE}/gtcrn_simple.onnx" "${MODELS}/denoiser.onnx"
# Модель голоса приложения на Android берётся из набора ort, где её сумма уже сверена.
# Жёсткая ссылка, если можно: вторая копия 51 МБ на диске не нужна.
rm -f "${MODELS}/redimnet2-b6.onnx"
ln "${MODELS}/ort/redimnet2-b6.onnx" "${MODELS}/redimnet2-b6.onnx" 2>/dev/null \
  || cp "${MODELS}/ort/redimnet2-b6.onnx" "${MODELS}/redimnet2-b6.onnx"
# Модель «ребёнок или взрослый» тоже из набора ort; 363 МБ, поэтому копия только если ссылка
# не получилась.
rm -f "${MODELS}/age-gender.onnx"
ln "${MODELS}/ort/age-gender.onnx" "${MODELS}/age-gender.onnx" 2>/dev/null \
  || cp "${MODELS}/ort/age-gender.onnx" "${MODELS}/age-gender.onnx"
python3 -I "${ROOT}/scripts/spm-to-bpe-vocab.py" "${tokenizer}" "${MODELS}/asr-tokens.txt" "${MODELS}/asr-bpe.vocab"
# Пример записи из архива GigaAM нужен для проверок на ноутбуке и устройстве, в манифест не входит.
cp "${CACHE}/${GIGAAM}/test_wavs/example.wav" "${MODELS}/example.wav"

{
  printf '{\n  "version": "%s",\n  "files": {\n' "${VERSION}"
  last=$((${#ROLES[@]} - 1))
  for i in "${!ROLES[@]}"; do
    role="${ROLES[$i]%% *}"
    file="${ROLES[$i]##* }"
    sep=","
    [[ "${i}" -eq "${last}" ]] && sep=""
    printf '    "%s": { "path": "%s", "size": %s, "sha256": "%s" }%s\n' \
      "${role}" "${file}" "$(size_of "${MODELS}/${file}")" "$(sha256_of "${MODELS}/${file}")" "${sep}"
  done
  printf '  }\n}\n'
} >"${manifest}"

echo "готово: ${manifest}"
