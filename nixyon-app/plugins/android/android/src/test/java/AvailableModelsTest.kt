package dev.demo.nixyon.speech

import dev.demo.nixyon.speech.ModelDownloadFixtures.catalog
import org.junit.Assert.assertEquals
import org.junit.Test

class AvailableModelsTest {
    private val catalog = catalog()
    private val nothing = InstalledModels(null, null)
    private val full = InstalledModels(catalog.speech, catalog.llm)

    @Test
    fun modelsBuiltIntoTheApkCountAsInstalled() {
        assertEquals(ModelPack.entries.toSet(), AvailableModels.packs(nothing, emptySet(), full))
    }

    @Test
    fun aLightApkWithoutDownloadsHasNothing() {
        assertEquals(emptySet<ModelPack>(), AvailableModels.packs(nothing, emptySet(), nothing))
    }

    @Test
    fun aDescriptionInTheAppFolderWinsOverTheApkEvenWhenItsFilesAreBroken() {
        val local = InstalledModels(catalog.speech, null)
        assertEquals(setOf(ModelPack.LLM), AvailableModels.packs(local, emptySet(), full))
    }

    @Test
    fun downloadedSpeechAndBuiltInLanguageModelAddUp() {
        val local = InstalledModels(catalog.speech, null)
        val ready = setOf(ModelPack.SPEECH, ModelPack.LIVE)
        assertEquals(setOf(ModelPack.SPEECH, ModelPack.LIVE, ModelPack.LLM), AvailableModels.packs(local, ready, full))
    }
}
