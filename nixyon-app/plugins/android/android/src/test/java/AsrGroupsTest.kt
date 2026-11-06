package dev.demo.nixyon.speech

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class AsrGroupsTest {
    @Test
    fun piecesWithShortPausesShareAGroup() {
        val bounds = listOf(0L to 1_000L, 1_500L to 3_000L, 3_200L to 4_000L)
        assertEquals(listOf(bounds), asrGroups(bounds, maxLength = 10_000, maxGap = 1_000))
    }

    @Test
    fun aLongPauseStartsANewGroup() {
        val bounds = listOf(0L to 1_000L, 2_500L to 3_000L)
        assertEquals(
            listOf(listOf(0L to 1_000L), listOf(2_500L to 3_000L)),
            asrGroups(bounds, maxLength = 10_000, maxGap = 1_000),
        )
    }

    @Test
    fun aGroupNeverGrowsPastMaxLength() {
        val bounds = listOf(0L to 4_000L, 4_100L to 8_000L, 8_100L to 12_000L)
        assertEquals(
            listOf(listOf(0L to 4_000L, 4_100L to 8_000L), listOf(8_100L to 12_000L)),
            asrGroups(bounds, maxLength = 10_000, maxGap = 1_000),
        )
    }

    @Test
    fun aPieceLongerThanMaxLengthStaysAlone() {
        val bounds = listOf(0L to 500L, 600L to 12_000L, 12_100L to 12_500L)
        assertEquals(
            listOf(listOf(0L to 500L), listOf(600L to 12_000L), listOf(12_100L to 12_500L)),
            asrGroups(bounds, maxLength = 10_000, maxGap = 1_000),
        )
    }

    @Test
    fun withGroupsOffEveryPieceIsRecognizedAlone() {
        val bounds = listOf(0L to 1_000L, 1_500L to 3_000L)
        assertEquals(
            listOf(listOf(0L to 1_000L), listOf(1_500L to 3_000L)),
            recognitionGroups(bounds, grouped = false, maxLength = 10_000, maxGap = 1_000),
        )
        assertEquals(
            listOf(bounds),
            recognitionGroups(bounds, grouped = true, maxLength = 10_000, maxGap = 1_000),
        )
    }

    @Test
    fun noPiecesGiveNoGroups() {
        assertTrue(asrGroups(emptyList(), maxLength = 10_000, maxGap = 1_000).isEmpty())
    }

    @Test
    fun wordsGoToThePieceWhereTheyStartWithTimesFromThatPiece() {
        val tokens = arrayOf("▁При", "вет", ",", "▁как", "▁дела", "?")
        val times = floatArrayOf(0.0f, 0.1f, 0.3f, 1.5f, 1.8f, 2.1f)
        val parts = splitByPieces(tokens, times, listOf(0.0 to 0.6, 1.2 to 2.4))!!
        assertEquals(listOf("Привет,", "как дела?"), parts.map { it.text })
        assertArrayEquals(arrayOf("▁При", "вет", ","), parts[0].tokens)
        assertArrayEquals(floatArrayOf(0.0f, 0.1f, 0.3f), parts[0].times, 1e-6f)
        assertArrayEquals(floatArrayOf(0.3f, 0.6f, 0.9f), parts[1].times, 1e-6f)
    }

    @Test
    fun aClosingMarkStaysWithTheWordBeforeItEvenPastTheCut() {
        val tokens = arrayOf("▁да", ".", "▁Нет")
        val times = floatArrayOf(0.1f, 1.0f, 1.5f)
        val parts = splitByPieces(tokens, times, listOf(0.0 to 0.5, 0.9 to 2.0))!!
        assertEquals(listOf("да.", "Нет"), parts.map { it.text })
        // Знак позже конца своего куска: время прижато к концу куска.
        assertArrayEquals(floatArrayOf(0.1f, 0.5f), parts[0].times, 1e-6f)
    }

    @Test
    fun aWordIsNeverCutBetweenItsTokens() {
        val tokens = arrayOf("▁пре", "крас", "но", "▁всё")
        val times = floatArrayOf(0.4f, 0.8f, 1.0f, 1.3f)
        val parts = splitByPieces(tokens, times, listOf(0.0 to 0.6, 0.8 to 1.6))!!
        assertEquals(listOf("прекрасно", "всё"), parts.map { it.text })
    }

    @Test
    fun letterTokensWithSpaceTokensSplitOnTheSpace() {
        // Так отдаёт GigaAM v3 без пунктуации: буквы и пробел отдельным токеном.
        val tokens = arrayOf("д", "а", " ", "н", "е", "т")
        val times = floatArrayOf(0.0f, 0.04f, 0.3f, 1.0f, 1.04f, 1.08f)
        val parts = splitByPieces(tokens, times, listOf(0.0 to 0.5, 0.9 to 1.5))!!
        assertEquals(listOf("да", "нет"), parts.map { it.text })
        assertArrayEquals(arrayOf("н", "е", "т"), parts[1].tokens)
    }

    @Test
    fun anOpeningMarkAndTheWordAfterItStayTogether() {
        // Слово начинается на открывающем знаке: кусок выбирается по времени знака.
        val tokens = arrayOf("▁Он", "▁сказал", ":", "▁«", "Ура", "»")
        val pieces = listOf(0.0 to 0.55, 0.85 to 1.4)
        val early = splitByPieces(tokens, floatArrayOf(0.0f, 0.2f, 0.5f, 0.6f, 0.8f, 1.1f), pieces)!!
        assertEquals(listOf("Он сказал: «Ура»", ""), early.map { it.text })
        val late = splitByPieces(tokens, floatArrayOf(0.0f, 0.2f, 0.5f, 0.75f, 0.8f, 1.1f), pieces)!!
        assertEquals(listOf("Он сказал:", "«Ура»"), late.map { it.text })
    }

    @Test
    fun aPieceWithoutWordsGetsEmptyText() {
        val tokens = arrayOf("▁да", "▁нет")
        val times = floatArrayOf(0.1f, 0.3f)
        val parts = splitByPieces(tokens, times, listOf(0.0 to 0.5, 1.0 to 1.5))!!
        assertEquals(listOf("да нет", ""), parts.map { it.text })
        assertEquals(0, parts[1].tokens.size)
    }

    @Test
    fun withoutTokenTimesThereIsNothingToSplitBy() {
        assertNull(splitByPieces(arrayOf("▁да"), FloatArray(0), listOf(0.0 to 1.0, 2.0 to 3.0)))
    }
}
