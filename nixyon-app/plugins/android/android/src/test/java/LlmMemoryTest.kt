package dev.demo.nixyon.speech

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class LlmMemoryTest {
    private val model = 2_740L shl 20

    @Test
    fun modelRunsWhenLlamaBuffersFitEvenIfTheWholeFileDoesNot() {
        assertTrue(LlmMemory.isEnough(LlmMemory.WORKING_BYTES, false))
        assertFalse(LlmMemory.isEnough(LlmMemory.WORKING_BYTES - 1, false))
        // Телефон с 8 ГБ, приложения закрыты: свободно около 2,5 ГБ, меньше файла модели.
        val phone8gb = 2_500L shl 20
        assertTrue(phone8gb < model)
        assertTrue(LlmMemory.isEnough(phone8gb, false))
    }

    @Test
    fun lowMemorySignalFromTheSystemStopsTheModelEvenWithEnoughBytes() {
        assertFalse(LlmMemory.isEnough(8_000L shl 20, true))
    }
}
