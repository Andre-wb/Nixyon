package dev.demo.nixyon.speech

// Сэмплы PCM 16 бит: в float -1..1 для моделей и в два байта little-endian для WAV. Одни
// формулы на запись с микрофона, импорт, живой текст и чтение WAV.
object Pcm16 {
    // Делитель 32768, как в sherpa-onnx и в движке на ноутбуке: -32768 даёт ровно -1.
    fun toFloat(sample: Short): Float = sample / 32768f

    // Float из декодера в сэмпл: всё за пределами -1..1 срезается, 1 даёт 32767.
    fun fromFloat(value: Float): Int = (value.coerceIn(-1f, 1f) * 32767f).toInt()

    fun get(bytes: ByteArray, at: Int): Short =
        ((bytes[at + 1].toInt() shl 8) or (bytes[at].toInt() and 0xff)).toShort()

    // Младшие 16 бит sample в bytes[at] и bytes[at + 1].
    fun put(bytes: ByteArray, at: Int, sample: Int) {
        bytes[at] = (sample and 0xff).toByte()
        bytes[at + 1] = (sample shr 8).toByte()
    }

    // Сводит кадры из channels каналов в моно средним, на месте: моно пишется в начало
    // bytes. Возвращает длину моно в байтах; неполный кадр в конце отбрасывается.
    fun downmix(bytes: ByteArray, length: Int, channels: Int): Int {
        if (channels <= 1) return length
        val frames = length / (2 * channels)
        for (f in 0 until frames) {
            var sum = 0
            for (c in 0 until channels) sum += get(bytes, (f * channels + c) * 2)
            put(bytes, f * 2, sum / channels)
        }
        return frames * 2
    }
}
