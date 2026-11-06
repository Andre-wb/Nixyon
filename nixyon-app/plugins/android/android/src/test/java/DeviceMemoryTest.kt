package dev.demo.nixyon.speech

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

// Перед большой моделью память проверяется, а модели речи отпускаются, только если её не
// хватает (DeviceMemory.check).
class DeviceMemoryTest {
    private val need = 1_000L
    private val enough = { m: MemorySnapshot -> !m.lowMemory && m.availableBytes >= need }

    private fun check(vararg snapshots: MemorySnapshot, freed: Boolean): Triple<MemorySnapshot, Boolean, Pair<Int, Int>> {
        var reads = 0
        var frees = 0
        val (memory, didFree) = DeviceMemory.check({ snapshots[reads++] }, enough, { frees++; freed })
        return Triple(memory, didFree, reads to frees)
    }

    @Test
    fun enoughMemoryReleasesNothing() {
        val plenty = MemorySnapshot(need, false)
        val (memory, freed, calls) = check(plenty, freed = true)
        assertSame(plenty, memory)
        assertFalse(freed)
        assertEquals(1 to 0, calls)
    }

    @Test
    fun whenNothingCanBeReleasedTheFirstLookStands() {
        val short = MemorySnapshot(need - 1, false)
        val (memory, freed, calls) = check(short, freed = false)
        assertSame(short, memory)
        assertFalse(freed)
        assertEquals(1 to 1, calls)
    }

    @Test
    fun afterReleasingTheMemoryIsLookedAtAgain() {
        val after = MemorySnapshot(need + 500, false)
        val (memory, freed, calls) = check(MemorySnapshot(10, true), after, freed = true)
        assertSame(after, memory)
        assertTrue(freed)
        assertEquals(2 to 1, calls)
        assertTrue(enough(memory))
    }

    @Test
    fun theAgeModelNeedsItsFileTwiceFromTheApkAndOnceFromAFolder() {
        val model = 363L shl 20
        val fromApk = 2 * model + AgeModel.HEADROOM_BYTES
        val fromFolder = model + AgeModel.HEADROOM_BYTES
        assertTrue(AgeModel.isEnoughMemory(MemorySnapshot(fromApk, false), model, inApk = true))
        assertFalse(AgeModel.isEnoughMemory(MemorySnapshot(fromApk - 1, false), model, inApk = true))
        assertTrue(AgeModel.isEnoughMemory(MemorySnapshot(fromFolder, false), model, inApk = false))
        assertFalse(AgeModel.isEnoughMemory(MemorySnapshot(fromFolder - 1, false), model, inApk = false))
        assertFalse(AgeModel.isEnoughMemory(MemorySnapshot(8_000L shl 20, true), model, inApk = false))
    }
}
