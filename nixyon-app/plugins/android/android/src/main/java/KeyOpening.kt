package dev.demo.nixyon.speech

// Что делать с ключом данных при открытии приложения.
enum class KeyOpening {
    // Отпечатка на телефоне нет и запертого им ключа нет: ключ открывается без запроса.
    PLAIN,

    // Ключ заперт отпечатком и цел: спросить палец и развернуть.
    BIOMETRIC,

    // Есть прежняя обёртка без отпечатка: перенести ключ на новую, спросив палец.
    MIGRATE,

    // Ключа ещё нет: создать новый под отпечатком.
    CREATE,

    // Ключ заперт отпечатком, а на телефоне сейчас нет отпечатка.
    UNAVAILABLE,

    // Ключ заперт отпечатком, но система его стёрла (снимали блокировку экрана) или файл
    // испорчен. Новый ключ молча не создаётся: старые файлы под ним ядро удалит.
    LOST,
}

// Состояние ключа на телефоне, из которого выбирается действие.
data class KeyState(
    val canAuthenticate: Boolean,
    val bioFile: Boolean,
    val bioFileValid: Boolean,
    val bioKey: Boolean,
    val bioKeyInvalidated: Boolean,
    val legacyFile: Boolean,
)

object KeyOpenings {
    fun choose(s: KeyState): KeyOpening {
        if (!s.canAuthenticate) return if (s.bioFile) KeyOpening.UNAVAILABLE else KeyOpening.PLAIN
        if (!s.bioFile) return if (s.legacyFile) KeyOpening.MIGRATE else KeyOpening.CREATE
        if (s.bioFileValid && s.bioKey && !s.bioKeyInvalidated) return KeyOpening.BIOMETRIC
        // Прежняя обёртка могла остаться, если перенос оборвался: ключ из неё ещё цел.
        return if (s.legacyFile) KeyOpening.MIGRATE else KeyOpening.LOST
    }
}
