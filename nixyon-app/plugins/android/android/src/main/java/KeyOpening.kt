package dev.demo.nixyon.speech

// Что делать с ключом данных при открытии приложения. Вход по отпечатку включается в
// настройках: без этого ключ обёрнут обычным ключом Keystore и открывается без запроса.
enum class KeyOpening {
    // Вход по отпечатку выключен: ключ открывается без запроса, при первом запуске создаётся.
    PLAIN,

    // Ключ заперт отпечатком и цел: спросить палец и развернуть.
    BIOMETRIC,

    // Ключ заперт отпечатком, но запертая обёртка не открывается, а прежняя без отпечатка
    // цела: запереть ключ из неё заново, спросив палец.
    MIGRATE,

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
        // Под отпечаток ключ попадает только по выбору в настройках, сам не переносится.
        if (!s.bioFile) return KeyOpening.PLAIN
        if (!s.canAuthenticate) return KeyOpening.UNAVAILABLE
        if (s.bioFileValid && s.bioKey && !s.bioKeyInvalidated) return KeyOpening.BIOMETRIC
        return if (s.legacyFile) KeyOpening.MIGRATE else KeyOpening.LOST
    }
}
