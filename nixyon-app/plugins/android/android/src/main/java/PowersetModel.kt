package dev.demo.nixyon.speech

import java.io.Closeable
import java.io.File
import java.util.concurrent.atomic.AtomicBoolean

// Модель сегментации pyannote напрямую, без обработки sherpa-onnx: по ней ищутся перекрытия
// голосов, которые sherpa теряет при сборке отрезков. Файл модели открывается только на
// время загрузки: Rust читает байты и держит модель у себя до close.
class PowersetModel private constructor(private var handle: Long) : Closeable {

    // Перекрытия по записи в файл request.out. Ход первого прохода идёт в progress;
    // отмена бросает AnalysisException.CANCELLED, как остальные проходы по записи.
    fun scan(
        wav: File,
        request: OverlapRequest,
        cancelled: AtomicBoolean,
        progress: (doneSec: Double) -> Unit,
    ) {
        check(handle != 0L) { "модель сегментации закрыта" }
        val done = try {
            PowersetModelNative.nativeScan(
                handle,
                wav.path,
                request.threshold,
                request.minSec,
                request.out.path,
            ) { doneSec ->
                progress(doneSec)
                !cancelled.get()
            }
        } catch (e: IllegalStateException) {
            throw AnalysisException(AnalysisException.ENGINE_FAILED, "модель сегментации: ${e.message}", e)
        }
        if (!done) {
            if (cancelled.get()) throw AnalysisException(AnalysisException.CANCELLED, "обработка отменена")
            throw AnalysisException(AnalysisException.ENGINE_FAILED, "модель сегментации прервала обход")
        }
    }

    override fun close() {
        if (handle != 0L) {
            PowersetModelNative.nativeClose(handle)
            handle = 0L
        }
    }

    companion object {
        fun open(source: ModelSource, file: ModelFile, threads: Int): PowersetModel {
            val (descriptor, offset, length) = VoiceModel.locate(source, file)
            val handle = descriptor.use {
                try {
                    PowersetModelNative.nativeOpen(it.fd, offset, length, threads)
                } catch (e: IllegalStateException) {
                    throw ModelsException(ModelsException.LOAD_FAILED, "модель сегментации не загрузилась: ${e.message}", e)
                }
            }
            if (handle == 0L) {
                throw ModelsException(ModelsException.LOAD_FAILED, "модель сегментации не загрузилась")
            }
            return PowersetModel(handle)
        }
    }
}
