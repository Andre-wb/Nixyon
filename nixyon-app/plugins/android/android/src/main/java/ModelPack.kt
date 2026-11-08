package dev.demo.nixyon.speech

// Что пользователь выбирает на экране загрузки моделей: обязательный набор речи и
// необязательные модели. key уходит в интерфейс и приходит обратно.
enum class ModelPack(val key: String, val required: Boolean = false) {
    SPEECH("speech", required = true),
    LIVE("live"),
    DENOISER("denoiser"),
    AGE("age"),
    EMOTION("emotion"),
    LLM("llm"),
    ;

    companion object {
        fun fromKey(key: String): ModelPack? = entries.firstOrNull { it.key == key }

        // В какой пакет входит роль речевого набора: обязательные роли в набор речи,
        // необязательные каждая в свой.
        fun of(role: ModelRole): ModelPack =
            when (role) {
                in ModelRole.LIVE, in ModelRole.LIVE_EN -> LIVE
                ModelRole.DENOISER -> DENOISER
                ModelRole.AGE_GENDER -> AGE
                ModelRole.EMOTION -> EMOTION
                else -> SPEECH
            }
    }
}
