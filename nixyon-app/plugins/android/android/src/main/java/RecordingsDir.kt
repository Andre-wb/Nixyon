package dev.demo.nixyon.speech

import java.io.File
import java.io.IOException
import javax.crypto.SecretKey

// Исходные записи: в P0 хранится только последняя. Новая запись или импорт пишется во
// временный файл и только после успешного окончания становится последней, а прежние
// удаляются; оборванная запись прежнюю не трогает.
class RecordingsDir(filesDir: File) : RecordingTarget(File(filesDir, "recordings")) {
    // Записи с микрофона, оборванные падением приложения: восстанавливаются (SealedRecovery)
    // и становятся готовыми записями. Прежнюю запись восстановленная не стирает: её ещё не
    // расшифровали, и пользователь не решил, нужна ли она; следующая запись или импорт
    // сотрут обе, как обычно. Звать до подготовки хранилища ядром (vault/prepare.rs): она
    // удаляет все `.part`. Невосстановимые файлы и оборванный импорт остаются ядру на
    // удаление: исходный файл импорта и так есть у пользователя. Ответ: восстановленные
    // записи от старых к новым.
    fun recoverParts(key: SecretKey): List<RecordingResult> {
        val parts = dir.listFiles { f -> f.isFile && f.name.startsWith("$RECORDING_PREFIX-") && f.name.endsWith(".wav.part") }
            .orEmpty()
            .sortedBy { it.name.removePrefix("$RECORDING_PREFIX-").removeSuffix(".wav.part").toLongOrNull() ?: 0L }
        return parts.mapNotNull { part ->
            val duration = try {
                SealedRecovery.recoverWav(part, key)
            } catch (_: IOException) {
                null
            } ?: return@mapNotNull null
            val done = File(dir, part.name.removeSuffix(".part"))
            if (part.renameTo(done)) RecordingResult(done.absolutePath, duration, CRASHED) else null
        }
    }

    companion object {
        const val MAX_SECONDS = 3 * 60 * 60

        // Префикс файлов записей с микрофона; у импорта свой, "imp".
        const val RECORDING_PREFIX = "rec"

        // Причина остановки восстановленной записи: приложение упало или его закрыла система.
        const val CRASHED = "crashed"
    }
}
