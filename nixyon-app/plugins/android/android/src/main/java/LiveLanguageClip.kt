package dev.demo.nixyon.speech

import kotlin.math.log10
import kotlin.math.sqrt

// Звук для определения языка живого текста: громкие куски с начала записи, пока их не
// наберётся NEEDED_SEC. Тихие куски (пауза, шум комнаты) пропускаются: по ним Whisper язык
// не услышит. VAD здесь не годится: микрофон пишет и не на 16 кГц, а VAD ждёт только её.
class LiveLanguageClip(private val rate: Int) {
    private val needed = (rate * NEEDED_SEC).toInt()
    private val samples = FloatArray(needed)
    private var filled = 0

    // Добавляет кусок, если он громкий. Возвращает звук, как только его набралось сколько
    // нужно; до этого и после этого null.
    fun add(chunk: FloatArray): FloatArray? {
        if (filled >= needed || chunk.isEmpty() || dbfs(chunk) < SPEECH_DBFS) return null
        val take = minOf(chunk.size, needed - filled)
        chunk.copyInto(samples, filled, 0, take)
        filled += take
        return if (filled == needed) samples.copyOf() else null
    }

    companion object {
        // Около двух фраз. Больше ждать нельзя: до решения живой текст идёт русской моделью.
        const val NEEDED_SEC = 10f
        // Куски тише этого считаются паузой или гулом комнаты.
        const val SPEECH_DBFS = -45f

        fun dbfs(chunk: FloatArray): Float {
            var sum = 0.0
            for (s in chunk) sum += s.toDouble() * s
            val rms = sqrt(sum / chunk.size)
            return if (rms <= 0.0) Float.NEGATIVE_INFINITY else (20 * log10(rms)).toFloat()
        }
    }
}
