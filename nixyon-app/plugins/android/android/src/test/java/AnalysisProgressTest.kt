package dev.demo.nixyon.speech

import org.junit.Assert.assertEquals
import org.junit.Assert.fail
import org.junit.Test
import java.util.concurrent.atomic.AtomicBoolean

class AnalysisProgressTest {
    private var clock = 1_000_000L
    private val sent = mutableListOf<Pair<Double, Double>>()
    private val cancelled = AtomicBoolean(false)
    private val pass = AnalysisProgress(cancelled, { done, total -> sent.add(done to total) }, { clock })

    @Test
    fun progressGoesOutAtMostFourTimesASecondAndAlwaysAtTheEnd() {
        pass.report(1.0, 10.0)
        clock += 100
        pass.report(2.0, 10.0)
        clock += 150
        pass.report(3.0, 10.0)
        clock += 249
        pass.report(4.0, 10.0)
        pass.done(10.0)
        assertEquals(listOf(1.0 to 10.0, 3.0 to 10.0, 10.0 to 10.0), sent)
    }

    @Test
    fun cancellationStopsThePassWithTheCancelledCode() {
        pass.checkCancelled()
        cancelled.set(true)
        try {
            pass.checkCancelled()
            fail("отмена не сработала")
        } catch (e: AnalysisException) {
            assertEquals(AnalysisException.CANCELLED, e.code)
        }
    }
}
