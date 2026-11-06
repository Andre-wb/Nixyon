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
}
