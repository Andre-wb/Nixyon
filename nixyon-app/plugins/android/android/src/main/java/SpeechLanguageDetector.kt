package dev.demo.nixyon.speech

import com.k2fsa.sherpa.onnx.SpokenLanguageIdentification
import com.k2fsa.sherpa.onnx.SpokenLanguageIdentificationConfig
import com.k2fsa.sherpa.onnx.SpokenLanguageIdentificationWhisperConfig
import java.io.File

// Язык записи по кускам речи: Whisper tiny слушает отрезки из LanguageClips, решает
// большинство. Модель грузится на одну запись и сразу отпускается: она нужна несколько
// секунд, а память занимала бы всё время.
class SpeechLanguageDetector(private val located: LocatedModels) {
    fun detect(wav: File, bounds: List<Pair<Long, Long>>, totalSamples: Long, checkCancelled: () -> Unit): SpeechLanguage {
        val clips = LanguageClips.pick(bounds, totalSamples)
        if (clips.isEmpty()) return LanguageClips.vote(emptyList())
        val model = SpokenLanguageIdentification(located.source.assetManager, config())
        try {
            val votes = mutableListOf<SpeechLanguage?>()
            WavPcm16Reader(wav).use { reader ->
                var position = 0L
                for ((from, to) in clips.sortedBy { it.first }) {
                    checkCancelled()
                    position += reader.skipSamples(from - position)
                    val samples = reader.readSamples(to - from)
                    position += samples.size
                    votes.add(language(model, samples))
                }
            }
            return LanguageClips.vote(votes)
        } finally {
            model.release()
        }
    }

    private fun language(model: SpokenLanguageIdentification, samples: FloatArray): SpeechLanguage? {
        val stream = model.createStream()
        try {
            stream.acceptWaveform(samples, SpeechModels.SAMPLE_RATE)
            return SpeechLanguage.fromCode(model.compute(stream))
        } finally {
            stream.release()
        }
    }

    private fun config(): SpokenLanguageIdentificationConfig {
        val source = located.source
        val manifest = located.manifest
        return SpokenLanguageIdentificationConfig(
            whisper = SpokenLanguageIdentificationWhisperConfig(
                encoder = source.pathOf(manifest.file(ModelRole.LID_ENCODER)),
                decoder = source.pathOf(manifest.file(ModelRole.LID_DECODER)),
                tailPaddings = 0,
            ),
            numThreads = THREADS,
        )
    }

    companion object {
        // Отрезков всего пять, по 30 с: потоки почти не помогают.
        private const val THREADS = 1
    }
}
