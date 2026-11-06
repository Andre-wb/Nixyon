package dev.demo.nixyon.speech

import kotlin.math.roundToLong

// Окна для эмбеддингов голоса внутри куска речи. Формула та же, что у WindowSpec::spans
// в nixyon-core: окна с шагом, последнее прижато к концу куска, кусок короче окна
// окон не получает. Ноутбук и телефон должны давать одинаковые окна.
class WindowSpans(windowSec: Double, stepSec: Double, rate: Int) {
    private val window = maxOf(1, (windowSec * rate).roundToLong().toInt())
    private val stride = maxOf(1, (stepSec * rate).roundToLong().toInt())

    // Пары (начало, конец) в сэмплах от начала куска.
    fun of(length: Int): List<Pair<Int, Int>> {
        if (length < window) return emptyList()
        val spans = (0..length - window step stride).map { it to it + window }.toMutableList()
        if (spans.last().second < length) spans.add(length - window to length)
        return spans
    }

    companion object {
        // Длина и шаг приходят из Rust; 0 значит «окна не нужны». Шаг длиннее окна
        // оставил бы дыры между окнами, такой запрос отклоняется.
        fun fromRequest(windowSec: Double, stepSec: Double, rate: Int): WindowSpans? {
            if (windowSec == 0.0 && stepSec == 0.0) return null
            val ok = windowSec.isFinite() && stepSec.isFinite() && windowSec > 0 && stepSec > 0 && stepSec <= windowSec
            if (!ok) throw AnalysisException(AnalysisException.ENGINE_FAILED, "окна: длина $windowSec с, шаг $stepSec с")
            return WindowSpans(windowSec, stepSec, rate)
        }
    }
}
