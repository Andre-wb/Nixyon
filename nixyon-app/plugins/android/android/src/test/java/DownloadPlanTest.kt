package dev.demo.nixyon.speech

import dev.demo.nixyon.speech.ModelDownloadFixtures.catalog
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class DownloadPlanTest {
    private val catalog = catalog()
    private val nothing = InstalledModels(null, null)
    private val required = ModelRole.entries.filter { !it.optional }.toSet()

    private fun installedSpeech(roles: Set<ModelRole>, version: String = catalog.speech.version) =
        ModelManifest(version, catalog.speech.files.filterKeys { it in roles })

    // Файлы на диске нужного размера, как после удачной загрузки.
    private val onDisk: (ModelTarget, ModelFile) -> Long? = { _, file -> file.size }

    @Test
    fun aFirstDownloadTakesTheRequiredSetAndWhatWasSelected() {
        val plan = DownloadPlan.build(catalog, setOf(ModelPack.LLM, ModelPack.DENOISER), nothing) { _, _ -> null }
        val roles = plan.items.mapNotNull { it.role }.toSet()
        assertEquals(required + ModelRole.DENOISER, roles)
        assertTrue(plan.items.any { it.target == ModelTarget.LLM })
        assertEquals(required + ModelRole.DENOISER, plan.speechFiles?.keys)
        assertNotNull(plan.llm)
    }

    @Test
    fun nothingIsFetchedOrRewrittenWhenTheSelectionIsAlreadyInstalled() {
        val installed = InstalledModels(installedSpeech(required), catalog.llm)
        val plan = DownloadPlan.build(catalog, setOf(ModelPack.LLM), installed, onDisk)
        assertTrue(plan.items.isEmpty())
        assertNull(plan.speechFiles)
        assertNull(plan.llm)
    }

    @Test
    fun addingAnOptionalModelFetchesOnlyItsFilesAndKeepsWhatIsInstalled() {
        val installed = InstalledModels(installedSpeech(required + ModelRole.DENOISER), null)
        val plan = DownloadPlan.build(catalog, setOf(ModelPack.LIVE), installed, onDisk)
        assertEquals((ModelRole.LIVE + ModelRole.LIVE_EN).toSet(), plan.items.mapNotNull { it.role }.toSet())
        assertEquals(required + ModelRole.DENOISER + ModelRole.LIVE + ModelRole.LIVE_EN, plan.speechFiles?.keys)
        assertNull(plan.llm)
    }

    @Test
    fun aFileOfTheWrongSizeOnDiskIsFetchedAgain() {
        val installed = InstalledModels(installedSpeech(required), null)
        val plan = DownloadPlan.build(catalog, emptySet(), installed) { _, file ->
            if (file.path == "vad.onnx") file.size - 1 else file.size
        }
        assertEquals(listOf(ModelRole.VAD), plan.items.map { it.role })
    }

    @Test
    fun aNewCatalogVersionRewritesTheDescriptionEvenWithTheSameFiles() {
        val installed = InstalledModels(installedSpeech(required, version = "set-0"), null)
        val plan = DownloadPlan.build(catalog, emptySet(), installed, onDisk)
        assertTrue(plan.items.isEmpty())
        assertEquals(required, plan.speechFiles?.keys)
    }
}
