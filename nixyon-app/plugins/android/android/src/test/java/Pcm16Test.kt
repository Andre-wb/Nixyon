package dev.demo.nixyon.speech

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Test

class Pcm16Test {
    @Test
    fun samplesBecomeFloatsWithTheSameDivisorAsSherpa() {
        assertEquals(-1f, Pcm16.toFloat(Short.MIN_VALUE), 0f)
        assertEquals(0f, Pcm16.toFloat(0), 0f)
        assertEquals(32767f / 32768f, Pcm16.toFloat(Short.MAX_VALUE), 0f)
    }

    @Test
    fun bytesAreLittleEndianAndEverySampleSurvivesARoundTrip() {
        val bytes = ByteArray(2)
        Pcm16.put(bytes, 0, 0x1234)
        assertArrayEquals(byteArrayOf(0x34, 0x12), bytes)
        for (v in Short.MIN_VALUE..Short.MAX_VALUE) {
            Pcm16.put(bytes, 0, v)
            assertEquals(v.toShort(), Pcm16.get(bytes, 0))
        }
    }

    @Test
    fun decoderFloatsAreClippedToTheSampleRange() {
        assertEquals(32767, Pcm16.fromFloat(1f))
        assertEquals(-32767, Pcm16.fromFloat(-1f))
        assertEquals(32767, Pcm16.fromFloat(2.5f))
        assertEquals(-32767, Pcm16.fromFloat(-2.5f))
        assertEquals(16383, Pcm16.fromFloat(0.5f))
        assertEquals(0, Pcm16.fromFloat(0f))
    }

    private fun bytes(vararg samples: Int) = ByteArray(samples.size * 2).also { b ->
        samples.forEachIndexed { i, v -> Pcm16.put(b, i * 2, v) }
    }

    private fun samples(b: ByteArray, length: Int) = (0 until length / 2).map { Pcm16.get(b, it * 2).toInt() }

    @Test
    fun channelsAreDownmixedToMonoByTheirMean() {
        val stereo = bytes(100, 300, -32768, -32768, 32767, -32767, 1, 2)
        val length = Pcm16.downmix(stereo, stereo.size, 2)
        assertEquals(listOf(200, -32768, 0, 1), samples(stereo, length))

        val three = bytes(3, 6, 9, -3, -3, -3)
        assertEquals(listOf(6, -3), samples(three, Pcm16.downmix(three, three.size, 3)))

        // Неполный кадр в конце отбрасывается, моно не меняется.
        val cut = bytes(10, 20, 30)
        assertEquals(listOf(15), samples(cut, Pcm16.downmix(cut, cut.size, 2)))
        val mono = bytes(5, 7)
        assertEquals(4, Pcm16.downmix(mono, mono.size, 1))
        assertEquals(listOf(5, 7), samples(mono, 4))
    }
}
