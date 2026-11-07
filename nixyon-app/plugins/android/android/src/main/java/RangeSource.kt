package dev.demo.nixyon.speech

// Источник байтов файла по адресу: кусок от start до end включительно. Прочитанное
// отдаётся в sink по мере чтения. Ошибки приходят как RangeException.
interface RangeSource {
    fun read(url: String, start: Long, end: Long, total: Long, sink: (ByteArray, Int) -> Unit)
}

class RangeException(val kind: Kind, message: String, cause: Throwable? = null) : Exception(message, cause) {
    enum class Kind {
        // Обрыв связи, ошибка сервера: кусок качается заново.
        TRANSIENT,
        // Сервер занят (429): кусок качается заново после паузы.
        BUSY,
        // Файла нет на зеркале: повтор не поможет.
        MISSING,
        // Сервер отдал не тот кусок или не той длины.
        WRONG,
    }
}
