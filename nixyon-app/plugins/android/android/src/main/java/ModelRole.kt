package dev.demo.nixyon.speech

// Роли файлов в models.json: по ним плагин находит нужный файл, не завися от имён.
// Необязательные роли может не быть в наборе: без живой модели запись идёт без живого текста
// (без английской живой модели английская речь идёт без живого текста),
// без шумоподавителя длинные куски VAD не режутся, без модели audEERING у спикеров нет метки
// «ребёнок или взрослый», без GigaAM-Emo у реплик нет эмоции.
enum class ModelRole(val key: String, val optional: Boolean = false) {
    VAD("vad"),
    ASR_ENCODER("asr_encoder"),
    ASR_DECODER("asr_decoder"),
    ASR_JOINER("asr_joiner"),
    ASR_TOKENS("asr_tokens"),
    // CAM++: его требует диаризатор sherpa-onnx, наши эмбеддинги считает модель из VoiceChoice.
    EMBEDDING("embedding"),
    REDIMNET2("redimnet2"),
    SEGMENTATION("segmentation"),
    ASR_BPE_VOCAB("asr_bpe_vocab"),
    // Parakeet TDT 0.6B v3: английская речь.
    ASR_EN_ENCODER("asr_en_encoder"),
    ASR_EN_DECODER("asr_en_decoder"),
    ASR_EN_JOINER("asr_en_joiner"),
    ASR_EN_TOKENS("asr_en_tokens"),
    // Whisper tiny: язык записи, русский или английский (SpeechLanguageDetector).
    LID_ENCODER("lid_encoder"),
    LID_DECODER("lid_decoder"),
    LIVE_ENCODER("live_encoder", optional = true),
    LIVE_DECODER("live_decoder", optional = true),
    LIVE_JOINER("live_joiner", optional = true),
    LIVE_TOKENS("live_tokens", optional = true),
    // Kroko: живой текст английской речи.
    LIVE_EN_ENCODER("live_en_encoder", optional = true),
    LIVE_EN_DECODER("live_en_decoder", optional = true),
    LIVE_EN_JOINER("live_en_joiner", optional = true),
    LIVE_EN_TOKENS("live_en_tokens", optional = true),
    // GTCRN: шумоподавление только для VAD, чтобы резать длинные куски (SpeechFinder), распознавание
    // и модель голоса слушают запись. Входит в основной набор; замеры в SpeechFinder.
    DENOISER("denoiser", optional = true),
    // audEERING wav2vec2: её hidden_states читает классификатор «ребёнок или взрослый».
    AGE_GENDER("age_gender", optional = true),
    // GigaAM-Emo: эмоция каждой реплики (TurnEmotions).
    EMOTION("emotion", optional = true),
    ;

    companion object {
        // Файлы живой модели: все вместе или ни одного.
        val LIVE = listOf(LIVE_ENCODER, LIVE_DECODER, LIVE_JOINER, LIVE_TOKENS)
        val LIVE_EN = listOf(LIVE_EN_ENCODER, LIVE_EN_DECODER, LIVE_EN_JOINER, LIVE_EN_TOKENS)

        // Файлы живой модели языка.
        fun live(language: SpeechLanguage): List<ModelRole> =
            when (language) {
                SpeechLanguage.RU -> LIVE
                SpeechLanguage.EN -> LIVE_EN
            }
    }
}
