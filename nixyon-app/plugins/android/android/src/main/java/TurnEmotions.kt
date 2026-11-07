package dev.demo.nixyon.speech

import java.io.File
import java.util.concurrent.atomic.AtomicBoolean

// Реплика в секундах от начала записи.
class EmotionTurn(val id: Int, val startSec: Double, val endSec: Double)

// Эмоция реплики: ключ из расшифровки или null, если ответа нет.
class EmotionVerdict(val id: Int, val emotion: String?)

// Эмоция каждой реплики. Модель открывается на этот проход и закрывается в конце, даже
// при ошибке или отмене. Модели речи ей не нужны, только где лежит набор: на время
// прохода их можно отпустить (SpeechEngine.turnEmotions).
class TurnEmotions(private val located: LocatedModels) {
    fun run(wav: File, turns: List<EmotionTurn>, cancelled: AtomicBoolean): List<EmotionVerdict> {
        val threads = Runtime.getRuntime().availableProcessors().coerceIn(1, MAX_THREADS)
        // Модель грузится секунды: отмена до начала не ждёт загрузки.
        checkCancelled(cancelled)
        return EmotionModel.open(located.source, located.manifest.file(ModelRole.EMOTION), threads).use { model ->
            val out = ArrayList<EmotionVerdict>(turns.size)
            TurnAudioReader(wav).use { reader ->
                for ((turn, bounds) in ordered(turns)) {
                    checkCancelled(cancelled)
                    val samples = bounds?.let { reader.read(it) }
                    val emotion = if (samples == null || samples.isEmpty()) null else model.turnEmotion(samples)
                    out.add(EmotionVerdict(turn.id, emotion))
                }
            }
            out
        }
    }

    private fun checkCancelled(cancelled: AtomicBoolean) {
        if (cancelled.get()) throw AnalysisException(AnalysisException.CANCELLED, "разбор отменён")
    }

    companion object {
        private const val MAX_THREADS = 4

        // Реплики по началу в записи вместе с границами в сэмплах; у пустой или вывернутой
        // границ нет. Ответы идут в том же порядке.
        fun ordered(turns: List<EmotionTurn>): List<Pair<EmotionTurn, Pair<Long, Long>?>> =
            turns
                .map { it to (toSample(it.startSec) to toSample(it.endSec)).takeIf { (a, b) -> b > a } }
                .sortedBy { it.second?.first ?: 0L }

        private fun toSample(sec: Double): Long =
            if (sec.isFinite() && sec > 0.0) Math.round(sec * SpeechModels.SAMPLE_RATE) else 0L
    }
}
