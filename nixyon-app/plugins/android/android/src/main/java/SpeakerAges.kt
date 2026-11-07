package dev.demo.nixyon.speech

import java.io.File
import java.util.concurrent.atomic.AtomicBoolean

// Кусок речи спикера в секундах от начала записи.
class AgePiece(val startSec: Double, val endSec: Double)

// Спикер и куски его речи; куски выбирает Rust (speaker_voice_pieces).
class AgeSpeaker(val id: Int, val pieces: List<AgePiece>)

// Ребёнок ли спикер и женский ли голос; null, если ответа нет.
class AgeVerdict(val id: Int, val child: Boolean?, val female: Boolean?)

// Ребёнок или взрослый по голосу каждого спикера. Модель audEERING открывается на этот
// разбор и закрывается в конце, даже при ошибке или отмене. Модели речи ей не нужны, только
// где лежит набор: на время разбора их можно отпустить (SpeechEngine.speakerAges).
class SpeakerAges(private val located: LocatedModels) {
    fun run(wav: File, speakers: List<AgeSpeaker>, cancelled: AtomicBoolean): List<AgeVerdict> {
        val manifest = located.manifest
        val threads = CpuCores.computeThreads
        // Модель грузится секунды: отмена до начала не ждёт загрузки.
        checkCancelled(cancelled)
        return AgeModel.open(located.source, manifest.file(ModelRole.AGE_GENDER), threads).use { model ->
            speakers.map { speaker ->
                checkCancelled(cancelled)
                val pieces = WavPcm16Reader(wav).use { readPieces(it, bounds(speaker.pieces)) }
                val (child, female) = model.speakerVoice(pieces)
                AgeVerdict(speaker.id, child, female)
            }
        }
    }

    private fun checkCancelled(cancelled: AtomicBoolean) {
        if (cancelled.get()) throw AnalysisException(AnalysisException.CANCELLED, "разбор отменён")
    }

    companion object {
        // Куски в сэмплах, по порядку в записи; пустые и вывернутые пропускаются.
        fun bounds(pieces: List<AgePiece>): List<Pair<Long, Long>> =
            pieces
                .map { toSample(it.startSec) to toSample(it.endSec) }
                .filter { (a, b) -> b > a }
                .sortedBy { it.first }

        // Каждый кусок отдельным массивом: модель оценивает их по отдельности и усредняет
        // с весом по длине. Перекрытие с прошлым куском не читается второй раз.
        fun readPieces(reader: WavPcm16Reader, bounds: List<Pair<Long, Long>>): List<FloatArray> {
            val out = ArrayList<FloatArray>()
            var position = 0L
            for ((a, b) in bounds) {
                val from = maxOf(a, position)
                if (b <= from) continue
                position += reader.skipSamples(from - position)
                if (position < from) break
                val piece = reader.readSamples(b - from)
                position += piece.size
                if (piece.isNotEmpty()) out.add(piece)
            }
            return out
        }

        private fun toSample(sec: Double): Long =
            if (sec.isFinite() && sec > 0.0) Math.round(sec * SpeechModels.SAMPLE_RATE) else 0L
    }
}
