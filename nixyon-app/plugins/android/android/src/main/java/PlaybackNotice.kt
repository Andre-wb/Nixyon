package dev.demo.nixyon.speech

// Что делать с уведомлением проигрывания. Сервис запускается только когда звук пошёл: пауза
// открытой, но ни разу не игравшей записи (например, при сворачивании) уведомление не
// показывает. Если сервис уже держит уведомление, оно обновляется.
enum class PlaybackNotice {
    START,
    UPDATE,
    NONE,
    ;

    companion object {
        fun choose(active: Boolean, playing: Boolean): PlaybackNotice = when {
            active -> UPDATE
            playing -> START
            else -> NONE
        }
    }
}
