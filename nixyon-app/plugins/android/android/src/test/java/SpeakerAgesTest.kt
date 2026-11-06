package dev.demo.nixyon.speech

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Test
import java.io.File
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.file.Files

class SpeakerAgesTest {
    private val dir = Files.createTempDirectory("nixyon-ages").toFile()

    // WAV, где сэмпл i равен i: так видно, какие сэмплы попали в кусок.
    private fun wav(samples: Int): File {
        val file = File(dir, "w$samples.wav")
        val data = ByteBuffer.allocate(samples * 2).order(ByteOrder.LITTLE_ENDIAN)
        for (i in 0 until samples) data.putShort(i.toShort())
        file.writeBytes(WavPcm16Writer.header(16_000, 1, samples * 2L) + data.array())
        return file
    }

    private fun expected(from: Int, to: Int) = FloatArray(to - from) { (from + it) / 32768f }

    @Test
    fun secondsBecomeSampleBoundsInOrderAndEmptyPiecesAreDropped() {
        val bounds = SpeakerAges.bounds(listOf(AgePiece(0.5, 1.0), AgePiece(0.0, 0.25), AgePiece(2.0, 2.0), AgePiece(-1.0, 0.0)))
        assertEquals(listOf(0L to 4_000L, 8_000L to 16_000L), bounds)
    }

    @Test
    fun eachPieceIsReadSeparatelyFromItsPlaceInTheFile() {
        val pieces = WavPcm16Reader(wav(20_000)).use { SpeakerAges.readPieces(it, listOf(100L to 300L, 1_000L to 1_500L)) }
        assertEquals(2, pieces.size)
        assertArrayEquals(expected(100, 300), pieces[0], 0f)
        assertArrayEquals(expected(1_000, 1_500), pieces[1], 0f)
    }

    @Test
    fun anOverlapIsNotReadTwiceAndThePastTheEndPieceIsCut() {
        val pieces = WavPcm16Reader(wav(2_000)).use { SpeakerAges.readPieces(it, listOf(0L to 1_000L, 500L to 1_200L, 1_900L to 5_000L)) }
        assertEquals(3, pieces.size)
        assertArrayEquals(expected(1_000, 1_200), pieces[1], 0f)
        assertArrayEquals(expected(1_900, 2_000), pieces[2], 0f)
    }
}
