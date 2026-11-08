package dev.demo.nixyon.speech

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

// Звук для определения языка живого текста (LiveLanguageClip).
class LiveLanguageClipTest {
    private val rate = 16_000

    // Кусок 0,1 с постоянной громкости: амплитуда 0,1 даёт -20 дБ, 0,001 даёт -60 дБ.
    private fun chunk(amplitude: Float, value: Float = amplitude) = FloatArray(rate / 10) { if (it % 2 == 0) value else -value }

    @Test
    fun loudChunksAreCollectedUntilTenSecondsAndGivenOnce() {
        val clip = LiveLanguageClip(rate)
        val needed = (LiveLanguageClip.NEEDED_SEC * 10).toInt()
        repeat(needed - 1) { assertNull(clip.add(chunk(0.1f))) }
        val heard = clip.add(chunk(0.1f, 0.2f))
        assertNotNull(heard)
        assertEquals(rate * 10, heard!!.size)
        assertEquals(0.2f, heard.first { it == 0.2f }, 0f)
        assertNull(clip.add(chunk(0.1f)))
    }

    @Test
    fun quietChunksDoNotCount() {
        val clip = LiveLanguageClip(rate)
        repeat(500) { assertNull(clip.add(chunk(0.001f))) }
        assertNull(clip.add(FloatArray(0)))
        repeat((LiveLanguageClip.NEEDED_SEC * 10).toInt() - 1) { assertNull(clip.add(chunk(0.1f))) }
        assertNotNull(clip.add(chunk(0.1f)))
    }

    @Test
    fun loudnessIsMeasuredInDecibelsOfFullScale() {
        assertEquals(-20f, LiveLanguageClip.dbfs(chunk(0.1f)), 0.01f)
        assertEquals(-60f, LiveLanguageClip.dbfs(chunk(0.001f)), 0.01f)
        assertTrue(LiveLanguageClip.dbfs(FloatArray(10)).isInfinite())
    }
}
