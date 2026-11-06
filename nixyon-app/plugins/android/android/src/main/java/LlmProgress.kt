package dev.demo.nixyon.speech

// Вызывается из C++ в потоке генерации. phase: READING (модель читает встречу,
// done и total в токенах) или WRITING (пишет ответ, total: предел длины ответа).
fun interface LlmProgress {
    fun onProgress(phase: Int, done: Int, total: Int)

    companion object {
        const val READING = 0
        const val WRITING = 1
    }
}
