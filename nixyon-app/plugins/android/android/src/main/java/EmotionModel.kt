package dev.demo.nixyon.speech

import java.io.Closeable

// GigaAM-Emo на время одного прохода по репликам: 257 МБ, поэтому открывается после
// разбора записи и закрывается сразу после ответа, а не живёт с остальными моделями.
class EmotionModel private constructor(private var handle: Long) : Closeable {
    // Эмоция реплики: ключ из расшифровки или null, если ответа нет.
    fun turnEmotion(samples: FloatArray): String? {
        check(handle != 0L) { "модель эмоций закрыта" }
        return try {
            EmotionModelNative.nativeTurnEmotion(handle, samples)
        } catch (e: IllegalStateException) {
            throw AnalysisException(AnalysisException.ENGINE_FAILED, "модель эмоций: ${e.message}", e)
        }
    }

    override fun close() {
        if (handle != 0L) {
            EmotionModelNative.nativeClose(handle)
            handle = 0L
        }
    }

    companion object {
        // Запас на окно в 8 с и рабочие буферы: внимание conformer на 200 кадрах и
        // активации 16 слоёв по 768. Пик на устройстве замеряется прогоном разбора.
        const val HEADROOM_BYTES = 192L shl 20

        // Хватит ли памяти, чтобы открыть модель. Из APK Rust читает файл модели целиком,
        // ONNX Runtime строит из этих байтов свою копию весов: в пике не меньше двух
        // размеров файла. Из папки модель открывается по пути, второй копии нет.
        fun isEnoughMemory(memory: MemorySnapshot, modelBytes: Long, inApk: Boolean): Boolean {
            val copies = if (inApk) 2 else 1
            return !memory.lowMemory && memory.availableBytes >= copies * modelBytes + HEADROOM_BYTES
        }

        fun open(source: ModelSource, file: ModelFile, threads: Int): EmotionModel {
            val handle = try {
                when (source) {
                    is ModelSource.Folder -> {
                        val path = source.pathOf(file)
                        if (!java.io.File(path).isFile) throw ModelsException(ModelsException.INCOMPLETE, "нет файла ${file.path}")
                        EmotionModelNative.nativeOpenFile(path, threads)
                    }
                    is ModelSource.Apk -> {
                        val (descriptor, offset, length) = VoiceModel.locate(source, file)
                        descriptor.use { EmotionModelNative.nativeOpen(it.fd, offset, length, threads) }
                    }
                }
            } catch (e: IllegalStateException) {
                throw ModelsException(ModelsException.LOAD_FAILED, "модель эмоций не загрузилась: ${e.message}", e)
            }
            if (handle == 0L) {
                throw ModelsException(ModelsException.LOAD_FAILED, "модель эмоций не загрузилась")
            }
            return EmotionModel(handle)
        }
    }
}
