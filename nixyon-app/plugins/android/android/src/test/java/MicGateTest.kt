package dev.demo.nixyon.speech

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class MicGateTest {
    private val gate = MicGate(reopenMs = 10_000)

    @Test
    fun soundIsWrittenWhileTheMicrophoneIsNotSilenced() {
        assertEquals(MicGate.Chunk.WRITE, gate.chunk())
        assertEquals(MicGate.Chunk.WRITE, gate.chunk())
    }

    @Test
    fun callStopsTheRecording() {
        assertEquals(MicGate.Chunk.WRITE, gate.chunk())
        gate.silenced(true)
        assertEquals(MicGate.Chunk.STOP, gate.chunk())
    }

    @Test
    fun microphoneReturnedAfterTheCallDoesNotResumeTheRecording() {
        gate.silenced(true)
        gate.silenced(false)
        assertEquals(MicGate.Chunk.STOP, gate.chunk())
    }

    @Test
    fun recordingStartedDuringACallStopsOnTheFirstChunk() {
        gate.silenced(true)
        assertEquals(MicGate.Chunk.STOP, gate.chunk())
    }

    @Test
    fun unsilencedReportChangesNothing() {
        gate.silenced(false)
        assertEquals(MicGate.Chunk.WRITE, gate.chunk())
    }

    @Test
    fun lostMicrophoneIsReopenedUntilTheTimeRunsOut() {
        assertTrue(gate.lost(1_000))
        assertTrue(gate.lost(5_000))
        assertTrue(gate.lost(10_999))
        assertFalse(gate.lost(11_000))
    }

    @Test
    fun soundAfterReopeningClearsTheError() {
        assertTrue(gate.lost(1_000))
        gate.reopened()
        assertEquals(MicGate.Chunk.WRITE, gate.chunk())
        // Новая ошибка через минуту получает свои 10 с.
        assertTrue(gate.lost(60_000))
        assertTrue(gate.lost(69_000))
        gate.reopened()
        assertEquals(2, gate.reopens())
    }

    @Test
    fun microphoneThatFailsRightAfterReopeningDoesNotGetEndlessTries() {
        assertTrue(gate.lost(0))
        gate.reopened()
        // Открылся, но первое же чтение снова ошибка: время считается с первой ошибки.
        assertTrue(gate.lost(6_000))
        gate.reopened()
        assertFalse(gate.lost(10_000))
    }

    @Test
    fun bufferHoldsTwoSecondsWithTheOldSizeAsFallback() {
        assertEquals(listOf(64_000, 6_400), MicBuffer.sizes(16_000, 1_280))
        assertEquals(listOf(192_000, 19_200), MicBuffer.sizes(48_000, 3_840))
        assertEquals(listOf(176_400, 17_640), MicBuffer.sizes(44_100, 3_528))
    }

    @Test
    fun bufferIsNeverBelowTheDeviceMinimum() {
        assertEquals(listOf(100_000), MicBuffer.sizes(16_000, 100_000))
        assertEquals(listOf(64_000, 10_000), MicBuffer.sizes(16_000, 10_000))
    }
}
