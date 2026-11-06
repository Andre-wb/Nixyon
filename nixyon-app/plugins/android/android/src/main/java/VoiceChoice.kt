package dev.demo.nixyon.speech

// Модель голоса приложения и порог, с которым её проверяли замером: при смене модели они
// меняются вместе. Порог: косинус центров двух групп, от которого группы считаются одним
// человеком, когда число спикеров неизвестно; 0,7 лучший для ReDimNet2 B6 с pyannote.
object VoiceChoice {
    val ROLE = ModelRole.REDIMNET2
    const val SAME_SPEAKER_COSINE = 0.7
}
