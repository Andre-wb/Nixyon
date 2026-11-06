package dev.demo.nixyon.speech

// Модель audEERING и вшитый классификатор «ребёнок или взрослый» в Rust-библиотеке
// приложения (nixyon-android, age_model). Контракт: указатель из nativeOpen закрывается
// ровно одним nativeClose; nativeSpeakerVoice и nativeClose одной модели не пересекаются.
// Ошибки приходят исключением IllegalStateException.
object AgeModelNative {
    init {
        System.loadLibrary("nixyon_lib")
    }

    // fd: открытый файл, где с offset начинаются length байт модели (отдельный файл или APK).
    // Дескриптор нужен только на время вызова, вызывающий закрывает его сам.
    external fun nativeOpen(fd: Int, offset: Long, length: Long, threads: Int): Long

    // Модель отдельным файлом на диске: сессия создаётся прямо из файла, без копии в памяти.
    external fun nativeOpenFile(path: String, threads: Int): Long

    // Куски речи одного спикера, 16 кГц: [ребёнок, пол]. Ребёнок: 1 да, 0 взрослый; пол:
    // 1 женский, 0 мужской; -1 ответа нет.
    external fun nativeSpeakerVoice(handle: Long, pieces: Array<FloatArray>): IntArray?

    external fun nativeClose(handle: Long)
}
