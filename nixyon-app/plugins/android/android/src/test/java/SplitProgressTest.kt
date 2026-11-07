package dev.demo.nixyon.speech

import org.junit.Assert.assertEquals
import org.junit.Test

class SplitProgressTest {
    private val sent = mutableListOf<Pair<Double, Double>>()

    @Test
    fun twoPassesFillTheirOwnHalvesSoTheBarNeverGoesBack() {
        val first = SplitProgress(0, 2) { done, total -> sent.add(done to total) }
        val second = SplitProgress(1, 2) { done, total -> sent.add(done to total) }
        first.report(0.0, 10.0)
        first.report(10.0, 10.0)
        second.report(0.0, 10.0)
        second.report(10.0, 10.0)
        assertEquals(listOf(0.0 to 10.0, 5.0 to 10.0, 5.0 to 10.0, 10.0 to 10.0), sent)
    }

    @Test
    fun singlePassKeepsTheWholeBarAndDoneIsClampedToTheTotal() {
        val only = SplitProgress(0, 1) { done, total -> sent.add(done to total) }
        only.report(4.0, 10.0)
        only.report(12.0, 10.0)
        assertEquals(listOf(4.0 to 10.0, 10.0 to 10.0), sent)
    }
}
