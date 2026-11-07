package dev.demo.nixyon.speech

// Ход загрузки моделей в байтах. phase: "downloading", "waiting" (ждёт сети или Wi-Fi),
// "checking" (сверка сумм).
class DownloadProgress(val phase: String, val done: Long, val total: Long) {
    companion object {
        const val DOWNLOADING = "downloading"
        const val WAITING = "waiting"
        const val CHECKING = "checking"
    }
}
