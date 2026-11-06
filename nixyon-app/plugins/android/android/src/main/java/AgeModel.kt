package dev.demo.nixyon.speech

import java.io.Closeable

// Модель audEERING на время одного разбора: 363 МБ, поэтому открывается после того, как
// спикеры сложены, и закрывается сразу после ответа, а не живёт с остальными моделями.
class AgeModel private constructor(private var handle: Long) : Closeable {
    // Голос спикера: ребёнок ли и женский ли; null, если ответа нет.
    fun speakerVoice(pieces: List<FloatArray>): Pair<Boolean?, Boolean?> {
        check(handle != 0L) { "модель audEERING закрыта" }
        val answer = try {
            AgeModelNative.nativeSpeakerVoice(handle, pieces.toTypedArray())
        } catch (e: IllegalStateException) {
            throw AnalysisException(AnalysisException.ENGINE_FAILED, "модель audEERING: ${e.message}", e)
        } ?: throw AnalysisException(AnalysisException.ENGINE_FAILED, "модель audEERING не дала ответа")
        if (answer.size != 2) throw AnalysisException(AnalysisException.ENGINE_FAILED, "модель audEERING: ответ из ${answer.size} чисел")
        return yesNo(answer[0]) to yesNo(answer[1])
    }

    override fun close() {
        if (handle != 0L) {
            AgeModelNative.nativeClose(handle)
            handle = 0L
        }
    }

    companion object {
        private const val YES = 1
        private const val NO = 0
        // Запас на окно в 10 с и рабочие буферы. Не замерялся: пик на телефоне показывает
        // age_gender_bench (scripts/bench-age-gender-android.sh, строка «память: пик»).
        const val HEADROOM_BYTES = 256L shl 20

        fun yesNo(value: Int): Boolean? =
            when (value) {
                YES -> true
                NO -> false
                else -> null
            }

        // Хватит ли памяти, чтобы открыть модель. Из APK Rust читает файл модели целиком,
        // ONNX Runtime строит из этих байтов свою копию весов, и байты отпускаются только
        // после неё: в пике не меньше двух размеров файла. Из папки модель открывается по
        // пути (open), второй копии нет.
        fun isEnoughMemory(memory: MemorySnapshot, modelBytes: Long, inApk: Boolean): Boolean {
            val copies = if (inApk) 2 else 1
            return !memory.lowMemory && memory.availableBytes >= copies * modelBytes + HEADROOM_BYTES
        }

        // Из папки модель открывается по пути: на загрузке в памяти нет второй копии файла,
        // пик меньше примерно на её размер. Из APK путь к файлу не получить, там байты.
        fun open(source: ModelSource, file: ModelFile, threads: Int): AgeModel {
            val handle = try {
                when (source) {
                    is ModelSource.Folder -> {
                        val path = source.pathOf(file)
                        if (!java.io.File(path).isFile) throw ModelsException(ModelsException.INCOMPLETE, "нет файла ${file.path}")
                        AgeModelNative.nativeOpenFile(path, threads)
                    }
                    is ModelSource.Apk -> {
                        val (descriptor, offset, length) = VoiceModel.locate(source, file)
                        descriptor.use { AgeModelNative.nativeOpen(it.fd, offset, length, threads) }
                    }
                }
            } catch (e: IllegalStateException) {
                throw ModelsException(ModelsException.LOAD_FAILED, "модель audEERING не загрузилась: ${e.message}", e)
            }
            if (handle == 0L) {
                throw ModelsException(ModelsException.LOAD_FAILED, "модель audEERING не загрузилась")
            }
            return AgeModel(handle)
        }
    }
}
