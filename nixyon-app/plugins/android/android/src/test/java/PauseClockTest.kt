package dev.demo.nixyon.speech

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class PauseClockTest {
    @Test
    fun pausesAddUpAndTheCurrentOneCountsUntilNow() {
        val clock = PauseClock()
        clock.pause(1_000)
        clock.resume(4_000)
        clock.pause(10_000)
        assertTrue(clock.isPaused)
        assertEquals(5_000, clock.pausedMs(12_000))
        clock.resume(12_000)
        assertFalse(clock.isPaused)
        assertEquals(5_000, clock.pausedMs(20_000))
    }

    @Test
    fun repeatedPauseOrResumeWithoutPauseChangesNothing() {
        val clock = PauseClock()
        clock.resume(5_000)
        assertEquals(0, clock.pausedMs(6_000))
        clock.pause(1_000)
        clock.pause(3_000)
        clock.resume(4_000)
        assertEquals(3_000, clock.pausedMs(9_000))
    }
}
