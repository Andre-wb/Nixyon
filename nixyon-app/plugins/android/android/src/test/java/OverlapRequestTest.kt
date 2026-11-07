package dev.demo.nixyon.speech

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.fail
import org.junit.Test

class OverlapRequestTest {
    @Test
    fun withoutAFileNoSearchIsNeeded() {
        assertNull(OverlapRequest.of(null, 0f, 0.0))
    }

    @Test
    fun validRequestKeepsItsValues() {
        val request = OverlapRequest.of("/tmp/o.json", 0.3f, 0.3)!!
        assertEquals("/tmp/o.json", request.out.path)
        assertEquals(0.3f, request.threshold, 0f)
        assertEquals(0.3, request.minSec, 0.0)
    }

    @Test
    fun thresholdOutsideZeroToOneOrBrokenLengthIsAnEngineError() {
        for ((threshold, minSec) in listOf(0f to 0.3, 1f to 0.3, Float.NaN to 0.3, 0.3f to -1.0, 0.3f to Double.NaN)) {
            try {
                OverlapRequest.of("/tmp/o.json", threshold, minSec)
                fail("порог $threshold, длина $minSec")
            } catch (e: AnalysisException) {
                assertEquals(AnalysisException.ENGINE_FAILED, e.code)
            }
        }
    }
}
