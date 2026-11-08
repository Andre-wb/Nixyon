package dev.demo.nixyon.speech

import com.k2fsa.sherpa.onnx.OnlineRecognizer

// Потоковые модели живого текста, по одной на язык, в памяти одна за раз: запись идёт одна,
// и ей нужна модель одного языка. Сразу грузится русская, чтобы ошибка в модели была видна
// при загрузке набора, а не на записи. available: языки, чьи файлы есть в наборе.
class LiveRecognizers(
    val available: Set<SpeechLanguage>,
    private val create: (SpeechLanguage) -> OnlineRecognizer,
) {
    private var language: SpeechLanguage? = null
    private var recognizer: OnlineRecognizer? = null

    init {
        SpeechLanguage.entries.firstOrNull { it in available }?.let { of(it) }
    }

    // Модель языка; null, если её нет в наборе. Прежняя модель отпускается до загрузки новой:
    // зовёт только поток живого текста, другой её в это время не трогает.
    @Synchronized
    fun of(wanted: SpeechLanguage): OnlineRecognizer? {
        if (wanted !in available) return null
        val current = recognizer
        if (current != null && language == wanted) return current
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
        language = null
    }
}
