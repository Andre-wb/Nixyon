package dev.demo.nixyon.speech

import dev.demo.nixyon.speech.ModelDownloadFixtures.catalog
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

class DisabledModelsTest {
    @get:Rule
    val dir = TemporaryFolder()

    @Test
    fun switchedOffModelsLeaveTheSetAndTheSpeechSetStays() {
        val store = DisabledModels(File(dir.root, "d.json"))
        store.write(setOf(ModelPack.LIVE, ModelPack.AGE, ModelPack.SPEECH))
        assertEquals(setOf(ModelPack.LIVE, ModelPack.AGE), store.read())

        val manifest = store.apply(catalog().speech)
        assertFalse(manifest.hasLiveModel)
        assertFalse(manifest.hasAgeGender)
        assertTrue(manifest.hasDenoiser)
        assertTrue(ModelRole.entries.filter { !it.optional }.all { it in manifest.files })
    }

    @Test
    fun withNothingSwitchedOffTheSetIsUnchangedAndABrokenFileMeansNothing() {
        val file = File(dir.root, "d.json")
        val store = DisabledModels(file)
        assertEquals(emptySet<ModelPack>(), store.read())
        file.writeText("{oops")
        assertEquals(emptySet<ModelPack>(), store.read())
        val speech = catalog().speech
        assertEquals(speech.files.keys, store.apply(speech).files.keys)
    }
}
