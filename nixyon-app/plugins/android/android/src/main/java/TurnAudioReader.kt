package dev.demo.nixyon.speech

import java.io.File

// Чтение кусков записи по порядку, каждый кусок целиком. Кусок, который начинается раньше
// уже прочитанного (реплики при наложении голосов), читается заново с начала файла.
class TurnAudioReader(private val wav: File) : AutoCloseable {
    private var reader: WavPcm16Reader? = null
    private var position = 0L

    // Сэмплы [a, b); за концом записи кусок обрезается, целиком за концом пуст.
    fun read(bounds: Pair<Long, Long>): FloatArray {
        val (a, b) = bounds
        var current = reader
        if (current == null || a < position) {
            current?.close()
            current = WavPcm16Reader(wav)
            reader = current
            position = 0L
        }
        position += current.skipSamples(a - position)
        if (position < a) return FloatArray(0)
        val samples = current.readSamples(b - a)
        position += samples.size
        return samples
    }

    override fun close() {
        reader?.close()
    }
}
