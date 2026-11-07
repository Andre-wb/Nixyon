package dev.demo.nixyon.speech

import dev.demo.nixyon.speech.ModelDownloadFixtures.catalog
import dev.demo.nixyon.speech.ModelDownloadFixtures.contents
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class InstalledModelsTest {
    @get:Rule
    val dir = TemporaryFolder()

    private val catalog = catalog()

    private fun install(folders: ModelFolders, roles: Set<ModelRole>, llm: Boolean) {
        val files = catalog.speech.files.filterKeys { it in roles }
        for ((role, file) in files) {
            folders.file(ModelTarget.SPEECH, file).apply { parentFile?.mkdirs() }.writeBytes(contents.getValue(role.key))
        }
        AtomicFile.write(folders.manifest(ModelTarget.SPEECH), ManifestJson.speech(catalog.speech.version, files))
        if (llm) {
            folders.file(ModelTarget.LLM, catalog.llm.model).apply { parentFile?.mkdirs() }.writeBytes(contents.getValue("llm"))
            AtomicFile.write(folders.manifest(ModelTarget.LLM), ManifestJson.llm(catalog.llm))
        }
    }

    @Test
    fun aCompleteSetIsReadyWithItsOptionalModels() {
        val folders = ModelFolders(dir.root)
        install(folders, ModelRole.entries.filter { !it.optional }.toSet() + ModelRole.DENOISER, llm = true)
        assertEquals(setOf(ModelPack.SPEECH, ModelPack.DENOISER, ModelPack.LLM), InstalledModels.read(folders).ready(folders))
    }

    @Test
    fun aTruncatedSpeechFileMakesTheSetNotReadyAndOnlyThatFileIsFetchedAgain() {
        val folders = ModelFolders(dir.root)
        install(folders, ModelRole.entries.filter { !it.optional }.toSet() + ModelRole.DENOISER, llm = true)
        val vad = catalog.speech.file(ModelRole.VAD)
        folders.file(ModelTarget.SPEECH, vad).writeBytes(ByteArray(1))

        val installed = InstalledModels.read(folders)
        assertEquals(setOf(ModelPack.LLM), installed.ready(folders))
        val plan = DownloadPlan.build(catalog, emptySet(), installed) { target, file ->
            folders.file(target, file).takeIf { it.isFile }?.length()
        }
        assertEquals(listOf(ModelRole.VAD), plan.items.map { it.role })
    }

    @Test
    fun aMissingLanguageModelFileIsNotReady() {
        val folders = ModelFolders(dir.root)
        install(folders, ModelRole.entries.filter { !it.optional }.toSet(), llm = true)
        folders.file(ModelTarget.LLM, catalog.llm.model).delete()
        assertEquals(setOf(ModelPack.SPEECH), InstalledModels.read(folders).ready(folders))
    }
}
