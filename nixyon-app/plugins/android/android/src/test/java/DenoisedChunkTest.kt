package dev.demo.nixyon.speech

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

class DenoisedChunkTest {
    @Test
    fun emptyAnswerOfAShortTailIsSkippedWhateverItsRate() {
        assertFalse(DenoisedChunk.usable(0, 0))
        assertFalse(DenoisedChunk.usable(0, SpeechModels.SAMPLE_RATE))
    }

    @Test
    fun soundIsHeardOnlyAtTheModelRate() {
        assertTrue(DenoisedChunk.usable(256, SpeechModels.SAMPLE_RATE))
        val error = assertThrows(AnalysisException::class.java) { DenoisedChunk.usable(256, 8_000) }
        assertEquals(AnalysisException.ENGINE_FAILED, error.code)
    }
}
