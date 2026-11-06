package dev.demo.nixyon.speech

// Хватит ли памяти на модель: файл модели целиком плюс запас на контекст и рабочие
// буферы llama.cpp. Без запаса система закроет приложение посреди ответа.
object LlmMemory {
    const val HEADROOM_BYTES = 768L shl 20

    fun isEnough(availableBytes: Long, lowMemory: Boolean, modelBytes: Long): Boolean =
        !lowMemory && availableBytes >= modelBytes + HEADROOM_BYTES
}
