package dev.demo.nixyon.speech

import org.junit.Assert.assertEquals
import org.junit.Test

// Куски VAD по исходному звуку, а длинные куски (в шуме VAD склеивает в них речь нескольких
// людей) заменяются кусками VAD после шумоподавления внутри них (SpeechFinder.splitLong).
class LongPiecesTest {
    private fun split(plain: List<Pair<Long, Long>>, clean: List<Pair<Long, Long>>) =
        SpeechFinder.splitLong(plain, clean, LONG)

    @Test
    fun withoutLongPiecesThePlainPiecesStayAsTheyAre() {
        val plain = listOf(0L to 100L, 150L to 249L, 300L to 320L)
        assertEquals(plain, split(plain, listOf(10L to 20L, 160L to 170L, 400L to 500L)))
        assertEquals(plain, split(plain, emptyList()))
    }

    @Test
    fun aLongPieceIsReplacedByTheCleanPiecesInsideItCutAtItsEdges() {
        val plain = listOf(0L to 50L, 100L to 400L, 450L to 500L)
        val clean = listOf(10L to 40L, 90L to 150L, 200L to 250L, 380L to 460L, 470L to 480L)
        assertEquals(listOf(0L to 50L, 100L to 150L, 200L to 250L, 380L to 400L, 450L to 500L), split(plain, clean))
    }

    @Test
    fun aLongPieceWithoutCleanSpeechIsDropped() {
        assertEquals(listOf(500L to 520L), split(listOf(0L to 300L, 500L to 520L), listOf(310L to 400L, 505L to 510L)))
    }

    @Test
    fun aCleanPieceAcrossTwoLongPiecesGoesToBoth() {
        val plain = listOf(0L to 250L, 260L to 600L)
        val clean = listOf(100L to 120L, 200L to 300L, 590L to 700L)
        assertEquals(listOf(100L to 120L, 200L to 250L, 260L to 300L, 590L to 600L), split(plain, clean))
    }

    private companion object {
        const val LONG = 250L
    }
}
