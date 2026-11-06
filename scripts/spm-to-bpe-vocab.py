"""Словарь BPE для горячих слов sherpa-onnx из модели SentencePiece, без пакета sentencepiece.

Запуск: python3 -I scripts/spm-to-bpe-vocab.py <tokenizer.model> <tokens.txt> <bpe.vocab>
Пишет строки «часть<TAB>вес». Отказывается, если части не совпадают с tokens.txt модели
распознавания (без последнего служебного <blk>): иначе горячие слова разобьются не на те токены.
"""

import struct
import sys


def varint(data, i):
    value = shift = 0
    while True:
        byte = data[i]
        i += 1
        value |= (byte & 0x7F) << shift
        shift += 7
        if byte < 0x80:
            return value, i


# Поля protobuf: (номер, тип, значение). Нужны только типы 0, 1, 2 и 5.
def fields(data):
    i = 0
    while i < len(data):
        key, i = varint(data, i)
        number, kind = key >> 3, key & 7
        if kind == 0:
            value, i = varint(data, i)
        elif kind == 1:
            value, i = data[i : i + 8], i + 8
        elif kind == 2:
            size, i = varint(data, i)
            value, i = data[i : i + size], i + size
        elif kind == 5:
            value, i = data[i : i + 4], i + 4
        else:
            raise ValueError(f"неизвестный тип поля protobuf {kind}")
        yield number, kind, value


# ModelProto: поле 1 повторяется, в нём поле 1 часть (строка) и поле 2 вес (float).
def pieces(data):
    for number, kind, value in fields(data):
        if number == 1 and kind == 2:
            piece, score = None, 0.0
            for inner, inner_kind, inner_value in fields(value):
                if inner == 1 and inner_kind == 2:
                    piece = inner_value.decode("utf-8")
                elif inner == 2 and inner_kind == 5:
                    score = struct.unpack("<f", inner_value)[0]
            if piece is None:
                raise ValueError("часть словаря без текста")
            yield piece, score


def main():
    if len(sys.argv) != 4:
        sys.exit("использование: spm-to-bpe-vocab.py <tokenizer.model> <tokens.txt> <bpe.vocab>")
    model, tokens_path, out = sys.argv[1:]
    with open(model, "rb") as f:
        vocab = list(pieces(f.read()))
    with open(tokens_path, encoding="utf-8") as f:
        tokens = [line.rsplit(" ", 1)[0] for line in f.read().splitlines()]
    if tokens[-1:] == ["<blk>"]:
        tokens = tokens[:-1]
    if [p for p, _ in vocab] != tokens:
        sys.exit(f"ошибка: части словаря ({len(vocab)}) не совпадают с {tokens_path} ({len(tokens)})")
    with open(out, "w", encoding="utf-8") as f:
        for piece, score in vocab:
            f.write(f"{piece}\t{score!r}\n")


if __name__ == "__main__":
    main()
