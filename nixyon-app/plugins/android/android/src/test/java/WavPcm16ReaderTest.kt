package dev.demo.nixyon.speech

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.fail
import org.junit.Test
import java.io.File
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.file.Files

class WavPcm16ReaderTest {
    private val dir = Files.createTempDirectory("nixyon-wav").toFile()

    // WAV, где сэмпл i равен i (по модулю 30000): так видно, какие сэмплы прочитаны.
    private fun wav(samples: Int, rate: Int = 16_000): File {
        val file = File(dir, "w$samples-$rate.wav")
        val data = ByteBuffer.allocate(samples * 2).order(ByteOrder.LITTLE_ENDIAN)
        for (i in 0 until samples) data.putShort((i % 30_000).toShort())
        file.writeBytes(WavPcm16Writer.header(rate, 1, samples * 2L) + data.array())
        return file
    }

    private fun expected(from: Int, to: Int) = FloatArray(to - from) { ((from + it) % 30_000) / 32768f }

    @Test
    fun skipAndReadWalkTheFileAcrossChunks() {
        WavPcm16Reader(wav(10_000)).use { reader ->
            assertEquals(10_000L, reader.totalSamples)
            assertEquals(1_700L, reader.skipSamples(1_700))
            assertArrayEquals(expected(1_700, 5_000), reader.readSamples(3_300), 0f)
            assertEquals(0L, reader.skipSamples(0))
            assertArrayEquals(expected(5_000, 5_001), reader.readSamples(1), 0f)
        }
    }

    @Test
    fun theEndOfTheFileShortensReadsAndSkips() {
        WavPcm16Reader(wav(3_000)).use { reader ->
            assertEquals(2_000L, reader.skipSamples(2_000))
            assertArrayEquals(expected(2_000, 3_000), reader.readSamples(5_000), 0f)
            assertEquals(0, reader.readSamples(10).size)
            assertEquals(0L, reader.skipSamples(10))
            assertNull(reader.next(10))
        }
    }

    @Test
    fun onlySixteenKilohertzMonoIsAccepted() {
        try {
            WavPcm16Reader(wav(100, rate = 48_000)).close()
            fail("48 кГц принят")
        } catch (e: AnalysisException) {
            assertEquals(AnalysisException.BAD_AUDIO, e.code)
        }
    }
}
