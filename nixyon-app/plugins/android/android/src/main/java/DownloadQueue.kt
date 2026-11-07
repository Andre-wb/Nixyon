package dev.demo.nixyon.speech

// Очередь загрузок файлов: в приложении это системный DownloadManager, в тестах подделка.
interface DownloadQueue {
    fun enqueue(request: DownloadRequest): Long

    // Состояние загрузок по номерам; загрузки, которой больше нет в очереди, нет и в ответе.
    fun query(ids: Collection<Long>): Map<Long, DownloadStatus>

    // Отменяет загрузки и удаляет их файлы, если они ещё лежат на месте.
    fun remove(ids: Collection<Long>)
}

// subPath: путь файла относительно getExternalFilesDir(null).
class DownloadRequest(val url: String, val subPath: String, val title: String, val allowMetered: Boolean)

// total: размер по ответу сервера, -1 пока неизвестен.
class DownloadStatus(val state: State, val downloaded: Long, val total: Long) {
    enum class State {
        RUNNING,
        // Ждёт сети или Wi-Fi.
        WAITING,
        DONE,
        FAILED,
    }
}
