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
#     "hydra-core==1.3.2",
#     "omegaconf==2.3.0",
#     "sentencepiece==0.2.1",
#     "tqdm==4.67.1",
#     "onnx-ir==1.1.0",
# ]
# ///
"""Выгрузка модели эмоций GigaAM-Emo (SberDevices, MIT) в ONNX и её int8-копии.

Python и torch нужны только на время выгрузки, во временной среде uv; в проект и в CI
они не входят. Запуск:
  git clone https://github.com/salute-developers/GigaAM <src> && git -C <src> checkout 7447938d791c4f3e643386ee22c33777004293a5
  curl -fL -o emo.ckpt https://cdn.chatwm.opensmodel.sberdevices.ru/GigaAM/emo.ckpt
  uv run --script scripts/export-gigaam-emo.py <src> emo.ckpt <выход fp32>.onnx <выход int8>.onnx <wav 16 кГц>...

Вход модели audio [1, N] float32 16 кГц, выход probs [1, 4]: вероятности классов в порядке
id2name чекпойнта (печатается при запуске). Лог-мел признаки GigaAM считаются внутри графа:
STFT через свёртку с ядрами ДПФ, потому что комплексный torch.stft старый экспорт ONNX не
выгружает. Перед выгрузкой признаки сверяются с torchaudio, после выгрузки вероятности
ONNX Runtime (fp32 и int8) сравниваются с PyTorch на кусках проверочных записей.

Чекпойнт читается через pickle (weights_only=False, иначе GigaAM его не открыть), поэтому
сначала сверяется md5 из официального gigaam/__init__.py, затем закреплённая SHA-256.
"""

import copy
import hashlib
import sys

import numpy as np
import onnxruntime as ort
import soundfile as sf
import torch
import torch.nn.functional as F
import torchaudio

# md5 из gigaam/__init__.py на коммите 7447938d и SHA-256 того же файла, снятая 2026-11-07.
CKPT_MD5 = "7ce76f9535cb254488985057c0d33006"
CKPT_SHA256 = "ee2da14e307a2af78a8935a5519f29c5b590d1f8a7c77ebc8c3061c440052a26"
SAMPLE_RATE = 16000
# Предел расхождения вероятностей с PyTorch: fp32 почти точно, 8 бит грубее.
MAX_DIFF_FP32 = 1e-3
MAX_DIFF_INT8 = 0.05
# Длины кусков проверочной записи в секундах.
CHECK_SECONDS = [1.0, 2.5, 5.0, 8.0, 10.0]


def digest(path, algo):
    h = hashlib.new(algo)
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def load(src, ckpt):
    if digest(ckpt, "md5") != CKPT_MD5:
        sys.exit(f"ошибка: md5 {ckpt} не совпала с официальной")
    if digest(ckpt, "sha256") != CKPT_SHA256:
        sys.exit(f"ошибка: SHA-256 {ckpt} не совпала с закреплённой")
    sys.path.insert(0, src)
    from gigaam.model import GigaAMEmo

    checkpoint = torch.load(ckpt, map_location="cpu", weights_only=False)
    model = GigaAMEmo(checkpoint["cfg"])
    res = model.load_state_dict(checkpoint["state_dict"])
    if res.missing_keys or res.unexpected_keys:
        sys.exit(f"ошибка: веса не подошли к модели: {res}")
    model = model.float().eval()
    print(f"классы: {list(model.id2name)}")
    print(f"признаки: {checkpoint['cfg'].preprocessor}")
    return model


