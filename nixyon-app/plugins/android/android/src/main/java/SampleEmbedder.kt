package dev.demo.nixyon.speech

import java.io.File

// Эмбеддинг голоса по образцу, тем же порядком, что на ноутбуке (nixyon-sherpa,
// sample_embedding): речь, которую нашёл VAD, склеивается подряд, затем модель голоса.
// Речи меньше minSpeechSec секунд или эмбеддинга нет: ошибка sample_too_short.
// Речь ищет SpeechFinder, как в записи встречи: с шумоподавителем длинные куски VAD
// режутся по звуку после него, а модель голоса берёт звук образца как есть.
class SampleEmbedder(private val models: SpeechModels) {
    fun run(wav: File, minSpeechSec: Double): FloatArray {
        val rate = SpeechModels.SAMPLE_RATE
        val (bounds, _) = SpeechFinder(models).run(wav) {}
        val speech = WavPcm16Reader(wav).use { SpeechFinder.cut(it, bounds) }
        val speechSec = speech.size.toDouble() / rate
        if (speechSec < minSpeechSec) {
            throw AnalysisException(AnalysisException.SAMPLE_TOO_SHORT, "в образце $speechSec с речи")
        }
        val embedding = models.voice.embed(speech)
        if (embedding.isEmpty()) {
            throw AnalysisException(AnalysisException.SAMPLE_TOO_SHORT, "модель голоса не дала эмбеддинг")
        }
        return embedding
    }
}
