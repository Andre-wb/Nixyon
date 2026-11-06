package dev.demo.nixyon.speech

import android.os.ParcelFileDescriptor
import java.io.Closeable
import java.io.File
import java.io.FileNotFoundException
import java.io.IOException

// Загруженная модель голоса. Файл открывается только на время загрузки: Rust читает
// байты модели и держит её у себя до close.
class VoiceModel private constructor(private var handle: Long) : Closeable {
    val dim: Int = VoiceModelNative.nativeDim(handle)

    // Сколько раз и сколько времени считались эмбеддинги: для строки замера в логе.
    var calls: Long = 0
        private set
    var nanos: Long = 0
        private set

    // Эмбеддинг куска 16 кГц; пустой массив, если кусок слишком короткий для модели.
    fun embed(samples: FloatArray): FloatArray {
        check(handle != 0L) { "модель голоса закрыта" }
        val started = System.nanoTime()
        try {
            return VoiceModelNative.nativeEmbed(handle, samples)
                ?: throw AnalysisException(AnalysisException.ENGINE_FAILED, "модель голоса не дала ответа")
        } catch (e: IllegalStateException) {
            throw AnalysisException(AnalysisException.ENGINE_FAILED, "модель голоса: ${e.message}", e)
        } finally {
            calls++
            nanos += System.nanoTime() - started
        }
    }

    override fun close() {
        if (handle != 0L) {
            VoiceModelNative.nativeClose(handle)
            handle = 0L
        }
    }

    companion object {
        fun open(source: ModelSource, file: ModelFile, threads: Int): VoiceModel {
            val (descriptor, offset, length) = locate(source, file)
            val handle = descriptor.use {
                try {
                    VoiceModelNative.nativeOpen(it.fd, offset, length, threads)
                } catch (e: IllegalStateException) {
                    throw ModelsException(ModelsException.LOAD_FAILED, "модель голоса не загрузилась: ${e.message}", e)
                }
            }
            if (handle == 0L) {
                throw ModelsException(ModelsException.LOAD_FAILED, "модель голоса не загрузилась")
            }
            return try {
                VoiceModel(handle)
            } catch (e: IllegalStateException) {
                VoiceModelNative.nativeClose(handle)
                throw ModelsException(ModelsException.LOAD_FAILED, "модель голоса: ${e.message}", e)
            }
        }

        // Дескриптор, смещение и длина модели. В APK модель лежит несжатой: openFd работает
        // только с такими файлами, сжатый даёт ошибку здесь, а не неверные байты.
        // Так же открывается модель audEERING (AgeModel).
        internal fun locate(source: ModelSource, file: ModelFile): Triple<ParcelFileDescriptor, Long, Long> =
            when (source) {
                is ModelSource.Folder -> {
                    val descriptor = try {
                        ParcelFileDescriptor.open(File(source.dir, file.path), ParcelFileDescriptor.MODE_READ_ONLY)
                    } catch (e: FileNotFoundException) {
                        throw ModelsException(ModelsException.INCOMPLETE, "нет файла ${file.path}", e)
                    }
                    Triple(descriptor, 0L, file.size)
                }
                is ModelSource.Apk -> {
                    val asset = try {
                        source.assetManager.openFd(source.pathOf(file))
                    } catch (e: IOException) {
                        throw ModelsException(ModelsException.CORRUPT, "модель голоса в APK не открылась: ${e.message}", e)
                    }
                    if (asset.length != file.size) {
                        asset.close()
                        throw ModelsException(ModelsException.CORRUPT, "размер ${file.path} в APK ${asset.length}, ожидался ${file.size}")
                    }
                    Triple(asset.parcelFileDescriptor, asset.startOffset, asset.length)
                }
            }
    }
}
