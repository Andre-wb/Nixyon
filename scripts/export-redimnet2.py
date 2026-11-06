# /// script
# requires-python = ">=3.12"
# dependencies = [
#     "torch==2.15.0",
#     "torchaudio==2.11.1",
#     "onnx==1.23.2",
#     "onnxscript==0.7.2",
#     "onnxruntime==1.31.1",
#     "numpy==2.5.4",
#     "soundfile==0.14.0",
#     "scipy==1.18.1",
# ]
# ///
"""Выгрузка модели голоса ReDimNet2 из весов PalabraAI в ONNX с динамической длиной звука.

Python и torch нужны только на время выгрузки, во временной среде uv; в проект и в CI
они не входят. Запуск:
  git clone https://github.com/PalabraAI/redimnet2 <src> && git -C <src> checkout c5bbe0b76e37df698c403f8844e41304ceab6307
  curl -fL -o <файл>.pt https://github.com/PalabraAI/redimnet2/releases/download/v1.0.0/<файл>.pt
  uv run --script scripts/export-redimnet2.py <src> <файл>.pt <выход>.onnx <проверочный wav 16 кГц>

Вход модели audio [1, N] float32 16 кГц, выход embedding [1, D] с единичной нормой: тот же
контракт, что у выгрузки soniqo B6, но длина N любая (граф считается от 800 отсчётов).
Веса сверяются по SHA-256 из релиза v1.0.0. После выгрузки выход ONNX Runtime сравнивается
с PyTorch на кусках проверочной записи от 0,5 до 10 с; косинус ниже 0,9999 считается
ошибкой. При тех же версиях пакетов выгрузка повторялась байт в байт (проверено на macOS
arm64).
"""

import hashlib
import os
import sys

import numpy as np
import onnxruntime as ort
import soundfile as sf
import torch
import torch.nn.functional as F

# SHA-256 весов, которые показывает релиз v1.0.0 на GitHub.
WEIGHTS = {
    "b0-vox2-lm.pt": "4b5a8c00f813ab0d92fc81ea7c504475d8201bed608fb2c6aaba65be723e9c61",
    "b1-vox2-lm.pt": "9f5b2364524c3ce42f4852b8d9e6c8eb0c14a5a8878dba27f0c93ca500ee37f3",
    "b2-vox2-lm.pt": "0545a29679a87fe1c662d2bbd05e3b3fe0d1b392832729abaa135e4079a2f77a",
    "b3-vox2-lm.pt": "02d14efdc184573873b5c3f62bea847fcdb6919fbba19a8da7af7c1ba36b76a2",
    "b3-vb2+vox2+cnc2_v0-lm.pt": "b64989536ef8822447d72517ad9019b870a951e85a3f176f0ed42d2a4c3d811d",
    "b6-vb2+vox2_v0-lm.pt": "e0a7d340a92f798720d1208949aa6a6bd0cddcb0ba7d4cec33596a17a484e6a2",
}
SAMPLE_RATE = 16000
MIN_COSINE = 0.9999
# Куски проверочной записи: начало и длина в секундах.
CHECKS = [(1.0, 0.5), (3.0, 1.0), (5.0, 1.5), (10.0, 3.0), (20.0, 6.0), (30.0, 10.0), (40.0, 4.3)]


class Normed(torch.nn.Module):
    """Модель с нормировкой выхода до длины 1, как у выгрузки soniqo."""

    def __init__(self, m):
        super().__init__()
        self.m = m

    def forward(self, audio):
        return F.normalize(self.m(audio), dim=-1)


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def load(src, weights):
    expected = WEIGHTS.get(os.path.basename(weights))
    if expected is None:
        sys.exit(f"ошибка: неизвестный файл весов {weights}, известны: {', '.join(WEIGHTS)}")
    if sha256(weights) != expected:
        sys.exit(f"ошибка: сумма {weights} не совпала с релизом")
    sys.path.insert(0, src)
    from redimnet2.redimnet2 import ReDimNet2Wrap

    # weights_only: файл не исполняет код при чтении.
    state = torch.load(weights, map_location="cpu", weights_only=True)
    model = ReDimNet2Wrap(**state["model_config"])
    res = model.load_state_dict(state["state_dict"])
    if res.missing_keys or res.unexpected_keys:
        sys.exit(f"ошибка: веса не подошли к модели: {res}")
    return Normed(model.eval()).eval()


def export(net, out):
    torch.onnx.export(
        net,
        (torch.randn(1, 48000),),
        out,
        input_names=["audio"],
        output_names=["embedding"],
        dynamic_axes={"audio": {1: "samples"}, "embedding": {0: "batch"}},
        # opset 18 читает ONNX Runtime 1.28 из sherpa-onnx.
        opset_version=18,
        dynamo=False,
        do_constant_folding=True,
    )


def check(net, out, wav):
    audio, sr = sf.read(wav, dtype="float32")
    if sr != SAMPLE_RATE or audio.ndim != 1:
        sys.exit(f"ошибка: {wav} не моно 16 кГц")
    sess = ort.InferenceSession(out, providers=["CPUExecutionProvider"])
    worst = 1.0
    for start, sec in CHECKS:
        a = audio[int(start * sr) : int((start + sec) * sr)][None, :]
        if a.shape[1] != int(sec * sr):
            sys.exit(f"ошибка: {wav} короче {start + sec} с")
        with torch.no_grad():
            ref = net(torch.from_numpy(a)).numpy()[0]
        got = sess.run(["embedding"], {"audio": a})[0][0]
        cos = float(np.dot(ref, got) / (np.linalg.norm(ref) * np.linalg.norm(got)))
        worst = min(worst, cos)
        print(f"{sec:5.1f} с: косинус с PyTorch {cos:.6f}")
    if worst < MIN_COSINE:
        sys.exit(f"ошибка: худший косинус {worst:.6f} ниже {MIN_COSINE}")
    print(f"готово: {out}, SHA-256 {sha256(out)}")


def main():
    if len(sys.argv) != 5:
        sys.exit(__doc__)
    src, weights, out, wav = sys.argv[1:5]
    net = load(src, weights)
    export(net, out)
    check(net, out, wav)


if __name__ == "__main__":
    main()
