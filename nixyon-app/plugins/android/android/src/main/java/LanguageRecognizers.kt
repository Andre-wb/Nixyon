package dev.demo.nixyon.speech

import com.k2fsa.sherpa.onnx.OfflineRecognizer

// Распознаватель одного языка за раз: GigaAM и Parakeet вместе занимают слишком много памяти.
// Запись на другом языке отпускает загруженный и грузит свой. Разбор записей идёт по одному
// (SpeechEngine), поэтому распознаватель не отпускается, пока им кто-то распознаёт.
class LanguageRecognizers(
    first: SpeechLanguage,
    private val create: (SpeechLanguage) -> OfflineRecognizer,
) {
    private var language: SpeechLanguage = first
    private var recognizer: OfflineRecognizer? = create(first)

    @Synchronized
    fun of(wanted: SpeechLanguage): OfflineRecognizer {
        val current = recognizer
        if (current != null && language == wanted) return current
        // Сначала отпустить прежний язык, потом грузить новый: две модели сразу не нужны.
        current?.let { runCatching { it.release() } }
        recognizer = null
        val loaded = create(wanted)
        recognizer = loaded
        language = wanted
        return loaded
    }

    @Synchronized
    fun release() {
        recognizer?.let { runCatching { it.release() } }
        recognizer = null
    }
}
