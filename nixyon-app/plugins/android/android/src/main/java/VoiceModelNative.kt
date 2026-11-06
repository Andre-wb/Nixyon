package dev.demo.nixyon.speech

// Модель голоса ReDimNet2 в Rust-библиотеке приложения (nixyon-android, voice_model).
// Контракт: указатель из nativeOpen закрывается ровно одним nativeClose; nativeEmbed,
// nativeDim и nativeClose одной модели не пересекаются. Ошибки приходят исключением
// IllegalStateException; если бросить его не вышло, приходит 0 или null.
object VoiceModelNative {
    init {
        System.loadLibrary("nixyon_lib")
    }

    // fd: открытый файл, где с offset начинаются length байт модели (отдельный файл или APK).
    // Дескриптор нужен только на время вызова, вызывающий закрывает его сам.
    external fun nativeOpen(fd: Int, offset: Long, length: Long, threads: Int): Long

    // Звук 16 кГц; пустой массив, если кусок слишком короткий для модели.
    external fun nativeEmbed(handle: Long, samples: FloatArray): FloatArray?

    external fun nativeDim(handle: Long): Int

    external fun nativeClose(handle: Long)
}
