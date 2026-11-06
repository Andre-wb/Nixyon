package dev.demo.nixyon.speech

import android.os.SystemClock
import java.util.concurrent.atomic.AtomicBoolean

// Отмена и ход прохода по записи (SpeechAnalyzer, SpeechSegmenter). Ход уходит в интерфейс
// не чаще раза в REPORT_EVERY_MS, первый сразу, а в конце (done) всегда. now: часы в мс,
// подменяются в тестах.
class AnalysisProgress(
    private val cancelled: AtomicBoolean,
    private val progress: (doneSec: Double, totalSec: Double) -> Unit,
    private val now: () -> Long = SystemClock::elapsedRealtime,
) {
    private var reported = 0L

    fun checkCancelled() {
        if (cancelled.get()) throw AnalysisException(AnalysisException.CANCELLED, "обработка отменена")
    }

    fun report(doneSec: Double, totalSec: Double) {
        val time = now()
        if (time - reported >= REPORT_EVERY_MS) {
            progress(doneSec, totalSec)
            reported = time
        }
    }

    fun done(totalSec: Double) = progress(totalSec, totalSec)

    companion object {
        const val REPORT_EVERY_MS = 250L
    }
}
