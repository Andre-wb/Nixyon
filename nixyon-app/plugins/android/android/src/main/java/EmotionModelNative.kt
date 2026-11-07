package dev.demo.nixyon.speech

// GigaAM-Emo в Rust-библиотеке приложения (nixyon-android, emotion_model). Контракт:
// указатель из nativeOpen закрывается ровно одним nativeClose; nativeTurnEmotion и
// nativeClose одной модели не пересекаются. Ошибки приходят исключением
// IllegalStateException.
object EmotionModelNative {
    init {
        System.loadLibrary("nixyon_lib")
    }

    // fd: открытый файл, где с offset начинаются length байт модели (отдельный файл или APK).
    // Дескриптор нужен только на время вызова, вызывающий закрывает его сам.
    external fun nativeOpen(fd: Int, offset: Long, length: Long, threads: Int): Long

    // Модель отдельным файлом на диске: сессия создаётся прямо из файла, без копии в памяти.
    external fun nativeOpenFile(path: String, threads: Int): Long

    // Звук одной реплики, 16 кГц: ключ эмоции из расшифровки ("neutral", "joy", "sadness",
    // "anger") или null, если реплика слишком короткая или модель не уверена.
    external fun nativeTurnEmotion(handle: Long, samples: FloatArray): String?

    external fun nativeClose(handle: Long)
}
