package dev.demo.nixyon.speech

// Куда встать плееру, когда играются только отрезки записи.
object PlaybackSpans {
    // spans по возрастанию начала; null: играется вся запись, позиция не меняется. Позиция
    // внутри отрезка остаётся, в промежутке переходит к началу следующего. null в ответе:
    // отрезков после позиции не осталось.
    fun target(spans: List<PlaybackSpan>?, positionSec: Double): Double? {
        if (spans == null) return positionSec
        val next = spans.firstOrNull { it.end > positionSec } ?: return null
        return maxOf(positionSec, next.start)
    }
}
