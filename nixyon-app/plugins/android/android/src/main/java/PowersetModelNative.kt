package dev.demo.nixyon.speech

// Модель сегментации pyannote в Rust-библиотеке приложения (nixyon-android, powerset_model).
// Контракт: указатель из nativeOpen закрывается ровно одним nativeClose; nativeScan и
// nativeClose одной модели не пересекаются. Ошибки приходят исключением
// IllegalStateException; если бросить его не вышло, приходит 0 или false.
object PowersetModelNative {
    init {
        System.loadLibrary("nixyon_lib")
    }

    // fd: открытый файл, где с offset начинаются length байт модели (отдельный файл или APK).
    // Дескриптор нужен только на время вызова, вызывающий закрывает его сам.
    external fun nativeOpen(fd: Int, offset: Long, length: Long, threads: Int): Long

    // Перекрытия по WAV 16 кГц в файл out. progress.keepGoing зовётся после каждого окна
    // первого прохода; false отменяет обход. true: файл записан; false: обход отменён.
    external fun nativeScan(
        handle: Long,
        wavPath: String,
        threshold: Float,
        minSec: Double,
        outPath: String,
        progress: OverlapScanProgress,
    ): Boolean

    external fun nativeClose(handle: Long)
}

// Ход первого прохода поиска перекрытий; false: обход отменить.
fun interface OverlapScanProgress {
    fun keepGoing(doneSec: Double): Boolean
}
