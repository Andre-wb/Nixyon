package dev.demo.nixyon.speech

// Номера уведомлений foreground service. Уведомления с одним номером затирают друг друга, а
// остановка одного сервиса убирает уведомление другого, поэтому номера разные.
object NotificationIds {
    const val RECORDING = 1
    const val MODEL_DOWNLOAD = 2
    const val PLAYBACK = 3

    val all = listOf(RECORDING, MODEL_DOWNLOAD, PLAYBACK)
}
