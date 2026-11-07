package dev.demo.nixyon.speech

// Сколько запись простояла на паузе. Время передаётся снаружи, поэтому класс проверяется
// тестами без часов Android. Повторная пауза или продолжение без паузы ничего не меняют.
class PauseClock {
    private var since: Long? = null
    private var total = 0L

    val isPaused: Boolean
        @Synchronized get() = since != null

    @Synchronized
    fun pause(now: Long) {
        if (since == null) since = now
    }

    @Synchronized
    fun resume(now: Long) {
        val start = since ?: return
        total += (now - start).coerceAtLeast(0)
        since = null
    }

    // Всё время пауз к моменту now, вместе с идущей.
    @Synchronized
    fun pausedMs(now: Long): Long = total + (since?.let { (now - it).coerceAtLeast(0) } ?: 0)
}
