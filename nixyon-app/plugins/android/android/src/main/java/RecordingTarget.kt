package dev.demo.nixyon.speech

import java.io.File

// Куда рекордер пишет запись: временный `.part` во время записи, затем готовый файл.
// В папке хранится только последний готовый файл: keep делает `.part` готовым и стирает
// прежние, а оборванный `.part` прежний файл не трогает.
abstract class RecordingTarget(val dir: File) {
    fun newPart(prefix: String): File {
        dir.mkdirs()
        return File(dir, "$prefix-${System.currentTimeMillis()}.wav.part")
    }

    fun keep(part: File): File {
        val done = File(dir, part.name.removeSuffix(".part"))
        if (!part.renameTo(done)) throw MediaException(MediaException.IMPORT_FAILED, "не удалось сохранить ${part.name}")
        dir.listFiles()?.filter { it != done }?.forEach { it.delete() }
        return done
    }

    // Для несуществующей папки Android сообщает 0 свободных байт: папка создаётся до проверки.
    fun hasRoom(): Boolean {
        dir.mkdirs()
        return dir.usableSpace >= MIN_FREE_BYTES
    }

    companion object {
        const val MIN_FREE_BYTES = 200L * 1024 * 1024
    }
}
