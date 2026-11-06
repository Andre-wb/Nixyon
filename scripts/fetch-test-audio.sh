#!/usr/bin/env bash
# Скачивает живые записи для проверки разделения спикеров и собирает из них
# диалоги с разметкой в models/dialogs/:
#   real3: три человека из тестовых записей sherpa-onnx (китайская речь);
#   ru2, ru4, ru6: 2, 4 и 6 человек из VoxForge RU (лицензия GPLv3, только локально);
#   ru4_fast, ru4_overlap: клипы ru4 со сменой голоса без паузы и с наложением речи.
# Границы речи клипов VoxForge берутся из scripts/data/voxforge-speech-bounds.json,
# опечатки текстов PROMPTS правятся по scripts/data/voxforge-text-fixes.json.
# Скачанное хранится в models/_download/test-audio/ и повторно не качается.
#
# Суммы VoxForge сняты при скачивании (своих сумм у VoxForge нет); суммы записей
# sherpa-onnx взяты из официального checksum.txt релиза.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CACHE="${ROOT}/models/_download/test-audio"
OUT="${ROOT}/models/dialogs"
VOXFORGE_URL="http://www.repository.voxforge1.org/downloads/Russian/Trunk/Audio/Main/16kHz_16bit"
SHERPA_URL="https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-recongition-models"

VOXFORGE=(
  "gav-20170207-bfb.tgz b150da9d1725e802e955619e498cf5f6dd8b84699aa5e0c6b90ca9bdbb3de3bf"
  "gav-20170207-ciw.tgz ba18fdd966b3e53ddf337be792b654e902abc882212db1c74b193d2d9f7e5a7c"
  "kalan-20100212-ddg.tgz 40a18ec7fc782a32ce0559cbabd22c0379bb55ad59795dbe28a7d8311e6dc777"
  "kalan-20100212-jnx.tgz c3b99aa3e00ed2a13778bb15c2687bfcafb86076022da245d44f62a047d89f77"
  "polazhenko-20140825-cvh.tgz c921e88368e66005cad2135226cf8d798264ab56812b77af70fb7d551195943a"
  "polazhenko-20140825-dqr.tgz f2dd6d613504353e1a8f31e39c9445d2050e785885bc01992aac0b991e9c59b3"
  "snoop-20110506-chl.tgz f78b0a5c8b77038987865fd5e5b04325be3b1f322538ca9050b9c13b0e444042"
  "snoop-20110506-gtp.tgz 083a7a1d353a0df28d3e4bc46bf3263d90db32413c8f3c2183e875792a2f8b7f"
  "uvgeek-20150518-rib.tgz 1df5d20116989db6afdc56264e2a4219b4783c3a66a692c2ad70098ef403c8e7"
  "uvgeek-20150531-fau.tgz ddb43b9083ab8dd6b0f01e2c9cdc63e014c73f74d5fa3b73104f0603a543bad1"
  "zend-20110502-qab.tgz df1f01a27a3d53c4ab1f6891fe5410df05916ff160628a44c81ed825c2990a05"
  "zend-20110502-txq.tgz ca906ce910cfbe6a62cda298240b27e73e6e8fe7ebb0833c83715c0605830785"
)

SHERPA=(
  "fangjun-sr-1.wav 33c24061180224d2350143ee19e3af031446995c676bd25996325d34bb20a4d5"
  "fangjun-sr-2.wav c9209ff0cc83cb3c30b3efb376cec6df93f33ceb0f3c1a8eb28fced7a8d08409"
  "fangjun-sr-3.wav 5a5f2fc98cb95cde4e457c31349dcbd5cf7ec2567528aac11487aa3884795bd8"
  "fangjun-test-sr-1.wav 9175e523081bf6a630ce72a55b05f92148eaafaf58cbbbe743686cd81c50848e"
  "leijun-sr-1.wav 160a3d9bf5dd5038da8191b4430e1f3f751461613ae9489401b6b35a61b488ad"
  "leijun-sr-2.wav 37a759c036f2520d143708dfe46d45901b38a0a6e621b52d6f1aef2c000d7fe0"
  "leijun-test-sr-1.wav 36cda04ee4d10e38095de73b77c99e8e7c54347232967a9cde4cf54f7d496bab"
  "leijun-test-sr-2.wav 84b85e7413b5348303a95bd78813e4d669b4dc70fa5a99990b9860ca6fda91e8"
  "liudehua-sr-1.wav f39ebb7357d537009a9b7e59a6c9c0199f2204fcffb3c2d9d829a6961813eca1"
  "liudehua-sr-2.wav 0f0afddb99d381a23c899f464864f692dc21190cebbd4edf4ff695e10a107c9c"
  "liudehua-test-sr-1.wav a855706ad8e8253e341e3f65892c66d7dfb4bdc83675bcf38b863f2f8d167553"
  "liudehua-test-sr-2.wav 2c9ba09e7fe9deca8bea0b67776c39665cad3a1036e7a8d3407d1e45e2ede23a"
)

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d' ' -f1
  else
    shasum -a 256 "$1" | cut -d' ' -f1
  fi
}

# fetch <url> <файл> <sha256>
fetch() {
  local url="$1" target="$2" expected="$3"
  if [[ -f "${target}" ]] && [[ "$(sha256_of "${target}")" == "${expected}" ]]; then
    return
  fi
  echo "скачиваю: ${target##*/}"
  rm -f "${target}.part"
  curl -fSL --retry 3 -o "${target}.part" "${url}"
  local actual
  actual="$(sha256_of "${target}.part")"
  if [[ "${actual}" != "${expected}" ]]; then
    rm -f "${target}.part"
    echo "ошибка: сумма ${target##*/} не совпала (ожидалась ${expected}, получена ${actual})" >&2
    exit 1
  fi
  mv "${target}.part" "${target}"
}

mkdir -p "${CACHE}/voxforge" "${CACHE}/sherpa" "${OUT}"

for entry in "${VOXFORGE[@]}"; do
  name="${entry%% *}"
  fetch "${VOXFORGE_URL}/${name}" "${CACHE}/voxforge/${name}" "${entry##* }"
  dir="${CACHE}/voxforge/${name%.tgz}"
  if [[ ! -d "${dir}" ]]; then
    tar -xzf "${CACHE}/voxforge/${name}" -C "${CACHE}/voxforge"
  fi
done

for entry in "${SHERPA[@]}"; do
  name="${entry%% *}"
  fetch "${SHERPA_URL}/${name}" "${CACHE}/sherpa/${name}" "${entry##* }"
done

cargo run --quiet --release -p nixyon-sherpa --bin make_test_dialogs -- \
  "${CACHE}" "${OUT}" "${ROOT}/scripts/data/voxforge-speech-bounds.json" \
  "${ROOT}/scripts/data/voxforge-text-fixes.json"
