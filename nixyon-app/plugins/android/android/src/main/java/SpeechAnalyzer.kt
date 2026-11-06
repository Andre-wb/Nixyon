package dev.demo.nixyon.speech

import java.io.File
import java.util.concurrent.atomic.AtomicBoolean

class AnalysisSummary(
    val segments: Int,
    val durationSec: Double,
    val vadModel: String,
    val asrModels: String,
    val embeddingModel: String,
    // Порог слияния групп в одного человека для этой модели голоса (VoiceChoice).
    val sameSpeakerCosine: Double,
    // Куски VAD (начало, конец) в секундах: по ним обрезаются отрезки нейросегментации.
    val speech: List<Pair<Double, Double>>,
)

// Почти тот же порядок, что у движка на ноутбуке (nixyon-sherpa): VAD режет запись на
// куски речи, каждый кусок получает свой текст и эмбеддинг голоса. Распознавание умеет
// брать соседние куски с короткой паузой вместе, раздавая слова по кускам обратно
// (AsrGroups.kt), чтобы фраза, разрезанная паузой, не рвалась на предложения; сейчас это
// выключено (ASR_GROUPS), куски распознаются по одному, как на ноутбуке (nixyon-sherpa,
// флаг eval --asr-groups). Группа разбирается сразу, как прочитана,
// а прогресс считается по пройденному звуку. Если окна запрошены, у куска ещё считаются
// эмбеддинги голоса по окнам. Где речь, ищет SpeechFinder: с шумоподавителем в наборе длинные
// куски VAD режутся по звуку после него, а распознавание и модель голоса всё равно слушают запись.
// hotwords: горячие слова одной строкой через «/», как их собирает Rust; пусто: без них.
class SpeechAnalyzer(
    private val models: SpeechModels,
    private val windows: WindowSpans?,
    private val hotwords: String,
) {
    private val speech = mutableListOf<Pair<Double, Double>>()

    fun run(
        wav: File,
        out: File,
        cancelled: AtomicBoolean,
        progress: (doneSec: Double, totalSec: Double) -> Unit,
    ): AnalysisSummary {
        val rate = SpeechModels.SAMPLE_RATE.toDouble()
        val pass = AnalysisProgress(cancelled, progress)
        // Первый проход: только VAD, границы кусков. Запас по краям куска зависит от
        // следующего куска, поэтому распознавание идёт вторым проходом по записи.
        val (found, totalSamples) = SpeechFinder(models).run(wav, pass::checkCancelled)
        val totalSec = totalSamples / rate
        val bounds = if (found.isEmpty() && totalSamples in 1..WHOLE_IF_NO_SPEECH_SAMPLES) {
            listOf(0L to totalSamples)
        } else {
            found
        }
        val padded = paddedBounds(bounds, totalSamples, SPEECH_PAD_SAMPLES)
        val groups = recognitionGroups(padded, ASR_GROUPS, MAX_GROUP_SAMPLES, MAX_GAP_SAMPLES)
        WavPcm16Reader(wav).use { reader ->
            SegmentsWriter(out).use { writer ->
                var position = 0L
                for (group in groups) {
                    pass.checkCancelled()
                    val from = group.first().first
                    position += reader.skipSamples(from - position)
                    val samples = reader.readSamples(group.last().second - from)
                    position += samples.size
                    if (samples.isNotEmpty()) analyzeGroup(from, group, samples, writer)
                    pass.report(position / rate, totalSec)
                }
                writer.finish()
                pass.done(totalSec)
                val manifest = models.located.manifest
                return AnalysisSummary(
                    segments = writer.count,
                    durationSec = totalSec,
                    vadModel = manifest.file(ModelRole.VAD).path,
                    asrModels = manifest.version,
                    embeddingModel = manifest.file(VoiceChoice.ROLE).path,
                    sameSpeakerCosine = VoiceChoice.SAME_SPEAKER_COSINE,
                    speech = speech.toList(),
                )
            }
        }
    }

    // Группа распознаётся одним отрезком звука вместе с паузами между кусками, слова
    // раздаются по её кускам. Если у результата нет времени токенов, куски группы
    // распознаются по одному, как раньше.
    private fun analyzeGroup(
        groupStart: Long,
        group: List<Pair<Long, Long>>,
        samples: FloatArray,
        writer: SegmentsWriter,
    ) {
        val rate = SpeechModels.SAMPLE_RATE.toDouble()
        val whole = recognize(samples)
        val texts = if (group.size == 1) {
            listOf(whole)
        } else {
            splitByPieces(
                whole.tokens,
                whole.times,
                group.map { (from, to) -> (from - groupStart) / rate to (to - groupStart) / rate },
            )
        }
        for ((i, piece) in group.withIndex()) {
            val from = (piece.first - groupStart).coerceIn(0L, samples.size.toLong()).toInt()
            val to = (piece.second - groupStart).coerceIn(from.toLong(), samples.size.toLong()).toInt()
            if (from == to) continue
            val pieceSamples = if (from == 0 && to == samples.size) samples else samples.copyOfRange(from, to)
            val text = texts?.get(i) ?: recognize(pieceSamples)
            writePiece(piece.first, pieceSamples, text, writer)
        }
    }

    private fun recognize(samples: FloatArray): PieceText {
        val stream = if (hotwords.isEmpty()) {
            models.recognizer.createStream()
        } else {
            models.recognizer.createStream(hotwords)
        }
        val result = try {
            stream.acceptWaveform(samples, SpeechModels.SAMPLE_RATE)
            models.recognizer.decode(stream)
            models.recognizer.getResult(stream)
        } finally {
            stream.release()
        }
        return PieceText(result.text, result.tokens, result.timestamps)
    }

    private fun writePiece(startSample: Long, samples: FloatArray, text: PieceText, writer: SegmentsWriter) {
        val rate = SpeechModels.SAMPLE_RATE
        val start = startSample.toDouble() / rate
        val end = start + samples.size.toDouble() / rate

        val embedding = embed(samples)
        val windowEmbeddings = windows?.of(samples.size).orEmpty().mapNotNull { (a, b) ->
            val e = embed(samples.copyOfRange(a, b))
            if (e.isEmpty()) null else WindowEmbedding(start + a.toDouble() / rate, start + b.toDouble() / rate, e)
        }

        writer.write(start, end, text.text, text.tokens, text.times, embedding, windowEmbeddings)
        speech.add(start to end)
    }

    // Кусок слишком короткий для модели голоса: эмбеддинга нет, как на ноутбуке.
    private fun embed(samples: FloatArray): FloatArray = models.voice.embed(samples)

    companion object {
        // Значения совпадают с движком на ноутбуке (nixyon-sherpa, config.rs): запас 0,2 с
        // с каждой стороны куска; запись до 20 с, где VAD речи не нашёл, идёт целиком.
        private const val SPEECH_PAD_SAMPLES = 3_200L
        private const val WHOLE_IF_NO_SPEECH_SAMPLES = 320_000L

        // Группа кусков для распознавания не длиннее 20 с, как самый длинный кусок VAD.
        // Пауза внутри группы до 2 с: звук паузы модель тоже слушает, а паузы длиннее
        // почти всегда стоят между фразами.
        private const val MAX_GROUP_SAMPLES = 320_000L
        private const val MAX_GAP_SAMPLES = 32_000L

        // Группы выключены: на смене голоса без паузы GigaAM пропускает в группе целые
        // фразы (замер на 8 диалогах: WER 21,3 % стал 22,4 %, смена голоса без паузы
        // 11,0 % стала 18,2 %). Куски распознаются по одному, как на ноутбуке.
        private const val ASR_GROUPS = false
    }
}
