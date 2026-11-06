package dev.demo.nixyon.speech

// Прослойка к llama.cpp (src/main/cpp/meeting_llm.cpp). Контракт: указатель из
// nativeOpen закрывается ровно одним nativeClose; nativeGenerate и nativeClose одной
// сессии не пересекаются; nativeCancel можно звать из любого потока до nativeClose.
// Ошибки приходят исключением LlmException.
object MeetingLlmNative {
    init {
        System.loadLibrary("nixyon_llm")
    }

    // fd: открытый файл, где с offset начинается модель (отдельный файл или APK).
    // Дескриптор копируется, вызывающий закрывает свой сам.
    external fun nativeOpen(fd: Int, offset: Long, contextTokens: Int, threads: Int): Long

    // Ответ в UTF-8: из байтов строка собирается в Kotlin, JNI не умеет символы вне BMP.
    external fun nativeGenerate(handle: Long, system: String, user: String, maxTokens: Int, progress: LlmProgress): ByteArray

    external fun nativeCancel(handle: Long)

    external fun nativeClose(handle: Long)
}
