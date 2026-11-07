package dev.demo.nixyon.speech

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class CpuCoresTest {
    @get:Rule
    val folder = TemporaryFolder()

    @Test
    fun littleCoresAreNotCountedAsFast() {
        // 1 X3, 2 A715, 2 A710, 3 A510: экономичные слабее половины X3.
        assertEquals(5, CpuCores.fastCount(listOf(325, 325, 325, 778, 778, 845, 845, 1024)))
        // 4 X4 и 4 A720: все сильнее половины.
        assertEquals(8, CpuCores.fastCount(listOf(1024, 1024, 1024, 1024, 700, 700, 700, 700)))
        assertEquals(8, CpuCores.fastCount(List(8) { 1024 }))
    }

    @Test
    fun unknownCapacityGivesNoCount() {
        assertNull(CpuCores.fastCount(null))
        assertNull(CpuCores.fastCount(emptyList()))
        assertNull(CpuCores.fastCount(listOf(1024, 0)))
    }

    @Test
    fun threadsNeverDropBelowTheOldRuleAndStopAtEight() {
        // Как раньше: 4 и меньше быстрых ядер или силы ядер неизвестны.
        assertEquals(4, CpuCores.threads(8, 2))
        assertEquals(4, CpuCores.threads(8, 4))
        assertEquals(4, CpuCores.threads(8, null))
        assertEquals(2, CpuCores.threads(2, 2))
        assertEquals(1, CpuCores.threads(0, null))
        // Больше быстрых ядер: столько же потоков, не больше 8 и не больше ядер.
        assertEquals(5, CpuCores.threads(8, 5))
        assertEquals(8, CpuCores.threads(8, 8))
        assertEquals(8, CpuCores.threads(12, 12))
        assertEquals(6, CpuCores.threads(6, 9))
    }

    @Test
    fun capacitiesAreReadPerCoreAndAnyGapMeansUnknown() {
        val root = folder.newFolder("cpu")
        listOf(400, 1024).forEachIndexed { i, c ->
            root.resolve("cpu$i").apply { mkdirs() }.resolve("cpu_capacity").writeText("$c\n")
        }
        assertEquals(listOf(400, 1024), CpuCores.capacities(root, 2))
        assertNull(CpuCores.capacities(root, 3))
        root.resolve("cpu1/cpu_capacity").writeText("много")
        assertNull(CpuCores.capacities(root, 2))
    }

    @Test
    fun featuresComeFromTheCpuinfoFeaturesLine() {
        val cpuinfo = "processor\t: 0\nBogoMIPS\t: 38.40\n" +
            "Features\t: fp asimd evtstrm aes pmull sha1 sha2 crc32 atomics fphp asimdhp asimddp i8mm bf16 sve sve2\n"
        assertEquals("asimddp i8mm sve sve2 bf16 asimdhp", CpuCores.features(cpuinfo))
        assertEquals("базовые", CpuCores.features("Features\t: fp asimd\n"))
        assertEquals("?", CpuCores.features(null))
        assertEquals("?", CpuCores.features("processor\t: 0\n"))
    }
}
