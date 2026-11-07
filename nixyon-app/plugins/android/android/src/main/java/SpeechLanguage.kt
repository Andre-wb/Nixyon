package dev.demo.nixyon.speech

// Язык речи записи; code: ISO 639-1, как его шлёт ядро и отдаёт Whisper. От языка зависит
// распознаватель: русский распознаёт GigaAM, английский Parakeet.
enum class SpeechLanguage(val code: String) {
    RU("ru"),
    EN("en"),
    ;

    // Файлы распознавателя: кодировщик, декодер, объединитель и токены.
    val asrRoles: List<ModelRole>
        get() = when (this) {
            RU -> listOf(ModelRole.ASR_ENCODER, ModelRole.ASR_DECODER, ModelRole.ASR_JOINER, ModelRole.ASR_TOKENS)
            EN -> listOf(ModelRole.ASR_EN_ENCODER, ModelRole.ASR_EN_DECODER, ModelRole.ASR_EN_JOINER, ModelRole.ASR_EN_TOKENS)
        }

    // Горячие слова только у русского: поиск по лучам sherpa-onnx 2.0.1 на Parakeet TDT
    // теряет фразы в кусках из нескольких фраз (на ноутбуке на en2 из трёх фраз куска
    // осталось две), поэтому английский распознаётся жадным поиском без горячих слов. Так же
    // на ноутбуке (nixyon-sherpa, Decoding::for_language).
    val hotwords: Boolean
        get() = this == RU

    companion object {
        // Другой язык или пустая строка: null.
        fun fromCode(code: String?): SpeechLanguage? = entries.firstOrNull { it.code == code }
    }
}
