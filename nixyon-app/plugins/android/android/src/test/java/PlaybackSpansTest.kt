package dev.demo.nixyon.speech

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class PlaybackSpansTest {
    private val spans = listOf(PlaybackSpan(1.0, 2.0), PlaybackSpan(5.0, 6.0))

    @Test
    fun withoutSpansThePositionStays() {
        assertEquals(3.5, PlaybackSpans.target(null, 3.5)!!, 0.0)
    }

    @Test
    fun insideASpanThePositionStays() {
        assertEquals(1.5, PlaybackSpans.target(spans, 1.5)!!, 0.0)
        assertEquals(5.0, PlaybackSpans.target(spans, 5.0)!!, 0.0)
    }

    @Test
    fun inAGapThePlayerJumpsToTheNextSpan() {
        assertEquals(1.0, PlaybackSpans.target(spans, 0.0)!!, 0.0)
        assertEquals(5.0, PlaybackSpans.target(spans, 2.0)!!, 0.0)
        assertEquals(5.0, PlaybackSpans.target(spans, 3.0)!!, 0.0)
    }

    @Test
    fun afterTheLastSpanNothingIsLeft() {
        assertNull(PlaybackSpans.target(spans, 6.0))
        assertNull(PlaybackSpans.target(emptyList(), 0.0))
    }
}
