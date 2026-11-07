package dev.demo.nixyon.speech

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class LlmMemoryTest {
    private val model = 2_293_388_448L

    @Test
    fun fastLoadNeedsTheWholeModelAndTheBuffers() {
        val fast = model + LlmMemory.WORKING_BYTES
        assertEquals(LlmLoad.FAST, LlmMemory.choose(fast, false, model))
        assertEquals(LlmLoad.LOW_MEMORY, LlmMemory.choose(fast - 1, false, model))
        assertEquals(true, LlmLoad.FAST.repack)
    }

    @Test
    fun phoneWithoutRoomForTheCopyLoadsFromTheFile() {
        // 8 ГБ телефон из отчёта: около 2,5 ГБ свободно при закрытых приложениях.
        val phone8gb = 2_500L shl 20
        assertEquals(LlmLoad.LOW_MEMORY, LlmMemory.choose(phone8gb, false, model))
        assertEquals(false, LlmLoad.LOW_MEMORY.repack)
        assertEquals(LlmLoad.LOW_MEMORY, LlmMemory.choose(LlmMemory.WORKING_BYTES, false, model))
        assertNull(LlmMemory.choose(LlmMemory.WORKING_BYTES - 1, false, model))
    }

    @Test
    fun lowMemorySignalFromTheSystemStopsTheModelEvenWithEnoughBytes() {
        assertNull(LlmMemory.choose(8_000L shl 20, true, model))
    }

    @Test
    fun hugeNumbersDoNotOverflow() {
        assertEquals(LlmLoad.LOW_MEMORY, LlmMemory.choose(Long.MAX_VALUE - 1, false, Long.MAX_VALUE))
        assertEquals(LlmLoad.FAST, LlmMemory.choose(Long.MAX_VALUE, false, 0))
    }
}
