package dev.demo.nixyon.speech

// Можно ли писать звук дальше, когда микрофон заглушён или отвалился.
//
// Звонок или другое приложение заглушают микрофон: Android 10 и новее вместо ошибки отдаёт
// тишину и сообщает об этом (AudioRecordingConfiguration.isClientSilenced). Тогда запись
// останавливается и сохраняется то, что успели записать; паузы нет. Заглушение необратимо:
// микрофон, вернувшийся после звонка, остановку не отменяет.
//
// Чтение микрофона вернуло ошибку (перезапуск аудиосервера, ERROR_DEAD_OBJECT): рекордер
// открывает микрофон заново, пока с первой ошибки не прошло reopenMs. Ошибка считается
// ушедшей, когда микрофон снова отдал звук.
//
// Время в мс от любого монотонного начала (SystemClock.elapsedRealtime на телефоне).
// Заглушение приходит на главном потоке, остальное на потоке записи, поэтому методы
// синхронизированы.
class MicGate(private val reopenMs: Long) {
    enum class Chunk { WRITE, STOP }

    private var silenced = false
    private var lostSince = NEVER
    private var reopens = 0

    @Synchronized
    fun silenced(silenced: Boolean) {
        if (silenced) this.silenced = true
    }

    // Микрофон отдал кусок звука: писать его или остановить запись.
    @Synchronized
    fun chunk(): Chunk {
        lostSince = NEVER
        return if (silenced) Chunk.STOP else Chunk.WRITE
    }

    // Чтение вернуло ошибку или микрофон ещё не открылся заново. true: пробовать открыть
    // снова; false: время вышло, запись останавливается.
    @Synchronized
    fun lost(now: Long): Boolean {
        if (lostSince == NEVER) lostSince = now
        return now - lostSince < reopenMs
    }

    @Synchronized
    fun reopened() {
        reopens += 1
    }

    // Сколько раз микрофон открывался заново: для строки в логе.
    @Synchronized
    fun reopens(): Int = reopens

    private companion object {
        const val NEVER = Long.MIN_VALUE
    }
}

// Размер буфера микрофона в байтах, по порядку попыток. Сначала 2 с звука: если поток записи
// задержится (запись в файл, сборка мусора, нагрузка от живого текста), звук ждёт в буфере, а
// не теряется. Если такой буфер устройство не даёт, прежний: 0,2 с, но не меньше минимума.
object MicBuffer {
    const val SECONDS = 2
    private const val FALLBACK_FRACTION = 5
    private const val BYTES_PER_SAMPLE = 2

    fun sizes(rate: Int, minBytes: Int): List<Int> =
        listOf(
            maxOf(minBytes, rate * BYTES_PER_SAMPLE * SECONDS),
            maxOf(minBytes, rate / FALLBACK_FRACTION * BYTES_PER_SAMPLE),
        ).distinct()
}