class LogMel(torch.nn.Module):
    """Лог-мел как у FeatureExtractor GigaAM, но STFT через conv1d с ядрами ДПФ."""

    def __init__(self, mel):
        super().__init__()
        stft = mel.spectrogram
        if stft.power != 2.0 or stft.normalized or stft.pad_mode != "reflect":
            sys.exit("ошибка: у MelSpectrogram другие настройки спектра")
        self.n_fft = stft.n_fft
        self.hop = stft.hop_length
        self.pad = stft.n_fft // 2 if stft.center else 0
        window = stft.window
        n = torch.arange(self.n_fft, dtype=torch.float64)
        k = torch.arange(self.n_fft // 2 + 1, dtype=torch.float64)
        angle = 2 * torch.pi * k[:, None] * n[None, :] / self.n_fft
        # Окно длиной win_length стоит по центру кадра длиной n_fft, как в torch.stft.
        win = torch.zeros(self.n_fft, dtype=torch.float64)
        left = (self.n_fft - window.numel()) // 2
        win[left : left + window.numel()] = window.double()
        real = (torch.cos(angle) * win).float()[:, None, :]
        imag = (-torch.sin(angle) * win).float()[:, None, :]
        self.register_buffer("kernel", torch.cat([real, imag], dim=0))
        self.register_buffer("fb", mel.mel_scale.fb.float())
        self.bins = self.n_fft // 2 + 1

    def forward(self, audio):
        x = audio[:, None, :]
        if self.pad:
            x = F.pad(x, (self.pad, self.pad), mode="reflect")
        spec = F.conv1d(x, self.kernel, stride=self.hop)
        power = spec[:, : self.bins] ** 2 + spec[:, self.bins :] ** 2
        mel = torch.matmul(power.transpose(1, 2), self.fb).transpose(1, 2)
        return torch.log(mel.clamp(1e-9, 1e9))


class PointwiseLinear(torch.nn.Module):
    """Свёртка с ядром 1 как линейный слой: в ONNX это MatMul, его веса квантуются."""

    def __init__(self, conv):
        super().__init__()
        if conv.kernel_size != (1,) or conv.groups != 1:
            sys.exit("ошибка: свёртка не точечная")
        self.linear = torch.nn.Linear(conv.in_channels, conv.out_channels, bias=conv.bias is not None)
        self.linear.weight.data.copy_(conv.weight.data[:, :, 0])
        if conv.bias is not None:
            self.linear.bias.data.copy_(conv.bias.data)

    def forward(self, x):
        return self.linear(x.transpose(1, 2)).transpose(1, 2)


class DynamicRelPos(torch.nn.Module):
    """Относительное позиционное кодирование GigaAM, посчитанное на длину входа.

    Исходный модуль хранит таблицу на 9999 позиций, и в ONNX она ложится константой на
    58 МБ. Значение в строке зависит только от позиции, поэтому счёт на месте даёт те же
    числа, что вырезка из таблицы.
    """

    def __init__(self, dim):
        super().__init__()
        # Признак для ConformerEncoder.forward: таблицу заранее не строить.
        self.pe = None
        self.dim = dim
        div = torch.exp(torch.arange(0, dim, 2) * -(np.log(10000.0) / dim))
        self.register_buffer("div", div.float())

    def forward(self, x):
        length = x.size(1)
        positions = torch.arange(length - 1, -length, -1, device=x.device).unsqueeze(1).float()
        angles = positions * self.div
        pe = torch.stack([torch.sin(angles), torch.cos(angles)], dim=-1).reshape(-1, self.dim)
        return x, pe.unsqueeze(0)


def patched_encoder(encoder):
    from gigaam.encoder import ConformerConvolution, RelPositionalEmbedding

    encoder = copy.deepcopy(encoder)
    if not isinstance(encoder.pos_enc, RelPositionalEmbedding):
        sys.exit("ошибка: у энкодера другое позиционное кодирование")
    encoder.pos_enc = DynamicRelPos(encoder.pos_enc.dim)
    for module in encoder.modules():
        if isinstance(module, ConformerConvolution):
            module.pointwise_conv1 = PointwiseLinear(module.pointwise_conv1)
            module.pointwise_conv2 = PointwiseLinear(module.pointwise_conv2)
    return encoder.eval()


class EmoFromAudio(torch.nn.Module):
    """Звук [1, N] превращается в вероятности классов [1, C]."""

    def __init__(self, model):
        super().__init__()
        mel = model.preprocessor.featurizer[0]
        if not isinstance(mel, torchaudio.transforms.MelSpectrogram):
            sys.exit("ошибка: у чекпойнта другой препроцессор")
        self.logmel = LogMel(mel)
        self.encoder = patched_encoder(model.encoder)
        self.head = model.head

    def forward(self, audio):
        feats = self.logmel(audio)
        lengths = torch.full((1,), feats.shape[-1], dtype=torch.long)
        encoded, _ = self.encoder(feats, lengths)
        return F.softmax(self.head(encoded.mean(dim=-1)), dim=-1)


def reference(model, audio):
    """Вероятности по пути самого GigaAM: torchaudio, энкодер, среднее, голова."""
    wav = torch.from_numpy(audio)[None, :]
    length = torch.full((1,), wav.shape[-1], dtype=torch.long)
    with torch.no_grad():
        encoded, _ = model.forward(wav, length)
        return F.softmax(model.head(encoded.mean(dim=-1)), dim=-1).numpy()[0]


def check_features(model, net, audio):
    wav = torch.from_numpy(audio)[None, :]
    length = torch.full((1,), wav.shape[-1], dtype=torch.long)
    with torch.no_grad():
        ref, _ = model.preprocessor(wav.clone(), length)
        got = net.logmel(wav)
    if ref.shape != got.shape:
        sys.exit(f"ошибка: форма признаков {tuple(got.shape)} вместо {tuple(ref.shape)}")
    # Сравнение там, где энергия заметна: в тишине лог от 1e-9 чувствителен к округлению.
    mask = ref > -10
    diff = float((ref - got).abs()[mask].max())
    print(f"признаки: расхождение с torchaudio {diff:.2e}")
    if diff > 1e-2:
        sys.exit("ошибка: признаки расходятся с torchaudio")


def export(net, out):
    torch.onnx.export(
        net,
        (torch.randn(1, 3 * SAMPLE_RATE),),
        out,
        input_names=["audio"],
        output_names=["probs"],
        dynamic_axes={"audio": {1: "samples"}},
        # opset 17 читает ONNX Runtime 1.28 из sherpa-onnx, им же выгружает сам GigaAM.
        opset_version=17,
        dynamo=False,
        do_constant_folding=True,
    )


def quantize(src, dst):
    import onnx
    from onnx import TensorProto, helper, numpy_helper
    from onnxruntime.quantization import matmul_nbits_quantizer as nbits

    # Только веса MatMul в 8 бит блоками по 128 (MatMulNBits), активации остаются fp32.
    # Обычное динамическое int8 квантует и активации: на 24 кусках класс совпал с fp32
    # в 15, у этого варианта на 280 записях RESD в 279.
    config = nbits.DefaultWeightOnlyQuantConfig(block_size=128, is_symmetric=True, bits=8, accuracy_level=None)
    quantizer = nbits.MatMulNBitsQuantizer(onnx.load(src), algo_config=config)
    quantizer.process()
    model = quantizer.model.model
    # Веса оставшихся свёрток хранятся в fp16 и приводятся к fp32 узлом Cast: чекпойнт
    # GigaAM сам хранит энкодер в fp16, точность не теряется.
    weights = {i.name: i for i in model.graph.initializer}
    casts = []
    for node in model.graph.node:
        if node.op_type != "Conv" or len(node.input) < 2:
            continue
        w = weights.get(node.input[1])
        if w is None or w.data_type != TensorProto.FLOAT:
            continue
        half = numpy_helper.from_array(numpy_helper.to_array(w).astype(np.float16), w.name + "_fp16")
        model.graph.initializer.remove(w)
        model.graph.initializer.append(half)
        casts.append(helper.make_node("Cast", [half.name], [w.name], to=TensorProto.FLOAT))
    for cast in reversed(casts):
        model.graph.node.insert(0, cast)
    onnx.save(model, dst)


def clips(wavs):
    for wav in wavs:
        audio, sr = sf.read(wav, dtype="float32")
        if sr != SAMPLE_RATE or audio.ndim != 1:
            sys.exit(f"ошибка: {wav} не моно 16 кГц")
        for sec in CHECK_SECONDS:
            n = int(sec * SAMPLE_RATE)
            if audio.shape[0] >= n:
                yield f"{wav.rsplit('/', 1)[-1]} {sec:4.1f} с", audio[:n]


def check(model, path, wavs, limit):
    sess = ort.InferenceSession(path, providers=["CPUExecutionProvider"])
    worst = 0.0
    same = total = 0
    for name, audio in clips(wavs):
        ref = reference(model, audio)
        got = sess.run(["probs"], {"audio": audio[None, :]})[0][0]
        diff = float(np.abs(ref - got).max())
        worst = max(worst, diff)
        total += 1
        same += int(ref.argmax() == got.argmax())
        print(f"  {name}: torch {np.round(ref, 3)}, onnx {np.round(got, 3)}, расхождение {diff:.4f}")
    print(f"{path}: худшее расхождение {worst:.4f}, класс совпал {same} из {total}")
    if total == 0:
        sys.exit("ошибка: нет ни одного куска для проверки")
    if worst > limit:
        sys.exit(f"ошибка: расхождение {worst:.4f} выше {limit}")


def main():
    if len(sys.argv) < 6:
        sys.exit(__doc__)
    src, ckpt, out, out_int8 = sys.argv[1:5]
    wavs = sys.argv[5:]
    torch.manual_seed(0)
    model = load(src, ckpt)
    net = EmoFromAudio(model).eval()
    first = next(clips(wavs), None)
    if first is None:
        sys.exit("ошибка: проверочные записи короче секунды")
    check_features(model, net, first[1])
    export(net, out)
    check(model, out, wavs, MAX_DIFF_FP32)
    quantize(out, out_int8)
    check(model, out_int8, wavs, MAX_DIFF_INT8)
    print(f"готово: {out} SHA-256 {digest(out, 'sha256')}")
    print(f"готово: {out_int8} SHA-256 {digest(out_int8, 'sha256')}")


if __name__ == "__main__":
    main()
