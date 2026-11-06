package dev.demo.nixyon.speech

import java.io.File
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToLong

// Нейросегментация вторым проходом по записи, тем же порядком, что у движка на ноутбуке
// (nixyon-sherpa, voice_spans): запись подаётся блоками по blockSec секунд, потому что
// диаризация берёт звук целиком массивом. Отрезки говорящих обрезаются по кускам VAD
// и получают эмбеддинг нашей модели голоса; номера говорящих sherpa отбрасываются.
class SpeechSegmenter(private val models: SpeechModels, blockSec: Double) {
    private val rate = SpeechModels.SAMPLE_RATE
    private val block: Int = maxOf(1, (blockSec * rate).roundToLong().coerceAtMost(Int.MAX_VALUE.toLong()).toInt())

    // speech: куски VAD (начало, конец) в секундах. Итог по времени.
    fun run(
        wav: File,
        speech: List<Pair<Double, Double>>,
        cancelled: AtomicBoolean,
        progress: (doneSec: Double, totalSec: Double) -> Unit,
    ): List<VoiceSpan> {
        val found = mutableListOf<VoiceSpan>()
        val pass = AnalysisProgress(cancelled, progress)
        WavPcm16Reader(wav).use { reader ->
            val totalSec = reader.totalSamples / rate.toDouble()
            var blockStart = 0L
            while (true) {
                pass.checkCancelled()
                val samples = reader.next(block) ?: break
                val offset = blockStart.toDouble() / rate
                val blockEnd = (blockStart + samples.size).toDouble() / rate
                val segments = models.diarizer.process(samples).sortedWith(
                    compareBy({ it.start }, { it.end }, { it.speaker }),
                )
                for (s in segments) {
                    // Отрезок может чуть выйти за конец блока из-за шага кадров модели.
                    val a = offset + s.start.toDouble()
                    val b = min(offset + s.end.toDouble(), blockEnd)
                    for ((c, d) in speech) {
                        val start = max(a, c)
                        val end = min(b, d)
                        if (start >= end) continue
                        pass.checkCancelled()
                        val from = (sampleIndex(start) - blockStart).coerceIn(0L, samples.size.toLong()).toInt()
                        val to = (sampleIndex(end) - blockStart).coerceIn(0L, samples.size.toLong()).toInt()
                        if (from >= to) continue
                        val embedding = embed(samples.copyOfRange(from, to))
                        if (embedding.isNotEmpty()) found.add(VoiceSpan(start, end, embedding))
                    }
                }
                blockStart += samples.size
                pass.report(blockStart / rate.toDouble(), totalSec)
            }
            pass.done(totalSec)
        }
        // Сортировка устойчивая, по началу и концу, как clip_spans на ноутбуке.
        return found.sortedWith(compareBy({ it.start }, { it.end }))
    }

    // Округление половины вверх, как f64::round у неотрицательных чисел на ноутбуке.
    private fun sampleIndex(sec: Double): Long = Math.round(sec * rate).coerceAtLeast(0L)

    private fun embed(samples: FloatArray): FloatArray = models.voice.embed(samples)
}
