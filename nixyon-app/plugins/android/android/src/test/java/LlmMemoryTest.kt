package dev.demo.nixyon.speech

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class LlmMemoryTest {
    private val model = 2_740L shl 20

    @Test
    fun modelRunsOnlyWhenTheWholeFileAndHeadroomFitIntoFreeMemory() {
        assertTrue(LlmMemory.isEnough(model + LlmMemory.HEADROOM_BYTES, false, model))
        assertFalse(LlmMemory.isEnough(model + LlmMemory.HEADROOM_BYTES - 1, false, model))
        assertFalse(LlmMemory.isEnough(1_800L shl 20, false, model))
    }

    @Test
    fun lowMemorySignalFromTheSystemStopsTheModelEvenWithEnoughBytes() {
        assertFalse(LlmMemory.isEnough(8_000L shl 20, true, model))
    }
}
