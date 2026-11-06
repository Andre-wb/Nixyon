package dev.demo.nixyon.speech

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

// Те же случаи, что у padded_bounds в nixyon-sherpa: телефон и ноутбук режут одинаково.
class PaddedBoundsTest {
    @Test
    fun loneRegionGetsTheFullPadInsideTheRecording() {
        assertEquals(listOf(700L to 2_300L), paddedBounds(listOf(1_000L to 2_000L), 10_000, 300))
    }

    @Test
    fun padStopsAtTheEdgesOfTheRecording() {
        assertEquals(listOf(0L to 10_000L), paddedBounds(listOf(100L to 9_900L), 10_000, 300))
    }

    @Test
    fun neighboursSplitAShortGapInTheMiddleAndNeverOverlap() {
        assertEquals(
            listOf(700L to 2_100L, 2_100L to 3_300L),
            paddedBounds(listOf(1_000L to 2_000L, 2_200L to 3_000L), 10_000, 300),
        )
    }

    @Test
    fun aWideGapLeavesTheFullPadOnBothSides() {
        assertEquals(
            listOf(700L to 2_300L, 4_700L to 6_300L),
            paddedBounds(listOf(1_000L to 2_000L, 5_000L to 6_000L), 10_000, 300),
        )
    }

    @Test
    fun touchingOrOverlappingRegionsKeepTheirVadEdgesBetweenThem() {
        assertEquals(
            listOf(700L to 2_000L, 1_900L to 3_300L),
            paddedBounds(listOf(1_000L to 2_000L, 1_900L to 3_000L), 10_000, 300),
        )
    }

    @Test
    fun noRegionsGiveNoBounds() {
        assertTrue(paddedBounds(emptyList(), 10_000, 300).isEmpty())
    }
}
