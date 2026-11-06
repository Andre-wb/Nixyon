package dev.demo.nixyon.speech

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.concurrent.CountDownLatch
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger

// Модели речи отпускаются, только когда их никто не держит (ModelSlot).
class ModelSlotTest {
    private val released = mutableListOf<String>()
    private val slot = ModelSlot<String> { released += it }

    @Test
    fun unloadReleasesTheLoadedModelsOnce() {
        slot.put("модели")
        assertTrue(slot.unload())
        assertEquals(listOf("модели"), released)
        assertNull(slot.current)
        assertFalse(slot.unload())
        assertEquals(1, released.size)
    }

    @Test
    fun heldModelsStayUntilEveryHolderIsDone() {
        slot.put("модели")
        val first = slot.hold()!!
        val second = slot.hold()!!
        assertEquals("модели", first.models)
        assertFalse(slot.unload())
        first.done()
        first.done()
        assertFalse(slot.unload())
        assertEquals("модели", slot.current)
        second.done()
        assertTrue(slot.unload())
        assertEquals(listOf("модели"), released)
    }

    @Test
    fun anEmptySlotHasNothingToHoldOrUnload() {
        assertNull(slot.hold())
        assertFalse(slot.unload())
        slot.put("новые")
        assertEquals("новые", slot.hold()!!.models)
        assertTrue(released.isEmpty())
    }

    @Test
    fun modelsAreNeverReleasedWhileSomeoneHoldsThem() {
        val gone = AtomicBoolean(false)
        val shared = ModelSlot<String> { gone.set(true) }
        shared.put("модели")
        val misuse = AtomicInteger(0)
        val start = CountDownLatch(1)
        val holders = (1..4).map {
            Thread {
                start.await()
                repeat(20_000) {
                    val hold = shared.hold() ?: return@repeat
                    if (gone.get()) misuse.incrementAndGet()
                    hold.done()
                }
            }.apply { start() }
        }
        start.countDown()
        while (!shared.unload()) Thread.yield()
        holders.forEach { it.join() }
        assertTrue(gone.get())
        assertEquals(0, misuse.get())
        assertNull(shared.hold())
    }
}
