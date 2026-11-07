package dev.demo.nixyon.speech

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File
import java.util.zip.ZipEntry
import java.util.zip.ZipOutputStream

class LlmGpuTest {
    @get:Rule
    val folder = TemporaryFolder()

    @Test
    fun gpuOnlyWhenSwitchedOnBuiltAndThereIsMemoryForTheWholeModel() {
        assertEquals(GpuChoice.OFF, LlmGpu.choose(false, true, LlmLoad.FAST))
        assertEquals(GpuChoice.NOT_BUILT, LlmGpu.choose(true, false, LlmLoad.FAST))
        assertEquals(GpuChoice.LOW_MEMORY, LlmGpu.choose(true, true, LlmLoad.LOW_MEMORY))
        assertEquals(GpuChoice.USE, LlmGpu.choose(true, true, LlmLoad.FAST))
        assertEquals(listOf(GpuChoice.USE), GpuChoice.entries.filter { it.wanted })
    }

    @Test
    fun switchIsOffByDefaultAndRemembered() {
        val marker = File(folder.root, LlmGpu.MARKER)
        val gpu = LlmGpu(marker) { true }
        assertFalse(gpu.enabled)
        assertEquals(GpuChoice.OFF, gpu.choose(LlmLoad.FAST))
        gpu.setEnabled(true)
        assertTrue(LlmGpu(marker) { true }.enabled)
        assertEquals(GpuChoice.USE, gpu.choose(LlmLoad.FAST))
        gpu.setEnabled(false)
        gpu.setEnabled(false)
        assertFalse(gpu.enabled)
    }

    @Test
    fun buildCheckRunsOnce() {
        var calls = 0
        val gpu = LlmGpu(File(folder.root, LlmGpu.MARKER)) { calls++; false }
        assertFalse(gpu.isBuilt)
        assertFalse(gpu.isBuilt)
        assertEquals(1, calls)
    }

    @Test
    fun libraryIsFoundUnpackedOrInsideTheApk() {
        val libs = folder.newFolder("lib")
        assertFalse(LlmGpu.hasLibrary(libs, emptyList()))
        File(libs, LlmGpu.LIBRARY).writeText("elf")
        assertTrue(LlmGpu.hasLibrary(libs, emptyList()))

        val withGpu = apk("with.apk", "lib/arm64-v8a/libggml-vulkan.so", "lib/arm64-v8a/libnixyon_llm.so")
        val withoutGpu = apk("without.apk", "lib/arm64-v8a/libnixyon_llm.so", "assets/libggml-vulkan.so")
        val notZip = folder.newFile("broken.apk").apply { writeText("не zip") }
        val missing = File(folder.root, "missing.apk")
        assertTrue(LlmGpu.hasLibrary(null, listOf(withoutGpu, withGpu)))
        assertFalse(LlmGpu.hasLibrary(null, listOf(withoutGpu, notZip, missing)))
        assertFalse(LlmGpu.hasLibrary(File(folder.root, "none"), emptyList()))
    }

    private fun apk(name: String, vararg entries: String): File {
        val file = File(folder.root, name)
        ZipOutputStream(file.outputStream()).use { zip ->
            for (entry in entries) {
                zip.putNextEntry(ZipEntry(entry))
                zip.write(1)
                zip.closeEntry()
            }
        }
        return file
    }
}
