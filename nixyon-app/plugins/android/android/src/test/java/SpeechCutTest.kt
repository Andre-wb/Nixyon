package dev.demo.nixyon.speech

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Test
import java.io.File
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.file.Files

// Образец голоса: речь режется из исходной записи по границам VAD (SpeechFinder.cut), а не
// берётся из VAD, который мог слушать звук после шумоподавления.
class SpeechCutTest {
    private val dir = Files.createTempDirectory("nixyon-cut").toFile()

    // WAV 16 кГц моно, где сэмпл i равен i (по модулю 30000): так видно, какие сэмплы вырезаны.
    private fun wav(samples: Int): File {
        val file = File(dir, "s$samples.wav")
        val data = ByteBuffer.allocate(samples * 2).order(ByteOrder.LITTLE_ENDIAN)
        for (i in 0 until samples) data.putShort((i % 30_000).toShort())
        file.writeBytes(WavPcm16Writer.header(16_000, 1, samples * 2L) + data.array())
        return file
    }

    private fun expected(vararg ranges: Pair<Int, Int>): FloatArray =
        ranges.flatMap { (a, b) -> (a until b).map { (it % 30_000) / 32768f } }.toFloatArray()

    private fun cut(file: File, bounds: List<Pair<Long, Long>>) = WavPcm16Reader(file).use { SpeechFinder.cut(it, bounds) }

    @Test
    fun speechIsJoinedFromTheRecordingInOrder() {
        val file = wav(10_000)
        val got = cut(file, listOf(100L to 700L, 1_590L to 1_610L, 3_200L to 6_500L))
        assertArrayEquals(expected(100 to 700, 1_590 to 1_610, 3_200 to 6_500), got, 0f)
    }

    @Test
    fun pieceAcrossManyChunksAndPiecesInsideOneChunk() {
        val file = wav(20_000)
        val got = cut(file, listOf(10L to 20L, 30L to 40L, 1_000L to 15_000L))
        assertArrayEquals(expected(10 to 20, 30 to 40, 1_000 to 15_000), got, 0f)
    }

    @Test
    fun pieceRunningPastTheEndStopsAtTheEnd() {
        val file = wav(5_000)
        val got = cut(file, listOf(4_000L to 6_000L, 7_000L to 8_000L))
        assertArrayEquals(expected(4_000 to 5_000), got, 0f)
    }

    @Test
    fun overlapIsTakenOnce() {
        val file = wav(5_000)
        val got = cut(file, listOf(100L to 300L, 200L to 400L))
        assertArrayEquals(expected(100 to 400), got, 0f)
    }

    @Test
    fun noSpeechGivesNothing() {
        assertEquals(0, cut(wav(3_000), emptyList()).size)
        assertEquals(0, cut(wav(3_000), listOf(500L to 500L)).size)
    }
}
