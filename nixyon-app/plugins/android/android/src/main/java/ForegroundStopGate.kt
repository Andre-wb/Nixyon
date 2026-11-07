package dev.demo.nixyon.speech

// Когда можно остановить foreground service. Сервис, запущенный через
// startForegroundService, нельзя останавливать до вызова startForeground: Android тогда
// завершает приложение (ForegroundServiceDidNotStartInTimeException). Ранняя остановка
// только запоминается, и сервис останавливается сам сразу после startForeground.
class ForegroundStopGate {
    private var foreground = false
    private var stopWanted = false

    // Перед запуском сервиса: прошлая просьба об остановке к новому запуску не относится.
    @Synchronized
    fun starting() {
        stopWanted = false
    }

    // Сервис вызвал startForeground. true: остановку уже просили, сервис должен
    // остановиться сам.
    @Synchronized
    fun enteredForeground(): Boolean {
        foreground = true
        return stopWanted
    }

    // Сервис закончил работу.
    @Synchronized
    fun destroyed() {
        foreground = false
    }

    // Просьба остановить. true: сервис уже в foreground, можно звать stopService.
    @Synchronized
    fun requestStop(): Boolean {
        stopWanted = true
        return foreground
    }
}
