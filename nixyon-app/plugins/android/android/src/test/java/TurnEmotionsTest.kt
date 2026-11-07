package dev.demo.nixyon.speech

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.file.Files

class TurnEmotionsTest {
    private val dir = Files.createTempDirectory("nixyon-emotions").toFile()

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
    fun turnsGoInRecordingOrderAndAnEmptyTurnHasNoBounds() {
        val ordered = TurnEmotions.ordered(listOf(EmotionTurn(1, 0.5, 1.0), EmotionTurn(2, 0.0, 0.25), EmotionTurn(3, 2.0, 2.0)))
        assertEquals(listOf(2, 3, 1), ordered.map { it.first.id })
        assertEquals(0L to 4_000L, ordered[0].second)
        assertNull(ordered[1].second)
        assertEquals(8_000L to 16_000L, ordered[2].second)
    }

    @Test
    fun eachTurnIsReadWholeEvenWhenTurnsOverlap() {
        TurnAudioReader(wav(20_000)).use { reader ->
            assertArrayEquals(expected(100, 1_000), reader.read(100L to 1_000L), 0f)
            assertArrayEquals(expected(500, 1_200), reader.read(500L to 1_200L), 0f)
            assertArrayEquals(expected(5_000, 5_100), reader.read(5_000L to 5_100L), 0f)
        }
    }

    @Test
    fun aTurnPastTheEndIsCutOrEmpty() {
        TurnAudioReader(wav(2_000)).use { reader ->
            assertArrayEquals(expected(1_900, 2_000), reader.read(1_900L to 5_000L), 0f)
            assertEquals(0, reader.read(3_000L to 4_000L).size)
        }
    }

    @Test
    fun theModelOpensOnlyWithTwoCopiesOfItsFileFromTheApkAndOneFromAFolder() {
        val model = 100L shl 20
        val room = { free: Long -> MemorySnapshot(availableBytes = free, lowMemory = false) }
        val one = model + EmotionModel.HEADROOM_BYTES
        assertTrue(EmotionModel.isEnoughMemory(room(one), model, inApk = false))
        assertFalse(EmotionModel.isEnoughMemory(room(one), model, inApk = true))
        assertTrue(EmotionModel.isEnoughMemory(room(one + model), model, inApk = true))
        assertFalse(EmotionModel.isEnoughMemory(MemorySnapshot(availableBytes = one * 4, lowMemory = true), model, inApk = false))
    }
}
