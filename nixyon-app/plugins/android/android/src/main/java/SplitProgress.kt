package dev.demo.nixyon.speech

// Одна фаза из нескольких проходов по записи: каждый проход занимает свою равную долю
// полосы, поэтому полоса не откатывается назад, когда начинается следующий проход.
// part: номер прохода с нуля, parts: сколько их.
class SplitProgress(
    private val part: Int,
    private val parts: Int,
    private val progress: (doneSec: Double, totalSec: Double) -> Unit,
) {
    fun report(doneSec: Double, totalSec: Double) {
        val share = totalSec / parts
        progress(part * share + doneSec.coerceIn(0.0, totalSec) / parts, totalSec)
    }
}
